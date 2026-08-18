//! AST → IR: declarations, then one CFG per body.
//!
//! Two halves, and the split is the phase's shape. This file walks
//! *declarations* — the blocks, their fields, and the bodies hanging off them —
//! and [`expr`] walks *expressions*, including the condition fusion that is
//! Phase 2's exit criterion.
//!
//! Nothing here invents a label or picks a register number for a temporary. A
//! statement creates blocks and points terminators at them ([`crate::cfg`]),
//! and what a label is stays entirely inside [`crate::layout`].
//!
//! Anything outside the exposed set is [`Code::NotYetImplemented`] — the honest
//! edge of the vertical slice (PLAN §0), and deliberately a different code from
//! [`Code::UnknownField`], which means *no version will ever accept this*. The
//! first is a five-second wait and the second is a five-second fix, and
//! collapsing them is how a `todo` count stops predicting anything.

pub mod control;
mod expr;
mod handle;
mod insttype;
mod sig;

use std::collections::{BTreeMap, BTreeSet};

use crate::alloc;
use crate::ast::*;
use crate::callgraph;
use crate::cfg::{self, BlockId, Body, Terminator, Test};
use crate::diag::{Code, Diagnostic, Diagnostics, Span};
use crate::ir;
use crate::regs::Slot;
use crate::resolve::{ConstValue, DeferredKind, Resolved};
use crate::table;
use crate::types::Ty;

pub use sig::{Inferred, Signature};

/// The `attributes {}` surface, read from the census rather than frozen here: a
/// name is an attribute exactly when a [`table::Class::Attribute`] row claims
/// it, which is what makes a new setting one overlay line (§15.23).
///
/// One exclusion: the dotted names are `versionInfo`'s members and are reached
/// through it, never written flat.
///
/// The overlap with [`V1_INSTALLER_FIELDS`] is not one. Every name in that list
/// is a script-wide NSIS command that `installer {}` also accepts, so they
/// belong to both blocks.
fn attribute_names() -> Vec<&'static str> {
    table::table()
        .iter()
        .filter(|entry| matches!(entry.class, table::Class::Attribute(_)))
        .filter_map(|entry| entry.installua)
        .filter(|field| !field.contains('.'))
        .collect()
}

/// The frozen v1 `installer {}` / `uninstaller {}` field surface.
///
/// The last four look page-scoped and are not. MUI2 writes each of them inside
/// an `!ifndef`-guarded `MUI_*PAGE_INTERFACE` macro, which runs on the **first**
/// page of its type and never again — so a `checkBitmap` on a second components
/// page would be read by nothing, and the block is the only home where that
/// cannot be written.
///
/// The pages themselves are not fields at all: `page.directory { … }` is a
/// positional entry, symmetric with `section(…)`, because a page has a body of
/// settings and a field would have to grow one.
const V1_INSTALLER_FIELDS: &[&str] = &[
    "installDir",
    "icon",
    "installTypes",
    "caption",
    "checkBitmap",
    "installColors",
    "progressBar",
    "licenseBkColor",
];

/// The fields NSIS reads once for the whole script, so only `installer {}` has
/// them: written in both blocks they would define one name twice, which is a
/// redefinition warning and so an error under `-WX` (§14 tier 3).
///
/// `icon` is not one of these. It is two defines — `MUI_ICON` and
/// `MUI_UNICON` — and is genuinely the same field for the other half (§15.3).
const ONCE_GLOBAL_FIELDS: &[&str] = &[
    "installDir",
    "checkBitmap",
    "installColors",
    "progressBar",
    "licenseBkColor",
];

/// NSIS numbers install types one to thirty-two and rejects anything else
/// outright — `SectionIn 0 out of range 1..32` — so the ceiling is the format's
/// and not a policy of this compiler's.
const MAX_INST_TYPES: usize = 32;

/// Which of the two halves a declaration belongs to (§15.3).
///
/// NSIS spells the difference as a `un.` prefix on function and section names
/// and a `MUI_UNPAGE_` prefix on page macros; Installua spells it as which
/// block the code was written in, and this type is the whole of the
/// translation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Half {
    Installer,
    Uninstaller,
}

impl Half {
    fn prefix(self) -> &'static str {
        match self {
            Half::Installer => "",
            Half::Uninstaller => "un.",
        }
    }

    fn page_prefix(self) -> &'static str {
        match self {
            Half::Installer => "MUI_PAGE_",
            Half::Uninstaller => "MUI_UNPAGE_",
        }
    }
}

impl std::fmt::Display for Half {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Half::Installer => f.write_str("installer"),
            Half::Uninstaller => f.write_str("uninstaller"),
        }
    }
}

/// A deferred section or group, and the block that listed it.
#[derive(Clone, Debug)]
struct Claim {
    /// Which executable it ended up in. A handle read from the other half names
    /// a section that does not exist there, and this is what catches it.
    half: Half,
    /// Where it was listed, so a second claim can point at the first.
    span: Span,
    /// What listed it, so the second claim's message names the right construct.
    site: Site,
}

/// The define a claimed section is addressed through.
///
/// Derived from the *local's* name rather than the section's, because the local
/// is the name that is unique: two sections may both be called `"Core"`, and one
/// of them is the uninstaller's. `UN` leads that half's for the same reason NSIS
/// puts `un.` on the section itself — one `.nsi` holds both, and one `!define`
/// twice is a redefinition warning and an error under `-WX`.
fn index_name(local: &str, half: Half) -> String {
    match half {
        Half::Installer => format!("SEC_{local}"),
        Half::Uninstaller => format!("UNSEC_{local}"),
    }
}

/// The `Var` a claimed control's handle lives in.
///
/// A `Var` and not a register, because a plugin call clobbers every one of them
/// (§15.11) and the handle has to survive from the creator into `leave` — two
/// NSIS functions, with the whole page in between (ruling 7). The allocator
/// never sees it, which is exactly what [`Slot::Global`] means.
///
/// Named from the local for the same reason a section's define is, and prefixed
/// like a generated label because it is one more name in the `Var` namespace the
/// author also writes in.
fn control_var(local: &str, half: Half) -> String {
    match half {
        Half::Installer => format!("{}ctl_{local}", cfg::LABEL_PREFIX),
        Half::Uninstaller => format!("{}unctl_{local}", cfg::LABEL_PREFIX),
    }
}

/// What a bare name is being listed by, and so which declarations it may name.
///
/// The claim rules are one set of rules over two constructs: a block lists
/// sections and groups, a page's `controls` lists controls, and everything after
/// that — listed twice, listed by both halves, listed by nothing — is the same
/// four checks in the same words (ruling 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Site {
    /// `installer {}` / `uninstaller {}`, or a `group`'s sections, which is the
    /// block's list one level down.
    Block,
    /// `page.custom { controls = { … } }`.
    Controls,
}

impl Site {
    fn accepts(self, kind: DeferredKind) -> bool {
        match self {
            Site::Block => !kind.is_control(),
            Site::Controls => kind.is_control(),
        }
    }

    /// What this site lists, for the two diagnostics that have to say.
    fn lists(self) -> &'static str {
        match self {
            Site::Block => "a section or a group",
            Site::Controls => "a control",
        }
    }

    fn how(self) -> &'static str {
        match self {
            Site::Block => {
                "a bare name here lists a declaration: bind one with `local x = section { … }` (§13)"
            }
            Site::Controls => {
                "a bare name here lists a declaration: bind one with `local x = text { … }` (§13)"
            }
        }
    }
}

/// A field of a section handle, and the instruction pair behind it.
///
/// Seven fields and four pairs, because `Sections.nsh` names seven bits and NSIS
/// exposes all of them through one `SectionGetFlags`/`SectionSetFlags` — handing
/// a user that integer means handing them `IntOp` and `${SECTION_OFF}`, so the
/// bit is the compiler's and the field is the surface (ruling 6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HandleField {
    /// One bit of the flags word. `shift` is `bit.trailing_zeros()`, kept beside
    /// it because a read normalises to `0`/`1` and a write has to put the value
    /// back where it came from.
    Flag {
        bit: u32,
        shift: u32,
        on: Where,
    },
    Text,
    Size,
    InstallTypes,
}

/// Which handles a field is on. `expanded` is a heading's, and `size` and
/// `installTypes` are a section's — a group has neither, since what it holds is
/// sections and each of those answers for itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Where {
    Both,
    Sections,
    Groups,
}

impl Where {
    fn accepts(self, kind: DeferredKind) -> bool {
        match (self, kind) {
            // A control's fields are its own and none of these are among them.
            (_, DeferredKind::Control(_)) => false,
            (Where::Both, _) => true,
            (Where::Sections, kind) => kind == DeferredKind::Section,
            (Where::Groups, kind) => kind == DeferredKind::Group,
        }
    }
}

/// The seven fields, by the name a program writes.
///
/// `SF_SECGRP` and `SF_SECGRPEND` are absent because they say what an index *is*
/// rather than what a user may change, and `SF_PSELECTED`, `SF_TOGGLED` and
/// `SF_NAMECHG` because `Sections.nsh` marks them internal.
fn handle_field(name: &str) -> Option<HandleField> {
    Some(match name {
        // SF_SELECTED
        "selected" => HandleField::Flag {
            bit: 1,
            shift: 0,
            on: Where::Both,
        },
        // SF_BOLD
        "bold" => HandleField::Flag {
            bit: 8,
            shift: 3,
            on: Where::Both,
        },
        // SF_RO
        "readOnly" => HandleField::Flag {
            bit: 16,
            shift: 4,
            on: Where::Both,
        },
        // SF_EXPAND
        "expanded" => HandleField::Flag {
            bit: 32,
            shift: 5,
            on: Where::Groups,
        },
        "text" => HandleField::Text,
        "size" => HandleField::Size,
        "installTypes" => HandleField::InstallTypes,
        _ => return None,
    })
}

/// The field names, for the error that has to list them.
const HANDLE_FIELDS: &[&str] = &[
    "selected",
    "bold",
    "readOnly",
    "expanded",
    "text",
    "size",
    "installTypes",
];

/// Whether a string is a measurement nsDialogs will read: a whole number, with
/// an optional `-` in front and an optional `u` or `%` after.
///
/// Checked rather than passed through, because everything nsDialogs does not
/// understand it reads as **0 pixels** — a control that is there, is the right
/// size, and sits in the corner. A typo in a width should not be a page that
/// looks broken at run time.
fn is_measurement(text: &str) -> bool {
    let digits = text
        .strip_prefix('-')
        .unwrap_or(text)
        .trim_end_matches(['u', '%']);
    // One suffix at most, and it is the last character.
    let suffix = text.len() - text.trim_end_matches(['u', '%']).len();
    !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()) && suffix <= 1
}

/// One control, lowered: what the creator writes for it, in the order it writes
/// it.
///
/// A struct rather than a list of instructions because the handle is not known
/// until the creator runs — a claimed control's is its `Var` and an unclaimed
/// one's is whatever register the allocator gives it — and the `items` after it
/// address that handle.
struct Created {
    /// `nsDialogs::CreateControl`'s eight arguments: class, style, extended
    /// style, x, y, width, height, text.
    create: Vec<ir::Arg>,
    /// The `Var` the handle is popped into, or `None` for a control the program
    /// never names.
    var: Option<String>,
    /// The `ADDSTRING` message for this control's class, where it has one.
    add_item: Option<u32>,
    /// One `"STR:…"` per item, already in the form `SendMessage` wants.
    items: Vec<ir::Arg>,
    span: Span,
}

/// A `group`'s member list, in either of §15.23's two forms and without judging
/// the call. The shape errors belong to [`Lowerer::group`], which reports them
/// once where the group is lowered; this is the claim pass looking for the bare
/// names inside.
/// A `page.custom`'s `controls = { … }`, without judging the page. The shape
/// errors belong to [`Lowerer::custom_page`]; this is the claim pass looking for
/// the bare names inside.
fn page_controls(value: &Expr) -> Option<&[TableField]> {
    let Expr::Call { args, .. } = value else {
        return None;
    };
    let [Expr::Table { fields, .. }] = args.as_slice() else {
        return None;
    };
    let controls = fields.iter().find_map(|field| match field {
        TableField::Named { name, value } if name.text == "controls" => Some(value),
        _ => None,
    })?;
    match controls {
        Expr::Table { fields, .. } => Some(fields),
        _ => None,
    }
}

fn group_members(value: &Expr) -> Option<&[TableField]> {
    let Expr::Call { args, .. } = value else {
        return None;
    };
    let members = match args.as_slice() {
        [_, members @ Expr::Table { .. }] => members,
        [Expr::Table { fields, .. }] => fields.iter().find_map(|field| match field {
            TableField::Named { name, value } if name.text == "sections" => Some(value),
            _ => None,
        })?,
        _ => return None,
    };
    match members {
        Expr::Table { fields, .. } => Some(fields),
        _ => None,
    }
}

/// What one field of a `page.* {}` table holds.
#[derive(Clone, Copy, Debug)]
enum Holds {
    Str,
    /// `true` defines it and `false` does not: the setting **is** the define's
    /// existence — MUI2 asks `!ifdef` and never expands it — so there is no
    /// value to write and no third state to have.
    Flag,
    /// A global, named bare: `variable = target`. NSIS wants a *variable* in
    /// this position rather than a value, because `DirVar` stores into it, so a
    /// string would be the wrong kind of thing even where it reads alike.
    Var,
    /// `function() … end`, lowered to an NSIS `Function` MUI2 calls by name.
    Callback,
    /// A string that also turns its setting on: `checkbox = "I accept"` is
    /// `MUI_LICENSEPAGE_CHECKBOX` *and* `…_CHECKBOX_TEXT`, because MUI2 asks
    /// `!ifdef` about the first and expands the second. The payload is the
    /// second, and it is cleared exactly when the first is.
    Text(&'static str),
    /// A table whose keys are further defines, and whose presence turns the
    /// setting on the way [`Holds::Text`] does.
    Nested(&'static [PageField]),
}

/// One setting of one page: the name a user writes, the `MUI_*` define it
/// becomes, and what it holds.
///
/// The NSIS line is absent on purpose. `DirText`, `ComponentText` and
/// `LicenseText` are written by MUI2, from these defines, inside the `PageEx`
/// it generates; a second one written by us assembles clean under `-WX` and
/// then loses the race (§15.7). What is private to MUI2 is the **line**, and
/// what stays public is the **setting** — the split `icon`/`MUI_ICON` already
/// lives on.
#[derive(Debug)]
struct PageField {
    installua: &'static str,
    define: &'static str,
    holds: Holds,
    /// Whether MUI2 clears the define once the page is inserted. `false` means
    /// the compiler emits the `!undef` itself, and that is not an alternative
    /// to MUI2's cleanup but the two holes in it: `UninstallConfirm.nsh` never
    /// clears `MUI_UNCONFIRMPAGE_VARIABLE`, and `License.nsh` clears
    /// `MUI_LICENSEPAGE_CHECKBOX_TEXT_ACCEPT`, which is a name nothing defines.
    /// Without this a second page of the same type inherits the first one's.
    cleared: bool,
}

/// The one field a page has that is not a [`PageField`], because it is not a
/// `!define` at all.
///
/// Two of the eight have one. `license`'s `file` is an argument of the
/// `!insertmacro` rather than a setting read from inside it, and `custom`'s
/// `controls` is what the compiler draws — no define exists for either, so
/// there is nothing for the table to hold and they are named here instead.
fn extra_field(page: &Page) -> Option<&'static str> {
    match page.installua {
        "license" => Some("file"),
        "custom" => Some("controls"),
        _ => None,
    }
}

const fn field(installua: &'static str, define: &'static str, holds: Holds) -> PageField {
    PageField {
        installua,
        define,
        holds,
        cleared: true,
    }
}

/// The same, for a define MUI2 leaves standing.
const fn sticky(installua: &'static str, define: &'static str, holds: Holds) -> PageField {
    PageField {
        installua,
        define,
        holds,
        cleared: false,
    }
}

/// The three hooks every page has. MUI2 clears all three itself, in
/// `MUI_PAGE_FUNCTION_CUSTOM`.
const COMMON_FIELDS: &[PageField] = &[
    field("pre", "MUI_PAGE_CUSTOMFUNCTION_PRE", Holds::Callback),
    field("show", "MUI_PAGE_CUSTOMFUNCTION_SHOW", Holds::Callback),
    field("leave", "MUI_PAGE_CUSTOMFUNCTION_LEAVE", Holds::Callback),
];

/// The bold heading strip inside the page — not the title bar, which is
/// `caption` on the block, and not the page's own body text.
///
/// Absent from `welcome` and `finish`: those are full-window pages with no
/// header to write into, and `Pages.nsh` calls `MUI_HEADER_TEXT_PAGE` from the
/// other five and not from them.
const HEADER_FIELDS: &[PageField] = &[
    field("headerText", "MUI_PAGE_HEADER_TEXT", Holds::Str),
    field("headerSubText", "MUI_PAGE_HEADER_SUBTEXT", Holds::Str),
];

const LICENSE_FIELDS: &[PageField] = &[
    field("bottomText", "MUI_LICENSEPAGE_TEXT_BOTTOM", Holds::Str),
    field("button", "MUI_LICENSEPAGE_BUTTON", Holds::Str),
    field(
        "checkbox",
        "MUI_LICENSEPAGE_CHECKBOX",
        Holds::Text("MUI_LICENSEPAGE_CHECKBOX_TEXT"),
    ),
    // The two texts are `sticky` because of a typo in MUI2: `License.nsh`
    // unsets `MUI_LICENSEPAGE_CHECKBOX_TEXT_ACCEPT` and `…_DECLINE`, which are
    // not names anything defines — the radio button texts are spelled
    // `MUI_LICENSEPAGE_RADIOBUTTONS_TEXT_*` — so they survive the page and
    // `MUI_DEFAULT` on the next one declines to overwrite them.
    field(
        "radioButtons",
        "MUI_LICENSEPAGE_RADIOBUTTONS",
        Holds::Nested(&[
            sticky(
                "accept",
                "MUI_LICENSEPAGE_RADIOBUTTONS_TEXT_ACCEPT",
                Holds::Str,
            ),
            sticky(
                "decline",
                "MUI_LICENSEPAGE_RADIOBUTTONS_TEXT_DECLINE",
                Holds::Str,
            ),
        ]),
    ),
];

const COMPONENTS_FIELDS: &[PageField] = &[
    field("topText", "MUI_COMPONENTSPAGE_TEXT_TOP", Holds::Str),
    field(
        "instTypeText",
        "MUI_COMPONENTSPAGE_TEXT_INSTTYPE",
        Holds::Str,
    ),
    field("listText", "MUI_COMPONENTSPAGE_TEXT_COMPLIST", Holds::Str),
];

const DIRECTORY_FIELDS: &[PageField] = &[
    field("topText", "MUI_DIRECTORYPAGE_TEXT_TOP", Holds::Str),
    field(
        "destinationText",
        "MUI_DIRECTORYPAGE_TEXT_DESTINATION",
        Holds::Str,
    ),
    field("variable", "MUI_DIRECTORYPAGE_VARIABLE", Holds::Var),
    field(
        "verifyOnLeave",
        "MUI_DIRECTORYPAGE_VERIFYONLEAVE",
        Holds::Flag,
    ),
];

const CONFIRM_FIELDS: &[PageField] = &[
    field("topText", "MUI_UNCONFIRMPAGE_TEXT_TOP", Holds::Str),
    field(
        "locationText",
        "MUI_UNCONFIRMPAGE_TEXT_LOCATION",
        Holds::Str,
    ),
    sticky("variable", "MUI_UNCONFIRMPAGE_VARIABLE", Holds::Var),
];

/// One MUI2 page, and the settings that belong to it rather than to the block.
///
/// A declaration written in §15.23's table form, taken apart: the array part is
/// the parameters and the hash part the options, so what comes out is one name,
/// the thing the construct encloses, and the switches beside them.
struct Declaration<'e> {
    /// The one positional entry.
    name: &'e Expr,
    /// A section's `body`, a group's `sections` — named, because NSIS does not
    /// pass it either.
    holds: &'e Expr,
    options: Vec<(&'e Name, &'e Expr)>,
}

/// Which of the two a setting is is MUI2's own source to say and not a
/// judgement: a setting written inside the generated `PageEx` and `!undef`'d
/// after is page-scoped, and one written inside an `!ifndef`-guarded
/// `MUI_*PAGE_INTERFACE` macro is applied once, on the first page of its type,
/// and ignored on every later one. The second kind is a block field — see
/// [`V1_INSTALLER_FIELDS`] — because putting it on the page would be a lie the
/// second page tells silently.
struct Page {
    installua: &'static str,
    nsis: &'static str,
    /// Which halves MUI2 defines a macro for. `confirm` exists only as
    /// `MUI_UNPAGE_CONFIRM`, and there is no `MUI_PAGE_CONFIRM` to fall back
    /// on — so this is a fact about MUI2 rather than a policy of ours.
    halves: [bool; 2],
    header: bool,
    /// Whether the page is one NSIS inserts (`!insertmacro MUI_PAGE_*`) or one
    /// the compiler writes the body of (`Page custom`). Exactly one page is the
    /// second kind, and everything that differs about it follows from this:
    /// there is no MUI2 macro to configure, so the settings that are `!define`s
    /// on the other seven are arguments and instructions here.
    custom: bool,
    own: &'static [PageField],
}

const fn page(installua: &'static str, nsis: &'static str, own: &'static [PageField]) -> Page {
    Page {
        installua,
        nsis,
        halves: [true, true],
        header: true,
        custom: false,
        own,
    }
}

/// The eight pages, as a **closed set**: this is why a page is reached by
/// member access (`page.directory`) where a section is reached by string
/// (`section("Tools", …)`). A user picks a section's name and MUI2 picks
/// these, so one completes and the other cannot (§15.1).
///
/// Seven of them are MUI2's and the eighth is not, and it is still in the same
/// list for the same reason: what a user picks from is a set an editor can
/// finish, and where the page's body comes from is not a fact about the name.
const V1_PAGES: &[Page] = &[
    Page {
        installua: "welcome",
        nsis: "WELCOME",
        halves: [true, true],
        header: false,
        custom: false,
        own: &[],
    },
    page("license", "LICENSE", LICENSE_FIELDS),
    page("components", "COMPONENTS", COMPONENTS_FIELDS),
    page("directory", "DIRECTORY", DIRECTORY_FIELDS),
    page("instFiles", "INSTFILES", &[]),
    Page {
        installua: "finish",
        nsis: "FINISH",
        halves: [true, true],
        header: false,
        custom: false,
        own: &[],
    },
    Page {
        installua: "confirm",
        nsis: "CONFIRM",
        halves: [false, true],
        header: true,
        custom: false,
        own: CONFIRM_FIELDS,
    },
    // The eighth, and the only one that is not MUI2's. It is reached by the
    // same member access as the other seven because it is a *page* — the set
    // stays closed, and what a user picks is still from a list an editor can
    // complete (§15.1). What it is not is a `!insertmacro`: `Page custom` names
    // two functions, and both of them are ours to write (§15.32).
    Page {
        installua: "custom",
        nsis: "custom",
        halves: [true, true],
        header: true,
        custom: true,
        own: &[],
    },
];

impl Page {
    fn has(&self, half: Half) -> bool {
        self.halves[half as usize]
    }

    /// Every field this page accepts, in the order the defines are emitted —
    /// which is this order and not the user's, because a Lua table has none
    /// (§12).
    fn fields(&self) -> impl Iterator<Item = &'static PageField> {
        let header: &'static [PageField] = if self.header { HEADER_FIELDS } else { &[] };
        self.own.iter().chain(header).chain(COMMON_FIELDS)
    }
}

/// The frozen v1 block surface, for the same reason.
const V1_BLOCKS: &[&str] = &[
    "attributes",
    "installer",
    "uninstaller",
    "languages",
    "func",
    "import",
    "plugin",
    "include",
];

/// How many rounds the signature fixpoint gets. Three is enough for the deepest
/// chain in the five programs — a caller learns a parameter, the callee learns
/// its return, the caller reads it — and the cap exists because a lattice with
/// `Unknown` at the top is not strictly monotone: a disagreement can flip a slot
/// back. A program that has not settled by then compiles against the last round,
/// which is sound: an unsettled type is `Unknown`, and `Unknown` is refused at
/// every point where guessing would matter (§15.14).
const MAX_ROUNDS: usize = 8;

pub fn lower(
    program: &Program,
    resolved: &Resolved<'_>,
    options: &crate::Options,
    diags: &mut Diagnostics,
) -> ir::Module {
    // 1. Types, to a fixpoint. Rounds before the last are lowered against a
    //    scratch collector: their diagnostics are about a type table that was
    //    still incomplete, so reporting them would be reporting the compiler's
    //    intermediate state to the user (§9-4).
    let mut inferred = Inferred::seed(resolved);
    for _ in 0..MAX_ROUNDS {
        let mut scratch = Diagnostics::new();
        let round = lower_once(program, resolved, options, &mut scratch, &inferred).1;
        if round == inferred {
            break;
        }
        inferred = round;
    }

    let (mut module, _) = lower_once(program, resolved, options, diags, &inferred);

    // 2. Registers. Every body is allocated before any call site is filled in,
    //    because a clobber set is a fact about *physical* registers and there
    //    are none until colouring has run (§9-3).
    let mut across = Vec::new();
    let mut direct: BTreeMap<String, BTreeSet<u8>> = BTreeMap::new();
    let functions = module.functions.len();
    for (index, (name, body)) in module.bodies_mut().into_iter().enumerate() {
        let allocation = alloc::allocate(body, diags);
        across.push(allocation.live_across);
        // `bodies_mut` yields the functions first, and only a function can be
        // called: a section is a root.
        if index < functions {
            direct.insert(name, allocation.clobbers);
        }
    }

    // 3. The call graph, built once and read twice here (§15.11).
    let graph = callgraph::build(&module);
    let clobbers = graph.clobbers(&direct);
    graph.lint_recursion(diags);

    // 4. `live ∩ clobbered`, at last.
    for (index, (_, body)) in module.bodies_mut().into_iter().enumerate() {
        alloc::insert_saves(body, &across[index], &clobbers);
    }

    module
}

fn lower_once(
    program: &Program,
    resolved: &Resolved<'_>,
    options: &crate::Options,
    diags: &mut Diagnostics,
    known: &Inferred,
) -> (ir::Module, Inferred) {
    let mut lowerer = Lowerer {
        diags,
        resolved,
        options,
        module: ir::Module::new(),
        globals: known
            .globals
            .iter()
            .map(|(name, ty)| (name.clone(), (*ty, Vec::new())))
            .collect(),
        known,
        learned: Inferred::seed(resolved),
        attributes_span: None,
        installer_span: None,
        uninstaller_span: None,
        mui: false,
        global_inits: Vec::new(),
        claims: BTreeMap::new(),
        on_init: false,
        requires: Requirements::default(),
    };
    lowerer.program(program);
    lowerer.finish()
}

/// A global's agreed type, and every site that agreed on it. §15.24 checks
/// across all assignments rather than pinning to the declaring one, because
/// §15.6 makes the language order-free and "the declaring assignment" is
/// therefore arbitrary.
type GlobalTypes = BTreeMap<String, (Ty, Vec<Span>)>;

struct Lowerer<'a, 'p> {
    diags: &'a mut Diagnostics,
    resolved: &'a Resolved<'p>,
    options: &'a crate::Options,
    module: ir::Module,
    globals: GlobalTypes,
    /// The previous round's type table: read, never written.
    known: &'a Inferred,
    /// This round's: written, never read.
    learned: Inferred,
    attributes_span: Option<Span>,
    installer_span: Option<Span>,
    uninstaller_span: Option<Span>,
    /// Whether anything asked for MUI2. Pages are the only thing that does in
    /// v1, and `!include "MUI2.nsh"` plus `MUI_LANGUAGE` follow from it.
    mui: bool,
    /// Top-level assignments, waiting for the `.onInit` they belong in.
    global_inits: Vec<Stmt>,
    /// Deferred sections and groups that a block has listed, by the local name
    /// they were bound to. A claim records which half listed it and where; the
    /// define it is addressed through is [`index_name`] of the two, and so is
    /// not stored beside them.
    claims: BTreeMap<String, Claim>,
    /// Whether an `.onInit` was written, so that one is not invented twice.
    on_init: bool,
    /// What the program needs included and initialised. Collected during
    /// lowering and emitted at the top, which is the only order that works
    /// (§15.21).
    requires: Requirements,
}

/// The collect-then-emit pass §15.21 asks for.
///
/// Both halves are sets: `import "FileFunc"` twice is one `!include`, and two
/// calls to `string.upper` are one `${Using:StrFunc} StrCase`. The second is
/// load-bearing rather than tidy — a missing `${Using:StrFunc}` line aborts the
/// build, and a duplicate one is a redefinition warning, so neither "always
/// emit" nor "never emit" is available.
#[derive(Debug, Default)]
pub struct Requirements {
    /// Headers to `!include`, by name and without the extension.
    pub headers: BTreeSet<String>,
    /// `StrFunc` functions to declare, by macro name: `StrCase`, `StrLoc`.
    pub str_func: BTreeSet<String>,
}

impl Requirements {
    /// Records that a `StrFunc` macro is used, and the header that carries it.
    pub fn str_func(&mut self, name: &str) {
        self.headers.insert("StrFunc".to_string());
        self.str_func.insert(name.to_string());
    }
}

impl<'p> Lowerer<'_, 'p> {
    fn program(&mut self, program: &Program) {
        // A top-level `<const>` is a `!define` (§7-1): build-time, folded in
        // every expression, and `${NAME}` in the output. Emitted in source
        // order because the preprocessor is textual and strictly sequential —
        // the one part of an NSIS script where order is semantics (§12).
        self.module.defines = self
            .resolved
            .const_order
            .iter()
            .filter_map(|name| {
                let value = self.resolved.consts.get(name)?;
                Some(ir::Define {
                    name: name.clone(),
                    value: Some(ir::Arg::str(value.value.text())),
                })
            })
            .collect();

        // `import "FileFunc"` is an `!include`, deduplicated against every
        // other import and against the ones an adapter pulls in on its own
        // (§15.21). A `plugin` needs no line at all: NSIS finds it by name.
        for namespace in self.resolved.namespaces.values() {
            if let crate::resolve::Namespace::Header(header) = namespace {
                self.requires.headers.insert(header.clone());
            }
        }

        // A bare assignment at the top level declares a global *and* gives it
        // a value (§15.24), and a `Var` has no initialiser — so the assignments
        // become the first lines of `.onInit`, which is the one body NSIS
        // guarantees runs before anything else. Collected here and lowered when
        // the callback is, since the block they belong to may be written above
        // them and §15.6 makes that legal.
        self.global_inits = program
            .block
            .iter()
            .filter(|stmt| matches!(stmt, Stmt::Assign { .. }))
            .cloned()
            .collect();

        // Which block listed which declaration, decided before anything is
        // lowered. A body can address a section the block lists *after* it —
        // `installer { onInit(…), core }` is ordinary — and the claim is what
        // says which half a handle names, so the map has to be complete before
        // the first body is walked (§15.6).
        self.claim_pass(program);

        for stmt in &program.block {
            self.top_level(stmt);
        }

        // Claim rule 1: a declaration no block listed. This is the
        // `installer { license = … }` bug batch 20 removed — data written at one
        // level that evaporates if nothing reads it — and the answer is the same
        // one: say so rather than emit nothing.
        let resolved = self.resolved;
        for local in &resolved.deferred_order {
            if self.claims.contains_key(local) {
                continue;
            }
            let Some(deferred) = resolved.deferred.get(local) else {
                continue;
            };
            let what = deferred.kind.word();
            let (lists, write, order) = if deferred.kind.is_control() {
                (
                    "page",
                    format!("write `{local},` among the `controls` of a `page.custom {{}}`"),
                    "the list's order is the tab order; the declaration's is nothing (§15.32)",
                )
            } else {
                (
                    "block",
                    format!(
                        "write `{local},` among the entries of `installer {{}}` or \
                         `uninstaller {{}}`"
                    ),
                    "the block's order is the install order; the declaration's is nothing (§13)",
                )
            };
            self.diags.push(
                Diagnostic::error(
                    Code::MissingAttribute,
                    deferred.span,
                    format!("`{local}` is a `{what}` no {lists} lists"),
                )
                .note(write)
                .note(order),
            );
        }

        // Nothing declared an `.onInit`, and there are globals to initialise:
        // the callback exists to hold them.
        if !self.global_inits.is_empty() && !self.on_init {
            let inits = std::mem::take(&mut self.global_inits);
            let span = inits.first().map(Stmt::span).unwrap_or_default();
            let body = self.body(&inits, &[], span, None, Some(Half::Installer));
            self.module.functions.push(ir::Function {
                name: ".onInit".to_string(),
                body,
            });
        }
        // Globals in first-seen order, emitted before the first body that
        // touches them (§12) — which the field order in `ir::Module` already
        // guarantees.
        self.module.vars = self
            .resolved
            .globals
            .iter()
            .map(|global| global.name.clone())
            .collect();
        // And one more `Var` per claimed control, because a handle outlives the
        // function that popped it (ruling 7). Only the claimed ones: a control
        // listed inline and bound to no `local` is addressed by nothing, so a
        // ten-control page costs the handful of `Var`s the program names.
        for local in &self.resolved.deferred_order {
            let Some(deferred) = self.resolved.deferred.get(local) else {
                continue;
            };
            if let (true, Some(claim)) = (deferred.kind.is_control(), self.claims.get(local)) {
                self.module.vars.push(control_var(local, claim.half));
            }
        }
    }

    fn finish(mut self) -> (ir::Module, Inferred) {
        // `!include`s, deduplicated and ordered: `MUI2.nsh` first because it is
        // the one header whose macros the others must not shadow, then the rest
        // alphabetically. Alphabetical rather than first-imported so that
        // moving an `import` line does not rewrite a golden (§14) — headers are
        // independent, unlike `!define`s, so there is nothing to preserve.
        if self.mui {
            self.module.includes.push("MUI2.nsh".to_string());
            self.module.languages.push(ir::Instruction::new(
                "!insertmacro",
                vec![ir::Arg::raw("MUI_LANGUAGE"), ir::Arg::str("English")],
            ));
        }
        self.module.includes.extend(
            self.requires
                .headers
                .iter()
                .map(|header| format!("{header}.nsh")),
        );
        // `${Using:StrFunc} StrCase` — one line per function actually reached,
        // after the `!include` and before anything that calls it (§15.21).
        self.module.inits.extend(
            self.requires
                .str_func
                .iter()
                .map(|name| ir::Instruction::new("${Using:StrFunc}", vec![ir::Arg::raw(name)])),
        );

        self.learned.globals = self
            .globals
            .iter()
            .map(|(name, (ty, _))| (name.clone(), *ty))
            .collect();
        (self.module, self.learned)
    }

    fn top_level(&mut self, stmt: &Stmt) {
        // A top-level `<const>` is a build-time value that folds at every use,
        // and `resolve` has already dealt with it. Anything else `local` was
        // rejected there too.
        if matches!(stmt, Stmt::Local { .. }) {
            return;
        }

        // Already dealt with: a top-level assignment initialises a global, and
        // its statements are lowered into `.onInit` rather than here.
        if matches!(stmt, Stmt::Assign { .. }) {
            return;
        }

        let Stmt::Call(call) = stmt else {
            self.todo(stmt.span(), "this declaration");
            return;
        };
        let Some((name, span)) = call.callee_name().map(|name| (name, call.span())) else {
            self.todo(call.span(), "this call");
            return;
        };

        match name {
            "attributes" => {
                let Some(fields) = self.block_fields(call) else {
                    return;
                };
                if self.duplicate(&mut Field::Attributes, span) {
                    return;
                }
                self.attributes(fields, span);
            }
            "installer" => {
                let Some(fields) = self.block_fields(call) else {
                    return;
                };
                if self.duplicate(&mut Field::Installer, span) {
                    return;
                }
                self.installer(fields, Half::Installer);
            }
            "uninstaller" => {
                let Some(fields) = self.block_fields(call) else {
                    return;
                };
                if self.duplicate(&mut Field::Uninstaller, span) {
                    return;
                }
                self.installer(fields, Half::Uninstaller);
            }
            "func" => self.function(name, call),
            other if V1_BLOCKS.contains(&other) => {
                self.todo(span, &format!("`{other}`"));
            }
            other => {
                self.diags.push(
                    Diagnostic::error(
                        Code::UnknownField,
                        span,
                        format!("`{other}` is not a declaration"),
                    )
                    .note(format!("the declarations are {}", list(V1_BLOCKS))),
                );
            }
        }
    }

    /// Both `attributes { … }` and `attributes({ … })` are the same call in Lua,
    /// so both arrive here as one table argument.
    fn block_fields<'e>(&mut self, call: &'e Expr) -> Option<&'e [TableField]> {
        let Expr::Call { args, .. } = call else {
            return None;
        };
        match args.as_slice() {
            [Expr::Table { fields, .. }] => Some(fields.as_slice()),
            _ => {
                self.todo(call.span(), "this block in this form");
                None
            }
        }
    }

    fn duplicate(&mut self, which: &mut Field, span: Span) -> bool {
        let (slot, name) = match which {
            Field::Attributes => (&mut self.attributes_span, "attributes"),
            Field::Installer => (&mut self.installer_span, "installer"),
            Field::Uninstaller => (&mut self.uninstaller_span, "uninstaller"),
        };
        if let Some(previous) = *slot {
            let previous = previous.start_line;
            self.diags.push(
                Diagnostic::error(
                    Code::DuplicateBlock,
                    span,
                    format!("`{name} {{}}` appears more than once"),
                )
                .note(format!("the first one is at line {previous}"))
                .note("it is script-global, so there is exactly one (§2)"),
            );
            return true;
        }
        *slot = Some(span);
        false
    }

    // -- attributes -------------------------------------------------------

    fn attributes(&mut self, fields: &[TableField], span: Span) {
        // `SetCompressor` first, whatever the author wrote first. NSIS refuses
        // it *after the header has changed* — "can't change compressor after
        // data already got compressed or header already changed!" — and
        // `AddBrandingImage` changes the header, so `{ brandingImage = …,
        // compressor = "lzma" }` would fail and the same table written the
        // other way round would not. A Lua table has no order (§12), so the
        // order is the compiler's, exactly as `VIProductVersion` before
        // `VIAddVersionKey` is in [`Self::version_info`].
        let mut fields: Vec<&TableField> = fields.iter().collect();
        fields.sort_by_key(|field| match field {
            TableField::Named { name, .. } if name.text == "compressor" => 0,
            _ => 1,
        });

        for field in fields {
            let TableField::Named { name, value } = field else {
                self.todo(span, "a positional entry in `attributes {}`");
                continue;
            };

            match name.text.as_str() {
                // The two nested ones. `versionInfo` is a table of its own and
                // `unicode` emits nothing — it sets a field the emitter reads
                // first, so a later `raw` can override it (§15.16).
                "versionInfo" => self.version_info(value),
                "unicode" => match self.constant(value) {
                    Some(ConstValue::Bool(value)) => self.module.unicode = value,
                    _ => self.bad_value(
                        value.span(),
                        "unicode",
                        "a `bool`",
                        "write `unicode = true`; NSIS's charset otherwise depends on how the \
                         local `makensis` was built, which is why it is always emitted (§15.16)",
                    ),
                },
                other => match table::by_installua(other) {
                    Some(entry) => self.setting(entry, &name.text, value),
                    // A name that belongs to the other block is a five-second
                    // fix rather than a five-second wait, so it says which
                    // block rather than which version (§9-4). Reached only by
                    // `pages` and `text`: the other four installer fields are
                    // `Attribute` rows and NSIS lets them be set script-wide.
                    None if V1_INSTALLER_FIELDS.contains(&other) => {
                        self.diags.push(
                            Diagnostic::error(
                                Code::UnknownField,
                                name.span,
                                format!("`{other}` is not an attribute"),
                            )
                            .note(
                                "it belongs in `installer {}` — and in `uninstaller {}`, \
                                 which is the same field for the other half (§15.3)",
                            ),
                        );
                    }
                    None => {
                        self.diags.push(
                            Diagnostic::error(
                                Code::UnknownField,
                                name.span,
                                format!("`{other}` is not an attribute"),
                            )
                            .note(format!("the attributes are {}", list(&attribute_names()))),
                        );
                    }
                },
            }
        }
    }

    /// One `attributes {}` field, lowered from its row.
    ///
    /// The whole of the per-field knowledge is [`table::Setting`], so this is
    /// the function that has to grow when a *shape* is new and not when a
    /// setting is (§15.23).
    fn setting(&mut self, entry: &'static table::Instruction, field: &str, value: &Expr) {
        let table::Class::Attribute(holds) = entry.class else {
            // A row reached by name from `attributes {}` that is not an
            // attribute at all: `name = detailPrint` names an instruction.
            self.diags.push(
                Diagnostic::error(
                    Code::UnknownField,
                    value.span(),
                    format!("`{field}` is not an attribute"),
                )
                .note(format!(
                    "it is `{}`, which is called rather than set",
                    entry.nsis
                )),
            );
            return;
        };

        match holds {
            // The only shape that is more than one line, and so the only one
            // that loops. Everything below emits exactly once.
            table::Setting::Each(one) => self.each_setting(entry, field, value, *one),
            other => self.setting_line(entry, field, other, value),
        }
    }

    /// One NSIS line, from the shape one entry of the field has.
    ///
    /// Called once for a plain setting and once *per element* for a
    /// [`table::Setting::Each`], which is why the two cannot come to disagree
    /// about what a line of this row looks like.
    fn setting_line(
        &mut self,
        entry: &'static table::Instruction,
        field: &str,
        holds: table::Setting,
        value: &Expr,
    ) {
        match holds {
            table::Setting::Table(parts) => self.table_setting(entry, field, value, parts),
            // Only reachable if a row grew a `Handled` setting without the arm
            // above that is supposed to shape it.
            table::Setting::Handled(_) => {
                self.todo(value.span(), &format!("the `{field}` attribute"));
            }
            // A list of lists has no meaning: the elements of an `Each` are
            // whatever one line of the row takes, and one line is never itself
            // a sequence.
            table::Setting::Each(_) => {
                self.todo(value.span(), &format!("`{field}` nested in itself"));
            }
            // A position the snapshot marks `Rep::Many` takes as many values as
            // the caller has, all on the one line.
            one if entry.params.first().is_some_and(table::Param::repeats) => {
                self.many_setting(entry, field, one, value);
            }
            // Every other shape is one value on one line, which is
            // [`Self::value_arg`] — the same function a *part* of a table goes
            // through, so the two cannot drift apart.
            one => {
                if let Some(arg) =
                    self.value_arg(one, entry.params.first(), field, entry.nsis, value)
                {
                    self.module
                        .attributes
                        .push(ir::Instruction::new(entry.nsis, vec![arg]));
                }
            }
        }
    }

    /// One value, read from the [`table::Setting`] that says what it holds.
    ///
    /// This is the function that has to grow when a *shape* is new, and the
    /// reason a setting written on its own line and a part of a
    /// [`table::Setting::Table`] cannot disagree about what a `bool` or a path
    /// is: they are this function called twice.
    ///
    /// `param` is the position the value stands against and is read only for an
    /// enum's members, which are the snapshot's and never a list here (§15.23).
    /// `line` is the NSIS command, which the notes name because that is what
    /// the field becomes.
    fn value_arg(
        &mut self,
        holds: table::Setting,
        param: Option<&table::Param>,
        field: &str,
        line: &str,
        value: &Expr,
    ) -> Option<ir::Arg> {
        match holds {
            table::Setting::Str { path } => {
                let arg = self.constant_arg(value, field)?;
                Some(if path { arg.into_path() } else { arg })
            }
            table::Setting::Bool { on, off } => match self.constant(value) {
                Some(ConstValue::Bool(flag)) => Some(ir::Arg::raw(if flag { on } else { off })),
                _ => {
                    self.bad_value(
                        value.span(),
                        field,
                        "a `bool`",
                        &format!("it becomes `{line} {on}` or `{line} {off}`"),
                    );
                    None
                }
            },
            table::Setting::Enum => {
                let text = self.keyword(value, field)?;
                // An **open** enum lists the keywords worth completing and does
                // not close the set: `ManifestSupportedOS`'s `{GUID}` stands for
                // every GUID there is, so checking against the seven names it
                // prints beside it would reject the values it exists to allow.
                if param.is_some_and(table::Param::open) {
                    return Some(ir::Arg::raw(text));
                }
                let allowed = param.map(table::Param::members).unwrap_or_default();
                self.enumerated(field, &text, allowed, value.span())
                    .then(|| ir::Arg::raw(text))
            }
            table::Setting::Int => match self.constant(value) {
                Some(ConstValue::Int(number)) => Some(ir::Arg::raw(number.to_string())),
                _ => {
                    self.bad_value(
                        value.span(),
                        field,
                        "an `int`",
                        &format!("it becomes `{line} <n>`, with the number written bare"),
                    );
                    None
                }
            },
            // All three are shapes rather than values: a table and an `Each` are
            // more than one of these, and `Handled` is not lowered here at all.
            table::Setting::Table(_) | table::Setting::Each(_) | table::Setting::Handled(_) => {
                self.todo(value.span(), &format!("`{field}` in this position"));
                None
            }
        }
    }

    /// The keyword an enum-valued field was written with.
    ///
    /// Almost always a string — `compressor = "lzma"` — but a registry root is
    /// a **bare** name, because `readRegStr(HKLM, …)` already spells it that way
    /// and one idea with two spellings is worse than either of them (§15.1).
    /// The names this accepts are exactly the sigil-less constants, so no other
    /// field changes: there is no constant called `lzma` for `compressor = lzma`
    /// to find, and an unknown bare name still fails as a value.
    fn keyword(&mut self, value: &Expr, field: &str) -> Option<String> {
        if let Expr::Name(name) = value
            && let Some(constant) = crate::builtins::constant_named(&name.text)
            && !constant.sigil
        {
            return Some(constant.nsis.to_string());
        }
        self.constant_string(value, field)
    }

    /// `peAddResource = { { file = …, … }, { … } }`: one whole NSIS line per
    /// element, in the order they were written.
    ///
    /// The elements are **positional** where the parts of a
    /// [`table::Setting::Table`] are named, and for the same reason in reverse:
    /// three strings on one line can only be told apart by a key, and two
    /// resources can only be told apart by their order. A Lua table keeps that
    /// order and no other (§12), which is the order NSIS adds them in.
    fn each_setting(
        &mut self,
        entry: &'static table::Instruction,
        field: &str,
        value: &Expr,
        each: table::Setting,
    ) {
        let entry_shape = match each {
            table::Setting::Table(parts) => format!("{{ {} }}", shape(parts)),
            _ => "…".to_string(),
        };
        let Expr::Table { fields, .. } = value else {
            self.bad_value(
                value.span(),
                field,
                "a list",
                &format!(
                    "write `{field} = {{ {entry_shape} }}`, with one entry per `{}` line",
                    entry.nsis
                ),
            );
            return;
        };

        for given in fields {
            let TableField::Positional { value: element } = given else {
                let TableField::Named { name, .. } = given else {
                    unreachable!("a table field is one or the other");
                };
                self.diags.push(
                    Diagnostic::error(
                        Code::UnknownField,
                        name.span,
                        format!("`{field}` takes a list, so `{}` names nothing", name.text),
                    )
                    .note(format!(
                        "write `{field} = {{ {entry_shape} }}`, with one entry per `{}` line",
                        entry.nsis
                    )),
                );
                continue;
            };
            // An element of the wrong shape is reported here and not in
            // [`Self::table_setting`], whose note says `field = { … }` — true of
            // a setting written on its own line and wrong inside a list, where
            // what belongs is one *entry*.
            if matches!(each, table::Setting::Table(_)) && !matches!(element, Expr::Table { .. }) {
                self.bad_value(
                    element.span(),
                    field,
                    "a table for every entry",
                    &format!(
                        "write `{field} = {{ {entry_shape} }}`, with one entry per `{}` line",
                        entry.nsis
                    ),
                );
                continue;
            }
            self.setting_line(entry, field, each, element);
        }
    }

    /// `manifestSupportedOS = { "Win7", "Win10" }`: one line, as many values as
    /// were written.
    ///
    /// The third thing in this file that reads a Lua list, and the only one the
    /// **snapshot** asks for: `Rep::Many` is `-CMDHELP`'s `[...]`, so no row
    /// says a position repeats and none can be wrong about it. The list is
    /// ordered for the same reason a [`table::Setting::Each`]'s is — NSIS reads
    /// these positionally — and the difference between the two is invisible in
    /// Lua and the whole of the difference in NSIS: one line here, one line per
    /// element there.
    ///
    /// A *call* spells the same repetition with varargs — `file(a, b, c)` — and
    /// a field cannot, because a field takes one value. The surface forces the
    /// asymmetry rather than choosing it.
    fn many_setting(
        &mut self,
        entry: &'static table::Instruction,
        field: &str,
        holds: table::Setting,
        value: &Expr,
    ) {
        let Expr::Table { fields, .. } = value else {
            self.bad_value(
                value.span(),
                field,
                "a list",
                &format!(
                    "`{}` takes one or more values on the one line, so write `{field} = {{ … }}`",
                    entry.nsis
                ),
            );
            return;
        };

        let mut args = Vec::new();
        for given in fields {
            let TableField::Positional { value: element } = given else {
                let TableField::Named { name, .. } = given else {
                    unreachable!("a table field is one or the other");
                };
                self.diags.push(
                    Diagnostic::error(
                        Code::UnknownField,
                        name.span,
                        format!("`{field}` takes a list, so `{}` names nothing", name.text),
                    )
                    .note(format!(
                        "`{}` reads its values by position, not by name",
                        entry.nsis
                    )),
                );
                continue;
            };
            if let Some(arg) =
                self.value_arg(holds, entry.params.first(), field, entry.nsis, element)
            {
                args.push(arg);
            }
        }

        // An empty list is not an empty line: `ManifestSupportedOS` with no
        // value is a syntax error to NSIS, and writing nothing at all is what
        // the caller meant.
        if args.is_empty() {
            self.bad_value(
                value.span(),
                field,
                "at least one value",
                &format!("leave `{field}` out to write no `{}` line", entry.nsis),
            );
            return;
        }
        self.module
            .attributes
            .push(ir::Instruction::new(entry.nsis, args));
    }

    /// `installDirRegKey = { root = HKLM, key = "Software/App", name = "Path" }`:
    /// one NSIS line built out of a Lua table, one key per position, emitted in
    /// the *table's* order rather than the source's — a Lua table has no order
    /// to preserve and NSIS counts arguments (§12).
    ///
    /// A part may be left out when its position is optional *and* nothing after
    /// it was written — `peAddResource`'s `reslang` is the one that is. Which
    /// positions those are is the snapshot's answer and not a part's, so a gap
    /// in the middle is the same error as a part nobody wrote: NSIS counts
    /// arguments, and a short line means a different thing rather than less.
    fn table_setting(
        &mut self,
        entry: &'static table::Instruction,
        field: &str,
        value: &Expr,
        parts: &'static [table::Part],
    ) {
        let Expr::Table { fields, .. } = value else {
            self.bad_value(
                value.span(),
                field,
                "a table",
                &format!("write `{field} = {{ {} }}`", shape(parts)),
            );
            return;
        };

        let mut written: Vec<(&str, &Expr)> = Vec::new();
        for given in fields {
            let TableField::Named { name, value: part } = given else {
                self.todo(value.span(), &format!("a positional entry in `{field}`"));
                continue;
            };
            if !parts.iter().any(|known| known.field == name.text) {
                self.diags.push(
                    Diagnostic::error(
                        Code::UnknownField,
                        name.span,
                        format!("`{}` is not part of `{field}`", name.text),
                    )
                    .note(format!(
                        "the parts are {}",
                        list(&parts.iter().map(|part| part.field).collect::<Vec<_>>())
                    )),
                );
                continue;
            }
            written.push((name.text.as_str(), part));
        }

        // The last part anybody wrote. Everything up to it has to be there, and
        // an optional position after it is simply not emitted.
        let last = parts
            .iter()
            .rposition(|part| written.iter().any(|(key, _)| *key == part.field));

        let mut args = Vec::new();
        for (index, part) in parts.iter().enumerate() {
            let optional = entry
                .params
                .get(index)
                .is_some_and(|param| !param.required());
            if optional && last.is_none_or(|last| index > last) {
                continue;
            }
            // Last one wins, which is what Lua does with a repeated key.
            let Some((_, given)) = written.iter().rev().find(|(key, _)| *key == part.field) else {
                let mut diag = Diagnostic::error(
                    Code::BadFieldValue,
                    value.span(),
                    format!("`{field}` has no `{}`", part.field),
                )
                .note(format!("write `{field} = {{ {} }}`", shape(parts)))
                .note(format!(
                    "`{}` counts its arguments, so nothing before a position you wrote can be left out",
                    entry.nsis
                ));
                let spare = optional_parts(entry, parts);
                if !spare.is_empty() {
                    diag = diag.note(format!("only {} may be left out", list(&spare)));
                }
                self.diags.push(diag);
                return;
            };
            let arg = match self.value_arg(
                part.holds,
                entry.params.get(index),
                part.field,
                entry.nsis,
                given,
            ) {
                Some(arg) => arg,
                None => return,
            };
            args.push(arg);
        }

        self.module
            .attributes
            .push(ir::Instruction::new(entry.nsis, args));
    }

    /// `Name "${APP}"` and its seven siblings: one attribute, one argument.
    fn string_attribute(&mut self, nsis: &str, field: &str, value: &Expr, path: bool) {
        let Some(arg) = self.constant_arg(value, field) else {
            return;
        };
        let arg = if path { arg.into_path() } else { arg };
        self.module
            .attributes
            .push(ir::Instruction::new(nsis, vec![arg]));
    }

    /// Whether a keyword is one of the closed set the position accepts.
    ///
    /// NSIS accepts an unknown keyword here and *ignores* it — `SetCompressor
    /// lmza` is not an error — so the closed set is checked here or not at all
    /// (§13).
    fn enumerated(&mut self, field: &str, text: &str, allowed: &[&str], span: Span) -> bool {
        if !allowed.contains(&text) {
            self.diags.push(
                Diagnostic::error(
                    Code::BadFieldValue,
                    span,
                    format!("`{text}` is not a `{field}`"),
                )
                .note(format!("the values are {}", list(allowed)))
                .note("NSIS ignores a keyword it does not know here rather than objecting (§13)"),
            );
            return false;
        }
        true
    }

    // There is no `flag_attribute` helper any more, nor an `int_` or `enum_`
    // one: the shapes are [`Self::setting`]'s arms, and a helper per shape would
    // be a second place to look for the same four lines.

    /// `versionInfo = { product = "1.4.2.0", keys = { … } }`.
    ///
    /// The keys are emitted in **sorted** order rather than source order. A Lua
    /// table has no order to preserve — `{ a = 1, b = 2 }` and `{ b = 2, a = 1 }`
    /// are the same table — so anything else would make the golden depend on
    /// something the language says is not there (§14).
    fn version_info(&mut self, value: &Expr) {
        let Expr::Table { fields, .. } = value else {
            self.bad_value(
                value.span(),
                "versionInfo",
                "a table",
                "write `versionInfo = { product = \"1.0.0.0\", keys = { … } }`",
            );
            return;
        };

        // `VIAddVersionKey` before `VIProductVersion` is a `makensis` error, so
        // the two are ordered here rather than left to the order the fields
        // happen to be written in — a table has no order (§12).
        let mut fields: Vec<&TableField> = fields.iter().collect();
        fields.sort_by_key(|field| match field {
            TableField::Named { name, .. } if name.text == "product" => 0,
            _ => 1,
        });

        for field in fields {
            let TableField::Named { name, value } = field else {
                self.todo(value.span(), "a positional entry in `versionInfo`");
                continue;
            };
            match name.text.as_str() {
                "product" => {
                    // `VIProductVersion` takes four unquoted numbers, and
                    // `makensis` rejects anything else — which is why this is a
                    // checked shape rather than a string passed through.
                    let Some(text) = self.constant_string(value, "product") else {
                        continue;
                    };
                    let quads = text.split('.').count() == 4
                        && text.split('.').all(|part| {
                            !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit())
                        });
                    if !quads {
                        self.bad_value(
                            value.span(),
                            "product",
                            "four dotted numbers",
                            "`VIProductVersion` wants `x.y.z.w` and `makensis` rejects any other \
                             shape",
                        );
                        continue;
                    }
                    self.module.attributes.push(ir::Instruction::new(
                        "VIProductVersion",
                        vec![ir::Arg::raw(text)],
                    ));
                }
                "keys" => {
                    let Expr::Table { fields, .. } = value else {
                        self.bad_value(
                            value.span(),
                            "keys",
                            "a table",
                            "write `keys = { ProductName = \"…\" }`",
                        );
                        continue;
                    };
                    let mut keys: Vec<(String, ir::Arg)> = Vec::new();
                    for field in fields {
                        let TableField::Named { name, value } = field else {
                            self.todo(value.span(), "a positional entry in `keys`");
                            continue;
                        };
                        if let Some(arg) = self.constant_arg(value, &name.text) {
                            keys.push((name.text.clone(), arg));
                        }
                    }
                    keys.sort_by(|a, b| a.0.cmp(&b.0));
                    for (key, arg) in keys {
                        self.module.attributes.push(ir::Instruction::new(
                            "VIAddVersionKey",
                            vec![ir::Arg::raw(key), arg],
                        ));
                    }
                }
                other => {
                    self.diags.push(
                        Diagnostic::error(
                            Code::UnknownField,
                            name.span,
                            format!("`{other}` is not a `versionInfo` field"),
                        )
                        .note("the fields are `product` and `keys`"),
                    );
                }
            }
        }
    }

    // -- bodies -----------------------------------------------------------

    /// `installer { … }` and `uninstaller { … }`, which are the same block with
    /// two spellings — the whole of §15.3's duality is [`Half`] threaded
    /// through this one function. `un.` has no surface spelling at all.
    ///
    /// Two passes rather than one: the named fields are settings the whole
    /// block carries, and the positional entries — sections, pages, callbacks —
    /// read some of them. A Lua table has no order for the user to get right
    /// (§12), so the order is the compiler's.
    fn installer(&mut self, fields: &[TableField], half: Half) {
        for field in fields {
            let TableField::Named { name, value } = field else {
                continue;
            };
            match name.text.as_str() {
                "installDir" if half == Half::Installer => {
                    self.string_attribute("InstallDir", "installDir", value, true);
                }
                // The five block-level MUI settings. Each is a line MUI2 writes
                // itself, from this define, so writing the line instead would
                // assemble clean under `-WX` and then lose (§15.7).
                "icon" => {
                    let define = match half {
                        Half::Installer => "MUI_ICON",
                        Half::Uninstaller => "MUI_UNICON",
                    };
                    self.mui_define(define, value, "icon", true);
                }
                "checkBitmap" if half == Half::Installer => {
                    self.mui_define("MUI_COMPONENTSPAGE_CHECKBITMAP", value, "checkBitmap", true);
                }
                "installColors" if half == Half::Installer => {
                    self.mui_define("MUI_INSTFILESPAGE_COLORS", value, "installColors", false);
                }
                "progressBar" if half == Half::Installer => {
                    self.mui_define("MUI_INSTFILESPAGE_PROGRESSBAR", value, "progressBar", false);
                }
                "licenseBkColor" if half == Half::Installer => {
                    self.mui_define("MUI_LICENSEPAGE_BGCOLOR", value, "licenseBkColor", false);
                }
                "installTypes" => self.install_types(value, half),
                other if half == Half::Uninstaller && ONCE_GLOBAL_FIELDS.contains(&other) => {
                    self.diags.push(
                        Diagnostic::error(
                            Code::UnknownField,
                            name.span,
                            format!("`{other}` is not an `uninstaller` field"),
                        )
                        .note(
                            "NSIS reads it once, script-wide, so both halves writing it would \
                             define one name twice — write it in `installer {}` (§15.3)",
                        ),
                    );
                }
                other if V1_INSTALLER_FIELDS.contains(&other) => {
                    self.todo(name.span, &format!("the `{other}` field"));
                }
                other => {
                    self.diags.push(
                        Diagnostic::error(
                            Code::UnknownField,
                            name.span,
                            format!("`{other}` is not an `{half}` field"),
                        )
                        .note(format!("the fields are {}", list(V1_INSTALLER_FIELDS))),
                    );
                }
            }
        }

        for field in fields {
            let TableField::Positional { value } = field else {
                continue;
            };
            self.body_entry(value, half);
        }
    }

    /// One block-level `!define MUI_*`, from a field whose value is build-time.
    fn mui_define(&mut self, define: &str, value: &Expr, field: &str, path: bool) {
        let Some(arg) = self.constant_arg(value, field) else {
            return;
        };
        let arg = if path { arg.into_path() } else { arg };
        self.module.mui_defines.push(ir::Define {
            name: define.to_string(),
            value: Some(arg),
        });
        self.mui = true;
    }

    /// `installTypes = { "Full", "Minimal" }` — the presets the components page
    /// offers, in the order it offers them.
    ///
    /// The order is the whole of §13's binding at this end. A section says which
    /// types it belongs to *by name*, NSIS reads only a one-based position, and
    /// this list is what turns one into the other — so the numbering exists in
    /// exactly one place and a user never writes a number that could go stale
    /// when a type is inserted in front of it.
    ///
    /// The two halves are two lists because NSIS numbers them separately: an
    /// `InstType un.` belongs to the uninstaller's components page and the
    /// installer's first type is still `1`.
    fn install_types(&mut self, value: &Expr, half: Half) {
        let Expr::Table { fields, .. } = value else {
            self.bad_value(
                value.span(),
                "installTypes",
                "a list",
                "write `installTypes = { \"Full\", \"Minimal\" }`",
            );
            return;
        };

        let mut names: Vec<String> = Vec::new();
        for field in fields {
            let TableField::Positional { value } = field else {
                self.todo(value.span(), "a named entry in `installTypes`");
                continue;
            };
            let Some(name) = self.constant_string(value, "installTypes") else {
                continue;
            };
            if names.contains(&name) {
                self.diags.push(
                    Diagnostic::error(
                        Code::BadFieldValue,
                        value.span(),
                        format!("`{name}` is already an install type"),
                    )
                    .note(
                        "a section names one of these, so two with one name cannot be told apart",
                    ),
                );
                continue;
            }
            if names.len() == MAX_INST_TYPES {
                self.diags.push(
                    Diagnostic::error(
                        Code::BadFieldValue,
                        value.span(),
                        format!("more than {MAX_INST_TYPES} install types"),
                    )
                    .note("NSIS numbers them 1 to 32 and rejects the rest"),
                );
                break;
            }
            names.push(name);
        }

        match half {
            Half::Installer => self.module.inst_types = names,
            Half::Uninstaller => self.module.uninst_types = names,
        }
    }

    /// `page.directory { topText = "…" }` — one page and the settings that
    /// belong to it, as a positional entry in the block that supplies its half.
    ///
    /// Page order is the order the entries were written, and it is the one list
    /// in the output that is never sorted: it is what the user sees.
    fn page(&mut self, call: &Expr, which: &Name, half: Half) {
        let Some(page) = V1_PAGES.iter().find(|page| page.installua == which.text) else {
            self.diags.push(
                Diagnostic::error(
                    Code::UnknownField,
                    which.span,
                    format!("`{}` is not a page", which.text),
                )
                .note(format!(
                    "the pages are {}",
                    list(
                        &V1_PAGES
                            .iter()
                            .map(|page| page.installua)
                            .collect::<Vec<_>>()
                    )
                )),
            );
            return;
        };
        if !page.has(half) {
            self.diags.push(
                Diagnostic::error(
                    Code::UnknownField,
                    which.span,
                    format!("there is no {half} `{}` page", which.text),
                )
                .note(format!(
                    "MUI2 defines no `{}{}`",
                    half.page_prefix(),
                    page.nsis
                )),
            );
            return;
        }

        let Expr::Call { args, .. } = call else {
            return;
        };
        let written: &[TableField] = match args.as_slice() {
            [] => &[],
            [Expr::Table { fields, .. }] => fields,
            _ => {
                self.todo(call.span(), "this page in this form");
                return;
            }
        };
        let mut named: Vec<(&Name, &Expr)> = Vec::new();
        let mut positional: Vec<&Expr> = Vec::new();
        for entry in written {
            match entry {
                TableField::Named { name, value } => named.push((name, value)),
                // §15.23's form: the array part is the parameter. Only the
                // custom page has one — the other seven are named by the macro
                // they insert and captioned by MUI2's own language file.
                TableField::Positional { value } if page.custom => positional.push(value),
                TableField::Positional { value } => {
                    self.todo(value.span(), "a positional entry in a page");
                }
            }
        }

        if page.custom {
            self.page_fields_known(&named, page);
            self.custom_page(&named, &positional, which, half, page);
            return;
        }

        // `MUI_PAGE_LICENSE` takes the file as its macro argument rather than
        // as a define, which is why `file` is handled here and not in
        // [`Page::own`]. It lives on the page and not on the block because a
        // block-level `license` with no License page listed evaporates without
        // a word, and here that is unwritable.
        let mut macro_args = Vec::new();
        if page.installua == "license" {
            let file = named
                .iter()
                .find(|(name, _)| name.text == "file")
                .and_then(|(_, value)| self.constant_arg(value, "file"));
            match file {
                Some(arg) => macro_args.push(arg.into_path()),
                None => {
                    self.diags.push(
                        Diagnostic::error(
                            Code::MissingAttribute,
                            which.span,
                            "a `license` page needs a `file`",
                        )
                        .note("write `page.license { file = \"LICENSE.txt\" }`")
                        .note("`MUI_PAGE_LICENSE` takes the file as its argument"),
                    );
                    return;
                }
            }
        }

        let mut defines = Vec::new();
        let mut undefines = Vec::new();
        for field in page.fields() {
            let Some((_, value)) = named.iter().find(|(name, _)| name.text == field.installua)
            else {
                continue;
            };
            self.page_field(field, value, half, page, &mut defines, &mut undefines);
        }

        // MUI2 reads `!ifdef MUI_LICENSEPAGE_CHECKBOX` first and the radio
        // buttons only in its `!else`, so writing both is not a page with two
        // controls — it is a page whose second setting does nothing.
        if page.installua == "license" {
            let both = ["checkbox", "radioButtons"]
                .iter()
                .all(|want| named.iter().any(|(name, _)| name.text == *want));
            if both {
                self.diags.push(
                    Diagnostic::error(
                        Code::BadFieldValue,
                        which.span,
                        "a `license` page takes `checkbox` or `radioButtons`, not both",
                    )
                    .note(
                        "MUI2 reads the checkbox first and the radio buttons only if it is absent",
                    ),
                );
            }
        }

        self.page_fields_known(&named, page);

        let mut all = vec![ir::Arg::raw(format!("{}{}", half.page_prefix(), page.nsis))];
        all.extend(macro_args);
        let lowered = ir::Page {
            defines,
            insert: ir::Instruction::new("!insertmacro", all),
            undefines,
        };
        match half {
            Half::Installer => self.module.pages.push(lowered),
            Half::Uninstaller => self.module.unpages.push(lowered),
        }
        self.mui = true;
    }

    /// Every field a page was given, checked against the ones it has.
    fn page_fields_known(&mut self, named: &[(&Name, &Expr)], page: &Page) {
        let mut fields: Vec<&str> = page.fields().map(|field| field.installua).collect();
        fields.extend(extra_field(page));
        for (name, _) in named {
            if !fields.contains(&name.text.as_str()) {
                self.diags.push(
                    Diagnostic::error(
                        Code::UnknownField,
                        name.span,
                        format!("`{}` is not a `{}` page field", name.text, page.installua),
                    )
                    .note(format!("the fields are {}", list(&fields))),
                );
            }
        }
    }

    /// `page.custom { "Registration", … }` — the eighth page, whose body is the
    /// compiler's to write.
    ///
    /// Everything here is the same *setting* as on the other seven and a
    /// different *mechanism*, which is the split §15.7 already draws. `Page
    /// custom` is a stock NSIS instruction and MUI2 never sees it, so
    /// `MUI_PAGE_HEADER_TEXT` — a define MUI2 reads from inside the `PageEx` it
    /// generates — would sit there doing nothing and then leak onto the next
    /// page that *does* read it. `MUI_HEADER_TEXT` inside the creator is the
    /// call MUI2's own documentation writes, and it has no ordering to get
    /// wrong: it is an instruction in a function body rather than a define with
    /// a lifetime.
    ///
    /// The three callbacks land in two places for a reason that is NSIS's, not
    /// ours. `Page custom` names a creator and a leave function; there is no
    /// third slot. So `pre` and `show` are *inlined* into the creator, on either
    /// side of the dialog: `pre` runs before it exists — early enough for
    /// `abort()` to skip the page — and `show` runs once every control is up and
    /// before the window is shown.
    fn custom_page(
        &mut self,
        named: &[(&Name, &Expr)],
        positional: &[&Expr],
        which: &Name,
        half: Half,
        page: &Page,
    ) {
        let written = |field: &str| {
            named
                .iter()
                .find(|(name, _)| name.text == field)
                .map(|(_, value)| *value)
        };

        // At most one, and it is the caption. More than one is not a page with
        // two names — it is a table whose array part was written by mistake.
        let caption = match positional {
            [] => None,
            [one] => self.constant_arg(one, "the page's caption"),
            [_, extra, ..] => {
                self.diags.push(
                    Diagnostic::error(
                        Code::WrongArity,
                        extra.span(),
                        "a `custom` page takes one caption",
                    )
                    .note("everything else in the table is a named field"),
                );
                return;
            }
        };

        // Both halves of `MUI_HEADER_TEXT`, because the macro takes two
        // arguments and there is no spelling for omitting one. An empty string
        // is what MUI2's own callers pass, and it leaves the strip's second line
        // blank rather than stale.
        let header = if written("headerText").is_some() || written("headerSubText").is_some() {
            let mut text = |field| match written(field) {
                Some(value) => self
                    .constant_arg(value, field)
                    .unwrap_or_else(|| ir::Arg::str("")),
                None => ir::Arg::str(""),
            };
            let (top, sub) = (text("headerText"), text("headerSubText"));
            Some(vec![ir::Arg::raw("MUI_HEADER_TEXT"), top, sub])
        } else {
            None
        };

        // Built before the body is, because a control is checked with the
        // `Lowerer` in hand — constants, diagnostics, the claim map — and what
        // comes back is instructions the creator threads together.
        let controls = match written("controls") {
            Some(value) => self.controls(value),
            None => Vec::new(),
        };

        let pre = written("pre").and_then(|value| self.callback_body(value, "pre"));
        let show = written("show").and_then(|value| self.callback_body(value, "show"));
        let leave =
            written("leave").and_then(|value| self.page_callback(value, half, page, "leave"));

        let span = which.span;
        let create = self.page_function_name(half, page, "create");
        let (body, _) = self.body_with(span, Some(half), |lowerer| {
            if let Some((block, _)) = pre {
                lowerer.block(block);
            }
            if let Some(header) = header {
                lowerer.emit(ir::Instruction::new("!insertmacro", header));
            }

            // 1018 is the id of the inner control MUI2 draws its pages into. It
            // is Microsoft's dialog resource in NSIS's own UI file rather than a
            // number anybody chose, which is why it is written here once and
            // never offered as a setting.
            let dialog = lowerer.body.vreg(span);
            lowerer.generated_plugin_call(
                "nsDialogs::Create",
                vec![ir::Arg::raw("1018")],
                vec![dialog.clone()],
                span,
            );

            // The plugin pushes the string `error` instead of a handle when the
            // dialog does not come up. Continuing past that shows the user an
            // empty page, so the generated form checks it even though nothing
            // the program did can cause it.
            let n = lowerer.body.construct();
            let failed = lowerer.fresh(format!("dialog_{n}_failed"));
            let created = lowerer.fresh(format!("dialog_{n}"));
            // Written as "carry on unless it failed" rather than "fail if it
            // did", so the failure arm is the last block in the body: `Abort`
            // ends the function, and a block that ends the body needs no
            // `Return` line after it.
            lowerer.terminate(
                Terminator::Branch {
                    test: Test::Str {
                        lhs: ir::Arg::slot(dialog),
                        rhs: ir::Arg::str("error"),
                        case_sensitive: true,
                        negate: true,
                    },
                    then_block: created,
                    else_block: failed,
                },
                created,
            );

            // The controls, in the order the list wrote them — which is the
            // order Windows gives them the tab key in, and the one thing about
            // a control that its declaration does not decide (ruling 3).
            for control in &controls {
                let handle = match &control.var {
                    Some(var) => Slot::Global(var.clone()),
                    // Popped and dropped: `CreateControl` pushes a handle
                    // whether or not anything wants it, and leaving it on the
                    // stack is how a page ends up reading its own control as a
                    // string later on.
                    None => lowerer.body.vreg(control.span),
                };
                lowerer.generated_plugin_call(
                    "nsDialogs::CreateControl",
                    control.create.clone(),
                    vec![handle.clone()],
                    control.span,
                );
                for item in &control.items {
                    lowerer.emit(ir::Instruction::new(
                        "SendMessage",
                        vec![
                            ir::Arg::slot(handle.clone()),
                            ir::Arg::raw(format!("0x{:04X}", control.add_item.unwrap_or(0))),
                            ir::Arg::int(0),
                            item.clone(),
                        ],
                    ));
                }
            }

            if let Some((block, _)) = show {
                lowerer.block(block);
            }
            lowerer.generated_plugin_call("nsDialogs::Show", Vec::new(), Vec::new(), span);
            lowerer.terminate(Terminator::Return, failed);
            lowerer.emit(ir::Instruction::new("Abort", Vec::new()));
        });
        self.module.functions.push(ir::Function {
            name: create.clone(),
            body,
        });

        // `Page custom creator leave caption`, and each trailing argument is
        // only omissible while the ones after it are too — so a page with a
        // caption and no `leave` writes the empty string NSIS reads as "none".
        let mut all = vec![ir::Arg::raw("custom"), ir::Arg::raw(create)];
        if leave.is_some() || caption.is_some() {
            all.push(match leave {
                Some(name) => ir::Arg::raw(name),
                None => ir::Arg::str(""),
            });
        }
        all.extend(caption);

        let lowered = ir::Page {
            defines: Vec::new(),
            insert: ir::Instruction::new(
                match half {
                    Half::Installer => "Page",
                    Half::Uninstaller => "UninstPage",
                },
                all,
            ),
            undefines: Vec::new(),
        };
        match half {
            Half::Installer => self.module.pages.push(lowered),
            Half::Uninstaller => self.module.unpages.push(lowered),
        }
        // Nothing is `!include`d for this: `nsDialogs.dll` ships with NSIS and a
        // plugin call needs no header. `nsDialogs.nsh` exists for the `${NSD_*}`
        // macros, and the compiler expands those itself rather than depending on
        // an include whose order the user would then have to know.
        //
        // `MUI_HEADER_TEXT` is MUI2's, and a custom page beside seven inserted
        // ones is the whole point: the header is on either way.
        self.mui = true;
    }

    /// `controls = { label { … }, serial, agree }` — the page's list, in the
    /// order it draws and tabs through.
    ///
    /// Two kinds of entry, and the difference is only whether the program says
    /// the control's name again later: a bare name is a declaration this page is
    /// claiming, and anything else is a declaration written where it is used.
    /// Neither is a different control (ruling 3).
    fn controls(&mut self, value: &Expr) -> Vec<Created> {
        let Expr::Table { fields, .. } = value else {
            self.bad_value(
                value.span(),
                "controls",
                "a list of controls",
                "a page draws what this list holds, in the order it holds it",
            );
            return Vec::new();
        };

        let mut created = Vec::new();
        for field in fields {
            let value = match field {
                TableField::Positional { value } => value,
                TableField::Named { name, value } => {
                    self.diags.push(
                        Diagnostic::error(
                            Code::BadFieldValue,
                            name.span,
                            format!("`{}` is a key in a list of controls", name.text),
                        )
                        .note(
                            "every entry is a control or the name of one; what a control is called \
                             is the `local` it was bound to",
                        )
                        .note(format!(
                            "the settings go inside the control: `{} {{ … }}`",
                            name.text
                        )),
                    );
                    value
                }
            };

            // A bare name: this page is listing a declaration, exactly as a
            // block lists a section, and the claim rules are the same four.
            if let Expr::Name(name) = value {
                let Some((declared, kind, var)) = self.listed(name) else {
                    continue;
                };
                let DeferredKind::Control(control) = kind else {
                    continue;
                };
                created.extend(self.control(declared, control, Some(var)));
                continue;
            }

            let Some(callee) = value.callee_name() else {
                self.todo(value.span(), "this control");
                continue;
            };
            let Some(control) = control::control(callee) else {
                self.diags.push(
                    Diagnostic::error(
                        Code::UnknownField,
                        value.span(),
                        format!("`{callee}` is not a control"),
                    )
                    .note(format!("the controls are {}", list(&control::names()))),
                );
                continue;
            };
            created.extend(self.control(value, control, None));
        }
        created
    }

    /// One control declaration, checked and turned into the plugin call that
    /// creates it.
    ///
    /// §15.23's shape with nothing positional but the text: `nsDialogs` takes
    /// x, y, width and height as four separate arguments and a table's array
    /// part is a sequence, so writing them unnamed would be four numbers in an
    /// order a reader has to know. The one thing that *is* the control's
    /// parameter — the text it is drawn with — stays where §15.23 puts it.
    fn control(
        &mut self,
        value: &Expr,
        control: &'static control::Control,
        var: Option<String>,
    ) -> Option<Created> {
        let what = control.installua;
        let Expr::Call { args, .. } = value else {
            self.todo(value.span(), "this control");
            return None;
        };
        let [Expr::Table { fields, span }] = args.as_slice() else {
            self.todo(value.span(), &format!("this `{what}` form"));
            return None;
        };
        let span = *span;

        let mut text = None;
        let mut geometry: [Option<String>; 4] = [None, None, None, None];
        let mut items = None;
        for field in fields {
            match field {
                TableField::Positional { value } if text.is_none() && control.text.is_some() => {
                    text = self.constant_arg(value, control.text.unwrap_or(what));
                }
                TableField::Positional { value } => {
                    let note = match control.text {
                        Some(is) => {
                            format!("the first entry is {is}, and everything else is named")
                        }
                        None => format!("a `{what}` is drawn with no text at all"),
                    };
                    self.diags.push(
                        Diagnostic::error(
                            Code::BadFieldValue,
                            value.span(),
                            format!("a `{what}` takes no second unnamed entry"),
                        )
                        .note(note),
                    );
                }
                TableField::Named { name, value } => {
                    let axis = ["x", "y", "width", "height"]
                        .iter()
                        .position(|field| *field == name.text);
                    if let Some(axis) = axis {
                        geometry[axis] = self.measurement(value, &name.text);
                        continue;
                    }
                    if name.text == "items" {
                        if !control.takes_items() {
                            self.diags.push(
                                Diagnostic::error(
                                    Code::UnknownField,
                                    name.span,
                                    format!("a `{what}` holds no items"),
                                )
                                .note(
                                    "`items` is a `dropList`'s and a `listBox`'s: they are the \
                                       two controls that are a list",
                                ),
                            );
                            continue;
                        }
                        items = Some(self.items(value));
                        continue;
                    }
                    let mut options = vec!["x", "y", "width", "height"];
                    if control.takes_items() {
                        options.push("items");
                    }
                    self.diags.push(
                        Diagnostic::error(
                            Code::UnknownField,
                            name.span,
                            format!("`{}` is not a `{what}` option", name.text),
                        )
                        .note(format!("the options are {}", list(&options))),
                    );
                }
            }
        }

        // `y` and `height` are required and `x` and `width` are not, and the
        // asymmetry is ruling 6 rather than an oversight: the two defaults here
        // are constants — the left edge, and the full width — while a default
        // `y` could only mean *under the last control*, which is the auto-flow
        // this surface does not have. A control that says where it sits says so
        // itself, and moving one never moves another.
        for (axis, field) in [(1, "y"), (3, "height")] {
            if geometry[axis].is_none() {
                self.diags.push(
                    Diagnostic::error(
                        Code::MissingAttribute,
                        span,
                        format!("this `{what}` has no `{field}`"),
                    )
                    .note(
                        "there is no auto-flow: a control that does not say where it sits would \
                         have to sit under the one written above it, and the order of the list is \
                         the only thing that would then decide its position",
                    ),
                );
                return None;
            }
        }
        let [x, y, width, height] = geometry;

        let mut create = vec![
            ir::Arg::raw(control.class),
            ir::Arg::raw(format!("0x{:08X}", control.style)),
            ir::Arg::raw(format!("0x{:08X}", control.exstyle)),
            ir::Arg::raw(x.unwrap_or_else(|| "0".to_string())),
            ir::Arg::raw(y?),
            ir::Arg::raw(width.unwrap_or_else(|| "100%".to_string())),
            ir::Arg::raw(height?),
        ];
        // The text is the last argument and NSIS has no way to omit one, so a
        // control with nothing written on it — and one that was given nothing —
        // passes the empty string the plugin reads as "no text".
        create.push(text.unwrap_or_else(|| ir::Arg::str("")));

        Some(Created {
            create,
            var,
            add_item: control.add_item,
            items: items.unwrap_or_default(),
            span,
        })
    }

    /// `items = { "Alpha", "Beta" }`, as the arguments of the `ADDSTRING`s that
    /// fill the list.
    ///
    /// `STR:` is `SendMessage`'s spelling for "this argument is a string rather
    /// than a number", and it is the compiler's to write: it is a fact about
    /// NSIS's instruction, not about the list.
    fn items(&mut self, value: &Expr) -> Vec<ir::Arg> {
        let Expr::Table { fields, .. } = value else {
            self.bad_value(
                value.span(),
                "items",
                "a list of strings",
                "each one becomes a row of the list, in the order written",
            );
            return Vec::new();
        };
        let mut items = Vec::new();
        for field in fields {
            let TableField::Positional { value } = field else {
                let TableField::Named { name, .. } = field else {
                    continue;
                };
                self.bad_value(
                    name.span,
                    "items",
                    "a list of strings",
                    "a row of a list has no name: it is addressed by what it says",
                );
                continue;
            };
            if let Some(item) = self.constant_string(value, "an item") {
                items.push(ir::Arg::str(format!("STR:{item}")));
            }
        }
        items
    }

    /// One of `x`, `y`, `width`, `height`, in the form nsDialogs reads.
    ///
    /// An integer is dialog units, written with the `u` that says so: nsDialogs
    /// reads a bare number as **pixels**, and a page laid out in pixels is one
    /// that comes apart at a different font size or DPI. A string passes
    /// through, which is how `"100%"` and `"-13u"` — a width relative to the
    /// dialog, and an edge measured from the far side — are said at all.
    fn measurement(&mut self, value: &Expr, field: &str) -> Option<String> {
        match self.constant(value) {
            Some(ConstValue::Int(units)) => Some(format!("{units}u")),
            Some(ConstValue::Str(text)) if is_measurement(&text) => Some(text),
            _ => {
                self.bad_value(
                    value.span(),
                    field,
                    "a whole number of dialog units",
                    "a string is passed through for the two forms a number cannot say: `\"100%\"` \
                     is a share of the dialog and `\"-13u\"` is measured from its far edge",
                );
                None
            }
        }
    }

    /// One page setting, as the `!define`s it becomes and the `!undef`s that
    /// keep it off the next page.
    fn page_field(
        &mut self,
        field: &PageField,
        value: &Expr,
        half: Half,
        page: &Page,
        defines: &mut Vec<ir::Define>,
        undefines: &mut Vec<String>,
    ) {
        let mut define = |name: &str, value: Option<ir::Arg>, cleared: bool| {
            defines.push(ir::Define {
                name: name.to_string(),
                value,
            });
            if !cleared {
                undefines.push(name.to_string());
            }
        };

        match field.holds {
            Holds::Str => {
                if let Some(arg) = self.constant_arg(value, field.installua) {
                    define(field.define, Some(arg), field.cleared);
                }
            }
            Holds::Flag => match self.constant(value) {
                // `false` is not a define with a false value: MUI2 asks
                // `!ifdef`, so the only way to say no is to say nothing.
                Some(ConstValue::Bool(false)) => {}
                Some(ConstValue::Bool(true)) => define(field.define, None, field.cleared),
                _ => self.bad_value(
                    value.span(),
                    field.installua,
                    "a `bool`",
                    "MUI2 reads this one with `!ifdef`, so it is on or absent",
                ),
            },
            Holds::Var => {
                let Some(name) = value.name() else {
                    self.bad_value(
                        value.span(),
                        field.installua,
                        "a global",
                        "NSIS stores the chosen directory into this one, so it wants the \
                         variable and not its value (§15.24)",
                    );
                    return;
                };
                let known = self
                    .resolved
                    .globals
                    .iter()
                    .any(|global| global.name == name);
                if !known {
                    self.diags.push(
                        Diagnostic::error(
                            Code::UnknownField,
                            value.span(),
                            format!("`{name}` is not a global"),
                        )
                        .note("a global is declared by assigning to it at the top level (§15.24)"),
                    );
                    return;
                }
                define(
                    field.define,
                    Some(ir::Arg::var(format!("${name}"))),
                    field.cleared,
                );
            }
            Holds::Callback => {
                let Some(name) = self.page_callback(value, half, page, field.installua) else {
                    return;
                };
                define(field.define, Some(ir::Arg::str(name)), field.cleared);
            }
            Holds::Text(text) => {
                if let Some(arg) = self.constant_arg(value, field.installua) {
                    define(field.define, None, field.cleared);
                    define(text, Some(arg), field.cleared);
                }
            }
            Holds::Nested(parts) => {
                let Expr::Table { fields, .. } = value else {
                    self.bad_value(
                        value.span(),
                        field.installua,
                        "a table",
                        "write `radioButtons = { accept = \"Yes\", decline = \"No\" }`",
                    );
                    return;
                };
                define(field.define, None, field.cleared);
                for part in parts {
                    let written = fields.iter().find_map(|entry| match entry {
                        TableField::Named { name, value } if name.text == part.installua => {
                            Some(value)
                        }
                        _ => None,
                    });
                    let Some(written) = written else { continue };
                    if let Some(arg) = self.constant_arg(written, part.installua) {
                        define(part.define, Some(arg), part.cleared);
                    }
                }
                for entry in fields {
                    let TableField::Named { name, .. } = entry else {
                        continue;
                    };
                    if !parts.iter().any(|part| part.installua == name.text) {
                        self.diags.push(
                            Diagnostic::error(
                                Code::UnknownField,
                                name.span,
                                format!("`{}` is not a `{}` field", name.text, field.installua),
                            )
                            .note(format!(
                                "the fields are {}",
                                list(&parts.iter().map(|part| part.installua).collect::<Vec<_>>())
                            )),
                        );
                    }
                }
            }
        }
    }

    /// `pre = function() … end` — an NSIS `Function` MUI2 calls by name.
    ///
    /// The name is the compiler's, because nothing in the source is one: the
    /// hook is written where it runs. `un.` leads the uninstaller's, since MUI2
    /// calls it from an uninstaller page and NSIS spells that half in the
    /// function's name (§15.3).
    fn page_callback(
        &mut self,
        value: &Expr,
        half: Half,
        page: &Page,
        which: &str,
    ) -> Option<String> {
        let (block, span) = self.callback_body(value, which)?;
        let name = self.page_function_name(half, page, which);
        let body = self.body(block, &[], span, None, Some(half));
        self.module.functions.push(ir::Function {
            name: name.clone(),
            body,
        });
        Some(name)
    }

    /// The block behind `pre = function() … end`, checked.
    ///
    /// Split from [`Self::page_callback`] because a custom page's `pre` and
    /// `show` become no function at all: `Page custom` has two slots and three
    /// hooks, so two of them are inlined into the creator (§15.32).
    fn callback_body<'a>(&mut self, value: &'a Expr, which: &str) -> Option<(&'a Block, Span)> {
        let Expr::Function {
            params,
            block,
            span,
        } = value
        else {
            self.todo(value.span(), &format!("this `{which}` form"));
            return None;
        };
        if !params.is_empty() {
            self.diags.push(
                Diagnostic::error(
                    Code::WrongArity,
                    *span,
                    format!("`{which}` takes no arguments"),
                )
                .note("NSIS calls it, and `Call` has no argument list (§3)"),
            );
            return None;
        }
        Some((block, *span))
    }

    /// A name for a function this page owns. A second `page.directory` in the
    /// same half is legal, so the name has to distinguish them.
    fn page_function_name(&self, half: Half, page: &Page, which: &str) -> String {
        let stem = format!("{}mui.{}.{which}", half.prefix(), page.installua);
        let mut name = stem.clone();
        let mut nth = 2;
        while self.module.functions.iter().any(|f| f.name == name) {
            name = format!("{stem}.{nth}");
            nth += 1;
        }
        name
    }

    /// A positional entry in `installer {}`: a `section`, a page or a callback.
    fn body_entry(&mut self, value: &Expr, half: Half) {
        let Some(name) = value.callee_name() else {
            // `page.directory { … }`: a member rather than a bare name, which
            // is what a **closed** set of names buys — an editor completes the
            // eight and a typo is caught where it is written (§15.1).
            if let Some((base, which)) = value.callee_field()
                && base == "page"
            {
                self.page(value, which, half);
                return;
            }
            // A bare name is a declaration this block is listing.
            if let Expr::Name(name) = value {
                self.claimed(name, half);
                return;
            }
            self.todo(value.span(), "this entry");
            return;
        };
        match name {
            "section" => {
                if let Some(section) = self.section(value, half) {
                    self.module.sections.push(ir::SectionItem::Section(section));
                }
            }
            "group" => self.group(value, half, None),
            "onInit" => self.callback(value, half, "onInit"),
            other => self.todo(value.span(), &format!("`{other}` here")),
        }
    }

    /// A bare name among a block's entries: the section or group that
    /// `local core = section { … }` bound, lowered here rather than where it was
    /// written because here is where its half and its block's install types are
    /// known (`PHASE-6-SECTIONS.md` ruling 2).
    ///
    /// The position in the block is what decides install order, and the `local`
    /// decides nothing — which is the one thing a reader has to learn that they
    /// did not before, and the price of a section being addressable at all.
    fn claimed(&mut self, name: &Name, half: Half) {
        let Some((value, kind, index)) = self.listed(name) else {
            return;
        };
        match kind {
            DeferredKind::Section => {
                if let Some(mut section) = self.section(value, half) {
                    section.index_name = Some(index);
                    self.module.sections.push(ir::SectionItem::Section(section));
                }
            }
            DeferredKind::Group => self.group(value, half, Some(index)),
            // Unreachable: a control's claim comes from a `controls` list, and
            // a bare control name among a block's entries never earns one
            // ([`Site::accepts`]), so [`Self::listed`] has already said `None`.
            DeferredKind::Control(_) => {}
        }
    }

    /// The declaration this entry lists, when this entry is the one that
    /// claimed it. A second listing lowers nothing: [`Self::claim_pass`] already
    /// said so, and emitting the body twice under two indices is the thing the
    /// rule exists to prevent.
    fn listed(&self, name: &Name) -> Option<(&'p Expr, DeferredKind, String)> {
        let claim = self.claims.get(&name.text)?;
        if claim.span != name.span {
            return None;
        }
        let deferred = self.resolved.deferred.get(&name.text)?;
        // The name the claim earned: a `!define` for the index of a section, a
        // `Var` for the handle of a control. Both are derived from the *local*,
        // and both are what every use of the handle then reads.
        let earned = match deferred.kind.is_control() {
            true => control_var(&name.text, claim.half),
            false => index_name(&name.text, claim.half),
        };
        Some((deferred.value, deferred.kind, earned))
    }

    /// Every bare name among the two blocks' entries, recorded as a claim.
    ///
    /// Syntactic, and deliberately: it reads the shape of `installer { … }` and
    /// `group { … }` without lowering either, because the shape errors belong to
    /// [`Self::installer`] and [`Self::group`] and reporting them from two
    /// places would report them twice.
    fn claim_pass(&mut self, program: &Program) {
        for stmt in &program.block {
            let Stmt::Call(call) = stmt else {
                continue;
            };
            let half = match call.callee_name() {
                Some("installer") => Half::Installer,
                Some("uninstaller") => Half::Uninstaller,
                _ => continue,
            };
            let Expr::Call { args, .. } = call else {
                continue;
            };
            let [Expr::Table { fields, .. }] = args.as_slice() else {
                continue;
            };
            for field in fields {
                let TableField::Positional { value } = field else {
                    continue;
                };
                if let Expr::Name(name) = value {
                    self.claim(name, half, Site::Block);
                    continue;
                }
                // A group lists declarations too, and its members are an
                // argument rather than entries of a block.
                if value.callee_name() == Some("group") {
                    self.claim_members(value, half);
                }
                // And so does a custom page, one construct further in: its
                // `controls` are claimed by the page, which is claimed by the
                // block, which is what carries the half down to them.
                if let Some(("page", which)) = value.callee_field()
                    && which.text == "custom"
                {
                    self.claim_controls(value, half);
                }
            }
        }
    }

    /// Records that this block lists this declaration. Claim rules 2 and 3, and
    /// the collision between the define this earns and the author's `<const>`s.
    fn claim(&mut self, name: &Name, half: Half, site: Site) {
        let Some(deferred) = self
            .resolved
            .deferred
            .get(&name.text)
            .map(|d| (d.kind, d.value))
        else {
            self.diags.push(
                Diagnostic::error(
                    Code::UnknownField,
                    name.span,
                    format!("`{}` is not {}", name.text, site.lists()),
                )
                .note(site.how()),
            );
            return;
        };

        // The right kind of declaration in the wrong construct. A control in a
        // block would be a window with no dialog to sit in, and a section in a
        // `controls` list would be an install-time thing among drawing ones.
        if !site.accepts(deferred.0) {
            self.diags.push(
                Diagnostic::error(
                    Code::BadFieldValue,
                    name.span,
                    format!(
                        "`{}` is a `{}`, and this lists {}",
                        name.text,
                        deferred.0.word(),
                        site.lists()
                    ),
                )
                .note(match site {
                    Site::Block => {
                        "a control is listed by the `controls` of a `page.custom {}`, because a \
                         window needs the dialog it sits in (§15.32)"
                    }
                    Site::Controls => {
                        "a section is listed by `installer {}` or `uninstaller {}`: it is what \
                         gets installed, not what is drawn (§13)"
                    }
                }),
            );
            return;
        }

        // One declaration, one place in the tree. Listing it twice in a block
        // would emit its body twice under two indices, and listing it in both
        // would emit it once with `un.` and once without — two sections sharing
        // a name, and a handle that means neither.
        if let Some(previous) = self.claims.get(&name.text) {
            let previous_line = previous.span.start_line;
            let message = if previous.half != half {
                format!(
                    "`{}` is listed in both `{}` and `{half}`",
                    name.text, previous.half
                )
            } else if site == Site::Controls && previous.site == Site::Controls {
                format!("`{}` is listed twice among `controls`", name.text)
            } else {
                format!("`{}` is listed twice in `{half} {{}}`", name.text)
            };
            self.diags.push(
                Diagnostic::error(Code::DuplicateBlock, name.span, message)
                    .note(format!("the first one is at line {previous_line}"))
                    .note(
                        "a declaration is one thing, in one place: list it once and address it by \
                         its name from either half's code",
                    ),
            );
            return;
        }

        // The name a claim earns is the compiler's, but it lands in a namespace
        // the author writes in too — a section's `!define` beside the
        // `<const>`s, a control's `Var` beside the globals — and NSIS holds one
        // name once: a second `!define` is a warning it then ships (§12), and a
        // second `Var` is an error.
        let (earned, kind_of_name) = match deferred.0.is_control() {
            true => (control_var(&name.text, half), "global"),
            false => (index_name(&name.text, half), "`<const>`"),
        };
        let taken = match deferred.0.is_control() {
            true => self
                .resolved
                .globals
                .iter()
                .any(|global| global.name == earned),
            false => self.resolved.consts.contains_key(&earned),
        };
        if taken {
            self.diags.push(
                Diagnostic::error(
                    Code::DuplicateBlock,
                    name.span,
                    format!("`{earned}` is already a {kind_of_name}"),
                )
                .note(format!(
                    "listing `{}` takes `{earned}` for its handle, and NSIS holds one name once",
                    name.text
                )),
            );
            return;
        }

        self.claims.insert(
            name.text.clone(),
            Claim {
                half,
                span: name.span,
                site,
            },
        );

        // A group's members are claimed by the block that claims the group: the
        // heading is what carries the half down to them, and a section under a
        // group is not listed anywhere else.
        let (kind, value) = deferred;
        if kind == DeferredKind::Group {
            self.claim_members(value, half);
        }
    }

    /// The bare names among a `group`'s sections, claimed for the half that
    /// claimed the group.
    fn claim_members(&mut self, group: &Expr, half: Half) {
        for member in group_members(group).unwrap_or_default() {
            if let TableField::Positional {
                value: Expr::Name(name),
            } = member
            {
                self.claim(name, half, Site::Block);
            }
        }
    }

    /// The bare names among a `page.custom`'s `controls`, claimed for the half
    /// whose block holds the page.
    ///
    /// Syntactic like the rest of the pass, and for the same reason: the shape
    /// errors belong to [`Self::custom_page`], which reports them once where the
    /// page is lowered.
    fn claim_controls(&mut self, page: &Expr, half: Half) {
        for control in page_controls(page).unwrap_or_default() {
            if let TableField::Positional {
                value: Expr::Name(name),
            } = control
            {
                self.claim(name, half, Site::Controls);
            }
        }
    }

    /// `onInit(function() … end)`. The leading `.` is emitted, never written
    /// (§15.7), and so is the `un.` on the uninstaller's.
    fn callback(&mut self, value: &Expr, half: Half, which: &str) {
        let Expr::Call { args, .. } = value else {
            return;
        };
        let [
            Expr::Function {
                params,
                block,
                span,
            },
        ] = args.as_slice()
        else {
            self.todo(value.span(), &format!("this `{which}` form"));
            return;
        };
        if !params.is_empty() {
            self.diags.push(
                Diagnostic::error(
                    Code::WrongArity,
                    *span,
                    format!("`{which}` takes no arguments"),
                )
                .note("NSIS calls it, and `Call` has no argument list (§3)"),
            );
            return;
        }

        let name = match half {
            Half::Installer => format!(".{which}"),
            Half::Uninstaller => format!("un.{which}"),
        };
        // The global initialisers go in front of whatever the user wrote, so a
        // `.onInit` that reads a global sees its value (§15.24).
        let block = if half == Half::Installer && which == "onInit" {
            self.on_init = true;
            let mut all = std::mem::take(&mut self.global_inits);
            all.extend(block.iter().cloned());
            all
        } else {
            block.clone()
        };
        let body = self.body(&block, &[], *span, None, Some(half));
        self.module.functions.push(ir::Function { name, body });
    }

    /// `group("Tools", { expanded = true }, { section(…), section(…) })` — a
    /// heading in the components tree, and `SectionGroup`/`SectionGroupEnd`.
    ///
    /// The sections are a **list argument** rather than the positional entries
    /// of a block, because a group is not a scope: it has no body, nothing runs
    /// in it, and the only thing between the two NSIS lines is other sections.
    /// A block would promise otherwise.
    ///
    /// One level deep. NSIS accepts nesting and MUI2's tree draws it, but a
    /// nested group has no separate meaning to anything else — it is still a
    /// flat run of sections with a heading — so the surface offers the level
    /// that pays for itself and says so.
    fn group(&mut self, value: &Expr, half: Half, index: Option<String>) {
        let Expr::Call { args, .. } = value else {
            self.todo(value.span(), "this entry");
            return;
        };
        // §15.23's pair, the same as `section`'s: a short form for a group with
        // nothing to configure, and a table form whose array part is the name
        // and whose hash part is `expanded` and the sections it holds.
        let (name, options, members) = match args.as_slice() {
            [name, members @ Expr::Table { .. }] => (name, Vec::new(), members),
            [Expr::Table { fields, span }] => {
                let Some(declared) = self.declaration(fields, *span, "group", "sections") else {
                    return;
                };
                (declared.name, declared.options, declared.holds)
            }
            _ => {
                self.todo(value.span(), "this `group` form");
                return;
            }
        };
        let Expr::Table {
            fields: members, ..
        } = members
        else {
            self.bad_value(
                members.span(),
                "sections",
                "a list of sections",
                "a group holds sections and nothing else: there is no body between \
                 `SectionGroup` and `SectionGroupEnd`",
            );
            return;
        };

        let Some(name) = self.constant_string(name, "group") else {
            return;
        };
        let mut expanded = false;
        for (name, value) in options {
            match name.text.as_str() {
                "expanded" => match self.constant(value) {
                    Some(ConstValue::Bool(flag)) => expanded = flag,
                    _ => self.bad_value(
                        value.span(),
                        "expanded",
                        "a `bool`",
                        "it becomes `SectionGroup /e`, which opens the heading in the components tree",
                    ),
                },
                other => {
                    self.diags.push(
                        Diagnostic::error(
                            Code::UnknownField,
                            name.span,
                            format!("`{other}` is not a `group` option"),
                        )
                        .note("the options are `expanded`"),
                    );
                }
            }
        }

        let mut sections = Vec::new();
        for member in members {
            let TableField::Positional { value } = member else {
                self.todo(value.span(), "a named entry in a `group`'s sections");
                continue;
            };
            if value.callee_name() == Some("group") {
                self.todo(value.span(), "a `group` inside a `group`");
                continue;
            }
            // A bare name: the group is listing a declaration, exactly as a
            // block does, and the claim rules are the same ones.
            if let Expr::Name(member) = value {
                let Some((declared, kind, index)) = self.listed(member) else {
                    continue;
                };
                if kind == DeferredKind::Group {
                    self.todo(value.span(), "a `group` inside a `group`");
                    continue;
                }
                if let Some(mut section) = self.section(declared, half) {
                    section.index_name = Some(index);
                    sections.push(section);
                }
                continue;
            }
            if let Some(section) = self.section(value, half) {
                sections.push(section);
            }
        }

        // An empty group compiles to a heading with nothing under it, which is
        // a run of two NSIS lines that does nothing at all. Saying so is worth
        // more than emitting it.
        if sections.is_empty() {
            self.diags.push(
                Diagnostic::error(
                    Code::BadFieldValue,
                    value.span(),
                    format!("`{name}` is a `group` with no sections"),
                )
                .note("a heading with nothing under it is not drawn"),
            );
            return;
        }

        self.module
            .sections
            .push(ir::SectionItem::Group(ir::SectionGroup {
                name: format!("{}{name}", half.prefix()),
                expanded,
                // Set when the group was listed by name; a `group { … }` written
                // inline in the block is addressed by nothing, so NSIS is asked
                // to define nothing.
                index_name: index,
                sections,
            }));
    }

    fn section(&mut self, value: &Expr, half: Half) -> Option<ir::Section> {
        let Expr::Call { callee, args, .. } = value else {
            self.todo(value.span(), "this entry");
            return None;
        };
        let Expr::Name(callee) = callee.as_ref() else {
            self.todo(value.span(), "this entry");
            return None;
        };
        if callee.text != "section" {
            self.todo(value.span(), &format!("`{}`", callee.text));
            return None;
        }

        // §15.23's pair: `section("Core", fn)` when there is nothing to
        // configure, and `section { "Core", required = true, body = fn }` when
        // there is. Two forms and not three — the middle-table
        // `section(name, options, body)` was the one shape in the surface that
        // put options between two parameters, and it is gone.
        //
        // In the table the array part is the parameters and the hash part the
        // options, exactly as `file { "docs/", recursive = true }` mirrors
        // `File /r "docs\"`. `body` is a named key rather than a second
        // positional because NSIS does not pass it either: it is what sits
        // between `Section` and `SectionEnd`.
        let (name, options, body) = match args.as_slice() {
            [name, body @ Expr::Function { .. }] => (name, Vec::new(), body),
            [Expr::Table { fields, span }] => {
                let declared = self.declaration(fields, *span, "section", "body")?;
                (declared.name, declared.options, declared.holds)
            }
            _ => {
                self.todo(value.span(), "this `section` form");
                return None;
            }
        };
        let Expr::Function { block, span, .. } = body else {
            self.bad_value(
                body.span(),
                "body",
                "a function",
                "it becomes the section's body, which is what NSIS writes between `Section` \
                 and `SectionEnd`",
            );
            return None;
        };

        let name = self.constant_string(name, "section")?;
        let mut optional = false;
        let mut required = false;
        let mut optional_span = None;
        let mut inst_types = Vec::new();
        let mut size = None;
        for (name, value) in options {
            match name.text.as_str() {
                "optional" => match self.constant(value) {
                    Some(ConstValue::Bool(flag)) => {
                        optional = flag;
                        optional_span = flag.then_some(name.span);
                    }
                    _ => self.bad_value(
                        value.span(),
                        "optional",
                        "a `bool`",
                        "it becomes `Section /o`, which starts unselected in the components tree",
                    ),
                },
                "required" => match self.constant(value) {
                    Some(ConstValue::Bool(flag)) => {
                        required = flag;
                    }
                    _ => self.bad_value(
                        value.span(),
                        "required",
                        "a `bool`",
                        "it becomes `SectionIn RO`, which greys the box out and always installs it",
                    ),
                },
                "size" => match self.constant(value) {
                    // NSIS reads `AddSize` as kilobytes and has no use for a
                    // negative one: a section cannot give space back.
                    Some(ConstValue::Int(kb)) if kb >= 0 => size = Some(kb as u32),
                    _ => self.bad_value(
                        value.span(),
                        "size",
                        "a whole number of kilobytes",
                        "it becomes `AddSize`, which is added to the space this section is shown \
                         as needing",
                    ),
                },
                "installTypes" => inst_types = self.section_in(value, half),
                other => {
                    self.diags.push(
                        Diagnostic::error(
                            Code::UnknownField,
                            name.span,
                            format!("`{other}` is not a `section` option"),
                        )
                        .note("the options are `optional`, `required`, `installTypes` and `size`"),
                    );
                }
            }
        }

        // `optional` and `required` are not opposites — one says what the box
        // starts as, the other that there is no box — but together they say the
        // section starts unticked and can never be unticked, and NSIS resolves
        // that silently in favour of `RO`. Whichever the author meant, one of
        // the two words is doing nothing.
        if let Some(here) = optional_span.filter(|_| required) {
            self.diags.push(
                Diagnostic::error(
                    Code::BadFieldValue,
                    here,
                    format!("`{name}` is both `optional` and `required`"),
                )
                .note("`optional` starts the box unticked; `required` removes the box"),
            );
        }

        Some(ir::Section {
            // `un.` is how NSIS marks a section as the uninstaller's, and it is
            // emitted rather than written — the whole of §15.3 at the surface
            // is that this prefix has no spelling.
            name: format!("{}{name}", half.prefix()),
            optional,
            inst_types,
            required,
            size,
            // Filled in by the caller when this section was listed by name: a
            // section written inline in the block is addressed by nothing, so
            // NSIS is asked to define nothing.
            index_name: None,
            body: self.body(block, &[], *span, None, Some(half)),
        })
    }

    /// §15.23's table form, split into the three parts a declaration is made
    /// of: the one positional parameter that is its name, the named key holding
    /// what it encloses, and the options beside them.
    ///
    /// The array part is the parameters and the hash part the options — the
    /// division `file { "docs/", recursive = true }` makes against
    /// `File /r "docs\"`. The contents key is in the hash part because NSIS does
    /// not pass it either: a section's body is what sits between `Section` and
    /// `SectionEnd`, not an argument to it.
    fn declaration<'e>(
        &mut self,
        fields: &'e [TableField],
        span: Span,
        what: &str,
        contents: &str,
    ) -> Option<Declaration<'e>> {
        let mut name = None;
        let mut held = None;
        let mut options = Vec::new();
        for field in fields {
            match field {
                TableField::Positional { value } if name.is_none() => name = Some(value),
                // A second unnamed entry is the mistake this shape invites:
                // everything past the name is a switch, and a switch has one.
                TableField::Positional { value } => self.diags.push(
                    Diagnostic::error(
                        Code::BadFieldValue,
                        value.span(),
                        format!("a `{what}` takes one name"),
                    )
                    .note(format!(
                        "the name is the first entry and everything else is named; what this \
                         `{what}` holds goes in `{contents} = …`"
                    )),
                ),
                TableField::Named { name: key, value } if key.text == contents => {
                    held = Some(value)
                }
                TableField::Named { name: key, value } => options.push((key, value)),
            }
        }

        let Some(name) = name else {
            self.diags.push(
                Diagnostic::error(
                    Code::MissingAttribute,
                    span,
                    format!("this `{what}` has no name"),
                )
                .note("the name is the first entry, written without a key"),
            );
            return None;
        };
        let Some(held) = held else {
            self.diags.push(
                Diagnostic::error(
                    Code::MissingAttribute,
                    span,
                    format!("this `{what}` has no `{contents}`"),
                )
                .note(format!(
                    "write `{contents} = …`; the table form names everything except the `{what}`'s \
                     own name"
                )),
            );
            return None;
        };
        Some(Declaration {
            name,
            holds: held,
            options,
        })
    }

    /// A section's `installTypes = { "Full" }`, resolved to the one-based
    /// positions `SectionIn` reads.
    ///
    /// This is the §13 binding, and it is a *compile-time* one: the name a user
    /// writes is the name they declared on the block, and the number NSIS wants
    /// never appears in the source. That the numbering exists in one place is
    /// what makes inserting an install type at the front safe — every section
    /// renumbers, and none of them says a number.
    ///
    /// The declaration list is already lowered by the time any section is,
    /// because `installer {}` reads its named fields before its positional
    /// entries. Ordering matters here and only here, so the two loops stay two.
    fn section_in(&mut self, value: &Expr, half: Half) -> Vec<usize> {
        let Expr::Table { fields, .. } = value else {
            self.bad_value(
                value.span(),
                "installTypes",
                "a list",
                "write `installTypes = { \"Full\" }`, naming types the block declares",
            );
            return Vec::new();
        };

        let declared = match half {
            Half::Installer => self.module.inst_types.clone(),
            Half::Uninstaller => self.module.uninst_types.clone(),
        };
        let mut chosen: Vec<usize> = Vec::new();
        for field in fields {
            let TableField::Positional { value } = field else {
                self.todo(
                    value.span(),
                    "a named entry in a `section`'s `installTypes`",
                );
                continue;
            };
            let Some(name) = self.constant_string(value, "installTypes") else {
                continue;
            };
            let Some(index) = declared.iter().position(|known| *known == name) else {
                let note = if declared.is_empty() {
                    format!(
                        "the `{half}` block declares no install types; write `installTypes = \
                         {{ \"{name}\" }}` beside its sections"
                    )
                } else {
                    format!(
                        "the install types are {}",
                        list(&declared.iter().map(String::as_str).collect::<Vec<_>>())
                    )
                };
                self.diags.push(
                    Diagnostic::error(
                        Code::UnknownField,
                        value.span(),
                        format!("`{name}` is not an install type"),
                    )
                    .note(note),
                );
                continue;
            };
            // NSIS accepts `SectionIn 1 1` and means it once. Writing it twice
            // is a mistake either way, so the duplicate is dropped and said.
            if chosen.contains(&(index + 1)) {
                self.diags.push(Diagnostic::error(
                    Code::BadFieldValue,
                    value.span(),
                    format!("`{name}` is listed twice"),
                ));
                continue;
            }
            chosen.push(index + 1);
        }
        chosen
    }

    fn function(&mut self, keyword: &str, call: &Expr) {
        let Expr::Call { args, .. } = call else {
            return;
        };
        // `resolve` has already validated the shape and reported anything else,
        // so a mismatch here means it was reported once already.
        let [
            Expr::Str(name),
            Expr::Function {
                params,
                block,
                span,
            },
        ] = args.as_slice()
        else {
            return;
        };
        let _ = keyword;

        let body = self.body(block, params, *span, Some(&name.value), None);
        self.module.functions.push(ir::Function {
            name: name.value.clone(),
            body,
        });
    }

    /// One body, one CFG, one register file. Everything about a body is local
    /// to it — the label counter resets (§15.25) and NSIS `Goto` cannot cross
    /// the boundary anyway (§8).
    fn body(
        &mut self,
        block: &Block,
        params: &[Name],
        span: Span,
        owner: Option<&str>,
        half: Option<Half>,
    ) -> Body {
        let signature = owner
            .and_then(|name| self.known.signature(name))
            .cloned()
            .unwrap_or_default();
        let (body, returns) = self.body_with(span, half, |lowerer| {
            lowerer.parameters(params, &signature);
            lowerer.block(block);
        });
        self.returns(owner, &returns);
        body
    }

    /// The same, for a body the compiler assembles rather than one the user
    /// wrote: a custom page's creator is generated instructions with the user's
    /// `pre` and `show` blocks between them, and there is no single [`Block`] to
    /// hand [`Self::body`].
    fn body_with(
        &mut self,
        span: Span,
        half: Option<Half>,
        build: impl FnOnce(&mut BodyLowerer),
    ) -> (Body, Vec<(Vec<Ty>, Span)>) {
        // The install types the block declared, for a `handle.installTypes`
        // write. Copied rather than borrowed because a `func` belongs to no half
        // and the two lists are the block's, not the body's.
        let inst_types = match half {
            Some(Half::Uninstaller) => self.module.uninst_types.clone(),
            _ => self.module.inst_types.clone(),
        };

        let mut lowerer = BodyLowerer {
            diags: self.diags,
            resolved: self.resolved,
            options: self.options,
            known: self.known,
            learned: &mut self.learned,
            globals: &mut self.globals,
            requires: &mut self.requires,
            claims: &self.claims,
            half,
            inst_types,
            body: Body::new(span),
            scopes: vec![Vec::new()],
            loops: Vec::new(),
            returns: Vec::new(),
            span,
            current: Body::ENTRY,
        };
        build(&mut lowerer);
        lowerer.finish()
    }

    /// Every `return` in one body has to agree on how many values it leaves,
    /// because `Call` has no arity: the callee pushes and the caller pops, and a
    /// disagreement is a stack that unbalances at runtime with NSIS reporting
    /// nothing at all (§3).
    fn returns(&mut self, owner: Option<&str>, returns: &[(Vec<Ty>, Span)]) {
        let Some((first, first_span)) = returns.first() else {
            return;
        };
        for (types, span) in returns {
            if types.len() != first.len() {
                self.diags.push(
                    Diagnostic::error(
                        Code::ReturnArity,
                        *span,
                        format!(
                            "this returns {} value(s), and another path returns {}",
                            types.len(),
                            first.len()
                        ),
                    )
                    .note(format!(
                        "the other one is at line {}",
                        first_span.start_line
                    ))
                    .note(
                        "`Call` has no arity — the callee pushes and the caller pops — so the \
                         two would unbalance the stack with no diagnostic from NSIS (§3)",
                    )
                    .note(
                        "a path that falls off the end of the body returns nothing, which counts",
                    ),
                );
                return;
            }
        }

        match owner {
            Some(name) => {
                for (types, _) in returns {
                    self.learned.learn_return(name, types);
                }
            }
            None if !first.is_empty() => {
                self.diags.push(
                    Diagnostic::error(
                        Code::ReturnArity,
                        *first_span,
                        "a `section` cannot return a value",
                    )
                    .note("nothing calls a section, so there is nobody to pop what it pushed")
                    .note("a bare `return` to leave early is fine"),
                );
            }
            None => {}
        }
    }

    // -- constants --------------------------------------------------------

    fn constant(&self, expr: &Expr) -> Option<ConstValue> {
        let consts = &self.resolved.consts;
        crate::resolve::fold(expr, &|name| consts.get(name).map(|c| c.value.clone()))
    }

    /// A field whose value has to be known at compile time. An attribute is a
    /// `!define`-shaped thing, not an instruction, so there is no register for a
    /// runtime value to arrive in.
    fn constant_string(&mut self, expr: &Expr, what: &str) -> Option<String> {
        match self.constant(expr) {
            Some(value) => Some(value.text()),
            None => {
                self.diags.push(
                    Diagnostic::error(
                        Code::BadFieldValue,
                        expr.span(),
                        format!("`{what}` wants a compile-time value"),
                    )
                    .note(
                        "an attribute is written into the script's header, before any \
                         instruction has run (§12)",
                    ),
                );
                None
            }
        }
    }

    /// An attribute's value, as the argument it becomes.
    ///
    /// Everything an attribute can hold is build-time, but "build-time" is not
    /// the same as "a string this compiler knows": `$PROGRAMFILES64` is
    /// expanded by the installer at run time and `${APP}` by the preprocessor,
    /// and both are constants a user writes as an ordinary name (§15.1). So
    /// this walks the same three shapes [`BodyLowerer::simple`] does, and folds
    /// only what is left.
    fn constant_arg(&mut self, expr: &Expr, what: &str) -> Option<ir::Arg> {
        match expr {
            Expr::Name(name) => {
                if let Some(constant) = crate::builtins::constant_named(&name.text) {
                    return Some(constant.arg());
                }
                if let Some(value) = self.resolved.consts.get(&name.text) {
                    return Some(ir::Arg::constant(&name.text, value.value.text()));
                }
                self.constant_string(expr, what).map(ir::Arg::str)
            }
            Expr::Binary {
                op: BinOp::Concat,
                lhs,
                rhs,
                ..
            } => {
                let lhs = self.constant_arg(lhs, what)?;
                let rhs = self.constant_arg(rhs, what)?;
                Some(lhs.concat(rhs))
            }
            other => self.constant_string(other, what).map(ir::Arg::str),
        }
    }

    fn bad_value(&mut self, span: Span, what: &str, wanted: &str, note: &str) {
        self.diags.push(
            Diagnostic::error(
                Code::BadFieldValue,
                span,
                format!("`{what}` wants {wanted}"),
            )
            .note(note),
        );
    }

    fn todo(&mut self, span: Span, what: &str) {
        todo_at(self.diags, span, what);
    }
}

enum Field {
    Attributes,
    Installer,
    Uninstaller,
}

// -- the body lowerer ------------------------------------------------------

/// What a name in a body means. `Const` is not a register at all — a `<const>`
/// is build-time (§7-1), so a use folds rather than reads.
#[derive(Clone, Debug)]
enum Binding {
    Local { slot: Slot, ty: Ty },
    Const(ConstValue),
}

/// Where `break` and `continue()` go. Both are ordinary terminators, which is
/// the whole reason no statement lowerer does label bookkeeping (§8).
struct LoopTargets {
    break_to: BlockId,
    continue_to: BlockId,
}

struct BodyLowerer<'a, 'p> {
    diags: &'a mut Diagnostics,
    resolved: &'a Resolved<'p>,
    options: &'a crate::Options,
    known: &'a Inferred,
    learned: &'a mut Inferred,
    globals: &'a mut GlobalTypes,
    /// Headers and `StrFunc` declarations, shared with every other body: the
    /// collect half of §15.21's collect-then-emit.
    requires: &'a mut Requirements,
    /// Which block listed which section, so that `core.selected` knows the
    /// define it reads and whether this half is the one that has a `core` at all
    /// (claim rule 4).
    claims: &'a BTreeMap<String, Claim>,
    /// The half this body runs in. `None` for a `func`, which either half may
    /// call: there is no wrong half to name a section from, so rule 4 has
    /// nothing to compare against and does not run.
    half: Option<Half>,
    /// The install types the block declared, in order — the name → position
    /// binding a `handle.installTypes = { … }` write resolves against, and the
    /// same one `SectionIn` uses at compile time (§13).
    inst_types: Vec<String>,
    body: Body,
    scopes: Vec<Vec<(String, Binding)>>,
    loops: Vec<LoopTargets>,
    /// What each `return` in this body leaves on the stack.
    returns: Vec<(Vec<Ty>, Span)>,
    /// The statement being lowered, stamped onto every instruction it produces
    /// (§15.22).
    span: Span,
    current: BlockId,
}

impl BodyLowerer<'_, '_> {
    fn finish(mut self) -> (Body, Vec<(Vec<Ty>, Span)>) {
        // Whatever block execution ends in returns; the layout pass drops the
        // instruction when it is the last line anyway.
        if matches!(
            self.body.block(self.current).terminator,
            Terminator::Unreachable
        ) {
            // Falling off the end returns nothing, and that counts against the
            // arity check — but only when it can happen. The block after an
            // unconditional `return` is not a second return path, and reporting
            // it as one would reject the ordinary shape where every path
            // returns explicitly.
            if self.body.reachable(self.current) {
                let span = self.body.block(self.current).span;
                self.returns.push((Vec::new(), span));
            }
            self.body.terminate(self.current, Terminator::Return);
        }
        (self.body, self.returns)
    }

    /// The callee's half of the calling convention: arguments come off the
    /// stack in source order, because the caller pushed them in reverse.
    fn parameters(&mut self, params: &[Name], signature: &Signature) {
        for (index, param) in params.iter().enumerate() {
            let slot = self.body.vreg(param.span);
            self.emit(ir::Instruction::new(
                "Pop",
                vec![ir::Arg::dest(slot.clone())],
            ));
            self.bind(
                &param.text,
                Binding::Local {
                    slot,
                    ty: signature.param(index),
                },
            );
        }
    }

    // -- scopes -----------------------------------------------------------

    fn bind(&mut self, name: &str, binding: Binding) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.push((name.to_string(), binding));
        }
    }

    fn lookup(&self, name: &str) -> Option<&Binding> {
        self.scopes
            .iter()
            .rev()
            .flat_map(|scope| scope.iter().rev())
            .find(|(bound, _)| bound == name)
            .map(|(_, binding)| binding)
    }

    /// A slot for a named value. There is nothing to fail here any more:
    /// twenty registers is a fact about how many values are live at once, and
    /// [`crate::alloc`] is the only pass that can know that (§9-3).
    fn claim_local(&mut self, span: Span) -> Slot {
        self.body.vreg(span)
    }

    /// A slot for an intermediate value. Counted, because Phase 2's exit
    /// criterion is a claim about this number: a fused condition materialises
    /// nothing, so it never moves.
    fn claim_temp(&mut self, span: Span) -> Slot {
        self.body.temps += 1;
        self.body.vreg(span)
    }

    // -- blocks and statements --------------------------------------------

    fn block(&mut self, block: &Block) {
        self.scopes.push(Vec::new());
        for stmt in block {
            self.stmt(stmt);
        }
        self.scopes.pop();
    }

    /// One instruction, attributed to the statement being lowered.
    ///
    /// The attribution is a field rather than a lookup because §15.22's map has
    /// to survive layout, and by then the statement is long gone: the emitter
    /// sees a flat list and the CFG that produced it does not exist any more.
    fn emit(&mut self, instruction: ir::Instruction) {
        let current = self.current;
        let span = self.span;
        self.body.push(current, instruction.at(span));
    }

    /// A plugin call the *compiler* writes: `nsDialogs::Create`, and the
    /// `CreateControl`s under it (§15.32).
    ///
    /// An opaque site rather than a bare [`Self::emit`], for the same reason a
    /// user's plugin call is one: a plugin clobbers every register, and the
    /// saves the allocator inserts are the only thing standing between a
    /// generated dialog and a live value it silently overwrites.
    fn generated_plugin_call(
        &mut self,
        nsis: &str,
        args: Vec<ir::Arg>,
        results: Vec<Slot>,
        span: Span,
    ) {
        let site =
            self.body
                .opaque_site(vec![ir::Instruction::new(nsis, args).at(span)], false, span);
        let current = self.current;
        self.body.push_step(current, ir::Step::Saves(site));
        self.body.calls[site].results = results;
        self.body.push_step(current, ir::Step::Call(site));
    }

    /// Ends the current block with `terminator` and continues in a fresh one.
    /// The fresh block is often unreachable — code after a `break` — and the
    /// layout pass drops it without anybody here having to notice.
    fn terminate(&mut self, terminator: Terminator, next: BlockId) {
        let current = self.current;
        self.body.terminate(current, terminator);
        self.current = next;
    }

    fn stmt(&mut self, stmt: &Stmt) {
        self.span = stmt.span();
        match stmt {
            Stmt::Local {
                names,
                is_const,
                values,
                span,
            } => self.local(names, *is_const, values, *span),

            Stmt::Assign {
                targets,
                values,
                span,
            } => self.assign(targets, values, *span),

            Stmt::Call(call) => self.call_statement(call),

            Stmt::If {
                cond,
                then_block,
                else_block,
                span,
            } => self.if_stmt(cond, then_block, else_block.as_ref(), *span),

            Stmt::While { cond, block, span } => self.while_stmt(cond, block, *span),

            Stmt::NumericFor {
                name,
                start,
                end,
                step,
                block,
                span,
            } => self.numeric_for(name, start, end, step.as_ref(), block, *span),

            Stmt::Do { block, .. } => self.block(block),

            Stmt::Break { span } => match self.loops.last() {
                Some(targets) => {
                    let target = targets.break_to;
                    let next = self.fresh("after_break");
                    self.terminate(Terminator::Jump(target), next);
                }
                None => self.diags.push(
                    Diagnostic::error(Code::BreakOutsideLoop, *span, "`break` is not in a loop")
                        .note("it leaves the innermost `while` or `for`, and there is none here"),
                ),
            },

            Stmt::Return { values, span } => self.return_stmt(values, *span),

            Stmt::GenericFor {
                names,
                iterator,
                block,
                span,
            } => self.generic_for(names, iterator, block, *span),
        }
    }

    /// `return`, which is the only place a value leaves a body.
    ///
    /// Values are pushed in **reverse source order**, so the caller's first
    /// `Pop` is the first return value — the mirror of the parameter rule, and
    /// the reason neither side needs an `Exch` (§11, program 4).
    fn return_stmt(&mut self, values: &[Expr], span: Span) {
        let mut lowered = Vec::with_capacity(values.len());
        for value in values {
            let Some(typed) = self.value(value) else {
                return;
            };
            lowered.push(typed);
        }

        self.returns
            .push((lowered.iter().map(|typed| typed.ty).collect(), span));

        for typed in lowered.into_iter().rev() {
            self.emit(ir::Instruction::new("Push", vec![typed.arg]));
        }

        let next = self.fresh("after_return");
        self.terminate(Terminator::Return, next);
    }

    fn local(&mut self, names: &[Name], is_const: bool, values: &[Expr], span: Span) {
        // `local a, b = f()`: one call, several names. Plural outputs are
        // invisible at the call site (§11), so the *declaration's* arity is what
        // decides, and it comes from the signature table rather than from here.
        if names.len() > 1
            && values.len() == 1
            && !is_const
            && matches!(&values[0], Expr::Call { .. })
        {
            let slots: Vec<Slot> = names
                .iter()
                .map(|name| self.claim_local(name.span))
                .collect();
            let Some(types) = self.call_multi(&values[0], &slots) else {
                return;
            };
            for ((name, slot), ty) in names.iter().zip(slots).zip(types) {
                self.bind(&name.text, Binding::Local { slot, ty });
            }
            return;
        }

        if names.len() != values.len() {
            self.todo(
                span,
                "a `local` with a different number of names and values",
            );
            return;
        }

        for (name, value) in names.iter().zip(values) {
            if is_const {
                match self.constant(value) {
                    Some(folded) => self.bind(&name.text, Binding::Const(folded)),
                    None => self.diags.push(
                        Diagnostic::error(
                            Code::BadFieldValue,
                            value.span(),
                            format!("`{}` is not a build-time constant", name.text),
                        )
                        .note(
                            "a `<const>` folds at compile time and never reaches a register \
                             (§7-1)",
                        ),
                    ),
                }
                continue;
            }

            // The slot is claimed *before* the initialiser is walked, so
            // `local sum = 1 + 1` is one `IntOp` into the local rather than an
            // `IntOp` into a temporary and a `StrCpy` after it (§12).
            let slot = self.claim_local(name.span);
            let Some(ty) = self.value_into(value, &slot) else {
                continue;
            };
            self.bind(&name.text, Binding::Local { slot, ty });
        }
    }

    fn assign(&mut self, targets: &[Expr], values: &[Expr], span: Span) {
        if targets.len() != values.len() {
            self.todo(
                span,
                "an assignment with a different number of targets and values",
            );
            return;
        }

        for (target, value) in targets.iter().zip(values) {
            // `docs.text = ""` — a section's field, which is a `Section*Set` and
            // not a register at all.
            if let Expr::Field { base, name, .. } = target
                && base
                    .name()
                    .is_some_and(|base| self.resolved.deferred.contains_key(base))
            {
                self.handle_write(base, name, value);
                continue;
            }

            let Expr::Name(name) = target else {
                self.todo(target.span(), "this assignment target");
                continue;
            };

            // `currentInstType = "Minimal"` — a name the compiler owns, whose
            // write is an instruction rather than a register (§13).
            if crate::builtins::owned(&name.text) {
                self.owned_write(name, value);
                continue;
            }

            let (slot, declared) = match self.lookup(&name.text).cloned() {
                Some(Binding::Local { slot, ty }) => (slot, Some(ty)),
                Some(Binding::Const(_)) => {
                    self.diags.push(
                        Diagnostic::error(
                            Code::TypeConflict,
                            name.span,
                            format!("`{}` is `<const>`", name.text),
                        )
                        .note("a build-time constant has no register to assign to (§7-1)"),
                    );
                    continue;
                }
                // `$INSTDIR` is a variable, not a constant: `.onInit` reading a
                // prior install location and assigning it is the shape §13
                // calls canonical, and it is the only reason a "constant" here
                // has a `writable` column at all.
                None => match crate::builtins::constant_named(&name.text) {
                    Some(constant) if constant.writable => {
                        (Slot::Global(constant.nsis.to_string()), Some(constant.ty))
                    }
                    Some(_) => {
                        self.diags.push(
                            Diagnostic::error(
                                Code::TypeConflict,
                                name.span,
                                format!("`{}` cannot be assigned to", name.text),
                            )
                            .note(
                                "it describes the machine the installer is running on, and NSIS \
                                 accepts the assignment silently rather than objecting (§13)",
                            ),
                        );
                        continue;
                    }
                    None if self.resolved.global(&name.text) => (
                        Slot::Global(name.text.clone()),
                        self.globals.get(&name.text).map(|(ty, _)| *ty),
                    ),
                    None => {
                        self.undefined(name);
                        continue;
                    }
                },
            };

            let Some(ty) = self.value_into(value, &slot) else {
                continue;
            };

            match (slot, declared) {
                (Slot::Global(name), Some(previous)) if previous != ty => {
                    let sites = self
                        .globals
                        .get(&name)
                        .map(|(_, spans)| spans.clone())
                        .unwrap_or_default();
                    let mut diagnostic = Diagnostic::error(
                        Code::TypeConflict,
                        target.span(),
                        format!("`{name}` is assigned a {ty} here and a {previous} elsewhere"),
                    )
                    .note(
                        "a `Var` is one slot, so a global has one type for its lifetime (§15.24)",
                    );
                    for site in sites {
                        diagnostic =
                            diagnostic.note(format!("assigned at line {}", site.start_line));
                    }
                    self.diags.push(diagnostic);
                }
                (Slot::Global(name), _) => {
                    let entry = self.globals.entry(name).or_insert((ty, Vec::new()));
                    entry.0 = ty;
                    entry.1.push(target.span());
                }
                (_, Some(previous)) if previous != ty => {
                    self.diags.push(
                        Diagnostic::error(
                            Code::TypeConflict,
                            target.span(),
                            format!("`{}` holds a {previous} and is assigned a {ty}", name.text),
                        )
                        .note("a register is one slot; declare a second `local` instead"),
                    );
                }
                _ => {}
            }
        }
    }

    /// A call used for its effect. `continue()` is one of these syntactically
    /// and a terminator semantically — a slight wart, and better than the
    /// alternatives, since Lua has no `continue` and `goto` is what 5.4 added to
    /// spell the idiom (§8).
    fn call_statement(&mut self, call: &Expr) {
        if call.callee_name() == Some("continue")
            && let Expr::Call { args, span, .. } = call
        {
            if !args.is_empty() {
                self.diags.push(
                    Diagnostic::error(Code::WrongArity, *span, "`continue()` takes no arguments")
                        .note("it is a jump wearing a call's syntax (§8)"),
                );
                return;
            }
            match self.loops.last() {
                Some(targets) => {
                    let target = targets.continue_to;
                    let next = self.fresh("after_continue");
                    self.terminate(Terminator::Jump(target), next);
                }
                None => self.diags.push(
                    Diagnostic::error(
                        Code::ContinueOutsideLoop,
                        *span,
                        "`continue()` is not in a loop",
                    )
                    .note("it skips to the next iteration of the innermost `while` or `for`"),
                ),
            }
            return;
        }

        self.call(call, None);
    }

    fn if_stmt(&mut self, cond: &Expr, then_block: &Block, else_block: Option<&Block>, span: Span) {
        // A `<const>` condition folds away entirely, which is what makes
        // `!if`/`!ifdef` need no surface spelling at all (§7-2, §15.6).
        if let Some(ConstValue::Bool(taken)) = self.constant(cond) {
            match (taken, else_block) {
                (true, _) => self.block(then_block),
                (false, Some(block)) => self.block(block),
                (false, None) => {}
            }
            return;
        }

        let n = self.body.construct();
        let then_id = self.fresh(format!("then_{n}"));
        let else_id = else_block.map(|_| self.fresh(format!("else_{n}")));
        let end = self.fresh(format!("endif_{n}"));

        self.branch(cond, then_id, else_id.unwrap_or(end), span);

        self.current = then_id;
        self.block(then_block);
        self.terminate(Terminator::Jump(end), end);

        if let (Some(else_id), Some(else_block)) = (else_id, else_block) {
            self.current = else_id;
            self.block(else_block);
            self.terminate(Terminator::Jump(end), end);
        }

        self.current = end;
    }

    fn while_stmt(&mut self, cond: &Expr, block: &Block, span: Span) {
        let n = self.body.construct();
        let top = self.fresh(format!("while_{n}_top"));
        let inside = self.fresh(format!("while_{n}_body"));
        let end = self.fresh(format!("while_{n}_end"));

        self.terminate(Terminator::Jump(top), top);
        self.branch(cond, inside, end, span);

        self.current = inside;
        self.loops.push(LoopTargets {
            break_to: end,
            continue_to: top,
        });
        self.block(block);
        self.loops.pop();
        self.terminate(Terminator::Jump(top), end);

        self.current = end;
    }

    /// `for i = a, b, step`. The step has to be a build-time constant, because
    /// its *sign* picks the comparison and a runtime step would need both
    /// comparisons and a branch to choose between them.
    fn numeric_for(
        &mut self,
        name: &Name,
        start: &Expr,
        end_expr: &Expr,
        step: Option<&Expr>,
        block: &Block,
        span: Span,
    ) {
        let step_value = match step {
            None => 1,
            Some(expr) => match self.constant(expr) {
                Some(ConstValue::Int(value)) if value != 0 => value,
                Some(ConstValue::Int(_)) => {
                    self.diags.push(
                        Diagnostic::error(Code::BadFieldValue, expr.span(), "a `for` step is zero")
                            .note("the loop would never finish"),
                    );
                    return;
                }
                _ => {
                    self.todo(
                        expr.span(),
                        "a `for` step that is not a build-time constant",
                    );
                    return;
                }
            },
        };

        let slot = self.claim_local(name.span);
        let Some(start_ty) = self.value_into(start, &slot) else {
            return;
        };
        if !start_ty.is_int() {
            self.diags.push(
                Diagnostic::error(
                    Code::TypeMismatch,
                    start.span(),
                    format!("a `for` bound is a {start_ty}, not an int"),
                )
                .note("`for i = 1, 10` counts, and there is nothing else to count over"),
            );
            return;
        }

        // Lua evaluates the bound **once**, before the first iteration, so a
        // temporary will not do: it does not survive the loop. A constant is
        // used where it stands and anything else spills to a local, which the
        // placeholder allocator never reclaims and Phase 3's liveness will.
        let (limit, limit_ty) = match self.constant(end_expr) {
            Some(ConstValue::Int(value)) => (ir::Arg::int(value), ConstValue::Int(value).ty()),
            _ => {
                let bound = self.claim_local(end_expr.span());
                let Some(ty) = self.value_into(end_expr, &bound) else {
                    return;
                };
                (ir::Arg::slot(bound), ty)
            }
        };
        if !limit_ty.is_int() {
            self.diags.push(
                Diagnostic::error(
                    Code::TypeMismatch,
                    end_expr.span(),
                    format!("a `for` bound is a {limit_ty}, not an int"),
                )
                .note("`for i = 1, 10` counts, and there is nothing else to count over"),
            );
            return;
        }

        let n = self.body.construct();
        let top = self.fresh(format!("for_{n}_top"));
        let inside = self.fresh(format!("for_{n}_body"));
        let step_block = self.fresh(format!("for_{n}_step"));
        let end = self.fresh(format!("for_{n}_end"));

        self.terminate(Terminator::Jump(top), top);

        self.scopes.push(vec![(
            name.text.clone(),
            Binding::Local {
                slot: slot.clone(),
                ty: start_ty,
            },
        )]);

        let counter = ir::Arg::slot(slot.clone());
        let op = if step_value > 0 {
            cfg::CmpOp::Le
        } else {
            cfg::CmpOp::Ge
        };
        let family = cfg::IntFamily::of(start_ty.join(limit_ty));
        self.terminate(
            Terminator::Branch {
                test: cfg::Test::Int {
                    op,
                    lhs: counter.clone(),
                    rhs: limit,
                    family,
                },
                then_block: inside,
                else_block: end,
            },
            inside,
        );

        self.loops.push(LoopTargets {
            break_to: end,
            continue_to: step_block,
        });
        self.block(block);
        self.loops.pop();
        self.terminate(Terminator::Jump(step_block), step_block);

        self.emit(ir::Instruction::new(
            "IntOp",
            vec![
                ir::Arg::dest(slot),
                counter,
                ir::Arg::raw("+"),
                ir::Arg::int(step_value),
            ],
        ));
        self.terminate(Terminator::Jump(top), end);

        self.scopes.pop();
        self.current = end;
        let _ = span;
    }

    /// `for x in <iterator>`, where the iterator set is closed (§7).
    ///
    /// The two members run on **different machines**, and a reader has to be
    /// able to tell which from the source alone: `glob` walks the build machine
    /// and unrolls, so there is no loop in the output at all, and `lines` walks
    /// a file the installer has in front of it, so there is.
    fn generic_for(&mut self, names: &[Name], iterator: &Expr, block: &Block, span: Span) {
        let Expr::Call { callee, args, .. } = iterator else {
            self.todo(iterator.span(), "this iterator");
            return;
        };
        let Some(kind) = callee.as_ref().name() else {
            self.todo(iterator.span(), "this iterator");
            return;
        };
        let [name] = names else {
            self.diags.push(
                Diagnostic::error(
                    Code::WrongArity,
                    span,
                    format!(
                        "`{kind}` yields one value, and {} names are bound",
                        names.len()
                    ),
                )
                .note("there are no pairs to unpack: `for k, v` has nothing to iterate over (§7)"),
            );
            return;
        };

        match kind {
            "glob" => self.glob_for(name, args, block, span),
            "lines" => self.lines_for(name, args, block, span),
            other => self.todo(span, &format!("`for … in {other}`")),
        }
    }

    /// `for path in glob("assets/*.txt")` — build-machine iteration.
    ///
    /// The glob runs where `makensis` runs, so the loop is **unrolled** and the
    /// body is lowered once per match with the name bound to a `<const>`.
    /// Matches are sorted, because a directory listing has no order and a
    /// golden file needs one (§14).
    fn glob_for(&mut self, name: &Name, args: &[Expr], block: &Block, span: Span) {
        let [pattern] = args else {
            self.diags.push(
                Diagnostic::error(Code::WrongArity, span, "`glob` takes one pattern").note(
                    "it runs on the build machine, so the pattern has to be known there (§7)",
                ),
            );
            return;
        };
        let Some(ConstValue::Str(pattern)) = self.constant(pattern) else {
            self.diags.push(
                Diagnostic::error(
                    Code::BadFieldValue,
                    pattern.span(),
                    "`glob` needs a build-time pattern",
                )
                .note(
                    "it is expanded while the installer is being built, so there is no register \
                       for a runtime value to arrive in (§7)",
                ),
            );
            return;
        };

        let Some(base) = self.options.base.clone() else {
            self.diags.push(
                Diagnostic::error(
                    Code::BadFieldValue,
                    span,
                    "`glob` needs a directory to be relative to",
                )
                .note(
                    "this source was compiled from a string rather than a file, so there is \
                     nothing for `assets/*.txt` to mean (§9-2)",
                ),
            );
            return;
        };

        let mut matches = match glob(&base, &pattern) {
            Ok(matches) => matches,
            Err(error) => {
                self.diags.push(
                    Diagnostic::error(
                        Code::BadFieldValue,
                        span,
                        format!("`glob` cannot read `{}`: {error}", base.display()),
                    )
                    .note("the pattern is resolved against the source file's directory"),
                );
                return;
            }
        };
        matches.sort();

        for path in matches {
            self.scopes.push(vec![(
                name.text.clone(),
                Binding::Const(ConstValue::Str(path)),
            )]);
            self.block(block);
            self.scopes.pop();
        }
    }

    /// `for line in lines(handle)` — install-time iteration over a file.
    ///
    /// `FileRead` sets the error flag at end of file and leaves the terminator
    /// on the line, so the loop is a `ClearErrors`/`FileRead`/`IfErrors` triple
    /// plus a `${TrimNewLines}`. The trim is what makes this *Lua's* `lines`
    /// rather than NSIS's `FileRead`, and it is the one line here that costs a
    /// header (§15.27).
    fn lines_for(&mut self, name: &Name, args: &[Expr], block: &Block, span: Span) {
        let [handle] = args else {
            self.diags.push(
                Diagnostic::error(Code::WrongArity, span, "`lines` takes one file handle")
                    .note("write `for line in lines(f)`, with `f` from `fileOpen`"),
            );
            return;
        };
        let Some(handle) = self.value(handle) else {
            return;
        };
        if handle.ty != Ty::Handle && handle.ty != Ty::Unknown {
            self.diags.push(
                Diagnostic::error(
                    Code::TypeMismatch,
                    span,
                    format!("`lines` wants a handle, and this is a {}", handle.ty),
                )
                .note("a handle comes from `fileOpen` and from nowhere else"),
            );
            return;
        }
        self.requires.headers.insert("TextFunc".to_string());

        let slot = self.claim_local(name.span);
        let n = self.body.construct();
        let top = self.fresh(format!("for_{n}_top"));
        let inside = self.fresh(format!("for_{n}_body"));
        let end = self.fresh(format!("for_{n}_end"));

        self.terminate(Terminator::Jump(top), top);
        // `FileRead` reports end of file through the error flag, so the flag has
        // to be clear before it — an error left over from anything earlier in
        // the body would end the loop before it started.
        self.emit(ir::Instruction::new("ClearErrors", Vec::new()));
        self.emit(ir::Instruction::new(
            "FileRead",
            vec![handle.arg.clone(), ir::Arg::dest(slot.clone())],
        ));
        self.terminate(
            Terminator::Branch {
                test: cfg::Test::Predicate {
                    name: "IfErrors".to_string(),
                    args: Vec::new(),
                    keywords: Vec::new(),
                },
                then_block: end,
                else_block: inside,
            },
            inside,
        );
        // Lua's `lines` yields the line without its terminator; `FileRead`
        // includes it. One macro, and the difference between the two languages.
        self.emit(
            ir::Instruction::new(
                "${TrimNewLines}",
                vec![ir::Arg::slot(slot.clone()), ir::Arg::dest(slot.clone())],
            )
            .atomic(),
        );

        self.scopes.push(vec![(
            name.text.clone(),
            Binding::Local { slot, ty: Ty::Str },
        )]);
        self.loops.push(LoopTargets {
            break_to: end,
            continue_to: top,
        });
        self.block(block);
        self.loops.pop();
        self.scopes.pop();

        self.terminate(Terminator::Jump(top), end);
        self.current = end;
    }

    fn fresh(&mut self, hint: impl std::fmt::Display) -> BlockId {
        let span = self.body.block(self.current).span;
        self.body
            .new_block(format!("{}{hint}", cfg::LABEL_PREFIX), span)
    }

    pub(super) fn constant(&self, expr: &Expr) -> Option<ConstValue> {
        let scopes = &self.scopes;
        let resolved = self.resolved;
        crate::resolve::fold(expr, &|name| {
            let local = scopes
                .iter()
                .rev()
                .flat_map(|scope| scope.iter().rev())
                .find(|(bound, _)| bound == name);
            match local {
                Some((_, Binding::Const(value))) => Some(value.clone()),
                // A `local` shadows a top-level `<const>` without folding to it.
                Some((_, Binding::Local { .. })) => None,
                None => resolved.consts.get(name).map(|c| c.value.clone()),
            }
        })
    }

    fn undefined(&mut self, name: &Name) {
        // An NSIS instruction with a Lua spelling is not an unknown name: the
        // compiler knows exactly what it is, and the generic error would tell
        // an NSIS user that it had never heard of the instruction they use most
        // (§5). Checked first, because `strCmp` is also within one case-fold of
        // nothing else.
        if let Some(retired) = crate::retired::lookup(&name.text) {
            self.diags.push(
                Diagnostic::error(
                    Code::NsisRetired,
                    name.span,
                    format!("`{}` is not a function here", name.text),
                )
                .note(format!("write {}", retired.instead))
                .note(format!(
                    "NSIS spells it `{}`; this compiler emits it for you",
                    retired.nsis
                )),
            );
            return;
        }

        let mut diagnostic = Diagnostic::error(
            Code::UndefinedName,
            name.span,
            format!("`{}` is not defined", name.text),
        );
        diagnostic = match crate::builtins::nearest(&name.text) {
            // camelCase in, NSIS casing out — so `detailprint` is a spelling
            // mistake with an obvious fix rather than an unknown name (§6).
            Some(suggestion) => diagnostic.note(format!("did you mean `{suggestion}`?")),
            None => diagnostic.note(
                "resolution is order-free, so this means nowhere in the file — not merely \
                 not yet (§15.6)",
            ),
        };
        self.diags.push(diagnostic);
    }

    fn todo(&mut self, span: Span, what: &str) {
        todo_at(self.diags, span, what);
    }
}

/// The build-machine half of `for … in glob(…)`.
///
/// Deliberately not a dependency: the pattern language is one directory and one
/// filename with `*` and `?` in it, which is what §7 exposes and what the five
/// programs use. Anything larger is a shell's job, and `BUILD.system` is where
/// a shell belongs (§15.8).
///
/// Paths come back **as the source would have written them**, with `/` and
/// relative to the source's directory, so the emitter's path handling applies
/// to a globbed file exactly as it does to a written one.
fn glob(base: &std::path::Path, pattern: &str) -> std::io::Result<Vec<String>> {
    let (directory, name) = match pattern.rsplit_once('/') {
        Some((directory, name)) => (directory, name),
        None => ("", pattern),
    };

    let mut out = Vec::new();
    for entry in std::fs::read_dir(base.join(directory))? {
        let entry = entry?;
        let file = entry.file_name().to_string_lossy().into_owned();
        if !matches_pattern(&file, name) || entry.path().is_dir() {
            continue;
        }
        out.push(if directory.is_empty() {
            file
        } else {
            format!("{directory}/{file}")
        });
    }
    Ok(out)
}

/// `*` matches any run, `?` matches one character, everything else is literal.
fn matches_pattern(name: &str, pattern: &str) -> bool {
    let (name, pattern): (Vec<char>, Vec<char>) =
        (name.chars().collect(), pattern.chars().collect());
    // The textbook two-pointer walk with one backtrack point, which is linear
    // and needs no allocation — a recursive matcher on a pathological pattern
    // is exponential, and a glob is user input.
    let (mut n, mut p) = (0usize, 0usize);
    let (mut star, mut resume) = (None, 0usize);
    while n < name.len() {
        if p < pattern.len() && (pattern[p] == '?' || pattern[p] == name[n]) {
            n += 1;
            p += 1;
        } else if p < pattern.len() && pattern[p] == '*' {
            star = Some(p);
            resume = n;
            p += 1;
        } else if let Some(previous) = star {
            p = previous + 1;
            resume += 1;
            n = resume;
        } else {
            return false;
        }
    }
    pattern[p..].iter().all(|c| *c == '*')
}

fn todo_at(diags: &mut Diagnostics, span: Span, what: &str) {
    diags.push(
        Diagnostic::error(
            Code::NotYetImplemented,
            span,
            format!("{what} is not in this version's exposed set"),
        )
        .note("it is scheduled rather than missing: `installua coverage` counts it (PLAN §0)"),
    );
}

/// Required attributes, checked once at the end rather than at the block, so
/// the message can name what is missing rather than what is present.
pub fn check_required(module: &ir::Module, diags: &mut Diagnostics) {
    let has_out_file = module
        .attributes
        .iter()
        .any(|instruction| instruction.name == "OutFile");
    if !has_out_file {
        diags.push(
            Diagnostic::error(
                Code::MissingAttribute,
                Span::default(),
                "`outFile` is required",
            )
            .note("`makensis` has no default for it, and fails without one"),
        );
    }
}

/// The Lua a [`table::Setting::Table`] wants, written out: `root = …, key = …`.
/// Every diagnostic about one of these ends by showing the shape, because the
/// keys are the whole of what a caller has to know and naming the missing one
/// without the rest sends them back to the documentation.
fn shape(parts: &[table::Part]) -> String {
    parts
        .iter()
        .map(|part| format!("{} = …", part.field))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The parts a caller may leave out, which is the snapshot's answer about their
/// positions and never the row's. Empty for most rows, so the note that names
/// them is only printed when there is something to name.
fn optional_parts(entry: &table::Instruction, parts: &[table::Part]) -> Vec<&'static str> {
    parts
        .iter()
        .enumerate()
        .filter(|(index, _)| {
            entry
                .params
                .get(*index)
                .is_some_and(|param| !param.required())
        })
        .map(|(_, part)| part.field)
        .collect()
}

fn list(names: &[&str]) -> String {
    names
        .iter()
        .map(|name| format!("`{name}`"))
        .collect::<Vec<_>>()
        .join(", ")
}
