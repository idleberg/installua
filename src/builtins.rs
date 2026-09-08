//! NSIS constants, and the lookup the lowerer calls.
//!
//! **The instructions used to live here and no longer do.** Phase 2 needed a
//! seed table before the `-CMDHELP` join existed; Phase 5 built the join, and a
//! seed that outlives the thing it seeded is just a second source of truth —
//! the exact failure the join exists to prevent, where a consumer reading two
//! tables sees half an
//! entry. [`lookup`] now answers from [`crate::table`], so one `exposed(…)` row
//! makes a command callable, completable and counted at once.
//!
//! What is still here is [`CONSTANTS`], and it stays for a reason rather than
//! by omission: `$INSTDIR` and `HKLM` have no `-CMDHELP` line at all. They are
//! not commands, so there is no generated half to join against and nothing is
//! written twice.
//!
//! Every name outside both is [`crate::diag::Code::NotYetImplemented`], which
//! is the honest edge of the vertical slice and counted by `installua
//! coverage`.

use crate::table::{self, Class, Instruction};
use crate::types::Ty;

pub fn lookup(name: &str) -> Option<&'static Instruction> {
    table::by_installua(name).filter(|entry| entry.class == Class::Exposed)
}

/// An NSIS constant: an ordinary read-only name here, joined with `..` rather
/// than interpolated into a literal. `$INSTDIR` inside a string is the single
/// most common NSIS habit this language does not have.
#[derive(Clone, Copy, Debug)]
pub struct Constant {
    pub installua: &'static str,
    pub nsis: &'static str,
    pub ty: Ty,
    /// Whether the name carries a `$`. `$INSTDIR` is a variable the installer
    /// expands at run time; `HKLM` is a bare keyword that only `Reg*` accepts,
    /// and writing `$HKLM` instead produces warning 6000 and a silently wrong
    /// installer.
    pub sigil: bool,
    /// Whether assigning to it is legal. `$INSTDIR` is a variable the user is
    /// *expected* to write — `.onInit` reading a prior install location and
    /// setting it is the canonical shape — while `$EXEDIR` is a fact about the
    /// machine and assigning to it is a mistake NSIS accepts silently.
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

/// The full predefined set, and full on purpose: these are spellings `makensis`
/// hard-codes, so a name missing here is not "not yet implemented" but a name
/// the user can write in NSIS and cannot write here. `$DOCUMENTS` and
/// `$HKLM64` were both missing that way. The lists are
/// `CEXEBuild::CEXEBuild` in `Source/build.cpp` (the variables and the shell
/// folders) and `ParseRegRootKey` in `Source/script.cpp` (the roots).
pub const CONSTANTS: &[Constant] = &[
    writable("INSTDIR", Ty::Str),
    writable("OUTDIR", Ty::Str),
    constant("PROGRAMFILES", Ty::Str),
    constant("PROGRAMFILES32", Ty::Str),
    constant("PROGRAMFILES64", Ty::Str),
    constant("COMMONFILES", Ty::Str),
    constant("COMMONFILES32", Ty::Str),
    constant("COMMONFILES64", Ty::Str),
    constant("DESKTOP", Ty::Str),
    constant("STARTMENU", Ty::Str),
    constant("SMPROGRAMS", Ty::Str),
    constant("SMSTARTUP", Ty::Str),
    constant("QUICKLAUNCH", Ty::Str),
    constant("DOCUMENTS", Ty::Str),
    constant("MUSIC", Ty::Str),
    constant("PICTURES", Ty::Str),
    constant("VIDEOS", Ty::Str),
    constant("FAVORITES", Ty::Str),
    constant("SENDTO", Ty::Str),
    constant("RECENT", Ty::Str),
    constant("NETHOOD", Ty::Str),
    constant("PRINTHOOD", Ty::Str),
    constant("FONTS", Ty::Str),
    constant("TEMPLATES", Ty::Str),
    constant("ADMINTOOLS", Ty::Str),
    constant("INTERNET_CACHE", Ty::Str),
    constant("COOKIES", Ty::Str),
    constant("HISTORY", Ty::Str),
    constant("PROFILE", Ty::Str),
    constant("RESOURCES", Ty::Str),
    constant("RESOURCES_LOCALIZED", Ty::Str),
    constant("CDBURN_AREA", Ty::Str),
    constant("APPDATA", Ty::Str),
    constant("LOCALAPPDATA", Ty::Str),
    // The `USER*` and `COMMON*` halves of the folders above, naming one side
    // outright instead of asking `setShellVarContext` which side is current.
    constant("USERAPPDATA", Ty::Str),
    constant("USERLOCALAPPDATA", Ty::Str),
    constant("USERTEMPLATES", Ty::Str),
    constant("USERSTARTMENU", Ty::Str),
    constant("USERSMPROGRAMS", Ty::Str),
    constant("USERDESKTOP", Ty::Str),
    constant("COMMONLOCALAPPDATA", Ty::Str),
    constant("COMMONPROGRAMDATA", Ty::Str),
    constant("COMMONTEMPLATES", Ty::Str),
    constant("COMMONSTARTMENU", Ty::Str),
    constant("COMMONSMPROGRAMS", Ty::Str),
    constant("COMMONDESKTOP", Ty::Str),
    constant("TEMP", Ty::Str),
    constant("WINDIR", Ty::Str),
    constant("SYSDIR", Ty::Str),
    constant("EXEDIR", Ty::Str),
    constant("EXEPATH", Ty::Str),
    constant("EXEFILE", Ty::Str),
    constant("PLUGINSDIR", Ty::Str),
    constant("CMDLINE", Ty::Str),
    constant("LANGUAGE", Ty::nonneg()),
    // The installer's own window, and the only handle a program can name
    // without having created the thing it addresses: `getDlgItem(HWNDPARENT,
    // 2)` reaches the Cancel button MUI2 drew, not one of ours. Read-only for
    // the same reason as `$EXEDIR` — it is a fact about the running installer,
    // and NSIS accepts a write to it silently.
    constant("HWNDPARENT", Ty::Handle),
    // `-CMDHELP` spells the roots `HKLM[32|64]`, and the `ANY` third of each
    // family it leaves out entirely — `makensis` takes all of them, so the
    // list here is `ParseRegRootKey`'s and not the usage line's. The long
    // aliases (`HKEY_LOCAL_MACHINE` and friends) are the same seven roots
    // spelled twice and stay out: one name per thing.
    root("HKLM"),
    root("HKLM32"),
    root("HKLM64"),
    root("HKLMANY"),
    root("HKCU"),
    root("HKCU32"),
    root("HKCU64"),
    root("HKCUANY"),
    root("HKCR"),
    root("HKCR32"),
    root("HKCR64"),
    root("HKCRANY"),
    root("HKU"),
    root("HKCC"),
    root("HKDD"),
    root("HKPD"),
    root("SHCTX"),
    root("SHCTX32"),
    root("SHCTX64"),
    root("SHCTXANY"),
];

/// A name the compiler owns: neither a constant nor a register, because its
/// read and its write are *instructions*. `currentInstType` is `GetCurInstType`
/// and `SetCurInstType`, and `instTypes` is a table addressed by string — so
/// neither may become a `Var`, which is what an unbound assignment target
/// otherwise does.
pub fn owned(name: &str) -> bool {
    matches!(name, "currentInstType" | "instTypes")
}

pub fn constant_named(name: &str) -> Option<&'static Constant> {
    CONSTANTS.iter().find(|c| c.installua == name)
}

/// Suggestions for a name that resolved to nothing. Every rejection names its
/// replacement, and for a misspelling the replacement is the spelling.
pub fn nearest(name: &str) -> Option<&'static str> {
    let lowered = name.to_lowercase();
    table::table()
        .iter()
        .filter(|entry| entry.class == Class::Exposed)
        .filter_map(|entry| entry.installua)
        .chain(CONSTANTS.iter().map(|c| c.installua))
        .find(|candidate| candidate.to_lowercase() == lowered && *candidate != name)
}
