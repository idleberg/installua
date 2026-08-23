//! The judgement half of the MUI inventory: one row per snapshot name.
//!
//! The overlay's opposite number (§15.23). `scan` says what MUI2's own text does
//! with a name and this says what Installua does with it — and the split is the
//! same one for the same reason: a fact about MUI2 that a human retyped is a
//! fact that will be wrong after the next release, and a judgement a machine
//! inferred is one nobody can argue with.
//!
//! Four buckets, and `internal` is the largest. That is not a gap: MUI2 keeps
//! most of its state in `!define`s, so the majority of what the headers mention
//! is MUI2 talking to itself. Its own `Readme.html` is what tells the two apart,
//! which is why the snapshot carries a `doc` tag.

use super::{Class, Row};

const fn exposed(name: &'static str, how: &'static str) -> Row {
    Row {
        name,
        class: Class::Exposed(how),
    }
}

/// MUI2's own state. No text, because there is nothing to say beyond the
/// bucket: a user does not write it, is not missing it, and would break MUI2 by
/// writing it anyway.
const fn internal(name: &'static str) -> Row {
    Row {
        name,
        class: Class::Internal,
    }
}

const fn rejected(name: &'static str, why: &'static str) -> Row {
    Row {
        name,
        class: Class::Rejected(why),
    }
}

/// Unused, and kept: this census has no `Todo` left in it, which is a state to
/// be able to *lose* rather than one to bake in. The next MUI carries names
/// nobody has read yet, and the first of them wants a bucket to land in that is
/// neither "exposed" nor "we decided against it".
#[allow(dead_code)]
const fn todo(name: &'static str, why: &'static str) -> Row {
    Row {
        name,
        class: Class::Todo(why),
    }
}

/// Every name in `tables/mui-3.12.txt`, in that file's order.
///
/// A `todo` reason is written per *group* rather than per row, because these
/// settings arrive in groups: `MUI_FINISHPAGE_RUN` without `…_RUN_TEXT` is a
/// checkbox with no label. Phase 6 spent five batches learning that a reason
/// written once and never re-read is how a row stays blocked after its blocker
/// is gone, so a group that shares a reason shares it visibly.
pub const ROWS: &[Row] = &[
    exposed("MUI_ABORTWARNING", "installer { abortPrompt = true }"),
    exposed(
        "MUI_ABORTWARNING_CANCEL_DEFAULT",
        "installer { abortPrompt = { default = \"cancel\" } }",
    ),
    exposed(
        "MUI_ABORTWARNING_TEXT",
        "installer { abortPrompt = { text = \"…\" } }",
    ),
    exposed(
        "MUI_BGCOLOR",
        "installer { headerColors = { text = \"…\", background = \"…\" } }",
    ),
    internal("MUI_COMPONENTSPAGE"),
    exposed(
        "MUI_COMPONENTSPAGE_CHECKBITMAP",
        "installer { checkBitmap = \"…\" }",
    ),
    internal("MUI_COMPONENTSPAGE_INTERFACE"),
    exposed(
        "MUI_COMPONENTSPAGE_SMALLDESC",
        "installer { smallDescriptions = true }",
    ),
    exposed(
        "MUI_COMPONENTSPAGE_TEXT_COMPLIST",
        "page.components { listText = \"…\" }",
    ),
    exposed(
        "MUI_COMPONENTSPAGE_TEXT_DESCRIPTION_INFO",
        "page.components { descriptionText = \"…\" }",
    ),
    exposed(
        "MUI_COMPONENTSPAGE_TEXT_DESCRIPTION_TITLE",
        "page.components { descriptionTitle = \"…\" }",
    ),
    exposed(
        "MUI_COMPONENTSPAGE_TEXT_INSTTYPE",
        "page.components { instTypeText = \"…\" }",
    ),
    exposed(
        "MUI_COMPONENTSPAGE_TEXT_TOP",
        "page.components { topText = \"…\" }",
    ),
    exposed(
        "MUI_CUSTOMFUNCTION_ABORT",
        "installer { onUserAbort(function() … end) }",
    ),
    exposed(
        "MUI_CUSTOMFUNCTION_GUIINIT",
        "installer { onGUIInit(function() … end) }",
    ),
    exposed(
        "MUI_CUSTOMFUNCTION_ONMOUSEOVERSECTION",
        "installer { onMouseOverSection(function() … end) }",
    ),
    exposed(
        "MUI_CUSTOMFUNCTION_UNABORT",
        "uninstaller { onUserAbort(function() … end) }",
    ),
    exposed(
        "MUI_CUSTOMFUNCTION_UNGUIINIT",
        "uninstaller { onGUIInit(function() … end) }",
    ),
    exposed(
        "MUI_CUSTOMFUNCTION_UNONMOUSEOVERSECTION",
        "uninstaller { onMouseOverSection(function() … end) }",
    ),
    internal("MUI_DEFAULT"),
    internal("MUI_DESCRIPTION_BEGIN"),
    internal("MUI_DESCRIPTION_END"),
    exposed(
        "MUI_DESCRIPTION_TEXT",
        "section { description = \"…\" }, and `group` takes the same option",
    ),
    internal("MUI_DIRECTORYPAGE"),
    exposed(
        "MUI_DIRECTORYPAGE_BGCOLOR",
        "page.directory { colors = { text = \"…\", background = \"…\" } }",
    ),
    internal("MUI_DIRECTORYPAGE_INTERFACE"),
    exposed(
        "MUI_DIRECTORYPAGE_TEXTCOLOR",
        "page.directory { colors = { text = \"…\", background = \"…\" } }",
    ),
    exposed(
        "MUI_DIRECTORYPAGE_TEXT_DESTINATION",
        "page.directory { destinationText = \"…\" }",
    ),
    exposed(
        "MUI_DIRECTORYPAGE_TEXT_TOP",
        "page.directory { topText = \"…\" }",
    ),
    exposed(
        "MUI_DIRECTORYPAGE_VARIABLE",
        "page.directory { variable = target }",
    ),
    exposed(
        "MUI_DIRECTORYPAGE_VERIFYONLEAVE",
        "page.directory { verifyOnLeave = true }",
    ),
    rejected(
        "MUI_DISABLE_INSERT_LANGUAGE_AFTER_PAGES_WARNING",
        "it silences a warning about include order, and the compiler writes `MUI_LANGUAGE` after the pages by construction (§15.7)",
    ),
    internal("MUI_FINISHPAGE"),
    // MUI2's own guard define and a macro of the same name — the `!ifndef`
    // that keeps one abort-warning function per half, not a setting a script
    // writes.
    internal("MUI_FINISHPAGE_ABORTWARNING"),
    rejected(
        "MUI_FINISHPAGE_ABORTWARNINGCHECK",
        "MUI2 only `!undef`s it: the sole reader is `Modern UI/System.nsh`, which is MUI 1, so a define here would be read by nothing in MUI2",
    ),
    exposed("MUI_FINISHPAGE_BUTTON", "page.finish { button = \"Done\" }"),
    exposed(
        "MUI_FINISHPAGE_CANCEL_ENABLED",
        "page.finish { cancelEnabled = true }",
    ),
    internal("MUI_FINISHPAGE_CANCEL_ENABLED_VARIABLES"),
    internal("MUI_FINISHPAGE_CURFIELD_BOTTOM"),
    internal("MUI_FINISHPAGE_CURFIELD_TOP"),
    internal("MUI_FINISHPAGE_GUIINIT"),
    internal("MUI_FINISHPAGE_INTERFACE"),
    exposed(
        "MUI_FINISHPAGE_LINK",
        "page.finish { link = { text = \"…\", url = \"https://…\" } }",
    ),
    exposed(
        "MUI_FINISHPAGE_LINK_COLOR",
        "page.finish { link = { text = \"…\", url = \"…\", color = \"0000FF\" } }",
    ),
    exposed(
        "MUI_FINISHPAGE_LINK_LOCATION",
        "page.finish { link = { text = \"…\", url = \"https://…\" } }",
    ),
    internal("MUI_FINISHPAGE_LINK_VARIABLES"),
    // The one exposed name MUI2 spells with its uninstaller prefix, so this row
    // covers `MUI_UNFINISHPAGE_NOAUTOCLOSE` too — and a block field rather than
    // a page one, because MUI2 reads it once per half.
    exposed(
        "MUI_FINISHPAGE_NOAUTOCLOSE",
        "installer { autoClose = false }",
    ),
    exposed(
        "MUI_FINISHPAGE_NOREBOOTSUPPORT",
        "page.finish { reboot = false }",
    ),
    exposed(
        "MUI_FINISHPAGE_REBOOTLATER_DEFAULT",
        "page.finish { reboot = { default = \"later\" } }",
    ),
    internal("MUI_FINISHPAGE_REBOOTLATER_TOP"),
    internal("MUI_FINISHPAGE_REBOOTNOW_TOP"),
    internal("MUI_FINISHPAGE_REBOOT_VARIABLES"),
    internal("MUI_FINISHPAGE_RETURNVALUE_VARIABLES"),
    // The path spelling and the function spelling write the same name, which is
    // why the two are two shapes rather than one shape with two optional
    // members: `MUI_FINISHPAGE_RUN` is the checkbox's existence either way.
    exposed(
        "MUI_FINISHPAGE_RUN",
        "page.finish { run = \"$INSTDIR\\foo.exe\" }",
    ),
    exposed(
        "MUI_FINISHPAGE_RUN_FUNCTION",
        "page.finish { run = { call = function() … end } }",
    ),
    exposed(
        "MUI_FINISHPAGE_RUN_NOTCHECKED",
        "page.finish { run = { path = \"…\", checked = false } }",
    ),
    exposed(
        "MUI_FINISHPAGE_RUN_PARAMETERS",
        "page.finish { run = { path = \"…\", parameters = \"--first-run\" } }",
    ),
    exposed(
        "MUI_FINISHPAGE_RUN_TEXT",
        "page.finish { run = { path = \"…\", text = \"Run Foo now\" } }",
    ),
    internal("MUI_FINISHPAGE_RUN_TOP"),
    internal("MUI_FINISHPAGE_RUN_VARIABLES"),
    // Spelled `readme` and not `showReadme`: the define names the checkbox by
    // what ticking it does, and the field names the thing itself.
    exposed(
        "MUI_FINISHPAGE_SHOWREADME",
        "page.finish { readme = \"$INSTDIR\\README.txt\" }",
    ),
    exposed(
        "MUI_FINISHPAGE_SHOWREADME_FUNCTION",
        "page.finish { readme = { call = function() … end } }",
    ),
    exposed(
        "MUI_FINISHPAGE_SHOWREADME_NOTCHECKED",
        "page.finish { readme = { path = \"…\", checked = false } }",
    ),
    exposed(
        "MUI_FINISHPAGE_SHOWREADME_TEXT",
        "page.finish { readme = { path = \"…\", text = \"View the readme\" } }",
    ),
    internal("MUI_FINISHPAGE_SHOWREADME_TOP"),
    internal("MUI_FINISHPAGE_SHOWREADME_VARIABLES"),
    exposed("MUI_FINISHPAGE_TEXT", "page.finish { text = \"…\" }"),
    internal("MUI_FINISHPAGE_TEXT_BOTTOM_BUTTONS"),
    internal("MUI_FINISHPAGE_TEXT_HEIGHT"),
    internal("MUI_FINISHPAGE_TEXT_HEIGHT_BUTTONS"),
    exposed(
        "MUI_FINISHPAGE_TEXT_LARGE",
        "page.finish { text = { text = \"…\", large = true } }",
    ),
    exposed(
        "MUI_FINISHPAGE_TEXT_REBOOT",
        "page.finish { reboot = { text = \"…\" } }",
    ),
    exposed(
        "MUI_FINISHPAGE_TEXT_REBOOTLATER",
        "page.finish { reboot = { later = \"…\" } }",
    ),
    exposed(
        "MUI_FINISHPAGE_TEXT_REBOOTNOW",
        "page.finish { reboot = { now = \"…\" } }",
    ),
    internal("MUI_FINISHPAGE_TEXT_TOP"),
    exposed("MUI_FINISHPAGE_TITLE", "page.finish { title = \"…\" }"),
    exposed(
        "MUI_FINISHPAGE_TITLE_3LINES",
        "page.finish { title = { text = \"…\", lines = 3 } }",
    ),
    internal("MUI_FINISHPAGE_TITLE_HEIGHT"),
    rejected(
        "MUI_FORCECLASSICCONTROLS",
        "undocumented, and it turns the finish page back into a pre-XP one: an option to look older is not one this compiler offers",
    ),
    internal("MUI_FUNCTION_ABORTWARNING"),
    internal("MUI_FUNCTION_COMPONENTSPAGE"),
    internal("MUI_FUNCTION_DESCRIPTION_BEGIN"),
    internal("MUI_FUNCTION_DESCRIPTION_END"),
    internal("MUI_FUNCTION_DIRECTORYPAGE"),
    internal("MUI_FUNCTION_FINISHPAGE"),
    internal("MUI_FUNCTION_GUIINIT"),
    internal("MUI_FUNCTION_INSTFILESPAGE"),
    internal("MUI_FUNCTION_LICENSEPAGE"),
    internal("MUI_FUNCTION_STARTMENUPAGE"),
    internal("MUI_FUNCTION_UNABORTWARNING"),
    internal("MUI_FUNCTION_WELCOMEPAGE"),
    internal("MUI_GUIINIT_OUTERDIALOG"),
    exposed(
        "MUI_HEADERIMAGE",
        "installer { headerImage = \"header.bmp\" }",
    ),
    exposed(
        "MUI_HEADERIMAGE_BITMAP",
        "installer { headerImage = \"header.bmp\" }",
    ),
    exposed(
        "MUI_HEADERIMAGE_BITMAP_RTL",
        "installer { headerImage = { file = \"…\", rtl = \"header-rtl.bmp\" } }",
    ),
    exposed(
        "MUI_HEADERIMAGE_BITMAP_RTL_STRETCH",
        "installer { headerImage = { rtl = { file = \"…\", stretch = \"FitControl\" } } }",
    ),
    exposed(
        "MUI_HEADERIMAGE_BITMAP_STRETCH",
        "installer { headerImage = { file = \"…\", stretch = \"AspectFitHeight\" } }",
    ),
    internal("MUI_HEADERIMAGE_INIT"),
    internal("MUI_HEADERIMAGE_INITHELPER_LOADIMAGE"),
    internal("MUI_HEADERIMAGE_INITHELPER_LOADIMAGEWITHMACRO"),
    exposed(
        "MUI_HEADERIMAGE_RIGHT",
        "installer { headerImage = { file = \"…\", right = true } }",
    ),
    exposed(
        "MUI_HEADERIMAGE_UNBITMAP",
        "uninstaller { headerImage = \"header-un.bmp\" }",
    ),
    exposed(
        "MUI_HEADERIMAGE_UNBITMAP_RTL",
        "uninstaller { headerImage = { file = \"…\", rtl = \"header-un-rtl.bmp\" } }",
    ),
    exposed(
        "MUI_HEADERIMAGE_UNBITMAP_RTL_STRETCH",
        "uninstaller { headerImage = { rtl = { file = \"…\", stretch = \"FitControl\" } } }",
    ),
    exposed(
        "MUI_HEADERIMAGE_UNBITMAP_STRETCH",
        "uninstaller { headerImage = { file = \"…\", stretch = \"AspectFitHeight\" } }",
    ),
    exposed(
        "MUI_HEADER_TEXT",
        "page.custom { headerText = \"…\" }, written by the creator",
    ),
    internal("MUI_HEADER_TEXT_PAGE"),
    exposed(
        "MUI_HEADER_TRANSPARENT_TEXT",
        "installer { headerImage = { file = \"…\", transparentText = true } }",
    ),
    exposed("MUI_ICON", "installer { icon = \"app.ico\" }"),
    internal("MUI_INCLUDED"),
    internal("MUI_INSERT"),
    internal("MUI_INSERT_NSISCONF"),
    internal("MUI_INSTFILESPAGE"),
    exposed(
        "MUI_INSTFILESPAGE_ABORTHEADER_SUBTEXT",
        "page.instFiles { abortHeaderSubText = \"Setup was not completed.\" }",
    ),
    exposed(
        "MUI_INSTFILESPAGE_ABORTHEADER_TEXT",
        "page.instFiles { abortHeaderText = \"Installation aborted\" }",
    ),
    // The pair beside them, and read by nothing: `InstallFiles.nsh` mentions
    // these two only to `MUI_UNSET` them, and the sole other file that names
    // them is `Modern UI/System.nsh`, which is MUI 1. A define here would be a
    // setting with no reader.
    rejected(
        "MUI_INSTFILESPAGE_ABORTWARNING_SUBTEXT",
        "MUI2 only `!undef`s it: the pair MUI2 reads is `…_ABORTHEADER_*`, and the only file that reads this spelling is MUI 1's `Modern UI/System.nsh`",
    ),
    rejected(
        "MUI_INSTFILESPAGE_ABORTWARNING_TEXT",
        "MUI2 only `!undef`s it: the pair MUI2 reads is `…_ABORTHEADER_*`, and the only file that reads this spelling is MUI 1's `Modern UI/System.nsh`",
    ),
    exposed(
        "MUI_INSTFILESPAGE_COLORS",
        "installer { installColors = \"…\" }",
    ),
    exposed(
        "MUI_INSTFILESPAGE_FINISHHEADER_SUBTEXT",
        "page.instFiles { finishHeaderSubText = \"Foo is installed.\" }",
    ),
    exposed(
        "MUI_INSTFILESPAGE_FINISHHEADER_TEXT",
        "page.instFiles { finishHeaderText = \"Installation complete\" }",
    ),
    internal("MUI_INSTFILESPAGE_INTERFACE"),
    exposed(
        "MUI_INSTFILESPAGE_PROGRESSBAR",
        "installer { progressBar = \"smooth\" }",
    ),
    internal("MUI_INSTFILESYPAGE_INTERFACE"),
    internal("MUI_INTERFACE"),
    internal("MUI_INTERNAL_FULLWINDOW_LOADWIZARDIMAGE"),
    internal("MUI_INTERNAL_LOADANDASPECTSTRETCHIMAGETOCONTROLHEIGHT"),
    internal("MUI_INTERNAL_LOADANDSIZEIMAGE"),
    internal("MUI_INTERNAL_LOADANDXALIGNIMAGE"),
    exposed(
        "MUI_LANGDLL_ALLLANGUAGES",
        "languages { ask = { allLanguages = true } } — every language, not only the ones the machine has a code page for",
    ),
    exposed(
        "MUI_LANGDLL_ALWAYSSHOW",
        "languages { ask = { alwaysShow = true } } — ask again even when `remember` has an answer",
    ),
    exposed(
        "MUI_LANGDLL_DISPLAY",
        "written into `.onInit` whenever `languages { ask }` is present, never spelled (§15.26)",
    ),
    exposed("MUI_LANGDLL_INFO", "languages { ask = { info = \"…\" } }"),
    internal("MUI_LANGDLL_LANGUAGES"),
    internal("MUI_LANGDLL_LANGUAGES_CP"),
    exposed(
        "MUI_LANGDLL_REGISTRY_KEY",
        "languages { ask = { remember = { key = \"…\" } } }, one of three (§15.23)",
    ),
    exposed(
        "MUI_LANGDLL_REGISTRY_ROOT",
        "languages { ask = { remember = { root = \"HKCU\" } } }, one of three (§15.23)",
    ),
    exposed(
        "MUI_LANGDLL_REGISTRY_VALUENAME",
        "languages { ask = { remember = { value = \"…\" } } }, one of three (§15.23)",
    ),
    internal("MUI_LANGDLL_REGISTRY_VARIABLES"),
    // Not a setting and not a gap: MUI2's own `instfiles` page inserts it,
    // at `Pages/InstallFiles.nsh:145`, so neither a user nor this compiler
    // has anywhere to write it.
    internal("MUI_LANGDLL_SAVELANGUAGE"),
    internal("MUI_LANGDLL_VARIABLES"),
    exposed(
        "MUI_LANGDLL_WINDOWTITLE",
        "languages { ask = { title = \"…\" } }",
    ),
    exposed(
        "MUI_LANGUAGE",
        "languages { \"English\" }, written after every page (§15.7)",
    ),
    rejected(
        "MUI_LANGUAGEEX",
        "an undocumented second spelling of `MUI_LANGUAGE`; the compiler writes the language lines from `languages {}` (§15.26)",
    ),
    internal("MUI_LICENSEPAGE"),
    exposed(
        "MUI_LICENSEPAGE_BGCOLOR",
        "installer { licenseBkColor = \"…\" }",
    ),
    exposed("MUI_LICENSEPAGE_BUTTON", "page.license { button = \"…\" }"),
    exposed(
        "MUI_LICENSEPAGE_CHECKBOX",
        "page.license { checkbox = \"I accept\" }",
    ),
    exposed(
        "MUI_LICENSEPAGE_CHECKBOX_TEXT",
        "page.license { checkbox = \"I accept\" }",
    ),
    rejected(
        "MUI_LICENSEPAGE_CHECKBOX_TEXT_ACCEPT",
        "a name only ever cleared: the radio texts are `MUI_LICENSEPAGE_RADIOBUTTONS_TEXT_*`, and this pair is a typo in `License.nsh`",
    ),
    rejected(
        "MUI_LICENSEPAGE_CHECKBOX_TEXT_DECLINE",
        "a name only ever cleared: the radio texts are `MUI_LICENSEPAGE_RADIOBUTTONS_TEXT_*`, and this pair is a typo in `License.nsh`",
    ),
    internal("MUI_LICENSEPAGE_INTERFACE"),
    exposed(
        "MUI_LICENSEPAGE_RADIOBUTTONS",
        "page.license { radioButtons = { … } }",
    ),
    exposed(
        "MUI_LICENSEPAGE_RADIOBUTTONS_TEXT_ACCEPT",
        "page.license { radioButtons = { accept = \"…\" } }",
    ),
    exposed(
        "MUI_LICENSEPAGE_RADIOBUTTONS_TEXT_DECLINE",
        "page.license { radioButtons = { decline = \"…\" } }",
    ),
    exposed(
        "MUI_LICENSEPAGE_TEXT_BOTTOM",
        "page.license { bottomText = \"…\" }",
    ),
    exposed(
        "MUI_LICENSEPAGE_TEXT_TOP",
        "page.license { topText = \"…\" }",
    ),
    internal("MUI_LOADANDASPECTSTRETCHIMAGETOCONTROLHEIGHT"),
    internal("MUI_LOADANDXALIGNIMAGE"),
    rejected(
        "MUI_OPTIMIZE_ALWAYSLTR",
        "undocumented, and it drops the right-to-left half of the UI to save a few bytes",
    ),
    internal("MUI_PAGEDECLARATION_COMPONENTS"),
    internal("MUI_PAGEDECLARATION_CONFIRM"),
    internal("MUI_PAGEDECLARATION_DIRECTORY"),
    internal("MUI_PAGEDECLARATION_FINISH"),
    internal("MUI_PAGEDECLARATION_INSTFILES"),
    internal("MUI_PAGEDECLARATION_LICENSE"),
    internal("MUI_PAGEDECLARATION_STARTMENU"),
    internal("MUI_PAGEDECLARATION_WELCOME"),
    exposed("MUI_PAGE_COMPONENTS", "installer { page.components { … } }"),
    // The fourth hook, and the only one that is not on every page: welcome,
    // finish and the start menu insert it and the other five do not.
    exposed(
        "MUI_PAGE_CUSTOMFUNCTION_DESTROYED",
        "page.welcome { destroyed = function() … end }",
    ),
    exposed(
        "MUI_PAGE_CUSTOMFUNCTION_LEAVE",
        "page.X { leave = function() … end }",
    ),
    exposed(
        "MUI_PAGE_CUSTOMFUNCTION_PRE",
        "page.X { pre = function() … end }",
    ),
    exposed(
        "MUI_PAGE_CUSTOMFUNCTION_SHOW",
        "page.X { show = function() … end }",
    ),
    exposed("MUI_PAGE_DIRECTORY", "installer { page.directory { … } }"),
    exposed("MUI_PAGE_FINISH", "installer { page.finish { … } }"),
    internal("MUI_PAGE_FUNCTION_ABORTWARNING"),
    internal("MUI_PAGE_FUNCTION_CUSTOM"),
    internal("MUI_PAGE_FUNCTION_FULLWINDOW"),
    internal("MUI_PAGE_FUNCTION_GUIINIT"),
    exposed(
        "MUI_PAGE_HEADER_SUBTEXT",
        "page.X { headerSubText = \"…\" }",
    ),
    exposed("MUI_PAGE_HEADER_TEXT", "page.X { headerText = \"…\" }"),
    internal("MUI_PAGE_INIT"),
    exposed("MUI_PAGE_INSTFILES", "installer { page.instFiles { … } }"),
    exposed("MUI_PAGE_LICENSE", "installer { page.license { … } }"),
    exposed(
        "MUI_PAGE_STARTMENU",
        "installer { local menu = page.startMenu { … } }",
    ),
    internal("MUI_PAGE_UNINSTALLER"),
    internal("MUI_PAGE_UNINSTALLER_FUNCPREFIX"),
    internal("MUI_PAGE_UNINSTALLER_PREFIX"),
    exposed("MUI_PAGE_WELCOME", "installer { page.welcome { … } }"),
    exposed(
        "MUI_RESERVEFILE_LANGDLL",
        "written after the language lines whenever `languages { ask }` is present, never spelled (§15.26)",
    ),
    internal("MUI_SET"),
    internal("MUI_STARTMENUPAGE"),
    rejected(
        "MUI_STARTMENUPAGE_BGCOLOR",
        "MUI2 3.12 paints `$mui.StartMenuMenu.FolderList` at `StartMenu.nsh:141`, and the variable it declares is `$mui.StartMenuPage.FolderList`: the line is reached only when this is defined, so a page that sets the colours raises `warning 6000` and cannot assemble under `-WX`",
    ),
    internal("MUI_STARTMENUPAGE_CURRENT_ID"),
    exposed(
        "MUI_STARTMENUPAGE_DEFAULTFOLDER",
        "page.startMenu { defaultFolder = \"…\" }",
    ),
    internal("MUI_STARTMENUPAGE_INTERFACE"),
    exposed(
        "MUI_STARTMENUPAGE_NODISABLE",
        "page.startMenu { checkbox = false }",
    ),
    exposed(
        "MUI_STARTMENUPAGE_REGISTRY_KEY",
        "page.startMenu { registry = { key = \"Software\\\\…\", … } }",
    ),
    exposed(
        "MUI_STARTMENUPAGE_REGISTRY_ROOT",
        "page.startMenu { registry = { root = \"HKCU\", … } }",
    ),
    exposed(
        "MUI_STARTMENUPAGE_REGISTRY_VALUENAME",
        "page.startMenu { registry = { value = \"…\", … } }",
    ),
    internal("MUI_STARTMENUPAGE_REGISTRY_VARIABLES"),
    rejected(
        "MUI_STARTMENUPAGE_TEXTCOLOR",
        "MUI2 3.12 paints `$mui.StartMenuMenu.FolderList` at `StartMenu.nsh:141`, and the variable it declares is `$mui.StartMenuPage.FolderList`: the line is reached only when this is defined, so a page that sets the colours raises `warning 6000` and cannot assemble under `-WX`",
    ),
    exposed(
        "MUI_STARTMENUPAGE_TEXT_CHECKBOX",
        "page.startMenu { checkbox = \"…\" }",
    ),
    exposed(
        "MUI_STARTMENUPAGE_TEXT_TOP",
        "page.startMenu { topText = \"…\" }",
    ),
    internal("MUI_STARTMENUPAGE_VARIABLE"),
    exposed(
        "MUI_STARTMENU_GETFOLDER",
        "uninstaller { section(\"…\", function() delete(menu.folder .. \"/…\") end) }",
    ),
    exposed("MUI_STARTMENU_WRITE_BEGIN", "menu.write(function() … end)"),
    exposed("MUI_STARTMENU_WRITE_END", "menu.write(function() … end)"),
    internal("MUI_SYSVERSION"),
    exposed(
        "MUI_TEXTCOLOR",
        "installer { headerColors = { text = \"…\", background = \"…\" } }",
    ),
    // The last `todo` in this census, and the four rows below say why it is a
    // `rejected` instead: naming a dialog resource directly contradicts the
    // settings that chose it. `MUI_UI` is the same answer one level up — it
    // replaces the whole UI, so every `page.*` field, every header image and
    // every colour would be describing a dialog that is no longer there. A user
    // who has built their own `.exe` UI has left this language's model of a
    // page behind, and `raw` is where that belongs (§10).
    rejected(
        "MUI_UI",
        "the dialog resource for the whole UI: replacing it contradicts every `page.*` setting \
         that shapes one, and a UI of your own is a `raw` block",
    ),
    rejected(
        "MUI_UI_COMPONENTSPAGE_NODESC",
        "MUI2 picks the dialog resource from `MUI_HEADERIMAGE` and the description settings; naming one directly contradicts the setting that chose it",
    ),
    rejected(
        "MUI_UI_COMPONENTSPAGE_SMALLDESC",
        "MUI2 picks the dialog resource from `MUI_HEADERIMAGE` and the description settings; naming one directly contradicts the setting that chose it",
    ),
    rejected(
        "MUI_UI_HEADERIMAGE",
        "MUI2 picks the dialog resource from `MUI_HEADERIMAGE` and the description settings; naming one directly contradicts the setting that chose it",
    ),
    rejected(
        "MUI_UI_HEADERIMAGE_RIGHT",
        "MUI2 picks the dialog resource from `MUI_HEADERIMAGE` and the description settings; naming one directly contradicts the setting that chose it",
    ),
    exposed("MUI_UNABORTWARNING", "uninstaller { abortPrompt = true }"),
    exposed(
        "MUI_UNABORTWARNING_CANCEL_DEFAULT",
        "uninstaller { abortPrompt = { default = \"cancel\" } }",
    ),
    exposed(
        "MUI_UNABORTWARNING_TEXT",
        "uninstaller { abortPrompt = { text = \"…\" } }",
    ),
    internal("MUI_UNCONFIRMPAGE"),
    internal("MUI_UNCONFIRMPAGE_INTERFACE"),
    exposed(
        "MUI_UNCONFIRMPAGE_TEXT_LOCATION",
        "page.confirm { locationText = \"…\" }",
    ),
    exposed(
        "MUI_UNCONFIRMPAGE_TEXT_TOP",
        "page.confirm { topText = \"…\" }",
    ),
    exposed(
        "MUI_UNCONFIRMPAGE_VARIABLE",
        "page.confirm { variable = target }",
    ),
    internal("MUI_UNFUNCTION_CONFIRMPAGE"),
    internal("MUI_UNFUNCTION_DESCRIPTION_BEGIN"),
    internal("MUI_UNFUNCTION_DESCRIPTION_END"),
    internal("MUI_UNFUNCTION_GUIINIT"),
    exposed(
        "MUI_UNGETLANGUAGE",
        "written into `un.onInit` whenever `languages { ask }` is present and there is an uninstaller (§15.26)",
    ),
    exposed("MUI_UNICON", "uninstaller { icon = \"app.ico\" }"),
    internal("MUI_UNINSTALLER"),
    internal("MUI_UNIQUEID"),
    exposed(
        "MUI_UNPAGE_COMPONENTS",
        "uninstaller { page.components { … } }",
    ),
    exposed("MUI_UNPAGE_CONFIRM", "uninstaller { page.confirm { … } }"),
    exposed(
        "MUI_UNPAGE_DIRECTORY",
        "uninstaller { page.directory { … } }",
    ),
    exposed("MUI_UNPAGE_FINISH", "uninstaller { page.finish { … } }"),
    internal("MUI_UNPAGE_FUNCTION_ABORTWARNING"),
    internal("MUI_UNPAGE_FUNCTION_GUIINIT"),
    internal("MUI_UNPAGE_INIT"),
    exposed(
        "MUI_UNPAGE_INSTFILES",
        "uninstaller { page.instFiles { … } }",
    ),
    exposed("MUI_UNPAGE_LICENSE", "uninstaller { page.license { … } }"),
    exposed("MUI_UNPAGE_WELCOME", "uninstaller { page.welcome { … } }"),
    internal("MUI_UNSET"),
    internal("MUI_VERBOSE"),
    exposed(
        "MUI_WELCOMEFINISHPAGE_BITMAP",
        "installer { wizardImage = \"wizard.bmp\" }",
    ),
    rejected(
        "MUI_WELCOMEFINISHPAGE_BITMAP_NOSTRETCH",
        "legacy: `Deprecated.nsh` maps it to `…_BITMAP_STRETCH NoStretchNoCropNoAlign`, which is the name to write",
    ),
    exposed(
        "MUI_WELCOMEFINISHPAGE_BITMAP_STRETCH",
        "installer { wizardImage = { file = \"…\", stretch = \"AspectFitHeight\" } }",
    ),
    internal("MUI_WELCOMEFINISHPAGE_GUINIT"),
    internal("MUI_WELCOMEPAGE"),
    internal("MUI_WELCOMEPAGE_GUIINIT"),
    internal("MUI_WELCOMEPAGE_INTERFACE"),
    exposed(
        "MUI_WELCOMEPAGE_TEXT",
        "page.welcome { text = \"This wizard will install Foo.\" }",
    ),
    internal("MUI_WELCOMEPAGE_TEXT_TOP"),
    exposed(
        "MUI_WELCOMEPAGE_TITLE",
        "page.welcome { title = \"Welcome to Foo\" }",
    ),
    exposed(
        "MUI_WELCOMEPAGE_TITLE_3LINES",
        "page.welcome { title = { text = \"…\", lines = 3 } }",
    ),
    internal("MUI_WELCOMEPAGE_TITLE_HEIGHT"),
    internal("MUI_WELCOMEWELCOMEPAGE_GUINIT"),
];
