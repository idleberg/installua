//! The Installua AST: what survives the whitelist pass over `full-moon`'s Lua CST.
//!
//! This is a *syntax* tree, not a resolved one. Nothing here knows whether
//! `section` is a declaration or `detailPrint` an instruction — that is Phase 2's
//! job, and keeping it out means the whitelist pass has exactly one
//! responsibility: deciding which Lua forms exist in this language at all.
//!
//! Two transformations have already happened by the time a tree of these
//! exists, because both are cheaper before names mean anything:
//!
//!   * `elseif` is desugared into a nested `If` in the else branch (§7), so
//!     every `If` here has exactly one condition.
//!   * string literals carry their *decoded* value, so no later pass ever
//!     re-interprets an escape.

use crate::diag::Span;

/// A whole source file: the top-level block.
#[derive(Clone, Debug)]
pub struct Program {
    pub block: Block,
}

pub type Block = Vec<Stmt>;

// The variants differ in size because `Expr` is inlined rather than boxed, and
// boxing every statement to flatten that would cost an allocation per node on
// the hot path to save bytes nothing is short of.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)]
pub enum Stmt {
    /// `local a, b = f()` — and `local X <const> = 5`, which is a different
    /// thing entirely (build-time, §7-1) but the same syntax.
    Local {
        names: Vec<Name>,
        is_const: bool,
        values: Vec<Expr>,
        span: Span,
    },
    /// `x = 1`, and `a, b = f()`. A bare assignment declares a global (§15.24).
    Assign {
        targets: Vec<Expr>,
        values: Vec<Expr>,
        span: Span,
    },
    /// A call used for its effect. Lua's grammar already restricts statement
    /// expressions to calls, so nothing else can appear here.
    Call(Expr),
    /// Install-time control flow. Exactly one condition — `elseif` is gone.
    If {
        cond: Expr,
        then_block: Block,
        else_block: Option<Block>,
        span: Span,
    },
    While {
        cond: Expr,
        block: Block,
        span: Span,
    },
    /// `for i = start, end, step do`.
    NumericFor {
        name: Name,
        start: Expr,
        end: Expr,
        step: Option<Expr>,
        block: Block,
        span: Span,
    },
    /// `for x in <iterator> do`. Which iterators exist is checked here, not
    /// later: the set is closed (§7) and syntax is all it takes to see.
    GenericFor {
        names: Vec<Name>,
        iterator: Expr,
        block: Block,
        span: Span,
    },
    Do {
        block: Block,
        span: Span,
    },
    Break {
        span: Span,
    },
    Return {
        values: Vec<Expr>,
        span: Span,
    },
}

impl Stmt {
    pub fn span(&self) -> Span {
        match self {
            Stmt::Local { span, .. }
            | Stmt::Assign { span, .. }
            | Stmt::If { span, .. }
            | Stmt::While { span, .. }
            | Stmt::NumericFor { span, .. }
            | Stmt::GenericFor { span, .. }
            | Stmt::Do { span, .. }
            | Stmt::Break { span }
            | Stmt::Return { span, .. } => *span,
            Stmt::Call(expr) => expr.span(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Name {
    pub text: String,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum Expr {
    /// Integers only. A float literal never gets this far (§6).
    Number {
        value: i64,
        span: Span,
    },
    /// Already decoded: `\t` is a tab in here, and a long string is
    /// indistinguishable from a short one, which is correct — the difference is
    /// lexical (§5).
    Str(StrLit),
    Bool {
        value: bool,
        span: Span,
    },
    Name(Name),
    /// `a.b` — a header's macro, a plugin's method, `lang.greeting`.
    Field {
        base: Box<Expr>,
        name: Name,
        span: Span,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
        span: Span,
    },
    /// `handle:close()`. Distinct from `Field` + `Call` because the receiver is
    /// an argument in NSIS and a syntactic prefix here.
    MethodCall {
        receiver: Box<Expr>,
        method: Name,
        args: Vec<Expr>,
        span: Span,
    },
    /// A declaration body. Only ever a direct call argument (§3).
    Function {
        params: Vec<Name>,
        block: Block,
        span: Span,
    },
    /// A compile-time table: `attributes { … }`, `pages = { … }`.
    Table {
        fields: Vec<TableField>,
        span: Span,
    },
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: Span,
    },
    Unary {
        op: UnOp,
        operand: Box<Expr>,
        span: Span,
    },
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Number { span, .. }
            | Expr::Bool { span, .. }
            | Expr::Field { span, .. }
            | Expr::Call { span, .. }
            | Expr::MethodCall { span, .. }
            | Expr::Function { span, .. }
            | Expr::Table { span, .. }
            | Expr::Binary { span, .. }
            | Expr::Unary { span, .. } => *span,
            Expr::Str(lit) => lit.span,
            Expr::Name(name) => name.span,
        }
    }

    /// This expression as a bare name, when that is what it is.
    pub fn name(&self) -> Option<&str> {
        match self {
            Expr::Name(name) => Some(&name.text),
            _ => None,
        }
    }

    /// The name of a call's callee, when the callee is a bare name. Used by the
    /// handful of *syntactic* whitelist rules that are about a spelling rather
    /// than a shape (`raw`, `require`, the iterators).
    pub fn callee_name(&self) -> Option<&str> {
        match self {
            Expr::Call { callee, .. } => match callee.as_ref() {
                Expr::Name(name) => Some(&name.text),
                _ => None,
            },
            _ => None,
        }
    }

    /// The same, for a callee written as `base.member` — `page.directory { … }`.
    /// The member comes back whole rather than as text, because it is the thing
    /// a diagnostic points at.
    pub fn callee_field(&self) -> Option<(&str, &Name)> {
        match self {
            Expr::Call { callee, .. } => match callee.as_ref() {
                Expr::Field { base, name, .. } => Some((base.name()?, name)),
                _ => None,
            },
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct StrLit {
    /// The decoded value: escapes are already resolved.
    pub value: String,
    /// A long string (`[[…]]`) processes no escapes, and `raw` takes one
    /// verbatim — both are lexical facts the emitter has no way to recover.
    pub long: bool,
    pub span: Span,
}

/// A field of a compile-time table. `pages = { "Welcome", … }` is positional and
/// `versionInfo = { product = … }` is named, so both exist.
#[derive(Clone, Debug)]
pub enum TableField {
    Named { name: Name, value: Expr },
    Positional { value: Expr },
}

/// Every operator that survives §6. `/` and `^` are absent because they are
/// rejected at the operator, not lowered to something close enough.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    /// Lua's `//`. NSIS truncates toward zero and Lua floors, so this carries a
    /// fixup obligation into Phase 2 (§15.4).
    FloorDiv,
    /// Lua's `%`: sign of the divisor, where NSIS takes the dividend's.
    Mod,
    Concat,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    BitAnd,
    BitOr,
    /// Lua's `~`, which is NSIS's `^` — the spellings swap (§6).
    BitXor,
    Shl,
    /// Lua's `>>` zero-fills, which is NSIS's `>>>`.
    Shr,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
    /// Lua's `~`: bitwise not, one-operand `IntOp` (§6).
    BitNot,
}
