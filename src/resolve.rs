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

#[derive(Debug, Default)]
pub struct Resolved<'a> {
    pub consts: BTreeMap<String, Const>,
    /// Top-level `<const>` names in **source order**, because each becomes a
    /// `!define` and the preprocessor is strictly sequential. The map is
    /// alphabetical and the output is not.
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

pub fn resolve<'a>(program: &'a Program, diags: &mut Diagnostics) -> Resolved<'a> {
    let mut resolved = Resolved::default();
    top_level(program, &mut resolved, diags);
    consts(program, &mut resolved, diags);
    globals(program, &mut resolved);
    resolved
}

/// Pass 1a: the declarations, collected without looking inside a body.
fn top_level<'a>(program: &'a Program, resolved: &mut Resolved<'a>, diags: &mut Diagnostics) {
    for stmt in &program.block {
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
                .note(format!(
                    "the first one is at line {}",
                    previous.span.start_line
                ))
                .note("resolution is order-free, so there is no later one that wins"),
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

/// Pass 1b: top-level `<const>`s, folded to a fixpoint so that one may refer to
/// another regardless of the order they were written in.
fn consts<'a>(program: &'a Program, resolved: &mut Resolved<'a>, diags: &mut Diagnostics) {
    let mut pending: Vec<(&Name, &Expr)> = Vec::new();

    for stmt in &program.block {
        let Stmt::Local {
            names,
            is_const,
            values,
            span,
        } = stmt
        else {
            continue;
        };

        // `local fileFunc = import "FileFunc"` is not a value binding at all —
        // it names a namespace, which is why it is the one non-`<const>`
        // `local` the top level accepts.
        if !is_const
            && let ([name], [value]) = (names.as_slice(), values.as_slice())
            && let Some(namespace) = namespace(value, diags)
        {
            resolved.namespaces.insert(name.text.clone(), namespace);
            continue;
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
                    .note(format!(
                        "the first one is at line {}",
                        previous.span.start_line
                    ))
                    .note("resolution is order-free, so there is no later one that wins"),
                );
                continue;
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
            continue;
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
            continue;
        }

        for (index, name) in names.iter().enumerate() {
            match values.get(index) {
                Some(value) => {
                    resolved.const_order.push(name.text.clone());
                    pending.push((name, value));
                }
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

    // A worklist rather than one pass: `local A <const> = B` is legal above `B`,
    // and saying so costs a loop that almost always runs twice.
    loop {
        let folded: Vec<(String, Const)> = pending
            .iter()
            .filter_map(|(name, value)| {
                let value = fold(value, &|n| resolved.consts.get(n).map(|c| c.value.clone()))?;
                Some((
                    name.text.clone(),
                    Const {
                        value,
                        span: name.span,
                    },
                ))
            })
            .collect();
        if folded.is_empty() {
            break;
        }
        for (name, value) in folded {
            resolved.consts.insert(name, value);
        }
        pending.retain(|(name, _)| !resolved.consts.contains_key(&name.text));
    }

    for (name, value) in pending {
        diags.push(
            Diagnostic::error(
                Code::BadFieldValue,
                value.span(),
                format!("`{}` is not a build-time constant", name.text),
            )
            .note(
                "a `<const>` folds at compile time, so its value has to be a literal or built \
                 from other `<const>`s",
            ),
        );
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
fn globals(program: &Program, resolved: &mut Resolved<'_>) {
    let mut scopes: Vec<HashSet<String>> = vec![HashSet::new()];
    let mut found: Vec<Global> = Vec::new();
    scan_block(&program.block, &mut scopes, &mut found, resolved);
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
