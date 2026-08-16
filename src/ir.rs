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
//! Phase 1 filled 1, 5 and 9; Phase 2 adds 7 and 8. The rest exist empty,
//! because a widening is a smaller change than a reordering.

use crate::cfg;

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
    /// Globals, declared by assignment (§15.24), collected during lowering and
    /// emitted before the first body that touches them (§12).
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

    /// Every body in the module, for a caller that wants to assert an IR
    /// property rather than read emitted text (§14 tier 0).
    pub fn bodies(&self) -> impl Iterator<Item = (&str, &cfg::Body)> {
        let functions = self
            .functions
            .iter()
            .map(|f| (f.name.as_str(), &f.body))
            .collect::<Vec<_>>();
        let mut all = functions;
        for item in &self.sections {
            match item {
                SectionItem::Section(section) => all.push((section.name.as_str(), &section.body)),
                SectionItem::Group(group) => {
                    for section in &group.sections {
                        all.push((section.name.as_str(), &section.body));
                    }
                }
            }
        }
        all.into_iter()
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
    pub body: cfg::Body,
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
    pub body: cfg::Body,
}

/// A laid-out body: what [`crate::layout`] produces from a CFG, and the only
/// thing the emitter ever sees. Labels are IR nodes rather than strings spliced
/// by the lowering code (§8-2), and by the time one exists the layout pass has
/// already decided which blocks need one at all.
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

/// One span of an argument's text.
///
/// The split exists because a `$` reaching the output means two opposite things
/// (§15.1): in [`Piece::Text`] it is five dollars and gets doubled, and in
/// [`Piece::Var`] it is a register and must not be touched. Concatenation of
/// constants is therefore free — `"into " .. INSTDIR` is one quoted argument
/// and no instruction at all — which is the whole reason the string model is
/// worth having.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Piece {
    /// Literal text. Escaped on the way out.
    Text(String),
    /// An NSIS variable or define, already in NSIS spelling: `$0`, `$INSTDIR`,
    /// `${VERSION}`. Spliced in bare.
    Var(String),
}

/// Whether the emitter quotes and escapes an argument — the data-versus-syntax
/// split (§12). Registers *as destinations*, `IntOp` opcodes, flag lists and
/// branch targets are NSIS **syntax**; a section name and a `DetailPrint`
/// message are **data**, and only data is escaped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Arg {
    Data {
        pieces: Vec<Piece>,
        /// A path position: `/` in the *text* pieces is normalised to `\`,
        /// since NSIS does not accept a forward slash everywhere (§5). A
        /// register's contents are runtime data and are left alone.
        path: bool,
    },
    /// Syntax. Emitted exactly as written, unquoted.
    Raw(String),
}

impl Arg {
    pub fn str(value: impl Into<String>) -> Self {
        Arg::data(vec![Piece::Text(value.into())])
    }

    pub fn path(value: impl Into<String>) -> Self {
        Arg::Data {
            pieces: vec![Piece::Text(value.into())],
            path: true,
        }
    }

    pub fn int(value: i64) -> Self {
        Arg::str(value.to_string())
    }

    /// A register or constant read: `$0`, `$INSTDIR`.
    pub fn var(nsis: impl Into<String>) -> Self {
        Arg::data(vec![Piece::Var(nsis.into())])
    }

    pub fn raw(value: impl Into<String>) -> Self {
        Arg::Raw(value.into())
    }

    pub fn data(pieces: Vec<Piece>) -> Self {
        Arg::Data {
            pieces,
            path: false,
        }
    }

    pub fn data_path(pieces: Vec<Piece>) -> Self {
        Arg::Data { pieces, path: true }
    }

    /// The same argument in a path position. Called at the parameter boundary,
    /// so the overlay decides pathness and the expression lowerer does not have
    /// to know where its result is going.
    pub fn into_path(self) -> Self {
        match self {
            Arg::Data { pieces, .. } => Arg::Data { pieces, path: true },
            raw => raw,
        }
    }

    /// The whole argument as literal text, when nothing in it is a register.
    /// Constant folding asks this; nothing else should.
    pub fn as_text(&self) -> Option<String> {
        match self {
            Arg::Data { pieces, .. } => pieces
                .iter()
                .map(|piece| match piece {
                    Piece::Text(text) => Some(text.as_str()),
                    Piece::Var(_) => None,
                })
                .collect::<Option<Vec<_>>>()
                .map(|parts| parts.concat()),
            Arg::Raw(_) => None,
        }
    }

    /// Concatenation, flattened and with adjacent text runs merged, so
    /// `"a" .. "b" .. INSTDIR` is two pieces rather than three.
    pub fn concat(self, other: Arg) -> Arg {
        let (mut pieces, path) = match self {
            Arg::Data { pieces, path } => (pieces, path),
            Arg::Raw(text) => (vec![Piece::Text(text)], false),
        };
        let (rhs, rhs_path) = match other {
            Arg::Data { pieces, path } => (pieces, path),
            Arg::Raw(text) => (vec![Piece::Text(text)], false),
        };
        for piece in rhs {
            match (pieces.last_mut(), piece) {
                (Some(Piece::Text(tail)), Piece::Text(text)) => tail.push_str(&text),
                (_, piece) => pieces.push(piece),
            }
        }
        Arg::Data {
            pieces,
            path: path || rhs_path,
        }
    }
}
