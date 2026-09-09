---
title: MUI2
description: The eight pages, the settings each one takes, and the block-level settings that govern all of them.
---

<!-- Hand-written. Not generated yet — see "Generating this file" in
     [commands.md](commands.md). Every spelling below is copied from
     `src/mui/rows.rs` and `src/lower/mod.rs`, so the entries are true as of the
     current tables. -->

The MUI2 half of the [command reference](/reference/commands/): the eight pages,
the settings on each, and the block-level settings that govern all of them.

Installua targets MUI2 and nothing else. It writes every `!insertmacro` and
`!define` itself, in the order MUI2 requires — **include order is never yours to
get right**, which is the single largest source of broken NSIS scripts.

**Coverage.** All 255 MUI2 names are accounted for: a setting below, one of the
17 [rejected](#rejected-mui2-names), or one of the 108 that are MUI2's own
internal state. Nothing is pending.

Everything else — attributes, sections, files, the registry, `page.custom` and
its controls — is in [the command reference](/reference/commands/).

---

## The pages

A page is a member of `page`, listed in the block it belongs to, in the order it
should appear.

The eight pages: `welcome`, `license`, `components`, `directory`, `startMenu`,
`instFiles`, `finish`, `confirm` — plus [`custom`](/reference/commands/windows-and-controls/).
`confirm` is uninstaller-only, `startMenu` installer-only; the rest exist in
both halves and are written by which block lists them.

```lua
installer {
	page.welcome { title = "Welcome to Example" },
	page.license { file = "LICENSE.txt" },
	page.components {},
	page.directory {},
	page.instFiles {},
	page.finish { run = INSTDIR .. "/Example.exe" },
}
```

### Every page

| MUI2                                 | Installua                                           |
| ------------------------------------ | --------------------------------------------------- |
| `MUI_PAGE_HEADER_TEXT`               | `headerText`                                        |
| `MUI_PAGE_HEADER_SUBTEXT`            | `headerSubText`                                     |
| `MUI_PAGE_CUSTOMFUNCTION_PRE`        | `pre = function() … end`                            |
| `MUI_PAGE_CUSTOMFUNCTION_SHOW`       | `show = function() … end`                           |
| `MUI_PAGE_CUSTOMFUNCTION_LEAVE`      | `leave = function() … end`                          |
| `MUI_PAGE_CUSTOMFUNCTION_DESTROYED`  | `destroyed = function() … end` (welcome and finish) |
| `SubCaption` / `UninstallSubCaption` | `subCaption`                                        |

### MUI_PAGE_WELCOME

**Usage** `page.welcome { title = …, text = …, pre = …, show = …, leave = …, destroyed = … }`

`title` and `text` also take a table: `title = { text = …, lines = 3 }` is
`MUI_WELCOMEPAGE_TITLE_3LINES`.

```lua
page.welcome {
	title = { text = "Welcome to Example", lines = 3 },
	text = "This wizard will install Example on your computer.",
}
```

### MUI_PAGE_LICENSE

**Usage** `page.license { file = …, topText = …, bottomText = …, button = …, checkbox = …, radioButtons = { … } }`

`file` is `LicenseData`, `bottomText` is `LicenseText`, and `checkbox` is
`LicenseForceSelection` — page fields here, because MUI2 takes the file as the
page macro's own argument and a block-level license would be dropped in silence
by a script that lists no license page.

`file` takes either one path, or a table keyed by locale — which is
`LicenseLangString`, and needs a `languages {}` block declaring exactly those
locales.

```lua
page.license {
	file = "LICENSE.txt",
	bottomText = "Press Page Down to see the rest of the agreement.",
	checkbox = "I accept the terms",
}

page.license { file = { English = "en.txt", German = "de.txt" } }
```

### MUI_PAGE_COMPONENTS

**Usage** `page.components { topText = …, instTypeText = …, listText = …, descriptionTitle = …, descriptionText = … }`

`topText` is `ComponentText`. The hover text for each entry is the section's own
`description` option (`MUI_DESCRIPTION_TEXT`).

```lua
page.components {
	topText = "Choose which features to install.",
	descriptionTitle = "Description",
}
```

### MUI_PAGE_DIRECTORY

**Usage** `page.directory { topText = …, destinationText = …, variable = …, verifyOnLeave = …, colors = { text = …, background = … } }`

`topText` is `DirText` and `variable` is `DirVar` — a global by name, which the
page stores the chosen directory into. `verifyOnLeave` is `DirVerify`, and it is
a page field for the reason `makensis` gives when you write the command outside
a `PageEx`: it is not valid there.

```lua
page.directory {
	topText = "Choose where to install Example.",
	verifyOnLeave = true,
}
```

### MUI_PAGE_STARTMENU

The one page that must be bound to a local: the local **is** the page's id, and
it is what `menu.folder` and `menu.write` name.

**Usage** `local menu = page.startMenu { defaultFolder = …, topText = …, checkbox = …, registry = { root = …, key = …, value = … } }`
· `menu.folder` → `string` · `menu.write(function() … end)`

```lua
local menu = page.startMenu {
	defaultFolder = "Example",
	registry = { root = "HKCU", key = "Software/Example", value = "StartMenu" },
}

installer {
	menu,
	page.instFiles {},
	section("Shortcuts", function()
		menu.write(function()
			createDirectory(SMPROGRAMS .. "/" .. menu.folder)
			createShortcut(SMPROGRAMS .. "/" .. menu.folder .. "/Example.lnk",
				INSTDIR .. "/Example.exe")
		end)
	end),
}
```

The page is installer-only, but `menu.folder` is **not**. MUI2 defines no
`MUI_UNPAGE_STARTMENU`, so the uninstaller never ran a page and has nothing to
read a variable from — it reads the folder back out of the registry instead.
One spelling, two lowerings, and the block decides which:

```lua
uninstaller {
	page.instFiles {},
	section("Shortcuts", function()
		delete(SMPROGRAMS .. "/" .. menu.folder .. "/Example.lnk")
		rmDir(SMPROGRAMS .. "/" .. menu.folder)
	end),
}
```

Two consequences. `registry` is what makes that read return the folder the user
actually picked: without it `MUI_STARTMENU_GETFOLDER` falls back to
`defaultFolder`, so an uninstaller for a program installed anywhere else removes
a directory nobody created and leaves the real one behind. It is the field that
makes the two halves agree, not decoration. And `menu.write` is the installer's
alone — the uninstaller had no page and so never chose a folder to write back.

A `menu.folder` read inside a `func` is refused rather than guessed: a `func`
can be called from either half, the two lowerings are different code, and
picking wrong is silent because an empty variable copies without complaint.

### MUI_PAGE_INSTFILES

**Usage** `page.instFiles { finishHeaderText = …, finishHeaderSubText = …, abortHeaderText = …, abortHeaderSubText = … }`

The two pairs are the page's second state: what the header says once the install
has finished, and what it says once it has been aborted.

```lua
page.instFiles {
	finishHeaderText = "Installation complete",
	abortHeaderText = "Installation aborted",
}
```

### MUI_PAGE_FINISH

**Usage** `page.finish { title = …, text = …, button = …, cancelEnabled = …, run = …, readme = …, link = { … }, reboot = … }`

`run` and `readme` take a path, or a table: `{ path = …, text = …, parameters = …, checked = false }`,
or `{ call = function() … end }` to run something of your own instead.

```lua
page.finish {
	run = { path = INSTDIR .. "/Example.exe", text = "Run Example now" },
	readme = { path = INSTDIR .. "/README.txt", checked = false },
	link = { text = "Visit our website", url = "https://example.com" },
}
```

### MUI_UNPAGE_CONFIRM

Uninstaller-only.

**Usage** `page.confirm { topText = …, locationText = …, variable = … }`

```lua
uninstaller {
	page.confirm { topText = "Example will be removed from this computer." },
	page.instFiles {},
}
```

### Block-level MUI settings

Written on `installer {}` or `uninstaller {}`, because their scope is the whole
half rather than one page.

Four of them are NSIS commands rather than MUI2 inventions — `checkBitmap` is
`CheckBitmap`, `installColors` is `InstallColors`, `progressBar` is
`InstProgressFlags`, `licenseBkColor` is `LicenseBkColor` — and they look
page-scoped and are not: MUI2 writes each one inside an `!ifndef` that runs on
the first page of its kind and never again, so putting them on a page would be a
lie a second page tells silently.

| MUI2                                     | Installua                                                                                                   |
| ---------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| `MUI_ICON` / `MUI_UNICON`                | `icon = "app.ico"`                                                                                          |
| `MUI_HEADERIMAGE` and its five relatives | `headerImage = "header.bmp"`, or `{ file = …, stretch = …, rtl = …, right = true, transparentText = true }` |
| `MUI_WELCOMEFINISHPAGE_BITMAP`           | `wizardImage = "wizard.bmp"`, or `{ file = …, stretch = … }`                                                |
| `MUI_BGCOLOR` / `MUI_TEXTCOLOR`          | `headerColors = { text = …, background = … }`                                                               |
| `MUI_ABORTWARNING`                       | `abortPrompt = true`, or `{ text = …, default = "cancel" }`                                                 |
| `MUI_FINISHPAGE_NOAUTOCLOSE`             | `autoClose = false`                                                                                         |
| `MUI_COMPONENTSPAGE_SMALLDESC`           | `smallDescriptions = true`                                                                                  |
| `MUI_COMPONENTSPAGE_CHECKBITMAP`         | `checkBitmap = "check.bmp"`                                                                                 |
| `MUI_INSTFILESPAGE_COLORS`               | `installColors = "…"`                                                                                       |
| `MUI_INSTFILESPAGE_PROGRESSBAR`          | `progressBar = "smooth"`                                                                                    |
| `MUI_LICENSEPAGE_BGCOLOR`                | `licenseBkColor = "…"`                                                                                      |
| `MUI_CUSTOMFUNCTION_GUIINIT`             | `onGUIInit(function() … end)`                                                                               |
| `MUI_CUSTOMFUNCTION_ABORT`               | `onUserAbort(function() … end)`                                                                             |
| `MUI_CUSTOMFUNCTION_ONMOUSEOVERSECTION`  | `onMouseOverSection(function() … end)`                                                                      |

```lua
installer {
	icon = "assets/app.ico",
	headerImage = { file = "assets/header.bmp", stretch = "AspectFitHeight" },
	abortPrompt = { text = "Really cancel the installation?" },
	onGUIInit(function() detailPrint("gui up") end),
}
```

---

### Rejected MUI2 names

`MUI_UI` · `MUI_UI_HEADERIMAGE` · `MUI_UI_HEADERIMAGE_RIGHT` ·
`MUI_UI_COMPONENTSPAGE_NODESC` · `MUI_UI_COMPONENTSPAGE_SMALLDESC` —
they replace the dialog resources MUI2 picks from the settings you already set.

`MUI_STARTMENUPAGE_BGCOLOR` · `MUI_STARTMENUPAGE_TEXTCOLOR` — a typo in MUI2
3.12 makes them raise `warning 6000`, which cannot assemble under `-WX`.

`MUI_FINISHPAGE_ABORTWARNINGCHECK` · `MUI_INSTFILESPAGE_ABORTWARNING_TEXT` ·
`MUI_INSTFILESPAGE_ABORTWARNING_SUBTEXT` · `MUI_LANGUAGEEX` ·
`MUI_LICENSEPAGE_CHECKBOX_TEXT_ACCEPT` · `MUI_LICENSEPAGE_CHECKBOX_TEXT_DECLINE` —
MUI 1 spellings that nothing in MUI2 reads.

`MUI_FORCECLASSICCONTROLS` · `MUI_OPTIMIZE_ALWAYSLTR` ·
`MUI_WELCOMEFINISHPAGE_BITMAP_NOSTRETCH` ·
`MUI_DISABLE_INSERT_LANGUAGE_AFTER_PAGES_WARNING` — deprecated by MUI2, or a
warning this compiler cannot produce in the first place.

The other 108 MUI2 names are MUI2's own internal state — variables, page
declarations and function names it defines for itself — and were never settings.
