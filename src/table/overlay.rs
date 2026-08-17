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

use super::{Class, Field, Kind, Offer, Part, Setting};
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
    /// Only ever read for an optional input that precedes another emitted
    /// position; a trailing optional is simply not emitted, which is what
    /// "optional" already meant.
    pub fill: Option<&'static str>,
    /// The name this position takes in the trailing options table. Required
    /// positions are positional and carry none (§15.23).
    pub field: Option<Field>,
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
    /// What to do with each flag `-CMDHELP` prints, positional against the
    /// skeleton's option list the way [`Ann`] is against its parameter list.
    /// The census checks the lengths agree on an `Exposed` row.
    pub options: &'static [Offer],
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
        field: None,
    }
}

/// An optional position, which is a *named field of the trailing options table*
/// rather than a counted argument (§15.23).
///
/// The name is the only thing added by hand: `req: false` is the snapshot's and
/// so is the type of thing that goes there. `-CMDHELP` calls these `showmode`
/// and `hex_string_like_12848412AB`, which is why the name cannot simply be
/// derived from it.
const fn opt(ty: Ty, kind: Kind, name: &'static str) -> Ann {
    Ann {
        field: Some(Field { name, toggle: None }),
        ..ann(ty, kind)
    }
}

/// An optional position that also says what to write when it is skipped and a
/// later position is not. Every named field before another emitted position
/// needs one, because NSIS counts arguments and the caller did not write them.
const fn filled(ty: Ty, kind: Kind, name: &'static str, fill: &'static str) -> Ann {
    Ann {
        fill: Some(fill),
        ..opt(ty, kind, name)
    }
}

/// An optional position whose only legal value is a token NSIS spells itself,
/// so the field is a `bool` and the compiler writes the token: `execShell(…,
/// { invokeIdList = true })` becomes `ExecShell /INVOKEIDLIST …`.
const fn toggle(name: &'static str, nsis: &'static str) -> Ann {
    Ann {
        field: Some(Field {
            name,
            toggle: Some(nsis),
        }),
        ..ann(Ty::Bool, Kind::Value)
    }
}

/// A position that is not one: see [`Kind::Fused`]. Never offered and never
/// emitted, so the argument count stays NSIS's rather than `-CMDHELP`'s.
const fn fused(ty: Ty) -> Ann {
    ann(ty, Kind::Fused)
}

/// A flag reached by this name in the trailing options table, holding a `bool`.
/// The `/FLAG` itself stays in the table: `rmDir(dir, { recursive = true })`
/// says what it does, where `/r` says what NSIS calls it.
const fn named(name: &'static str) -> Offer {
    Offer::Named(name)
}

/// A flag with no decision in it, written on every call. See [`Offer::Always`].
const fn always() -> Offer {
    Offer::Always
}

/// A flag reached by this name and holding a list, one `/FLAG value` pair per
/// element. See [`Offer::List`].
const fn list(name: &'static str, kind: Kind) -> Offer {
    Offer::List { name, kind }
}

/// A flag a hand-shaped row writes itself. See [`Offer::Handled`].
const fn handled(name: &'static str) -> Offer {
    Offer::Handled(name)
}

// There is no `unoffered` helper: every flag on an `Exposed` row is reachable
// now, so writing one would take saying which flag is not — and the only
// [`Offer::Unoffered`] left is the one the join gives a row that has said
// nothing at all.

const fn row(nsis: &'static str, installua: Option<&'static str>, class: Class) -> Row {
    Row {
        nsis,
        example: None,
        installua,
        class,
        params: &[],
        options: &[],
        conflicts: &[],
        predicate: false,
    }
}

/// A row plus its flag judgements, one per flag `-CMDHELP` prints for it, in
/// that order. Separate from [`exposed`] because thirty rows have flags and
/// two hundred and forty-six do not.
const fn flagged(row: Row, options: &'static [Offer]) -> Row {
    Row { options, ..row }
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

/// A field of one of the four blocks (§15.10, §15.26), and what it holds.
///
/// The [`Setting`] is the whole of the row: [`crate::lower`] switches on it
/// rather than on the field's name, so adding a script-wide setting is this one
/// line. There is no [`Ann`] list, because an attribute's value is a Lua
/// expression in a table and not an argument list.
const fn attribute(nsis: &'static str, field: &'static str, holds: Setting) -> Row {
    row(nsis, Some(field), Class::Attribute(holds))
}

/// One position of a [`Setting::Table`], under the Lua key a caller writes it
/// with. The parts stand against the snapshot's positions in order, so the
/// count is checked by the census rather than by reading.
const fn part(field: &'static str, holds: Setting) -> Part {
    Part { field, holds }
}

/// The two commonest [`Setting`]s, spelled short because the rows are a column.
const STR: Setting = Setting::Str { path: false };
const PATH: Setting = Setting::Str { path: true };

/// A `bool` NSIS spells `on|off`, which is most of them.
const ONOFF: Setting = Setting::Bool {
    on: "on",
    off: "off",
};

/// A `bool` NSIS spells `true|false`, which is the manifest's half.
const TRUEFALSE: Setting = Setting::Bool {
    on: "true",
    off: "false",
};

/// A `bool` whose off-word is `notset`, for the two manifest settings whose
/// syntax line is `notset|true` and has no `false` in it.
///
/// The third state the syntax line names is not a third state here: *unset* is
/// the field being absent from the table, which emits no line at all. `notset`
/// is only what `= false` has to be spelled as, because writing the field is a
/// statement and NSIS has one word for "I looked and chose the default".
const NOTSET: Setting = Setting::Bool {
    on: "true",
    off: "notset",
};

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
        &[opt(Ty::Str, Kind::Value, "message")],
        "abort(\"stopped\")",
    ),
    // MUI2 has no define for the branding area and no opinion about it: it
    // *styles* whatever it finds (`SetCtlColors $mui.Branding.Text /BRANDING`),
    // which is the opposite of owning it. So this is an ordinary attribute.
    //
    // `size` is `STR` although the snapshot prints members for it. `(height|
    // width)` there is a metavariable saying *which dimension the edge implies*
    // and not a pair of keywords — the value NSIS reads is a number, optionally
    // suffixed `u` for dialog units. Enumerating it would reject every legal
    // value and complete to two illegal ones. The same trap as
    // `PERemoveResource`, and the second row to hit it.
    //
    // `padding` is `STR` for a second reason, found by running the line rather
    // than by reading it: the two numbers have to agree on their unit. `top 20u
    // 2u` assembles and `top 20u 2` is *Invalid number!*, so an `Int` padding
    // could never be written beside a `u` size. And it would have to be: a
    // bare-pixel size is *Must use dialog units on non-Win32 platforms!*, which
    // makes `u` the only form that builds on the machine this compiles on.
    //
    // That the two agree is a constraint no `Setting` can state, and the
    // compiler does not check it — `makensis` does, by name, which is the one
    // case where deferring is better than a worse message.
    attribute(
        "AddBrandingImage",
        "brandingImage",
        Setting::Table(&[
            part("edge", Setting::Enum),
            part("size", STR),
            part("padding", STR),
        ]),
    ),
    language("AddSize", "a `section`'s `size` option"),
    todo(
        "AutoCloseWindow",
        "addresses a window by handle; the `hwnd` surface wants nsDialogs designed first",
    ),
    // The full-screen background is a *second window*, drawn behind the wizard
    // and unaffected by which page UI is in front of it. MUI2 never mentions
    // either row, so neither was ever a classic-UI question.
    //
    // `/ITALIC`, `/UNDERLINE` and `/STRIKE` are unreachable: an attribute has no
    // options table, and giving it one is a shape rather than a row.
    attribute(
        "BGFont",
        "bgFont",
        Setting::Table(&[
            part("face", STR),
            part("height", Setting::Int),
            part("weight", Setting::Int),
        ]),
    ),
    todo(
        "BGGradient",
        "`off | (top [bottom [text]])` is an alternation, and the snapshot flattens \
         it to one required position: the shape has no `Setting`",
    ),
    // `/TRIMLEFT`, `/TRIMRIGHT` and `/TRIMCENTER` are one fused flag with three
    // suffixes, which the options table cannot say and an attribute cannot hold.
    attribute("BrandingText", "brandingText", STR),
    todo(
        "BringToFront",
        "addresses a window by handle; the `hwnd` surface wants nsDialogs designed first",
    ),
    lowering("Call", "a call: `f(x)`"),
    rejected(
        "CallInstDLL",
        "a plugin is called as `plugin.method(…)` (§11)",
    ),
    attribute("Caption", "caption", STR),
    rejected(
        "ChangeUI",
        "MUI2 calls it five times to install its own dialog resources; a sixth call \
         does not configure the UI, it replaces it",
    ),
    exposed("ClearErrors", "clearErrors", &[], "clearErrors()"),
    todo(
        "ComponentText",
        "MUI2 emits this line itself from `MUI_COMPONENTSPAGE_TEXT_TOP`, `…_TEXT_INSTTYPE` and `…_TEXT_COMPLIST`, so a raw one assembles clean under \
         `-WX` and then loses: it has to lower to the define, and where MUI \
         settings live is unruled",
    ),
    // The four rows that write *two* registers. A 64-bit value split across a
    // high and a low half is one number in every language that has one, and
    // Installua does not: §3 has no 64-bit type, so the halves stay halves and
    // the call binds both. `/ProductVersion` reads the *product* version rather
    // than the file version out of the same resource, which is a different
    // question about the same file and so a flag rather than a second name.
    flagged(
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
        &[named("productVersion")],
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
    // The one row where both halves of the options table are in use: a trailing
    // optional position that stays an argument, and two flags that were never
    // positions at all.
    flagged(
        exposed(
            "CopyFiles",
            "copyFiles",
            &[
                ann(Ty::Str, Kind::Path),
                ann(Ty::Str, Kind::Path),
                opt(Ty::nonneg(), Kind::Value, "sizeInKb"),
            ],
            "copyFiles(INSTDIR .. \"/data\", INSTDIR .. \"/backup\")",
        ),
        &[named("silent"), named("filesOnly")],
    ),
    attribute("CRCCheck", "crcCheck", ONOFF),
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
    // The row the options table was designed for. Two required positions and
    // six named ones: setting the description used to mean writing all nine
    // arguments, four of which the author does not care about and three of
    // which are enums they would have to look up.
    //
    // `icon index` is one NSIS argument with a typo in its name — the parser
    // reads token 5 once, with `gettoken_int` — hence the [`Kind::Fused`] half.
    // Every field but the last carries a fill, because NSIS still counts the
    // positions the caller declined.
    flagged(
        exposed(
            "CreateShortcut",
            "createShortcut",
            &[
                ann(Ty::Str, Kind::Path),
                ann(Ty::Str, Kind::Path),
                filled(Ty::Str, Kind::Value, "parameters", "\"\""),
                filled(Ty::Str, Kind::Path, "iconFile", "\"\""),
                filled(Ty::nonneg(), Kind::Value, "iconIndex", "0"),
                fused(Ty::nonneg()),
                filled(Ty::Str, Kind::Enum, "showMode", "SW_SHOWNORMAL"),
                filled(Ty::Str, Kind::Enum, "hotkey", "\"\""),
                opt(Ty::Str, Kind::Value, "comment"),
            ],
            "createShortcut(DESKTOP .. \"/App.lnk\", INSTDIR .. \"/app.exe\", \
             { comment = \"Launch App\" })",
        ),
        // The flag joins the six named positions in the same table, which is
        // the point of naming them: the caller writes what they mean and never
        // learns that this one goes *before* the first argument.
        &[named("noWorkingDir")],
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
    flagged(
        exposed(
            "DeleteRegKey",
            "deleteRegKey",
            &[ann(Ty::Handle, Kind::Value), ann(Ty::Str, Kind::Path)],
            "deleteRegKey(HKLM, \"Software/Example\")",
        ),
        &[named("ifEmpty")],
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
    // `/REBOOTOK` is the flag the uninstaller half of every real script wants:
    // a file the user has open cannot be deleted now, and this schedules it for
    // the next boot instead of failing silently.
    flagged(
        exposed(
            "Delete",
            "delete",
            &[ann(Ty::Str, Kind::Path)],
            "delete(INSTDIR .. \"/old.txt\", { rebootOk = true })",
        ),
        &[named("rebootOk")],
    ),
    exposed(
        "DetailPrint",
        "detailPrint",
        &[ann(Ty::Str, Kind::Value)],
        "detailPrint(\"installing\")",
    ),
    todo(
        "DirText",
        "MUI2 emits this line itself from `MUI_DIRECTORYPAGE_TEXT_TOP` and `…_TEXT_DESTINATION`, so a raw one assembles clean under \
         `-WX` and then loses: it has to lower to the define, and where MUI \
         settings live is unruled",
    ),
    rejected("DirShow", "NSIS itself reports this one as not working"),
    todo(
        "DirVar",
        "an installer-wide flag or mode: one overlay row each",
    ),
    // Not an attribute: `makensis` answers `command DirVerify not valid
    // outside PageEx`, which the tier-3 assembly of the attribute rows found
    // the first time it ran. `PageEx` has no Installua shape yet.
    todo(
        "DirVerify",
        "only valid inside `PageEx`, and the page surface has no design yet",
    ),
    exposed(
        "GetInstDirError",
        "getInstDirError",
        &[ann(Ty::nonneg(), Kind::Value)],
        "local why = getInstDirError()\ndetailPrint(\"instdir \" .. why)",
    ),
    // `(true|false)` rather than `on|off`, which is why the pair is on the row.
    attribute("AllowRootDirInstall", "allowRootDirInstall", TRUEFALSE),
    todo(
        "CheckBitmap",
        "MUI2 emits this line itself from `MUI_COMPONENTSPAGE_CHECKBITMAP`, so a raw one assembles clean under \
         `-WX` and then loses: it has to lower to the define, and where MUI \
         settings live is unruled",
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
    // `ExecShell [flags] verb file [parameters [showmode]]`. The optional
    // position is *first*, which is what the options table settles: `verb` and
    // `file` are the two required positions and nothing else is counted, so
    // `execShell("open", url)` can no longer bind `"open"` to `flags`.
    //
    // `flags` is a `toggle` because its only legal value is `/INVOKEIDLIST`,
    // which the compiler spells. It is also the one optional here that needs no
    // fill: NSIS tells it from `verb` by the leading `/`.
    //
    // `file` is `Kind::Value` and that is not an oversight. §5's `/`-to-`\`
    // rewrite is about a Windows *file* path, and this position is a shell
    // target — a path, a URL, or a registered document. Win32 takes `/` as a
    // separator, so `INSTDIR .. "/readme.txt"` still opens; a URL put through
    // §5 becomes `https:\\…` and does not.
    exposed(
        "ExecShell",
        "execShell",
        &[
            toggle("invokeIdList", "/INVOKEIDLIST"),
            ann(Ty::Str, Kind::Enum),
            ann(Ty::Str, Kind::Value),
            filled(Ty::Str, Kind::Value, "parameters", "\"\""),
            opt(Ty::Str, Kind::Enum, "showMode"),
        ],
        "execShell(\"open\", \"https://example.invalid\", { showMode = \"SW_HIDE\" })",
    ),
    exposed(
        "ExecShellWait",
        "execShellWait",
        &[
            toggle("invokeIdList", "/INVOKEIDLIST"),
            ann(Ty::Str, Kind::Value),
            ann(Ty::Str, Kind::Value),
            filled(Ty::Str, Kind::Value, "parameters", "\"\""),
            opt(Ty::Str, Kind::Value, "showMode"),
        ],
        "execShellWait(\"open\", INSTDIR .. \"/readme.txt\")",
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
    // Three of the four flags are booleans and become fields; `/x` is the row
    // `Offer::List` exists for. It takes a filespec *and* repeats, so its field
    // holds the exclusions and the emitter writes one `/x` each — which is the
    // only shape in which a caller can say two of them.
    flagged(
        exposed(
            "File",
            "file",
            &[ann(Ty::Str, Kind::Path)],
            "file(\"assets/icon.ico\", { exclude = { \"*.tmp\", \"*.log\" } })",
        ),
        &[
            named("nonFatal"),
            named("keepAttributes"),
            named("recursive"),
            list("exclude", Kind::Path),
        ],
    ),
    attribute("FileBufSize", "fileBufSize", Setting::Int),
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
    // `[text (can contain $0)] [text without ignore (can contain $0)]`: two
    // optional strings, once the parser stops reading the commentary and the
    // caption as positions. `$0` in them is NSIS's own runtime substitution and
    // not a §5 sigil, so both parts are plain `STR` — a path here would be
    // wrong twice over, since these are sentences shown to a user.
    attribute(
        "FileErrorText",
        "fileErrorText",
        Setting::Table(&[part("text", STR), part("withoutIgnore", STR)]),
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
            opt(Ty::nonneg(), Kind::Value, "maxLen"),
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
            opt(Ty::nonneg(), Kind::Value, "maxLen"),
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
            filled(Ty::Str, Kind::Enum, "mode", "SET"),
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
    // The flag goes before the *output* register — `GetFullPathName /SHORT $0
    // path` — which is the clearest case for `Opt::after` being a position in
    // the emitted line rather than an argument index the caller could count.
    flagged(
        exposed(
            "GetFullPathName",
            "getFullPathName",
            &[ann(Ty::Str, Kind::Value), ann(Ty::Str, Kind::Path)],
            "local full = getFullPathName(INSTDIR .. \"/app.exe\")\ndetailPrint(full)",
        ),
        &[named("short")],
    ),
    exposed(
        "GetTempFileName",
        "getTempFileName",
        &[
            ann(Ty::Str, Kind::Value),
            opt(Ty::Str, Kind::Path, "baseDir"),
        ],
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
    attribute("Icon", "icon", PATH),
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
    // Not a registry call but a *setting*: it names where the installer looks
    // for a previous install directory, and NSIS falls back to `installDir`
    // when the key is missing. Which is why the two are siblings in
    // `attributes {}` rather than one being an instruction.
    //
    // `key` is a [`PATH`] for the same reason `readRegStr`'s subkey is: a
    // registry path is written with `/` here and emitted with `\` (§5), so the
    // one shape is not spelled two ways depending on which row reaches it.
    attribute(
        "InstallDirRegKey",
        "installDirRegKey",
        Setting::Table(&[
            part("root", Setting::Enum),
            part("key", PATH),
            part("name", STR),
        ]),
    ),
    todo(
        "InstallColors",
        "MUI2 emits this line itself from `MUI_INSTFILESPAGE_COLORS`, so a raw one assembles clean under \
         `-WX` and then loses: it has to lower to the define, and where MUI \
         settings live is unruled",
    ),
    attribute("InstallDir", "installDir", PATH),
    todo(
        "InstProgressFlags",
        "MUI2 emits this line itself from `MUI_INSTFILESPAGE_PROGRESSBAR`, so a raw one assembles clean under \
         `-WX` and then loses: it has to lower to the define, and where MUI \
         settings live is unruled",
    ),
    language("InstType", "an `installer`'s `installTypes` field"),
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
    attribute("LicenseData", "license", PATH),
    todo(
        "LicenseForceSelection",
        "MUI2 emits this line itself from `MUI_LICENSEPAGE_CHECKBOX_TEXT`, `…_RADIOBUTTONS_TEXT_ACCEPT` and `…_DECLINE`, so a raw one assembles clean under \
         `-WX` and then loses: it has to lower to the define, and where MUI \
         settings live is unruled",
    ),
    todo(
        "LicenseLangString",
        "§15.26's locale tables are designed and unimplemented",
    ),
    todo(
        "LicenseText",
        "MUI2 emits this line itself from `MUI_LICENSEPAGE_TEXT_BOTTOM` and `MUI_LICENSEPAGE_BUTTON`, so a raw one assembles clean under \
         `-WX` and then loses: it has to lower to the define, and where MUI \
         settings live is unruled",
    ),
    todo(
        "LicenseBkColor",
        "MUI2 emits this line itself from `MUI_LICENSEPAGE_BGCOLOR`, so a raw one assembles clean under \
         `-WX` and then loses: it has to lower to the define, and where MUI \
         settings live is unruled",
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
    flagged(
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
            "messageBox { text = \"Restart now?\", buttons = \"YESNO\", silentAnswer = \"NO\" }",
        ),
        // `silentAnswer` is a field of §15.18's own table rather than of the
        // generic one, because the answer has to be legal for the `buttons`
        // beside it: `silentAnswer = "YES"` under `buttons = "OKCANCEL"` is an
        // error, and only the hand-shaped lowering can see both fields at once.
        &[handled("silentAnswer")],
    ),
    rejected(
        "Nop",
        "a statement that does nothing has no spelling: write nothing",
    ),
    attribute("Name", "name", STR),
    attribute("OutFile", "outFile", PATH),
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
        &[
            ann(Ty::Str, Kind::Path),
            opt(Ty::Str, Kind::Value, "entryPoint"),
        ],
        "regDll(INSTDIR .. \"/shell.dll\")",
    ),
    flagged(
        exposed(
            "Rename",
            "rename",
            &[ann(Ty::Str, Kind::Path), ann(Ty::Str, Kind::Path)],
            "rename(INSTDIR .. \"/old.txt\", INSTDIR .. \"/new.txt\")",
        ),
        &[named("rebootOk")],
    ),
    language("Return", "`return`"),
    // `/r` is the difference between removing an empty directory and removing
    // an installation, and `recursive` is what it is called here: the name says
    // what happens, where `/r` says what NSIS calls it.
    flagged(
        exposed(
            "RMDir",
            "rmDir",
            &[ann(Ty::Str, Kind::Path)],
            "rmDir(INSTDIR, { recursive = true, rebootOk = true })",
        ),
        &[named("recursive"), named("rebootOk")],
    ),
    language("Section", "`section`"),
    language("SectionEnd", "the end of a `section` body"),
    rejected(
        "SectionInstType",
        "an undocumented second name for `SectionIn`, which takes the same arguments in the same places: write a `section`'s `installTypes`",
    ),
    language(
        "SectionIn",
        "a `section`'s `installTypes` and `required` options",
    ),
    rejected(
        "SubSection",
        "deprecated by NSIS itself; `sectionGroup` is the spelling",
    ),
    language("SectionGroup", "`group`"),
    rejected(
        "SubSectionEnd",
        "deprecated by NSIS itself; `sectionGroup` is the spelling",
    ),
    language("SectionGroupEnd", "the end of a `group`'s section list"),
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
    // The three that change a control while the installer runs rather than
    // setting anything at compile time. Two of them take a handle nobody can
    // get — MUI2 keeps its `$mui.*` controls to itself — and the third has to be
    // called from a page callback, which is the same missing design.
    todo(
        "SetCtlColors",
        "addresses a control by handle; the `hwnd` surface wants nsDialogs designed first",
    ),
    todo(
        "SetBrandingImage",
        "runs from a page callback, and there is no way to write one yet (nsDialogs)",
    ),
    todo(
        "LoadAndSetImage",
        "addresses a control by handle; the `hwnd` surface wants nsDialogs designed first",
    ),
    todo(
        "SetCompress",
        "compile time and positional: it changes the `file` calls after it rather than \
         executing, so a call inside an `if` would be a lie",
    ),
    attribute("SetCompressor", "compressor", Setting::Enum),
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
    attribute("SetDateSave", "dateSave", ONOFF),
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
    // Not superseded by MUI2 — *read* by it. `Interface.nsh` builds its bold
    // header font out of `$(^Font)` and `$(^FontSize)`, which is exactly what
    // this line sets, so it is the input MUI derives from rather than something
    // MUI replaces. An installer-wide attribute that happens to be spelled with
    // a `Set` prefix.
    //
    // `/LANG=` is unreachable, which makes this the repeated-per-language shape
    // batch 13 built for `LangString` and one row short of needing it.
    attribute(
        "SetFont",
        "font",
        Setting::Table(&[part("face", STR), part("size", Setting::Int)]),
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
    attribute("ShowInstDetails", "showInstDetails", Setting::Enum),
    attribute("ShowUninstDetails", "showUninstDetails", Setting::Enum),
    todo(
        "ShowWindow",
        "addresses a window by handle; the `hwnd` surface wants nsDialogs designed first",
    ),
    attribute("SilentInstall", "silentInstall", Setting::Enum),
    attribute("SilentUnInstall", "silentUninstall", Setting::Enum),
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
        "MUI2 blanks exactly one index of it and leaves the rest free; a row owned \
         for one argument value and open for the others has no shape in the table",
    ),
    // `Target x86-unicode` is `cpu` and `unicode` hyphenated together, and both
    // of those are rows already. A third spelling of the same two settings would
    // be a second way to set `unicode`, which is not a line but a field the
    // emitter reads — so the two rows below are the whole of it.
    todo(
        "Target",
        "says `cpu` and `unicode` in one word, and both are attributes already",
    ),
    attribute("CPU", "cpu", Setting::Enum),
    attribute("Unicode", "unicode", Setting::Handled("boolean")),
    rejected(
        "UninstallExeName",
        "NSIS retired it: write `writeUninstaller` from a section",
    ),
    attribute("UninstallCaption", "uninstallCaption", STR),
    // Already done, under the name §15.3 gives it: `icon` inside `uninstaller
    // {}` is the same field for the other half, and it lowers to `MUI_UNICON`
    // rather than to this line, because MUI2 emits `UninstallIcon` itself from
    // that define and would otherwise win.
    language("UninstallIcon", "`icon` in `uninstaller {}`"),
    todo(
        "UninstPage",
        "a page construct; custom pages need a design (nsDialogs) that does not exist yet",
    ),
    todo(
        "UninstallText",
        "MUI2 emits this line itself from `MUI_UNCONFIRMPAGE_TEXT_TOP` and `…_TEXT_LOCATION`, so a raw one assembles clean under \
         `-WX` and then loses: it has to lower to the define, and where MUI \
         settings live is unruled",
    ),
    todo(
        "UninstallSubCaption",
        "MUI2 blanks exactly one index of it and leaves the rest free; a row owned \
         for one argument value and open for the others has no shape in the table",
    ),
    exposed(
        "UnRegDLL",
        "unRegDll",
        &[ann(Ty::Str, Kind::Path)],
        "unRegDll(INSTDIR .. \"/shell.dll\")",
    ),
    attribute("WindowIcon", "windowIcon", ONOFF),
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
    // The row `Offer::Always` exists for. `/REGEDIT5` is spelled like a flag and
    // behaves like a keyword: NSIS rejects the line without it, and there is no
    // second form to choose between, so the caller has nothing to decide and
    // the compiler writes it on every call. The value is a hex string for the
    // same reason `writeRegBin`'s is — a REG_MULTI_SZ is bytes, and §3 has no
    // list type to build them from.
    flagged(
        exposed(
            "WriteRegMultiStr",
            "writeRegMultiStr",
            &[
                ann(Ty::Handle, Kind::Value),
                ann(Ty::Str, Kind::Path),
                ann(Ty::Str, Kind::Value),
                ann(Ty::Str, Kind::Value),
            ],
            "writeRegMultiStr(HKLM, \"Software/Example\", \"List\", \"660000000000\")",
        ),
        &[always()],
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
            opt(Ty::Str, Kind::Value, "hexData"),
        ],
        "writeRegNone(HKLM, \"Software/Example\", \"Marker\")",
    ),
    exposed(
        "WriteUninstaller",
        "writeUninstaller",
        &[ann(Ty::Str, Kind::Path)],
        "writeUninstaller(INSTDIR .. \"/uninstall.exe\")",
    ),
    // The first two settings NSIS writes more than once, so the field is a
    // *list* of tables and each entry is one line.
    //
    // `restype` and `resname` are `#N` or a type NSIS knows by name, never an
    // arbitrary word, and `Setting::Str` says only "a string". That narrowing is
    // NSIS's rather than this language's, so it lives in the example the way
    // `peSubsysVer`'s does and not on the row.
    attribute(
        "PEAddResource",
        "peAddResource",
        Setting::Each(&Setting::Table(&[
            part("file", PATH),
            part("restype", STR),
            part("resname", STR),
            // Optional, and the snapshot is what says so.
            part("reslang", STR),
        ])),
    ),
    // The language is `STR` and not `Setting::Enum` although the snapshot lists
    // members for it: `-CMDHELP` prints `reslang|ALL`, and `reslang` there is a
    // placeholder rather than a keyword. Offering it would complete to a word
    // `makensis` rejects, which is worse than offering nothing.
    attribute(
        "PERemoveResource",
        "peRemoveResource",
        Setting::Each(&Setting::Table(&[
            part("restype", STR),
            part("resname", STR),
            part("reslang", STR),
        ])),
    ),
    // Two bit masks on one line. They are one field rather than two attributes
    // because NSIS takes them together — writing only the bits to add still
    // has to say that nothing is removed — and a table makes that one write.
    attribute(
        "PEDllCharacteristics",
        "peDllCharacteristics",
        Setting::Table(&[part("add", Setting::Int), part("remove", Setting::Int)]),
    ),
    // `major.minor`, which is a string and not a number: `5.1` as a Lua number
    // would round-trip through a float and arrive as `5.1` only by luck.
    attribute("PESubsysVer", "peSubsysVer", STR),
    todo(
        "XPStyle",
        "MUI2 emits `XPStyle On` unconditionally, so a user's `off` is a last-one-wins \
         race with no diagnostic: reject it or promise an ordering, and neither is ruled",
    ),
    attribute(
        "RequestExecutionLevel",
        "requestExecutionLevel",
        Setting::Enum,
    ),
    // `path` is an XPath into the manifest — `/assembly` — and not a file path,
    // so it is `STR` and must never be `PATH`: §5 would turn its `/` into `\`
    // and NSIS would reject the line the compiler built.
    attribute(
        "ManifestAppendCustomString",
        "manifestAppendCustomString",
        Setting::Each(&Setting::Table(&[part("path", STR), part("string", STR)])),
    ),
    attribute("ManifestDPIAware", "manifestDpiAware", TRUEFALSE),
    // A comma-separated list in one string, which NSIS parses and this compiler
    // does not: `"PerMonitorV2,system"` is one argument to both.
    attribute("ManifestDPIAwareness", "manifestDpiAwareness", STR),
    attribute("ManifestLongPathAware", "manifestLongPathAware", TRUEFALSE),
    // The row says `Enum` and nothing else. That the position repeats is
    // `-CMDHELP`'s `[...]`, and that the seven names do not close the set is its
    // `{GUID}` — so `manifestSupportedOS = { "Win7", "Win10" }` and
    // `{ "{e2011457-1546-43c5-a5fe-008deee3d3f0}" }` are both this one line.
    attribute("ManifestSupportedOS", "manifestSupportedOS", Setting::Enum),
    // `maj.min.bld.rev`, a string for the same reason as `PESubsysVer`.
    attribute("ManifestMaxVersionTested", "manifestMaxVersionTested", STR),
    attribute(
        "ManifestDisableWindowFiltering",
        "manifestDisableWindowFiltering",
        NOTSET,
    ),
    attribute("ManifestGdiScaling", "manifestGdiScaling", NOTSET),
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
    // The six button and status labels. MUI2 supplies no define for any of
    // them, so they are ordinary attributes — but they are a §15.26 interaction
    // rather than a UI one: the default text comes from the NLF of whatever
    // language is running, and writing one of these overrides *every* language
    // at once. That is worth a diagnostic and is not a reason to withhold them.
    //
    // The four labels of `MiscButtonText` are one line and therefore one field:
    // NSIS reads them by position, so writing only the last still means writing
    // the three before it, and a table is where that is checkable.
    attribute(
        "MiscButtonText",
        "buttonText",
        Setting::Table(&[
            part("back", STR),
            part("next", STR),
            part("cancel", STR),
            part("close", STR),
        ]),
    ),
    attribute("DetailsButtonText", "detailsButtonText", STR),
    attribute("UninstallButtonText", "uninstallButtonText", STR),
    attribute("InstallButtonText", "installButtonText", STR),
    todo(
        "SpaceTexts",
        "`none | (required [available])` is an alternation, and the snapshot flattens \
         it to one required position: the shape has no `Setting`",
    ),
    attribute("CompletedText", "completedText", STR),
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
    attribute("AllowSkipFiles", "allowSkipFiles", ONOFF),
    language("Var", "a global is declared by assigning to it (§15.24)"),
    attribute(
        "VIAddVersionKey",
        "versionInfo.keys",
        Setting::Handled("table"),
    ),
    attribute(
        "VIProductVersion",
        "versionInfo.product",
        Setting::Handled("string"),
    ),
    attribute(
        "VIFileVersion",
        "versionInfo.file",
        Setting::Handled("string"),
    ),
    exposed(
        "LockWindow",
        "lockWindow",
        &[ann(Ty::Str, Kind::Enum)],
        "lockWindow(\"on\")",
    ),
];
