//! NSIS constants, and the lookup the lowerer calls.
//!
//! **The instructions used to live here and no longer do.** Phase 2 needed a
//! seed table before the `-CMDHELP` join existed; Phase 5 built the join, and a
//! seed that outlives the thing it seeded is just a second source of truth —
//! §15.23's exact failure, where a consumer reading two tables sees half an
//! entry. [`lookup`] now answers from [`crate::table`], so one `exposed(…)` row
//! makes a command callable, completable and counted at once.
//!
//! What is still here is [`CONSTANTS`], and it stays for a reason rather than
//! by omission: `$INSTDIR` and `HKLM` have no `-CMDHELP` line at all. They are
//! not commands, so there is no generated half to join against and nothing is
//! written twice.
//!
//! Every name outside both is [`crate::diag::Code::NotYetImplemented`], which
//! is the honest edge of the vertical slice and counted by `installua coverage`
//! (PLAN §0).

use crate::table::{self, Class, Instruction};
use crate::types::Ty;

pub fn lookup(name: &str) -> Option<&'static Instruction> {
    table::by_installua(name).filter(|entry| entry.class == Class::Exposed)
}

/// An NSIS constant: an ordinary read-only name here, joined with `..` rather
/// than interpolated into a literal (§15.1). `$INSTDIR` inside a string is the
/// single most common NSIS habit this language does not have.
#[derive(Clone, Copy, Debug)]
pub struct Constant {
    pub installua: &'static str,
    pub nsis: &'static str,
    pub ty: Ty,
    /// Whether the name carries a `$`. `$INSTDIR` is a variable the installer
    /// expands at run time; `HKLM` is a bare keyword that only `Reg*` accepts,
    /// and writing `$HKLM` instead produces warning 6000 and a silently wrong
    /// installer (§15.1).
    pub sigil: bool,
    /// Whether assigning to it is legal. `$INSTDIR` is a variable the user is
    /// *expected* to write — `.onInit` reading a prior install location and
    /// setting it is the canonical shape — while `$EXEDIR` is a fact about the
    /// machine and assigning to it is a mistake NSIS accepts silently (§13).
    pub writable: bool,
}

const fn constant(name: &'static str, ty: Ty) -> Constant {
    Constant {
        installua: name,
        // Every one of these has the same spelling on both sides, which is the
        // point: the name a user already knows is the name they write.
        nsis: name,
        ty,
        sigil: true,
        writable: false,
    }
}

/// A constant the user may assign to.
const fn writable(name: &'static str, ty: Ty) -> Constant {
    Constant {
        writable: true,
        ..constant(name, ty)
    }
}

/// A registry root: a keyword, not a variable.
const fn root(name: &'static str) -> Constant {
    Constant {
        installua: name,
        nsis: name,
        ty: Ty::Handle,
        sigil: false,
        writable: false,
    }
}

impl Constant {
    /// The argument this constant becomes.
    pub fn arg(&self) -> crate::ir::Arg {
        if self.sigil {
            crate::ir::Arg::var(format!("${}", self.nsis))
        } else {
            crate::ir::Arg::raw(self.nsis)
        }
    }
}

pub const CONSTANTS: &[Constant] = &[
    writable("INSTDIR", Ty::Str),
    writable("OUTDIR", Ty::Str),
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
    root("HKLM"),
    root("HKCU"),
    root("HKCR"),
    root("HKU"),
    root("HKCC"),
    root("SHCTX"),
];

/// A name the compiler owns: neither a constant nor a register, because its
/// read and its write are *instructions*. `currentInstType` is `GetCurInstType`
/// and `SetCurInstType`, and `instTypes` is a table addressed by string — so
/// neither may become a `Var`, which is what an unbound assignment target
/// otherwise does (§13).
pub fn owned(name: &str) -> bool {
    matches!(name, "currentInstType" | "instTypes")
}

pub fn constant_named(name: &str) -> Option<&'static Constant> {
    CONSTANTS.iter().find(|c| c.installua == name)
}

/// Suggestions for a name that resolved to nothing. Every rejection names its
/// replacement (PLAN §2), and for a misspelling the replacement is the spelling.
pub fn nearest(name: &str) -> Option<&'static str> {
    let lowered = name.to_lowercase();
    table::table()
        .iter()
        .filter(|entry| entry.class == Class::Exposed)
        .filter_map(|entry| entry.installua)
        .chain(CONSTANTS.iter().map(|c| c.installua))
        .find(|candidate| candidate.to_lowercase() == lowered && *candidate != name)
}
