//! The NSIS-side IR. Everything here is already in NSIS spelling; the emitter
//! decides layout and quoting and nothing else.
//!
//! `Module`'s field order **is** the emission order (§12, as the five programs
//! forced it in Phase 0), so the one thing a reader has to keep straight is
//! written down once, in the type, rather than in the emitter's control flow:
//!
//!   1. `Unicode` — first, so a later `raw` overrides it rather than being
//!      silently overridden (§15.16)
//!   2. `!define`s, in source order — the preprocessor is textual and strictly
//!      sequential, unlike everything below it
//!   3. `!include`s
//!   4. header init lines (`${Using:StrFunc}`)
//!   5. attributes, in overlay order
//!   6. MUI defines, page macros, `MUI_LANGUAGE`
//!   7. `Var`s
//!   8. functions
//!   9. sections, in source order — a sequence, never reordered
//!
//! Phase 1 fills 1, 5 and 9. The rest exist empty, because a widening is a
//! smaller change than a reordering.

/// A whole `.nsi` file.
#[derive(Clone, Debug, Default)]
pub struct Module {
    /// Always emitted, always first, defaults true (§15.16).
    pub unicode: bool,
    pub defines: Vec<Define>,
    pub includes: Vec<String>,
    pub inits: Vec<Instruction>,
    pub attributes: Vec<Instruction>,
    pub mui: Vec<Instruction>,
    pub vars: Vec<String>,
    pub functions: Vec<Function>,
    pub sections: Vec<SectionItem>,
}

impl Module {
    pub fn new() -> Self {
        Module {
            unicode: true,
            ..Module::default()
        }
    }
}

#[derive(Clone, Debug)]
pub struct Define {
    pub name: String,
    pub value: Arg,
}

#[derive(Clone, Debug)]
pub struct Function {
    pub name: String,
    pub body: Vec<Item>,
}

#[derive(Clone, Debug)]
pub enum SectionItem {
    Section(Section),
    Group(SectionGroup),
}

#[derive(Clone, Debug)]
pub struct SectionGroup {
    pub name: String,
    /// `SectionGroup /e` — expanded in the components tree.
    pub expanded: bool,
    pub sections: Vec<Section>,
}

#[derive(Clone, Debug)]
pub struct Section {
    pub name: String,
    /// `Section /o` — unselected by default.
    pub optional: bool,
    pub body: Vec<Item>,
}

/// Labels are IR nodes rather than strings spliced by the lowering code (§8-2).
/// Phase 2 replaces this flat list with a block graph; the emitter's contract
/// does not change when it does.
#[derive(Clone, Debug)]
pub enum Item {
    Instruction(Instruction),
    Label(String),
}

/// `name` is a `String` and not `&'static str` because macro invocations
/// (`${WinVerGetMajor}`) and plugin calls (`System::Call`) are composed rather
/// than looked up whole.
#[derive(Clone, Debug)]
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

/// Whether the emitter quotes and escapes an argument — the data-versus-syntax
/// split (§12). Registers, integers, `IntOp` opcodes, flag lists and branch
/// targets are NSIS *syntax*; a section name and a `DetailPrint` message are
/// *data*, and only data is escaped.
#[derive(Clone, Debug)]
pub enum Arg {
    /// Data. Quoted, and every `$` doubled (§15.1).
    Str(String),
    /// Data in a path position: `/` is normalised to `\` before quoting, since
    /// NSIS does not accept a forward slash everywhere (§5).
    Path(String),
    /// Syntax. Emitted exactly as written.
    Raw(String),
}

impl Arg {
    pub fn str(value: impl Into<String>) -> Self {
        Arg::Str(value.into())
    }

    pub fn path(value: impl Into<String>) -> Self {
        Arg::Path(value.into())
    }

    pub fn raw(value: impl Into<String>) -> Self {
        Arg::Raw(value.into())
    }
}
