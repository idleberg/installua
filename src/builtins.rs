//! The exposed surface: NSIS constants, instructions and predicates.
//!
//! **This is a seed, not the overlay.** §15.23's `Instruction` struct is joined
//! at build time from a `-CMDHELP` generator and a hand-written half, and that
//! is Phase 5. What is here is the subset Phase 2's mechanisms need in order to
//! be exercised at all — a type has to come from somewhere, and a `bool` has to
//! come from a predicate — written in the shape the generated table will have
//! so that widening it is a row rather than a redesign (§15.17).
//!
//! Every name outside this table is [`crate::diag::Code::NotYetImplemented`],
//! which is the honest edge of the vertical slice and counted by `installua
//! coverage` (PLAN §0).

use crate::types::Ty;

/// Whether a builtin is a line of output or a branch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// One line, with the output register first when there is one.
    Instruction,
    /// A branching instruction: `IfFileExists f <then> <else>`. §15.20 makes
    /// these ordinary `bool`-valued calls rather than a second condition shape,
    /// so `if fileExists(p) then` fuses and `local ok = fileExists(p)`
    /// materialises, from one definition.
    Predicate,
}

#[derive(Clone, Copy, Debug)]
pub struct Param {
    pub ty: Ty,
    /// A path position: `/` is normalised to `\` (§5). This is the overlay's
    /// `kind: Path` (§15.23), and the reason the expression lowerer never has to
    /// know where its result is going.
    pub path: bool,
}

const fn data(ty: Ty) -> Param {
    Param { ty, path: false }
}

const fn path() -> Param {
    Param {
        ty: Ty::Str,
        path: true,
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Builtin {
    pub installua: &'static str,
    pub nsis: &'static str,
    pub kind: Kind,
    pub params: &'static [Param],
    /// `Some` when the instruction writes a result. For a [`Kind::Predicate`]
    /// this is always `bool` and the branch supplies it.
    pub returns: Option<Ty>,
    /// `IfErrors` **clears** the flag it reads — verified under wine (§15.20) —
    /// so it is a consuming read rather than a query, and eliminating it when
    /// its result is unused would silently break error handling.
    pub pure: bool,
}

const fn instruction(
    installua: &'static str,
    nsis: &'static str,
    params: &'static [Param],
) -> Builtin {
    Builtin {
        installua,
        nsis,
        kind: Kind::Instruction,
        params,
        returns: None,
        pure: true,
    }
}

const fn predicate(
    installua: &'static str,
    nsis: &'static str,
    params: &'static [Param],
) -> Builtin {
    Builtin {
        installua,
        nsis,
        kind: Kind::Predicate,
        params,
        returns: Some(Ty::Bool),
        pure: true,
    }
}

pub const BUILTINS: &[Builtin] = &[
    instruction("detailPrint", "DetailPrint", &[data(Ty::Str)]),
    instruction("setOutPath", "SetOutPath", &[path()]),
    instruction("createDirectory", "CreateDirectory", &[path()]),
    instruction("sleep", "Sleep", &[data(Ty::nonneg())]),
    instruction("clearErrors", "ClearErrors", &[]),
    Builtin {
        installua: "string.len",
        nsis: "StrLen",
        kind: Kind::Instruction,
        params: &[data(Ty::Str)],
        // `StrLen` is non-negative by construction, which is what makes
        // `string.len(p) // 2` a bare `IntOp` with no sign fixup (§15.14).
        returns: Some(Ty::nonneg()),
        pure: true,
    },
    predicate("fileExists", "IfFileExists", &[path()]),
    predicate("silent", "IfSilent", &[]),
    Builtin {
        installua: "errors",
        nsis: "IfErrors",
        kind: Kind::Predicate,
        params: &[],
        returns: Some(Ty::Bool),
        pure: false,
    },
];

pub fn lookup(name: &str) -> Option<&'static Builtin> {
    BUILTINS.iter().find(|builtin| builtin.installua == name)
}

/// An NSIS constant: an ordinary read-only name here, joined with `..` rather
/// than interpolated into a literal (§15.1). `$INSTDIR` inside a string is the
/// single most common NSIS habit this language does not have.
#[derive(Clone, Copy, Debug)]
pub struct Constant {
    pub installua: &'static str,
    pub nsis: &'static str,
    pub ty: Ty,
}

const fn constant(name: &'static str, ty: Ty) -> Constant {
    Constant {
        installua: name,
        // Every one of these has the same spelling on both sides, which is the
        // point: the name a user already knows is the name they write.
        nsis: name,
        ty,
    }
}

pub const CONSTANTS: &[Constant] = &[
    constant("INSTDIR", Ty::Str),
    constant("OUTDIR", Ty::Str),
    constant("PROGRAMFILES", Ty::Str),
    constant("PROGRAMFILES64", Ty::Str),
    constant("COMMONFILES", Ty::Str),
    constant("DESKTOP", Ty::Str),
    constant("STARTMENU", Ty::Str),
    constant("SMPROGRAMS", Ty::Str),
    constant("APPDATA", Ty::Str),
    constant("LOCALAPPDATA", Ty::Str),
    constant("TEMP", Ty::Str),
    constant("WINDIR", Ty::Str),
    constant("SYSDIR", Ty::Str),
    constant("EXEDIR", Ty::Str),
    constant("EXEPATH", Ty::Str),
    constant("EXEFILE", Ty::Str),
    constant("PLUGINSDIR", Ty::Str),
    constant("LANGUAGE", Ty::nonneg()),
];

pub fn constant_named(name: &str) -> Option<&'static Constant> {
    CONSTANTS.iter().find(|c| c.installua == name)
}

/// Suggestions for a name that resolved to nothing. Every rejection names its
/// replacement (PLAN §2), and for a misspelling the replacement is the spelling.
pub fn nearest(name: &str) -> Option<&'static str> {
    let lowered = name.to_lowercase();
    BUILTINS
        .iter()
        .map(|b| b.installua)
        .chain(CONSTANTS.iter().map(|c| c.installua))
        .find(|candidate| candidate.to_lowercase() == lowered && *candidate != name)
}
