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

/// Whether a spliced variable (`$INSTDIR`) is one of these and names a
/// directory — so a `/`-leading literal joined straight after it is a path
/// wherever it ends up, and is normalised as one. `$EXEPATH`, `$EXEFILE` and
/// `$CMDLINE` are strings but no directory; a user global might be one, and the
/// compiler cannot tell.
pub fn is_directory(var: &str) -> bool {
    var.strip_prefix('$').is_some_and(|name| {
        !matches!(name, "EXEPATH" | "EXEFILE" | "CMDLINE")
            && CONSTANTS
                .iter()
                .any(|c| c.sigil && c.ty == Ty::Str && c.nsis == name)
    })
}

impl Constant {
    /// A registry root. Not merely `!sigil`, which a message number is too.
    pub fn is_root(&self) -> bool {
        !self.sigil && self.ty == Ty::Handle
    }

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
    constants().find(|c| c.installua == name)
}

/// Suggestions for a name that resolved to nothing. Every rejection names its
/// replacement, and for a misspelling the replacement is the spelling.
pub fn nearest(name: &str) -> Option<&'static str> {
    let lowered = name.to_lowercase();
    table::table()
        .iter()
        .filter(|entry| entry.class == Class::Exposed)
        .filter_map(|entry| entry.installua)
        .chain(constants().map(|c| c.installua))
        .find(|candidate| candidate.to_lowercase() == lowered && *candidate != name)
}

/// `WinMessages.nsh`'s names, `WM_SETTEXT` and the rest, as the numbers the
/// header defines them to be. A name is its number in the output, so there is
/// no `!include` to order and nothing for a `raw` block to redefine.
///
/// Flat beside `HWNDPARENT` rather than under a namespace: every one is an
/// upper-case prefixed name, none clashes with [`CONSTANTS`], and a `local` of
/// the same name shadows it as it shadows those. `WinCore.nsh` could not be
/// read this way — it defines `HKLM` as a number.
///
/// The eleven `${_NSIS_DEFAW}` names, `LVM_GETITEMTEXT` and friends, are left
/// out: which of the `A` and `W` twins they are depends on `unicode`, and the
/// twins are here under their own names.
const MESSAGES: &str = include_str!("../tables/winmessages-3.12.txt");

fn messages() -> &'static [Constant] {
    static PARSED: std::sync::OnceLock<Vec<Constant>> = std::sync::OnceLock::new();
    PARSED.get_or_init(|| {
        MESSAGES
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .filter_map(|line| line.split_once(' '))
            .map(|(name, value)| Constant {
                installua: name,
                nsis: value,
                ty: if value.starts_with('-') {
                    Ty::int()
                } else {
                    Ty::nonneg()
                },
                sigil: false,
                writable: false,
            })
            .collect()
    })
}

/// [`CONSTANTS`], then the message names.
pub fn constants() -> impl Iterator<Item = &'static Constant> {
    CONSTANTS.iter().chain(messages())
}

/// The message snapshot, regenerated: `tests/headers.rs` under
/// `UPDATE_SNAPSHOTS`.
///
/// Four shapes of `!define` carry a number — a literal, `/math ${X} + n`, an
/// alias `${X}`, and any of those behind `/ifndef` — and the rest (`SYSSTRUCT_*`
/// layouts, the `_NSIS_DEFAW` plumbing) are skipped. A reference to a name not
/// yet read is an error rather than a skip, so a reordered header cannot drop
/// a name quietly.
pub fn scan_messages(root: &std::path::Path) -> Result<String, String> {
    let path = root.join("Include").join("WinMessages.nsh");
    let text =
        std::fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut values = std::collections::BTreeMap::<String, i64>::new();
    for line in text.lines() {
        let code = line.split([';', '#']).next().unwrap_or_default();
        let mut tokens = code.split_whitespace().peekable();
        if !tokens
            .next()
            .is_some_and(|t| t.eq_ignore_ascii_case("!define"))
        {
            continue;
        }
        let mut math = false;
        while let Some(flag) = tokens.next_if(|t| t.starts_with('/')) {
            math |= flag.eq_ignore_ascii_case("/math");
        }
        let Some(name) = tokens.next().filter(|name| {
            name.starts_with(|c: char| c.is_ascii_uppercase())
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        }) else {
            continue;
        };
        let rest: Vec<&str> = tokens.collect();
        let operand = |token: &str| -> Result<Option<i64>, String> {
            if let Some(reference) = token.strip_prefix("${").and_then(|t| t.strip_suffix('}')) {
                return values
                    .get(reference)
                    .copied()
                    .map(Some)
                    .ok_or(format!("`{name}` reads `{reference}` before it is defined"));
            }
            let (negative, digits) = match token.strip_prefix('-') {
                Some(digits) => (true, digits),
                None => (false, token),
            };
            let parsed = match digits.strip_prefix("0x").or(digits.strip_prefix("0X")) {
                Some(hex) => i64::from_str_radix(hex, 16).ok(),
                None => digits.parse().ok(),
            };
            Ok(parsed.map(|value| if negative { -value } else { value }))
        };
        let value = match (math, rest.as_slice()) {
            (true, [lhs, "+", rhs]) => match (operand(lhs)?, operand(rhs)?) {
                (Some(lhs), Some(rhs)) => lhs + rhs,
                _ => return Err(format!("`{name}`: `/math` on something not a number")),
            },
            (true, _) => return Err(format!("`{name}`: a `/math` this does not read")),
            (false, [token]) => match operand(token)? {
                Some(value) => value,
                None => continue,
            },
            (false, _) => continue,
        };
        values.insert(name.to_string(), value);
    }

    let mut out = String::from(MESSAGES_HEADER);
    for (name, value) in values {
        out.push_str(&format!("{name} {value}\n"));
    }
    Ok(out)
}

const MESSAGES_HEADER: &str = "\
# The numbers `WinMessages.nsh` defines, as `builtins::constants` reads them.
#
# One `NAME value` per line, sorted, the value in decimal. Regenerate with:
#
#     NSISDIR=\"...\" UPDATE_SNAPSHOTS=1 cargo test --test headers

";
