//! The hand-written half of §15.23's table: everything `-CMDHELP` cannot say.
//!
//! One row per NSIS command, keyed by the NSIS name. What lives here is
//! judgement — the Installua spelling, the census class, the type of each
//! position and which positions are paths. What does not live here is anything
//! `makensis` already prints: arity, optionality, flag names and enum members
//! all come from [`super::generated`], because transcribing them is how a table
//! drifts (§14).
//!
//! **Every command is classified, including the ones that say no.** A command
//! with no row is a census failure rather than a silent omission, and that is
//! the whole mechanism: a new NSIS version adds a command, the snapshot refresh
//! moves it into `generated.rs`, and the census goes red until somebody puts it
//! in a bucket — including the bucket that means "not this, and here is why".
//!
//! Two things that look like they belong here and do not. The `Class::Rejected`
//! text is about the *NSIS* name and is read by the census; the retired-
//! instruction diagnostic is keyed on the Installua spelling a user would have
//! typed (`strCmp`, `intOp`) and lives in [`crate::retired`], because they
//! answer two different questions — "what bucket is `StrCmp` in" and "what do I
//! write instead". `StrCmp` is therefore a `LoweringTarget` here *and* a
//! retired row there, which is not a contradiction: the emitter needs its
//! shape and the user needs the replacement.

use super::{Class, Kind};
use crate::types::Ty;

/// The hand-written half of one parameter, positional against the skeleton's
/// list. The census checks the lengths agree, which is what stops an annotation
/// sliding one position left when NSIS adds an argument.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ann {
    pub ty: Ty,
    pub kind: Kind,
    /// The token the emitter writes when the caller omitted this position and a
    /// *later* one still has to be emitted.
    ///
    /// `FileSeek handle offset [mode] [$(user_var: new position)]` is the only
    /// command in the table that needs it, and it needs it badly: NSIS parses
    /// `FileSeek $1 0 $0` with `$0` as the **mode** and rejects it, so an author
    /// who wants the new position without naming a mode cannot be served by
    /// leaving the position out. `SET` is what NSIS itself uses when `mode` is
    /// absent, so supplying it changes nothing about what the line does.
    ///
    /// Only ever read for an optional input that precedes an output; a trailing
    /// optional is simply not emitted, which is what "optional" already meant.
    pub fill: Option<&'static str>,
}

pub struct Row {
    pub nsis: &'static str,
    /// The Installua source that emits this command, and the reason §14 calls
    /// the granular tests "impossible to omit": the census requires one on
    /// every `Exposed` row, and `tests/overlay.rs` compiles all of them into a
    /// single golden. Adding an instruction without its test is therefore
    /// unrepresentable rather than merely discouraged — which is what makes
    /// PLAN's Phase 6 data entry instead of a debate.
    pub example: Option<&'static str>,
    /// The Installua spelling, when there is one. An `Attribute` carries its
    /// *field path* — `versionInfo.product` — because that is where a user
    /// writes it, and a `Directive` carries nothing at all.
    pub installua: Option<&'static str>,
    pub class: Class,
    pub params: &'static [Ann],
    /// Mutually exclusive option sets: `File`'s `/oname=` branch against its
    /// repeated-filespec branch. The error names both spellings (§15.23).
    pub conflicts: &'static [&'static [&'static str]],
    /// A branching instruction read as an ordinary `bool`-valued call (§15.20):
    /// `if fileExists(p) then` fuses into the branch and `local ok =
    /// fileExists(p)` materialises, from this one bit.
    ///
    /// It cannot be derived from [`Kind::Label`], which is the obvious guess:
    /// `MessageBox` has label positions and is not a predicate. The label kind
    /// says *the compiler fills this*; this says *the call answers a question*.
    pub predicate: bool,
}

const fn ann(ty: Ty, kind: Kind) -> Ann {
    Ann {
        ty,
        kind,
        fill: None,
    }
}

/// An annotation that also says what to write when the position is skipped.
const fn filled(ty: Ty, kind: Kind, fill: &'static str) -> Ann {
    Ann {
        fill: Some(fill),
        ..ann(ty, kind)
    }
}

const fn row(nsis: &'static str, installua: Option<&'static str>, class: Class) -> Row {
    Row {
        nsis,
        example: None,
        installua,
        class,
        params: &[],
        conflicts: &[],
        predicate: false,
    }
}

/// A callable, with one annotation per parameter. The census requires the two
/// halves to be the same length here and nowhere else: an `Exposed` row is the
/// only kind anything reads the types of.
const fn exposed(
    nsis: &'static str,
    installua: &'static str,
    params: &'static [Ann],
    example: &'static str,
) -> Row {
    Row {
        params,
        example: Some(example),
        ..row(nsis, Some(installua), Class::Exposed)
    }
}

/// An `Exposed` row whose call site is a question rather than a statement
/// (§15.20). The `Kind::Label` positions are the compiler's; the surface takes
/// the ones before them.
const fn predicate(
    nsis: &'static str,
    installua: &'static str,
    params: &'static [Ann],
    example: &'static str,
) -> Row {
    Row {
        predicate: true,
        ..exposed(nsis, installua, params, example)
    }
}

/// A field of one of the four blocks (§15.10, §15.26). The types come from the
/// block's own lowering rather than from here, because an attribute's value is
/// a Lua expression in a table and not an argument list.
const fn attribute(nsis: &'static str, field: &'static str) -> Row {
    row(nsis, Some(field), Class::Attribute)
}

/// Emitter-only: the compiler writes it, the user never does. The text says
/// what the user writes instead, which is what `installua coverage` prints
/// beside it.
const fn lowering(nsis: &'static str, instead: &'static str) -> Row {
    row(nsis, None, Class::LoweringTarget(instead))
}

/// The name is an Installua construct instead (§6).
const fn language(nsis: &'static str, instead: &'static str) -> Row {
    row(nsis, None, Class::Language(instead))
}

/// Deliberately unavailable, with the text that says so.
const fn rejected(nsis: &'static str, why: &'static str) -> Row {
    row(nsis, None, Class::Rejected(why))
}

/// A preprocessor command. §2 rules the whole preprocessor out of the surface,
/// so these carry no parameter model and no reason: the reason is §2.
const fn directive(nsis: &'static str) -> Row {
    row(nsis, None, Class::Directive)
}

/// Not yet done, with a one-line reason. The only honest backlog (§14).
const fn todo(nsis: &'static str, why: &'static str) -> Row {
    row(nsis, None, Class::Todo(why))
}

pub fn lookup(nsis: &str) -> Option<&'static Row> {
    ROWS.iter().find(|row| row.nsis == nsis)
}

/// In `-CMDHELP` order, so this file and `generated.rs` read side by side.
pub const ROWS: &[Row] = &[
    exposed(
        "Abort",
        "abort",
        &[ann(Ty::Str, Kind::Value)],
        "abort(\"stopped\")",
    ),
    todo(
        "AddBrandingImage",
        "the classic UI's appearance; MUI supersedes it, and §15.7's sequential-`!define` hazard is unruled",
    ),
    todo(
        "AddSize",
        "addresses a section by index: a real compile-time to install-time name binding (§13)",
    ),
    todo(
        "AutoCloseWindow",
        "addresses a window by handle; the `hwnd` surface wants nsDialogs designed first",
    ),
    todo(
        "BGFont",
        "the classic UI's appearance; MUI supersedes it, and §15.7's sequential-`!define` hazard is unruled",
    ),
    todo(
        "BGGradient",
        "the classic UI's appearance; MUI supersedes it, and §15.7's sequential-`!define` hazard is unruled",
    ),
    todo(
        "BrandingText",
        "the classic UI's appearance; MUI supersedes it, and §15.7's sequential-`!define` hazard is unruled",
    ),
    todo(
        "BringToFront",
        "addresses a window by handle; the `hwnd` surface wants nsDialogs designed first",
    ),
    lowering("Call", "a call: `f(x)`"),
    rejected(
        "CallInstDLL",
        "a plugin is called as `plugin.method(…)` (§11)",
    ),
    attribute("Caption", "caption"),
    todo(
        "ChangeUI",
        "the classic UI's appearance; MUI supersedes it, and §15.7's sequential-`!define` hazard is unruled",
    ),
    exposed("ClearErrors", "clearErrors", &[], "clearErrors()"),
    todo(
        "ComponentText",
        "a classic-UI caption or button label; each needs a home in `installer {}` or `page {}` first",
    ),
    // The four rows that write *two* registers. A 64-bit value split across a
    // high and a low half is one number in every language that has one, and
    // Installua does not: §3 has no 64-bit type, so the halves stay halves and
    // the call binds both. `/ProductVersion` is not reachable — no row emits an
    // option yet — but it is optional, so the row is honest without it.
    exposed(
        "GetDLLVersion",
        "getDllVersion",
        &[
            ann(Ty::Str, Kind::Path),
            ann(Ty::nonneg(), Kind::Value),
            ann(Ty::nonneg(), Kind::Value),
        ],
        "local high, low = getDllVersion(INSTDIR .. \"/shell.dll\")\ndetailPrint(high .. \".\" .. low)",
    ),
    // The lowering is the same as `GetDLLVersion`'s and the row is not the
    // problem: the `Local` twin reads the *build* machine at compile time, so
    // its example needs a real PE carrying a version resource sitting in
    // `tests/fixtures`, and neither an `.ico` nor a shipped NSIS plugin has one
    // (both give *"error reading version info"*). A fixture that is not what it
    // claims to be is what that directory's README exists to forbid.
    todo(
        "GetDLLVersionLocal",
        "its example reads the build machine at compile time, and no fixture yet carries a \
         version resource",
    ),
    exposed(
        "GetFileTime",
        "getFileTime",
        &[
            ann(Ty::Str, Kind::Path),
            ann(Ty::nonneg(), Kind::Value),
            ann(Ty::nonneg(), Kind::Value),
        ],
        "local high, low = getFileTime(INSTDIR .. \"/app.exe\")\ndetailPrint(high .. \" \" .. low)",
    ),
    // `Kind::Value` rather than `Kind::Path`, and the difference is which
    // machine reads the string. §5 turns `/` into `\` because that is what
    // *Windows* wants at install time; this path is opened by `makensis` at
    // compile time, on whatever host is building, and a `\` there is a
    // filename character rather than a separator. `assets\icon.ico` is
    // *"error reading date"* on macOS and `assets/icon.ico` is fine on both.
    exposed(
        "GetFileTimeLocal",
        "getFileTimeLocal",
        &[
            ann(Ty::Str, Kind::Value),
            ann(Ty::nonneg(), Kind::Value),
            ann(Ty::nonneg(), Kind::Value),
        ],
        "local high, low = getFileTimeLocal(\"assets/icon.ico\")\ndetailPrint(high .. \" \" .. low)",
    ),
    exposed(
        "CopyFiles",
        "copyFiles",
        &[
            ann(Ty::Str, Kind::Path),
            ann(Ty::Str, Kind::Path),
            ann(Ty::nonneg(), Kind::Value),
        ],
        "copyFiles(INSTDIR .. \"/data\", INSTDIR .. \"/backup\")",
    ),
    attribute("CRCCheck", "crcCheck"),
    exposed(
        "CreateDirectory",
        "createDirectory",
        &[ann(Ty::Str, Kind::Path)],
        "createDirectory(INSTDIR .. \"/logs\")",
    ),
    todo(
        "CreateFont",
        "addresses a window by handle; the `hwnd` surface wants nsDialogs designed first",
    ),
    exposed(
        "CreateShortcut",
        "createShortcut",
        &[
            ann(Ty::Str, Kind::Path),
            ann(Ty::Str, Kind::Path),
            ann(Ty::Str, Kind::Value),
            ann(Ty::Str, Kind::Path),
            ann(Ty::nonneg(), Kind::Value),
            ann(Ty::nonneg(), Kind::Value),
            ann(Ty::Str, Kind::Enum),
            ann(Ty::Str, Kind::Enum),
            ann(Ty::Str, Kind::Value),
        ],
        "createShortcut(DESKTOP .. \"/App.lnk\", INSTDIR .. \"/app.exe\")",
    ),
    todo(
        "SetDatablockOptimize",
        "compile time and positional: it changes the `file` calls after it rather than \
         executing, so a call inside an `if` would be a lie",
    ),
    // The INI family spells `INI` as `Ini` — `deleteIniStr`, not
    // `deleteINIStr` — because every other name here is camel case over words
    // and an acronym is a word. `Sec` becomes `Section` for the same reason
    // `DetailPrint` did not become `detPrint`: `-CMDHELP` abbreviates, the
    // surface does not.
    exposed(
        "DeleteINISec",
        "deleteIniSection",
        &[ann(Ty::Str, Kind::Path), ann(Ty::Str, Kind::Value)],
        "deleteIniSection(INSTDIR .. \"/app.ini\", \"Settings\")",
    ),
    exposed(
        "DeleteINIStr",
        "deleteIniStr",
        &[
            ann(Ty::Str, Kind::Path),
            ann(Ty::Str, Kind::Value),
            ann(Ty::Str, Kind::Value),
        ],
        "deleteIniStr(INSTDIR .. \"/app.ini\", \"Settings\", \"Path\")",
    ),
    exposed(
        "DeleteRegKey",
        "deleteRegKey",
        &[ann(Ty::Handle, Kind::Value), ann(Ty::Str, Kind::Path)],
        "deleteRegKey(HKLM, \"Software/Example\")",
    ),
    exposed(
        "DeleteRegValue",
        "deleteRegValue",
        &[
            ann(Ty::Handle, Kind::Value),
            ann(Ty::Str, Kind::Path),
            ann(Ty::Str, Kind::Value),
        ],
        "deleteRegValue(HKLM, \"Software/Example\", \"Path\")",
    ),
    exposed(
        "Delete",
        "delete",
        &[ann(Ty::Str, Kind::Path)],
        "delete(INSTDIR .. \"/old.txt\")",
    ),
    exposed(
        "DetailPrint",
        "detailPrint",
        &[ann(Ty::Str, Kind::Value)],
        "detailPrint(\"installing\")",
    ),
    todo(
        "DirText",
        "a classic-UI caption or button label; each needs a home in `installer {}` or `page {}` first",
    ),
    rejected("DirShow", "NSIS itself reports this one as not working"),
    todo(
        "DirVar",
        "an installer-wide flag or mode: one overlay row each",
    ),
    todo(
        "DirVerify",
        "an installer-wide flag or mode: one overlay row each",
    ),
    exposed(
        "GetInstDirError",
        "getInstDirError",
        &[ann(Ty::nonneg(), Kind::Value)],
        "local why = getInstDirError()\ndetailPrint(\"instdir \" .. why)",
    ),
    todo(
        "AllowRootDirInstall",
        "an installer-wide flag or mode: one overlay row each",
    ),
    todo(
        "CheckBitmap",
        "the classic UI's appearance; MUI supersedes it, and §15.7's sequential-`!define` hazard is unruled",
    ),
    todo(
        "EnableWindow",
        "addresses a window by handle; the `hwnd` surface wants nsDialogs designed first",
    ),
    exposed(
        "EnumRegKey",
        "enumRegKey",
        &[
            ann(Ty::Str, Kind::Value),
            ann(Ty::Handle, Kind::Value),
            ann(Ty::Str, Kind::Path),
            ann(Ty::nonneg(), Kind::Value),
        ],
        "local key = enumRegKey(HKLM, \"Software/Example\", 0)\ndetailPrint(key)",
    ),
    exposed(
        "EnumRegValue",
        "enumRegValue",
        &[
            ann(Ty::Str, Kind::Value),
            ann(Ty::Handle, Kind::Value),
            ann(Ty::Str, Kind::Path),
            ann(Ty::nonneg(), Kind::Value),
        ],
        "local entry = enumRegValue(HKLM, \"Software/Example\", 0)\ndetailPrint(entry)",
    ),
    lowering(
        "Exch",
        "nothing: arguments and returns are the calling convention (§15.11)",
    ),
    // A command line is neither a path nor a value: `Kind::Path` turns the `/S`
    // in `setup.exe /S` into `\S`, and `Kind::Value` ships the forward slashes
    // of `INSTDIR .. "/app.exe"` to a program that will not find it. §5 gives
    // the surface one rule — write `/`, get `\` — and a position that is *part*
    // path has no way to obey it, so these wait for a spelling that separates
    // the program from its arguments.
    todo(
        "Exec",
        "one argument that is part path and part switches; §5's `/`-to-`\\` rule cannot apply to half a string",
    ),
    todo(
        "ExecWait",
        "one argument that is part path and part switches; §5's `/`-to-`\\` rule cannot apply to half a string",
    ),
    // `ExecShell [flags] verb file [parameters [showmode]]`: the optional
    // position is *first*, so `execShell("open", f)` would bind `"open"` to
    // `flags`. Trailing optionals work because they are positional and a
    // leading one is not — the same gap PHASE-6 records, met by a real row.
    todo(
        "ExecShell",
        "a *leading* optional parameter: positional arguments cannot say which optional they mean",
    ),
    todo(
        "ExecShellWait",
        "a *leading* optional parameter: positional arguments cannot say which optional they mean",
    ),
    exposed(
        "ExpandEnvStrings",
        "expandEnvStrings",
        &[ann(Ty::Str, Kind::Value), ann(Ty::Str, Kind::Value)],
        "local temp = expandEnvStrings(\"%TEMP%\")\ndetailPrint(temp)",
    ),
    todo(
        "FindWindow",
        "addresses a window by handle; the `hwnd` surface wants nsDialogs designed first",
    ),
    todo(
        "FindClose",
        "runtime directory iteration; `for … in glob` is unrolled on the build machine instead (§15.19)",
    ),
    todo(
        "FindFirst",
        "runtime directory iteration; `for … in glob` is unrolled on the build machine instead (§15.19)",
    ),
    todo(
        "FindNext",
        "runtime directory iteration; `for … in glob` is unrolled on the build machine instead (§15.19)",
    ),
    exposed(
        "File",
        "file",
        &[ann(Ty::Str, Kind::Path)],
        "file(\"assets/icon.ico\")",
    ),
    todo(
        "FileBufSize",
        "file surface beyond `file`/`delete`/`fileOpen`: one overlay row each",
    ),
    exposed(
        "FlushINI",
        "flushIni",
        &[ann(Ty::Str, Kind::Path)],
        "flushIni(INSTDIR .. \"/app.ini\")",
    ),
    todo(
        "ReserveFile",
        "file surface beyond `file`/`delete`/`fileOpen`: one overlay row each",
    ),
    exposed(
        "FileClose",
        "f:close",
        &[ann(Ty::Handle, Kind::Value)],
        "local f = fileOpen(INSTDIR .. \"/log.txt\", \"w\")\nf:close()",
    ),
    todo(
        "FileErrorText",
        "file surface beyond `file`/`delete`/`fileOpen`: one overlay row each",
    ),
    exposed(
        "FileOpen",
        "fileOpen",
        &[
            ann(Ty::Handle, Kind::Value),
            ann(Ty::Str, Kind::Path),
            ann(Ty::Str, Kind::Enum),
        ],
        "local f = fileOpen(INSTDIR .. \"/log.txt\", \"w\")\nf:close()",
    ),
    exposed(
        "FileRead",
        "f:read",
        &[
            ann(Ty::Handle, Kind::Value),
            ann(Ty::Str, Kind::Value),
            ann(Ty::nonneg(), Kind::Value),
        ],
        "local f = fileOpen(INSTDIR .. \"/log.txt\", \"r\")\nfor line in lines(f) do detailPrint(line) end\nf:close()",
    ),
    exposed(
        "FileWrite",
        "f:write",
        &[ann(Ty::Handle, Kind::Value), ann(Ty::Str, Kind::Value)],
        "local f = fileOpen(INSTDIR .. \"/log.txt\", \"w\")\nf:write(\"done\")\nf:close()",
    ),
    // `FileReadByte handle $(user_var: output)` puts its output **second**, and
    // the emitter used to write destinations first unconditionally: the row
    // would have emitted `FileReadByte $0 $1`, which is two registers, which
    // assembles. At run time it reads from the destination and writes over the
    // handle. `place` reads the position from here instead.
    exposed(
        "FileReadByte",
        "f:readByte",
        &[ann(Ty::Handle, Kind::Value), ann(Ty::nonneg(), Kind::Value)],
        "local f = fileOpen(INSTDIR .. \"/log.txt\", \"r\")\nlocal b = f:readByte()\ndetailPrint(\"byte \" .. b)\nf:close()",
    ),
    todo(
        "FileWriteByte",
        "file surface beyond `file`/`delete`/`fileOpen`: one overlay row each",
    ),
    // The output is in the *middle* here, which is why "outputs lead" and
    // "outputs trail" were never the two cases.
    exposed(
        "FileReadUTF16LE",
        "f:readUtf16Le",
        &[
            ann(Ty::Handle, Kind::Value),
            ann(Ty::Str, Kind::Value),
            ann(Ty::nonneg(), Kind::Value),
        ],
        "local f = fileOpen(INSTDIR .. \"/log.txt\", \"r\")\nlocal line = f:readUtf16Le()\ndetailPrint(line)\nf:close()",
    ),
    todo(
        "FileWriteUTF16LE",
        "file surface beyond `file`/`delete`/`fileOpen`: one overlay row each",
    ),
    exposed(
        "FileReadWord",
        "f:readWord",
        &[ann(Ty::Handle, Kind::Value), ann(Ty::nonneg(), Kind::Value)],
        "local f = fileOpen(INSTDIR .. \"/log.txt\", \"r\")\nlocal w = f:readWord()\ndetailPrint(\"word \" .. w)\nf:close()",
    ),
    todo(
        "FileWriteWord",
        "file surface beyond `file`/`delete`/`fileOpen`: one overlay row each",
    ),
    // The row that needed `fill`. `mode` is optional and sits *before* the
    // output, and `FileSeek $1 0 $0` is not the answer — NSIS reads `$0` as the
    // mode and rejects the line. So `local p = f:seek(0)` has to write a mode
    // nobody named, and `SET` is the one NSIS uses when the position is absent.
    // `f:seek(0)` with nothing reading the result still emits `FileSeek $1 0`:
    // the fill is written only when a later position is.
    exposed(
        "FileSeek",
        "f:seek",
        &[
            ann(Ty::Handle, Kind::Value),
            ann(Ty::int(), Kind::Value),
            filled(Ty::Str, Kind::Enum, "SET"),
            ann(Ty::nonneg(), Kind::Value),
        ],
        "local f = fileOpen(INSTDIR .. \"/log.txt\", \"r\")\nlocal size = f:seek(0, \"END\")\ndetailPrint(\"size \" .. size)\nf:close()",
    ),
    language("Function", "`func`"),
    language("FunctionEnd", "the end of a `func` body"),
    todo(
        "GetDlgItem",
        "addresses a window by handle; the `hwnd` surface wants nsDialogs designed first",
    ),
    exposed(
        "GetFullPathName",
        "getFullPathName",
        &[ann(Ty::Str, Kind::Value), ann(Ty::Str, Kind::Path)],
        "local full = getFullPathName(INSTDIR .. \"/app.exe\")\ndetailPrint(full)",
    ),
    exposed(
        "GetTempFileName",
        "getTempFileName",
        &[ann(Ty::Str, Kind::Value), ann(Ty::Str, Kind::Path)],
        "local scratch = getTempFileName()\ndetailPrint(scratch)",
    ),
    // The argument is a `KNOWNFOLDERID` GUID, not a name: NSIS ships no
    // constants for them, so the string is what the user has and the row does
    // not pretend otherwise (§13). A `knownFolder` table of the common ones is
    // a header, not an instruction.
    exposed(
        "GetKnownFolderPath",
        "getKnownFolderPath",
        &[ann(Ty::Str, Kind::Value), ann(Ty::Str, Kind::Value)],
        "local downloads = getKnownFolderPath(\"{374DE290-123F-4565-9164-39C4925E467B}\")\ndetailPrint(downloads)",
    ),
    // The field is an enum and the result is a *number*, which is the whole
    // reason anybody asks: `getWinVer("MAJOR") >= 10` is a comparison and
    // `Ty::Str` would have made it a string one (§15.14).
    exposed(
        "GetWinVer",
        "getWinVer",
        &[ann(Ty::nonneg(), Kind::Value), ann(Ty::Str, Kind::Enum)],
        "if getWinVer(\"MAJOR\") >= 10 then detailPrint(\"modern\") end",
    ),
    exposed(
        "ReadMemory",
        "readMemory",
        &[
            ann(Ty::Str, Kind::Value),
            ann(Ty::int(), Kind::Value),
            ann(Ty::nonneg(), Kind::Value),
        ],
        "local bytes = readMemory(0, 4)\ndetailPrint(bytes)",
    ),
    // Not the `hwnd` group its old reason put it in: `HideWindow` takes no
    // handle at all and hides the installer's own window. `LockWindow` is the
    // same mistake, and both are rows rather than a design.
    exposed("HideWindow", "hideWindow", &[], "hideWindow()"),
    attribute("Icon", "icon"),
    // A predicate's name drops the `If` and keeps the rest — `IfSilent` is
    // `silent` — except where that collides: `IfAbort` would be `abort`, which
    // is already `Abort`, so it is `aborted`. The past tense is also what it
    // means: the question is whether an abort has *happened*.
    predicate(
        "IfAbort",
        "aborted",
        &[ann(Ty::Str, Kind::Label), ann(Ty::Str, Kind::Label)],
        "if aborted() then detailPrint(\"cancelled\") end",
    ),
    // `IfErrors` **clears** the flag it reads — verified under wine (§15.20) —
    // so the call is the side effect and eliminating it when its result is
    // unused would silently break error handling. Nothing eliminates calls
    // today; when something does, this comment is the reason it must not.
    predicate(
        "IfErrors",
        "errors",
        &[ann(Ty::Str, Kind::Label), ann(Ty::Str, Kind::Label)],
        "clearErrors()\nif errors() then detailPrint(\"failed\") end",
    ),
    predicate(
        "IfFileExists",
        "fileExists",
        &[
            ann(Ty::Str, Kind::Path),
            ann(Ty::Str, Kind::Label),
            ann(Ty::Str, Kind::Label),
        ],
        "if fileExists(INSTDIR .. \"/app.exe\") then detailPrint(\"present\") end",
    ),
    predicate(
        "IfRebootFlag",
        "rebootFlag",
        &[ann(Ty::Str, Kind::Label), ann(Ty::Str, Kind::Label)],
        "if rebootFlag() then detailPrint(\"a restart is needed\") end",
    ),
    predicate(
        "IfSilent",
        "silent",
        &[ann(Ty::Str, Kind::Label), ann(Ty::Str, Kind::Label)],
        "if silent() then detailPrint(\"quiet\") end",
    ),
    predicate(
        "IfRtlLanguage",
        "rtlLanguage",
        &[ann(Ty::Str, Kind::Label), ann(Ty::Str, Kind::Label)],
        "if rtlLanguage() then detailPrint(\"right to left\") end",
    ),
    todo(
        "InstallDirRegKey",
        "registry surface beyond `readReg`/`writeReg`: one overlay row each",
    ),
    todo(
        "InstallColors",
        "the classic UI's appearance; MUI supersedes it, and §15.7's sequential-`!define` hazard is unruled",
    ),
    attribute("InstallDir", "installDir"),
    todo(
        "InstProgressFlags",
        "the classic UI's appearance; MUI supersedes it, and §15.7's sequential-`!define` hazard is unruled",
    ),
    todo(
        "InstType",
        "addresses a section by index: a real compile-time to install-time name binding (§13)",
    ),
    lowering("IntOp", "the arithmetic operators: `a + b`"),
    lowering(
        "IntPtrOp",
        "the arithmetic operators: pointer width is a type attribute (§15.14)",
    ),
    lowering("IntCmp", "a comparison: `a < b`"),
    lowering(
        "IntCmpU",
        "a comparison: the unsigned form is chosen from the operands' types (§15.14)",
    ),
    lowering(
        "Int64Cmp",
        "a comparison: 64-bit width is a type attribute, not a spelling (§15.14)",
    ),
    lowering(
        "Int64CmpU",
        "a comparison: width and sign are both type attributes (§15.14)",
    ),
    lowering(
        "IntPtrCmp",
        "a comparison: pointer width is a type attribute (§15.14)",
    ),
    lowering(
        "IntPtrCmpU",
        "a comparison: width and sign are both type attributes (§15.14)",
    ),
    lowering("IntFmt", "`string.format` (§15.21)"),
    lowering(
        "Int64Fmt",
        "`string.format`: 64-bit width is a type attribute (§15.14)",
    ),
    todo(
        "IsWindow",
        "addresses a window by handle; the `hwnd` surface wants nsDialogs designed first",
    ),
    lowering("Goto", "`if`, `while` and `break`"),
    todo(
        "LangString",
        "§15.26's locale tables are designed and unimplemented",
    ),
    rejected(
        "LangStringUP",
        "NSIS retired it: `langString` is the spelling",
    ),
    attribute("LicenseData", "license"),
    todo(
        "LicenseForceSelection",
        "a classic-UI caption or button label; each needs a home in `installer {}` or `page {}` first",
    ),
    todo(
        "LicenseLangString",
        "§15.26's locale tables are designed and unimplemented",
    ),
    todo(
        "LicenseText",
        "a classic-UI caption or button label; each needs a home in `installer {}` or `page {}` first",
    ),
    todo(
        "LicenseBkColor",
        "a classic-UI caption or button label; each needs a home in `installer {}` or `page {}` first",
    ),
    todo(
        "LoadLanguageFile",
        "§15.26's locale tables are designed and unimplemented",
    ),
    // Tier 3, immediately: *"Error: LogSet specified, NSIS_CONFIG_LOG not
    // defined."* — not a warning, and not a runtime surprise either. The stock
    // `makensis` cannot assemble a script containing this, so a row exposing it
    // would ship a call that fails on most machines and works on the author's.
    todo(
        "LogSet",
        "the stock `makensis` errors on it: logging needs a build with `NSIS_CONFIG_LOG`",
    ),
    todo(
        "LogText",
        "the stock `makensis` errors on it: logging needs a build with `NSIS_CONFIG_LOG`",
    ),
    exposed(
        "MessageBox",
        "messageBox",
        &[
            ann(Ty::Str, Kind::Flags),
            ann(Ty::Str, Kind::Value),
            ann(Ty::Str, Kind::Label),
            ann(Ty::Str, Kind::Label),
            ann(Ty::Str, Kind::Label),
            ann(Ty::Str, Kind::Label),
        ],
        "messageBox(\"finished\")",
    ),
    rejected(
        "Nop",
        "a statement that does nothing has no spelling: write nothing",
    ),
    attribute("Name", "name"),
    attribute("OutFile", "outFile"),
    todo(
        "Page",
        "a page construct; custom pages need a design (nsDialogs) that does not exist yet",
    ),
    todo(
        "PageCallbacks",
        "a page construct; custom pages need a design (nsDialogs) that does not exist yet",
    ),
    todo(
        "PageEx",
        "a page construct; custom pages need a design (nsDialogs) that does not exist yet",
    ),
    todo(
        "PageExEnd",
        "a page construct; custom pages need a design (nsDialogs) that does not exist yet",
    ),
    lowering(
        "Pop",
        "nothing: arguments and returns are the calling convention (§15.11)",
    ),
    lowering(
        "Push",
        "nothing: arguments and returns are the calling convention (§15.11)",
    ),
    exposed("Quit", "os.exit", &[], "os.exit()"),
    exposed(
        "ReadINIStr",
        "readIniStr",
        &[
            ann(Ty::Str, Kind::Value),
            ann(Ty::Str, Kind::Path),
            ann(Ty::Str, Kind::Value),
            ann(Ty::Str, Kind::Value),
        ],
        "local port = readIniStr(INSTDIR .. \"/app.ini\", \"Settings\", \"Port\")\ndetailPrint(port)",
    ),
    // `readRegDword` rather than a second dispatch of `readReg`: `writeReg`
    // can pick `WriteRegStr` or `WriteRegDWORD` from the type of the value it
    // was handed, and a *read* has no such argument. The name is the only
    // place the width can be said.
    exposed(
        "ReadRegDWORD",
        "readRegDword",
        &[
            ann(Ty::int(), Kind::Value),
            ann(Ty::Handle, Kind::Value),
            ann(Ty::Str, Kind::Path),
            ann(Ty::Str, Kind::Value),
        ],
        "local build = readRegDword(HKLM, \"Software/Example\", \"Build\")\ndetailPrint(\"build \" .. build)",
    ),
    exposed(
        "ReadRegStr",
        "readRegStr",
        &[
            ann(Ty::Str, Kind::Value),
            ann(Ty::Handle, Kind::Value),
            ann(Ty::Str, Kind::Path),
            ann(Ty::Str, Kind::Value),
        ],
        "local path = readRegStr(HKLM, \"Software/Example\", \"Path\")\ndetailPrint(path)",
    ),
    exposed(
        "ReadEnvStr",
        "readEnvStr",
        &[ann(Ty::Str, Kind::Value), ann(Ty::Str, Kind::Value)],
        "local temp = readEnvStr(\"TEMP\")\ndetailPrint(temp)",
    ),
    exposed("Reboot", "reboot", &[], "reboot()"),
    // Grouped with `InitPluginsDir` under §11 and it does not belong there:
    // `RegDLL` calls `DllRegisterServer` on a file already on the target and
    // needs nothing from the plugin directory. `UnRegDLL` is the same row with
    // the other entry point, and the alphabet reaches it later.
    exposed(
        "RegDLL",
        "regDll",
        &[ann(Ty::Str, Kind::Path), ann(Ty::Str, Kind::Value)],
        "regDll(INSTDIR .. \"/shell.dll\")",
    ),
    exposed(
        "Rename",
        "rename",
        &[ann(Ty::Str, Kind::Path), ann(Ty::Str, Kind::Path)],
        "rename(INSTDIR .. \"/old.txt\", INSTDIR .. \"/new.txt\")",
    ),
    language("Return", "`return`"),
    exposed(
        "RMDir",
        "rmDir",
        &[ann(Ty::Str, Kind::Path)],
        "rmDir(INSTDIR)",
    ),
    language("Section", "`section`"),
    language("SectionEnd", "the end of a `section` body"),
    todo(
        "SectionInstType",
        "addresses a section by index: a real compile-time to install-time name binding (§13)",
    ),
    todo(
        "SectionIn",
        "addresses a section by index: a real compile-time to install-time name binding (§13)",
    ),
    rejected(
        "SubSection",
        "deprecated by NSIS itself; `sectionGroup` is the spelling",
    ),
    todo(
        "SectionGroup",
        "addresses a section by index: a real compile-time to install-time name binding (§13)",
    ),
    rejected(
        "SubSectionEnd",
        "deprecated by NSIS itself; `sectionGroup` is the spelling",
    ),
    todo(
        "SectionGroupEnd",
        "addresses a section by index: a real compile-time to install-time name binding (§13)",
    ),
    // The input is a bare file name rather than a path — `SearchPath` is what
    // walks `%PATH%` — so §5's `/`-to-`\` rule has nothing to convert and
    // `Kind::Value` is the honest annotation. The *result* is a full path, and
    // it is a string like every other output.
    exposed(
        "SearchPath",
        "searchPath",
        &[ann(Ty::Str, Kind::Value), ann(Ty::Str, Kind::Value)],
        "local found = searchPath(\"notepad.exe\")\ndetailPrint(found)",
    ),
    todo(
        "SectionSetFlags",
        "addresses a section by index: a real compile-time to install-time name binding (§13)",
    ),
    todo(
        "SectionGetFlags",
        "addresses a section by index: a real compile-time to install-time name binding (§13)",
    ),
    todo(
        "SectionSetInstTypes",
        "addresses a section by index: a real compile-time to install-time name binding (§13)",
    ),
    todo(
        "SectionGetInstTypes",
        "addresses a section by index: a real compile-time to install-time name binding (§13)",
    ),
    todo(
        "SectionGetText",
        "addresses a section by index: a real compile-time to install-time name binding (§13)",
    ),
    todo(
        "SectionSetText",
        "addresses a section by index: a real compile-time to install-time name binding (§13)",
    ),
    todo(
        "SectionGetSize",
        "addresses a section by index: a real compile-time to install-time name binding (§13)",
    ),
    todo(
        "SectionSetSize",
        "addresses a section by index: a real compile-time to install-time name binding (§13)",
    ),
    todo(
        "GetCurInstType",
        "addresses a section by index: a real compile-time to install-time name binding (§13)",
    ),
    todo(
        "SetCurInstType",
        "addresses a section by index: a real compile-time to install-time name binding (§13)",
    ),
    todo(
        "InstTypeSetText",
        "addresses a section by index: a real compile-time to install-time name binding (§13)",
    ),
    todo(
        "InstTypeGetText",
        "addresses a section by index: a real compile-time to install-time name binding (§13)",
    ),
    todo(
        "SendMessage",
        "addresses a window by handle; the `hwnd` surface wants nsDialogs designed first",
    ),
    // The third row filed under "addresses a window by handle" that takes no
    // handle, after `HideWindow` and `LockWindow`. A group reason is a guess
    // about every member; the syntax line un-guesses it.
    exposed(
        "SetAutoClose",
        "setAutoClose",
        &[ann(Ty::Str, Kind::Enum)],
        "setAutoClose(\"true\")",
    ),
    todo(
        "SetCtlColors",
        "the classic UI's appearance; MUI supersedes it, and §15.7's sequential-`!define` hazard is unruled",
    ),
    todo(
        "SetBrandingImage",
        "the classic UI's appearance; MUI supersedes it, and §15.7's sequential-`!define` hazard is unruled",
    ),
    todo(
        "LoadAndSetImage",
        "the classic UI's appearance; MUI supersedes it, and §15.7's sequential-`!define` hazard is unruled",
    ),
    todo(
        "SetCompress",
        "compile time and positional: it changes the `file` calls after it rather than \
         executing, so a call inside an `if` would be a lie",
    ),
    attribute("SetCompressor", "compressor"),
    todo(
        "SetCompressorDictSize",
        "compile time and positional: it changes the `file` calls after it rather than \
         executing, so a call inside an `if` would be a lie",
    ),
    todo(
        "SetCompressionLevel",
        "compile time and positional: it changes the `file` calls after it rather than \
         executing, so a call inside an `if` would be a lie",
    ),
    attribute("SetDateSave", "dateSave"),
    exposed(
        "SetDetailsView",
        "setDetailsView",
        &[ann(Ty::Str, Kind::Enum)],
        "setDetailsView(\"show\")",
    ),
    exposed(
        "SetDetailsPrint",
        "setDetailsPrint",
        &[ann(Ty::Str, Kind::Enum)],
        "setDetailsPrint(\"listonly\")",
    ),
    // The twin of `clearErrors`, and it has been sitting one row away from it
    // in the census the whole time.
    exposed("SetErrors", "setErrors", &[], "setErrors()"),
    // `Ty::int()` to match `getErrorLevel`: NSIS does not restrict the sign,
    // and a row that narrowed the setter below its getter would reject
    // `setErrorLevel(getErrorLevel())`.
    exposed(
        "SetErrorLevel",
        "setErrorLevel",
        &[ann(Ty::int(), Kind::Value)],
        "setErrorLevel(2)",
    ),
    exposed(
        "GetErrorLevel",
        "getErrorLevel",
        &[ann(Ty::int(), Kind::Value)],
        "local level = getErrorLevel()\ndetailPrint(\"level \" .. level)",
    ),
    // The second `Kind::Flags` position in the table, after `messageBox`'s. The
    // members are joined with `|` rather than repeated, so one argument holds
    // however many the author names.
    exposed(
        "SetFileAttributes",
        "setFileAttributes",
        &[ann(Ty::Str, Kind::Path), ann(Ty::Str, Kind::Flags)],
        "setFileAttributes(INSTDIR .. \"/readme.txt\", \"READONLY\")",
    ),
    todo(
        "SetFont",
        "the classic UI's appearance; MUI supersedes it, and §15.7's sequential-`!define` hazard is unruled",
    ),
    exposed(
        "SetOutPath",
        "setOutPath",
        &[ann(Ty::Str, Kind::Path)],
        "setOutPath(INSTDIR)",
    ),
    todo(
        "SetOverwrite",
        "compile time and positional: it changes the `file` calls after it rather than \
         executing, so a call inside an `if` would be a lie",
    ),
    rejected(
        "SetPluginUnload",
        "NSIS retired it: plug-ins handle unloading themselves",
    ),
    exposed(
        "SetRebootFlag",
        "setRebootFlag",
        &[ann(Ty::Str, Kind::Enum)],
        "setRebootFlag(\"true\")",
    ),
    exposed(
        "GetRegView",
        "getRegView",
        &[ann(Ty::Str, Kind::Value)],
        "local view = getRegView()\ndetailPrint(view)",
    ),
    // `"32"` and `"64"` are enum members, not numbers: the argument names a
    // view rather than counting anything, and `getRegView` already returns the
    // string.
    exposed(
        "SetRegView",
        "setRegView",
        &[ann(Ty::Str, Kind::Enum)],
        "setRegView(\"64\")",
    ),
    predicate(
        "IfAltRegView",
        "altRegView",
        &[ann(Ty::Str, Kind::Label), ann(Ty::Str, Kind::Label)],
        "if altRegView() then detailPrint(\"the other view\") end",
    ),
    exposed(
        "GetShellVarContext",
        "getShellVarContext",
        &[ann(Ty::Str, Kind::Value)],
        "local context = getShellVarContext()\ndetailPrint(context)",
    ),
    exposed(
        "SetShellVarContext",
        "setShellVarContext",
        &[ann(Ty::Str, Kind::Enum)],
        "setShellVarContext(\"all\")",
    ),
    predicate(
        "IfShellVarContextAll",
        "shellVarContextAll",
        &[ann(Ty::Str, Kind::Label), ann(Ty::Str, Kind::Label)],
        "if shellVarContextAll() then detailPrint(\"all users\") end",
    ),
    // Written as a row and reverted. `makensis -WX` takes `SetSilent silent`
    // inside a section without a word, and NSIS then ignores it at run time:
    // the instruction is only meaningful from `.onInit`. Neither tier catches
    // that, because nothing is wrong with the *output* — the call is simply
    // dead. A row needs a mandatory example, `tests/overlay.rs` puts every
    // example in a section by design, and an example that does nothing is
    // worse than no row.
    todo(
        "SetSilent",
        "only meaningful from `.onInit`, and an example lives in a section: the row \
         needs a place to say where a call is legal",
    ),
    todo(
        "ShowInstDetails",
        "an installer-wide flag or mode: one overlay row each",
    ),
    todo(
        "ShowUninstDetails",
        "an installer-wide flag or mode: one overlay row each",
    ),
    todo(
        "ShowWindow",
        "addresses a window by handle; the `hwnd` surface wants nsDialogs designed first",
    ),
    todo(
        "SilentInstall",
        "an installer-wide flag or mode: one overlay row each",
    ),
    todo(
        "SilentUnInstall",
        "an installer-wide flag or mode: one overlay row each",
    ),
    exposed(
        "Sleep",
        "sleep",
        &[ann(Ty::nonneg(), Kind::Value)],
        "sleep(500)",
    ),
    lowering(
        "StrCmp",
        "`string.lower(a) == b`, which folds to one case-insensitive compare (§15.9)",
    ),
    lowering("StrCmpS", "`==`, which is case-sensitive (§15.9)"),
    lowering("StrCpy", "assignment: `x = y`"),
    rejected(
        "UnsafeStrCpy",
        "`StrCpy` with the bounds check removed; assignment is `=`, and \
         §15.31 checks the length",
    ),
    exposed(
        "StrLen",
        "string.len",
        &[ann(Ty::nonneg(), Kind::Value), ann(Ty::Str, Kind::Value)],
        "local n = string.len(\"abc\")\ndetailPrint(\"len \" .. n)",
    ),
    todo(
        "SubCaption",
        "a classic-UI caption or button label; each needs a home in `installer {}` or `page {}` first",
    ),
    todo(
        "Target",
        "a script-wide setting, not an instruction: it needs a home in `attributes {}` before it needs a row",
    ),
    todo(
        "CPU",
        "a script-wide setting, not an instruction: it needs a home in `attributes {}` before it needs a row",
    ),
    attribute("Unicode", "unicode"),
    rejected(
        "UninstallExeName",
        "NSIS retired it: write `writeUninstaller` from a section",
    ),
    todo(
        "UninstallCaption",
        "a classic-UI caption or button label; each needs a home in `installer {}` or `page {}` first",
    ),
    todo(
        "UninstallIcon",
        "the classic UI's appearance; MUI supersedes it, and §15.7's sequential-`!define` hazard is unruled",
    ),
    todo(
        "UninstPage",
        "a page construct; custom pages need a design (nsDialogs) that does not exist yet",
    ),
    todo(
        "UninstallText",
        "a classic-UI caption or button label; each needs a home in `installer {}` or `page {}` first",
    ),
    todo(
        "UninstallSubCaption",
        "a classic-UI caption or button label; each needs a home in `installer {}` or `page {}` first",
    ),
    exposed(
        "UnRegDLL",
        "unRegDll",
        &[ann(Ty::Str, Kind::Path)],
        "unRegDll(INSTDIR .. \"/shell.dll\")",
    ),
    todo(
        "WindowIcon",
        "the classic UI's appearance; MUI supersedes it, and §15.7's sequential-`!define` hazard is unruled",
    ),
    exposed(
        "WriteINIStr",
        "writeIniStr",
        &[
            ann(Ty::Str, Kind::Path),
            ann(Ty::Str, Kind::Value),
            ann(Ty::Str, Kind::Value),
            ann(Ty::Str, Kind::Value),
        ],
        "writeIniStr(INSTDIR .. \"/app.ini\", \"Settings\", \"Path\", INSTDIR)",
    ),
    // `writeRegBin` rather than a third dispatch of `writeReg`: the argument is
    // a hex *string* and so is `WriteRegStr`'s, so nothing in the call could
    // tell the two apart. `writeReg` dispatches on the type of the value it was
    // handed, and here the type does not differ.
    exposed(
        "WriteRegBin",
        "writeRegBin",
        &[
            ann(Ty::Handle, Kind::Value),
            ann(Ty::Str, Kind::Path),
            ann(Ty::Str, Kind::Value),
            ann(Ty::Str, Kind::Value),
        ],
        "writeRegBin(HKLM, \"Software/Example\", \"Blob\", \"12848412AB\")",
    ),
    todo(
        "WriteRegMultiStr",
        "its `/REGEDIT5` is required rather than optional, and nothing emits an \
         option that no argument supplies",
    ),
    exposed(
        "WriteRegDWORD",
        "writeReg",
        &[
            ann(Ty::Handle, Kind::Value),
            ann(Ty::Str, Kind::Path),
            ann(Ty::Str, Kind::Value),
            ann(Ty::int(), Kind::Value),
        ],
        "writeReg(HKLM, \"Software/Example\", \"Build\", 42)",
    ),
    exposed(
        "WriteRegStr",
        "writeReg",
        &[
            ann(Ty::Handle, Kind::Value),
            ann(Ty::Str, Kind::Path),
            ann(Ty::Str, Kind::Value),
            ann(Ty::Str, Kind::Value),
        ],
        "writeReg(HKLM, \"Software/Example\", \"Path\", INSTDIR)",
    ),
    // Same argument types as `WriteRegStr` and a different meaning — the target
    // expands `%VAR%` at read time — so this is a name, not a dispatch, for the
    // reason `writeRegBin` is.
    exposed(
        "WriteRegExpandStr",
        "writeRegExpandStr",
        &[
            ann(Ty::Handle, Kind::Value),
            ann(Ty::Str, Kind::Path),
            ann(Ty::Str, Kind::Value),
            ann(Ty::Str, Kind::Value),
        ],
        "writeRegExpandStr(HKLM, \"Software/Example\", \"Data\", \"%APPDATA%/Example\")",
    ),
    exposed(
        "WriteRegNone",
        "writeRegNone",
        &[
            ann(Ty::Handle, Kind::Value),
            ann(Ty::Str, Kind::Path),
            ann(Ty::Str, Kind::Value),
            ann(Ty::Str, Kind::Value),
        ],
        "writeRegNone(HKLM, \"Software/Example\", \"Marker\")",
    ),
    exposed(
        "WriteUninstaller",
        "writeUninstaller",
        &[ann(Ty::Str, Kind::Path)],
        "writeUninstaller(INSTDIR .. \"/uninstall.exe\")",
    ),
    todo(
        "PEAddResource",
        "a script-wide setting, not an instruction: it needs a home in `attributes {}` before it needs a row",
    ),
    todo(
        "PERemoveResource",
        "a script-wide setting, not an instruction: it needs a home in `attributes {}` before it needs a row",
    ),
    todo(
        "PEDllCharacteristics",
        "a script-wide setting, not an instruction: it needs a home in `attributes {}` before it needs a row",
    ),
    todo(
        "PESubsysVer",
        "a script-wide setting, not an instruction: it needs a home in `attributes {}` before it needs a row",
    ),
    todo(
        "XPStyle",
        "the classic UI's appearance; MUI supersedes it, and §15.7's sequential-`!define` hazard is unruled",
    ),
    attribute("RequestExecutionLevel", "requestExecutionLevel"),
    todo(
        "ManifestAppendCustomString",
        "a script-wide setting, not an instruction: it needs a home in `attributes {}` before it needs a row",
    ),
    todo(
        "ManifestDPIAware",
        "a script-wide setting, not an instruction: it needs a home in `attributes {}` before it needs a row",
    ),
    todo(
        "ManifestDPIAwareness",
        "a script-wide setting, not an instruction: it needs a home in `attributes {}` before it needs a row",
    ),
    todo(
        "ManifestLongPathAware",
        "a script-wide setting, not an instruction: it needs a home in `attributes {}` before it needs a row",
    ),
    todo(
        "ManifestSupportedOS",
        "a script-wide setting, not an instruction: it needs a home in `attributes {}` before it needs a row",
    ),
    todo(
        "ManifestMaxVersionTested",
        "a script-wide setting, not an instruction: it needs a home in `attributes {}` before it needs a row",
    ),
    todo(
        "ManifestDisableWindowFiltering",
        "a script-wide setting, not an instruction: it needs a home in `attributes {}` before it needs a row",
    ),
    todo(
        "ManifestGdiScaling",
        "a script-wide setting, not an instruction: it needs a home in `attributes {}` before it needs a row",
    ),
    directive("!packhdr"),
    directive("!finalize"),
    directive("!uninstfinalize"),
    directive("!system"),
    directive("!execute"),
    directive("!makensis"),
    directive("!addincludedir"),
    directive("!include"),
    directive("!cd"),
    directive("!if"),
    directive("!ifdef"),
    directive("!ifndef"),
    directive("!endif"),
    directive("!define"),
    directive("!undef"),
    directive("!else"),
    directive("!echo"),
    directive("!warning"),
    directive("!error"),
    directive("!assert"),
    directive("!verbose"),
    directive("!pragma"),
    directive("!macro"),
    directive("!macroend"),
    directive("!macroundef"),
    directive("!insertmacro"),
    directive("!ifmacrodef"),
    directive("!ifmacrondef"),
    directive("!tempfile"),
    directive("!delfile"),
    directive("!appendfile"),
    directive("!appendmemfile"),
    directive("!getdllversion"),
    directive("!gettlbversion"),
    directive("!searchparse"),
    directive("!searchreplace"),
    todo(
        "MiscButtonText",
        "a classic-UI caption or button label; each needs a home in `installer {}` or `page {}` first",
    ),
    todo(
        "DetailsButtonText",
        "a classic-UI caption or button label; each needs a home in `installer {}` or `page {}` first",
    ),
    todo(
        "UninstallButtonText",
        "a classic-UI caption or button label; each needs a home in `installer {}` or `page {}` first",
    ),
    todo(
        "InstallButtonText",
        "a classic-UI caption or button label; each needs a home in `installer {}` or `page {}` first",
    ),
    todo(
        "SpaceTexts",
        "a classic-UI caption or button label; each needs a home in `installer {}` or `page {}` first",
    ),
    todo(
        "CompletedText",
        "a classic-UI caption or button label; each needs a home in `installer {}` or `page {}` first",
    ),
    todo(
        "GetFunctionAddress",
        "takes the address of a function or label; `Call`-by-address has no Lua shape (§3)",
    ),
    todo(
        "GetLabelAddress",
        "takes the address of a function or label; `Call`-by-address has no Lua shape (§3)",
    ),
    todo(
        "GetCurrentAddress",
        "takes the address of a function or label; `Call`-by-address has no Lua shape (§3)",
    ),
    directive("!addplugindir"),
    todo(
        "InitPluginsDir",
        "the plugin directory and the DLL registration pair, neither of which `plugin` covers yet (§11)",
    ),
    todo(
        "AllowSkipFiles",
        "file surface beyond `file`/`delete`/`fileOpen`: one overlay row each",
    ),
    language("Var", "a global is declared by assigning to it (§15.24)"),
    attribute("VIAddVersionKey", "versionInfo.keys"),
    attribute("VIProductVersion", "versionInfo.product"),
    attribute("VIFileVersion", "versionInfo.file"),
    exposed(
        "LockWindow",
        "lockWindow",
        &[ann(Ty::Str, Kind::Enum)],
        "lockWindow(\"on\")",
    ),
];
