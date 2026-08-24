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

use super::{Class, Field, Kind, Offer, Part, Place, Setting};
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
    /// Where a call to this row is honoured, as opposed to accepted. See
    /// [`Place`] — one row is not [`Place::Anywhere`], and this exists because
    /// no test tier can see the difference.
    pub place: Place,
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

/// A position the compiler fills from a name — a section handle or an install
/// type — rather than from an argument: see [`Kind::Bound`]. The `Ty` is what
/// *NSIS* reads, because the surface has no position here to have a type.
const fn bound(ty: Ty) -> Ann {
    ann(ty, Kind::Bound)
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

/// A flag reached by this name and holding one value, glued to the flag with
/// an `=`. See [`Offer::Valued`].
const fn valued(name: &'static str, ty: Ty, kind: Kind) -> Offer {
    Offer::Valued { name, ty, kind }
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
        place: Place::Anywhere,
    }
}

/// A row NSIS honours only from `.onInit`. Separate from [`exposed`] for the
/// same reason [`flagged`] is: one row needs it and two hundred and seventy-five
/// do not.
const fn on_init(row: Row) -> Row {
    Row {
        place: Place::OnInit,
        ..row
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
///
/// Unused as of Phase 6, and kept for the reason the MUI census's twin is: an
/// empty backlog is a state to be able to *lose*. `-CMDHELP` grows with every
/// NSIS, and the first command nobody has read yet needs a bucket that is neither
/// "exposed" nor "we decided against it" — which is why the join already files a
/// skeleton with no row here on its own.
#[allow(dead_code)]
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
    // Filed under "addresses a window by handle" and its syntax line is
    // `(false|true)`: no handle, no window named, nothing to design. It is the
    // compile-time twin of `SetAutoClose`, which has been exposed since batch 16
    // — the two sat in different groups for the whole of Phase 6 because one
    // reason was written about all fourteen rows at once.
    attribute("AutoCloseWindow", "autoCloseWindow", TRUEFALSE),
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
    // The first of the two alternations, and the row [`Setting::Off`] was
    // written for: `bgGradient = false` turns the second window off, a table
    // colours it. Only `top` is required — a gradient from one colour to
    // nothing is a solid background, which is a thing people ask for.
    attribute(
        "BGGradient",
        "bgGradient",
        Setting::Off {
            word: "off",
            parts: &[part("top", STR), part("bottom", STR), part("text", STR)],
            least: 1,
        },
    ),
    // `/TRIMLEFT`, `/TRIMRIGHT` and `/TRIMCENTER` are one fused flag with three
    // suffixes, which the options table cannot say and an attribute cannot hold.
    attribute("BrandingText", "brandingText", STR),
    // The fourth row in that group to take no handle, after `HideWindow`,
    // `LockWindow` and `SetAutoClose`: it raises the installer's own window and
    // `-CMDHELP` prints it with no arguments at all.
    exposed("BringToFront", "bringToFront", &[], "bringToFront()"),
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
    // The first of the page settings, and the row that carries the convention
    // for all of them: the field path is `page.<page>.<field>`, and it is
    // dotted for the reason `versionInfo.keys` is — a dotted name is reached
    // through its owner and never written flat, so these are out of
    // `attributes {}` by construction.
    //
    // One row can carry several fields, the way `versionInfo.keys` carries
    // every key: the row is named for the field its **first** argument becomes,
    // and the arguments after it are the fields beside it. Here that is
    // `instTypeText` and `listText`.
    //
    // What MUI2 owns is the *line*. `ComponentText` is written by MUI2, from
    // these three defines, inside the `PageEx` it generates — so a second one
    // written by us assembles clean under `-WX` and then loses. What stays
    // public is the *setting*, exactly as `icon` is public and `Icon` is not.
    attribute("ComponentText", "page.components.topText", STR),
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
    // `Kind::Value` and not `Kind::Path`, for the reason spelled out on
    // `GetFileTimeLocal` below: this path is opened by `makensis` on whatever
    // host is building, so §5's `/`-to-`\` would break it.
    //
    // The row was never the problem. It waited for a *file*: nothing shipped
    // with NSIS carries a version resource — every plugin and stub answers
    // *"error reading version info"* — and a fixture that is not what it claims
    // to be is what that directory's README exists to forbid. So
    // `tests/fixtures/assets/version.dll` is a real PE with a real
    // `VS_VERSIONINFO` in it and nothing else, generated by the script beside
    // it, with a file version and a product version that deliberately differ.
    //
    // **No `productVersion` flag here, and `GetDLLVersion` has one.** Real
    // `makensis` 3.12 accepts `GetDLLVersionLocal /ProductVersion` and reads the
    // right half — this was checked by hand against the fixture — but
    // `-CMDHELP` prints no options for this row, and the census judges flags
    // against the snapshot rather than against what the assembler turns out to
    // tolerate. Offering one the table cannot see would be the first place this
    // language guessed, and the guess would be invisible the day it was wrong.
    exposed(
        "GetDLLVersionLocal",
        "getDllVersionLocal",
        &[
            ann(Ty::Str, Kind::Value),
            ann(Ty::nonneg(), Kind::Value),
            ann(Ty::nonneg(), Kind::Value),
        ],
        "local high, low = getDllVersionLocal(\"assets/version.dll\")\ndetailPrint(high .. \".\" .. low)",
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
    // The first of the six the compiler writes behind a control's fields. None
    // of them is `exposed`: a call would need a handle, and the only handles
    // there are belong to controls this compiler drew — so the field *is* the
    // call, with the kind checked and the register spilled (§15.32).
    lowering(
        "CreateFont",
        "a control's `font`: `serial.font = { face = \"Tahoma\", size = 8 }` (§15.32)",
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
    // `off|on`, and on by default. Turning it off is a debugging move — it
    // stops NSIS from sharing identical data blocks between files — which makes
    // it exactly the kind of thing an author sets once for the whole installer.
    attribute("SetDatablockOptimize", "datablockOptimize", ONOFF),
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
    // `destinationText` beside it.
    attribute("DirText", "page.directory.topText", STR),
    rejected("DirShow", "NSIS itself reports this one as not working"),
    // A *variable* rather than a value: NSIS stores the chosen directory into
    // it, so the field takes a global by name (§15.24) and `Handled` says the
    // page's own lowering shapes it.
    attribute(
        "DirVar",
        "page.directory.variable",
        Setting::Handled("string"),
    ),
    // `makensis` answers `command DirVerify not valid outside PageEx`, which
    // the tier-3 assembly of the attribute rows found the first time it ran —
    // and that is the whole argument for the page block, not against it. MUI2
    // writes the line inside the `PageEx` it generates, from
    // `MUI_DIRECTORYPAGE_VERIFYONLEAVE`, and the define is the surface.
    //
    // `Handled` rather than a `Bool`, because there is no off-word to emit:
    // MUI2 asks `!ifdef`, so `false` is the define's absence.
    attribute(
        "DirVerify",
        "page.directory.verifyOnLeave",
        Setting::Handled("boolean"),
    ),
    exposed(
        "GetInstDirError",
        "getInstDirError",
        &[ann(Ty::nonneg(), Kind::Value)],
        "local why = getInstDirError()\ndetailPrint(\"instdir \" .. why)",
    ),
    // `(true|false)` rather than `on|off`, which is why the pair is on the row.
    attribute("AllowRootDirInstall", "allowRootDirInstall", TRUEFALSE),
    // The first of the four settings that *look* page-scoped and are not. MUI2
    // writes this one inside `MUI_COMPONENTSPAGE_INTERFACE`, behind an
    // `!ifndef` that runs on the first components page and never again — so
    // putting it on the page would be a lie a second page tells silently. It is
    // a block field, and the path says which block: `installer {}` and not
    // `attributes {}`, because a raw `CheckBitmap` line loses to MUI2's.
    attribute("CheckBitmap", "installer.checkBitmap", PATH),
    lowering(
        "EnableWindow",
        "a control's `enabled`: `agree.enabled = false` (§15.32)",
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
    // The old reason said a command line is *part* path and so could obey
    // neither `Kind::Path` nor `Kind::Value`, and the half of that which is true
    // — `Kind::Path` would turn the `/S` in `setup.exe /S` into `\S` — never
    // implied the other half. §15.2 normalises `/` where **NSIS** demands a
    // backslash, not where Windows does: Windows accepts forward slashes at the
    // API level, and `Exec` hands its string to `CreateProcess`, which resolves
    // the program through that same parser. So the forward slashes go out
    // unchanged and the program is found.
    //
    // This is exactly the ruling `ExecShell`'s `file` position already carries
    // two rows down, for the same reason and in the same words. It was written
    // there while these two sat on a `todo` that contradicted it.
    exposed(
        "Exec",
        "exec",
        &[ann(Ty::Str, Kind::Value)],
        "exec(INSTDIR .. \"/app.exe /S\")",
    ),
    // `ExecWait command_line [$(user_var: return value)]` — the exit code is an
    // optional trailing output, so `FileSeek`'s rule applies: it is emitted only
    // when something reads it, and `execWait(cmd)` alone still writes two words.
    exposed(
        "ExecWait",
        "execWait",
        &[ann(Ty::Str, Kind::Value), ann(Ty::int(), Kind::Value)],
        "local code = execWait(INSTDIR .. \"/app.exe /S\")\ndetailPrint(\"exit \" .. code)",
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
    // The group reason has the direction backwards: this row *produces* a
    // handle rather than addressing one, and the window it finds belongs to
    // another process. nsDialogs is about windows this installer creates, so
    // nothing here waits on it.
    exposed(
        "FindWindow",
        "findWindow",
        &[
            ann(Ty::Handle, Kind::Value),
            ann(Ty::Str, Kind::Value),
            filled(Ty::Str, Kind::Value, "title", "\"\""),
            filled(Ty::Handle, Kind::Value, "parent", "0"),
            opt(Ty::Handle, Kind::Value, "childAfter"),
        ],
        "local window = findWindow(\"Notepad\")\nif isWindow(window) then\n\tdetailPrint(\"already running\")\nend",
    ),
    // `Todo` for six phases, and it was always a `Rejected`: the reason names a
    // decision this language already made rather than work nobody has done.
    // §15.19 iterates a directory on the **build** machine and unrolls the
    // result, so `for … in glob` is a known list of files by the time anything
    // runs. The three `Find*` are the other answer — a cursor over whatever is
    // on the *target* disk at install time — and the two cannot be offered side
    // by side without the language having two meanings for "the files in this
    // directory". A row that will not be written is a `rejected`, and calling it
    // a `todo` for six phases said the opposite to everyone reading the census.
    rejected(
        "FindClose",
        "iterating the target's disk at install time; §15.19 unrolls `for … in glob` on the build machine instead",
    ),
    rejected(
        "FindFirst",
        "iterating the target's disk at install time; §15.19 unrolls `for … in glob` on the build machine instead",
    ),
    rejected(
        "FindNext",
        "iterating the target's disk at install time; §15.19 unrolls `for … in glob` on the build machine instead",
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
    // `file`'s parameter list without `/a`, which is the whole of the first
    // alternative — and the snapshot records exactly those three flags, because
    // [`Note::Alternation`] keeps the first alternative and files the rest as
    // mutual exclusion (§15.23).
    //
    // The second alternative, `/plugin file.dll`, is deliberately not offered
    // and is not [`Offer::Unoffered`] either: a plugin the program calls before
    // the data block can be reached is reserved *by the compiler*, from the
    // call sites it already has to know about. See [`crate::lower::reserved`].
    // Writing it by hand is the include-order hazard in its purest form — a
    // line whose absence costs nothing until the day compression changes.
    flagged(
        exposed(
            "ReserveFile",
            "reserveFile",
            &[ann(Ty::Str, Kind::Path)],
            "reserveFile(\"assets/icon.ico\", { exclude = { \"*.tmp\" } })",
        ),
        &[
            named("nonFatal"),
            named("recursive"),
            list("exclude", Kind::Path),
        ],
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
    // The write half of the row above, and the group reason said so: *"one
    // overlay row each"* is a statement about cost, not about a missing design.
    // `f:readByte` has been exposed since the read pass, and this is the same
    // two positions with the direction reversed — `handle_input` in,
    // `bytevalue` in — so there was never a second thing to decide.
    exposed(
        "FileWriteByte",
        "f:writeByte",
        &[ann(Ty::Handle, Kind::Value), ann(Ty::nonneg(), Kind::Value)],
        "local f = fileOpen(INSTDIR .. \"/log.txt\", \"w\")\nf:writeByte(65)\nf:close()",
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
    // `text` rather than a number, so `Ty::Str` — and unlike its reader there
    // is no `maxLen`, because a write knows how much it is writing. `/BOM` sits
    // at `after: 0`, before the handle, which is the same position `/SHORT`
    // taught `Opt::after` to carry: the caller writes `{ bom = true }` and the
    // emitter decides where the word goes. Only the first write to a file wants
    // it, so it is a decision and therefore `named`, not `always`.
    flagged(
        exposed(
            "FileWriteUTF16LE",
            "f:writeUtf16Le",
            &[ann(Ty::Handle, Kind::Value), ann(Ty::Str, Kind::Value)],
            "local f = fileOpen(INSTDIR .. \"/log.txt\", \"w\")\nf:writeUtf16Le(\"done\", { bom = true })\nf:close()",
        ),
        &[named("bom")],
    ),
    exposed(
        "FileReadWord",
        "f:readWord",
        &[ann(Ty::Handle, Kind::Value), ann(Ty::nonneg(), Kind::Value)],
        "local f = fileOpen(INSTDIR .. \"/log.txt\", \"r\")\nlocal w = f:readWord()\ndetailPrint(\"word \" .. w)\nf:close()",
    ),
    exposed(
        "FileWriteWord",
        "f:writeWord",
        &[ann(Ty::Handle, Kind::Value), ann(Ty::nonneg(), Kind::Value)],
        "local f = fileOpen(INSTDIR .. \"/log.txt\", \"w\")\nf:writeWord(1024)\nf:close()",
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
    // The one window row a program calls rather than reaches: every other
    // control instruction is behind a field, and this is what produces a control
    // the compiler did not draw. The ids are Microsoft's and MUI2's — 1 is OK, 2
    // is Cancel, 3 is Back — so they are numbers here rather than names this
    // compiler invented for someone else's dialog (§15.32).
    exposed(
        "GetDlgItem",
        "getDlgItem",
        &[
            ann(Ty::Handle, Kind::Value),
            ann(Ty::Handle, Kind::Value),
            ann(Ty::nonneg(), Kind::Value),
        ],
        "local cancel = getDlgItem(HWNDPARENT, 2)\ncancel.enabled = false",
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
    // `MUI_INSTFILESPAGE_INTERFACE`, once, on the first InstFiles page.
    attribute("InstallColors", "installer.installColors", STR),
    attribute("InstallDir", "installDir", PATH),
    // The same macro, and the name is the surface's rather than NSIS's: what a
    // caller is choosing is how the progress bar looks, not which flags a line
    // carries.
    attribute("InstProgressFlags", "installer.progressBar", STR),
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
    // A predicate over the handle `findWindow` returns, and §15.20 has known
    // how to lower one of those since batch 8. The two positions after the
    // handle are the compiler's labels, not the caller's arguments.
    predicate(
        "IsWindow",
        "isWindow",
        &[
            ann(Ty::Handle, Kind::Value),
            ann(Ty::Str, Kind::Label),
            ann(Ty::Str, Kind::Label),
        ],
        "local window = findWindow(\"Notepad\")\nif isWindow(window) then\n\tdetailPrint(\"still open\")\nend",
    ),
    lowering("Goto", "`if`, `while` and `break`"),
    // Written by the compiler, never by a script: `languages {}` is keyed by
    // locale because that is what a translator owns, and NSIS wants it keyed by
    // name, so the transposition is the lowering (§15.26).
    lowering("LangString", "`languages { locales = { … } }`"),
    rejected(
        "LangStringUP",
        "NSIS retired it: `langString` is the spelling",
    ),
    // Not a block field any more. `installer { license = … }` was page data
    // written at block level: exactly one page read it, and a script that named
    // no License page dropped it without a word. `MUI_PAGE_LICENSE` takes the
    // file as its macro *argument*, so the page is where it is required and
    // where forgetting it is a diagnostic.
    attribute("LicenseData", "page.license.file", PATH),
    // `radioButtons` beside it, as a table of `accept` and `decline`. The row
    // is named for the checkbox because that is the branch MUI2 reads first.
    attribute(
        "LicenseForceSelection",
        "page.license.checkbox",
        Setting::Handled("string"),
    ),
    // The same transposition `LangString` is, arriving from the other side:
    // `file` takes a table keyed by locale, and the compiler files each path
    // under a name of its own so that the page macro reads `$(licenseData)`.
    // Not a row of its own, because a license page with translated text is one
    // page with one license on it — the plural is in the file, not the page.
    lowering(
        "LicenseLangString",
        "`page.license { file = { English = \"en.txt\", … } }`",
    ),
    // `button` beside it.
    attribute("LicenseText", "page.license.bottomText", STR),
    // `MUI_LICENSEPAGE_INTERFACE`, once, on the first License page — so a block
    // field, like the other three of its kind.
    attribute("LicenseBkColor", "installer.licenseBkColor", STR),
    // Not unimplemented any more, and not exposed either. `MUI_LANGUAGE` is
    // what loads a language file, and it accumulates `MUI_LANGDLL_LANGUAGES`
    // as it goes — the list `MUI_LANGDLL_DISPLAY` hands the plugin. A bare
    // `LoadLanguageFile` would load a language the dialog cannot offer.
    rejected(
        "LoadLanguageFile",
        "`languages { locales = { … } }` loads them through MUI2, which keeps the list the language dialog reads (§15.26)",
    ),
    // Tier 3, immediately: *"Error: LogSet specified, NSIS_CONFIG_LOG not
    // defined."* — not a warning, and not a runtime surprise either. The stock
    // `makensis` cannot assemble a script containing this, so a row exposing it
    // would ship a call that fails on most machines and works on the author's.
    //
    // That is a `rejected` and not a `todo`, because nothing here is waiting on
    // anything: the blocker is a **compile-time flag in someone else's build of
    // the assembler**, and a language whose surface depended on how the user's
    // `makensis` was compiled would have a portability question in every script.
    // `detailPrint` writes to the details window, which every build has.
    rejected(
        "LogSet",
        "logging needs `makensis` built with `NSIS_CONFIG_LOG`, which the stock build is not; use `detailPrint`",
    ),
    rejected(
        "LogText",
        "logging needs `makensis` built with `NSIS_CONFIG_LOG`, which the stock build is not; use `detailPrint`",
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
    // These four shared one reason, and the page block splits them in two.
    //
    // `Page` and `UninstPage` are the compiler's lines now. Their syntax line
    // is an alternation, and both halves are written from `page.*`: the seven
    // MUI2 pages become `!insertmacro MUI_PAGE_*`, and `page.custom` becomes
    // the `Page custom` this row spells, with the creator and the leave
    // function generated around it (§15.32). Nobody writes the line, because
    // its two arguments are names only the compiler has.
    //
    // The other three are `Rejected` now. `PageEx` is the classic page block
    // MUI2 *generates* around every one of its pages — `PageEx directory` …
    // `PageExEnd` is what `!insertmacro MUI_PAGE_DIRECTORY` expands to — so
    // writing one is not configuring the UI, it is reimplementing MUI2 beside
    // it.
    language("Page", "`page.*` inside `installer {}`"),
    rejected(
        "PageCallbacks",
        "legal only inside `PageEx`, where MUI2 fills it with the names of the functions it \
         generates; the page's `pre`, `show` and `leave` are the surface",
    ),
    rejected(
        "PageEx",
        "the classic page block MUI2 generates around every page; `page.directory {}` \
         configures that block, and a second one beside it replaces the UI rather than \
         settling it",
    ),
    rejected(
        "PageExEnd",
        "the other half of `PageEx`, which MUI2 generates and Installua does not open",
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
    // The eight section-indexed rows, reached through a handle rather than
    // called. `${SEC_core}` is the index and the handle is the `local` a block
    // listed, so every one of them carries a [`Kind::Bound`] position and none
    // of them has an argument list at all: `handle.text = "…"` is a *field*, and
    // the number NSIS reads appears nowhere in the source (§13).
    //
    // The flags pair is one row per direction and four fields per row —
    // `selected`, `readOnly`, `bold`, `expanded` are bits of one word — so its
    // spelling here is the commonest of them and the other three are the
    // `installua.Section` class in `stubs.rs`. The word itself is `Bound` too:
    // the surface writes a `bool` and the compiler does the mask, the shift and
    // the read-modify-write.
    exposed(
        "SectionSetFlags",
        "handle.selected",
        &[bound(Ty::Handle), bound(Ty::nonneg())],
        "handle.selected = false",
    ),
    exposed(
        "SectionGetFlags",
        "handle.selected",
        &[bound(Ty::Handle), bound(Ty::nonneg())],
        "if handle.selected then\n\tdetailPrint(\"the addressed section is ticked\")\nend",
    ),
    // The write is a bit field over the block's `installTypes` list, and the
    // list is named rather than numbered on both sides.
    exposed(
        "SectionSetInstTypes",
        "handle.installTypes",
        &[bound(Ty::Handle), bound(Ty::nonneg())],
        "handle.installTypes = { \"Minimal\" }",
    ),
    // The read is the same field called rather than assigned to, because the
    // bit field NSIS hands back has no list value here to become — and a script
    // asking about install types at run time is asking about one of them. The
    // name is the argument, the answer is a `bool`, and the position is the
    // compiler's on both sides (§13).
    exposed(
        "SectionGetInstTypes",
        "handle.installTypes",
        &[bound(Ty::Handle), bound(Ty::Bool)],
        "if handle.installTypes(\"Full\") then\n\tdetailPrint(\"the addressed section is in Full\")\nend",
    ),
    exposed(
        "SectionGetText",
        "handle.text",
        &[bound(Ty::Handle), ann(Ty::Str, Kind::Value)],
        "local label = handle.text\ndetailPrint(label)",
    ),
    exposed(
        "SectionSetText",
        "handle.text",
        &[bound(Ty::Handle), ann(Ty::Str, Kind::Value)],
        "handle.text = \"Core files\"",
    ),
    exposed(
        "SectionGetSize",
        "handle.size",
        &[bound(Ty::Handle), ann(Ty::nonneg(), Kind::Value)],
        "local kilobytes = handle.size\ndetailPrint(\"charging \" .. kilobytes .. \" KB\")",
    ),
    exposed(
        "SectionSetSize",
        "handle.size",
        &[bound(Ty::Handle), ann(Ty::nonneg(), Kind::Value)],
        "handle.size = 4096",
    ),
    // The four install-type rows, addressed by the name the block declared
    // rather than by a handle: an install type is a line in a block's field and
    // there is nothing for a `local` to bind (§13, ruling 4).
    //
    // `currentInstType` is a name the compiler owns — the read is one
    // instruction and a comparison chain, the write is another — so it is
    // `Bound` on both sides and is written like a variable rather than called.
    exposed(
        "GetCurInstType",
        "currentInstType",
        &[bound(Ty::nonneg())],
        "local chosen = currentInstType\ndetailPrint(\"installing \" .. chosen)",
    ),
    exposed(
        "SetCurInstType",
        "currentInstType",
        &[bound(Ty::nonneg())],
        "currentInstType = \"Minimal\"",
    ),
    // The one table in the surface addressed by string. The position is `Bound`
    // in NSIS's reading and a `string` in the caller's: what is written is the
    // declared name, which is why these two are calls where the four above are
    // not.
    exposed(
        "InstTypeSetText",
        "instTypes.setText",
        &[ann(Ty::Str, Kind::Value), ann(Ty::Str, Kind::Value)],
        "instTypes.setText(\"Full\", \"Everything\")",
    ),
    exposed(
        "InstTypeGetText",
        "instTypes.getText",
        &[ann(Ty::Str, Kind::Value), ann(Ty::Str, Kind::Value)],
        "local label = instTypes.getText(\"Full\")\ndetailPrint(label)",
    ),
    // Three fields at once — `checked`, `value` and the second half of `font` —
    // because a message *is* the setter for most of what a control holds. The
    // one field it cannot serve is reading `value`: `WM_GETTEXT` wants a buffer
    // and NSIS has nowhere to put one, so that read is `System::Call`.
    lowering(
        "SendMessage",
        "a control's fields: `agree.checked = true`, `serial.value = \"\"` (§15.32)",
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
    lowering(
        "SetCtlColors",
        "a control's `colors`: `serial.colors = { text = \"800000\", background = \"transparent\" }` \
         (§15.32)",
    ),
    // Its reason said page callbacks did not exist, and batch 20 gave every
    // page `pre`, `show` and `leave`. The control it writes into is the one
    // `brandingImage` creates, which has been an attribute for as long.
    flagged(
        exposed(
            "SetBrandingImage",
            "setBrandingImage",
            &[ann(Ty::Str, Kind::Path)],
            "setBrandingImage(\"assets/icon.ico\", { imgId = 1032, resizeToFit = true })",
        ),
        &[
            valued("imgId", Ty::nonneg(), Kind::Value),
            named("resizeToFit"),
        ],
    ),
    lowering(
        "LoadAndSetImage",
        "a `bitmap`'s `image`: `bitmap { image = \"check.bmp\", y = 90, height = 20 }` (§15.32)",
    ),
    // The four compression settings and the overwrite default. All five carry
    // the objection that retired itself: they are positional, so a *call* would
    // be a lie inside an `if` — but an attribute is not a call. It is the
    // installer-wide default, written once, and the compiler decides where the
    // line goes. That is the footing `SetCompressor` has stood on since batch
    // 1, and the only reason these four were not beside it is that nobody
    // looked at the group again after writing the reason.
    //
    // The per-`file` override is a different feature and still absent: `file`
    // has no `overwrite = …` yet. Withholding the installer-wide default until
    // that exists would be withholding the common case for the rare one.
    attribute("SetCompress", "compress", Setting::Enum),
    attribute("SetCompressor", "compressor", Setting::Enum),
    // `dict_size_mb`, so an `Int` in megabytes — and LZMA's alone. The comment
    // that stood here called that "a fact about the value and not a shape", and
    // measuring is what overturned it: `makensis` emits *warning 8026:
    // SetCompressorDictSize: compressor is not set to LZMA. Effectively
    // ignored.*, which this compiler's own `-WX` turns into an error naming the
    // NSIS command rather than the field.
    attribute(
        "SetCompressorDictSize",
        "compressorDictSize",
        Setting::Only {
            of: &Setting::Int,
            sibling: "compressor",
            holds: &["lzma"],
            default: "zlib",
        },
    ),
    // `level_0-9` in the snapshot, which is an `Int` with a range no `Setting`
    // can state. `makensis` states it — *Invalid compression level* — and by
    // name, so deferring costs a worse message and nothing else.
    //
    // The exact complement of the row above, and warning 8025 is its exact
    // mirror: the two settings partition the compressors between them, which is
    // why neither is worth a shape alone and both are worth one together.
    attribute(
        "SetCompressionLevel",
        "compressionLevel",
        Setting::Only {
            of: &Setting::Int,
            sibling: "compressor",
            holds: &["zlib", "bzip2"],
            default: "zlib",
        },
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
    // Five keywords, not a boolean: `on | off | try | ifnewer | ifdiff`. The
    // installer-wide default; a per-`file` override is a `file` feature and is
    // not this row.
    attribute("SetOverwrite", "overwrite", Setting::Enum),
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
    // The runtime half of `silentInstall`, which is an attribute and therefore
    // decided at build time. This is the only way to make a run silent — or to
    // make it *stop* being silent — from something the installer reads on the
    // machine it is running on.
    //
    // The row was written once before and reverted, because `makensis -WX`
    // takes `SetSilent silent` inside a section without a word and NSIS then
    // ignores it: neither test tier can see a call that assembles and does
    // nothing. `Place` is what the reverted attempt was missing, and it is a
    // compiler rule rather than a test because that is where the difference is
    // visible at all.
    on_init(exposed(
        "SetSilent",
        "setSilent",
        &[ann(Ty::Str, Kind::Enum)],
        "setSilent(\"silent\")",
    )),
    attribute("ShowInstDetails", "showInstDetails", Setting::Enum),
    attribute("ShowUninstDetails", "showUninstDetails", Setting::Enum),
    lowering(
        "ShowWindow",
        "a control's `visible`: `badge.visible = false` (§15.32)",
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
    // The five indices are the classic UI's page list, and this language names
    // those pages — so the row is not one field but four, `subCaption` on each
    // page it numbers, and the block it is written in picks the command (§15.3).
    //
    // Index 4, *Completed*, is the one MUI2 claims: `MUI_PAGE_INSTFILES` writes
    // `SubCaption 4 " "`. It needs no shape to refuse it, because 3 and 4 are
    // the same page in two states — installing, then done — and this language
    // has a name for the page and none for the state. The row that was waiting
    // for "owned at one argument value and open at the others" was waiting for
    // a shape it turns out not to need.
    attribute(
        "SubCaption",
        "page.*.subCaption",
        Setting::Handled("string"),
    ),
    // `Target x86-unicode` is `cpu` and `unicode` hyphenated together, and both
    // of those are rows already. A third spelling would also be a second way to
    // set `unicode`, which is not a line but a field the emitter reads before it
    // writes anything — so the two rows below are the whole of it.
    rejected(
        "Target",
        "one word for `cpu` and `unicode`, which are separate settings here",
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
    language("UninstPage", "`page.*` inside `uninstaller {}`"),
    // `locationText` beside it. `confirm` is the uninstaller's first page and
    // exists in no other half, which is why the field path has no `un.` in it:
    // the block the page is written in supplies that (§15.3).
    attribute("UninstallText", "page.confirm.topText", STR),
    // Three indices where the installer has five, and they are not the same
    // three: `confirm` is 0 and `instFiles` is 1, so a page carries both
    // numbers and `subCaption` on a `license` page inside `uninstaller {}` is
    // refused — NSIS gives it no number, which is a smaller thing than a
    // decision. Index 2 is MUI2's, exactly as index 4 is above.
    attribute(
        "UninstallSubCaption",
        "page.*.subCaption in `uninstaller {}`",
        Setting::Handled("string"),
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
    // MUI2 emits `XPStyle On` unconditionally, from inside `MUI_INTERFACE`, so
    // the only value a user could want is `off` and it is exactly the one that
    // cannot work: whichever line lands last wins, silently, and `-WX` sees
    // nothing wrong with either. Exposing it would mean promising an ordering
    // the manifest may override anyway — the visual style is the shell's
    // decision, not the script's.
    //
    // Rejecting says so once, at the place the author wrote it, instead of
    // producing an installer whose appearance depends on a link order.
    rejected(
        "XPStyle",
        "MUI2 emits `XPStyle On` itself, so an `off` here is a last-one-wins race with \
         no diagnostic; visual style is the shell's decision",
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
    // The second alternation, and the same shape: `spaceTexts = false` hides
    // both labels on the components page, a table rewrites them. `available`
    // may be left out, which is NSIS's `[available]` and not a choice here.
    attribute(
        "SpaceTexts",
        "spaceTexts",
        Setting::Off {
            word: "none",
            parts: &[part("required", STR), part("available", STR)],
            least: 1,
        },
    ),
    attribute("CompletedText", "completedText", STR),
    // Its old reason — §3's "`Call`-by-address has no Lua shape" — is still true
    // of the *surface*, and the events are why it stops being a backlog entry
    // anyway: the address of a generated callback exists in exactly one place,
    // and the program that wants it wrote `onClick`. `GetLabelAddress` keeps the
    // reason, because §8 owns labels and there is nothing to take the address of.
    lowering(
        "GetFunctionAddress",
        "an event: `button { \"Check\", onClick = function() … end }` (§15.32)",
    ),
    // The two that stayed behind when `GetFunctionAddress` became a lowering
    // target, and they stayed for a reason that is final rather than pending.
    // §8 owns labels: there is no label in this language to take the address of,
    // and §3 has no value type an address could be held in — the number these
    // produce is only ever consumed by `Call`, which reaches its target by name.
    // `GetCurrentAddress` is worse still, since "here" in a compiled body is not
    // a position any Installua program can name.
    rejected(
        "GetLabelAddress",
        "§8 owns labels, so there is none to address; `Call` reaches its target by name (§3)",
    ),
    rejected(
        "GetCurrentAddress",
        "the address of the current instruction, which no Installua program has a name for (§3, §8)",
    ),
    directive("!addplugindir"),
    // `ReserveFile /plugin`'s twin, and compiler-written for the same reason:
    // `$PLUGINSDIR` expands to nothing until something creates it, and NSIS
    // assembles the program that forgot without a word. Every body that names
    // the directory opens with the line, which is a fact the compiler can see
    // and a user has to remember. See [`crate::lower::plugins_dir`].
    lowering("InitPluginsDir", "a body that names `PLUGINSDIR`"),
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
