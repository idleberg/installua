//! The MUI2 page tables: which pages exist, what each takes, and what NSIS
//! define every field writes.
//!
//! Data, not lowering. It lives apart from [`super`] because it is edited for
//! its own reason — MUI2 gained a setting, or this language exposed one it
//! already had — and that reason has nothing to do with the rest of the
//! lowerer, which is edited for attributes, bodies and constants.

use super::{Half, V1_INSTALLER_FIELDS};

/// What one field of a `page.* {}` table holds.
#[derive(Clone, Copy, Debug)]
pub(super) enum Holds {
    Str,
    /// `true` defines it and `false` does not: the setting **is** the define's
    /// existence — MUI2 asks `!ifdef` and never expands it — so there is no
    /// value to write and no third state to have.
    Flag,
    /// `subCaption = "Terms"`: an NSIS line this compiler writes itself, which
    /// no other page setting is.
    ///
    /// The doc comment on [`PageField`] says the line is absent on purpose,
    /// because MUI2 writes `DirText` and `ComponentText` from its own defines
    /// and a second line loses the race. This row is the exception that proves
    /// the rule rather than a hole in it: MUI2 writes **one** `SubCaption` in
    /// its entire source — `SubCaption 4 " "`, inside `MUI_PAGE_INSTFILES`,
    /// blanking the *Completed* caption — and index 4 is the one this language
    /// has no page for. Indices 0 to 3 are unclaimed, so there is nothing to
    /// race.
    ///
    /// The payload is the page's index in each half, `None` where the page has
    /// no number in that half's numbering.
    Caption([Option<u8>; 2]),
    /// A global, named bare: `variable = target`. NSIS wants a *variable* in
    /// this position rather than a value, because `DirVar` stores into it, so a
    /// string would be the wrong kind of thing even where it reads alike.
    Var,
    /// `function() … end`, lowered to an NSIS `Function` MUI2 calls by name.
    Callback,
    /// A string that also turns its setting on: `checkbox = "I accept"` is
    /// `MUI_LICENSEPAGE_CHECKBOX` *and* `…_CHECKBOX_TEXT`, because MUI2 asks
    /// `!ifdef` about the first and expands the second. The payload is the
    /// second, and it is cleared exactly when the first is.
    Text(&'static str),
    /// A table whose keys are further defines, and whose presence turns the
    /// setting on the way [`Holds::Text`] does.
    Nested(&'static [PageField]),
    /// `colors = { text = "000000", background = "FFFFFF" }`: the field's own
    /// define takes the background and the payload takes the text.
    ///
    /// One field holding two for the reason a control's fields give one level
    /// down — MUI2 spends both of these on a single `SetCtlColors`, so
    /// `bgColor` and `textColor` as separate fields would let a script write
    /// one and get the other from whatever MUI2 had defaulted it to. It is also
    /// what the pair's cross-field constraint becomes: MUI2 reads the text
    /// colour only inside an `!ifdef` on the background, so a text colour
    /// written alone is read by nothing, and a shape that asks for both cannot
    /// say the case that does nothing.
    Colors(&'static str),
    /// A string and the one define that gives it more room: `title = "Done"`,
    /// or `title = { text = "Done", lines = 3 }`.
    ///
    /// One field holding two, because the second is geometry for the first and
    /// nothing else. `MUI_FINISHPAGE_TITLE_3LINES` and `…_TEXT_LARGE` change
    /// the height of the box the string is drawn in — a field apiece would let
    /// a script make room and never say what goes in it, and the plain string
    /// stays the plain string.
    Roomy(&'static str, Room),
    /// A table of parts whose *presence* means the setting is on, where the
    /// field's own define is MUI2's opt-**out**: `reboot = false` writes
    /// `MUI_FINISHPAGE_NOREBOOTSUPPORT` and a table writes nothing but its
    /// parts.
    ///
    /// [`Holds::Nested`] inverted, and inverted because MUI2 is: the three
    /// reboot strings and `REBOOTLATER_DEFAULT` are read only inside the
    /// `!ifndef MUI_FINISHPAGE_NOREBOOTSUPPORT` branch, so a table that also
    /// turned the opt-out on would write four defines nothing reads.
    Off(&'static [PageField]),
    /// One of two words, one of which writes the define and the other nothing:
    /// `default = "later"` is `MUI_FINISHPAGE_REBOOTLATER_DEFAULT` and
    /// `default = "now"` is MUI2's own default, which is the absence of it.
    ///
    /// Two words rather than a `bool` because the choice is between two named
    /// things and not between doing and not doing, and a build script can pass
    /// either one without branching.
    Word {
        writes: &'static str,
        silent: &'static str,
    },
    /// A callback that also turns its setting on, the way [`Holds::Text`] does
    /// a string: `call = function() … end` is `MUI_FINISHPAGE_RUN ""` *and*
    /// `…_RUN_FUNCTION`, because MUI2 asks `!ifdef` about the first — that is
    /// what draws the checkbox — and `Call`s the second when it is ticked.
    ///
    /// The stem names the function the compiler writes, since `call` is the
    /// same word under `run` and under `readme` and the two are different
    /// functions.
    Calls {
        function: &'static str,
        stem: &'static str,
    },
    /// A `bool` whose **`false`** writes the define, because MUI2's is an
    /// opt-out: `checked = false` is `MUI_FINISHPAGE_RUN_NOTCHECKED` and `true`
    /// is the absence of it. [`Holds::Flag`] the other way round.
    Not,
    /// A checkbox or a link: a table in one of a few spellings, where the
    /// spelling picks the parts and one of the parts writes the widget's own
    /// define.
    Widget(&'static [Form]),
    /// A box that is either worded or taken away: `checkbox = "Do not create
    /// shortcuts"` writes the payload, `checkbox = false` writes the field's own
    /// define — MUI2's `…_NODISABLE` — and `true` writes neither, which is
    /// MUI2's own box with MUI2's own words.
    ///
    /// One field holding two for [`Holds::Colors`]'s reason: `StartMenu.nsh`
    /// expands `…_TEXT_CHECKBOX` only inside the `!ifndef …_NODISABLE` branch,
    /// so a wording written beside the switch that removes the box is a define
    /// nothing reads, and a shape that asks for both cannot say the case that
    /// does nothing.
    Checkbox(&'static str),
}

/// One spelling of a [`Holds::Widget`].
///
/// `run` has two — a program to `Exec` and a function to `Call` — and they are
/// two part lists rather than one list with optional members, because that is
/// what makes `parameters` beside a function unspellable: MUI2 expands
/// `MUI_FINISHPAGE_RUN_PARAMETERS` only in the branch where there is no
/// function, so a shape that permitted both would write a define nothing reads.
#[derive(Debug)]
pub(super) struct Form {
    /// The part that has to be written, and that writes the widget's own
    /// define. Which key is present is what picks the spelling.
    pub(super) key: &'static str,
    /// The parts that have to come with it. `link` is a label *and* the place
    /// it goes: MUI2 writes a click handler that `ExecShell`s
    /// `MUI_FINISHPAGE_LINK_LOCATION` under `!ifdef MUI_FINISHPAGE_LINK`, so a
    /// label without one is a link to nowhere.
    pub(super) needs: &'static [&'static str],
    pub(super) parts: &'static [PageField],
}

/// How a [`Holds::Roomy`] field spells its second half.
///
/// Two spellings and not one, because MUI2's two are not the same kind of
/// switch: the title box is two lines tall or three, and the body text either
/// gets the taller box or does not.
#[derive(Clone, Copy, Debug)]
pub(super) enum Room {
    /// `lines = 3`.
    Lines,
    /// `large = true`.
    Large,
}

/// One setting of one page: the name a user writes, the `MUI_*` define it
/// becomes, and what it holds.
///
/// The NSIS line is absent on purpose. `DirText`, `ComponentText` and
/// `LicenseText` are written by MUI2, from these defines, inside the `PageEx`
/// it generates; a second one written by us assembles clean under `-WX` and
/// then loses the race. What is private to MUI2 is the **line**, and what stays
/// public is the **setting** — the split `icon`/`MUI_ICON` already lives on.
#[derive(Debug)]
pub(super) struct PageField {
    pub(super) installua: &'static str,
    pub(super) define: &'static str,
    pub(super) holds: Holds,
    /// Whether MUI2 clears the define once the page is inserted. `false` means
    /// the compiler emits the `!undef` itself, and that is not an alternative
    /// to MUI2's cleanup but the two holes in it: `UninstallConfirm.nsh` never
    /// clears `MUI_UNCONFIRMPAGE_VARIABLE`, and `License.nsh` clears
    /// `MUI_LICENSEPAGE_CHECKBOX_TEXT_ACCEPT`, which is a name nothing defines.
    /// Without this a second page of the same type inherits the first one's.
    pub(super) cleared: bool,
}

/// The one field a page has that is not a [`PageField`], because it is not a
/// `!define` at all.
///
/// Two of the eight have one. `license`'s `file` is an argument of the
/// `!insertmacro` rather than a setting read from inside it, and `custom`'s
/// `controls` is what the compiler draws — no define exists for either, so
/// there is nothing for the table to hold and they are named here instead.
pub(super) fn extra_field(page: &Page) -> Option<&'static str> {
    match page.installua {
        "license" => Some("file"),
        "custom" => Some("controls"),
        _ => None,
    }
}

pub(super) const fn field(
    installua: &'static str,
    define: &'static str,
    holds: Holds,
) -> PageField {
    PageField {
        installua,
        define,
        holds,
        cleared: true,
    }
}

/// `subCaption`, the one page setting that is an NSIS **line** and not a
/// `!define`.
///
/// The pair is the page's index in each half, because `SubCaption` and
/// `UninstallSubCaption` number their own pages and neither numbering is the
/// other's: `instFiles` is 3 installing and 1 uninstalling, and three of the
/// five installer pages have no uninstaller number at all. `define` is empty
/// and unread — see [`Holds::Caption`] for why this one is safe to write where
/// `ComponentText` is not.
pub(super) const fn caption(indices: [Option<u8>; 2]) -> PageField {
    PageField {
        installua: "subCaption",
        define: "",
        holds: Holds::Caption(indices),
        cleared: true,
    }
}

/// The same, for a define MUI2 leaves standing.
pub(super) const fn sticky(
    installua: &'static str,
    define: &'static str,
    holds: Holds,
) -> PageField {
    PageField {
        installua,
        define,
        holds,
        cleared: false,
    }
}

/// The three hooks every page has. MUI2 clears all three itself, in
/// `MUI_PAGE_FUNCTION_CUSTOM`.
pub(super) const COMMON_FIELDS: &[PageField] = &[
    field("pre", "MUI_PAGE_CUSTOMFUNCTION_PRE", Holds::Callback),
    field("show", "MUI_PAGE_CUSTOMFUNCTION_SHOW", Holds::Callback),
    field("leave", "MUI_PAGE_CUSTOMFUNCTION_LEAVE", Holds::Callback),
];

/// The fourth hook, and why it is not in `COMMON_FIELDS`: `Pages.nsh` writes it
/// the same way as the other three, but only the nsDialogs pages — welcome,
/// finish and the start menu — insert `MUI_PAGE_FUNCTION_CUSTOM DESTROYED`.
/// Offered on `page.directory` it would define a name nothing ever calls.
pub(super) const DESTROYED_FIELD: PageField = field(
    "destroyed",
    "MUI_PAGE_CUSTOMFUNCTION_DESTROYED",
    Holds::Callback,
);

/// The bold heading strip inside the page — not the title bar, which is
/// `caption` on the block, and not the page's own body text.
///
/// Absent from `welcome` and `finish`: those are full-window pages with no
/// header to write into, and `Pages.nsh` calls `MUI_HEADER_TEXT_PAGE` from the
/// other five and not from them.
pub(super) const HEADER_FIELDS: &[PageField] = &[
    field("headerText", "MUI_PAGE_HEADER_TEXT", Holds::Str),
    field("headerSubText", "MUI_PAGE_HEADER_SUBTEXT", Holds::Str),
];

/// The welcome page: a title, a body, and the hook the full-window pages have.
///
/// `text` is a plain string and not a [`Holds::Roomy`] one, unlike the finish
/// page's: MUI2 draws this box at a fixed 130u and has no `…_LARGE` for it.
pub(super) const WELCOME_FIELDS: &[PageField] = &[
    field(
        "title",
        "MUI_WELCOMEPAGE_TITLE",
        Holds::Roomy("MUI_WELCOMEPAGE_TITLE_3LINES", Room::Lines),
    ),
    field("text", "MUI_WELCOMEPAGE_TEXT", Holds::Str),
    DESTROYED_FIELD,
];

/// The install-log page's two endings.
///
/// MUI2 swaps the header strip when the copy stops: one wording for the run
/// that finished and one for the run that did not. Each half stands alone —
/// MUI2 falls back to its own language string for whichever is missing — so
/// these are four fields and not two pairs.
pub(super) const INSTFILES_FIELDS: &[PageField] = &[
    // 3 installing, 1 uninstalling. The only page with a number in both
    // halves, and the only one whose *other* number — 4 and 2, "Completed" —
    // is the pair MUI2 blanks: this page is that state, and this language has
    // no second name for it.
    caption([Some(3), Some(1)]),
    field(
        "finishHeaderText",
        "MUI_INSTFILESPAGE_FINISHHEADER_TEXT",
        Holds::Str,
    ),
    field(
        "finishHeaderSubText",
        "MUI_INSTFILESPAGE_FINISHHEADER_SUBTEXT",
        Holds::Str,
    ),
    // `sticky`, and this is MUI2's own hole rather than ours:
    // `InstallFiles.nsh` unsets `FINISHHEADER_*` and `ABORTWARNING_*` and
    // never unsets `ABORTHEADER_*`, which is the pair it actually reads. The
    // same class of typo as `License.nsh`'s `CHECKBOX_TEXT_ACCEPT`, and
    // without the compiler's own `!undef` a second instfiles page is headed
    // with the first one's abort wording.
    sticky(
        "abortHeaderText",
        "MUI_INSTFILESPAGE_ABORTHEADER_TEXT",
        Holds::Str,
    ),
    sticky(
        "abortHeaderSubText",
        "MUI_INSTFILESPAGE_ABORTHEADER_SUBTEXT",
        Holds::Str,
    ),
];

pub(super) const LICENSE_FIELDS: &[PageField] = &[
    caption([Some(0), None]),
    field("topText", "MUI_LICENSEPAGE_TEXT_TOP", Holds::Str),
    field("bottomText", "MUI_LICENSEPAGE_TEXT_BOTTOM", Holds::Str),
    field("button", "MUI_LICENSEPAGE_BUTTON", Holds::Str),
    field(
        "checkbox",
        "MUI_LICENSEPAGE_CHECKBOX",
        Holds::Text("MUI_LICENSEPAGE_CHECKBOX_TEXT"),
    ),
    // The two texts are `sticky` because of a typo in MUI2: `License.nsh`
    // unsets `MUI_LICENSEPAGE_CHECKBOX_TEXT_ACCEPT` and `…_DECLINE`, which are
    // not names anything defines — the radio button texts are spelled
    // `MUI_LICENSEPAGE_RADIOBUTTONS_TEXT_*` — so they survive the page and
    // `MUI_DEFAULT` on the next one declines to overwrite them.
    field(
        "radioButtons",
        "MUI_LICENSEPAGE_RADIOBUTTONS",
        Holds::Nested(&[
            sticky(
                "accept",
                "MUI_LICENSEPAGE_RADIOBUTTONS_TEXT_ACCEPT",
                Holds::Str,
            ),
            sticky(
                "decline",
                "MUI_LICENSEPAGE_RADIOBUTTONS_TEXT_DECLINE",
                Holds::Str,
            ),
        ]),
    ),
];

pub(super) const COMPONENTS_FIELDS: &[PageField] = &[
    caption([Some(1), None]),
    field("topText", "MUI_COMPONENTSPAGE_TEXT_TOP", Holds::Str),
    field(
        "instTypeText",
        "MUI_COMPONENTSPAGE_TEXT_INSTTYPE",
        Holds::Str,
    ),
    field("listText", "MUI_COMPONENTSPAGE_TEXT_COMPLIST", Holds::Str),
    // The description box's caption, and the words in it while the pointer is
    // over nothing. Page-scoped, unlike `smallDescriptions`: MUI2 reads both
    // through `MUI_DEFAULT` inside the page declaration and `MUI_UNSET`s them
    // after, so a second components page may differ.
    field(
        "descriptionTitle",
        "MUI_COMPONENTSPAGE_TEXT_DESCRIPTION_TITLE",
        Holds::Str,
    ),
    field(
        "descriptionText",
        "MUI_COMPONENTSPAGE_TEXT_DESCRIPTION_INFO",
        Holds::Str,
    ),
];

pub(super) const DIRECTORY_FIELDS: &[PageField] = &[
    caption([Some(2), None]),
    field("topText", "MUI_DIRECTORYPAGE_TEXT_TOP", Holds::Str),
    field(
        "destinationText",
        "MUI_DIRECTORYPAGE_TEXT_DESTINATION",
        Holds::Str,
    ),
    field("variable", "MUI_DIRECTORYPAGE_VARIABLE", Holds::Var),
    field(
        "verifyOnLeave",
        "MUI_DIRECTORYPAGE_VERIFYONLEAVE",
        Holds::Flag,
    ),
    // `sticky` because `Directory.nsh` clears neither one: MUI2 reads them from
    // the Show function it writes per page and leaves them standing, so without
    // the `!undef` a second directory page is painted in the first one's
    // colours.
    sticky(
        "colors",
        "MUI_DIRECTORYPAGE_BGCOLOR",
        Holds::Colors("MUI_DIRECTORYPAGE_TEXTCOLOR"),
    ),
];

/// The reboot half of the finish page, which MUI2 draws instead of the normal
/// one when the install set `SetRebootFlag`.
///
/// Reached only through `reboot = { … }`, and that is the point: all four are
/// read inside `!ifndef MUI_FINISHPAGE_NOREBOOTSUPPORT`, so the shape that
/// writes them is the shape that cannot also have turned reboot support off.
pub(super) const REBOOT_FIELDS: &[PageField] = &[
    field("text", "MUI_FINISHPAGE_TEXT_REBOOT", Holds::Str),
    field("now", "MUI_FINISHPAGE_TEXT_REBOOTNOW", Holds::Str),
    field("later", "MUI_FINISHPAGE_TEXT_REBOOTLATER", Holds::Str),
    field(
        "default",
        "MUI_FINISHPAGE_REBOOTLATER_DEFAULT",
        Holds::Word {
            writes: "later",
            silent: "now",
        },
    ),
];

/// The run checkbox, spelled as a program to start.
pub(super) const RUN_PATH_FIELDS: &[PageField] = &[
    field("path", "MUI_FINISHPAGE_RUN", Holds::Str),
    field("parameters", "MUI_FINISHPAGE_RUN_PARAMETERS", Holds::Str),
    field("text", "MUI_FINISHPAGE_RUN_TEXT", Holds::Str),
    field("checked", "MUI_FINISHPAGE_RUN_NOTCHECKED", Holds::Not),
];

/// The same checkbox, spelled as a function to call. No `parameters`: MUI2
/// reads them only where it builds the `Exec` line.
pub(super) const RUN_CALL_FIELDS: &[PageField] = &[
    field(
        "call",
        "MUI_FINISHPAGE_RUN",
        Holds::Calls {
            function: "MUI_FINISHPAGE_RUN_FUNCTION",
            stem: "run",
        },
    ),
    field("text", "MUI_FINISHPAGE_RUN_TEXT", Holds::Str),
    field("checked", "MUI_FINISHPAGE_RUN_NOTCHECKED", Holds::Not),
];

pub(super) const RUN_FORMS: &[Form] = &[
    Form {
        key: "path",
        needs: &[],
        parts: RUN_PATH_FIELDS,
    },
    Form {
        key: "call",
        needs: &[],
        parts: RUN_CALL_FIELDS,
    },
];

/// The readme checkbox. The same two spellings as `run`, and no `parameters`
/// in either: MUI2 opens this one with `ExecShell open`, which takes none.
pub(super) const README_PATH_FIELDS: &[PageField] = &[
    field("path", "MUI_FINISHPAGE_SHOWREADME", Holds::Str),
    field("text", "MUI_FINISHPAGE_SHOWREADME_TEXT", Holds::Str),
    field(
        "checked",
        "MUI_FINISHPAGE_SHOWREADME_NOTCHECKED",
        Holds::Not,
    ),
];

pub(super) const README_CALL_FIELDS: &[PageField] = &[
    field(
        "call",
        "MUI_FINISHPAGE_SHOWREADME",
        Holds::Calls {
            function: "MUI_FINISHPAGE_SHOWREADME_FUNCTION",
            stem: "readme",
        },
    ),
    field("text", "MUI_FINISHPAGE_SHOWREADME_TEXT", Holds::Str),
    field(
        "checked",
        "MUI_FINISHPAGE_SHOWREADME_NOTCHECKED",
        Holds::Not,
    ),
];

pub(super) const README_FORMS: &[Form] = &[
    Form {
        key: "path",
        needs: &[],
        parts: README_PATH_FIELDS,
    },
    Form {
        key: "call",
        needs: &[],
        parts: README_CALL_FIELDS,
    },
];

/// The link along the bottom of the page: one spelling, and `url` is not
/// optional in it.
pub(super) const LINK_FIELDS: &[PageField] = &[
    field("text", "MUI_FINISHPAGE_LINK", Holds::Str),
    field("url", "MUI_FINISHPAGE_LINK_LOCATION", Holds::Str),
    field("color", "MUI_FINISHPAGE_LINK_COLOR", Holds::Str),
];

pub(super) const LINK_FORMS: &[Form] = &[Form {
    key: "text",
    needs: &["url"],
    parts: LINK_FIELDS,
}];

/// The finish page.
///
/// `autoClose` is not here and is a block field: MUI2 reads
/// `MUI_FINISHPAGE_NOAUTOCLOSE` from `MUI_FINISHPAGE_GUIINIT`, behind an
/// `!ifndef` on the half's own `WELCOMEFINISHPAGE_GUINIT`, so it is read on the
/// first welcome-or-finish page of that half and never again — the same rule
/// that put `checkBitmap` on the block.
pub(super) const FINISH_FIELDS: &[PageField] = &[
    field(
        "title",
        "MUI_FINISHPAGE_TITLE",
        Holds::Roomy("MUI_FINISHPAGE_TITLE_3LINES", Room::Lines),
    ),
    field(
        "text",
        "MUI_FINISHPAGE_TEXT",
        Holds::Roomy("MUI_FINISHPAGE_TEXT_LARGE", Room::Large),
    ),
    field("button", "MUI_FINISHPAGE_BUTTON", Holds::Str),
    field(
        "cancelEnabled",
        "MUI_FINISHPAGE_CANCEL_ENABLED",
        Holds::Flag,
    ),
    field(
        "reboot",
        "MUI_FINISHPAGE_NOREBOOTSUPPORT",
        Holds::Off(REBOOT_FIELDS),
    ),
    field("run", "MUI_FINISHPAGE_RUN", Holds::Widget(RUN_FORMS)),
    field(
        "readme",
        "MUI_FINISHPAGE_SHOWREADME",
        Holds::Widget(README_FORMS),
    ),
    field("link", "MUI_FINISHPAGE_LINK", Holds::Widget(LINK_FORMS)),
    DESTROYED_FIELD,
];

/// Where the page remembers the folder, as one field holding three.
///
/// `StartMenu.nsh` guards every read of the three with
/// `!ifdef …_REGISTRY_ROOT & …_REGISTRY_KEY & …_REGISTRY_VALUENAME`, so any one
/// of them alone is a define nothing reads — and any two are as well. A
/// [`Form`] with the other two under `needs` is exactly that constraint: the
/// shape that writes one is the shape that has written all three.
pub(super) const REGISTRY_FIELDS: &[PageField] = &[
    field("root", "MUI_STARTMENUPAGE_REGISTRY_ROOT", Holds::Str),
    field("key", "MUI_STARTMENUPAGE_REGISTRY_KEY", Holds::Str),
    field("value", "MUI_STARTMENUPAGE_REGISTRY_VALUENAME", Holds::Str),
];

pub(super) const REGISTRY_FORMS: &[Form] = &[Form {
    key: "root",
    needs: &["key", "value"],
    parts: REGISTRY_FIELDS,
}];

/// The Start Menu folder page.
///
/// The only page bound to a `local`, and the fields say why: MUI2 reads the
/// folder back through `MUI_STARTMENU_GETFOLDER <id>` and wraps the shortcut
/// writing in `MUI_STARTMENU_WRITE_BEGIN <id>`, so the page has a *name* that
/// install-time code uses. Nothing here spells that name — the id is the local
/// and the variable is minted beside it — which is the whole of what binding it
/// buys.
pub(super) const STARTMENU_FIELDS: &[PageField] = &[
    field(
        "defaultFolder",
        "MUI_STARTMENUPAGE_DEFAULTFOLDER",
        Holds::Str,
    ),
    field("topText", "MUI_STARTMENUPAGE_TEXT_TOP", Holds::Str),
    field(
        "checkbox",
        "MUI_STARTMENUPAGE_NODISABLE",
        Holds::Checkbox("MUI_STARTMENUPAGE_TEXT_CHECKBOX"),
    ),
    field(
        "registry",
        "MUI_STARTMENUPAGE_REGISTRY_ROOT",
        Holds::Widget(REGISTRY_FORMS),
    ),
    // No `colors`, and the reason is a typo in MUI2 rather than a decision
    // here. `StartMenu.nsh:141` paints `$mui.StartMenuMenu.FolderList`; the
    // variable it declares at line 17 and fills at line 136 is
    // `$mui.StartMenuPage.FolderList`. The line is reached only when
    // `MUI_STARTMENUPAGE_BGCOLOR` is defined, so a page that sets the colours
    // raises `warning 6000: unknown variable/constant` — and this compiler
    // assembles under `-WX`. The two defines are refused in the inventory with
    // that reason, which is the only place a name can be *unusable* rather than
    // unimplemented.
    DESTROYED_FIELD,
];

pub(super) const CONFIRM_FIELDS: &[PageField] = &[
    caption([None, Some(0)]),
    field("topText", "MUI_UNCONFIRMPAGE_TEXT_TOP", Holds::Str),
    field(
        "locationText",
        "MUI_UNCONFIRMPAGE_TEXT_LOCATION",
        Holds::Str,
    ),
    sticky("variable", "MUI_UNCONFIRMPAGE_VARIABLE", Holds::Var),
];

/// One MUI2 page, and the settings that belong to it rather than to the block.
///
/// Which of the two a setting is is MUI2's own source to say and not a
/// judgement: a setting written inside the generated `PageEx` and `!undef`'d
/// after is page-scoped, and one written inside an `!ifndef`-guarded
/// `MUI_*PAGE_INTERFACE` macro is applied once, on the first page of its type,
/// and ignored on every later one. The second kind is a block field — see
/// [`V1_INSTALLER_FIELDS`] — because putting it on the page would be a lie the
/// second page tells silently.
pub(super) struct Page {
    pub(super) installua: &'static str,
    pub(super) nsis: &'static str,
    /// Which halves MUI2 defines a macro for. `confirm` exists only as
    /// `MUI_UNPAGE_CONFIRM`, and there is no `MUI_PAGE_CONFIRM` to fall back
    /// on — so this is a fact about MUI2 rather than a policy of ours.
    pub(super) halves: [bool; 2],
    pub(super) header: bool,
    /// Whether the page is one NSIS inserts (`!insertmacro MUI_PAGE_*`) or one
    /// the compiler writes the body of (`Page custom`). Exactly one page is the
    /// second kind, and everything that differs about it follows from this:
    /// there is no MUI2 macro to configure, so the settings that are `!define`s
    /// on the other seven are arguments and instructions here.
    pub(super) custom: bool,
    pub(super) own: &'static [PageField],
}

pub(super) const fn page(
    installua: &'static str,
    nsis: &'static str,
    own: &'static [PageField],
) -> Page {
    Page {
        installua,
        nsis,
        halves: [true, true],
        header: true,
        custom: false,
        own,
    }
}

/// The eight pages, as a **closed set**: this is why a page is reached by
/// member access (`page.directory`) where a section is reached by string
/// (`section("Tools", …)`). A user picks a section's name and MUI2 picks these,
/// so one completes and the other cannot.
///
/// Seven of them are MUI2's and the eighth is not, and it is still in the same
/// list for the same reason: what a user picks from is a set an editor can
/// finish, and where the page's body comes from is not a fact about the name.
pub(super) const V1_PAGES: &[Page] = &[
    Page {
        installua: "welcome",
        nsis: "WELCOME",
        halves: [true, true],
        header: false,
        custom: false,
        own: WELCOME_FIELDS,
    },
    page("license", "LICENSE", LICENSE_FIELDS),
    page("components", "COMPONENTS", COMPONENTS_FIELDS),
    page("directory", "DIRECTORY", DIRECTORY_FIELDS),
    page("instFiles", "INSTFILES", INSTFILES_FIELDS),
    // Installer-only, and that is MUI2's fact rather than our policy: there is
    // no `MUI_UNPAGE_STARTMENU`. The uninstaller reaches the same folder
    // through `MUI_STARTMENU_GETFOLDER`, which is what a `menu.folder` read in
    // that half becomes.
    Page {
        installua: "startMenu",
        nsis: "STARTMENU",
        halves: [true, false],
        header: true,
        custom: false,
        own: STARTMENU_FIELDS,
    },
    Page {
        installua: "finish",
        nsis: "FINISH",
        halves: [true, true],
        header: false,
        custom: false,
        own: FINISH_FIELDS,
    },
    Page {
        installua: "confirm",
        nsis: "CONFIRM",
        halves: [false, true],
        header: true,
        custom: false,
        own: CONFIRM_FIELDS,
    },
    // The eighth, and the only one that is not MUI2's. It is reached by the
    // same member access as the other seven because it is a *page* — the set
    // stays closed, and what a user picks is still from a list an editor can
    // complete. What it is not is a `!insertmacro`: `Page custom` names two
    // functions, and both of them are ours to write.
    Page {
        installua: "custom",
        nsis: "custom",
        halves: [true, true],
        header: true,
        custom: true,
        own: &[],
    },
];

impl Page {
    pub(super) fn has(&self, half: Half) -> bool {
        self.halves[half as usize]
    }

    /// Every field this page accepts, in the order the defines are emitted —
    /// which is this order and not the user's, because a Lua table has none.
    pub(super) fn fields(&self) -> impl Iterator<Item = &'static PageField> {
        let header: &'static [PageField] = if self.header { HEADER_FIELDS } else { &[] };
        self.own.iter().chain(header).chain(COMMON_FIELDS)
    }
}

/// The page surface as names: each page's spelling, the halves it exists in,
/// and every field it accepts.
///
/// Public for the stub's sake, and for the same reason [`mui_defines`] is public
/// for the inventory's: the editor's page classes are hand-written — a block's
/// value is an expression in a table and the parameter model has nothing to say
/// about it — so the only thing keeping them from drifting away from this table
/// is a test that can read both. It drifted once, and the whole of `startMenu`
/// went missing from completion for as long as nobody looked.
pub fn v1_page_surface() -> Vec<(&'static str, [bool; 2], Vec<&'static str>)> {
    V1_PAGES
        .iter()
        .map(|page| {
            (
                page.installua,
                page.halves,
                page.fields().map(|field| field.installua).collect(),
            )
        })
        .collect()
}

/// The `installer {}` and `uninstaller {}` field names, for the same reason.
pub fn v1_installer_fields() -> &'static [&'static str] {
    V1_INSTALLER_FIELDS
}

/// The block-level `MUI_*` defines, listed beside the arms of `block_field`
/// that write them.
///
/// A list *and* the arms, which is one name in two places — deliberately, and
/// only these six. The arms differ in ways a table would have to grow a column
/// for apiece (`icon` writes one of two names depending on the half, three of
/// the others are paths and two are not), and the duplication is caught rather
/// than trusted: `tests/mui.rs` asserts that this list and the MUI inventory's
/// `Exposed` rows are the same set.
pub(super) const BLOCK_MUI_DEFINES: &[&str] = &[
    "MUI_BGCOLOR",
    "MUI_TEXTCOLOR",
    "MUI_ICON",
    "MUI_UNICON",
    "MUI_COMPONENTSPAGE_CHECKBITMAP",
    "MUI_INSTFILESPAGE_COLORS",
    "MUI_INSTFILESPAGE_PROGRESSBAR",
    "MUI_LICENSEPAGE_BGCOLOR",
    "MUI_ABORTWARNING",
    "MUI_ABORTWARNING_TEXT",
    "MUI_ABORTWARNING_CANCEL_DEFAULT",
    // One name for two spellings, and the only entry here that is: MUI2 builds
    // this one with its uninstaller prefix, so the uninstaller's is
    // `MUI_UNFINISHPAGE_NOAUTOCLOSE` and the snapshot records the pair as the
    // single row the `un` tag marks.
    "MUI_FINISHPAGE_NOAUTOCLOSE",
    "MUI_UNABORTWARNING",
    "MUI_UNABORTWARNING_TEXT",
    "MUI_UNABORTWARNING_CANCEL_DEFAULT",
    // The `languages {}` dialog. Seven settings and no macro: the three macros
    // the block writes are `!insertmacro` lines rather than `!define`s, so they
    // are exposed without being here — same shape as `MUI_LANGUAGE` itself.
    "MUI_LANGDLL_WINDOWTITLE",
    "MUI_LANGDLL_INFO",
    "MUI_LANGDLL_ALLLANGUAGES",
    "MUI_LANGDLL_ALWAYSSHOW",
    "MUI_LANGDLL_REGISTRY_ROOT",
    "MUI_LANGDLL_REGISTRY_KEY",
    "MUI_LANGDLL_REGISTRY_VALUENAME",
    "MUI_HEADERIMAGE",
    "MUI_HEADERIMAGE_BITMAP",
    "MUI_HEADERIMAGE_BITMAP_STRETCH",
    "MUI_HEADERIMAGE_BITMAP_RTL",
    "MUI_HEADERIMAGE_BITMAP_RTL_STRETCH",
    // The uninstaller's four are spelled out rather than tagged `un`, because
    // MUI2 spells them out: `UNBITMAP` is a name of its own in the snapshot,
    // where `MUI_UNWELCOMEFINISHPAGE_BITMAP` below is not.
    "MUI_HEADERIMAGE_UNBITMAP",
    "MUI_HEADERIMAGE_UNBITMAP_STRETCH",
    "MUI_HEADERIMAGE_UNBITMAP_RTL",
    "MUI_HEADERIMAGE_UNBITMAP_RTL_STRETCH",
    "MUI_HEADERIMAGE_RIGHT",
    "MUI_HEADER_TRANSPARENT_TEXT",
    // Two more of the `un`-tagged kind, like `MUI_FINISHPAGE_NOAUTOCLOSE`: MUI2
    // builds these through `${_un}`, so one row apiece covers the uninstaller's
    // `MUI_UNWELCOMEFINISHPAGE_BITMAP` and `…_BITMAP_STRETCH` too.
    "MUI_WELCOMEFINISHPAGE_BITMAP",
    "MUI_WELCOMEFINISHPAGE_BITMAP_STRETCH",
    "MUI_COMPONENTSPAGE_SMALLDESC",
    // The hover hook, which is a define holding a function name and so belongs
    // here rather than beside the description block it is called from.
    "MUI_CUSTOMFUNCTION_ONMOUSEOVERSECTION",
    "MUI_CUSTOMFUNCTION_UNONMOUSEOVERSECTION",
    "MUI_CUSTOMFUNCTION_GUIINIT",
    "MUI_CUSTOMFUNCTION_UNGUIINIT",
    "MUI_CUSTOMFUNCTION_ABORT",
    "MUI_CUSTOMFUNCTION_UNABORT",
];

/// Every `MUI_*` define this compiler writes: the block's, then every page's,
/// then the nested ones a field expands into.
///
/// Public for the inventory's sake (`crate::mui`). A define the emitter writes
/// while the inventory still calls it `todo` is drift in the direction that
/// matters — the burndown claiming work is left when it is done — and the only
/// way to catch it is a list both sides can read.
pub fn mui_defines() -> Vec<&'static str> {
    fn walk(field: &'static PageField, out: &mut Vec<&'static str>) {
        // `subCaption` writes an NSIS line and no define at all, so it has
        // nothing to join against the MUI2 inventory. It is the only field that
        // does, and the empty name is what says so.
        if !field.define.is_empty() {
            out.push(field.define);
        }
        match field.holds {
            Holds::Text(text)
            | Holds::Colors(text)
            | Holds::Checkbox(text)
            | Holds::Roomy(text, _)
            | Holds::Calls { function: text, .. } => out.push(text),
            Holds::Nested(fields) | Holds::Off(fields) => {
                for nested in fields {
                    walk(nested, out);
                }
            }
            // Every spelling, since each writes what the others do not — and
            // the ones they share are deduped below.
            Holds::Widget(forms) => {
                for form in forms {
                    for part in form.parts {
                        walk(part, out);
                    }
                }
            }
            _ => {}
        }
    }

    let mut out: Vec<&'static str> = BLOCK_MUI_DEFINES.to_vec();
    for page in V1_PAGES {
        for field in page.fields() {
            walk(field, &mut out);
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}
