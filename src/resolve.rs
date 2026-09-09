//! Order-free name resolution.
//!
//! Every top-level name is resolved before any body is lowered, so a section on
//! line 3 can call a `func` declared on line 300 and read a `<const>` defined
//! below it. That is possible only because Installua **compiles** rather than
//! transliterates: there is no build-time execution for an ordering rule to be
//! about, so the order things appear in the source is decoupled from the order
//! they appear in the output.
//!
//! It is worth being deliberate that this diverges from Lua, which does not
//! hoist — `function greet() end` is sugar for an assignment executed in order,
//! and calling `greet()` above it is a runtime error in real Lua. Order-free is
//! the honest model for a compiled language and it costs one row in the
//! "Lua-shaped, not Lua" table.
//!
//! NSIS itself is inconsistent about this, which is why hoisting is the
//! compiler's job rather than the user's: `Function`/`Call` resolves late,
//! `Var` and `!include` are hard errors when used early, and a mis-ordered
//! `!define` is a *warning* that silently ships the wrong string.

use std::collections::{BTreeMap, HashSet};

use crate::ast::*;
use crate::builtins;
use crate::diag::{Code, Diagnostic, Diagnostics, Span};
use crate::lower::control::{self, Control};
use crate::types::Ty;

/// A compile-time value. These never reach a register: `<const>` is build-time,
/// so a use folds rather than reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConstValue {
    Int(i64),
    Str(String),
    Bool(bool),
}

impl ConstValue {
    pub fn ty(&self) -> Ty {
        match self {
            // A literal's sign is known, and that is the cheapest place the
            // lattice ever learns it.
            ConstValue::Int(value) if *value >= 0 => Ty::nonneg(),
            ConstValue::Int(_) => Ty::int(),
            ConstValue::Str(_) => Ty::Str,
            ConstValue::Bool(_) => Ty::Bool,
        }
    }

    /// The text a folded value contributes to an argument.
    pub fn text(&self) -> String {
        match self {
            ConstValue::Int(value) => value.to_string(),
            ConstValue::Str(value) => value.clone(),
            ConstValue::Bool(true) => crate::cfg::TRUE.to_string(),
            ConstValue::Bool(false) => crate::cfg::FALSE.to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Const {
    pub value: ConstValue,
    pub span: Span,
}

/// What `local x = import "FileFunc"` and `local y = plugin "nsExec"` bind.
///
/// Neither is a value: there is nothing at run time for `fileFunc` to be, and
/// `fileFunc.getSize(…)` is one macro expansion rather than a field access
/// followed by a call. Binding it to a name is what makes the namespace
/// *visible* — three namespaces exist and NSIS enforces the boundary between
/// them, so the name a user chooses is how they tell which is which.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Namespace {
    /// A `!include`d header: its macros are `${Name}`.
    Header(String),
    /// A plugin: its methods are `Plugin::Method`, and it clobbers everything.
    Plugin(String),
}

impl Namespace {
    pub fn name(&self) -> &str {
        match self {
            Namespace::Header(name) | Namespace::Plugin(name) => name,
        }
    }
}

/// What `local core = section { … }`, `local tools = group { … }` and
/// `local serial = text { … }` bind.
///
/// Not a value either, for the same reason a namespace is not: there is nothing
/// at run time for `core` to be. NSIS spells a section as an *index*, and the
/// local is how the author addresses one without ever saying the number — the
/// section binding, and the reason the set of sections is never enumerated by
/// the compiler.
///
/// The call is held rather than lowered, because lowering it needs the construct
/// that lists it: a section's half and its block's install types, a control's
/// dialog. Neither is in scope where the `local` is written (ruling 2), and a
/// control's is stronger still — `nsDialogs::CreateControl` only means anything
/// between a `Create` and a `Show`.
#[derive(Clone, Debug)]
pub struct Deferred<'a> {
    pub kind: DeferredKind,
    pub value: &'a Expr,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeferredKind {
    Section,
    Group,
    /// `local menu = page.startMenu { … }` — the one page that is bound to a
    /// local, because `MUI_PAGE_STARTMENU` takes an id and a variable and both
    /// of them are the compiler's to mint. The local is what a shortcut-writing
    /// section then addresses the chosen folder through.
    StartMenu,
    /// One of the kinds in [`crate::lower::control::CONTROLS`], carried rather
    /// than looked up again: the name is what decided this is a declaration at
    /// all, so the row it matched is already in hand.
    Control(&'static Control),
}

impl DeferredKind {
    pub fn word(self) -> &'static str {
        match self {
            DeferredKind::Section => "section",
            DeferredKind::Group => "group",
            DeferredKind::StartMenu => "start menu page",
            DeferredKind::Control(control) => control.installua,
        }
    }

    /// Whether this is a control, which is the question every claim rule asks:
    /// a control is listed by a page and the other two by a block.
    pub fn is_control(self) -> bool {
        matches!(self, DeferredKind::Control(_))
    }
}

/// A `func("name", function(…) … end)` declaration.
#[derive(Clone, Debug)]
pub struct Func<'a> {
    pub name: String,
    pub params: &'a [Name],
    pub block: &'a Block,
    pub span: Span,
}

/// A global, declared by assigning to it.
#[derive(Clone, Debug)]
pub struct Global {
    pub name: String,
    /// The first assignment seen. Every assignment must agree on the type, and
    /// a conflict names all of them rather than privileging this one.
    pub span: Span,
}

/// A build parameter: a `<const>` whose value the invocation may set.
///
/// The declaration is what makes `-D` checkable, which is the whole of why
/// parameters are declared in the source rather than listed in
/// `installua.toml`. NSIS's own idiom is `!ifndef VERSION / !define VERSION
/// "1.4.2" / !endif`, and its failure mode is that a misspelt `-DVERSOIN`
/// defines a second thing nobody reads: the build succeeds, the default ships,
/// and nothing says so. Here the set of names is known, so a `-D` outside it is
/// an error.
#[derive(Clone, Debug)]
pub struct Param {
    /// The `<const>` it was bound to, which is also the `!define` it becomes.
    pub bound: String,
    /// Where the declaration is, for the diagnostic an ill-typed `-D` raises:
    /// the flag has no span, and the default beside the name is what says what
    /// type the flag had to be.
    pub span: Span,
    /// The value with the override applied, or the default when there was
    /// none. Kept beside the [`Const`] rather than only inside it so that
    /// `installua stubs` and a future `--list-params` have something to read.
    pub value: ConstValue,
}

#[derive(Debug, Default)]
pub struct Resolved<'a> {
    /// The top level the rest of the compiler sees: the source's own statements
    /// with every build-time `if` replaced by the branch it took.
    ///
    /// Every later pass reads this rather than `program.block`, which is what
    /// makes a top-level `if` cost the emitter nothing — by the time anything is
    /// bucketed the conditional no longer exists, so there is nothing left to
    /// order. See [`select`].
    pub block: Vec<&'a Stmt>,
    pub consts: BTreeMap<String, Const>,
    /// Build parameters by the name `-D` sets them under — which is the string
    /// in the `param(…)` call, not the `<const>` it was bound to. The two are
    /// usually spelled the same and are not required to be: the string is an
    /// interface to the outside and the local is the program's own.
    pub params: BTreeMap<String, Param>,
    /// Top-level `<const>` names in **selected-source order**, because each
    /// becomes a `!define` and the preprocessor is strictly sequential. The map
    /// is alphabetical and the output is not. Rebuilt from [`Resolved::block`]
    /// once the last branch has been taken, so a `<const>` inside a build-time
    /// `if` lands where the `if` was written rather than where it was folded.
    pub const_order: Vec<String>,
    /// Header and plugin namespaces, by the local name they were bound to.
    pub namespaces: BTreeMap<String, Namespace>,
    /// Sections and groups waiting for the block that lists them, by the local
    /// name they were bound to.
    pub deferred: BTreeMap<String, Deferred<'a>>,
    /// Those names in source order, so that "never claimed" is reported where
    /// it was written rather than alphabetically.
    pub deferred_order: Vec<String>,
    pub functions: BTreeMap<String, Func<'a>>,
    /// In first-seen order, because `Var` declarations are emitted in it and
    /// the goldens are diffed.
    pub globals: Vec<Global>,
}

impl Resolved<'_> {
    pub fn global(&self, name: &str) -> bool {
        self.globals.iter().any(|global| global.name == name)
    }
}

pub fn resolve<'a>(
    program: &'a Program,
    options: &crate::Options,
    diags: &mut Diagnostics,
) -> Resolved<'a> {
    let mut resolved = Resolved::default();
    // Consts first, and not for tidiness: folding them is what decides which
    // branch of each top-level `if` is part of the program, and until that is
    // settled there is no top level for the other two passes to walk.
    consts(program, &mut resolved, options, diags);
    unknown_params(&resolved, options, diags);
    top_level(&mut resolved, diags);
    globals(&mut resolved);
    resolved
}

/// The name `param(…)` is spelled with. Not in [`crate::builtins`]: there is no
/// instruction behind it and nothing to lower — a parameter is gone by the time
/// any body is walked, exactly as a `<const>` is.
pub const PARAM: &str = "param";

/// Every `-D` that named nothing, reported once the declarations are all in.
///
/// [`Span::default`] because the flag genuinely has no source position — the
/// same shape [`crate::lower::check_required`] uses for the other diagnostic
/// that is about the program as a whole rather than a line of it.
fn unknown_params(resolved: &Resolved, options: &crate::Options, diags: &mut Diagnostics) {
    for name in options.params.keys() {
        if resolved.params.contains_key(name) {
            continue;
        }
        let mut diagnostic = Diagnostic::error(
            Code::UnknownParam,
            Span::default(),
            format!("`-D {name}` sets a parameter this program does not declare"),
        );
        diagnostic = match resolved.params.keys().next() {
            Some(_) => diagnostic.note(format!(
                "it declares {}",
                resolved
                    .params
                    .keys()
                    .map(|declared| format!("`{declared}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
            None => diagnostic.note(format!(
                "it declares none — write `local {name} <const> = {PARAM}(\"{name}\", …)` at the \
                 top level"
            )),
        };
        diags.push(diagnostic.note(
            "an ignored `-D` is the `!ifndef` failure this replaces: the build succeeds with the \
             default and nothing says so",
        ));
    }
}

/// One `<const>` waiting for its value to fold.
///
/// `param` is carried alongside rather than resolved first, because the
/// override is applied at the moment the default folds — which is the only
/// moment the default has a *type* for the command line's text to be read as.
struct Pending<'a> {
    name: &'a Name,
    /// The initialiser, or for a parameter its default.
    value: &'a Expr,
    /// The name `-D` sets this under, when it is a parameter.
    param: Option<String>,
}

/// What a top-level `<const>`'s initialiser is.
enum Initialiser<'a> {
    /// An ordinary one: fold the expression and that is the value.
    Value(&'a Expr),
    /// `param(name, default)`: fold the default, then let `-D name` replace it.
    Param {
        name: String,
        span: Span,
        default: &'a Expr,
    },
    /// `param(name)`: no default, so `-D name` is the only value there is and a
    /// build without it stops.
    ///
    /// This is `!ifndef NAME` / `!error` as a declaration. The guard form is a
    /// line somebody has to remember to paste; the declaration is checked for
    /// every build because it *is* the parameter, and it says so in the one
    /// place a reader looks for what the build takes.
    Required { name: String, span: Span },
    /// A `param(…)` that does not hold together, already reported.
    Broken,
}

/// Reads an initialiser, reporting a malformed `param(…)` where it is written.
///
/// **Only the whole initialiser.** `param("V", "1") .. "-beta"` is rejected
/// rather than folded, because a parameter is a declaration and a declaration
/// has to be readable without evaluating anything around it — that is what lets
/// `-D` be checked against a known set of names before a single body is walked.
/// Composition is not lost, only moved one line: the derived value is an
/// ordinary `<const>` over the parameter's, and resolution is order-free.
fn initialiser<'a>(value: &'a Expr, diags: &mut Diagnostics) -> Initialiser<'a> {
    let Expr::Call { callee, args, span } = value else {
        return buried(value, diags);
    };
    if callee.name() != Some(PARAM) {
        return buried(value, diags);
    }

    match args.as_slice() {
        [Expr::Str(name), default] => Initialiser::Param {
            name: name.value.clone(),
            span: *span,
            default,
        },
        // No default: the parameter is required, and its type is `string`. The
        // default is what declares a type — `param("PORT", 8080)` is why `-D
        // PORT=abc` is an error — so a parameter without one has nothing to
        // read the flag's text as but text.
        [Expr::Str(name)] => Initialiser::Required {
            name: name.value.clone(),
            span: *span,
        },
        _ => {
            diags.push(
                Diagnostic::error(
                    Code::ParamForm,
                    *span,
                    format!("`{PARAM}` takes a name, and a default unless the build requires one"),
                )
                .note(format!(
                    "write `local X <const> = {PARAM}(\"X\", \"1.4.2\")`, or \
                     `{PARAM}(\"X\")` to make `-D X=…` required"
                ))
                .note(
                    "the name has to be a literal, since it is what `-D` is checked against \
                     before anything is folded",
                ),
            );
            Initialiser::Broken
        }
    }
}

/// An initialiser that is *not* a `param(…)` — unless one is hiding inside it,
/// in which case that is the mistake and saying "not a build-time constant"
/// would point at the wrong half of the line.
fn buried<'a>(value: &'a Expr, diags: &mut Diagnostics) -> Initialiser<'a> {
    let Some(span) = mentions_param(value) else {
        return Initialiser::Value(value);
    };
    diags.push(
        Diagnostic::error(
            Code::ParamForm,
            span,
            format!("`{PARAM}` declares a build parameter, so it stands alone"),
        )
        .note("split it: `local RAW <const> = param(…)` and then build this value from `RAW`")
        .note(
            "a declaration has to be readable before anything folds, or `-D` has nothing to be \
             checked against",
        ),
    );
    Initialiser::Broken
}

/// Where a `param(…)` call appears inside `expr`, if one does.
///
/// Only the shapes a `<const>` initialiser can be: a table field or a function
/// body is not one, and a `param` in either is a *body* use, which
/// [`crate::lower`] reports where it is written.
fn mentions_param(expr: &Expr) -> Option<Span> {
    match expr {
        Expr::Call { callee, args, span } => {
            if callee.name() == Some(PARAM) {
                return Some(*span);
            }
            args.iter().find_map(mentions_param)
        }
        Expr::Binary { lhs, rhs, .. } => mentions_param(lhs).or_else(|| mentions_param(rhs)),
        Expr::Unary { operand, .. } => mentions_param(operand),
        _ => None,
    }
}

/// `-D NAME=text`, as the type the default declared.
///
/// The default is the type declaration — there is nowhere else for one to
/// live — so `param("PORT", 8080)` makes `-D PORT=abc` an error rather than a
/// string arriving at an `IntOp`, and `param("QUIET", false)` accepts exactly
/// the two words that fold to a boolean.
fn coerce(text: &str, default: &ConstValue) -> Option<ConstValue> {
    match default {
        ConstValue::Int(_) => text.parse().ok().map(ConstValue::Int),
        ConstValue::Bool(_) => match text {
            "true" => Some(ConstValue::Bool(true)),
            "false" => Some(ConstValue::Bool(false)),
            _ => None,
        },
        ConstValue::Str(_) => Some(ConstValue::Str(text.to_string())),
    }
}

/// Pass 1b: the declarations, collected without looking inside a body.
///
/// Over [`Resolved::block`] rather than the source's own: a `func` inside a
/// branch that was not taken is not part of the program, and one inside a branch
/// that was is indistinguishable from one written at the top level.
fn top_level<'a>(resolved: &mut Resolved<'a>, diags: &mut Diagnostics) {
    // Cloned so the walk can insert: a `Vec` of references, so this is the
    // pointers and not the tree.
    let block = resolved.block.clone();
    for stmt in block {
        let Stmt::Call(Expr::Call { callee, args, .. }) = stmt else {
            continue;
        };
        let Expr::Name(callee) = callee.as_ref() else {
            continue;
        };
        if callee.text != "func" {
            continue;
        }

        let [Expr::Str(name), Expr::Function { params, block, .. }] = args.as_slice() else {
            diags.push(
                Diagnostic::error(
                    Code::BadFieldValue,
                    stmt.span(),
                    "`func` takes a name and a body",
                )
                .note("write `func(\"name\", function(…) … end)`"),
            );
            continue;
        };

        if let Some(previous) = resolved.functions.get(&name.value) {
            diags.push(
                Diagnostic::error(
                    Code::DuplicateBlock,
                    stmt.span(),
                    format!("`{}` is declared more than once", name.value),
                )
                .note_at("the first one is at", previous.span)
                .note("resolution is order-free, so there is no later one that wins"),
            );
            continue;
        }

        if let Some(instead) = reserved(&name.value) {
            diags.push(
                Diagnostic::error(
                    Code::DuplicateBlock,
                    name.span,
                    format!("`{}` is a name the compiler generates", name.value),
                )
                .note(instead)
                .note(
                    "NSIS holds one name once, so the collision would surface as its error on \
                     the emitted script rather than as one on this line",
                ),
            );
            continue;
        }

        resolved.functions.insert(
            name.value.clone(),
            Func {
                name: name.value.clone(),
                params,
                block,
                span: stmt.span(),
            },
        );
    }
}

/// The names `lower` mints for itself, which a `func` may therefore not take —
/// and the note saying where the author writes that code instead.
///
/// Checked here rather than against the module `lower` ends up building, because
/// two `Function`s of one name is not a *lowering* fact: `.onInit` is invented
/// from a global initialiser and `mui.welcome.pre` from a page's `pre`, so the
/// collision depends on what else the program contains and the author's line
/// does not change. Rejecting the name outright is the answer that reads the
/// same either way — and every one of these has a spelling that works.
///
/// Deliberately **not** the whole of NSIS's `.`-led callback namespace:
/// `func(".onVerifyInstDir", …)` is the only way to write that callback today,
/// and this compiler generates nothing by that name.
fn reserved(name: &str) -> Option<&'static str> {
    if name == ".onInit" || name == "un.onInit" {
        return Some(
            "write `onInit(function() … end)` among the entries of `installer {}` or \
             `uninstaller {}` — the `.` and the `un.` are the compiler's",
        );
    }
    if name.starts_with("mui.") || name.starts_with("un.mui.") {
        return Some(
            "`mui.` is where the page callbacks and MUI2 hooks land: write the code on the page \
             or the block that runs it, and the compiler names the function",
        );
    }
    if name.starts_with(crate::cfg::LABEL_PREFIX) {
        return Some("that prefix is the compiler's, for the labels and callbacks it invents");
    }
    None
}

/// Pass 1a: top-level `<const>`s, folded to a fixpoint so that one may refer to
/// another regardless of the order they were written in — and, in the same
/// fixpoint, the top-level `if`s those constants decide.
///
/// The two are one pass because neither finishes without the other: an `if`'s
/// condition is folded from constants, and a constant may be declared inside the
/// branch an `if` takes. So each round declares whatever the selected top level
/// now holds, folds as far as it can, and takes every branch that has become
/// decidable; a round that takes none is the fixpoint. See [`select`].
fn consts<'a>(
    program: &'a Program,
    resolved: &mut Resolved<'a>,
    options: &crate::Options,
    diags: &mut Diagnostics,
) {
    let mut pending: Vec<Pending> = Vec::new();
    let mut items: Vec<Item> = program.block.iter().map(Item::unseen).collect();

    loop {
        for item in &mut items {
            let Item::Stmt { stmt, declared } = item else {
                continue;
            };
            if *declared {
                continue;
            }
            *declared = true;
            declare(stmt, resolved, &mut pending, options, diags);
        }

        fold_pending(&mut pending, resolved, options, diags);

        if !select(&mut items, resolved, diags) {
            break;
        }
    }

    // Every `if` still standing: nothing folded its condition, and no later
    // round can, since the fixpoint has run out of new constants to learn.
    for item in &items {
        if let Item::Cond { cond, .. } = item {
            undecidable(cond, diags);
        }
    }

    resolved.block = items
        .iter()
        .filter_map(|item| match item {
            Item::Stmt { stmt, .. } => Some(*stmt),
            Item::Cond { .. } => None,
        })
        .collect();

    // Source order, read off the selected top level rather than accumulated as
    // the rounds learned things: a `<const>` inside a build-time `if` is
    // declared in a later round than the statements around it, and the `!define`
    // it becomes belongs where the `if` was written.
    resolved.const_order = resolved
        .block
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Local {
                names,
                is_const: true,
                ..
            } => Some(names),
            _ => None,
        })
        .flatten()
        .filter(|name| resolved.consts.contains_key(&name.text))
        .map(|name| name.text.clone())
        .collect();

    for Pending { name, value, param } in pending {
        // A parameter whose default did not fold is reported as a parameter:
        // the `<const>` wording would send the reader looking at the binding,
        // and the mistake is in the default beside the name.
        let (what, note) = match &param {
            Some(param) => (
                format!("the default for `{param}` is not a build-time constant"),
                "a parameter's default is the value a build without a `-D` gets, so it has to be \
                 known here",
            ),
            None => (
                format!("`{}` is not a build-time constant", name.text),
                "a `<const>` folds at compile time, so its value has to be a literal or built \
                 from other `<const>`s",
            ),
        };
        diags.push(Diagnostic::error(Code::BadFieldValue, value.span(), what).note(note));
    }
}

/// One entry in the top level being selected.
///
/// A `Cond` is a hole: it stands where the `if` was written and is replaced, in
/// place, by the statements of whichever branch its condition picks. Holding the
/// position is the whole job — `!define` order and install order are both source
/// order, and a branch that appended its statements at the end would quietly be
/// a different program.
enum Item<'a> {
    Stmt {
        stmt: &'a Stmt,
        /// Whether this statement's declarations have been read. Reading them
        /// twice would report every parameter as declared more than once.
        declared: bool,
    },
    Cond {
        cond: &'a Expr,
        then_block: &'a Block,
        else_block: Option<&'a Block>,
    },
}

impl<'a> Item<'a> {
    fn unseen(stmt: &'a Stmt) -> Item<'a> {
        match stmt {
            Stmt::If {
                cond,
                then_block,
                else_block,
                ..
            } => Item::Cond {
                cond,
                then_block,
                else_block: else_block.as_ref(),
            },
            stmt => Item::Stmt {
                stmt,
                declared: false,
            },
        }
    }
}

/// Takes every branch whose condition has become foldable, in place. `true` when
/// one was taken, which is what the fixpoint iterates on.
///
/// **This is the design point that protects the emitter's spine.** A top-level
/// `if` is not a directive that survives into the output and gets ordered
/// against everything else — it is a branch the compiler takes, and it is gone
/// before a single statement is bucketed. That is why the feature costs
/// `src/emit.rs` nothing.
///
/// An `elseif` needs no case of its own: the frontend has already desugared it
/// into a nested `If` in the else branch, so it arrives here as a `Cond` inside
/// the block this one splices in, and the next round decides it.
fn select(items: &mut Vec<Item>, resolved: &Resolved, diags: &mut Diagnostics) -> bool {
    let mut taken = false;
    let mut out: Vec<Item> = Vec::with_capacity(items.len());
    for item in std::mem::take(items) {
        let Item::Cond {
            cond,
            then_block,
            else_block,
        } = item
        else {
            out.push(item);
            continue;
        };
        let Some(value) = fold(cond, &|name| {
            resolved.consts.get(name).map(|c| c.value.clone())
        }) else {
            out.push(Item::Cond {
                cond,
                then_block,
                else_block,
            });
            continue;
        };
        taken = true;
        let ConstValue::Bool(value) = value else {
            not_bool(cond, &value, diags);
            continue;
        };
        let block = match value {
            true => Some(then_block),
            false => else_block,
        };
        out.extend(block.into_iter().flatten().map(Item::unseen));
    }
    *items = out;
    taken
}

/// A build-time `if` on something that is not a `bool`. The wording is the
/// runtime rule's, because it is the same rule: Lua's truthiness would run
/// `if count then` on `0`, and disagreeing with Lua on the value most likely to
/// be tested is not a trade a compile-time branch gets to make either.
fn not_bool(cond: &Expr, value: &ConstValue, diags: &mut Diagnostics) {
    let replacement = match value {
        ConstValue::Str(_) => "compare it: `x ~= \"\"`",
        _ => "compare it: `x ~= 0`",
    };
    diags.push(
        Diagnostic::error(
            Code::NotBool,
            cond.span(),
            format!("a condition needs a `bool`, and this is a {}", value.ty()),
        )
        .note(replacement)
        .note(
            "in Lua every value but `nil` and `false` is truthy, so `if count then` would run on \
             `0` — a by-type rule would disagree with Lua on exactly the value most likely to be \
             tested",
        ),
    );
}

/// A top-level `if` whose condition never folded.
///
/// The same rule a `<const>` lives under, and the note points at where the
/// runtime `if` does work: outside every body there is nothing to test, since
/// no register has been written yet and the branch would have to be taken by
/// `makensis` rather than by the installer.
fn undecidable(cond: &Expr, diags: &mut Diagnostics) {
    diags.push(
        Diagnostic::error(
            Code::ConstIf,
            cond.span(),
            "a top-level `if` is decided at build time, and this condition is not a build-time \
             constant",
        )
        .note(
            "build it from `<const>`s and `param(…)`s, which fold before anything is emitted — \
             `if param(\"ARCH\", \"x86\") == \"x64\" then`",
        )
        .note(
            "an `if` on a value read at install time belongs inside a `section` or a `func`; out \
             here there is no register to have been written yet",
        ),
    );
}

/// Reads one top-level statement's declarations: a namespace, a deferred
/// section, or a `<const>` for the worklist.
fn declare<'a>(
    stmt: &'a Stmt,
    resolved: &mut Resolved<'a>,
    pending: &mut Vec<Pending<'a>>,
    options: &crate::Options,
    diags: &mut Diagnostics,
) {
    let Stmt::Local {
        names,
        is_const,
        values,
        span,
    } = stmt
    else {
        return;
    };

    // `local fileFunc = import "FileFunc"` is not a value binding at all —
    // it names a namespace, which is why it is the one non-`<const>`
    // `local` the top level accepts.
    if !is_const
        && let ([name], [value]) = (names.as_slice(), values.as_slice())
        && let Some(namespace) = namespace(value, diags)
    {
        resolved.namespaces.insert(name.text.clone(), namespace);
        return;
    }

    // `local core = section { … }` names a section so that install-time
    // code can address it. Held here and lowered by the block that lists
    // it, which is the only place its half is known.
    if !is_const
        && let ([name], [value]) = (names.as_slice(), values.as_slice())
        && let Some(kind) = deferred_kind(value)
    {
        if let Some(previous) = resolved.deferred.get(&name.text) {
            diags.push(
                Diagnostic::error(
                    Code::DuplicateBlock,
                    *span,
                    format!("`{}` is declared more than once", name.text),
                )
                .note_at("the first one is at", previous.span)
                .note("resolution is order-free, so there is no later one that wins"),
            );
            return;
        }
        resolved.deferred_order.push(name.text.clone());
        resolved.deferred.insert(
            name.text.clone(),
            Deferred {
                kind,
                value,
                span: name.span,
            },
        );
        return;
    }

    if !is_const {
        diags.push(
            Diagnostic::error(
                Code::NotYetImplemented,
                *span,
                "a `local` at the top level has nowhere to live",
            )
            .note(
                "there is no install-time code outside a section or a `func`, so a register \
                 here would never be written",
            )
            .note(
                "write `local X <const> = …` for a build-time value, or assign to a bare \
                 name for a global",
            )
            .note(
                "`local x = section { … }` is the other one: it names a section for a block \
                 to list and for install-time code to address",
            ),
        );
        return;
    }

    for (index, name) in names.iter().enumerate() {
        match values.get(index) {
            Some(value) => match initialiser(value, diags) {
                Initialiser::Value(value) => pending.push(Pending {
                    name,
                    value,
                    param: None,
                }),
                Initialiser::Param {
                    name: param,
                    span,
                    default,
                } => {
                    if declared_twice(resolved, &param, span, diags) {
                        continue;
                    }
                    resolved.params.insert(
                        param.clone(),
                        Param {
                            bound: name.text.clone(),
                            span,
                            // Replaced the moment the default folds. A
                            // parameter whose default is not constant never
                            // gets that far and is reported as the `<const>`
                            // it failed to be.
                            value: ConstValue::Bool(false),
                        },
                    );
                    pending.push(Pending {
                        name,
                        value: default,
                        param: Some(param),
                    });
                }
                // Nothing to fold and nothing to wait for: the value is the
                // flag's text or there is no build. So this settles here rather
                // than going on the worklist, and the missing flag is reported
                // at the declaration, which is the line that says it is needed.
                Initialiser::Required { name: param, span } => {
                    if declared_twice(resolved, &param, span, diags) {
                        continue;
                    }
                    let value = match options.params.get(&param) {
                        Some(text) => ConstValue::Str(text.clone()),
                        None => {
                            missing_param(&param, name, span, diags);
                            // Bound anyway, to the empty string. The build is
                            // already failing; every use of the name reporting
                            // "not a build-time constant" on top of it would
                            // bury the one diagnostic that says what to do.
                            ConstValue::Str(String::new())
                        }
                    };
                    resolved.params.insert(
                        param.clone(),
                        Param {
                            bound: name.text.clone(),
                            span,
                            value: value.clone(),
                        },
                    );
                    resolved.consts.insert(
                        name.text.clone(),
                        Const {
                            value,
                            span: name.span,
                        },
                    );
                }
                Initialiser::Broken => {}
            },
            None => diags.push(
                Diagnostic::error(
                    Code::BadFieldValue,
                    name.span,
                    format!("`{}` is `<const>` with no value", name.text),
                )
                .note("a build-time constant is its value; there is nothing to assign later"),
            ),
        }
    }
}

/// Whether this `-D` name is already a parameter, reported if it is.
///
/// Declared twice is a genuine ambiguity rather than a tidiness rule: two
/// declarations of one `-D` name have no answer, and resolution being order-free
/// means there is no later one to let win.
fn declared_twice(resolved: &Resolved, param: &str, span: Span, diags: &mut Diagnostics) -> bool {
    let Some(previous) = resolved.params.get(param) else {
        return false;
    };
    diags.push(
        Diagnostic::error(
            Code::DuplicateBlock,
            span,
            format!("`{param}` is declared as a parameter more than once"),
        )
        .note_at(
            format!("the first one, bound to `{}`, is at", previous.bound),
            previous.span,
        )
        .note("a parameter is declared once, since `-D` sets it once"),
    );
    true
}

/// A required parameter the invocation did not set.
///
/// Reported at the declaration and not at a use: the declaration is what says
/// the build cannot be done without the value, and every use of it is a
/// consequence.
fn missing_param(param: &str, bound: &Name, span: Span, diags: &mut Diagnostics) {
    diags.push(
        Diagnostic::error(
            Code::MissingParam,
            span,
            format!("`{param}` has no default, so the build needs `-D {param}=…`"),
        )
        .note(format!(
            "`{PARAM}(\"{param}\")` declares a value the invocation has to supply; write \
             `{PARAM}(\"{param}\", …)` to give it a default instead"
        ))
        .note(format!(
            "`{}` is a `string` here: with no default there is nothing to say what type the \
             flag's text should be read as",
            bound.text
        )),
    );
}

/// Folds every `<const>` that can fold, to a fixpoint, applying `-D` as it goes.
///
/// A worklist rather than one pass: `local A <const> = B` is legal above `B`,
/// and saying so costs a loop that almost always runs twice. Whatever is left in
/// `pending` when this returns did not fold *this round* — which is not yet an
/// error, since a later round may declare the constant it was waiting for.
fn fold_pending(
    pending: &mut Vec<Pending>,
    resolved: &mut Resolved,
    options: &crate::Options,
    diags: &mut Diagnostics,
) {
    loop {
        let folded: Vec<(String, Const, Option<String>)> = pending
            .iter()
            .filter_map(|entry| {
                let value = fold(entry.value, &|n| {
                    resolved.consts.get(n).map(|c| c.value.clone())
                })?;
                Some((
                    entry.name.text.clone(),
                    Const {
                        value,
                        span: entry.name.span,
                    },
                    entry.param.clone(),
                ))
            })
            .collect();
        if folded.is_empty() {
            break;
        }
        for (name, mut folded, param) in folded {
            // The override lands here rather than in `fold`, because this is
            // the first point at which the default has a *type* for the text on
            // the command line to be read as.
            if let Some(param) = param {
                if let Some(text) = options.params.get(&param) {
                    match coerce(text, &folded.value) {
                        Some(value) => folded.value = value,
                        None => diags.push(
                            Diagnostic::error(
                                Code::BadFieldValue,
                                folded.span,
                                format!("`-D {param}={text}` is not {}", article(&folded.value)),
                            )
                            .note(format!(
                                "the default here is `{}`, and that is what says what type the \
                                 flag has to be",
                                folded.value.text()
                            )),
                        ),
                    }
                }
                if let Some(entry) = resolved.params.get_mut(&param) {
                    entry.value = folded.value.clone();
                }
            }
            resolved.consts.insert(name, folded);
        }
        pending.retain(|entry| !resolved.consts.contains_key(&entry.name.text));
    }
}

/// `an integer` / `a boolean` / `a string`, for the ill-typed-`-D` message.
fn article(value: &ConstValue) -> &'static str {
    match value {
        ConstValue::Int(_) => "an integer",
        ConstValue::Bool(_) => "`true` or `false`",
        // Unreachable: every text coerces to a string, so a string-defaulted
        // parameter has no ill-typed value to report.
        ConstValue::Str(_) => "a string",
    }
}

/// `section { … }`, `group { … }` or a control, as the kind of declaration it
/// defers.
///
/// The shape of the call is not checked here: that is `fn section`'s work, and
/// it happens where the call is lowered so that one wrong `section` reports once
/// rather than once per pass.
fn deferred_kind(value: &Expr) -> Option<DeferredKind> {
    // The one page written as a declaration. Every other `page.*` is an entry
    // and nothing else, because nothing else has anything for a local to hold.
    if let Some(("page", which)) = value.callee_field()
        && which.text == "startMenu"
    {
        return Some(DeferredKind::StartMenu);
    }
    let callee = value.callee_name()?;
    match callee {
        "section" => Some(DeferredKind::Section),
        "group" => Some(DeferredKind::Group),
        _ => control::control(callee).map(DeferredKind::Control),
    }
}

/// `import "FileFunc"` / `plugin "nsExec"`, as the namespace it names.
///
/// `None` when the call is neither, which leaves the ordinary "a `local` at the
/// top level has nowhere to live" rejection to fire.
fn namespace(value: &Expr, diags: &mut Diagnostics) -> Option<Namespace> {
    let Expr::Call { callee, args, span } = value else {
        return None;
    };
    let Expr::Name(callee) = callee.as_ref() else {
        return None;
    };
    let build: fn(String) -> Namespace = match callee.text.as_str() {
        "import" => Namespace::Header,
        "plugin" => Namespace::Plugin,
        _ => return None,
    };

    let [Expr::Str(name)] = args.as_slice() else {
        diags.push(
            Diagnostic::error(
                Code::BadFieldValue,
                *span,
                format!("`{}` takes one name", callee.text),
            )
            .note(format!(
                "write `local x = {} \"Name\"`, with a literal — a header is read at build \
                 time, so there is nothing for a computed name to be",
                callee.text
            )),
        );
        return None;
    };
    Some(build(name.value.clone()))
}

/// Pass 1c: globals. A bare assignment declares one, and it can happen anywhere
/// — inside a section, inside a `func` — so this walks every body.
fn globals(resolved: &mut Resolved<'_>) {
    let mut scopes: Vec<HashSet<String>> = vec![HashSet::new()];
    let mut found: Vec<Global> = Vec::new();
    // The selected top level, so a global assigned only inside a branch that
    // was not taken never gets a `Var`.
    let block = resolved.block.clone();
    scopes.push(HashSet::new());
    for stmt in block {
        scan_stmt(stmt, &mut scopes, &mut found, resolved);
    }
    resolved.globals = found;
}

fn scan_block(
    block: &Block,
    scopes: &mut Vec<HashSet<String>>,
    found: &mut Vec<Global>,
    resolved: &Resolved<'_>,
) {
    scopes.push(HashSet::new());
    for stmt in block {
        scan_stmt(stmt, scopes, found, resolved);
    }
    scopes.pop();
}

fn scan_stmt(
    stmt: &Stmt,
    scopes: &mut Vec<HashSet<String>>,
    found: &mut Vec<Global>,
    resolved: &Resolved<'_>,
) {
    match stmt {
        Stmt::Local { names, values, .. } => {
            for value in values {
                scan_expr(value, scopes, found, resolved);
            }
            // Bound *after* the initialiser, as Lua binds them.
            for name in names {
                bind(scopes, &name.text);
            }
        }

        Stmt::Assign {
            targets, values, ..
        } => {
            for value in values {
                scan_expr(value, scopes, found, resolved);
            }
            for target in targets {
                if let Expr::Name(name) = target
                    && !bound(scopes, &name.text)
                    && !found.iter().any(|g| g.name == name.text)
                    && builtins::constant_named(&name.text).is_none()
                    // `currentInstType = "Minimal"` is `SetCurInstType`, not a
                    // slot to allocate.
                    && !builtins::owned(&name.text)
                    && !resolved.consts.contains_key(&name.text)
                {
                    found.push(Global {
                        name: name.text.clone(),
                        span: name.span,
                    });
                }
            }
        }

        Stmt::Call(expr) => scan_expr(expr, scopes, found, resolved),

        Stmt::If {
            cond,
            then_block,
            else_block,
            ..
        } => {
            scan_expr(cond, scopes, found, resolved);
            scan_block(then_block, scopes, found, resolved);
            if let Some(block) = else_block {
                scan_block(block, scopes, found, resolved);
            }
        }

        Stmt::While { cond, block, .. } => {
            scan_expr(cond, scopes, found, resolved);
            scan_block(block, scopes, found, resolved);
        }

        Stmt::NumericFor {
            name,
            start,
            end,
            step,
            block,
            ..
        } => {
            scan_expr(start, scopes, found, resolved);
            scan_expr(end, scopes, found, resolved);
            if let Some(step) = step {
                scan_expr(step, scopes, found, resolved);
            }
            scopes.push(HashSet::from([name.text.clone()]));
            scan_block(block, scopes, found, resolved);
            scopes.pop();
        }

        Stmt::GenericFor {
            names,
            iterator,
            block,
            ..
        } => {
            scan_expr(iterator, scopes, found, resolved);
            scopes.push(names.iter().map(|n| n.text.clone()).collect());
            scan_block(block, scopes, found, resolved);
            scopes.pop();
        }

        Stmt::Do { block, .. } => scan_block(block, scopes, found, resolved),

        Stmt::Return { values, .. } => {
            for value in values {
                scan_expr(value, scopes, found, resolved);
            }
        }

        Stmt::Break { .. } => {}
    }
}

fn scan_expr(
    expr: &Expr,
    scopes: &mut Vec<HashSet<String>>,
    found: &mut Vec<Global>,
    resolved: &Resolved<'_>,
) {
    match expr {
        Expr::Function { params, block, .. } => {
            scopes.push(params.iter().map(|p| p.text.clone()).collect());
            scan_block(block, scopes, found, resolved);
            scopes.pop();
        }
        Expr::Call { callee, args, .. } => {
            scan_expr(callee, scopes, found, resolved);
            for arg in args {
                scan_expr(arg, scopes, found, resolved);
            }
        }
        Expr::MethodCall { receiver, args, .. } => {
            scan_expr(receiver, scopes, found, resolved);
            for arg in args {
                scan_expr(arg, scopes, found, resolved);
            }
        }
        Expr::Field { base, .. } => scan_expr(base, scopes, found, resolved),
        Expr::Table { fields, .. } => {
            for field in fields {
                let (TableField::Named { value, .. } | TableField::Positional { value }) = field;
                scan_expr(value, scopes, found, resolved);
            }
        }
        Expr::Binary { lhs, rhs, .. } => {
            scan_expr(lhs, scopes, found, resolved);
            scan_expr(rhs, scopes, found, resolved);
        }
        Expr::Unary { operand, .. } => scan_expr(operand, scopes, found, resolved),
        Expr::Number { .. } | Expr::Str(_) | Expr::Bool { .. } | Expr::Name(_) => {}
    }
}

fn bind(scopes: &mut [HashSet<String>], name: &str) {
    if let Some(scope) = scopes.last_mut() {
        scope.insert(name.to_string());
    }
}

fn bound(scopes: &[HashSet<String>], name: &str) -> bool {
    scopes.iter().any(|scope| scope.contains(name))
}

/// Constant folding, which is also why `!if`/`!ifdef` never need a surface
/// spelling: a `<const>` condition folds before a branch is ever built.
///
/// `lookup` is a closure rather than a map so that the same function serves the
/// top-level pass, where only top-level constants are visible, and a body,
/// where a `<const>` local shadows one.
pub fn fold(expr: &Expr, lookup: &dyn Fn(&str) -> Option<ConstValue>) -> Option<ConstValue> {
    match expr {
        Expr::Number { value, .. } => Some(ConstValue::Int(*value)),
        Expr::Str(literal) => Some(ConstValue::Str(literal.value.clone())),
        Expr::Bool { value, .. } => Some(ConstValue::Bool(*value)),
        Expr::Name(name) => lookup(&name.text),

        Expr::Unary { op, operand, .. } => match (op, fold(operand, lookup)?) {
            (UnOp::Neg, ConstValue::Int(value)) => Some(ConstValue::Int(value.wrapping_neg())),
            (UnOp::Not, ConstValue::Bool(value)) => Some(ConstValue::Bool(!value)),
            (UnOp::BitNot, ConstValue::Int(value)) => Some(ConstValue::Int(!value)),
            _ => None,
        },

        Expr::Binary { op, lhs, rhs, .. } => {
            let (lhs, rhs) = (fold(lhs, lookup)?, fold(rhs, lookup)?);
            match (op, &lhs, &rhs) {
                // Concatenation folds across types, exactly as it does at
                // runtime: a number in a message is its digits.
                (BinOp::Concat, _, _) => {
                    Some(ConstValue::Str(format!("{}{}", lhs.text(), rhs.text())))
                }
                // Ordering folds to a `bool`, so it cannot go through
                // [`integer`], which answers with the `int` an arithmetic
                // operator produces.
                (BinOp::Lt, ConstValue::Int(a), ConstValue::Int(b)) => {
                    Some(ConstValue::Bool(a < b))
                }
                (BinOp::Le, ConstValue::Int(a), ConstValue::Int(b)) => {
                    Some(ConstValue::Bool(a <= b))
                }
                (BinOp::Gt, ConstValue::Int(a), ConstValue::Int(b)) => {
                    Some(ConstValue::Bool(a > b))
                }
                (BinOp::Ge, ConstValue::Int(a), ConstValue::Int(b)) => {
                    Some(ConstValue::Bool(a >= b))
                }
                (op, ConstValue::Int(a), ConstValue::Int(b)) => {
                    integer(*op, *a, *b).map(ConstValue::Int)
                }
                (BinOp::And, ConstValue::Bool(a), ConstValue::Bool(b)) => {
                    Some(ConstValue::Bool(*a && *b))
                }
                (BinOp::Or, ConstValue::Bool(a), ConstValue::Bool(b)) => {
                    Some(ConstValue::Bool(*a || *b))
                }
                (BinOp::Eq, a, b) => Some(ConstValue::Bool(a == b)),
                (BinOp::Ne, a, b) => Some(ConstValue::Bool(a != b)),
                _ => None,
            }
        }

        _ => None,
    }
}

/// Integer folding at Lua's semantics, not NSIS's — `//` floors and `%` takes
/// the sign of the divisor. Folding is the one place the fixup is free, because
/// it happens in Rust.
fn integer(op: BinOp, a: i64, b: i64) -> Option<i64> {
    match op {
        BinOp::Add => Some(a.wrapping_add(b)),
        BinOp::Sub => Some(a.wrapping_sub(b)),
        BinOp::Mul => Some(a.wrapping_mul(b)),
        // Lua floors and NSIS truncates, so folding does what Lua says and the
        // runtime lowering carries the fixup.
        BinOp::FloorDiv if b != 0 => {
            let (quotient, remainder) = (a.wrapping_div(b), a.wrapping_rem(b));
            Some(quotient - i64::from(remainder != 0 && (remainder < 0) != (b < 0)))
        }
        // Lua's `%` takes the sign of the divisor; NSIS's takes the dividend's.
        BinOp::Mod if b != 0 => {
            let remainder = a.wrapping_rem(b);
            Some(if remainder != 0 && (remainder < 0) != (b < 0) {
                remainder + b
            } else {
                remainder
            })
        }
        BinOp::BitAnd => Some(a & b),
        BinOp::BitOr => Some(a | b),
        BinOp::BitXor => Some(a ^ b),
        BinOp::Shl => Some(a.wrapping_shl(b as u32)),
        BinOp::Shr => Some(((a as u64).wrapping_shr(b as u32)) as i64),
        _ => None,
    }
}
