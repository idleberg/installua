//! The NSIS-side IR. Everything here is already in NSIS spelling; the emitter
//! only decides layout and quoting.

pub struct Module {
    /// `!include`d headers, emitted once and only when something used them.
    pub includes: Vec<&'static str>,
    /// `!getdllversion` and `!define`, in source order — the preprocessor is
    /// textual and strictly sequential, unlike everything below it.
    pub directives: Vec<Instruction>,
    /// One-time header init lines (`${StrCase}`), after the includes.
    pub inits: Vec<Instruction>,
    /// Attributes emitted before the first section (`Name`, `OutFile`, …).
    pub attributes: Vec<Instruction>,
    pub functions: Vec<Function>,
    pub sections: Vec<SectionItem>,
}

pub struct Function {
    pub name: String,
    pub body: Vec<Item>,
}

pub enum SectionItem {
    Section(Section),
    Group(SectionGroup),
}

pub struct SectionGroup {
    pub name: String,
    pub expanded: bool,
    pub sections: Vec<Section>,
}

pub struct Section {
    pub name: String,
    pub optional: bool,
    pub body: Vec<Item>,
}

/// Labels are IR nodes, not strings spliced by the lowering code. This is still
/// a flat list rather than a block graph — that is the next step.
pub enum Item {
    Instruction(Instruction),
    Label(String),
}

/// `name` is a `String` rather than `&'static str` because macro invocations
/// (`${WinVerGetMajor}`) and plugin calls (`System::Call`) are composed, not
/// looked up whole.
pub struct Instruction {
    pub name: String,
    pub args: Vec<Arg>,
}

impl Instruction {
    pub fn new(name: impl Into<String>, args: Vec<Arg>) -> Self {
        Instruction {
            name: name.into(),
            args,
        }
    }
}

/// Whether the emitter quotes an argument. Registers, integers, `IntOp`
/// opcodes, flag lists and branch targets are NSIS syntax, not data.
pub enum Arg {
    Str(String),
    Raw(String),
}

impl Arg {
    pub fn raw(value: impl Into<String>) -> Self {
        Arg::Raw(value.into())
    }

    pub fn str(value: impl Into<String>) -> Self {
        Arg::Str(value.into())
    }
}
