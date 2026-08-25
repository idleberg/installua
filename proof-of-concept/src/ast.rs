//! The Luis AST: what survives the whitelist pass over full-moon's Lua CST.

use crate::diag::Span;
use crate::overlay;

/// The whole type lattice of this PoC: enough to keep `IntOp` and `IntCmp` off
/// strings, nowhere near the real thing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Ty {
    Int,
    Str,
}

impl Ty {
    pub fn name(self) -> &'static str {
        match self {
            Ty::Int => "int",
            Ty::Str => "string",
        }
    }
}

#[derive(Debug)]
pub struct Program {
    pub installer: Option<Installer>,
    /// Headers actually imported, in overlay order.
    pub includes: Vec<&'static overlay::Header>,
    /// One-time init lines a used macro asked for, deduplicated.
    pub inits: Vec<&'static str>,
    /// `local X <const>` → `!define`, in source order: unlike everything else,
    /// the preprocessor is strictly sequential.
    pub defines: Vec<Define>,
    pub functions: Vec<Function>,
    /// Sections and section groups. Order is install order, so this is the one
    /// sequence the emitter may not reorder.
    pub items: Vec<SectionItem>,
}

/// A build-time constant. `value` is already NSIS text, so a `<const>` that
/// refers to another one holds `${OTHER}` rather than a copy.
#[derive(Debug)]
pub struct Define {
    pub name: String,
    pub value: String,
    /// The build-machine command that has to run before the `!define`, when the
    /// value came from `pre.*`.
    pub pre: Option<PreCall>,
    pub span: Span,
}

#[derive(Debug)]
pub struct PreCall {
    pub pre: &'static overlay::Pre,
    pub args: Vec<String>,
    /// The symbol prefix the command writes its outputs into.
    pub prefix: String,
}

#[derive(Debug)]
pub struct Function {
    pub name: String,
    pub span: Span,
    pub body: Vec<Stmt>,
}

#[derive(Debug)]
pub struct Installer {
    pub span: Span,
    pub fields: Vec<Field>,
}

#[derive(Debug)]
pub struct Field {
    pub name: String,
    pub value: String,
    pub span: Span,
}

#[derive(Debug)]
pub enum SectionItem {
    Section(Section),
    Group(SectionGroup),
}

#[derive(Debug)]
pub struct SectionGroup {
    pub name: String,
    /// `SectionGroup /e` — expanded in the components tree.
    pub expanded: bool,
    pub sections: Vec<Section>,
    pub span: Span,
}

#[derive(Debug)]
pub struct Section {
    pub name: String,
    /// `Section /o` — unselected by default.
    pub optional: bool,
    pub span: Span,
    pub body: Vec<Stmt>,
}

#[derive(Debug)]
pub enum Stmt {
    /// `local x = <expr>` — becomes a register.
    Local(Local),
    Call(Call),
    /// Install-time control flow, not build-time. `elseif` is desugared into a
    /// nested `If` in the else branch by the frontend.
    If(If),
    /// A call to a user-declared `function` — `Call name`.
    CallFunction {
        name: String,
        span: Span,
    },
    /// A macro from an imported header, used for its effect.
    Macro(MacroCall),
    Plugin(PluginCall),
    /// Its own node because it is its own lowering: a statement, a flag set and
    /// a jump table at once.
    MessageBox(MessageBox),
}

#[derive(Debug)]
pub struct MessageBox {
    pub buttons: &'static overlay::ButtonSet,
    pub icon: Option<&'static overlay::Icon>,
    pub text: Expr,
    /// `/SD` — the answer a silent install takes.
    pub default: Option<&'static overlay::Button>,
    pub handlers: Vec<Handler>,
    pub span: Span,
}

/// A branch of the jump table. The body is a block, not a value: closures exist
/// as declaration bodies only.
#[derive(Debug)]
pub struct Handler {
    pub button: &'static overlay::Button,
    pub body: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug)]
pub struct MacroCall {
    pub header: &'static overlay::Header,
    pub mac: &'static overlay::Macro,
    pub args: Vec<Expr>,
    pub span: Span,
}

/// `plugin "System"` then `system.call(...)`. Plugin methods are open-ended, so
/// unlike instructions they cannot be overlay-checked — only their shape can.
#[derive(Debug)]
pub struct PluginCall {
    pub plugin: String,
    pub method: String,
    pub args: Vec<Expr>,
    pub span: Span,
}

#[derive(Debug)]
pub struct Local {
    pub name: String,
    pub value: Expr,
    pub span: Span,
}

/// An instruction call.
#[derive(Debug)]
pub struct Call {
    pub name: String,
    pub args: Vec<Expr>,
    pub span: Span,
}

#[derive(Debug)]
pub struct If {
    pub cond: Condition,
    pub then_body: Vec<Stmt>,
    pub else_body: Vec<Stmt>,
    pub span: Span,
}

/// A condition is a comparison, never a materialized boolean: it lowers
/// straight into the branch targets of one `IntCmp`.
#[derive(Debug)]
pub struct Condition {
    pub op: CmpOp,
    pub lhs: Expr,
    pub rhs: Expr,
    pub span: Span,
}

#[derive(Debug)]
pub enum Expr {
    Number(i64, Span),
    Str(String, Span),
    /// A reference to a `local`.
    Local(String, Span),
    /// A reference to a `<const>`: build-time, so it carries its own NSIS text
    /// (`${NAME}`) rather than a register.
    Const {
        text: String,
        ty: Ty,
        span: Span,
    },
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: Span,
    },
    /// `..` is usually zero instructions: NSIS interpolates variables inside
    /// literals, so this lowers to a string template.
    Concat(Vec<Expr>, Span),
    /// A macro with an output variable, used for its value.
    Macro(Box<MacroCall>),
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Number(_, span) | Expr::Str(_, span) | Expr::Local(_, span) => *span,
            Expr::Const { span, .. } => *span,
            Expr::Binary { span, .. } => *span,
            Expr::Concat(_, span) => *span,
            Expr::Macro(call) => call.span,
        }
    }
}

/// Operators are instructions: each of these is an `IntOp` opcode. Lua's `/`
/// and `^` are absent on purpose — they mean something else in NSIS.
#[derive(Debug, Clone, Copy)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    /// Lua's `//`, which is NSIS's `/`. The rounding differs on negatives:
    /// Lua floors, NSIS truncates toward zero.
    Div,
    /// Lua's `%` takes the sign of the divisor, NSIS's of the dividend.
    Mod,
}

impl BinOp {
    pub fn nsis(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Mod => "%",
        }
    }

    /// Build-time evaluation, in NSIS's semantics rather than Lua's, so a
    /// folded operation and an emitted one cannot disagree.
    pub fn fold(self, lhs: i64, rhs: i64) -> Option<i64> {
        match self {
            BinOp::Add => lhs.checked_add(rhs),
            BinOp::Sub => lhs.checked_sub(rhs),
            BinOp::Mul => lhs.checked_mul(rhs),
            BinOp::Div => lhs.checked_div(rhs),
            BinOp::Mod => lhs.checked_rem(rhs),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum CmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}
