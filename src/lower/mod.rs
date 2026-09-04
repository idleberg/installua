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
//! edge of the vertical slice, and deliberately a different code from
//! [`Code::UnknownField`], which means *no version will ever accept this*. The
//! first is a five-second wait and the second is a five-second fix, and
//! collapsing them is how a `todo` count stops predicting anything.

pub mod callback;
pub mod control;
mod expr;
mod fields;
mod handle;
mod insttype;
mod languages;
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

use fields::Fields;

/// The `attributes {}` surface, read from the census rather than frozen here: a
/// name is an attribute exactly when a [`table::Class::Attribute`] row claims
/// it, which is what makes a new setting one overlay line.
///
/// A dotted name is not one of these: it is a member of a group, and the
/// *group* is the name written here — which is why [`attribute_groups`] is
/// chained on rather than the dots being merely dropped.
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
        .chain(attribute_groups())
        .collect()
}

/// The nested `attributes {}` fields: `manifest = { … }`, `versionInfo = { … }`
/// and whatever the table grows next.
///
/// A group is **derived** and never listed: a name is one exactly when some
/// `Attribute` row's field path is `group.field`, which keeps a new nested
/// setting one overlay line the way a flat one is. The alternative was a
/// `const` beside the rows, and a second place to add the same name is a second
/// place to forget it.
///
/// One dot, not two. `page.license.file` is a page setting, reached through the
/// page it names and never through `attributes {}` — the same dotted
/// convention, one level deeper, and the depth is what tells the two apart.
///
/// [`BLOCK_OWNERS`] is the other half of that: a dotted name whose owner is a
/// block belongs to the block, not to a group inside `attributes {}`.
pub fn attribute_groups() -> Vec<&'static str> {
    let mut groups: Vec<&'static str> = Vec::new();
    for field in table::table().iter().filter_map(|entry| match entry.class {
        table::Class::Attribute(_) => entry.installua,
        _ => None,
    }) {
        let Some((group, member)) = field.split_once('.') else {
            continue;
        };
        if !member.contains('.') && !BLOCK_OWNERS.contains(&group) && !groups.contains(&group) {
            groups.push(group);
        }
    }
    groups
}

/// The dotted owners that are **blocks**, and so never a group inside
/// `attributes {}`.
///
/// `installer.checkBitmap` is a field of `installer {}`, written there and
/// nowhere else; `manifest.gdiScaling` is a field of a table inside
/// `attributes {}`. Both are one dot, so the depth cannot tell them apart and
/// something has to. Three names, checked against the table by
/// `every_dotted_owner_is_a_block_or_a_group`, which is what stops a fourth
/// convention from quietly becoming a group in `attributes {}`.
const BLOCK_OWNERS: &[&str] = &["installer", "uninstaller", "page"];

/// What `attributes {}` has to emit before what, measured rather than guessed.
///
/// NSIS has no general rule here. A handful of commands refuse to run once
/// something ahead of them has changed the header or chosen the stub, and every
/// other attribute is indifferent — so the only honest way to find the handful
/// was to ask `makensis`: each attribute line the overlay can write, moved to
/// the front of the block and then to the back, assembled under `-WX`. Three
/// fields answered, and the third was not a refusal at all:
///
/// | field | what `makensis` 3.12 says when it comes later |
/// | --- | --- |
/// | `cpu` | *Can't change target architecture after data already got compressed or header already changed!* — `brandingImage` and the `portableExecutable` rows all change the header. |
/// | `compressor` | the same error, and *warning 8026: SetCompressorDictSize … Effectively ignored* when `compressorDictSize` is read first. |
/// | `brandingImage` | nothing: `PERemoveResource` before `AddBrandingImage` **segfaults** `makensis`, exit −11, no diagnostic and no installer. |
///
/// That last one is why this is a list and not a rule of thumb. The other two
/// announce themselves the first time somebody writes the table the wrong way
/// round; a crash announces nothing, and a build that dies with no message is
/// the one failure a user cannot act on.
///
/// `unicode` is absent because it is not a line the lowering places: it sets a
/// field the emitter reads first, so it is already ahead of everything here.
/// Every field absent from this list ranks [`LATE`] and keeps the order it was
/// written in, which a stable sort preserves.
const ORDERED: &[(&str, u8)] = &[("cpu", 0), ("compressor", 1), ("brandingImage", 2)];

/// The rank of an attribute with no ordering constraint.
const LATE: u8 = 3;

fn attribute_rank(field: &TableField) -> u8 {
    let TableField::Named { name, .. } = field else {
        return LATE;
    };
    ORDERED
        .iter()
        .find(|(ordered, _)| *ordered == name.text)
        .map_or(LATE, |(_, rank)| *rank)
}

/// The group a name belongs to, when the name is a member written outside it.
///
/// Two spellings reach here and both are the same mistake. `manifestGdiScaling`
/// is the prefix NSIS puts on the command, carried into a language that puts it
/// on the table instead; `gdiScaling` on its own is the member with the table
/// left off. Neither is a name this language has, and the useful answer to both
/// is where it does live.
///
/// A member claimed by two groups is answered with the first, which is the
/// table's order. There are none today, and if there ever are, naming one place
/// to write it beats naming none.
fn flattened(name: &str) -> Option<(&'static str, &'static str)> {
    attribute_groups().into_iter().find_map(|group| {
        let member = group_fields(group).into_iter().find(|member| {
            let mut capitalised = member.chars();
            let tail = match capitalised.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), capitalised.as_str()),
                None => return false,
            };
            name == *member || name == format!("{group}{tail}")
        })?;
        Some((group, member))
    })
}

/// The members of one group, without the group and the dot.
///
/// In the order the table lists them, which is `-CMDHELP`'s: a group is a
/// window onto rows that were always there, so the order a user reads in a
/// diagnostic is the order they read in the reference.
pub fn group_fields(group: &str) -> Vec<&'static str> {
    table::table()
        .iter()
        .filter(|entry| matches!(entry.class, table::Class::Attribute(_)))
        .filter_map(|entry| entry.installua?.strip_prefix(group)?.strip_prefix('.'))
        .filter(|member| !member.contains('.'))
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
    "headerColors",
    "abortPrompt",
    "autoClose",
    "headerImage",
    "wizardImage",
    "smallDescriptions",
];

/// The four words MUI2's image branch tests for, in the order its `!if` chain
/// tests them.
///
/// MUI2 spells them and this compiler does not rename them: an unknown value is
/// a `!warning` there and an error here, and the words are the ones a user
/// finds in MUI2's own Readme and in every script they are porting.
const STRETCH_MODES: &[&str] = &[
    "FitControl",
    "AspectFitHeight",
    "NoStretchNoCrop",
    "NoStretchNoCropNoAlign",
];

/// A hook MUI2 calls from a callback of its own: a define holding a function
/// name, and the function beside it.
///
/// Three of the four NSIS callbacks a MUI2 script wants are **MUI2's** —
/// `MUI2.nsh` writes `.onGUIInit`, `.onUserAbort` and `.onMouseOverSection`
/// itself — so a script cannot write them and this is the only door in. `onInit`
/// is not here for exactly that reason: NSIS's `.onInit` is nobody else's, so
/// the compiler writes it directly.
#[derive(Clone, Copy, Debug)]
struct MuiHook {
    /// The word written in the block, which is NSIS's own name for the callback
    /// MUI2 calls it from — so a script being ported greps for it and finds it.
    word: &'static str,
    install: &'static str,
    uninstall: &'static str,
    /// Whether MUI2 reaches this one only when the uninstaller has a page.
    /// `MUI_INSERT` writes the `un.` halves of `.onGUIInit` and `.onUserAbort`
    /// behind `!ifdef MUI_UNINSTALLER`, which `MUI_UNPAGE_INIT` sets — so
    /// without a page the define is written, the function is written, and
    /// nothing calls either.
    needs_unpage: bool,
}

const MUI_HOOKS: &[MuiHook] = &[
    MuiHook {
        word: "onGUIInit",
        install: "MUI_CUSTOMFUNCTION_GUIINIT",
        uninstall: "MUI_CUSTOMFUNCTION_UNGUIINIT",
        needs_unpage: true,
    },
    MuiHook {
        word: "onUserAbort",
        install: "MUI_CUSTOMFUNCTION_ABORT",
        uninstall: "MUI_CUSTOMFUNCTION_UNABORT",
        needs_unpage: true,
    },
    // Not `needs_unpage`: the block this one is called from is the compiler's
    // own, so it exists whenever the hook does.
    MuiHook {
        word: "onMouseOverSection",
        install: "MUI_CUSTOMFUNCTION_ONMOUSEOVERSECTION",
        uninstall: "MUI_CUSTOMFUNCTION_UNONMOUSEOVERSECTION",
        needs_unpage: false,
    },
];

impl MuiHook {
    fn named(word: &str) -> Option<&'static MuiHook> {
        MUI_HOOKS.iter().find(|hook| hook.word == word)
    }

    fn define(&self, half: Half) -> &'static str {
        match half {
            Half::Installer => self.install,
            Half::Uninstaller => self.uninstall,
        }
    }
}

/// The fields NSIS reads once for the whole script, so only `installer {}` has
/// them: written in both blocks they would define one name twice, which is a
/// redefinition warning and so an error under `-WX` (tier 3).
///
/// `icon` is not one of these. It is two defines — `MUI_ICON` and `MUI_UNICON`
/// — and is genuinely the same field for the other half.
const ONCE_GLOBAL_FIELDS: &[&str] = &[
    "installDir",
    "checkBitmap",
    "installColors",
    "progressBar",
    "licenseBkColor",
    "headerColors",
    "smallDescriptions",
];

/// NSIS numbers install types one to thirty-two and rejects anything else
/// outright — `SectionIn 0 out of range 1..32` — so the ceiling is the format's
/// and not a policy of this compiler's.
const MAX_INST_TYPES: usize = 32;

/// Which of the two halves a declaration belongs to.
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

    /// Which slot of a two-element per-half array this is.
    fn index(self) -> usize {
        match self {
            Half::Installer => 0,
            Half::Uninstaller => 1,
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
/// and the handle has to survive from the creator into `leave` — two NSIS
/// functions, with the whole page in between (ruling 7). The allocator never
/// sees it, which is exactly what [`Slot::Global`] means.
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

/// The `Var` MUI2 leaves the chosen Start Menu folder in.
///
/// A `Var` because `MUI_PAGE_STARTMENU` takes one — the page stores into it —
/// and it has to outlive the page by the whole of the install. Named from the
/// local like a control's, and prefixed like a generated label for the same
/// reason: it is one more name in a namespace the author also writes in.
fn start_menu_var(local: &str) -> String {
    format!("{}sm_{local}", cfg::LABEL_PREFIX)
}

/// The name a claim earns, and which namespace it lands in.
///
/// Three kinds of declaration and two namespaces: a section's index is a
/// `!define` beside the author's `<const>`s, and a control's handle and a start
/// menu page's folder are `Var`s beside the author's globals. NSIS holds one
/// name once in either, which is what the caller checks.
fn earned_name(local: &str, kind: DeferredKind, half: Half) -> (String, &'static str) {
    match kind {
        DeferredKind::Control(_) => (control_var(local, half), "global"),
        DeferredKind::StartMenu => (start_menu_var(local), "global"),
        _ => (index_name(local, half), "`<const>`"),
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
                "a bare name here lists a declaration: bind one with `local x = section { … }`"
            }
            Site::Controls => {
                "a bare name here lists a declaration: bind one with `local x = text { … }`"
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

/// Which of the seven a section has and which a group has, for the stub.
///
/// A group is a section to NSIS — one index, one flags word — and the two it
/// does not have are the two only a section answers for: it is charged no space
/// and belongs to no install type. The editor offered both for as long as its
/// class simply inherited the other's.
pub fn handle_field_surface() -> Vec<(&'static str, bool, bool)> {
    HANDLE_FIELDS
        .iter()
        .map(|name| {
            let on = match handle_field(name) {
                Some(HandleField::Flag { on, .. }) => on,
                Some(HandleField::Size | HandleField::InstallTypes) => Where::Sections,
                _ => Where::Both,
            };
            (
                *name,
                on.accepts(DeferredKind::Section),
                on.accepts(DeferredKind::Group),
            )
        })
        .collect()
}

/// The `section(…)` options, and the `group(…)` options, which are the note the
/// two errors read out — and the list a stub can be checked against.
pub const SECTION_OPTIONS: &[&str] = &[
    "optional",
    "required",
    "installTypes",
    "size",
    "description",
];

pub const GROUP_OPTIONS: &[&str] = &["expanded", "description"];

/// The `versionInfo = { … }` fields, for the same reason.
pub const VERSION_INFO_FIELDS: &[&str] = &["product", "file", "keys"];

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
    /// What the creator runs against the control once it exists: the
    /// `ADDSTRING`s that fill a list, and the `LoadAndSetImage` that gives a
    /// `bitmap` its picture.
    post: Vec<Post>,
    /// The callbacks this control was declared with, as the plugin method that
    /// registers each and the generated function it points at.
    ///
    /// Separate from [`Created::post`] because registering one is not a single
    /// instruction: the address has to be taken into a register first, and the
    /// plugin call that takes it is an opaque site.
    events: Vec<(&'static str, String)>,
    span: Span,
}

/// One instruction addressed to a control that has just been made.
///
/// The handle is a hole rather than an argument, because it is not known until
/// the creator runs — a claimed control's is its `Var` and an unclaimed one's is
/// whatever register the allocator gives it — and the two instructions here do
/// not put it in the same place.
struct Post {
    nsis: &'static str,
    /// Arguments before the handle: `LoadAndSetImage`'s `/STRINGID`, and
    /// nothing at all for a `SendMessage`.
    before: Vec<ir::Arg>,
    after: Vec<ir::Arg>,
}

/// A `group`'s member list, in either of its two forms and without judging the
/// call. The shape errors belong to [`Lowerer::group`], which reports them once
/// where the group is lowered; this is the claim pass looking for the bare
/// names inside. A `page.custom`'s `controls = { … }`, without judging the
/// page. The shape errors belong to [`Lowerer::custom_page`]; this is the claim
/// pass looking for the bare names inside.
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

/// One named member of a table, by the name a user writes.
fn named<'e>(fields: &'e [TableField], name: &str) -> Option<&'e Expr> {
    fields.iter().find_map(|entry| match entry {
        TableField::Named { name: key, value } if key.text == name => Some(value),
        _ => None,
    })
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
    /// `subCaption = "Terms"`: an NSIS line this compiler writes itself, which
    /// no other page setting is.
    ///
    /// The doc comment on [`PageField`] says the line is absent on purpose,
    /// because MUI2 writes `DirText` and `ComponentText` from its own defines
    /// and a second line loses the race. This row is the exception that proves
    /// the rule rather than a hole in it: MUI2 writes **one** `SubCaption` in
    /// its entire source — `SubCaption 4 " "`, inside `MUI_PAGE_INSTFILES`,
    /// blanking the *Completed* caption — and index 4 is the one this language
    /// has no page for. Indices 0 to 3 are unclaimed, so there is nothing to
    /// race.
    ///
    /// The payload is the page's index in each half, `None` where the page has
    /// no number in that half's numbering.
    Caption([Option<u8>; 2]),
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
    /// `colors = { text = "000000", background = "FFFFFF" }`: the field's own
    /// define takes the background and the payload takes the text.
    ///
    /// One field holding two for the reason a control's fields give one level
    /// down — MUI2 spends both of these on a single `SetCtlColors`, so
    /// `bgColor` and `textColor` as separate fields would let a script write
    /// one and get the other from whatever MUI2 had defaulted it to. It is also
    /// what the pair's cross-field constraint becomes: MUI2 reads the text
    /// colour only inside an `!ifdef` on the background, so a text colour
    /// written alone is read by nothing, and a shape that asks for both cannot
    /// say the case that does nothing.
    Colors(&'static str),
    /// A string and the one define that gives it more room: `title = "Done"`,
    /// or `title = { text = "Done", lines = 3 }`.
    ///
    /// One field holding two, because the second is geometry for the first and
    /// nothing else. `MUI_FINISHPAGE_TITLE_3LINES` and `…_TEXT_LARGE` change
    /// the height of the box the string is drawn in — a field apiece would let
    /// a script make room and never say what goes in it, and the plain string
    /// stays the plain string.
    Roomy(&'static str, Room),
    /// A table of parts whose *presence* means the setting is on, where the
    /// field's own define is MUI2's opt-**out**: `reboot = false` writes
    /// `MUI_FINISHPAGE_NOREBOOTSUPPORT` and a table writes nothing but its
    /// parts.
    ///
    /// [`Holds::Nested`] inverted, and inverted because MUI2 is: the three
    /// reboot strings and `REBOOTLATER_DEFAULT` are read only inside the
    /// `!ifndef MUI_FINISHPAGE_NOREBOOTSUPPORT` branch, so a table that also
    /// turned the opt-out on would write four defines nothing reads.
    Off(&'static [PageField]),
    /// One of two words, one of which writes the define and the other nothing:
    /// `default = "later"` is `MUI_FINISHPAGE_REBOOTLATER_DEFAULT` and
    /// `default = "now"` is MUI2's own default, which is the absence of it.
    ///
    /// Two words rather than a `bool` because the choice is between two named
    /// things and not between doing and not doing, and a build script can pass
    /// either one without branching.
    Word {
        writes: &'static str,
        silent: &'static str,
    },
    /// A callback that also turns its setting on, the way [`Holds::Text`] does
    /// a string: `call = function() … end` is `MUI_FINISHPAGE_RUN ""` *and*
    /// `…_RUN_FUNCTION`, because MUI2 asks `!ifdef` about the first — that is
    /// what draws the checkbox — and `Call`s the second when it is ticked.
    ///
    /// The stem names the function the compiler writes, since `call` is the
    /// same word under `run` and under `readme` and the two are different
    /// functions.
    Calls {
        function: &'static str,
        stem: &'static str,
    },
    /// A `bool` whose **`false`** writes the define, because MUI2's is an
    /// opt-out: `checked = false` is `MUI_FINISHPAGE_RUN_NOTCHECKED` and `true`
    /// is the absence of it. [`Holds::Flag`] the other way round.
    Not,
    /// A checkbox or a link: a table in one of a few spellings, where the
    /// spelling picks the parts and one of the parts writes the widget's own
    /// define.
    Widget(&'static [Form]),
    /// A box that is either worded or taken away: `checkbox = "Do not create
    /// shortcuts"` writes the payload, `checkbox = false` writes the field's own
    /// define — MUI2's `…_NODISABLE` — and `true` writes neither, which is
    /// MUI2's own box with MUI2's own words.
    ///
    /// One field holding two for [`Holds::Colors`]'s reason: `StartMenu.nsh`
    /// expands `…_TEXT_CHECKBOX` only inside the `!ifndef …_NODISABLE` branch,
    /// so a wording written beside the switch that removes the box is a define
    /// nothing reads, and a shape that asks for both cannot say the case that
    /// does nothing.
    Checkbox(&'static str),
}

/// One spelling of a [`Holds::Widget`].
///
/// `run` has two — a program to `Exec` and a function to `Call` — and they are
/// two part lists rather than one list with optional members, because that is
/// what makes `parameters` beside a function unspellable: MUI2 expands
/// `MUI_FINISHPAGE_RUN_PARAMETERS` only in the branch where there is no
/// function, so a shape that permitted both would write a define nothing reads.
#[derive(Debug)]
struct Form {
    /// The part that has to be written, and that writes the widget's own
    /// define. Which key is present is what picks the spelling.
    key: &'static str,
    /// The parts that have to come with it. `link` is a label *and* the place
    /// it goes: MUI2 writes a click handler that `ExecShell`s
    /// `MUI_FINISHPAGE_LINK_LOCATION` under `!ifdef MUI_FINISHPAGE_LINK`, so a
    /// label without one is a link to nowhere.
    needs: &'static [&'static str],
    parts: &'static [PageField],
}

/// How a [`Holds::Roomy`] field spells its second half.
///
/// Two spellings and not one, because MUI2's two are not the same kind of
/// switch: the title box is two lines tall or three, and the body text either
/// gets the taller box or does not.
#[derive(Clone, Copy, Debug)]
enum Room {
    /// `lines = 3`.
    Lines,
    /// `large = true`.
    Large,
}

/// One setting of one page: the name a user writes, the `MUI_*` define it
/// becomes, and what it holds.
///
/// The NSIS line is absent on purpose. `DirText`, `ComponentText` and
/// `LicenseText` are written by MUI2, from these defines, inside the `PageEx`
/// it generates; a second one written by us assembles clean under `-WX` and
/// then loses the race. What is private to MUI2 is the **line**, and what stays
/// public is the **setting** — the split `icon`/`MUI_ICON` already lives on.
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

/// `subCaption`, the one page setting that is an NSIS **line** and not a
/// `!define`.
///
/// The pair is the page's index in each half, because `SubCaption` and
/// `UninstallSubCaption` number their own pages and neither numbering is the
/// other's: `instFiles` is 3 installing and 1 uninstalling, and three of the
/// five installer pages have no uninstaller number at all. `define` is empty
/// and unread — see [`Holds::Caption`] for why this one is safe to write where
/// `ComponentText` is not.
const fn caption(indices: [Option<u8>; 2]) -> PageField {
    PageField {
        installua: "subCaption",
        define: "",
        holds: Holds::Caption(indices),
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

/// The fourth hook, and why it is not in `COMMON_FIELDS`: `Pages.nsh` writes it
/// the same way as the other three, but only the nsDialogs pages — welcome,
/// finish and the start menu — insert `MUI_PAGE_FUNCTION_CUSTOM DESTROYED`.
/// Offered on `page.directory` it would define a name nothing ever calls.
const DESTROYED_FIELD: PageField = field(
    "destroyed",
    "MUI_PAGE_CUSTOMFUNCTION_DESTROYED",
    Holds::Callback,
);

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

/// The welcome page: a title, a body, and the hook the full-window pages have.
///
/// `text` is a plain string and not a [`Holds::Roomy`] one, unlike the finish
/// page's: MUI2 draws this box at a fixed 130u and has no `…_LARGE` for it.
const WELCOME_FIELDS: &[PageField] = &[
    field(
        "title",
        "MUI_WELCOMEPAGE_TITLE",
        Holds::Roomy("MUI_WELCOMEPAGE_TITLE_3LINES", Room::Lines),
    ),
    field("text", "MUI_WELCOMEPAGE_TEXT", Holds::Str),
    DESTROYED_FIELD,
];

/// The install-log page's two endings.
///
/// MUI2 swaps the header strip when the copy stops: one wording for the run
/// that finished and one for the run that did not. Each half stands alone —
/// MUI2 falls back to its own language string for whichever is missing — so
/// these are four fields and not two pairs.
const INSTFILES_FIELDS: &[PageField] = &[
    // 3 installing, 1 uninstalling. The only page with a number in both
    // halves, and the only one whose *other* number — 4 and 2, "Completed" —
    // is the pair MUI2 blanks: this page is that state, and this language has
    // no second name for it.
    caption([Some(3), Some(1)]),
    field(
        "finishHeaderText",
        "MUI_INSTFILESPAGE_FINISHHEADER_TEXT",
        Holds::Str,
    ),
    field(
        "finishHeaderSubText",
        "MUI_INSTFILESPAGE_FINISHHEADER_SUBTEXT",
        Holds::Str,
    ),
    // `sticky`, and this is MUI2's own hole rather than ours:
    // `InstallFiles.nsh` unsets `FINISHHEADER_*` and `ABORTWARNING_*` and
    // never unsets `ABORTHEADER_*`, which is the pair it actually reads. The
    // same class of typo as `License.nsh`'s `CHECKBOX_TEXT_ACCEPT`, and
    // without the compiler's own `!undef` a second instfiles page is headed
    // with the first one's abort wording.
    sticky(
        "abortHeaderText",
        "MUI_INSTFILESPAGE_ABORTHEADER_TEXT",
        Holds::Str,
    ),
    sticky(
        "abortHeaderSubText",
        "MUI_INSTFILESPAGE_ABORTHEADER_SUBTEXT",
        Holds::Str,
    ),
];

const LICENSE_FIELDS: &[PageField] = &[
    caption([Some(0), None]),
    field("topText", "MUI_LICENSEPAGE_TEXT_TOP", Holds::Str),
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
    caption([Some(1), None]),
    field("topText", "MUI_COMPONENTSPAGE_TEXT_TOP", Holds::Str),
    field(
        "instTypeText",
        "MUI_COMPONENTSPAGE_TEXT_INSTTYPE",
        Holds::Str,
    ),
    field("listText", "MUI_COMPONENTSPAGE_TEXT_COMPLIST", Holds::Str),
    // The description box's caption, and the words in it while the pointer is
    // over nothing. Page-scoped, unlike `smallDescriptions`: MUI2 reads both
    // through `MUI_DEFAULT` inside the page declaration and `MUI_UNSET`s them
    // after, so a second components page may differ.
    field(
        "descriptionTitle",
        "MUI_COMPONENTSPAGE_TEXT_DESCRIPTION_TITLE",
        Holds::Str,
    ),
    field(
        "descriptionText",
        "MUI_COMPONENTSPAGE_TEXT_DESCRIPTION_INFO",
        Holds::Str,
    ),
];

const DIRECTORY_FIELDS: &[PageField] = &[
    caption([Some(2), None]),
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
    // `sticky` because `Directory.nsh` clears neither one: MUI2 reads them from
    // the Show function it writes per page and leaves them standing, so without
    // the `!undef` a second directory page is painted in the first one's
    // colours.
    sticky(
        "colors",
        "MUI_DIRECTORYPAGE_BGCOLOR",
        Holds::Colors("MUI_DIRECTORYPAGE_TEXTCOLOR"),
    ),
];

/// The reboot half of the finish page, which MUI2 draws instead of the normal
/// one when the install set `SetRebootFlag`.
///
/// Reached only through `reboot = { … }`, and that is the point: all four are
/// read inside `!ifndef MUI_FINISHPAGE_NOREBOOTSUPPORT`, so the shape that
/// writes them is the shape that cannot also have turned reboot support off.
const REBOOT_FIELDS: &[PageField] = &[
    field("text", "MUI_FINISHPAGE_TEXT_REBOOT", Holds::Str),
    field("now", "MUI_FINISHPAGE_TEXT_REBOOTNOW", Holds::Str),
    field("later", "MUI_FINISHPAGE_TEXT_REBOOTLATER", Holds::Str),
    field(
        "default",
        "MUI_FINISHPAGE_REBOOTLATER_DEFAULT",
        Holds::Word {
            writes: "later",
            silent: "now",
        },
    ),
];

/// The run checkbox, spelled as a program to start.
const RUN_PATH_FIELDS: &[PageField] = &[
    field("path", "MUI_FINISHPAGE_RUN", Holds::Str),
    field("parameters", "MUI_FINISHPAGE_RUN_PARAMETERS", Holds::Str),
    field("text", "MUI_FINISHPAGE_RUN_TEXT", Holds::Str),
    field("checked", "MUI_FINISHPAGE_RUN_NOTCHECKED", Holds::Not),
];

/// The same checkbox, spelled as a function to call. No `parameters`: MUI2
/// reads them only where it builds the `Exec` line.
const RUN_CALL_FIELDS: &[PageField] = &[
    field(
        "call",
        "MUI_FINISHPAGE_RUN",
        Holds::Calls {
            function: "MUI_FINISHPAGE_RUN_FUNCTION",
            stem: "run",
        },
    ),
    field("text", "MUI_FINISHPAGE_RUN_TEXT", Holds::Str),
    field("checked", "MUI_FINISHPAGE_RUN_NOTCHECKED", Holds::Not),
];

const RUN_FORMS: &[Form] = &[
    Form {
        key: "path",
        needs: &[],
        parts: RUN_PATH_FIELDS,
    },
    Form {
        key: "call",
        needs: &[],
        parts: RUN_CALL_FIELDS,
    },
];

/// The readme checkbox. The same two spellings as `run`, and no `parameters`
/// in either: MUI2 opens this one with `ExecShell open`, which takes none.
const README_PATH_FIELDS: &[PageField] = &[
    field("path", "MUI_FINISHPAGE_SHOWREADME", Holds::Str),
    field("text", "MUI_FINISHPAGE_SHOWREADME_TEXT", Holds::Str),
    field(
        "checked",
        "MUI_FINISHPAGE_SHOWREADME_NOTCHECKED",
        Holds::Not,
    ),
];

const README_CALL_FIELDS: &[PageField] = &[
    field(
        "call",
        "MUI_FINISHPAGE_SHOWREADME",
        Holds::Calls {
            function: "MUI_FINISHPAGE_SHOWREADME_FUNCTION",
            stem: "readme",
        },
    ),
    field("text", "MUI_FINISHPAGE_SHOWREADME_TEXT", Holds::Str),
    field(
        "checked",
        "MUI_FINISHPAGE_SHOWREADME_NOTCHECKED",
        Holds::Not,
    ),
];

const README_FORMS: &[Form] = &[
    Form {
        key: "path",
        needs: &[],
        parts: README_PATH_FIELDS,
    },
    Form {
        key: "call",
        needs: &[],
        parts: README_CALL_FIELDS,
    },
];

/// The link along the bottom of the page: one spelling, and `url` is not
/// optional in it.
const LINK_FIELDS: &[PageField] = &[
    field("text", "MUI_FINISHPAGE_LINK", Holds::Str),
    field("url", "MUI_FINISHPAGE_LINK_LOCATION", Holds::Str),
    field("color", "MUI_FINISHPAGE_LINK_COLOR", Holds::Str),
];

const LINK_FORMS: &[Form] = &[Form {
    key: "text",
    needs: &["url"],
    parts: LINK_FIELDS,
}];

/// The finish page.
///
/// `autoClose` is not here and is a block field: MUI2 reads
/// `MUI_FINISHPAGE_NOAUTOCLOSE` from `MUI_FINISHPAGE_GUIINIT`, behind an
/// `!ifndef` on the half's own `WELCOMEFINISHPAGE_GUINIT`, so it is read on the
/// first welcome-or-finish page of that half and never again — the same rule
/// that put `checkBitmap` on the block.
const FINISH_FIELDS: &[PageField] = &[
    field(
        "title",
        "MUI_FINISHPAGE_TITLE",
        Holds::Roomy("MUI_FINISHPAGE_TITLE_3LINES", Room::Lines),
    ),
    field(
        "text",
        "MUI_FINISHPAGE_TEXT",
        Holds::Roomy("MUI_FINISHPAGE_TEXT_LARGE", Room::Large),
    ),
    field("button", "MUI_FINISHPAGE_BUTTON", Holds::Str),
    field(
        "cancelEnabled",
        "MUI_FINISHPAGE_CANCEL_ENABLED",
        Holds::Flag,
    ),
    field(
        "reboot",
        "MUI_FINISHPAGE_NOREBOOTSUPPORT",
        Holds::Off(REBOOT_FIELDS),
    ),
    field("run", "MUI_FINISHPAGE_RUN", Holds::Widget(RUN_FORMS)),
    field(
        "readme",
        "MUI_FINISHPAGE_SHOWREADME",
        Holds::Widget(README_FORMS),
    ),
    field("link", "MUI_FINISHPAGE_LINK", Holds::Widget(LINK_FORMS)),
    DESTROYED_FIELD,
];

/// Where the page remembers the folder, as one field holding three.
///
/// `StartMenu.nsh` guards every read of the three with
/// `!ifdef …_REGISTRY_ROOT & …_REGISTRY_KEY & …_REGISTRY_VALUENAME`, so any one
/// of them alone is a define nothing reads — and any two are as well. A
/// [`Form`] with the other two under `needs` is exactly that constraint: the
/// shape that writes one is the shape that has written all three.
const REGISTRY_FIELDS: &[PageField] = &[
    field("root", "MUI_STARTMENUPAGE_REGISTRY_ROOT", Holds::Str),
    field("key", "MUI_STARTMENUPAGE_REGISTRY_KEY", Holds::Str),
    field("value", "MUI_STARTMENUPAGE_REGISTRY_VALUENAME", Holds::Str),
];

const REGISTRY_FORMS: &[Form] = &[Form {
    key: "root",
    needs: &["key", "value"],
    parts: REGISTRY_FIELDS,
}];

/// The Start Menu folder page.
///
/// The only page bound to a `local`, and the fields say why: MUI2 reads the
/// folder back through `MUI_STARTMENU_GETFOLDER <id>` and wraps the shortcut
/// writing in `MUI_STARTMENU_WRITE_BEGIN <id>`, so the page has a *name* that
/// install-time code uses. Nothing here spells that name — the id is the local
/// and the variable is minted beside it — which is the whole of what binding it
/// buys.
const STARTMENU_FIELDS: &[PageField] = &[
    field(
        "defaultFolder",
        "MUI_STARTMENUPAGE_DEFAULTFOLDER",
        Holds::Str,
    ),
    field("topText", "MUI_STARTMENUPAGE_TEXT_TOP", Holds::Str),
    field(
        "checkbox",
        "MUI_STARTMENUPAGE_NODISABLE",
        Holds::Checkbox("MUI_STARTMENUPAGE_TEXT_CHECKBOX"),
    ),
    field(
        "registry",
        "MUI_STARTMENUPAGE_REGISTRY_ROOT",
        Holds::Widget(REGISTRY_FORMS),
    ),
    // No `colors`, and the reason is a typo in MUI2 rather than a decision
    // here. `StartMenu.nsh:141` paints `$mui.StartMenuMenu.FolderList`; the
    // variable it declares at line 17 and fills at line 136 is
    // `$mui.StartMenuPage.FolderList`. The line is reached only when
    // `MUI_STARTMENUPAGE_BGCOLOR` is defined, so a page that sets the colours
    // raises `warning 6000: unknown variable/constant` — and this compiler
    // assembles under `-WX`. The two defines are refused in the inventory with
    // that reason, which is the only place a name can be *unusable* rather than
    // unimplemented.
    DESTROYED_FIELD,
];

const CONFIRM_FIELDS: &[PageField] = &[
    caption([None, Some(0)]),
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
/// A declaration written in the table form, taken apart: the array part is the
/// parameters and the hash part the options, so what comes out is one name, the
/// thing the construct encloses, and the switches beside them.
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
/// (`section("Tools", …)`). A user picks a section's name and MUI2 picks these,
/// so one completes and the other cannot.
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
        own: WELCOME_FIELDS,
    },
    page("license", "LICENSE", LICENSE_FIELDS),
    page("components", "COMPONENTS", COMPONENTS_FIELDS),
    page("directory", "DIRECTORY", DIRECTORY_FIELDS),
    page("instFiles", "INSTFILES", INSTFILES_FIELDS),
    // Installer-only, and that is MUI2's fact rather than our policy: there is
    // no `MUI_UNPAGE_STARTMENU`. The uninstaller reaches the same folder
    // through `MUI_STARTMENU_GETFOLDER`, which is what a `menu.folder` read in
    // that half becomes.
    Page {
        installua: "startMenu",
        nsis: "STARTMENU",
        halves: [true, false],
        header: true,
        custom: false,
        own: STARTMENU_FIELDS,
    },
    Page {
        installua: "finish",
        nsis: "FINISH",
        halves: [true, true],
        header: false,
        custom: false,
        own: FINISH_FIELDS,
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
    // complete. What it is not is a `!insertmacro`: `Page custom` names two
    // functions, and both of them are ours to write.
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
    /// which is this order and not the user's, because a Lua table has none.
    fn fields(&self) -> impl Iterator<Item = &'static PageField> {
        let header: &'static [PageField] = if self.header { HEADER_FIELDS } else { &[] };
        self.own.iter().chain(header).chain(COMMON_FIELDS)
    }
}

/// The page surface as names: each page's spelling, the halves it exists in,
/// and every field it accepts.
///
/// Public for the stub's sake, and for the same reason [`mui_defines`] is public
/// for the inventory's: the editor's page classes are hand-written — a block's
/// value is an expression in a table and the parameter model has nothing to say
/// about it — so the only thing keeping them from drifting away from this table
/// is a test that can read both. It drifted once, and the whole of `startMenu`
/// went missing from completion for as long as nobody looked.
pub fn v1_page_surface() -> Vec<(&'static str, [bool; 2], Vec<&'static str>)> {
    V1_PAGES
        .iter()
        .map(|page| {
            (
                page.installua,
                page.halves,
                page.fields().map(|field| field.installua).collect(),
            )
        })
        .collect()
}

/// The `installer {}` and `uninstaller {}` field names, for the same reason.
pub fn v1_installer_fields() -> &'static [&'static str] {
    V1_INSTALLER_FIELDS
}

/// The block-level `MUI_*` defines, listed beside the arms of `block_field`
/// that write them.
///
/// A list *and* the arms, which is one name in two places — deliberately, and
/// only these six. The arms differ in ways a table would have to grow a column
/// for apiece (`icon` writes one of two names depending on the half, three of
/// the others are paths and two are not), and the duplication is caught rather
/// than trusted: `tests/mui.rs` asserts that this list and the MUI inventory's
/// `Exposed` rows are the same set.
const BLOCK_MUI_DEFINES: &[&str] = &[
    "MUI_BGCOLOR",
    "MUI_TEXTCOLOR",
    "MUI_ICON",
    "MUI_UNICON",
    "MUI_COMPONENTSPAGE_CHECKBITMAP",
    "MUI_INSTFILESPAGE_COLORS",
    "MUI_INSTFILESPAGE_PROGRESSBAR",
    "MUI_LICENSEPAGE_BGCOLOR",
    "MUI_ABORTWARNING",
    "MUI_ABORTWARNING_TEXT",
    "MUI_ABORTWARNING_CANCEL_DEFAULT",
    // One name for two spellings, and the only entry here that is: MUI2 builds
    // this one with its uninstaller prefix, so the uninstaller's is
    // `MUI_UNFINISHPAGE_NOAUTOCLOSE` and the snapshot records the pair as the
    // single row the `un` tag marks.
    "MUI_FINISHPAGE_NOAUTOCLOSE",
    "MUI_UNABORTWARNING",
    "MUI_UNABORTWARNING_TEXT",
    "MUI_UNABORTWARNING_CANCEL_DEFAULT",
    // The `languages {}` dialog. Seven settings and no macro: the three macros
    // the block writes are `!insertmacro` lines rather than `!define`s, so they
    // are exposed without being here — same shape as `MUI_LANGUAGE` itself.
    "MUI_LANGDLL_WINDOWTITLE",
    "MUI_LANGDLL_INFO",
    "MUI_LANGDLL_ALLLANGUAGES",
    "MUI_LANGDLL_ALWAYSSHOW",
    "MUI_LANGDLL_REGISTRY_ROOT",
    "MUI_LANGDLL_REGISTRY_KEY",
    "MUI_LANGDLL_REGISTRY_VALUENAME",
    "MUI_HEADERIMAGE",
    "MUI_HEADERIMAGE_BITMAP",
    "MUI_HEADERIMAGE_BITMAP_STRETCH",
    "MUI_HEADERIMAGE_BITMAP_RTL",
    "MUI_HEADERIMAGE_BITMAP_RTL_STRETCH",
    // The uninstaller's four are spelled out rather than tagged `un`, because
    // MUI2 spells them out: `UNBITMAP` is a name of its own in the snapshot,
    // where `MUI_UNWELCOMEFINISHPAGE_BITMAP` below is not.
    "MUI_HEADERIMAGE_UNBITMAP",
    "MUI_HEADERIMAGE_UNBITMAP_STRETCH",
    "MUI_HEADERIMAGE_UNBITMAP_RTL",
    "MUI_HEADERIMAGE_UNBITMAP_RTL_STRETCH",
    "MUI_HEADERIMAGE_RIGHT",
    "MUI_HEADER_TRANSPARENT_TEXT",
    // Two more of the `un`-tagged kind, like `MUI_FINISHPAGE_NOAUTOCLOSE`: MUI2
    // builds these through `${_un}`, so one row apiece covers the uninstaller's
    // `MUI_UNWELCOMEFINISHPAGE_BITMAP` and `…_BITMAP_STRETCH` too.
    "MUI_WELCOMEFINISHPAGE_BITMAP",
    "MUI_WELCOMEFINISHPAGE_BITMAP_STRETCH",
    "MUI_COMPONENTSPAGE_SMALLDESC",
    // The hover hook, which is a define holding a function name and so belongs
    // here rather than beside the description block it is called from.
    "MUI_CUSTOMFUNCTION_ONMOUSEOVERSECTION",
    "MUI_CUSTOMFUNCTION_UNONMOUSEOVERSECTION",
    "MUI_CUSTOMFUNCTION_GUIINIT",
    "MUI_CUSTOMFUNCTION_UNGUIINIT",
    "MUI_CUSTOMFUNCTION_ABORT",
    "MUI_CUSTOMFUNCTION_UNABORT",
];

/// Every `MUI_*` define this compiler writes: the block's, then every page's,
/// then the nested ones a field expands into.
///
/// Public for the inventory's sake (`crate::mui`). A define the emitter writes
/// while the inventory still calls it `todo` is drift in the direction that
/// matters — the burndown claiming work is left when it is done — and the only
/// way to catch it is a list both sides can read.
pub fn mui_defines() -> Vec<&'static str> {
    fn walk(field: &'static PageField, out: &mut Vec<&'static str>) {
        // `subCaption` writes an NSIS line and no define at all, so it has
        // nothing to join against the MUI2 inventory. It is the only field that
        // does, and the empty name is what says so.
        if !field.define.is_empty() {
            out.push(field.define);
        }
        match field.holds {
            Holds::Text(text)
            | Holds::Colors(text)
            | Holds::Checkbox(text)
            | Holds::Roomy(text, _)
            | Holds::Calls { function: text, .. } => out.push(text),
            Holds::Nested(fields) | Holds::Off(fields) => {
                for nested in fields {
                    walk(nested, out);
                }
            }
            // Every spelling, since each writes what the others do not — and
            // the ones they share are deduped below.
            Holds::Widget(forms) => {
                for form in forms {
                    for part in form.parts {
                        walk(part, out);
                    }
                }
            }
            _ => {}
        }
    }

    let mut out: Vec<&'static str> = BLOCK_MUI_DEFINES.to_vec();
    for page in V1_PAGES {
        for field in page.fields() {
            walk(field, &mut out);
        }
    }
    out.sort_unstable();
    out.dedup();
    out
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
];

/// How many rounds the signature fixpoint gets. Three is enough for the deepest
/// chain in the five programs — a caller learns a parameter, the callee learns
/// its return, the caller reads it — and the cap exists because a lattice with
/// `Unknown` at the top is not strictly monotone: a disagreement can flip a
/// slot back. A program that has not settled by then compiles against the last
/// round, which is sound: an unsettled type is `Unknown`, and `Unknown` is
/// refused at every point where guessing would matter.
const MAX_ROUNDS: usize = 8;

pub fn lower(
    resolved: &Resolved<'_>,
    options: &crate::Options,
    diags: &mut Diagnostics,
) -> ir::Module {
    // 1. Types, to a fixpoint. Rounds before the last are lowered against a
    //    scratch collector: their diagnostics are about a type table that was
    //    still incomplete, so reporting them would be reporting the compiler's
    //    intermediate state to the user.
    let mut inferred = Inferred::seed(resolved);
    for _ in 0..MAX_ROUNDS {
        let mut scratch = Diagnostics::new();
        let round = lower_once(resolved, options, &mut scratch, &inferred).1;
        if round == inferred {
            break;
        }
        inferred = round;
    }

    let (mut module, _) = lower_once(resolved, options, diags, &inferred);

    // 2. Registers. Every body is allocated before any call site is filled in,
    //    because a clobber set is a fact about *physical* registers and there
    //    are none until colouring has run.
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

    // 3. The call graph, built once and read three times here.
    let graph = callgraph::build(&module);
    let clobbers = graph.clobbers(&direct);
    graph.lint_recursion(diags);
    reserved(&mut module, &graph);
    addplugindir(&mut module, options);

    // 4. `live ∩ clobbered`, at last.
    for (index, (_, body)) in module.bodies_mut().into_iter().enumerate() {
        alloc::insert_saves(body, &across[index], &clobbers);
    }

    // 5. `InitPluginsDir`, last, because it is the one line here that reads the
    //    finished body rather than building one: it takes no register, saves
    //    nothing across itself and touches no call site, so running it after
    //    allocation keeps it out of every pass that counts steps.
    plugins_dir(&mut module);

    module
}

/// `ReserveFile /plugin X.dll`, for every plugin an init callback can reach.
///
/// **The second alternative of `ReserveFile`, and not a surface row.** A plugin
/// DLL lives in the compressed data block like any other file, and `.onInit`
/// runs before a byte of it has been extracted — so a plugin called from there
/// works or does not work depending on the compressor, which is the include-order
/// hazard wearing a different hat. MUI2 makes it the user's problem and hands
/// them `MUI_RESERVEFILE_LANGDLL` to insert in the right place; the compiler
/// already knows every call site and every edge between bodies, so it does not
/// have to ask.
///
/// **Reachability, not presence.** A reserved file is excluded from solid
/// compression, so reserving a plugin only a section calls would cost size for
/// nothing. The roots are the two init callbacks and the graph answers the rest
/// — which is why [`crate::cfg::Body::plugins`] is per body rather than a
/// module-wide set.
pub fn reserved(module: &mut ir::Module, graph: &callgraph::CallGraph) {
    let mut queue: Vec<usize> = graph
        .names
        .iter()
        .enumerate()
        .filter(|(_, name)| matches!(name.as_str(), ".onInit" | "un.onInit"))
        .map(|(node, _)| node)
        .collect();
    let mut seen: BTreeSet<usize> = queue.iter().copied().collect();
    let mut plugins: BTreeSet<&str> = BTreeSet::new();
    while let Some(node) = queue.pop() {
        plugins.extend(
            module.functions[node]
                .body
                .plugins
                .iter()
                .map(String::as_str),
        );
        for callee in &graph.edges[node] {
            if seen.insert(*callee) {
                queue.push(*callee);
            }
        }
    }

    // The DLL is the namespace and nothing else: `nsExec.execToStack` becomes
    // `nsExec::ExecToStack` out of `nsExec.dll`, which is the same lookup
    // `!addplugindir` does and the reason a plugin's declaration needs no
    // separate file name.
    module.reserved = plugins
        .into_iter()
        .map(|plugin| {
            ir::Instruction::new(
                "ReserveFile",
                vec![
                    ir::Arg::raw("/plugin"),
                    ir::Arg::raw(format!("{plugin}.dll")),
                ],
            )
        })
        .collect();
}

/// `!addplugindir` for every directory a called plugin was declared in.
///
/// **Called, not reachable — a wider set than [`reserved`]'s on purpose.** The
/// two passes answer different questions about the same DLL. A reservation is
/// about the *data block*, so only what an init callback can reach earns one;
/// a search path is about *`makensis` finding the file at all*, and every call
/// site needs that. A plugin only a section calls fails with `Plugin not found,
/// cannot call Foo::Bar` at compile time if this pass copied `reserved`'s
/// reachability walk.
///
/// **Absolutised, because `makensis` resolves a relative plugin directory
/// against its own working directory** — which [`crate::assemble`] sets to the
/// emitted script's parent, not the project root the declaration was written
/// against. With no base to resolve against (an in-memory compile, which has no
/// project root by definition) the path goes out as written: a caller that
/// handed the compiler a string and no directory has already said the file
/// system is not involved.
///
/// **Not an anchored `raw`, and that is the rule rather than the exception.**
/// A user writing this line at the top of their source would land it above
/// `Unicode`, bind it to the default target, and silently break every
/// `unicode = false` build. Text whose meaning depends on compiler-generated
/// state is a declaration the compiler places; only position-independent text
/// gets an anchor.
pub fn addplugindir(module: &mut ir::Module, options: &crate::Options) {
    let mut called: BTreeSet<&str> = BTreeSet::new();
    for (_, body) in module.bodies() {
        called.extend(body.plugins.iter().map(String::as_str));
    }

    // Sorted and deduplicated: two plugins vendored into one directory are one
    // line, and the output is read in diffs.
    let mut dirs: BTreeSet<String> = BTreeSet::new();
    for plugin in called {
        for dir in options.declarations.plugin_dirs(plugin) {
            let path = match &options.base {
                Some(base) => base.join(dir),
                None => std::path::PathBuf::from(dir),
            };
            dirs.insert(path.display().to_string());
        }
    }

    module.plugin_dirs = dirs
        .into_iter()
        .map(|dir| ir::Instruction::new("!addplugindir", vec![ir::Arg::str(dir)]))
        .collect();
}

/// The NSIS spelling of `$PLUGINSDIR`.
const PLUGINS_DIR: &str = "$PLUGINSDIR";

/// The escape hatch's name, in a body and at an anchor alike.
const RAW: &str = "raw";

/// Where a top-level [`RAW`] block goes.
///
/// Two, and growing only on evidence. Each has to name a **documented boundary
/// between numbered slots** rather than a region: "before everything" and "after
/// everything" are boundaries no future slot can move, which is why these two
/// are safe to promise before the rest of the spine is settled. A region like
/// "in the MUI part" would be a promise about an interior the compiler owns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Anchor {
    Head,
    Tail,
}

impl Anchor {
    fn parse(text: &str) -> Option<Anchor> {
        match text {
            "head" => Some(Anchor::Head),
            "tail" => Some(Anchor::Tail),
            _ => None,
        }
    }

    fn text(self) -> &'static str {
        match self {
            Anchor::Head => "head",
            Anchor::Tail => "tail",
        }
    }

    /// What the anchor is for, as a diagnostic note reads it. Written once
    /// because three messages quote it and a fourth would drift.
    fn purpose(self) -> &'static str {
        match self {
            Anchor::Head => {
                "`head` is above every line the compiler writes, for text producing a value the \
                 script then reads — `!system`, `!tempfile`, `!getdllversion`"
            }
            Anchor::Tail => {
                "`tail` is below everything, for registrations `makensis` acts on when the build \
                 ends — `!packhdr`, `!finalize`, `!uninstfinalize`"
            }
        }
    }
}

/// A `raw` block's text, one [`ir::Instruction`] per non-blank line.
///
/// Leading whitespace goes, so the block sits where the emitter's indentation
/// puts everything else. Nothing else is touched: the text is not read, which is
/// the whole of what `raw` promises.
fn raw_lines(text: &str, span: Option<Span>) -> Vec<ir::Instruction> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| ir::Instruction {
            span,
            ..ir::Instruction::new(line, Vec::new())
        })
        .collect()
}

/// `InitPluginsDir`, at the top of every body that names `$PLUGINSDIR`.
///
/// **The line whose absence NSIS never mentions.** `$PLUGINSDIR` is not a
/// constant the way `$WINDIR` is — it is a temporary directory that does not
/// exist until something creates it, and until then the variable expands to
/// nothing at all. So `SetOutPath "$PLUGINSDIR"` without the init is
/// `SetOutPath ""`, which `makensis -WX` assembles without a word and which
/// puts the files somewhere else on a user's machine. That is the include-order
/// hazard again with a different name: a line that costs nothing to
/// omit until the day it costs everything.
///
/// **Per body, at the top, and not hoisted.** A body is the smallest unit that
/// is correct without a reachability argument — a mention in one arm of an `if`
/// is covered by the same line as a mention in the other — and NSIS defines the
/// instruction as a no-op when the directory already exists, so a caller and a
/// callee both having one is not a bug to be optimised away. Hoisting to
/// `.onInit` instead would create the directory on every run of every installer
/// whose one plugin section is never selected, and would mean writing an
/// `.onInit` for programs that have none.
///
/// **Textual, over the lowered lines.** A `raw` block is the user's own text
/// and it is the one place a `$PLUGINSDIR` can arrive without having passed
/// through [`crate::builtins::CONSTANTS`], so the scan reads the instruction
/// names too — an escape hatch that skipped this would be an escape hatch into
/// the exact failure the pass exists to prevent.
pub fn plugins_dir(module: &mut ir::Module) {
    for (_, body) in module.bodies_mut() {
        let named = body
            .blocks
            .iter()
            .flat_map(|block| &block.steps)
            .any(|step| match step {
                ir::Step::Instruction(line) => mentions(line),
                ir::Step::Saves(_) | ir::Step::Call(_) => false,
            })
            || body.calls.iter().any(|site| {
                site.args.iter().any(arg_mentions)
                    || match &site.kind {
                        ir::CallKind::Opaque { lines, .. } => lines.iter().any(mentions),
                        ir::CallKind::Function => false,
                    }
            });
        if named {
            body.blocks[cfg::Body::ENTRY.0].steps.insert(
                0,
                ir::Step::Instruction(ir::Instruction::new("InitPluginsDir", Vec::new())),
            );
        }
    }
}

/// Whether a line names the plugins directory, in its arguments or — for a
/// `raw` block, whose whole line is the name — in the name itself.
fn mentions(line: &ir::Instruction) -> bool {
    line.name.contains(PLUGINS_DIR) || line.args.iter().any(arg_mentions)
}

fn arg_mentions(arg: &ir::Arg) -> bool {
    match arg {
        // [`ir::Piece::Text`] is deliberately not here. A `$` in text is five
        // dollars and the emitter doubles it, so a literal `"$PLUGINSDIR"` in a
        // Lua string ships as `$$PLUGINSDIR` and reads the directory no more
        // than any other sentence does.
        ir::Arg::Data { pieces, .. } => pieces
            .iter()
            .any(|piece| matches!(piece, ir::Piece::Var(var) if var == PLUGINS_DIR)),
        ir::Arg::Raw(text) => text.contains(PLUGINS_DIR),
        ir::Arg::Dest(_) => false,
    }
}

fn lower_once(
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
        on_init: [false, false],
        init_prelude: [Vec::new(), Vec::new()],
        lang_strings: BTreeSet::new(),
        locales: Vec::new(),
        license_tables: 0,
        descriptions: [Vec::new(), Vec::new()],
        hover: [false, false],
        minted: 0,
        un_hooks: Vec::new(),
        requires: Requirements::default(),
    };
    lowerer.program();
    lowerer.finish()
}

/// A global's agreed type, and every site that agreed on it. A global is
/// declared by assigning to it, and the check runs across every assignment
/// rather than pinning to the declaring one: resolution is order-free, so "the
/// declaring assignment" is arbitrary.
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
    /// Whether an `.onInit` was written, so that one is not invented twice. The
    /// uninstaller's is the second slot (`languages {}` needs both).
    on_init: [bool; 2],
    /// Lines the compiler owes the *first* of each half's init callback:
    /// `MUI_LANGDLL_DISPLAY` and `MUI_UNGETLANGUAGE`, which have to run before
    /// anything reads `$LANGUAGE` and so cannot wait for a body to ask for them.
    init_prelude: [Vec<ir::Instruction>; 2],
    /// The `LangString` names `languages {}` declared, so `lang.greeting` is a
    /// resolved read rather than a `$(…)` nobody checked.
    lang_strings: BTreeSet<String>,
    /// The locales `languages {}` declared, in the order it listed them — read
    /// by the license page, which has to check its own per-locale table against
    /// exactly this set. Declaration order rather than a set, because
    /// the first one is the default language and a diagnostic that has to name
    /// *some* locale should name that one.
    locales: Vec<String>,
    /// How many per-locale license tables have been lowered, so the second one
    /// gets a name the first did not.
    license_tables: usize,
    /// Each half's components-page hover texts, as `(index define, text)` in
    /// section order — the block MUI2 spells with three macros.
    descriptions: [Vec<(String, ir::Arg)>; 2],
    /// Whether a half wrote `onMouseOverSection`. MUI2 calls the hook from
    /// inside the block and from nowhere else, so this forces the block even
    /// when no section carries a `description`.
    hover: [bool; 2],
    /// How many description indices have been minted, so the next one is new.
    minted: usize,
    /// Uninstaller hooks MUI2 reaches only through `!ifdef MUI_UNINSTALLER`,
    /// with the entry that wrote each — checked once the block's pages are
    /// known, since resolution is order-free and the page may be written below
    /// the hook.
    un_hooks: Vec<(Span, &'static str)>,
    /// What the program needs included and initialised. Collected during
    /// lowering and emitted at the top, which is the only order that works.
    requires: Requirements,
}

/// The collect-then-emit pass the headers and `StrFunc` adapters ask for.
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
    /// Functions a body asked the compiler to write: the callback behind a
    /// `for … in fileFunc.locate(…)`.
    ///
    /// A `Vec` rather than a set, and a channel out of a body rather than a
    /// field on one, because a [`BodyLowerer`] has no module to push to — it
    /// owns one `Body` and knows nothing about the program around it. The
    /// [`Lowerer`] drains this after each body it builds, which keeps the
    /// ordering deterministic without either half knowing the other's shape.
    pub functions: Vec<ir::Function>,
    /// How many the compiler has written so far, ever.
    ///
    /// Separate from `functions.len()` because that list is *drained* after
    /// every body, so it would restart at zero in the next section — and two
    /// sections that each walk a directory would then both name their callback
    /// `…_locate_0`. NSIS rejects the duplicate rather than picking one, so it
    /// is loud rather than silent, but a name that depends on how much of the
    /// program has been emitted is not a name.
    pub generated: usize,
}

impl Requirements {
    /// Records that a `StrFunc` macro is used, and the header that carries it.
    pub fn str_func(&mut self, name: &str) {
        self.headers.insert("StrFunc".to_string());
        self.str_func.insert(name.to_string());
    }
}

impl<'p> Lowerer<'_, 'p> {
    fn program(&mut self) {
        // A top-level `<const>` is a `!define`: build-time, folded in every
        // expression, and `${NAME}` in the output. Emitted in source order
        // because the preprocessor is textual and strictly sequential — the one
        // part of an NSIS script where order is semantics.
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
        // other import and against the ones an adapter pulls in on its own. A
        // `plugin` needs no line at all: NSIS finds it by name.
        for namespace in self.resolved.namespaces.values() {
            if let crate::resolve::Namespace::Header(header) = namespace {
                self.requires.headers.insert(header.clone());
            }
        }

        // A bare assignment at the top level declares a global *and* gives it a
        // value, and a `Var` has no initialiser — so the assignments become the
        // first lines of `.onInit`, which is the one body NSIS guarantees runs
        // before anything else. Collected here and lowered when the callback
        // is, since the block they belong to may be written above them and
        // resolution being order-free makes that legal.
        self.global_inits = self
            .resolved
            .block
            .iter()
            .filter(|stmt| matches!(stmt, Stmt::Assign { .. }))
            .map(|stmt| (*stmt).clone())
            .collect();

        // Which block listed which declaration, decided before anything is
        // lowered. A body can address a section the block lists *after* it —
        // `installer { onInit(…), core }` is ordinary — and the claim is what
        // says which half a handle names, so the map has to be complete before
        // the first body is walked. `languages {}` too, before anything that
        // could read `lang.greeting` or ask for an `.onInit` — same
        // order-freeness argument as the claims.
        self.languages_pass();
        self.claim_pass();

        for stmt in self.resolved.block.clone() {
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
            let (lists, write, order) = match deferred.kind {
                DeferredKind::Control(_) => (
                    "page",
                    format!("write `{local},` among the `controls` of a `page.custom {{}}`"),
                    "the list's order is the tab order; the declaration's is nothing",
                ),
                DeferredKind::StartMenu => (
                    "block",
                    format!("write `{local},` among the entries of `installer {{}}`"),
                    "the block's order is the page order; the declaration's is nothing",
                ),
                _ => (
                    "block",
                    format!(
                        "write `{local},` among the entries of `installer {{}}` or \
                         `uninstaller {{}}`"
                    ),
                    "the block's order is the install order; the declaration's is nothing",
                ),
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

        // Nothing declared an `.onInit`, and there is something for one to do:
        // globals to initialise, or a language to pick before anything reads
        // `$LANGUAGE`. The callback exists to hold them, and inventing it is
        // the same ruling as inventing the `.` on its name.
        for half in [Half::Installer, Half::Uninstaller] {
            let prelude = std::mem::take(&mut self.init_prelude[half.index()]);
            let inits = match half {
                Half::Installer => std::mem::take(&mut self.global_inits),
                Half::Uninstaller => Vec::new(),
            };
            if self.on_init[half.index()] || (prelude.is_empty() && inits.is_empty()) {
                continue;
            }
            // The uninstaller's is worth nothing when there is no uninstaller:
            // `un.onInit` in a script with no `uninstaller {}` is a function
            // NSIS never calls. Keyed off the block rather than off its pages,
            // since a silent uninstaller has none and still runs.
            if half == Half::Uninstaller && self.uninstaller_span.is_none() {
                continue;
            }
            let span = inits.first().map(Stmt::span).unwrap_or_default();
            let (body, _) = self.body_with(span, Some(half), table::Place::OnInit, |lowerer| {
                for instruction in prelude {
                    lowerer.emit(instruction);
                }
                lowerer.block(&inits);
            });
            self.module.functions.push(ir::Function {
                name: format!(
                    "{}onInit",
                    if half == Half::Installer { "." } else { "un." }
                ),
                body,
            });
        }
        // Globals in first-seen order, emitted before the first body that
        // touches them — which the field order in `ir::Module` already
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
            let Some(claim) = self.claims.get(local) else {
                continue;
            };
            match deferred.kind {
                DeferredKind::Control(_) => {
                    self.module.vars.push(control_var(local, claim.half));
                }
                // And one per listed start menu page, for the same reason one
                // level up: `MUI_PAGE_STARTMENU` stores the folder the user
                // picked into it, and the sections that write the shortcuts run
                // long after the page is gone.
                DeferredKind::StartMenu => self.module.vars.push(start_menu_var(local)),
                _ => {}
            }
        }
    }

    fn finish(mut self) -> (ir::Module, Inferred) {
        // `!include`s, deduplicated and ordered: `MUI2.nsh` first because it is
        // the one header whose macros the others must not shadow, then the rest
        // alphabetically. Alphabetical rather than first-imported so that
        // moving an `import` line does not rewrite a golden — headers are
        // independent, unlike `!define`s, so there is nothing to preserve.
        if self.mui {
            self.module.includes.push("MUI2.nsh".to_string());
            // A program with no `languages {}` still needs one language line —
            // MUI2 `!warning`s without one — so English stands in. Written out
            // it would be `languages { locales = { English = {} } }`, which is
            // why this is a default and not a policy.
            if self.module.languages.is_empty() {
                self.module.languages.push(ir::Instruction::new(
                    "!insertmacro",
                    vec![ir::Arg::raw("MUI_LANGUAGE"), ir::Arg::str("English")],
                ));
            }
        }
        self.module.includes.extend(
            self.requires
                .headers
                .iter()
                .map(|header| format!("{header}.nsh")),
        );
        // `${Using:StrFunc} StrCase` — one line per function actually reached,
        // after the `!include` and before anything that calls it.
        self.module.inits.extend(
            self.requires
                .str_func
                .iter()
                .map(|name| ir::Instruction::new("${Using:StrFunc}", vec![ir::Arg::raw(name)])),
        );

        // An uninstaller hook with no uninstaller page. `MUI_INSERT` writes
        // `un.onGUIInit` and `un.onUserAbort` behind `!ifdef MUI_UNINSTALLER`,
        // and `MUI_UNPAGE_INIT` is the only thing that sets it — so without a
        // page the define is written, the function is written, and nothing ever
        // calls either. Checked here rather than where the hook is written,
        // because resolution is order-free and the page may be written below
        // it.
        if self.module.unpages.is_empty() {
            for (span, word) in std::mem::take(&mut self.un_hooks) {
                self.diags.push(
                    Diagnostic::error(
                        Code::MissingAttribute,
                        span,
                        format!("`{word}` in `uninstaller {{}}` needs an uninstaller page"),
                    )
                    .note(
                        "MUI2 writes the `un.` half of the callback that calls it only for a \
                         script that has one, so this function would never run",
                    ),
                );
            }
        }

        // One description block per half that has anything to say. A half with
        // texts needs it to show them; a half with only the hook needs it
        // because MUI2 calls the hook from inside it.
        for half in [Half::Installer, Half::Uninstaller] {
            let texts = std::mem::take(&mut self.descriptions[half.index()]);
            if texts.is_empty() && !self.hover[half.index()] {
                continue;
            }
            self.module.descriptions.push(ir::Descriptions {
                un: half == Half::Uninstaller,
                texts,
            });
        }

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
        // `raw.head [[ … ]]` — the one declaration written with a dotted
        // callee, and the dot is the feature: it is what makes the anchor
        // impossible to leave off. See [`Self::anchored`].
        if let Some((base, anchor)) = call.callee_field()
            && base == RAW
        {
            self.anchored(anchor, call);
            return;
        }

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
            // Bare `raw` at the top level: the name is right and the position
            // is missing. Reported here rather than as "not a declaration",
            // because the fix is two characters and the generic message names
            // every declaration except the one that was written.
            RAW => {
                self.diags.push(
                    Diagnostic::error(
                        Code::RawAnchor,
                        span,
                        format!("a top-level `{RAW}` needs an anchor"),
                    )
                    .note(format!(
                        "write `{RAW}.{}` or `{RAW}.{}`; there is no \"here\" at the top level, \
                         since the emitter's slots are fixed and statement order is not emission \
                         order",
                        Anchor::Head.text(),
                        Anchor::Tail.text()
                    ))
                    .note(Anchor::Head.purpose())
                    .note(Anchor::Tail.purpose()),
                );
            }
            // Lowered by [`Self::languages_pass`] before this loop began, so
            // that a body written above it can still read `lang.greeting`.
            "languages" => {}
            // `import` and `plugin` are the last two, and both are exposed as
            // *expressions* — `local mui = import "MUI2"`. What is missing is
            // this position, not the name, and the message says which rather
            // than claiming a name the compiler answers to is unknown.
            other if V1_BLOCKS.contains(&other) => {
                self.todo(span, &format!("`{other}` as a statement"));
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

    /// `raw.head [[ … ]]` and `raw.tail [[ … ]]` — the escape hatch at the top
    /// level, where the hatch needs a position and there is none to infer.
    ///
    /// The anchor is part of the callee rather than a field in a table, which is
    /// what makes it un-omittable: there is no form of this declaration that
    /// carries no anchor, so "the anchor is required" is a fact about the
    /// grammar instead of a rule with a check behind it. It also keeps the two
    /// `raw`s visibly different — `raw [[ … ]]` in a body means *here*, and
    /// "here" is exactly what the top level does not have.
    ///
    /// **What may go at an anchor** is a rule about content, and it is the rule
    /// that keeps this from selling the spine's guarantee back to the user:
    /// *text whose meaning is position-independent*. Text whose meaning depends
    /// on compiler-generated state is a declaration the compiler places —
    /// `!addplugindir` written at `head` would land above `Unicode`, bind to the
    /// default target and silently break every `unicode = false` build, so it is
    /// slot 1b and not an anchor. When a directive turns out to be
    /// position-sensitive, the answer is a slot.
    fn anchored(&mut self, anchor: &Name, call: &Expr) {
        let span = call.span();
        let Some(anchor) = Anchor::parse(&anchor.text) else {
            self.diags.push(
                Diagnostic::error(
                    Code::RawAnchor,
                    anchor.span,
                    format!("`{}` is not an anchor", anchor.text),
                )
                .note(format!(
                    "the anchors are `{}` and `{}`",
                    Anchor::Head.text(),
                    Anchor::Tail.text()
                ))
                .note(Anchor::Head.purpose())
                .note(Anchor::Tail.purpose())
                .note(
                    "there are two on purpose: an anchor names a documented boundary between \
                     numbered slots, never a region, and a third one arrives when a real script \
                     needs it",
                ),
            );
            return;
        };

        let Expr::Call { args, .. } = call else {
            return;
        };
        let [Expr::Str(text)] = args.as_slice() else {
            self.diags.push(
                Diagnostic::error(
                    Code::WrongArity,
                    span,
                    format!("`{RAW}.{}` takes one literal block", anchor.text()),
                )
                .note(format!(
                    "write `{RAW}.{} [[ … ]]`; a computed string would be text this compiler \
                     assembled and did not read",
                    anchor.text()
                )),
            );
            return;
        };

        // The invariant [`plugins_dir`] exists to hold, at the one position
        // that pass cannot reach. `$PLUGINSDIR` is a run-time directory a body
        // gets by having `InitPluginsDir` inserted above the statement that
        // names it; an anchor is outside every body, so there is no statement,
        // nothing runs, and the name is text. Diagnosed rather than scanned,
        // because there is nowhere here for the fix to be inserted.
        if text.value.contains(PLUGINS_DIR) {
            self.diags.push(
                Diagnostic::error(
                    Code::RawAnchor,
                    span,
                    format!(
                        "`{PLUGINS_DIR}` cannot mean anything at `{RAW}.{}`",
                        anchor.text()
                    ),
                )
                .note(
                    "the directory is created by an `InitPluginsDir` the compiler puts above the \
                     statement that names it, and an anchor is outside every body — so nothing \
                     creates it and nothing runs",
                )
                .note(format!(
                    "a `{RAW} [[ … ]]` in a section or a `func` gets the `InitPluginsDir`"
                )),
            );
            return;
        }

        let lines = raw_lines(&text.value, Some(span));
        match anchor {
            Anchor::Head => self.module.head.extend(lines),
            Anchor::Tail => self.module.tail.extend(lines),
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
            self.diags.push(
                Diagnostic::error(
                    Code::DuplicateBlock,
                    span,
                    format!("`{name} {{}}` appears more than once"),
                )
                .note_at("the first one is at", previous)
                .note("it is script-global, so there is exactly one"),
            );
            return true;
        }
        *slot = Some(span);
        false
    }

    // -- attributes -------------------------------------------------------

    fn attributes(&mut self, fields: &[TableField], span: Span) {
        // A Lua table has no order, so the order is the compiler's — see
        // [`ORDERED`] for which fields need one and how that was measured.
        let mut fields: Vec<&TableField> = fields.iter().collect();
        fields.sort_by_key(|field| attribute_rank(field));

        // The cross-field constraints, before anything is emitted — see
        // [`Self::ignored_settings`]. Ordering the block was never enough for
        // these two: `SetCompressorDictSize` beside `SetCompressor zlib` is
        // ignored in whichever order the two lines are written.
        self.ignored_settings(&fields);
        self.solid_ignores_compress(&fields);

        for field in fields {
            let TableField::Named { name, value } = field else {
                self.todo(span, "a positional entry in `attributes {}`");
                continue;
            };

            match name.text.as_str() {
                // The nested ones. `versionInfo` is hand-shaped — its members
                // are ordered against each other and `keys` is a free map — and
                // `unicode` emits nothing: it sets a field the emitter reads
                // first, so a later `raw` can override it.
                "versionInfo" => self.version_info(value),
                "unicode" => match self.constant(value) {
                    Some(ConstValue::Bool(value)) => self.module.unicode = value,
                    _ => self.bad_value(
                        value.span(),
                        "unicode",
                        "a `bool`",
                        "write `unicode = true`; NSIS's charset otherwise depends on how the \
                         local `makensis` was built, which is why it is always emitted",
                    ),
                },
                // Every other group is its rows and nothing else, so one
                // function reads all of them — see [`Self::group`].
                group if attribute_groups().contains(&group) => self.attribute_group(group, value),
                other => match table::by_installua(other) {
                    Some(entry) => self.setting(entry, &name.text, value),
                    // A name that belongs to the other block is a five-second
                    // fix rather than a five-second wait, so it says which
                    // block rather than which version. Reached only by `pages`
                    // and `text`: the other four installer fields are
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
                                 which is the same field for the other half",
                            ),
                        );
                    }
                    None => {
                        let diagnostic = Diagnostic::error(
                            Code::UnknownField,
                            name.span,
                            format!("`{other}` is not an attribute"),
                        );
                        // A flattened member reads as a plausible attribute and
                        // is nothing but the group written wrong, so it is
                        // answered with the group rather than with the list of
                        // everything: `manifestGdiScaling` is `manifest = {
                        // gdiScaling = … }`.
                        self.diags.push(match flattened(other) {
                            Some((group, member)) => diagnostic.note(format!(
                                "write `{group} = {{ {member} = … }}` — the settings NSIS \
                                 prefixes are a table here, once, rather than a prefix on \
                                 every field"
                            )),
                            None => diagnostic
                                .note(format!("the attributes are {}", list(&attribute_names()))),
                        });
                    }
                },
            }
        }
    }

    /// The settings NSIS would read and then ignore, refused before they are
    /// emitted.
    ///
    /// A [`table::Setting::Only`] is read only when a sibling field holds one
    /// of a few values. `makensis` says so itself — *warning 8026:
    /// SetCompressorDictSize: compressor is not set to LZMA. Effectively
    /// ignored.* — which means the failure is already caught by tier 3, but
    /// only for a program somebody wrote a fixture for, and with the NSIS
    /// command's name on it rather than the field's, and the rule the five
    /// programs set is that a user does not meet this at all.
    ///
    /// Here rather than in [`Self::setting`] because the constraint is about the
    /// **block**: it is the one per-field fact that cannot be decided from the
    /// field. The page world already has this shape one level down — a `Form`'s
    /// `needs`, which is what makes the start menu page's registry triple
    /// unspellable as a pair.
    fn ignored_settings(&mut self, fields: &[&TableField]) {
        for field in fields {
            let TableField::Named { name, value } = field else {
                continue;
            };
            let Some(entry) = table::by_installua(&name.text) else {
                continue;
            };
            let table::Class::Attribute(table::Setting::Only {
                sibling,
                holds,
                default,
                ..
            }) = entry.class
            else {
                continue;
            };

            let written = fields
                .iter()
                .find_map(|field| match field {
                    TableField::Named { name, value } if name.text == sibling => Some(value),
                    _ => None,
                })
                // The sibling may be a [`table::Setting::Flags`] table, and the
                // value this constraint is about is the positional entry inside
                // it. Reading the table itself would find no constant, skip the
                // check, and let `compressorDictSize` through beside
                // `compressor = { "zlib", solid = true }` — which is warning
                // 8026 and, under `-WX`, the failure a user is never supposed to
                // meet.
                .map(|value| flagged_value(value).unwrap_or(value));
            // The sibling's value, or NSIS's default for it. An absent
            // `compressor` is `zlib` and not "no compressor", which is why the
            // field that wants LZMA is as wrong on its own as it is beside
            // `bzip2` — and is the case a check that read only what was written
            // would let through.
            let effective = match written {
                // Not a constant, or not a string: the sibling's own row says so
                // with `bad-field-value`, and a second diagnostic about a value
                // nobody could read would be noise on top of it.
                Some(expr) => match self.constant(expr) {
                    Some(value) => value.text(),
                    None => continue,
                },
                None => default.to_string(),
            };
            if holds.contains(&effective.as_str()) {
                continue;
            }

            let diagnostic = Diagnostic::error(
                Code::IgnoredSetting,
                value.span(),
                format!(
                    "`{}` is read only when `{sibling}` is {}{}",
                    name.text,
                    if holds.len() > 1 { "one of " } else { "" },
                    list(holds)
                ),
            );
            let diagnostic = match written {
                Some(_) => diagnostic.note(format!("`{sibling}` is `{effective}` here")),
                None => diagnostic.note(format!(
                    "no `{sibling}` is set, so it is `{effective}` — NSIS's own default"
                )),
            };
            self.diags.push(diagnostic.note(format!(
                "NSIS accepts `{}` here and ignores it, which is a warning and so an error \
                 under `-WX`",
                entry.nsis
            )));
        }
    }

    /// One `attributes {}` field, lowered from its row.
    ///
    /// The whole of the per-field knowledge is [`table::Setting`], so this is
    /// the function that has to grow when a *shape* is new and not when a
    /// setting is.
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

        // The cross-field constraint has already been checked, in
        // [`Self::ignored_settings`] where the siblings are; what is left of the
        // shape is the one it wraps, which is what the value has to be.
        let holds = match holds {
            table::Setting::Only { of, .. } => *of,
            other => other,
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
            table::Setting::Off { word, parts, least } => {
                self.off_setting(entry, field, value, word, parts, least);
            }
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
            table::Setting::Flags(of) => self.flagged_setting(entry, field, value, *of),
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

    /// `compress = "off"` beside a solid compressor, refused before `makensis`
    /// turns it into a build failure.
    ///
    /// `script.cpp`'s `TOK_SETCOMPRESS` is `if (build_compress==0 &&
    /// build_compress_whole)` ⇒ *warning 8021: 'SetCompress off' encountered,
    /// and in whole compression mode. Effectively ignored.* — which under this
    /// compiler's own `-WX` is an error naming `SetCompress` rather than the
    /// field. The same argument [`table::Setting::Only`] was written for, and
    /// the pair is exact: only `off` collides, because `auto` and `force` are
    /// enum values 1 and 2.
    ///
    /// Hand-shaped rather than a `Setting` because what decides it is a
    /// *sibling's flag* and not a sibling's value, and `Only` reads values.
    /// One constraint of that shape is a function; a second would be a variant.
    ///
    /// It became reachable with [`table::Setting::Flags`] and not before —
    /// while `/SOLID` had no spelling, no program could get here — which is why
    /// the check arrives in the same change as the flag.
    fn solid_ignores_compress(&mut self, fields: &[&TableField]) {
        let named = |want: &str| {
            let field = table::by_nsis(want).and_then(|entry| entry.installua)?;
            fields.iter().find_map(|each| match each {
                TableField::Named { name, value } if name.text == field => Some((field, value)),
                _ => None,
            })
        };

        let (Some((compress, off)), Some((compressor, whole))) =
            (named("SetCompress"), named("SetCompressor"))
        else {
            return;
        };
        if !self
            .constant(off)
            .is_some_and(|value| value.text() == "off")
        {
            return;
        }
        let solid = matches!(whole, Expr::Table { fields, .. } if fields.iter().any(|each| {
            matches!(each, TableField::Named { name, value }
                if name.text == "solid"
                    && matches!(self.constant(value), Some(ConstValue::Bool(true))))
        }));
        if !solid {
            return;
        }

        self.diags.push(
            Diagnostic::error(
                Code::IgnoredSetting,
                off.span(),
                format!("`{compress}` is not read when `{compressor}` is solid"),
            )
            .note(
                "solid compression is one stream over the whole installer, so there is no \
                 per-file compression left to turn off — NSIS says so as warning 8021, which \
                 is an error under `-WX`",
            ),
        );
    }

    /// `compressor = "lzma"` or `compressor = { "lzma", solid = true }`: the
    /// value, or the value with this row's `/FLAG`s in front of it.
    ///
    /// The two branches are one line of NSIS with a different number of leading
    /// raw tokens, so the *value* goes through [`Self::value_arg`] either way —
    /// the same function a part of a [`table::Setting::Table`] goes through, and
    /// the reason a flagged field cannot come to disagree with an unflagged one
    /// about what its value is.
    ///
    /// The one thing the table branch does not ask is whether the position
    /// repeats, which [`Self::setting_line`] does. No flagged row has a
    /// repeating position — `Rep::Many` and a leading `/FLAG` would be a line
    /// whose flags and values are told apart only by the `/`, which is a shape
    /// nobody has needed.
    fn flagged_setting(
        &mut self,
        entry: &'static table::Instruction,
        field: &str,
        value: &Expr,
        of: table::Setting,
    ) {
        let Expr::Table { fields, .. } = value else {
            // Not a table, so no flags: the shape behind the wrapper, through
            // the path it took before this row had flags at all. Every existing
            // program is in this branch.
            self.setting_line(entry, field, of, value);
            return;
        };

        let mut flags: Vec<ir::Arg> = Vec::new();
        let mut written: Option<&Expr> = None;
        // The row's own order, which is the snapshot's — a Lua table has none,
        // and NSIS echoes `/FINAL` before `/SOLID` in its own trace.
        let mut on: Vec<&'static str> = Vec::new();

        for given in fields {
            match given {
                TableField::Positional { value: inner } => {
                    if written.is_some() {
                        self.bad_value(
                            inner.span(),
                            field,
                            "one value",
                            &format!(
                                "`{}` takes one, and the other entries are its flags",
                                entry.nsis
                            ),
                        );
                        return;
                    }
                    written = Some(inner);
                }
                TableField::Named { name, value: held } => {
                    let Some(flag) = entry.flag(&name.text) else {
                        self.diags.push(
                            Diagnostic::error(
                                Code::UnknownField,
                                name.span,
                                format!("`{}` is not a flag of `{field}`", name.text),
                            )
                            .note(format!(
                                "the flags are {}",
                                list(&entry.flags().map(|(name, _)| name).collect::<Vec<_>>())
                            )),
                        );
                        return;
                    };
                    match self.constant(held) {
                        // `false` is the flag left out, and not a third state:
                        // NSIS starts each of these cleared rather than
                        // remembered, so writing the word and writing nothing
                        // are the same line.
                        Some(ConstValue::Bool(true)) => on.push(flag.opt.nsis),
                        Some(ConstValue::Bool(false)) => {}
                        _ => {
                            self.bad_value(
                                held.span(),
                                &name.text,
                                "a `bool`",
                                &format!(
                                    "it writes `{}` on the `{}` line, or leaves it off",
                                    flag.opt.nsis, entry.nsis
                                ),
                            );
                            return;
                        }
                    }
                }
            }
        }

        for (_, flag) in entry.flags() {
            if on.contains(&flag.opt.nsis) {
                flags.push(ir::Arg::raw(flag.opt.nsis));
            }
        }

        let Some(inner) = written else {
            self.bad_value(
                value.span(),
                field,
                "a value beside its flags",
                &format!(
                    "write `{field} = {{ …, {} }}` — the flags say how, not what",
                    entry
                        .flags()
                        .map(|(name, _)| format!("{name} = true"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            );
            return;
        };

        if let Some(arg) = self.value_arg(of, entry.params.first(), field, entry.nsis, inner) {
            flags.push(arg);
            self.module
                .attributes
                .push(ir::Instruction::new(entry.nsis, flags));
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
    /// enum's members, which are the snapshot's and never a list here. `line`
    /// is the NSIS command, which the notes name because that is what the field
    /// becomes.
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
            // The keyword branch is tried first and only for a `string`, so the
            // shape behind the words keeps every value it would have taken
            // alone: `reslang = 1033` never meets the word list at all.
            //
            // A `string` that is not one of the words is *this* shape's error
            // and not the inner one's. Delegating would report `an int`, naming
            // one branch of an alternation the user wrote the other branch of —
            // the error that sends someone looking for the wrong mistake.
            table::Setting::Or { words, of } => match self.constant(value) {
                Some(ConstValue::Str(text)) => {
                    match words.iter().find(|word| word.eq_ignore_ascii_case(&text)) {
                        // Emitted in the row's spelling: NSIS compares these with
                        // `_tcsicmp`, so `all` and `ALL` are one value to it and
                        // letting both through would make two scripts that differ
                        // only in case emit two different lines.
                        Some(word) => Some(ir::Arg::raw(*word)),
                        None => {
                            self.bad_value(
                                value.span(),
                                field,
                                &format!("{} or {}", list(words), noun(*of)),
                                &format!("it becomes `{line} <keyword>` or `{line} <value>`"),
                            );
                            None
                        }
                    }
                }
                _ => self.value_arg(*of, param, field, line, value),
            },
            // An `Only` never reaches here: [`Self::setting`] unwraps it to the
            // shape it wraps before anything reads the value, because the
            // constraint is about which fields are written together and not
            // about what any one of them holds.
            table::Setting::Only { of, .. } => self.value_arg(*of, param, field, line, value),
            // All five are shapes rather than values: a table, an `Off` and an
            // `Each` are more than one of these, `Handled` is not lowered here
            // at all, and a `Flags` is a whole line's worth — a *position* has
            // nowhere to put a `/FLAG`, so unwrapping it here would emit the
            // value and drop the flags, which is the silence this row exists to
            // end.
            table::Setting::Table(_)
            | table::Setting::Off { .. }
            | table::Setting::Each(_)
            | table::Setting::Flags(_)
            | table::Setting::Handled(_) => {
                self.todo(value.span(), &format!("`{field}` in this position"));
                None
            }
        }
    }

    /// The keyword an enum-valued field was written with.
    ///
    /// Almost always a string — `compressor = "lzma"` — but a registry root is
    /// a **bare** name, because `readRegStr(HKLM, …)` already spells it that
    /// way and one idea with two spellings is worse than either of them. The
    /// names this accepts are exactly the sigil-less constants, so no other
    /// field changes: there is no constant called `lzma` for `compressor =
    /// lzma` to find, and an unknown bare name still fails as a value.
    fn keyword(&mut self, value: &Expr, field: &str) -> Option<String> {
        if let Expr::Name(name) = value
            && let Some(constant) = crate::builtins::constant_named(&name.text)
            && !constant.sigil
        {
            return Some(constant.nsis.to_string());
        }
        self.constant_string(value, field)
    }

    /// `portableExecutable.addResource = { { file = …, … }, { … } }`: one whole NSIS line per
    /// element, in the order they were written.
    ///
    /// The elements are **positional** where the parts of a
    /// [`table::Setting::Table`] are named, and for the same reason in reverse:
    /// three strings on one line can only be told apart by a key, and two
    /// resources can only be told apart by their order. A Lua table keeps that
    /// order and no other, which is the order NSIS adds them in.
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

    /// `manifest.supportedOS = { "Win7", "Win10" }`: one line, as many values as
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

    /// `installDirRegKey = { root = HKLM, key = "Software/App", name = "Path"
    /// }`: one NSIS line built out of a Lua table, one key per position,
    /// emitted in the *table's* order rather than the source's — a Lua table
    /// has no order to preserve and NSIS counts arguments.
    ///
    /// A part may be left out when its position is optional *and* nothing after
    /// it was written — `addResource`'s `reslang` is the one that is. Which
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

        if let Some(args) = self.part_args(entry, field, value, fields, parts, None) {
            self.module
                .attributes
                .push(ir::Instruction::new(entry.nsis, args));
        }
    }

    /// `bgGradient = false` or `bgGradient = { top = …, … }`: the two branches
    /// of an alternation.
    ///
    /// `false` and not `"off"`, because the word is NSIS's spelling of a state
    /// Lua already has one of — and not `nil` either, since leaving the field
    /// out has to keep meaning "write no line at all". The table branch is a
    /// [`table::Setting::Table`] in every respect but where its optionality
    /// comes from, which is why both go through the same loop.
    fn off_setting(
        &mut self,
        entry: &'static table::Instruction,
        field: &str,
        value: &Expr,
        word: &'static str,
        parts: &'static [table::Part],
        least: usize,
    ) {
        let how = format!(
            "`false` writes `{} {word}`, and `{field} = {{ {} }}` writes the other branch",
            entry.nsis,
            shape(parts)
        );

        match value {
            Expr::Table { fields, .. } => {
                if let Some(args) = self.part_args(entry, field, value, fields, parts, Some(least))
                {
                    self.module
                        .attributes
                        .push(ir::Instruction::new(entry.nsis, args));
                }
            }
            // `true` is the one wrong value worth its own reading: it says "yes,
            // do this", and the thing it would be turning on has no default
            // colours for this row to guess at.
            _ => match self.constant(value) {
                Some(ConstValue::Bool(false)) => {
                    self.module
                        .attributes
                        .push(ir::Instruction::new(entry.nsis, vec![ir::Arg::raw(word)]));
                }
                _ => self.bad_value(value.span(), field, "`false` or a table", &how),
            },
        }
    }

    /// The parts of one table, in the order NSIS reads them.
    ///
    /// `least` is written only by [`table::Setting::Off`], whose alternation the
    /// snapshot flattened; `None` asks the snapshot, which is what every other
    /// row does and the only answer that cannot be wrong.
    fn part_args(
        &mut self,
        entry: &'static table::Instruction,
        field: &str,
        value: &Expr,
        fields: &[TableField],
        parts: &'static [table::Part],
        least: Option<usize>,
    ) -> Option<Vec<ir::Arg>> {
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
            let optional = match least {
                Some(least) => index >= least,
                None => entry
                    .params
                    .get(index)
                    .is_some_and(|param| !param.required()),
            };
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
                let spare = match least {
                    Some(least) => parts[least.min(parts.len())..]
                        .iter()
                        .map(|part| part.field)
                        .collect(),
                    None => optional_parts(entry, parts),
                };
                if !spare.is_empty() {
                    diag = diag.note(format!("only {} may be left out", list(&spare)));
                }
                self.diags.push(diag);
                return None;
            };
            // An alternation has no position to stand against — the snapshot
            // kept one for the branch that is a bare word — so its parts are
            // read as what the row says they hold and nothing more.
            let param = match least {
                Some(_) => None,
                None => entry.params.get(index),
            };
            let arg = self.value_arg(part.holds, param, part.field, entry.nsis, given)?;
            args.push(arg);
        }

        Some(args)
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
    /// lmza` is not an error — so the closed set is checked here or not at all.
    fn enumerated(&mut self, field: &str, text: &str, allowed: &[&str], span: Span) -> bool {
        if !allowed.contains(&text) {
            self.diags.push(
                Diagnostic::error(
                    Code::BadFieldValue,
                    span,
                    format!("`{text}` is not a `{field}`"),
                )
                .note(format!("the values are {}", list(allowed)))
                .note("NSIS ignores a keyword it does not know here rather than objecting"),
            );
            return false;
        }
        true
    }

    // There is no `flag_attribute` helper any more, nor an `int_` or `enum_`
    // one: the shapes are [`Self::setting`]'s arms, and a helper per shape would
    // be a second place to look for the same four lines.

    /// One nested group — `manifest = { … }`, `portableExecutable = { … }` —
    /// which is its rows read through their own prefix.
    ///
    /// Nothing here knows what a manifest is. A group's members are ordinary
    /// `Attribute` rows that happen to share a prefix, so each one goes through
    /// [`Self::setting`], the same function the flat fields go through: the
    /// grouping is a *spelling*, and a spelling must not become a second place
    /// where a shape is lowered. `versionInfo` is the exception above precisely
    /// because it is not one — its members are ordered against each other.
    ///
    /// The field is named to the user by its full path. `supportedOS` alone
    /// would be ambiguous the moment a second group grows one, and the path is
    /// what they wrote.
    fn attribute_group(&mut self, group: &str, value: &Expr) {
        let Expr::Table { fields, .. } = value else {
            let fields = group_fields(group);
            self.bad_value(
                value.span(),
                group,
                "a table",
                &format!(
                    "write `{group} = {{ {} = … }}`; the fields are {}",
                    fields.first().copied().unwrap_or("…"),
                    list(&fields)
                ),
            );
            return;
        };

        for field in fields {
            let TableField::Named { name, value } = field else {
                self.todo(value.span(), &format!("a positional entry in `{group}`"));
                continue;
            };
            let path = format!("{group}.{}", name.text);
            match table::by_installua(&path) {
                Some(entry) => self.setting(entry, &path, value),
                None => self.diags.push(
                    Diagnostic::error(
                        Code::UnknownField,
                        name.span,
                        format!("`{}` is not a `{group}` field", name.text),
                    )
                    .note(format!("the fields are {}", list(&group_fields(group)))),
                ),
            }
        }
    }

    /// `versionInfo = { product = "1.4.2.0", keys = { … } }`.
    ///
    /// The keys are emitted in **sorted** order rather than source order. A Lua
    /// table has no order to preserve — `{ a = 1, b = 2 }` and `{ b = 2, a = 1
    /// }` are the same table — so anything else would make the golden depend on
    /// something the language says is not there.
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
        // happen to be written in — a table has no order.
        let mut fields: Vec<&TableField> = fields.iter().collect();
        fields.sort_by_key(|field| match field {
            TableField::Named { name, .. } if name.text == "product" => 0,
            TableField::Named { name, .. } if name.text == "file" => 1,
            _ => 2,
        });

        for field in fields {
            let TableField::Named { name, value } = field else {
                self.todo(value.span(), "a positional entry in `versionInfo`");
                continue;
            };
            match name.text.as_str() {
                // `VIProductVersion` and `VIFileVersion` take four unquoted
                // numbers, and `makensis` rejects anything else — which is why
                // this is a checked shape rather than a string passed through.
                //
                // Two fields because Windows shows two versions: the product's,
                // which every installer needs, and the file's, which is this
                // build of it. NSIS defaults the second to the first, so
                // `file` alone is the shape that has nothing to default from —
                // and it is `makensis` that says so, not this compiler.
                field @ ("product" | "file") => {
                    let nsis = match field {
                        "product" => "VIProductVersion",
                        _ => "VIFileVersion",
                    };
                    let Some(text) = self.constant_string(value, field) else {
                        continue;
                    };
                    let quads = text.split('.').count() == 4
                        && text.split('.').all(|part| {
                            !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit())
                        });
                    if !quads {
                        self.bad_value(
                            value.span(),
                            field,
                            "four dotted numbers",
                            &format!(
                                "`{nsis}` wants `x.y.z.w` and `makensis` rejects any other shape"
                            ),
                        );
                        continue;
                    }
                    self.module
                        .attributes
                        .push(ir::Instruction::new(nsis, vec![ir::Arg::raw(text)]));
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
                        .note(format!("the fields are {}", list(VERSION_INFO_FIELDS))),
                    );
                }
            }
        }
    }

    // -- bodies -----------------------------------------------------------

    /// `installer { … }` and `uninstaller { … }`, which are the same block with
    /// two spellings — the whole of the uninstaller's duality is [`Half`]
    /// threaded through this one function. `un.` has no surface spelling at
    /// all.
    ///
    /// Two passes rather than one: the named fields are settings the whole
    /// block carries, and the positional entries — sections, pages, callbacks —
    /// read some of them. A Lua table has no order for the user to get right,
    /// so the order is the compiler's.
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
                // assemble clean under `-WX` and then lose.
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
                // The header strip's two colours, which MUI2 spends on the four
                // `SetCtlColors` in `MUI_INTERFACE`: the background, the two
                // lines of text and the image behind them. Block-scoped because
                // MUI2 reads them inside the `!ifndef`-guarded macro, so a
                // second page could not differ even if it asked.
                "headerColors" if half == Half::Installer => {
                    if let Some((text, back)) = self.colours(value, "headerColors") {
                        self.module.mui_defines.push(ir::Define {
                            name: "MUI_TEXTCOLOR".to_string(),
                            value: Some(text),
                        });
                        self.module.mui_defines.push(ir::Define {
                            name: "MUI_BGCOLOR".to_string(),
                            value: Some(back),
                        });
                        self.mui = true;
                    }
                }
                // The narrow description box, which is a different dialog
                // resource rather than a layout tweak: MUI2 spends it on a
                // `ChangeUI IDD_SELCOM` inside the `!ifndef`-guarded interface
                // macro, so it is the block's and the installer's alone.
                "smallDescriptions" if half == Half::Installer => match self.constant(value) {
                    Some(ConstValue::Bool(false)) => {}
                    Some(ConstValue::Bool(true)) => {
                        self.module.mui_defines.push(ir::Define {
                            name: "MUI_COMPONENTSPAGE_SMALLDESC".to_string(),
                            value: None,
                        });
                        self.mui = true;
                    }
                    _ => self.bad_value(
                        value.span(),
                        "smallDescriptions",
                        "a `bool`",
                        "`true` swaps the components page for the one with the short \
                         description box",
                    ),
                },
                "abortPrompt" => self.abort_prompt(value, half),
                "headerImage" => self.header_image(value, half),
                "wizardImage" => self.wizard_image(value, half),
                // `autoClose = false` keeps the install log on screen instead
                // of stepping to the finish page by itself. A block field and
                // not a `page.finish` one: MUI2 reads it from
                // `MUI_FINISHPAGE_GUIINIT`, behind an `!ifndef` on the half's
                // `WELCOMEFINISHPAGE_GUINIT`, so the second finish page of a
                // half could not differ even if it asked.
                "autoClose" => {
                    let define = match half {
                        Half::Installer => "MUI_FINISHPAGE_NOAUTOCLOSE",
                        Half::Uninstaller => "MUI_UNFINISHPAGE_NOAUTOCLOSE",
                    };
                    match self.constant(value) {
                        // MUI2 closes by itself unless told otherwise, so `true`
                        // is the default and writes nothing.
                        Some(ConstValue::Bool(true)) => {}
                        Some(ConstValue::Bool(false)) => {
                            self.module.mui_defines.push(ir::Define {
                                name: define.to_string(),
                                value: None,
                            });
                            self.mui = true;
                        }
                        _ => self.bad_value(
                            value.span(),
                            "autoClose",
                            "a `bool`",
                            "`false` leaves the install log up until the user clicks Next",
                        ),
                    }
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
                             define one name twice — write it in `installer {}`",
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

    /// `abortPrompt = true`, or `abortPrompt = { text = …, default = "cancel" }`
    /// — the "are you sure you want to quit" box, on the Cancel button.
    ///
    /// One field holding three defines, for the reason `colors` holds two:
    /// MUI2 reads `MUI_ABORTWARNING_TEXT` and `…_CANCEL_DEFAULT` only inside an
    /// `!ifdef MUI_ABORTWARNING`, so three flat fields would let a script write
    /// a message that nothing ever shows. Here the field's presence *is* the
    /// enable, and the invalid state has no spelling.
    ///
    /// Per half, unlike `headerColors`: `MUI_ABORTWARNING` and
    /// `MUI_UNABORTWARNING` are two names MUI2 reads in two places, so
    /// `uninstaller { abortPrompt = … }` is real and not a redefinition.
    fn abort_prompt(&mut self, value: &Expr, half: Half) {
        let (on, text, cancel) = match half {
            Half::Installer => (
                "MUI_ABORTWARNING",
                "MUI_ABORTWARNING_TEXT",
                "MUI_ABORTWARNING_CANCEL_DEFAULT",
            ),
            Half::Uninstaller => (
                "MUI_UNABORTWARNING",
                "MUI_UNABORTWARNING_TEXT",
                "MUI_UNABORTWARNING_CANCEL_DEFAULT",
            ),
        };

        let fields = match value {
            // `false` and leaving the field out are the same thing, so a
            // configuration that switches the prompt off has a spelling that is
            // not deleting the line.
            _ if matches!(self.constant(value), Some(ConstValue::Bool(false))) => return,
            _ if matches!(self.constant(value), Some(ConstValue::Bool(true))) => &[][..],
            Expr::Table { fields, .. } => fields,
            _ => {
                self.bad_value(
                    value.span(),
                    "abortPrompt",
                    "`true` or a table",
                    "write `abortPrompt = true` for MUI2's own wording, or `abortPrompt = { text \
                     = \"Really quit?\" }` for yours",
                );
                return;
            }
        };

        let mut defines = vec![ir::Define {
            name: on.to_string(),
            value: None,
        }];

        for field in fields {
            let TableField::Named { name, value } = field else {
                self.bad_value(
                    value.span(),
                    "abortPrompt",
                    "named fields",
                    "the fields are `text` and `default`",
                );
                continue;
            };
            match name.text.as_str() {
                // MUI2's own default is a language string, translated in every
                // language file it ships; a literal here is one language's
                // wording in all of them, which is the user's call to make.
                "text" => {
                    if let Some(arg) = self.constant_arg(value, "text") {
                        defines.push(ir::Define {
                            name: text.to_string(),
                            value: Some(arg),
                        });
                    }
                }
                // Which button Enter presses. `"ok"` writes nothing, because
                // that is what MUI2 does already — a define for the default
                // would be a line whose only effect is to exist.
                "default" => match self.constant(value).map(|constant| constant.text()) {
                    Some(button) if button == "cancel" => defines.push(ir::Define {
                        name: cancel.to_string(),
                        value: None,
                    }),
                    Some(button) if button == "ok" => {}
                    _ => self.bad_value(
                        value.span(),
                        "default",
                        "`\"ok\"` or `\"cancel\"`",
                        "`\"cancel\"` makes Enter keep installing; `\"ok\"` is MUI2's own default, \
                         where Enter quits",
                    ),
                },
                other => {
                    self.diags.push(
                        Diagnostic::error(
                            Code::UnknownField,
                            name.span,
                            format!("`{other}` is not an `abortPrompt` field"),
                        )
                        .note("the fields are `text` and `default`"),
                    );
                }
            }
        }

        self.module.mui_defines.extend(defines);
        self.mui = true;
    }

    /// `onGUIInit(function() … end)` and the other two — the function, and the
    /// define that is the only way MUI2 can be told about it.
    ///
    /// `onMouseOverSection` is the one whose absence would be silent in the
    /// *compiler's* output rather than MUI2's: MUI2 reads that define from
    /// inside `MUI_FUNCTION_DESCRIPTION_END` and from nowhere else, so writing
    /// the hook is what makes the block exist — a program with a hook and
    /// nothing to describe still gets `BEGIN`/`END` around an empty `${if}`.
    fn mui_callback(&mut self, value: &Expr, half: Half, hook: &'static MuiHook) {
        let Expr::Call { args, span, .. } = value else {
            return;
        };
        let [written] = args.as_slice() else {
            self.todo(*span, &format!("this `{}` form", hook.word));
            return;
        };
        let Some((block, at)) = self.callback_body(written, hook.word) else {
            return;
        };
        let name = format!("{}mui.{}", half.prefix(), hook.word);
        let body = self.body(block, &[], at, None, Some(half), table::Place::Anywhere);
        self.module.functions.push(ir::Function {
            name: name.clone(),
            body,
        });
        self.module.mui_defines.push(ir::Define {
            name: hook.define(half).to_string(),
            value: Some(ir::Arg::str(name)),
        });
        if hook.word == "onMouseOverSection" {
            self.hover[half.index()] = true;
        }
        if half == Half::Uninstaller && hook.needs_unpage {
            self.un_hooks.push((*span, hook.word));
        }
        self.mui = true;
    }

    /// Records one section's or group's hover text, and hands back the define
    /// the description will address it through.
    ///
    /// A section listed by a `local` already has one, and it is used: a minted
    /// name is never invented over a real one. A section written inline has
    /// none, and gets `SEC.desc.N` — a dot, so it cannot collide with the define
    /// [`index_name`] builds from a Lua local, which has no way to spell one.
    fn describe(&mut self, index: Option<String>, text: ir::Arg, half: Half) -> String {
        let index = index.unwrap_or_else(|| {
            let name = format!("SEC.desc.{}", self.minted);
            self.minted += 1;
            name
        });
        self.descriptions[half.index()].push((index.clone(), text));
        self.mui = true;
        index
    }

    /// `headerImage = "header.bmp"`, `= true`, or the table with the RTL half
    /// and the two script-wide switches.
    ///
    /// One field holding six defines, for `abortPrompt`'s reason: `Interface.nsh`
    /// reads every one of them inside `!ifdef MUI_HEADERIMAGE`, so the field's
    /// presence *is* the enable and a bitmap nothing displays has no spelling.
    /// `= true` is the whole of the enable — MUI2 then defaults the bitmap to the
    /// one it ships.
    ///
    /// Per half like `icon`, and by the same mechanism: the half picks
    /// `…_BITMAP` or `…_UNBITMAP`, which are two names MUI2 reads in two
    /// places. The enable itself is neither half's, so it is written once.
    fn header_image(&mut self, value: &Expr, half: Half) {
        let (bitmap, stretch, rtl, rtl_stretch) = match half {
            Half::Installer => (
                "MUI_HEADERIMAGE_BITMAP",
                "MUI_HEADERIMAGE_BITMAP_STRETCH",
                "MUI_HEADERIMAGE_BITMAP_RTL",
                "MUI_HEADERIMAGE_BITMAP_RTL_STRETCH",
            ),
            Half::Uninstaller => (
                "MUI_HEADERIMAGE_UNBITMAP",
                "MUI_HEADERIMAGE_UNBITMAP_STRETCH",
                "MUI_HEADERIMAGE_UNBITMAP_RTL",
                "MUI_HEADERIMAGE_UNBITMAP_RTL_STRETCH",
            ),
        };
        const MEMBERS: &str = "the fields are `file`, `stretch`, `rtl`, `right` and \
                               `transparentText`";

        let fields = match value {
            _ if matches!(self.constant(value), Some(ConstValue::Bool(true))) => &[][..],
            Expr::Table { fields, .. } => fields,
            // The short spelling, and the common one: a bitmap and MUI2's
            // defaults for everything about it.
            _ => {
                if let Some(file) = self.image_file(value, "headerImage") {
                    self.header_image_on();
                    self.module.mui_defines.push(ir::Define {
                        name: bitmap.to_string(),
                        value: Some(file),
                    });
                }
                return;
            }
        };

        self.header_image_on();
        for field in fields {
            let TableField::Named { name, value } = field else {
                self.bad_value(value.span(), "headerImage", "named fields", MEMBERS);
                continue;
            };
            match name.text.as_str() {
                "file" => {
                    if let Some(file) = self.image_file(value, "file") {
                        self.module.mui_defines.push(ir::Define {
                            name: bitmap.to_string(),
                            value: Some(file),
                        });
                    }
                }
                "stretch" => {
                    if let Some(mode) = self.stretch(value) {
                        self.module.mui_defines.push(ir::Define {
                            name: stretch.to_string(),
                            value: Some(mode),
                        });
                    }
                }
                // The right-to-left bitmap, and its own stretch: MUI2 reads
                // `…_RTL_STRETCH` only where `…_RTL` is defined, so the pair is
                // a table whose `file` is not optional.
                "rtl" => self.header_image_rtl(value, rtl, rtl_stretch),
                // Script-wide, with no `UN` spelling of their own — MUI2 reads
                // both once, for both halves.
                "right" | "transparentText" if half == Half::Uninstaller => {
                    self.diags.push(
                        Diagnostic::error(
                            Code::UnknownField,
                            name.span,
                            format!("`{}` is not an `uninstaller` field", name.text),
                        )
                        .note(
                            "MUI2 reads this one once for the whole script, so it governs both \
                             halves — write it in `installer { headerImage = { … } }`",
                        ),
                    );
                }
                "right" => self.header_image_flag(value, "right", "MUI_HEADERIMAGE_RIGHT"),
                "transparentText" => {
                    self.header_image_flag(value, "transparentText", "MUI_HEADER_TRANSPARENT_TEXT")
                }
                other => {
                    self.diags.push(
                        Diagnostic::error(
                            Code::UnknownField,
                            name.span,
                            format!("`{other}` is not a `headerImage` field"),
                        )
                        .note(MEMBERS),
                    );
                }
            }
        }
    }

    /// `MUI_HEADERIMAGE`, once. Both blocks may carry the field and the enable
    /// is neither one's: written twice it is a redefinition, which is a warning
    /// and so an error under `-WX` (tier 3).
    fn header_image_on(&mut self) {
        self.mui = true;
        if self
            .module
            .mui_defines
            .iter()
            .any(|define| define.name == "MUI_HEADERIMAGE")
        {
            return;
        }
        self.module.mui_defines.push(ir::Define {
            name: "MUI_HEADERIMAGE".to_string(),
            value: None,
        });
    }

    fn header_image_flag(&mut self, value: &Expr, which: &str, define: &str) {
        match self.constant(value) {
            Some(ConstValue::Bool(false)) => {}
            Some(ConstValue::Bool(true)) => self.module.mui_defines.push(ir::Define {
                name: define.to_string(),
                value: None,
            }),
            _ => self.bad_value(
                value.span(),
                which,
                "a `bool`",
                "MUI2 reads this one with `!ifdef`, so it is on or absent",
            ),
        }
    }

    /// `rtl = "header-rtl.bmp"` or `rtl = { file = …, stretch = … }`.
    fn header_image_rtl(&mut self, value: &Expr, rtl: &str, rtl_stretch: &str) {
        let fields = match value {
            Expr::Table { fields, .. } => fields,
            _ => {
                if let Some(file) = self.image_file(value, "rtl") {
                    self.module.mui_defines.push(ir::Define {
                        name: rtl.to_string(),
                        value: Some(file),
                    });
                }
                return;
            }
        };

        let Some(file) = named(fields, "file") else {
            self.diags.push(
                Diagnostic::error(
                    Code::MissingAttribute,
                    value.span(),
                    "`rtl` has no `file`".to_string(),
                )
                .note(
                    "MUI2 reads the right-to-left stretch only where the right-to-left bitmap \
                     is defined, so a table without one writes nothing that is read",
                ),
            );
            return;
        };
        if let Some(file) = self.image_file(file, "file") {
            self.module.mui_defines.push(ir::Define {
                name: rtl.to_string(),
                value: Some(file),
            });
        }

        for field in fields {
            let TableField::Named { name, value } = field else {
                self.bad_value(
                    value.span(),
                    "rtl",
                    "named fields",
                    "the fields are `file` and `stretch`",
                );
                continue;
            };
            match name.text.as_str() {
                "file" => {}
                "stretch" => {
                    if let Some(mode) = self.stretch(value) {
                        self.module.mui_defines.push(ir::Define {
                            name: rtl_stretch.to_string(),
                            value: Some(mode),
                        });
                    }
                }
                other => {
                    self.diags.push(
                        Diagnostic::error(
                            Code::UnknownField,
                            name.span,
                            format!("`{other}` is not an `rtl` field"),
                        )
                        .note("the fields are `file` and `stretch`"),
                    );
                }
            }
        }
    }

    /// `wizardImage = "wizard.bmp"` or `= { file = …, stretch = … }` — the tall
    /// bitmap down the side of the welcome and finish pages.
    ///
    /// A block field because it is *two* pages' and neither's: welcome and
    /// finish read the same define, so a page that carried it would be one of
    /// two places to write one setting. There is no enable to go with it —
    /// MUI2 draws its own `win.bmp` unless told otherwise — so `file` is what
    /// the field is for and a table without one is refused.
    fn wizard_image(&mut self, value: &Expr, half: Half) {
        let (bitmap, stretch) = match half {
            Half::Installer => (
                "MUI_WELCOMEFINISHPAGE_BITMAP",
                "MUI_WELCOMEFINISHPAGE_BITMAP_STRETCH",
            ),
            Half::Uninstaller => (
                "MUI_UNWELCOMEFINISHPAGE_BITMAP",
                "MUI_UNWELCOMEFINISHPAGE_BITMAP_STRETCH",
            ),
        };

        let fields = match value {
            Expr::Table { fields, .. } => fields,
            _ => {
                if let Some(file) = self.image_file(value, "wizardImage") {
                    self.module.mui_defines.push(ir::Define {
                        name: bitmap.to_string(),
                        value: Some(file),
                    });
                    self.mui = true;
                }
                return;
            }
        };

        let Some(file) = named(fields, "file") else {
            self.diags.push(
                Diagnostic::error(
                    Code::MissingAttribute,
                    value.span(),
                    "`wizardImage` has no `file`".to_string(),
                )
                .note(
                    "write `wizardImage = \"wizard.bmp\"`, or leave the field out for MUI2's own",
                ),
            );
            return;
        };
        if let Some(file) = self.image_file(file, "file") {
            self.module.mui_defines.push(ir::Define {
                name: bitmap.to_string(),
                value: Some(file),
            });
            self.mui = true;
        }

        for field in fields {
            let TableField::Named { name, value } = field else {
                self.bad_value(
                    value.span(),
                    "wizardImage",
                    "named fields",
                    "the fields are `file` and `stretch`",
                );
                continue;
            };
            match name.text.as_str() {
                "file" => {}
                "stretch" => {
                    if let Some(mode) = self.stretch(value) {
                        self.module.mui_defines.push(ir::Define {
                            name: stretch.to_string(),
                            value: Some(mode),
                        });
                    }
                }
                other => {
                    self.diags.push(
                        Diagnostic::error(
                            Code::UnknownField,
                            name.span,
                            format!("`{other}` is not a `wizardImage` field"),
                        )
                        .note("the fields are `file` and `stretch`"),
                    );
                }
            }
        }
    }

    /// A bitmap named by a build-time path, the way `icon` is.
    fn image_file(&mut self, value: &Expr, which: &str) -> Option<ir::Arg> {
        Some(self.constant_arg(value, which)?.into_path())
    }

    /// One of MUI2's four stretch modes, checked against its own `!if` chain.
    fn stretch(&mut self, value: &Expr) -> Option<ir::Arg> {
        let mode = self.constant(value).map(|constant| constant.text());
        match mode {
            Some(mode) if STRETCH_MODES.contains(&mode.as_str()) => Some(ir::Arg::str(mode)),
            _ => {
                self.bad_value(
                    value.span(),
                    "stretch",
                    &list(STRETCH_MODES),
                    "MUI2 warns and falls back to `\"FitControl\"`, which is a wrong image at \
                     build time and a right one only by accident",
                );
                None
            }
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
    /// The order is the whole of the install-type binding at this end. A
    /// section says which types it belongs to *by name*, NSIS reads only a
    /// one-based position, and this list is what turns one into the other — so
    /// the numbering exists in exactly one place and a user never writes a
    /// number that could go stale when a type is inserted in front of it.
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
    ///
    /// `bound` is the local a `page.startMenu { … }` was bound to, and `None`
    /// for every other page — the seven that are entries and nothing else.
    fn page(&mut self, call: &Expr, which: &Name, half: Half, bound: Option<&str>) {
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
                // The table form: the array part is the parameter. Only the
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
        // `MUI_PAGE_STARTMENU` takes an id and a variable, and both are the
        // compiler's: the id is the local, which is already unique, and the
        // variable is minted from it. A start menu page written inline has
        // neither, and nothing to address the folder it chose — so it is
        // refused here rather than assembled into a page whose answer is
        // unreachable.
        if page.installua == "startMenu" {
            let Some(local) = bound else {
                self.diags.push(
                    Diagnostic::error(
                        Code::MissingAttribute,
                        which.span,
                        "a `startMenu` page has to be bound to a local",
                    )
                    .note("write `local menu = page.startMenu { … }` and list `menu,` in the block")
                    .note(
                        "the local is the page's id: it is what `menu.folder` and `menu.write` \
                         name, and `MUI_PAGE_STARTMENU` takes one either way",
                    ),
                );
                return;
            };
            macro_args.push(ir::Arg::raw(local));
            macro_args.push(ir::Arg::var(format!("${}", start_menu_var(local))));
        }
        if page.installua == "license" {
            let file = named.iter().find(|(name, _)| name.text == "file");
            let arg = match file {
                // A table is one file per locale, which NSIS spells
                // `LicenseLangString` — the same transposition `languages {}`
                // does, arriving from the other direction.
                Some((_, value @ Expr::Table { .. })) => self.license_langstring(value),
                Some((_, value)) => self.constant_arg(value, "file").map(ir::Arg::into_path),
                None => None,
            };
            match arg {
                Some(arg) => macro_args.push(arg),
                None => {
                    if file.is_none() {
                        self.diags.push(
                            Diagnostic::error(
                                Code::MissingAttribute,
                                which.span,
                                "a `license` page needs a `file`",
                            )
                            .note("write `page.license { file = \"LICENSE.txt\" }`")
                            .note("`MUI_PAGE_LICENSE` takes the file as its argument"),
                        );
                    }
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

    /// `file = { English = "en.txt", German = "de.txt" }` — one license per
    /// locale, which is `LicenseLangString`.
    ///
    /// The name it files them under is the compiler's, so it collides with no
    /// `LangString` a translator wrote, and what the page macro gets back is a
    /// `$(…)` read rather than a path. Everything else about the page is
    /// unchanged: this is a second shape for one field, not a second field and
    /// not a block of its own — a license page with translated text is still
    /// one page with one license on it.
    ///
    /// **The set has to match `languages { locales }` exactly.** Both halves of
    /// that are the same failure with different symptoms. A declared locale with
    /// no file leaves `$(…)` expanding to nothing, which is a blank license page
    /// on one machine in one country — the failure hardest to find and cheapest
    /// to prevent, and the same argument [`Self::completeness`] makes about
    /// `LangString`s. A file for an undeclared locale names a `${LANG_…}` that
    /// no `MUI_LANGUAGE` defined, which NSIS reports against a line the user
    /// never wrote.
    fn license_langstring(&mut self, value: &Expr) -> Option<ir::Arg> {
        let Expr::Table { fields, span } = value else {
            return None;
        };
        let span = *span;

        if self.locales.is_empty() {
            self.diags.push(
                Diagnostic::error(
                    Code::MissingAttribute,
                    span,
                    "a license file per locale needs a `languages {}` block",
                )
                .note(
                    "each entry becomes `LicenseLangString name ${LANG_…} path`, and the \
                     `${LANG_…}` is defined by the `MUI_LANGUAGE` that `languages {}` emits",
                )
                .note("one license for every language is `file = \"LICENSE.txt\"`"),
            );
            return None;
        }

        let mut files: BTreeMap<String, ir::Arg> = BTreeMap::new();
        let mut bad = false;
        for field in fields {
            let TableField::Named { name, value } = field else {
                self.bad_value(
                    field.span(),
                    "file",
                    "a table keyed by language",
                    "every entry is `<Language> = \"path\"`",
                );
                bad = true;
                continue;
            };
            if !crate::locale::is_locale(&name.text) {
                self.unknown_locale(&name.text, name.span);
                bad = true;
                continue;
            }
            if !self.locales.contains(&name.text) {
                let (text, at) = (name.text.clone(), name.span);
                let declared = list(&self.locales.iter().map(String::as_str).collect::<Vec<_>>());
                self.diags.push(
                    Diagnostic::error(
                        Code::UnknownField,
                        at,
                        format!("`{text}` is not one of this program's languages"),
                    )
                    .note(format!("`languages {{ locales }}` declares {declared}"))
                    .note(format!(
                        "`${{LANG_{}}}` is defined by `!insertmacro MUI_LANGUAGE \"{text}\"`, \
                         which nothing here emits",
                        text.to_uppercase()
                    )),
                );
                bad = true;
                continue;
            }
            match self.constant_arg(value, &name.text) {
                Some(arg) => {
                    files.insert(name.text.clone(), arg.into_path());
                }
                None => bad = true,
            }
        }

        // The other direction, reported against the locale that is missing
        // rather than against the table, so the message names the language a
        // translator has to go and find.
        let first = self.locales[0].clone();
        for locale in self.locales.clone() {
            if files.contains_key(&locale) {
                continue;
            }
            self.diags.push(
                Diagnostic::error(
                    Code::MissingAttribute,
                    span,
                    format!("`{locale}` has no license file"),
                )
                .note(format!("`{first}` has one"))
                .note(
                    "NSIS expands a license string with no entry for the running language to \
                     nothing at all, so the gap would be a blank license page rather than an \
                     error",
                ),
            );
            bad = true;
        }
        if bad {
            return None;
        }

        self.license_tables += 1;
        let name = match self.license_tables {
            1 => "licenseData".to_string(),
            n => format!("licenseData{n}"),
        };
        // Emitted in declaration order, which is the order the `MUI_LANGUAGE`
        // lines above them run in — the first is NSIS's default language.
        for locale in &self.locales {
            self.module.license_data.push(ir::Instruction::new(
                "LicenseLangString",
                vec![
                    ir::Arg::raw(name.clone()),
                    ir::Arg::raw(format!("${{{}}}", crate::locale::define(locale))),
                    files[locale].clone(),
                ],
            ));
        }
        Some(ir::Arg::var(format!("$({name})")))
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
    /// different *mechanism*, which is the split MUI2's page world already
    /// draws. `Page custom` is a stock NSIS instruction and MUI2 never sees it,
    /// so `MUI_PAGE_HEADER_TEXT` — a define MUI2 reads from inside the `PageEx`
    /// it generates — would sit there doing nothing and then leak onto the next
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
            Some(value) => self.controls(value, half),
            None => Vec::new(),
        };

        let pre = written("pre").and_then(|value| self.callback_body(value, "pre"));
        let show = written("show").and_then(|value| self.callback_body(value, "show"));
        let leave =
            written("leave").and_then(|value| self.page_callback(value, half, page, "leave"));

        let span = which.span;
        let create = self.page_function_name(half, page, "create");
        let (body, _) = self.body_with(span, Some(half), table::Place::Anywhere, |lowerer| {
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
                for post in &control.post {
                    let mut args = post.before.clone();
                    args.push(ir::Arg::slot(handle.clone()));
                    args.extend(post.after.iter().cloned());
                    lowerer.emit(ir::Instruction::new(post.nsis, args));
                }

                // The callbacks. `GetFunctionAddress` is a `todo` row and stays
                // one: the reason — *`Call`-by-address has no Lua shape* — is
                // still true of the **surface**, and the compiler emitting it
                // is the same move as emitting `SectionGetFlags`. The address
                // of a generated function exists in exactly one place, which is
                // what makes writing it here safe and writing it by hand not.
                for (nsis, function) in &control.events {
                    let address = lowerer.body.vreg(control.span);
                    lowerer.emit(ir::Instruction::new(
                        "GetFunctionAddress",
                        vec![ir::Arg::dest(address.clone()), ir::Arg::raw(function)],
                    ));
                    lowerer.generated_plugin_call(
                        nsis,
                        vec![ir::Arg::slot(handle.clone()), ir::Arg::slot(address)],
                        Vec::new(),
                        control.span,
                    );
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
    fn controls(&mut self, value: &Expr, half: Half) -> Vec<Created> {
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
                created.extend(self.control(declared, control, Some(var), half, Some(&name.text)));
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
            created.extend(self.control(value, control, None, half, None));
        }
        created
    }

    /// One control declaration, checked and turned into the plugin call that
    /// creates it.
    ///
    /// The table form with nothing positional but the text: `nsDialogs` takes
    /// x, y, width and height as four separate arguments and a table's array
    /// part is a sequence, so writing them unnamed would be four numbers in an
    /// order a reader has to know. The one thing that *is* the control's
    /// parameter — the text it is drawn with — stays where the table form puts
    /// it.
    fn control(
        &mut self,
        value: &Expr,
        control: &'static control::Control,
        var: Option<String>,
        half: Half,
        local: Option<&str>,
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
        let mut post = Vec::new();
        let mut events = Vec::new();
        let mut url = None;
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
                        let message = handle::message(control.add_item.unwrap_or_default());
                        post.extend(self.items(value).into_iter().map(|item| Post {
                            nsis: "SendMessage",
                            before: Vec::new(),
                            after: vec![message.clone(), ir::Arg::int(0), item],
                        }));
                        continue;
                    }
                    // A `bitmap` with no picture is an empty rectangle, so the
                    // field that gives it one is also an option: a control that
                    // has to be claimed and written to in a callback before it
                    // draws anything is a declaration that does not declare.
                    if name.text == "image" {
                        if !control.imageable() {
                            self.diags.push(
                                Diagnostic::error(
                                    Code::UnknownField,
                                    name.span,
                                    format!("a `{what}` draws no image"),
                                )
                                .note("`image` is a `bitmap`'s, which is the kind that is one"),
                            );
                            continue;
                        }
                        if let Some(path) = self.constant_arg(value, "image") {
                            let mut args = handle::image_args(path);
                            let before = vec![args.remove(0)];
                            post.push(Post {
                                nsis: "LoadAndSetImage",
                                before,
                                after: args,
                            });
                        }
                        continue;
                    }
                    // The two events, which are options rather than fields
                    // because the address of a function is a build-time fact:
                    // there is no moment at install time when a callback could
                    // be *assigned* that is not already inside the callback
                    // this compiler generates (ruling 8).
                    if let Some(event) = control::event(&name.text) {
                        if !event.on(control) {
                            self.diags.push(
                                Diagnostic::error(
                                    Code::UnknownField,
                                    name.span,
                                    format!("a `{what}` has no `{}`", event.option()),
                                )
                                .note(format!("{} does", event.kinds())),
                            );
                            continue;
                        }
                        if let Some(function) = self.control_callback(value, half, local, event) {
                            events.push((event.nsis(), function));
                        }
                        continue;
                    }
                    // A link's address, which is the click it would otherwise
                    // have to be written as. `url` and `onClick` are the same
                    // slot, so a control that writes both is asking for two
                    // things to happen and getting one.
                    if name.text == "url" {
                        if !control.clicks() {
                            self.diags.push(
                                Diagnostic::error(
                                    Code::UnknownField,
                                    name.span,
                                    format!("a `{what}` opens nothing"),
                                )
                                .note(
                                    "`url` is what a click opens, so it is on the kinds that are \
                                     clicked; a `link` is the one drawn as one",
                                ),
                            );
                            continue;
                        }
                        url = self.constant_arg(value, "url").map(|arg| (arg, name.span));
                        continue;
                    }
                    let mut options = vec!["x", "y", "width", "height"];
                    if control.takes_items() {
                        options.push("items");
                    }
                    if control.imageable() {
                        options.push("image");
                    }
                    if control.clicks() {
                        options.push("onClick");
                        options.push("url");
                    }
                    if control.changes() {
                        options.push("onChange");
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

        // The address, as the click it would otherwise have to be written as.
        // Resolved after the loop, because `url` and `onClick` register the same
        // callback and the table they are written in has no order.
        if let Some((address, span)) = url {
            let taken = events
                .iter()
                .any(|(nsis, _)| *nsis == control::Event::Click.nsis());
            match taken {
                true => self.diags.push(
                    Diagnostic::error(
                        Code::DuplicateBlock,
                        span,
                        format!("this `{what}` has both a `url` and an `onClick`"),
                    )
                    .note(
                        "a `url` is an `onClick` this compiler writes: one control has one click, \
                         and the two would be one silently replacing the other",
                    )
                    .note("write the `execShell` yourself, inside the `onClick`"),
                ),
                false => {
                    let function = self.callback_function(half, local, "url", span, |lowerer| {
                        // `ExecShell "open"` is what a shortcut to a URL
                        // does, which is the behaviour a user expects of a
                        // link: their browser, not one this installer picks.
                        lowerer.emit(ir::Instruction::new(
                            "ExecShell",
                            vec![ir::Arg::str("open"), address],
                        ));
                    });
                    events.push((control::Event::Click.nsis(), function));
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
            post,
            events,
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
        // A free function rather than the closure it was, because
        // `Holds::Nested` and `Holds::Off` now lower their parts by calling
        // this method again — and a closure holding `defines` would be the
        // borrow that recursion cannot get past.
        fn define(
            defines: &mut Vec<ir::Define>,
            undefines: &mut Vec<String>,
            name: &str,
            value: Option<ir::Arg>,
            cleared: bool,
        ) {
            defines.push(ir::Define {
                name: name.to_string(),
                value,
            });
            if !cleared {
                undefines.push(name.to_string());
            }
        }

        match field.holds {
            Holds::Str => {
                if let Some(arg) = self.constant_arg(value, field.installua) {
                    define(defines, undefines, field.define, Some(arg), field.cleared);
                }
            }
            Holds::Flag => match self.constant(value) {
                // `false` is not a define with a false value: MUI2 asks
                // `!ifdef`, so the only way to say no is to say nothing.
                Some(ConstValue::Bool(false)) => {}
                Some(ConstValue::Bool(true)) => {
                    define(defines, undefines, field.define, None, field.cleared);
                }
                _ => self.bad_value(
                    value.span(),
                    field.installua,
                    "a `bool`",
                    "MUI2 reads this one with `!ifdef`, so it is on or absent",
                ),
            },
            Holds::Caption(indices) => {
                let Some(index) = indices[half as usize] else {
                    // Three of the five installer pages have no uninstaller
                    // number, and this is not a gap in the row: NSIS numbers
                    // three uninstaller pages and license is not among them, so
                    // there is no line to write rather than one we decline to.
                    self.diags.push(
                        Diagnostic::error(
                            Code::UnknownField,
                            value.span(),
                            format!("`{}` has no uninstaller half", field.installua),
                        )
                        .note(format!(
                            "`UninstallSubCaption` numbers only the confirm, instFiles and \
                             completed pages, so a `{}` page has none",
                            page.installua
                        )),
                    );
                    return;
                };
                if let Some(arg) = self.constant_arg(value, field.installua) {
                    let line = match half {
                        Half::Installer => "SubCaption",
                        Half::Uninstaller => "UninstallSubCaption",
                    };
                    self.module.attributes.push(ir::Instruction::new(
                        line,
                        vec![ir::Arg::raw(index.to_string()), arg],
                    ));
                }
            }
            Holds::Var => {
                let Some(name) = value.name() else {
                    self.bad_value(
                        value.span(),
                        field.installua,
                        "a global",
                        "NSIS stores the chosen directory into this one, so it wants the \
                         variable and not its value",
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
                        .note("a global is declared by assigning to it at the top level"),
                    );
                    return;
                }
                define(
                    defines,
                    undefines,
                    field.define,
                    Some(ir::Arg::var(format!("${name}"))),
                    field.cleared,
                );
            }
            Holds::Callback => {
                let Some(name) = self.page_callback(value, half, page, field.installua) else {
                    return;
                };
                define(
                    defines,
                    undefines,
                    field.define,
                    Some(ir::Arg::str(name)),
                    field.cleared,
                );
            }
            Holds::Text(text) => {
                if let Some(arg) = self.constant_arg(value, field.installua) {
                    define(defines, undefines, field.define, None, field.cleared);
                    define(defines, undefines, text, Some(arg), field.cleared);
                }
            }
            Holds::Colors(text) => {
                if let Some((text_arg, back)) = self.colours(value, field.installua) {
                    define(defines, undefines, field.define, Some(back), field.cleared);
                    define(defines, undefines, text, Some(text_arg), field.cleared);
                }
            }
            // A string on its own, or one with the define that gives it room.
            // Written apart, because the plain spelling has to stay plain: the
            // table is the exception and `title = "Done"` is the rule.
            Holds::Roomy(more, room) => {
                let (text, roomy) = self.roomy(value, field, room);
                if let Some(text) = text
                    && let Some(arg) = self.constant_arg(text, field.installua)
                {
                    define(defines, undefines, field.define, Some(arg), field.cleared);
                }
                if roomy {
                    define(defines, undefines, more, None, field.cleared);
                }
            }
            Holds::Word { writes, silent } => {
                match self.constant(value).map(|constant| constant.text()) {
                    Some(word) if word == writes => {
                        define(defines, undefines, field.define, None, field.cleared);
                    }
                    // MUI2's own default, which is the absence of the define —
                    // so the word that names it writes nothing, and a script
                    // can pass either without branching.
                    Some(word) if word == silent => {}
                    _ => self.bad_value(
                        value.span(),
                        field.installua,
                        &format!("`\"{writes}\"` or `\"{silent}\"`"),
                        &format!("`\"{silent}\"` is MUI2's own default"),
                    ),
                }
            }
            // The inverse of `Nested`: the table is the *on* state, and the
            // field's own define is the opt-out that only `false` writes.
            Holds::Off(parts) => match self.constant(value) {
                Some(ConstValue::Bool(false)) => {
                    define(defines, undefines, field.define, None, field.cleared);
                }
                Some(ConstValue::Bool(true)) => {}
                _ => {
                    let Expr::Table { fields, .. } = value else {
                        self.bad_value(
                            value.span(),
                            field.installua,
                            "`false` or a table",
                            "write `reboot = false` to drop the reboot half of the page, or \
                             `reboot = { later = \"Restart later\" }` to word it yourself",
                        );
                        return;
                    };
                    self.nested_fields(field, fields, parts, half, page, defines, undefines);
                }
            },
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
                define(defines, undefines, field.define, None, field.cleared);
                self.nested_fields(field, fields, parts, half, page, defines, undefines);
            }
            Holds::Calls { function, stem } => {
                let Some(name) = self.page_callback(value, half, page, stem) else {
                    return;
                };
                // Empty rather than absent: MUI2's own documentation writes
                // `!define MUI_FINISHPAGE_RUN ""` for this case, and the
                // define is read by an `!ifdef` in every branch that a
                // function reaches.
                define(
                    defines,
                    undefines,
                    field.define,
                    Some(ir::Arg::str(String::new())),
                    field.cleared,
                );
                define(
                    defines,
                    undefines,
                    function,
                    Some(ir::Arg::str(name)),
                    field.cleared,
                );
            }
            Holds::Not => match self.constant(value) {
                // MUI2's default is ticked, and the define is how a script
                // says otherwise — so the *true* case is the one that writes
                // nothing.
                Some(ConstValue::Bool(true)) => {}
                Some(ConstValue::Bool(false)) => {
                    define(defines, undefines, field.define, None, field.cleared);
                }
                _ => self.bad_value(
                    value.span(),
                    field.installua,
                    "a `bool`",
                    "the box is ticked when the page opens unless this says `false`",
                ),
            },
            Holds::Widget(forms) => {
                self.widget(field, value, forms, half, page, defines, undefines);
            }
            // Three states and two defines, and the third state writes neither:
            // MUI2 draws the box and words it from its own language file unless
            // told otherwise.
            Holds::Checkbox(text) => match self.constant(value) {
                Some(ConstValue::Bool(false)) => {
                    define(defines, undefines, field.define, None, field.cleared);
                }
                Some(ConstValue::Bool(true)) => {}
                _ => {
                    if let Some(arg) = self.constant_arg(value, field.installua) {
                        define(defines, undefines, text, Some(arg), field.cleared);
                    }
                }
            },
        }
    }

    /// `run = { path = … }`, `run = { call = … }` or `run = "…"`: the spelling
    /// picked by which key is written, and then lowered like any other table.
    ///
    /// Exactly one key, because the two spellings are two different MUI2
    /// branches and a table with both would say which program to `Exec` and
    /// then call a function instead of `Exec`ing anything.
    #[allow(clippy::too_many_arguments)]
    fn widget(
        &mut self,
        field: &PageField,
        value: &Expr,
        forms: &'static [Form],
        half: Half,
        page: &Page,
        defines: &mut Vec<ir::Define>,
        undefines: &mut Vec<String>,
    ) {
        let keys = list(&forms.iter().map(|form| form.key).collect::<Vec<_>>());

        // The short spelling: `run = "$INSTDIR\\foo.exe"` is the first form's
        // key and nothing else. Offered only where that form needs nothing
        // more, since a `link` is never one string.
        let Expr::Table { fields, .. } = value else {
            let first = &forms[0];
            let short = first
                .needs
                .is_empty()
                .then(|| first.parts.iter().find(|part| part.installua == first.key))
                .flatten();
            let Some(key) = short else {
                self.bad_value(
                    value.span(),
                    field.installua,
                    "a table",
                    &format!(
                        "the fields are {}",
                        list(
                            &first
                                .parts
                                .iter()
                                .map(|part| part.installua)
                                .collect::<Vec<_>>()
                        )
                    ),
                );
                return;
            };
            self.page_field(key, value, half, page, defines, undefines);
            return;
        };

        let written: Vec<&Form> = forms
            .iter()
            .filter(|form| named(fields, form.key).is_some())
            .collect();
        let [form] = written.as_slice() else {
            let (message, note) = if written.is_empty() {
                (
                    format!("`{}` says nothing to do", field.installua),
                    format!("write {keys} to say what this one is for"),
                )
            } else {
                (
                    format!("`{}` says two things to do at once", field.installua),
                    format!("{keys} are two spellings of the same checkbox, so write one"),
                )
            };
            self.diags
                .push(Diagnostic::error(Code::BadFieldValue, value.span(), message).note(note));
            return;
        };

        for need in form.needs {
            if named(fields, need).is_none() {
                self.diags.push(
                    Diagnostic::error(
                        Code::MissingAttribute,
                        value.span(),
                        format!("`{}` has no `{need}`", field.installua),
                    )
                    .note(format!(
                        "`{}` is what the user reads and `{need}` is where the click goes, \
                         so MUI2 wants both",
                        form.key
                    )),
                );
                return;
            }
        }
        self.nested_fields(field, fields, form.parts, half, page, defines, undefines);
    }

    /// The parts of a nested field, lowered by their own [`Holds`] and checked
    /// for names the part list does not have.
    ///
    /// Shared by [`Holds::Nested`] and [`Holds::Off`], which differ in what
    /// they do about the *outer* define and in nothing else.
    #[allow(clippy::too_many_arguments)]
    fn nested_fields(
        &mut self,
        field: &PageField,
        fields: &[TableField],
        parts: &'static [PageField],
        half: Half,
        page: &Page,
        defines: &mut Vec<ir::Define>,
        undefines: &mut Vec<String>,
    ) {
        for part in parts {
            let Some(written) = named(fields, part.installua) else {
                continue;
            };
            self.page_field(part, written, half, page, defines, undefines);
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

    /// `title = "Done"` or `title = { text = "Done", lines = 3 }`, as the
    /// string and whether the second define comes with it.
    fn roomy<'e>(
        &mut self,
        value: &'e Expr,
        field: &PageField,
        room: Room,
    ) -> (Option<&'e Expr>, bool) {
        let key = match room {
            Room::Lines => "lines",
            Room::Large => "large",
        };
        let Expr::Table { fields, .. } = value else {
            return (Some(value), false);
        };

        let mut text = None;
        let mut roomy = false;
        for entry in fields {
            let TableField::Named { name, value } = entry else {
                self.bad_value(
                    value.span(),
                    field.installua,
                    "named fields",
                    &format!("the fields are `text` and `{key}`"),
                );
                continue;
            };
            match name.text.as_str() {
                "text" => text = Some(value),
                written if written == key => roomy = self.room(value, room, field.installua),
                other => {
                    self.diags.push(
                        Diagnostic::error(
                            Code::UnknownField,
                            name.span,
                            format!("`{other}` is not a `{}` field", field.installua),
                        )
                        .note(format!("the fields are `text` and `{key}`")),
                    );
                }
            }
        }
        (text, roomy)
    }

    /// Whether a [`Holds::Roomy`] field asked for the taller box.
    fn room(&mut self, value: &Expr, room: Room, which: &str) -> bool {
        match (room, self.constant(value)) {
            // Two or three, and nothing between or beyond: MUI2 has one title
            // height apiece and no third. A number rather than a `bool`
            // because that is what the setting is — how many lines the title
            // is given — and `lines = 4` is worth an error rather than a
            // silent 3.
            (Room::Lines, Some(ConstValue::Int(3))) => true,
            (Room::Lines, Some(ConstValue::Int(2))) => false,
            (Room::Large, Some(ConstValue::Bool(large))) => large,
            (Room::Lines, _) => {
                self.bad_value(
                    value.span(),
                    "lines",
                    "`2` or `3`",
                    &format!("MUI2 draws `{which}` in a box of one height or the other"),
                );
                false
            }
            (Room::Large, _) => {
                self.bad_value(
                    value.span(),
                    "large",
                    "a `bool`",
                    &format!("`true` gives `{which}` the taller box, for text that needs it"),
                );
                false
            }
        }
    }

    /// `pre = function() … end` — an NSIS `Function` MUI2 calls by name.
    ///
    /// The name is the compiler's, because nothing in the source is one: the
    /// hook is written where it runs. `un.` leads the uninstaller's, since MUI2
    /// calls it from an uninstaller page and NSIS spells that half in the
    /// function's name.
    fn page_callback(
        &mut self,
        value: &Expr,
        half: Half,
        page: &Page,
        which: &str,
    ) -> Option<String> {
        let (block, span) = self.callback_body(value, which)?;
        let name = self.page_function_name(half, page, which);
        let body = self.body(block, &[], span, None, Some(half), table::Place::Anywhere);
        self.module.functions.push(ir::Function {
            name: name.clone(),
            body,
        });
        Some(name)
    }

    /// `onClick = function() … end`, as the function nsDialogs will call.
    fn control_callback(
        &mut self,
        value: &Expr,
        half: Half,
        local: Option<&str>,
        event: control::Event,
    ) -> Option<String> {
        let (block, span) = self.callback_body(value, event.option())?;
        Some(
            self.callback_function(half, local, event.word(), span, |lowerer| {
                lowerer.block(block);
            }),
        )
    }

    /// A function nsDialogs calls, with the one line of protocol it owes.
    ///
    /// **The handle has to be popped.** nsDialogs pushes the control's `HWND`
    /// before calling, and a callback that leaves it there corrupts the stack
    /// for everything after — which shows up as a wrong string in an unrelated
    /// instruction rather than as a crash. The program has no use for it: it
    /// already knows which control this is, because it wrote the callback on
    /// that control's declaration.
    fn callback_function(
        &mut self,
        half: Half,
        local: Option<&str>,
        which: &str,
        span: Span,
        build: impl FnOnce(&mut BodyLowerer),
    ) -> String {
        let stem = match local {
            Some(local) => format!("{}mui.control.{local}.{which}", half.prefix()),
            None => format!("{}mui.control.{which}", half.prefix()),
        };
        let mut name = stem.clone();
        let mut nth = 2;
        while self.module.functions.iter().any(|f| f.name == name) {
            name = format!("{stem}.{nth}");
            nth += 1;
        }

        let (body, _) = self.body_with(span, Some(half), table::Place::Anywhere, |lowerer| {
            let pushed = lowerer.body.vreg(span);
            lowerer.emit(ir::Instruction::new("Pop", vec![ir::Arg::dest(pushed)]));
            build(lowerer);
        });
        self.module.functions.push(ir::Function {
            name: name.clone(),
            body,
        });
        name
    }

    /// The block behind `pre = function() … end`, checked.
    ///
    /// Split from [`Self::page_callback`] because a custom page's `pre` and
    /// `show` become no function at all: `Page custom` has two slots and three
    /// hooks, so two of them are inlined into the creator.
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
                .note("NSIS calls it, and `Call` has no argument list"),
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
            // eight and a typo is caught where it is written.
            if let Some((base, which)) = value.callee_field()
                && base == "page"
            {
                self.page(value, which, half, None);
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
                if let Some(section) = self.section(value, half, None) {
                    self.module.sections.push(ir::SectionItem::Section(section));
                }
            }
            "group" => self.group(value, half, None),
            "onInit" => self.callback(value, half, "onInit"),
            other => match MuiHook::named(other) {
                // The MUI2 hooks, which are entries and not fields for the
                // reason `onInit` is: a block's positional entries are its
                // declarations of code, and its named fields are its settings.
                Some(hook) => self.mui_callback(value, half, hook),
                None => self.todo(value.span(), &format!("`{other}` here")),
            },
        }
    }

    /// A bare name among a block's entries: the section or group that `local
    /// core = section { … }` bound, lowered here rather than where it was
    /// written because here is where its half and its block's install types are
    /// known.
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
                if let Some(section) = self.section(value, half, Some(index)) {
                    self.module.sections.push(ir::SectionItem::Section(section));
                }
            }
            DeferredKind::Group => self.group(value, half, Some(index)),
            // The page goes where the block listed it, like every other entry:
            // the `local` decides nothing about page order either.
            DeferredKind::StartMenu => {
                if let Some(("page", which)) = value.callee_field() {
                    self.page(value, which, half, Some(&name.text));
                }
            }
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
        let earned = earned_name(&name.text, deferred.kind, claim.half).0;
        Some((deferred.value, deferred.kind, earned))
    }

    /// Every bare name among the two blocks' entries, recorded as a claim.
    ///
    /// Syntactic, and deliberately: it reads the shape of `installer { … }` and
    /// `group { … }` without lowering either, because the shape errors belong to
    /// [`Self::installer`] and [`Self::group`] and reporting them from two
    /// places would report them twice.
    fn claim_pass(&mut self) {
        for stmt in self.resolved.block.clone() {
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
                         window needs the dialog it sits in"
                    }
                    Site::Controls => {
                        "a section is listed by `installer {}` or `uninstaller {}`: it is what \
                         gets installed, not what is drawn"
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
            let previous_span = previous.span;
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
                    .note_at("the first one is at", previous_span)
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
        // name once: a second `!define` is a warning it then ships, and a
        // second `Var` is an error.
        let (earned, kind_of_name) = earned_name(&name.text, deferred.0, half);
        let taken = match kind_of_name {
            "global" => self
                .resolved
                .globals
                .iter()
                .any(|global| global.name == earned),
            _ => self.resolved.consts.contains_key(&earned),
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

    /// `onInit(function() … end)`. The leading `.` is emitted, never written,
    /// and so is the `un.` on the uninstaller's.
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
                .note("NSIS calls it, and `Call` has no argument list"),
            );
            return;
        }

        let name = match half {
            Half::Installer => format!(".{which}"),
            Half::Uninstaller => format!("un.{which}"),
        };
        // The global initialisers go in front of whatever the user wrote, so a
        // `.onInit` that reads a global sees its value.
        let block = if half == Half::Installer && which == "onInit" {
            self.on_init[half.index()] = true;
            let mut all = std::mem::take(&mut self.global_inits);
            all.extend(block.iter().cloned());
            all
        } else {
            if which == "onInit" {
                self.on_init[half.index()] = true;
            }
            block.clone()
        };
        // The `languages {}` line goes in front of everything, including the
        // global initialisers: one of them may read `lang.greeting`, and until
        // the dialog has run `$LANGUAGE` is whatever the system said.
        let prelude = match which {
            "onInit" => std::mem::take(&mut self.init_prelude[half.index()]),
            _ => Vec::new(),
        };
        // The one callback with a rule of its own: `SetSilent` is read before
        // any page runs, so `.onInit` is the only body it survives. Every other
        // callback is `Anywhere` — none of them is early enough.
        let place = match which {
            "onInit" => table::Place::OnInit,
            _ => table::Place::Anywhere,
        };
        let body = if prelude.is_empty() {
            self.body(&block, &[], *span, None, Some(half), place)
        } else {
            let (body, _) = self.body_with(*span, Some(half), place, |lowerer| {
                for instruction in prelude {
                    lowerer.emit(instruction);
                }
                lowerer.block(&block);
            });
            body
        };
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
        // The short-and-table pair, the same as `section`'s: a short form for a
        // group with nothing to configure, and a table form whose array part is
        // the name and whose hash part is `expanded` and the sections it holds.
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
        let mut description = None;
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
                // A heading has an index of its own and the tree reports it on
                // hover, so it takes the same field a section does.
                "description" => description = self.constant_arg(value, "description"),
                other => {
                    self.diags.push(
                        Diagnostic::error(
                            Code::UnknownField,
                            name.span,
                            format!("`{other}` is not a `group` option"),
                        )
                        .note(format!("the options are {}", list(GROUP_OPTIONS))),
                    );
                }
            }
        }

        // Before the members, so the heading's text precedes its sections' in
        // the block — the order the tree lists them in, which is the only order
        // a reader of the generated function can check against the source.
        let index = match description {
            Some(text) => Some(self.describe(index, text, half)),
            None => index,
        };

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
                if let Some(section) = self.section(declared, half, Some(index)) {
                    sections.push(section);
                }
                continue;
            }
            if let Some(section) = self.section(value, half, None) {
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

    /// `index` is the define the caller has already earned for it — `Some` when
    /// a `local` was listed by name, `None` for a section written inline. A
    /// `description` turns `None` into a minted one rather than refusing, since
    /// the index is MUI2's business and not the author's ([`Self::describe`]).
    fn section(&mut self, value: &Expr, half: Half, index: Option<String>) -> Option<ir::Section> {
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

        // The short-and-table pair: `section("Core", fn)` when there is nothing
        // to configure, and `section { "Core", required = true, body = fn }`
        // when there is. Two forms and not three — the middle-table
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
        let mut description = None;
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
                // The words the components page shows while the pointer is over
                // this section. Written here rather than on the page, because
                // the page has no way to name a section and MUI2's own macro
                // addresses one by its index.
                "description" => description = self.constant_arg(value, "description"),
                other => {
                    self.diags.push(
                        Diagnostic::error(
                            Code::UnknownField,
                            name.span,
                            format!("`{other}` is not a `section` option"),
                        )
                        .note(format!("the options are {}", list(SECTION_OPTIONS))),
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
            // emitted rather than written — the whole of the uninstaller block
            // at the surface is that this prefix has no spelling.
            name: format!("{}{name}", half.prefix()),
            optional,
            inst_types,
            required,
            size,
            // The name the caller earned by listing a `local`, or the one a
            // `description` minted. A section that is neither addressed nor
            // described gets no third word: the `!define` NSIS would make is one
            // more name in a namespace shared with the author's.
            index_name: match description {
                Some(text) => Some(self.describe(index, text, half)),
                None => index,
            },
            body: self.body(block, &[], *span, None, Some(half), table::Place::Anywhere),
        })
    }

    /// The table form, split into the three parts a declaration is made of: the
    /// one positional parameter that is its name, the named key holding what it
    /// encloses, and the options beside them.
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
    /// This is the install-type binding, and it is a *compile-time* one: the
    /// name a user writes is the name they declared on the block, and the
    /// number NSIS wants never appears in the source. That the numbering exists
    /// in one place is what makes inserting an install type at the front safe —
    /// every section renumbers, and none of them says a number.
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

        let body = self.body(
            block,
            params,
            *span,
            Some(&name.value),
            None,
            table::Place::Anywhere,
        );
        self.module.functions.push(ir::Function {
            name: name.value.clone(),
            body,
        });
    }

    /// One body, one CFG, one register file. Everything about a body is local
    /// to it — the label counter resets and NSIS `Goto` cannot cross the
    /// boundary anyway.
    fn body(
        &mut self,
        block: &Block,
        params: &[Name],
        span: Span,
        owner: Option<&str>,
        half: Option<Half>,
        place: table::Place,
    ) -> Body {
        let signature = owner
            .and_then(|name| self.known.signature(name))
            .cloned()
            .unwrap_or_default();
        let (body, returns) = self.body_with(span, half, place, |lowerer| {
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
        place: table::Place,
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
            lang_strings: &self.lang_strings,
            half,
            place,
            inst_types,
            body: Body::new(span),
            scopes: vec![Vec::new()],
            loops: Vec::new(),
            callback: None,
            returns: Vec::new(),
            span,
            current: Body::ENTRY,
        };
        build(&mut lowerer);
        let finished = lowerer.finish();
        // Whatever the body asked the compiler to write on its behalf. Drained
        // here rather than inside the body because a callback is a sibling of
        // the function that installs it, not a part of it, and this is the one
        // place that holds both.
        let generated = std::mem::take(&mut self.requires.functions);
        self.module.functions.extend(generated);
        finished
    }

    /// Every `return` in one body has to agree on how many values it leaves,
    /// because `Call` has no arity: the callee pushes and the caller pops, and
    /// a disagreement is a stack that unbalances at runtime with NSIS reporting
    /// nothing at all.
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
                    .note_at("the other one is at", *first_span)
                    .note(
                        "`Call` has no arity — the callee pushes and the caller pops — so the \
                         two would unbalance the stack with no diagnostic from NSIS",
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
                         instruction has run",
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
    /// and both are constants a user writes as an ordinary name. So this walks
    /// the same three shapes [`BodyLowerer::simple`] does, and folds only what
    /// is left.
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
/// is build-time, so a use folds rather than reads.
#[derive(Clone, Debug)]
enum Binding {
    Local { slot: Slot, ty: Ty },
    Const(ConstValue),
}

/// Where `break` and `continue()` go. Both are ordinary terminators, which is
/// the whole reason no statement lowerer does label bookkeeping.
struct LoopTargets {
    break_to: BlockId,
    continue_to: BlockId,
}

/// The two ways out of a callback body, as blocks that push before returning.
///
/// `break` reaches `stop` through [`LoopTargets`] like any other loop; `return`
/// reaches `next` through here, because in a walk "I am done with this one" is
/// what a bare `return` means and it is the same thing as falling off the end.
#[derive(Clone, Copy)]
struct CallbackExits {
    protocol: &'static callback::Protocol,
    next: BlockId,
    stop: BlockId,
}

struct BodyLowerer<'a, 'p> {
    diags: &'a mut Diagnostics,
    resolved: &'a Resolved<'p>,
    options: &'a crate::Options,
    known: &'a Inferred,
    learned: &'a mut Inferred,
    globals: &'a mut GlobalTypes,
    /// Headers and `StrFunc` declarations, shared with every other body: the
    /// collect half of the collect-then-emit pass.
    requires: &'a mut Requirements,
    /// Which block listed which section, so that `core.selected` knows the
    /// define it reads and whether this half is the one that has a `core` at all
    /// (claim rule 4).
    claims: &'a BTreeMap<String, Claim>,
    /// The `LangString` names in scope, which is every one `languages {}`
    /// declared: `lang.greeting` is `$(greeting)` and an unknown name is an
    /// error rather than an empty string.
    lang_strings: &'a BTreeSet<String>,
    /// The half this body runs in. `None` for a `func`, which either half may
    /// call: there is no wrong half to name a section from, so rule 4 has
    /// nothing to compare against and does not run.
    half: Option<Half>,
    /// Which NSIS body this is, for the one row that is honoured in only one of
    /// them. Unlike [`Self::half`] there is no `None` case: a `func` is
    /// [`table::Place::Anywhere`] rather than unknown, because a call NSIS
    /// would ignore is refused wherever the compiler cannot prove otherwise.
    place: table::Place,
    /// The install types the block declared, in order — the name → position
    /// binding a `handle.installTypes = { … }` write resolves against, and the
    /// same one `SectionIn` uses at compile time.
    inst_types: Vec<String>,
    body: Body,
    scopes: Vec<Vec<(String, Binding)>>,
    loops: Vec<LoopTargets>,
    /// Set while lowering a body NSIS will call back into, and what it means is
    /// that **`return` is not a return here**. The walk reads a pushed string
    /// to decide whether to carry on, so a path that leaves without pushing one
    /// hands the caller's `Pop` whatever was underneath it — a wrong string in
    /// an unrelated instruction, much later, with nothing to connect the two.
    /// Every exit therefore goes through one of these blocks.
    callback: Option<CallbackExits>,
    /// What each `return` in this body leaves on the stack.
    returns: Vec<(Vec<Ty>, Span)>,
    /// The statement being lowered, stamped onto every instruction it produces.
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

    /// A slot for a named value. There is nothing to fail here any more: twenty
    /// registers is a fact about how many values are live at once, and
    /// [`crate::alloc`] is the only pass that can know that.
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
    /// The attribution is a field rather than a lookup because the line map has
    /// to survive layout, and by then the statement is long gone: the emitter
    /// sees a flat list and the CFG that produced it does not exist any more.
    fn emit(&mut self, instruction: ir::Instruction) {
        let current = self.current;
        let span = self.span;
        self.body.push(current, instruction.at(span));
    }

    /// A plugin call the *compiler* writes: `nsDialogs::Create`, and the
    /// `CreateControl`s under it.
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
    /// the reason neither side needs an `Exch` (program 4).
    fn return_stmt(&mut self, values: &[Expr], span: Span) {
        if let Some(exits) = self.callback {
            self.callback_return(exits, values, span);
            return;
        }
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
        // invisible at the call site, so the *declaration's* arity is what
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
                            ",
                        ),
                    ),
                }
                continue;
            }

            // The slot is claimed *before* the initialiser is walked, so `local
            // sum = 1 + 1` is one `IntOp` into the local rather than an `IntOp`
            // into a temporary and a `StrCpy` after it.
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
            // `docs.text = ""` — a handle's field, which is an instruction and
            // not a register at all. A control's `serial.value = "…"` is the
            // same shape over a window, and a local holding one is not a
            // declaration, so the test is what the base *resolves to* rather
            // than whether it was declared.
            if let Expr::Field { base, name, .. } = target
                && base.name().is_some_and(|base| {
                    self.resolved.deferred.contains_key(base)
                        || matches!(
                            self.lookup(base),
                            Some(Binding::Local {
                                ty: Ty::Handle | Ty::Unknown,
                                ..
                            })
                        )
                })
            {
                self.field_write(base, name, value);
                continue;
            }

            let Expr::Name(name) = target else {
                self.todo(target.span(), "this assignment target");
                continue;
            };

            // `currentInstType = "Minimal"` — a name the compiler owns, whose
            // write is an instruction rather than a register.
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
                        .note("a build-time constant has no register to assign to"),
                    );
                    continue;
                }
                // `$INSTDIR` is a variable, not a constant: `.onInit` reading a
                // prior install location and assigning it is the canonical
                // shape, and it is the only reason a "constant" here has a
                // `writable` column at all.
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
                                 accepts the assignment silently rather than objecting",
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
                    .note("a `Var` is one slot, so a global has one type for its lifetime");
                    for site in sites {
                        diagnostic = diagnostic.note_at("assigned at", site);
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
    /// alternatives, since Lua has no `continue` and `goto` is what 5.4 added
    /// to spell the idiom.
    fn call_statement(&mut self, call: &Expr) {
        if call.callee_name() == Some("continue")
            && let Expr::Call { args, span, .. } = call
        {
            if !args.is_empty() {
                self.diags.push(
                    Diagnostic::error(Code::WrongArity, *span, "`continue()` takes no arguments")
                        .note("it is a jump wearing a call's syntax"),
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
        // `!if`/`!ifdef` need no surface spelling at all.
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

    /// `for x in <iterator>`, where the iterator set is closed.
    ///
    /// Three kinds, and they run in three different places — a reader has to be
    /// able to tell which from the source alone. `glob` walks the build machine
    /// and unrolls, so there is no loop in the output at all. `lines` walks a
    /// file the installer has in front of it, so there is one. And a declared
    /// walker — `fileFunc.locate` and the three like it — walks the target's
    /// disk *inside NSIS*, which puts the body in a function NSIS calls rather
    /// than in a loop at all.
    fn generic_for(&mut self, names: &[Name], iterator: &Expr, block: &Block, span: Span) {
        let Expr::Call { callee, args, .. } = iterator else {
            self.todo(iterator.span(), "this iterator");
            return;
        };
        // `fileFunc.locate(…)` and the rest: a method on a namespace a `local`
        // bound. Checked before the arity rule below, because a walker yields
        // as many values as its protocol has and the "one value" message would
        // be wrong for every one of them.
        if let Some((base, method)) = iterator.callee_field() {
            self.walker_for(names, base, method, args, block, span);
            return;
        }
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
                .note("there are no pairs to unpack: `for k, v` has nothing to iterate over"),
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
    /// golden file needs one.
    fn glob_for(&mut self, name: &Name, args: &[Expr], block: &Block, span: Span) {
        let [pattern] = args else {
            self.diags.push(
                Diagnostic::error(Code::WrongArity, span, "`glob` takes one pattern")
                    .note("it runs on the build machine, so the pattern has to be known there"),
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
                       for a runtime value to arrive in",
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
                     nothing for `assets/*.txt` to mean",
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
    /// header.
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

    /// `for path, name in fileFunc.locate(INSTDIR, "/L=F")` — the target's own
    /// disk, walked by NSIS.
    ///
    /// There is no loop in the output. NSIS calls the script back once per
    /// match, so the body becomes a **function**, and the loop's two exits
    /// become the two strings that function can push: falling off the end
    /// pushes the empty one and `break` pushes `StopLocate`. That inversion is
    /// the whole reason this is not [`Self::lines_for`] with a different
    /// instruction in it.
    ///
    /// The body sees **no enclosing local**. `${Locate}` uses `$0`–`$9` for its
    /// own bookkeeping while the walk runs, so a register holding a section's
    /// local does not survive to the callback — and a language that let one be
    /// named here would be promising something NSIS takes away. Globals do
    /// survive, and are the way out.
    fn walker_for(
        &mut self,
        names: &[Name],
        base: &str,
        method: &Name,
        args: &[Expr],
        block: &Block,
        span: Span,
    ) {
        let Some(entry) = self.walker(base, method, span) else {
            return;
        };
        let Some(protocol) = callback::protocol(&entry.nsis) else {
            return;
        };
        if protocol.shape != callback::Shape::Iterate {
            self.diags.push(
                Diagnostic::error(
                    Code::UnsupportedIterator,
                    span,
                    format!("`{base}.{}` is not a loop", method.text),
                )
                .note(
                    "its callback answers with the line to write, and a loop body has nowhere \
                     to put that — call it with a `function(…)` instead",
                ),
            );
            return;
        }
        if names.len() > protocol.inputs.len() {
            self.diags.push(
                Diagnostic::error(
                    Code::WrongArity,
                    span,
                    format!(
                        "`{base}.{}` yields {} value(s), and {} names are bound",
                        method.text,
                        protocol.inputs.len(),
                        names.len()
                    ),
                )
                .note(format!(
                    "they are {}",
                    protocol
                        .names
                        .iter()
                        .map(|name| format!("`{name}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                )),
            );
            return;
        }

        let Some(mut lowered) = self.walker_arguments(base, method, args, &entry, span) else {
            return;
        };

        let function = self.callback_body(names, protocol, block, span);
        lowered.push(ir::Arg::raw(function));
        self.requires.headers.insert(entry.header.clone());
        self.emit(ir::Instruction::new(format!("${{{}}}", entry.nsis), lowered).atomic());
    }

    /// The declaration behind `base.method`, if it is a callback macro at all.
    ///
    /// Three ways to not be one, and each gets its own sentence: the name is
    /// not a header, the header declares no such method, or the method takes no
    /// function. The last is the one a reader hits by writing `for … in
    /// fileFunc.getSize(…)`, and "that is not a walker" is more use than
    /// "unsupported iterator".
    fn walker(
        &mut self,
        base: &str,
        method: &Name,
        span: Span,
    ) -> Option<crate::declarations::Macro> {
        let namespace = self.resolved.namespaces.get(base).cloned();
        let header = match namespace {
            Some(crate::resolve::Namespace::Header(header)) => header,
            Some(crate::resolve::Namespace::Plugin(_)) => {
                self.diags.push(
                    Diagnostic::error(
                        Code::UnsupportedIterator,
                        span,
                        format!("`{base}` is a plugin, and no plugin walks anything"),
                    )
                    .note("a plugin pushes its values and returns; only a header macro calls back"),
                );
                return None;
            }
            None => {
                self.diags.push(
                    Diagnostic::error(
                        Code::UndefinedName,
                        span,
                        format!("`{base}` is not a header"),
                    )
                    .note("write `local fileFunc = import \"FileFunc\"` and iterate that"),
                );
                return None;
            }
        };
        let entry = self.options.declarations.lookup(&header, &method.text)?;
        if !entry.params.iter().any(|param| param.callback) {
            self.diags.push(
                Diagnostic::error(
                    Code::UnsupportedIterator,
                    span,
                    format!("`{base}.{}` is not a walker", method.text),
                )
                .note(
                    "only a macro declared with a `callback` parameter calls back, and only \
                     those can be iterated",
                ),
            );
            return None;
        }
        // A declaration may name a `callback` macro this compiler has no
        // protocol for, which is exactly what a third-party one would be. The
        // register map is not something a declaration can carry, so the honest
        // answer is that the name is unknown rather than a guess at `$R9`.
        if callback::protocol(&entry.nsis).is_none() {
            self.diags.push(
                Diagnostic::error(
                    Code::NotYetImplemented,
                    span,
                    format!("`${{{}}}`'s callback protocol is not known", entry.nsis),
                )
                .note(format!(
                    "a callback receives its arguments in named registers, which a declaration \
                     cannot describe; the ones with a protocol are `{}`",
                    callback::known()
                )),
            );
            return None;
        }
        Some(entry.clone())
    }

    /// The arguments before the callback, lowered and checked against the
    /// declaration — the same rules [`expr::Lowerer::namespaced`] applies, over
    /// the `params` list with its last entry dropped.
    fn walker_arguments(
        &mut self,
        base: &str,
        method: &Name,
        args: &[Expr],
        entry: &crate::declarations::Macro,
        span: Span,
    ) -> Option<Vec<ir::Arg>> {
        let before: Vec<crate::declarations::Param> = entry
            .params
            .iter()
            .copied()
            .filter(|param| !param.callback)
            .collect();
        if args.len() != before.len() {
            self.diags.push(
                Diagnostic::error(
                    Code::WrongArity,
                    span,
                    format!(
                        "`{base}.{}` takes {} argument(s) before its body, and {} were given",
                        method.text,
                        before.len(),
                        args.len()
                    ),
                )
                .note(format!("it becomes `${{{}}}`", entry.nsis)),
            );
            return None;
        }

        let mut lowered = Vec::with_capacity(entry.params.len());
        for (argument, param) in args.iter().zip(before) {
            let value = self.value(argument)?;
            if param.ty != Ty::Unknown && value.ty != param.ty && value.ty != Ty::Unknown {
                self.diags.push(
                    Diagnostic::error(
                        Code::TypeMismatch,
                        argument.span(),
                        format!(
                            "`{base}.{}` wants a {}, and this is a {}",
                            method.text, param.ty, value.ty
                        ),
                    )
                    .note("types come from the declaration, never from an annotation"),
                );
                return None;
            }
            lowered.push(if param.path {
                value.arg.into_path()
            } else {
                value.arg
            });
        }
        Some(lowered)
    }

    /// The function NSIS calls, built and handed to the module.
    ///
    /// Its shape is the protocol and nothing else: read the inputs, run the
    /// body, push a sentinel. Returns the name to write in the macro's last
    /// argument — the macro takes the *name* and does its own
    /// `GetFunctionAddress`, which is the one part of this that costs nothing.
    fn callback_body(
        &mut self,
        names: &[Name],
        protocol: &'static callback::Protocol,
        block: &Block,
        span: Span,
    ) -> String {
        let name = format!(
            "{}{}callback_{}_{}",
            cfg::LABEL_PREFIX,
            self.half.map(Half::prefix).unwrap_or_default(),
            protocol.nsis.to_lowercase(),
            self.requires.generated
        );
        self.requires.generated += 1;

        let body = self.nested_body(span, |lowerer| {
            let next = lowerer.fresh("continue");
            let stop = lowerer.fresh("stop");
            lowerer.callback = Some(CallbackExits {
                protocol,
                next,
                stop,
            });

            // Every input register is read **before** any of them is written.
            // The slots below are virtual until `alloc` colours them, and it may
            // well colour the first one onto the register the second one is
            // still sitting in — so the reads go through the stack, where the
            // order is the program's rather than the allocator's.
            let bound: Vec<&u8> = protocol.inputs.iter().take(names.len()).collect();
            for register in bound.iter().rev() {
                lowerer.emit(ir::Instruction::new(
                    "Push",
                    vec![ir::Arg::slot(Slot::Reg(**register))],
                ));
            }
            let mut scope = Vec::new();
            for (index, name) in names.iter().enumerate() {
                let slot = lowerer.claim_local(name.span);
                lowerer.emit(ir::Instruction::new(
                    "Pop",
                    vec![ir::Arg::dest(slot.clone())],
                ));
                scope.push((
                    name.text.clone(),
                    Binding::Local {
                        slot,
                        ty: match protocol.types[index] {
                            callback::Kind::Text => Ty::Str,
                            callback::Kind::Count => Ty::nonneg(),
                        },
                    },
                ));
            }

            lowerer.scopes.push(scope);
            lowerer.loops.push(LoopTargets {
                break_to: stop,
                continue_to: next,
            });
            lowerer.block(block);
            lowerer.loops.pop();
            lowerer.scopes.pop();

            // Falling off the end and `break`, as the two strings the walk
            // reads. Nothing else may leave this function: a bare `return`
            // would skip the push and the caller's `Pop` would take whatever
            // was underneath, which `return_stmt` refuses for that reason.
            lowerer.terminate(Terminator::Jump(next), next);
            lowerer.emit(ir::Instruction::new("Push", vec![ir::Arg::str("")]));
            lowerer.terminate(Terminator::Return, stop);
            lowerer.emit(ir::Instruction::new(
                "Push",
                vec![ir::Arg::str(protocol.stop)],
            ));
        });

        self.requires.functions.push(ir::Function {
            name: name.clone(),
            body,
        });
        name
    }

    /// `return` inside a body NSIS calls back into.
    ///
    /// It is not a return. The walk decides whether to continue by reading a
    /// string this function pushes, so every exit has to go through a block
    /// that pushes one — which is what the two [`CallbackExits`] blocks are.
    ///
    /// In a walker, a bare `return` means "done with this one", which is what
    /// falling off the end already means, so it jumps to the same place.
    fn callback_return(&mut self, exits: CallbackExits, values: &[Expr], span: Span) {
        if exits.protocol.shape == callback::Shape::Iterate {
            if !values.is_empty() {
                self.diags.push(
                    Diagnostic::error(
                        Code::ReturnArity,
                        span,
                        "a walker's body returns nothing".to_string(),
                    )
                    .note(
                        "it answers by running to the end or by `break`; there is no third \
                         answer for a value to carry",
                    ),
                );
                return;
            }
            let after = self.fresh("after_return");
            self.terminate(Terminator::Jump(exits.next), after);
            return;
        }
        self.rewrite_return(exits, values, span);
    }

    /// `return line` / `return skip` / `return stop`, inside `lineFind`.
    ///
    /// Three answers where a loop has two, which is the whole reason this macro
    /// is not a loop. The line to write goes back through the register it
    /// arrived in — `$R9`, read again by `${LineFind}` after the call — and the
    /// sentinel says whether to write it at all.
    ///
    /// `skip` and `stop` are recognised as **bare words in this position** and
    /// nowhere else, so they cost the program no name: anything with a value,
    /// including a local called `stop`, is still the line to write everywhere
    /// but here.
    fn rewrite_return(&mut self, exits: CallbackExits, values: &[Expr], span: Span) {
        let word = match values {
            [Expr::Name(name)] => Some(name.text.as_str()),
            _ => None,
        };
        match word {
            Some("stop") => {
                let after = self.fresh("after_return");
                self.terminate(Terminator::Jump(exits.stop), after);
                return;
            }
            Some("skip") => {
                let Some(skip) = exits.protocol.skip else {
                    self.diags.push(
                        Diagnostic::error(
                            Code::UndefinedName,
                            span,
                            format!("`${{{}}}` has no line to skip", exits.protocol.nsis),
                        )
                        .note("`skip` is `lineFind`'s alone: it is the one that writes a file"),
                    );
                    return;
                };
                self.emit(ir::Instruction::new("Push", vec![ir::Arg::str(skip)]));
                let after = self.fresh("after_return");
                self.terminate(Terminator::Return, after);
                return;
            }
            _ => {}
        }

        // A value: the line that gets written. Into `$R9` last of all, because
        // the slot holding it may well *be* `$R9` after allocation — a copy
        // onto itself is a line NSIS is happy to run and this pass need not
        // know about.
        if let [value] = values {
            let Some(typed) = self.value(value) else {
                return;
            };
            let out = Slot::Reg(exits.protocol.inputs[0]);
            self.emit(ir::Instruction::new(
                "StrCpy",
                vec![ir::Arg::dest(out), typed.arg],
            ));
        } else if !values.is_empty() {
            self.diags.push(
                Diagnostic::error(
                    Code::ReturnArity,
                    span,
                    format!(
                        "this returns {} values, and a rewrite writes one line",
                        values.len()
                    ),
                )
                .note("return the line, or `skip`, or `stop`"),
            );
            return;
        }

        self.emit(ir::Instruction::new("Push", vec![ir::Arg::str("")]));
        let after = self.fresh("after_return");
        self.terminate(Terminator::Return, after);
    }

    /// A second body, lowered with this one's borrows.
    ///
    /// The scopes start empty on purpose — see [`Self::walker_for`] — and the
    /// returns are dropped, because a callback's arity is the protocol's and
    /// not something the body gets a say in.
    fn nested_body(&mut self, span: Span, build: impl FnOnce(&mut BodyLowerer)) -> Body {
        let mut lowerer = BodyLowerer {
            diags: self.diags,
            resolved: self.resolved,
            options: self.options,
            known: self.known,
            learned: self.learned,
            globals: self.globals,
            requires: self.requires,
            claims: self.claims,
            lang_strings: self.lang_strings,
            half: self.half,
            place: self.place,
            inst_types: self.inst_types.clone(),
            body: Body::new(span),
            scopes: vec![Vec::new()],
            loops: Vec::new(),
            callback: None,
            returns: Vec::new(),
            span,
            current: Body::ENTRY,
        };
        build(&mut lowerer);
        lowerer.finish().0
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

    /// The twin of [`Lowerer::bad_value`], for the fields a *body* writes: a
    /// control's `colors` and `font` are tables checked here rather than at the
    /// top level, since the write is a statement.
    pub(super) fn bad_value(&mut self, span: Span, what: &str, wanted: &str, note: &str) {
        self.diags.push(
            Diagnostic::error(
                Code::BadFieldValue,
                span,
                format!("`{what}` wants {wanted}"),
            )
            .note(note),
        );
    }

    fn undefined(&mut self, name: &Name) {
        // An NSIS instruction with a Lua spelling is not an unknown name: the
        // compiler knows exactly what it is, and the generic error would tell
        // an NSIS user that it had never heard of the instruction they use
        // most. Checked first, because `strCmp` is also within one case-fold of
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

        // `param` resolves to nothing on purpose: it is read by
        // [`crate::resolve`] where a `<const>` is declared and is gone by the
        // time any body is walked. Without this the one construct whose whole
        // job is to be a declaration gets reported as a missing function.
        if name.text == crate::resolve::PARAM {
            self.diags.push(
                Diagnostic::error(
                    Code::ParamForm,
                    name.span,
                    format!(
                        "`{}` declares a build parameter, so it stands alone",
                        name.text
                    ),
                )
                .note(format!(
                    "write `local {0} <const> = {1}(\"{0}\", …)` at the top level, and read the \
                     `<const>` here",
                    "VERSION",
                    crate::resolve::PARAM
                ))
                .note(
                    "the set of parameter names has to be known before anything folds, or `-D` \
                     has nothing to be checked against",
                ),
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
            // mistake with an obvious fix rather than an unknown name.
            Some(suggestion) => diagnostic.note(format!("did you mean `{suggestion}`?")),
            None => diagnostic.note(
                "resolution is order-free, so this means nowhere in the file — not merely \
                 not yet",
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
/// filename with `*` and `?` in it, which is what the compile-time surface
/// exposes and what the five programs use. Anything larger is a shell's job,
/// and `BUILD.system` is where a shell belongs.
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
        .note("it is scheduled rather than missing: `installua coverage` counts it"),
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

/// The value inside a [`table::Setting::Flags`] table, for a reader that wants
/// what the field *holds* rather than how the line is written.
///
/// Shape-based rather than row-based on purpose: the caller already knows which
/// field it is looking at, and the only table with a positional entry an
/// attribute can hold is this one. `None` for anything else, including a flag
/// table written wrong — the row's own lowering says so, and a second
/// diagnostic about a value nobody could read is noise on top of it.
fn flagged_value(value: &Expr) -> Option<&Expr> {
    let Expr::Table { fields, .. } = value else {
        return None;
    };
    let mut positional = fields.iter().filter_map(|field| match field {
        TableField::Positional { value } => Some(value),
        TableField::Named { .. } => None,
    });
    let only = positional.next()?;
    positional.next().is_none().then_some(only)
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

/// What a [`table::Setting`] is called where a diagnostic names it *beside*
/// something else, which is [`table::Setting::Or`]'s message and nowhere else.
///
/// Only the scalars are spelled out, because only a scalar can stand opposite a
/// list of keywords: a table or an `Each` is more than one value and there is no
/// alternation between "the word `ALL`" and three positions. The fallback is
/// there so adding a shape does not have to touch this, and it says the true
/// thing rather than a wrong specific one.
fn noun(holds: table::Setting) -> &'static str {
    match holds {
        table::Setting::Int => "an `int`",
        table::Setting::Str { .. } => "a `string`",
        table::Setting::Bool { .. } => "a `bool`",
        _ => "a value",
    }
}

fn list(names: &[&str]) -> String {
    names
        .iter()
        .map(|name| format!("`{name}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The two lowerers, taught to read a shared field shape (`fields::Fields`).
///
/// Four lines apiece and no more: what they have in common is somewhere to
/// report and a way to fold a constant, and everything else about them differs.
impl Fields for Lowerer<'_, '_> {
    fn diags(&mut self) -> &mut Diagnostics {
        self.diags
    }

    fn constant_value(&self, expr: &Expr) -> Option<ConstValue> {
        self.constant(expr)
    }
}

impl Fields for BodyLowerer<'_, '_> {
    fn diags(&mut self) -> &mut Diagnostics {
        self.diags
    }

    fn constant_value(&self, expr: &Expr) -> Option<ConstValue> {
        self.constant(expr)
    }
}
