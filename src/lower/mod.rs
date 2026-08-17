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

mod expr;
mod sig;

use std::collections::{BTreeMap, BTreeSet};

use crate::alloc;
use crate::ast::*;
use crate::callgraph;
use crate::cfg::{self, BlockId, Body, Terminator};
use crate::diag::{Code, Diagnostic, Diagnostics, Span};
use crate::ir;
use crate::regs::Slot;
use crate::resolve::{ConstValue, Resolved};
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
/// The overlap with [`V1_INSTALLER_FIELDS`] is not one. `caption`, `icon`,
/// `installDir` and `license` are script-wide NSIS commands that `installer {}`
/// also accepts, so they belong to both blocks; `pages` and `text` have no row
/// at all and are the two names that guard rejects.
fn attribute_names() -> Vec<&'static str> {
    table::table()
        .iter()
        .filter(|entry| matches!(entry.class, table::Class::Attribute(_)))
        .filter_map(|entry| entry.installua)
        .filter(|field| !field.contains('.'))
        .collect()
}

/// The frozen v1 `installer {}` / `uninstaller {}` field surface.
const V1_INSTALLER_FIELDS: &[&str] = &["installDir", "icon", "license", "pages", "text", "caption"];

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

/// One MUI2 page. The `MUI_*` surface is roughly seventy settings and this is
/// the subset the five programs reach (PLAN §0); the rest is overlay data.
struct Page {
    installua: &'static str,
    nsis: &'static str,
    /// Which halves MUI2 defines a macro for. `Confirm` exists only as
    /// `MUI_UNPAGE_CONFIRM`, and there is no `MUI_PAGE_CONFIRM` to fall back
    /// on — so this is a fact about MUI2 rather than a policy of ours.
    halves: [bool; 2],
}

const fn page(installua: &'static str, nsis: &'static str) -> Page {
    Page {
        installua,
        nsis,
        halves: [true, true],
    }
}

const V1_PAGES: &[Page] = &[
    page("Welcome", "WELCOME"),
    page("License", "LICENSE"),
    page("Components", "COMPONENTS"),
    page("Directory", "DIRECTORY"),
    page("InstFiles", "INSTFILES"),
    page("Finish", "FINISH"),
    Page {
        installua: "Confirm",
        nsis: "CONFIRM",
        halves: [false, true],
    },
];

impl Page {
    fn has(&self, half: Half) -> bool {
        self.halves[half as usize]
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

impl Lowerer<'_, '_> {
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
                    value: ir::Arg::str(value.value.text()),
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

        for stmt in &program.block {
            self.top_level(stmt);
        }

        // Nothing declared an `.onInit`, and there are globals to initialise:
        // the callback exists to hold them.
        if !self.global_inits.is_empty() && !self.on_init {
            let inits = std::mem::take(&mut self.global_inits);
            let span = inits.first().map(Stmt::span).unwrap_or_default();
            let body = self.body(&inits, &[], span, None);
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
    /// Three passes rather than one, because a field's meaning can depend on
    /// another field written below it: `license` is an argument to the
    /// `License` page, and a table has no order for the user to get right (§12).
    fn installer(&mut self, fields: &[TableField], half: Half) {
        let mut license = None;
        for field in fields {
            let TableField::Named { name, value } = field else {
                continue;
            };
            match name.text.as_str() {
                "installDir" if half == Half::Installer => {
                    self.string_attribute("InstallDir", "installDir", value, true);
                }
                "icon" => {
                    let define = match half {
                        Half::Installer => "MUI_ICON",
                        Half::Uninstaller => "MUI_UNICON",
                    };
                    if let Some(arg) = self.constant_arg(value, "icon") {
                        self.module.mui_defines.push(ir::Define {
                            name: define.to_string(),
                            value: arg.into_path(),
                        });
                    }
                }
                "license" => license = self.constant_arg(value, "license").map(ir::Arg::into_path),
                "pages" => {}
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
            if let TableField::Named { name, value } = field
                && name.text == "pages"
            {
                self.pages(value, half, license.as_ref());
            }
        }

        for field in fields {
            let TableField::Positional { value } = field else {
                continue;
            };
            self.body_entry(value, half);
        }
    }

    /// `pages = { "Welcome", "License", … }`, in the order written: page order
    /// is what the user sees, so it is the one list in the output that is never
    /// sorted.
    fn pages(&mut self, value: &Expr, half: Half, license: Option<&ir::Arg>) {
        let Expr::Table { fields, .. } = value else {
            self.bad_value(
                value.span(),
                "pages",
                "a list",
                "write `pages = { \"Welcome\", \"Directory\", \"InstFiles\" }`",
            );
            return;
        };

        for field in fields {
            let TableField::Positional { value } = field else {
                self.todo(value.span(), "a named entry in `pages`");
                continue;
            };
            let Some(name) = self.constant_string(value, "pages") else {
                continue;
            };
            let Some(page) = V1_PAGES.iter().find(|page| page.installua == name) else {
                self.diags.push(
                    Diagnostic::error(
                        Code::UnknownField,
                        value.span(),
                        format!("`{name}` is not a page"),
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
                continue;
            };
            if !page.has(half) {
                self.diags.push(
                    Diagnostic::error(
                        Code::UnknownField,
                        value.span(),
                        format!("there is no {half} `{name}` page"),
                    )
                    .note(format!(
                        "MUI2 defines no `{}{}`",
                        half.page_prefix(),
                        page.nsis
                    )),
                );
                continue;
            }

            let mut args = Vec::new();
            if page.installua == "License" {
                match license {
                    Some(arg) => args.push(arg.clone()),
                    None => {
                        self.diags.push(
                            Diagnostic::error(
                                Code::MissingAttribute,
                                value.span(),
                                "a `License` page needs a `license`",
                            )
                            .note("write `license = \"LICENSE.txt\"` beside `pages`")
                            .note("`MUI_PAGE_LICENSE` takes the file as its argument"),
                        );
                        continue;
                    }
                }
            }

            let macro_name = format!("{}{}", half.page_prefix(), page.nsis);
            let mut all = vec![ir::Arg::raw(macro_name)];
            all.extend(args);
            let instruction = ir::Instruction::new("!insertmacro", all);
            match half {
                Half::Installer => self.module.pages.push(instruction),
                Half::Uninstaller => self.module.unpages.push(instruction),
            }
            self.mui = true;
        }
    }

    /// A positional entry in `installer {}`: a `section` or a callback.
    fn body_entry(&mut self, value: &Expr, half: Half) {
        let Some(name) = value.callee_name() else {
            self.todo(value.span(), "this entry");
            return;
        };
        match name {
            "section" => {
                if let Some(section) = self.section(value, half) {
                    self.module.sections.push(ir::SectionItem::Section(section));
                }
            }
            "onInit" => self.callback(value, half, "onInit"),
            other => self.todo(value.span(), &format!("`{other}` here")),
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
        let body = self.body(&block, &[], *span, None);
        self.module.functions.push(ir::Function { name, body });
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

        // `section(name, body)` and `section(name, { optional = true }, body)`.
        let (name, options, body) = match args.as_slice() {
            [name, body @ Expr::Function { .. }] => (name, None, body),
            [
                name,
                Expr::Table { fields, .. },
                body @ Expr::Function { .. },
            ] => (name, Some(fields), body),
            _ => {
                self.todo(value.span(), "this `section` form");
                return None;
            }
        };
        let Expr::Function { block, span, .. } = body else {
            unreachable!("matched above")
        };

        let name = self.constant_string(name, "section")?;
        let mut optional = false;
        for field in options.into_iter().flatten() {
            let TableField::Named { name, value } = field else {
                self.todo(value.span(), "a positional entry in a `section`'s options");
                continue;
            };
            match name.text.as_str() {
                "optional" => match self.constant(value) {
                    Some(ConstValue::Bool(flag)) => optional = flag,
                    _ => self.bad_value(
                        value.span(),
                        "optional",
                        "a `bool`",
                        "it becomes `Section /o`, which starts unselected in the components tree",
                    ),
                },
                other => {
                    self.diags.push(
                        Diagnostic::error(
                            Code::UnknownField,
                            name.span,
                            format!("`{other}` is not a `section` option"),
                        )
                        .note("the options are `optional`"),
                    );
                }
            }
        }

        Some(ir::Section {
            // `un.` is how NSIS marks a section as the uninstaller's, and it is
            // emitted rather than written — the whole of §15.3 at the surface
            // is that this prefix has no spelling.
            name: format!("{}{name}", half.prefix()),
            optional,
            body: self.body(block, &[], *span, None),
        })
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

        let body = self.body(block, params, *span, Some(&name.value));
        self.module.functions.push(ir::Function {
            name: name.value.clone(),
            body,
        });
    }

    /// One body, one CFG, one register file. Everything about a body is local
    /// to it — the label counter resets (§15.25) and NSIS `Goto` cannot cross
    /// the boundary anyway (§8).
    fn body(&mut self, block: &Block, params: &[Name], span: Span, owner: Option<&str>) -> Body {
        let signature = owner
            .and_then(|name| self.known.signature(name))
            .cloned()
            .unwrap_or_default();

        let mut lowerer = BodyLowerer {
            diags: self.diags,
            resolved: self.resolved,
            options: self.options,
            known: self.known,
            learned: &mut self.learned,
            globals: &mut self.globals,
            requires: &mut self.requires,
            body: Body::new(span),
            scopes: vec![Vec::new()],
            loops: Vec::new(),
            returns: Vec::new(),
            span,
            current: Body::ENTRY,
        };
        lowerer.parameters(params, &signature);
        lowerer.block(block);
        let (body, returns) = lowerer.finish();
        self.returns(owner, &returns);
        body
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
            let Expr::Name(name) = target else {
                self.todo(target.span(), "this assignment target");
                continue;
            };

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
