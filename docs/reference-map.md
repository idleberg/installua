<!-- Hand-written. Not generated yet — see §"Generating this file".
     Every signature, spelling and example below is copied from
     `src/table/overlay.rs`, `src/mui/rows.rs` and `src/lower/mod.rs`, so the
     entries are true as of the current tables. -->

# Installua Reference

The surface reference. Where `installua coverage` is a census — bucket counts
over every NSIS command, answering _is it done_ — this one is a reference:
things grouped by what a person is trying to do, each with a description, a
signature and something you can paste.

**Reading an entry.** The heading is the NSIS or MUI2 name, because that is what
you already know and what search engines have indexed. Everything under it is
Installua. Square brackets are optional positions, `…` a repeated one, `{ … }`
the trailing options table. `→` names what comes back; `→ nothing` means the
call is a statement.

**Coverage.** Every one of the 276 commands `makensis -CMDHELP` prints is
accounted for: either in a group below, or in [Not available](#not-available).
The 255 MUI2 names are in [mui-reference.md](mui-reference.md). Nothing is
pending — the `todo` bucket of both censuses is empty.

---

## The groups

| Group                                                             | Covers                                                                        |
| ----------------------------------------------------------------- | ----------------------------------------------------------------------------- |
| [Program structure](#program-structure)                           | the blocks a file is made of, and the declarations inside them                |
| [Script attributes](#script-attributes)                           | the `attributes {}` block: name, output, compression, manifest, version info  |
| [Sections and install types](#sections-and-install-types)         | what the components page offers and what each part costs                      |
| [Pages and MUI](mui-reference.md)                                 | `page.welcome`, `page.license`, … and everything drawn on them — its own file |
| [Windows and controls](#windows-and-controls)                     | `page.custom`: controls, colours, fonts, events                               |
| [Languages and locales](#languages-and-locales)                   | `languages { locales = { … } }` and the language dialog                       |
| [Files and directories](#files-and-directories)                   | packing files in, and what happens to them on the target disk                 |
| [Files on the target at runtime](#files-on-the-target-at-runtime) | `fileOpen` and the handle methods                                             |
| [Registry and INI](#registry-and-ini)                             | reading and writing `HKLM`, `HKCU`, and `.ini` files                          |
| [Processes and the shell](#processes-and-the-shell)               | `exec`, shortcuts, DLLs, reboot                                               |
| [Strings and numbers](#strings-and-numbers)                       | `string.*`, arithmetic, comparison                                            |
| [Flow, errors and messages](#flow-errors-and-messages)            | aborting, testing, telling the user                                           |
| [Windows facts](#windows-facts)                                   | what the machine is: version, shell folders, registry view                    |
| [Plugins and headers](#plugins-and-headers)                       | `plugin`, `import`, `raw`, declaring a third-party one                        |
| [Constants](#constants)                                           | `INSTDIR`, `PROGRAMFILES64`, `HKLM`, …                                        |
| [Not available](#not-available)                                   | what has no Installua spelling, and what to write instead                     |

---

## Program structure

A file is a Lua program, read top to bottom, with no preprocessor. Declaration
order does not matter: every top-level name is resolved before any body
is lowered, so a `func` may call one declared below it.

The order of the fields _inside_ `attributes {}` does not matter either, and for
a different reason: NSIS has a handful of commands that refuse to run — or, in
one case, crash — once something ahead of them has changed the header, so the
compiler emits the block in an order it chose rather than the one you typed.
Which fields those are was measured against `makensis` rather than reasoned
about; the list is `ORDERED` in `src/lower/mod.rs`.

### Function / FunctionEnd

Declares a function. Parameters and returns are ordinary Lua; the compiler
writes the stack traffic NSIS needs.

**Usage** `func(name, body)` → nothing

```lua
func("majorOf", function(version)
	local dot = string.find(version, ".")
	return string.sub(version, 1, dot - 1)
end)
```

### .onInit / un.onInit

The callback NSIS runs before anything is shown. Written _inside_ the block it
belongs to, so the same spelling covers both halves and the `un.` prefix is the
compiler's business.

**Usage** `onInit(body)`, listed in `installer {}` or `uninstaller {}`

```lua
installer {
	page.instFiles {},
	onInit(function()
		if fileExists(INSTDIR .. "/app.exe") then
			detailPrint("upgrading")
		end
	end),
}
```

### Installer and uninstaller blocks

`installer {}` holds the pages, sections and callbacks of the installer;
`uninstaller {}` the same for the uninstaller, and writing one is what makes
`writeUninstaller` legal. Both also carry the MUI settings whose scope is the
whole half — see [Pages and MUI](mui-reference.md).

**Usage** `installer { … }`, `uninstaller { … }`

```lua
installer {
	installDir = PROGRAMFILES64 .. "/Example",
	page.directory {},
	page.instFiles {},
	section("Core", function()
		setOutPath(INSTDIR)
		file("assets/Example.exe")
		writeUninstaller(INSTDIR .. "/uninstall.exe")
	end),
}

uninstaller {
	page.confirm {},
	page.instFiles {},
	section("Remove", function()
		rmDir(INSTDIR, { recursive = true })
	end),
}
```

### Var

Globals are declared by assigning to one at the top level. A `local`
inside a body is a register the compiler allocates and reuses.

**Usage** assignment at the top level

```lua
gitDescribe = ""
```

### !include

Source layout, not a module system: the named file's declarations are merged
into this one and nothing is emitted. A path is relative to the file
that names it.

**Usage** `include(path)` → nothing

```lua
include("lib/shortcuts.lua")
```

### glob

Walks the **build** machine at compile time and unrolls the result, so the file
list is fixed before anything ships. This is what replaces
`FindFirst`/`FindNext`/`FindClose`, which walk the target's disk instead.

**Usage** `for path in glob(pattern) do … end`

```lua
for path in glob("assets/docs/*.pdf") do
	file(path)
end
```

### param

A build-time constant the invocation may set. This is what replaces
`!ifndef VERSION` / `!define VERSION "1.4.2"` / `!endif`, and `-D` is the same
flag `makensis` spells the same way.

**Usage** `local NAME <const> = param(name, default)` → the value

```lua
local VERSION <const> = param("VERSION", "1.4.2")
local PORT    <const> = param("PORT", 8080)
local SIGNED  <const> = param("SIGNED", false)
```

```console
$ installua build install.lua -D VERSION=2.0.0 -D SIGNED=true
$ installua build install.lua --param VERSION=2.0.0 --param SIGNED=true
```

`-D` is the spelling `makensis -D` teaches; `--param` is the same flag written
out. It is not `--define` or `--declare` because the source is what *declares* a
parameter and the invocation is what *sets* one — a flag named for the declaring
half would read as doing the thing the `unknown-param` error says it does not.

Three things follow from the declaration being written in the source rather
than passed only on the command line:

- **A `-D` naming a parameter the program does not declare is an error.** That
  is the whole point. NSIS's version fails the other way — a misspelt
  `-DVERSOIN` defines a second thing nobody reads, the build succeeds, the
  default ships, and nothing says so.
- **The default is the type.** `param("PORT", 8080)` makes `PORT` an integer, so
  `-D PORT=abc` is rejected instead of arriving at an `IntOp` as a string.
  `param("SIGNED", false)` takes `true` and `false` and nothing else.
- **The name and the binding are separate.** The string is what `-D` sets; the
  `local` is what the program reads. They are usually spelled the same and do
  not have to be.

`param(…)` is a **declaration**, so it stands alone as the whole initialiser of
a top-level `local … <const>` — not inside a body, and not composed into a
larger expression. Compose one line further down instead, which costs nothing
since resolution is order-free:

```lua
local VERSION <const> = param("VERSION", "1.4.2")
local FULL    <const> = VERSION .. "-win64"   -- an ordinary <const>
```

`installua check` takes the same `-D`s, because a program whose parameters are
overridden is a different program to check.

---

## Script attributes

Everything in this group is a field of the top-level `attributes {}` block, not
a call. It is set once, read by the compiler, and never executed. `outFile` is
the only one with no default.

```lua
attributes {
	name = "Example",
	outFile = "Example-setup.exe",
	unicode = true,
	compressor = "lzma",
	requestExecutionLevel = "admin",
}
```

### Identity and output

| NSIS                  | Installua                                            | Holds               |
| --------------------- | ---------------------------------------------------- | ------------------- |
| `Name`                | `name`                                               | string              |
| `OutFile`             | `outFile`                                            | path — **required** |
| `Caption`             | `caption`                                            | string              |
| `UninstallCaption`    | `uninstallCaption`                                   | string              |
| `Icon`                | `icon`                                               | path                |
| `UninstallIcon`       | `icon`, inside `uninstaller {}`                      | path                |
| `WindowIcon`          | `windowIcon`                                         | boolean             |
| `BrandingText`        | `brandingText`                                       | string              |
| `InstallDir`          | `installDir`                                         | path                |
| `InstallDirRegKey`    | `installDirRegKey = { root = …, key = …, name = … }` | table               |
| `AllowRootDirInstall` | `allowRootDirInstall`                                | boolean             |

### Build and compression

| NSIS                    | Installua            | Holds                                                      |
| ----------------------- | -------------------- | ---------------------------------------------------------- |
| `Unicode`               | `unicode`            | boolean — defaults `true`, always emitted first            |
| `CPU`                   | `cpu`                | `"x86"` \| `"amd64"`                                       |
| `SetCompressor`         | `compressor`         | `"zlib"` \| `"bzip2"` \| `"lzma"`                          |
| `SetCompressionLevel`   | `compressionLevel`   | int — read only when `compressor` is `"zlib"` or `"bzip2"` |
| `SetCompressorDictSize` | `compressorDictSize` | int (MB) — read only when `compressor` is `"lzma"`         |
| `SetCompress`           | `compress`           | `"off"` \| `"auto"` \| `"force"`                           |
| `SetDatablockOptimize`  | `datablockOptimize`  | boolean                                                    |
| `SetDateSave`           | `dateSave`           | boolean                                                    |
| `CRCCheck`              | `crcCheck`           | boolean                                                    |
| `FileBufSize`           | `fileBufSize`        | int                                                        |
| `SetOverwrite`          | `overwrite`          | `"on"` \| `"off"` \| `"try"` \| `"ifnewer"` \| `"ifdiff"`  |
| `AllowSkipFiles`        | `allowSkipFiles`     | boolean                                                    |

```lua
attributes {
	compressor = "lzma",
	compressorDictSize = 32,
}
```

The two compression settings are the one place an `attributes {}` field depends
on another. NSIS reads a dictionary size only under LZMA and a level only under
the other two, and it accepts the wrong pairing and silently ignores it — so
this compiler refuses it instead, `error[ignored-setting]`. An absent
`compressor` counts as `"zlib"`, which is NSIS's own default, so
`compressorDictSize` written on its own is refused too.

### The manifest

The `Manifest*` commands are one table rather than eight prefixed fields: NSIS
puts the word on every command, and this language puts it on the table once.

**Usage** `manifest = { … }`

| NSIS                             | Installua                                                          |
| -------------------------------- | ------------------------------------------------------------------ |
| `RequestExecutionLevel`          | `requestExecutionLevel = "none" \| "user" \| "highest" \| "admin"` |
| `ManifestSupportedOS`            | `manifest.supportedOS = { … }`                                     |
| `ManifestMaxVersionTested`       | `manifest.maxVersionTested`                                        |
| `ManifestDPIAware`               | `manifest.dpiAware`                                                |
| `ManifestDPIAwareness`           | `manifest.dpiAwareness`                                            |
| `ManifestLongPathAware`          | `manifest.longPathAware`                                           |
| `ManifestGdiScaling`             | `manifest.gdiScaling`                                              |
| `ManifestDisableWindowFiltering` | `manifest.disableWindowFiltering`                                  |
| `ManifestAppendCustomString`     | `manifest.customStrings = { { path = …, string = … }, … }`         |

`requestExecutionLevel` lands in the manifest too and is written flat, because
its NSIS name says nothing about a manifest: the groups are the prefixes NSIS
itself uses, so the map from one to the other stays mechanical.

```lua
attributes {
	requestExecutionLevel = "admin",
	manifest = {
		supportedOS = { "Win7", "Win10" },
		dpiAwareness = "PerMonitorV2,system",
		longPathAware = true,
	},
}
```

### Version info

`VIProductVersion` is the four-part version Explorer shows on the Properties
tab; `keys` are the free-form pairs beside it.

**Usage** `versionInfo = { product = <string>, file = <string>, keys = { … } }`

```lua
attributes {
	versionInfo = {
		product = "1.4.2.0",
		keys = {
			ProductName = "Example",
			CompanyName = "Example Ltd",
			FileVersion = "1.4.2",
		},
	},
}
```

### The PE image

One table, for the reason [the manifest](#the-manifest) is one — and the group
is spelled out, because `pe` is an abbreviation only someone who already works
on Windows executables reads at a glance.

**Usage** `portableExecutable = { … }`

| NSIS                   | Installua                                                                                     |
| ---------------------- | --------------------------------------------------------------------------------------------- |
| `PEAddResource`        | `portableExecutable.addResource = { { file = …, restype = …, resname = …, reslang = … }, … }` |
| `PERemoveResource`     | `portableExecutable.removeResource = { { restype = …, resname = …, reslang = … }, … }`        |
| `PEDllCharacteristics` | `portableExecutable.dllCharacteristics = { add = …, remove = … }`                             |
| `PESubsysVer`          | `portableExecutable.subsystemVersion`                                                         |

```lua
attributes {
	portableExecutable = {
		addResource = { { file = "assets/app.ico", restype = "#100", resname = "#1" } },
		subsystemVersion = "5.1",
	},
}
```

### Classic UI text and colours

These configure the window MUI2 draws into. The per-page text lives on the page
instead — see [Pages and MUI](mui-reference.md).

| NSIS                  | Installua                                                    |
| --------------------- | ------------------------------------------------------------ |
| `AddBrandingImage`    | `brandingImage = { edge = …, size = …, padding = … }`        |
| `AutoCloseWindow`     | `autoCloseWindow`                                            |
| `BGFont`              | `bgFont = { face = …, height = …, weight = … }`              |
| `BGGradient`          | `bgGradient = false` or `{ top = …, bottom = …, text = … }`  |
| `SetFont`             | `font = { face = …, size = … }`                              |
| `MiscButtonText`      | `buttonText = { back = …, next = …, cancel = …, close = … }` |
| `InstallButtonText`   | `installButtonText`                                          |
| `UninstallButtonText` | `uninstallButtonText`                                        |
| `DetailsButtonText`   | `detailsButtonText`                                          |
| `CompletedText`       | `completedText`                                              |
| `FileErrorText`       | `fileErrorText = { text = …, withoutIgnore = … }`            |
| `SpaceTexts`          | `spaceTexts = false` or `{ required = …, available = … }`    |
| `ShowInstDetails`     | `showInstDetails = "hide" \| "show" \| "nevershow"`          |
| `ShowUninstDetails`   | `showUninstDetails`                                          |
| `SilentInstall`       | `silentInstall = "normal" \| "silent" \| "silentlog"`        |
| `SilentUnInstall`     | `silentUninstall = "normal" \| "silent"`                     |

---

## Sections and install types

A section is a unit of work the user can turn off. Sections are written inside
`installer {}` or `uninstaller {}`; the components page lists them in the order
they appear.

### Section / SectionEnd

Declares a section: a name and a body. The table form takes options beside the
name — `optional` starts it unticked, `required` makes it untickable,
`installTypes` puts it in the presets, `description` is its hover text.

**Usage** `section(name, body)` or
`section { <name>, body = …, optional = …, required = …, installTypes = { … }, size = …, description = … }`

```lua
section("Core", function()
	setOutPath(INSTDIR)
	file("assets/Example.exe")
	writeUninstaller(INSTDIR .. "/uninstall.exe")
end)

section { "Start menu shortcut",
	optional = true,
	description = "Adds Example to the Start menu.",
	body = function()
		createDirectory(SMPROGRAMS .. "/Example")
		createShortcut(SMPROGRAMS .. "/Example/Example.lnk", INSTDIR .. "/Example.exe")
	end,
}
```

### SectionGroup / SectionGroupEnd

Groups sections under one collapsible heading on the components page. Ticking
the heading ticks the children. Takes `description` like a section does.

**Usage** `group(name, { <section>, … })`

```lua
group("Extras", {
	section("Documentation", function()
		file("assets/manual.pdf")
	end),
	section { "Samples", optional = true, body = function()
		file("assets/samples/*.example")
	end },
})
```

### InstType

The presets the components page offers, listed by name on the block. A section
joins one through `installTypes`; there are no indices here.

**Usage** an `installer`'s or `uninstaller`'s `installTypes = { <name>, … }`

```lua
installer {
	installTypes = { "Typical", "Full" },
	page.components {},
	page.instFiles {},
}
```

### SectionIn

Which install types a section belongs to, by name, plus `required` for the
untickable one.

**Usage** a `section`'s `installTypes = { <name>, … }` and `required = <boolean>`

```lua
section { "Documentation",
	installTypes = { "Full" },
	body = function() file("assets/manual.pdf") end,
}
```

### AddSize

Adds kilobytes to a section's reported size, for things the compiler cannot
weigh — a download, a generated file, a database built on first run.

**Usage** a `section`'s `size = <int>` (kilobytes)

```lua
section { "Offline map data",
	size = 240000,
	body = function() exec(INSTDIR .. "/fetch-maps.exe") end,
}
```

### SectionGetText / SectionSetText / SectionGetFlags / SectionSetFlags / SectionGetSize / SectionSetSize / SectionGetInstTypes / SectionSetInstTypes

A running program addresses a section through the handle `section(…)` returns,
not through an index. `group(…)` returns the same kind of handle.

| NSIS                                  | Installua                                                                                                                                              |
| ------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `SectionGetText` / `SectionSetText`   | `handle.text` — read and write                                                                                                                         |
| `SectionGetFlags` / `SectionSetFlags` | `handle.selected` — read and write, a boolean                                                                                                          |
| `SectionGetSize` / `SectionSetSize`   | `handle.size` — read and write                                                                                                                         |
| `SectionGetInstTypes`                 | `handle.installTypes(name)` → `boolean` — a membership question, because NSIS stores it as a bit field and this language has no list to decode it into |
| `SectionSetInstTypes`                 | `handle.installTypes = { … }`                                                                                                                          |

```lua
local core = section("Core", function()
	file("assets/Example.exe")
end)

installer {
	core,
	page.components {},
	page.instFiles {},
	onInit(function()
		core.text = "Core files"
		core.selected = true
	end),
}
```

### GetCurInstType / SetCurInstType / InstTypeGetText / InstTypeSetText

The install type the user has chosen, by name.

**Usage** `currentInstType` → `string`, `currentInstType = <name>`,
`instTypes.getText(name)` → `string`, `instTypes.setText(name, text)`

```lua
onInit(function()
	currentInstType = "Full"
	instTypes.setText("Full", "Everything")
end)
```

### WriteUninstaller

Writes the uninstaller executable, built from the `uninstaller {}` block. Call
it from a section — usually the first — after `setOutPath`.

**Usage** `writeUninstaller(path)` → nothing

```lua
writeUninstaller(INSTDIR .. "/uninstall.exe")
```

---

## Windows and controls

`page.custom` is the eighth page and the only one that is not a MUI2 macro: it
is a `Page custom`, and both functions behind it are the compiler's to write
. A control is a call listed in the page's `controls`; the `local` it is
bound to is how a running program addresses it, and decides nothing about where
it sits.

**Usage** `page.custom { <headerText>, controls = { … }, pre = …, show = …, leave = … }`

### The control types

`label` · `text` · `password` · `number` · `button` · `checkbox` ·
`radioButton` · `groupBox` · `hLine` · `dropList` · `listBox` · `fileRequest` ·
`dirRequest` · `bitmap` · `link`

**Usage** `<type> { <text>, x = …, y = …, width = …, height = …, items = { … }, image = …, url = …, onClick = …, onChange = … }`
→ a control handle

`y` and `height` are required — there is no auto-flow. `x` defaults to 0 and
`width` to the dialog's.

```lua
local agree = checkbox { "I have read the notes", y = 60, height = 12 }
local serial = text { x = 60, y = 90, width = 120, height = 14 }

installer {
	page.custom { "Details",
		controls = { agree, serial },
		leave = function()
			if not agree.checked then abort("please read the notes") end
		end,
	},
	page.instFiles {},
}
```

### A control's fields

| NSIS                 | Installua                                                 | Direction      |
| -------------------- | --------------------------------------------------------- | -------------- |
| `SendMessage`        | `handle.value` — its text                                 | read and write |
| `SendMessage`        | `handle.checked` — a `checkbox`'s or `radioButton`'s tick | read and write |
| `EnableWindow`       | `handle.enabled`                                          | write only     |
| `ShowWindow`         | `handle.visible`                                          | write only     |
| `SetCtlColors`       | `handle.colors = { text = …, background = … }`            | write only     |
| `CreateFont`         | `handle.font = { face = …, size = …, bold = … }`          | write only     |
| `LoadAndSetImage`    | `handle.image = "check.bmp"`                              | write only     |
| `GetFunctionAddress` | `onClick` / `onChange` on the declaration                 | —              |

Five of the seven are write-only because Windows offers no instruction that
reports them: NSIS can set a control's colours and cannot ask what they are.

```lua
serial.font = { face = "Tahoma", size = 8 }
serial.colors = { text = "800000", background = "transparent" }
agree.enabled = false
```

### GetDlgItem / FindWindow / IsWindow / SendMessage

Reaching a window Installua did not draw — MUI2's own Cancel button, or another
process's.

| NSIS         | Installua                                                                |
| ------------ | ------------------------------------------------------------------------ |
| `GetDlgItem` | `getDlgItem(dialog, itemId)` → `handle`                                  |
| `FindWindow` | `findWindow(class[, title[, parent[, childAfter[, { … }]]]])` → `handle` |
| `IsWindow`   | `isWindow(hwnd)` → `boolean`                                             |

```lua
local cancel = getDlgItem(HWNDPARENT, 2)
cancel.enabled = false
```

### HideWindow / BringToFront / LockWindow / SetBrandingImage / SetDetailsView

| NSIS               | Installua                          |
| ------------------ | ---------------------------------- |
| `HideWindow`       | `hideWindow()`                     |
| `BringToFront`     | `bringToFront()`                   |
| `LockWindow`       | `lockWindow("on" \| "off")`        |
| `SetBrandingImage` | `setBrandingImage(path[, { … }])`  |
| `SetDetailsView`   | `setDetailsView("hide" \| "show")` |

---

## Languages and locales

One block declares every language the installer carries and every string in
them. It is written **locale-first**, because a translator owns a locale; the
compiler transposes it into one `LangString` per name. Every name has
to appear in every locale — NSIS expands a missing one to nothing, which is a
blank label on one machine in one country.

**Usage** `languages { locales = { <Locale> = { <name> = <string>, … }, … }, ask = { … } }`
· a read is `lang.<name>`

```lua
languages {
	locales = {
		English = { greeting = "Welcome", bye = "Done" },
		German  = { greeting = "Willkommen", bye = "Fertig" },
	},
	ask = {
		title = "Choose a language",
		remember = { root = "HKCU", key = "Software/Example", value = "Language" },
	},
}

installer {
	page.instFiles {},
	section("Core", function()
		detailPrint(lang.greeting)
	end),
}
```

### The language dialog

`ask` is `MUI_LANGDLL_DISPLAY` and its settings. Present it and the compiler
writes the dialog into `.onInit`, the `ReserveFile` above it, and
`MUI_UNGETLANGUAGE` into `un.onInit` — none of which is spellable, and none of
which you can put in the wrong place.

| MUI2                       | Installua                                               |
| -------------------------- | ------------------------------------------------------- |
| `MUI_LANGDLL_WINDOWTITLE`  | `ask = { title = … }`                                   |
| `MUI_LANGDLL_INFO`         | `ask = { info = … }`                                    |
| `MUI_LANGDLL_ALLLANGUAGES` | `ask = { allLanguages = true }`                         |
| `MUI_LANGDLL_ALWAYSSHOW`   | `ask = { alwaysShow = true }`                           |
| `MUI_LANGDLL_REGISTRY_*`   | `ask = { remember = { root = …, key = …, value = … } }` |

### IfRtlLanguage

True when the running language is written right to left.

**Usage** `rtlLanguage()` → `boolean`

---

## Files and directories

The two halves of this group do different things at different times. `file`
packs a file from the **build** machine into the installer; everything else
moves, copies or deletes files on the **target** machine while the installer
runs. Paths take forward slashes and are normalised on the way out.

### File

Packs one or more files into the installer and unpacks them into the current
output directory. A missing file is a build error unless `nonFatal` says
otherwise.

**Usage** `file(filespec, …[, { … }])` → nothing
**Options** `recursive`, `exclude = { … }`, `keepAttributes`, `nonFatal`

```lua
setOutPath(INSTDIR)
file("assets/icon.ico", { exclude = { "*.tmp", "*.log" } })
```

### ReserveFile

Puts a file at the head of the data block, so it can be extracted before the
rest. Reserving a **plugin** is the compiler's job and has no spelling: it works
out which plugins `.onInit` can reach and writes `ReserveFile /plugin` itself.

**Usage** `reserveFile(filespec, …[, { … }])` → nothing
**Options** `nonFatal`, `recursive`, `exclude = { … }`

### SetOutPath

Sets the directory that following `file` calls unpack into, creating it if it
does not exist, and the working directory for shortcuts made afterwards.

**Usage** `setOutPath(path)` → nothing

```lua
setOutPath(INSTDIR)
```

### InitPluginsDir

No spelling, and nothing to remember: any body that names `PLUGINSDIR` gets the
line at the top of it, because `$PLUGINSDIR` expands to nothing until something
creates it and NSIS never says so.

```lua
setOutPath(PLUGINSDIR)   -- InitPluginsDir is written above this
file("assets/splash.bmp")
```

### CreateDirectory

Creates a directory and every missing parent. Unlike `setOutPath` it does not
change where later `file` calls land.

**Usage** `createDirectory(directoryName)` → nothing

```lua
createDirectory(INSTDIR .. "/logs")
```

### Delete

Deletes a file, or every file matching a wildcard. Deleting something that is
not there is not an error — check with `fileExists` if you care.

**Usage** `delete(filespec[, { … }])` → nothing
**Options** `rebootOk`

```lua
delete(INSTDIR .. "/old.txt", { rebootOk = true })
```

### RMDir

Removes a directory. It must be empty unless `recursive` is set, and `recursive`
on a directory the user chose is how an uninstaller eats a disk.

**Usage** `rmDir(directoryName[, { … }])` → nothing
**Options** `recursive`, `rebootOk`

```lua
rmDir(INSTDIR, { recursive = true, rebootOk = true })
```

### Rename

Moves a file, across directories as well as within one. With `rebootOk`, a
rename blocked by a lock is queued for the next restart.

**Usage** `rename(sourceFile, destinationFile[, { … }])` → nothing
**Options** `rebootOk`

```lua
rename(INSTDIR .. "/old.txt", INSTDIR .. "/new.txt")
```

### CopyFiles

Copies files already on the target disk, with the shell's copy progress dialog.
For files coming out of the installer, use `file`.

**Usage** `copyFiles(sourcePath, destinationPath[, sizeInKb[, { … }]])` → nothing
**Options** `silent`, `filesOnly`

```lua
copyFiles(INSTDIR .. "/data", INSTDIR .. "/backup")
```

### SetFileAttributes

**Usage** `setFileAttributes(file, attribute)` → nothing

```lua
setFileAttributes(INSTDIR .. "/config.ini", "READONLY")
```

### GetFullPathName / GetTempFileName / SearchPath / GetFileTime / GetFileTimeLocal

| NSIS               | Installua                                                          |
| ------------------ | ------------------------------------------------------------------ |
| `GetFullPathName`  | `getFullPathName(pathOrFile[, { … }])` → `string`                  |
| `GetTempFileName`  | `getTempFileName([baseDir])` → `string`                            |
| `SearchPath`       | `searchPath(filename)` → `string`                                  |
| `GetFileTime`      | `getFileTime(file)` → `int, int` (high, low)                       |
| `GetFileTimeLocal` | `getFileTimeLocal(localFile)` → `int, int` — the **build** machine |

```lua
local temp = getTempFileName()
detailPrint(temp)
```

---

## Files on the target at runtime

`fileOpen` returns a handle with methods on it; NSIS's `FileWrite`-style
instructions are all reached that way.

**Usage** `fileOpen(path, mode)` → `handle`, where mode is `"r"`, `"w"` or `"a"`

| NSIS               | Installua                                 |
| ------------------ | ----------------------------------------- |
| `FileRead`         | `handle:read([maxlen])` → `string`        |
| `FileWrite`        | `handle:write(text)`                      |
| `FileReadByte`     | `handle:readByte()` → `int`               |
| `FileWriteByte`    | `handle:writeByte(value)`                 |
| `FileReadWord`     | `handle:readWord()` → `int`               |
| `FileWriteWord`    | `handle:writeWord(value)`                 |
| `FileReadUTF16LE`  | `handle:readUtf16Le([maxlen])` → `string` |
| `FileWriteUTF16LE` | `handle:writeUtf16Le(text[, { … }])`      |
| `FileSeek`         | `handle:seek(offset[, mode])` → `int`     |
| `FileClose`        | `handle:close()`                          |

```lua
local f = fileOpen(INSTDIR .. "/install.log", "w")
f:write("installed\r\n")
f:close()
```

---

## Registry and INI

Root keys are the predefined globals `HKLM`, `HKCU`, `HKCR`, `HKU`, `HKCC` and
`SHCTX`. Subkey paths take forward slashes and are rewritten to backslashes on
the way out.

### WriteRegStr / WriteRegDWORD

`writeReg` is both: the value's type picks the instruction.

**Usage** `writeReg(rootKey, subKey, entryName, value)` → nothing

```lua
writeReg(HKLM, "Software/Example", "Path", INSTDIR)
writeReg(HKLM, "Software/Example", "Build", 42)
```

### The other write forms

| NSIS                | Installua                                         |
| ------------------- | ------------------------------------------------- |
| `WriteRegExpandStr` | `writeRegExpandStr(root, subKey, name, value)`    |
| `WriteRegBin`       | `writeRegBin(root, subKey, name, hexString)`      |
| `WriteRegMultiStr`  | `writeRegMultiStr(root, subKey, name, hexString)` |
| `WriteRegNone`      | `writeRegNone(root, subKey, name[, hexData])`     |

### ReadRegStr / ReadRegDWORD

A missing key or value gives `""` or `0` and sets the error flag, so the empty
string is the test most scripts want.

**Usage** `readRegStr(root, subKey, entryName)` → `string` ·
`readRegDword(root, subKey, entryName)` → `int`

```lua
local path = readRegStr(HKLM, "Software/Example", "Path")
```

### DeleteRegKey / DeleteRegValue

`deleteRegKey` deletes the key and everything under it — what an uninstaller
wants and a footgun everywhere else.

**Usage** `deleteRegKey(root, subKey[, { … }])` · `deleteRegValue(root, subKey, entryName)`
**Options** `ifEmpty`, `ifNoSubKeys`, `ifNoValues`

```lua
deleteRegKey(HKLM, "Software/Example")
```

### EnumRegKey / EnumRegValue

The name of the _n_-th subkey or value, counting from zero, or `""` past the
end.

**Usage** `enumRegKey(root, subKey, index)` → `string` ·
`enumRegValue(root, subKey, index)` → `string`

```lua
local key = enumRegKey(HKLM, "Software/Example", 0)
```

### ReadINIStr / WriteINIStr / DeleteINIStr / DeleteINISec / FlushINI

| NSIS           | Installua                                            |
| -------------- | ---------------------------------------------------- |
| `ReadINIStr`   | `readIniStr(iniFile, section, entryName)` → `string` |
| `WriteINIStr`  | `writeIniStr(iniFile, section, entryName, value)`    |
| `DeleteINIStr` | `deleteIniStr(iniFile, section, entryName)`          |
| `DeleteINISec` | `deleteIniSection(iniFile, section)`                 |
| `FlushINI`     | `flushIni(iniFile)`                                  |

```lua
writeIniStr(INSTDIR .. "/app.ini", "General", "Path", INSTDIR)
flushIni(INSTDIR .. "/app.ini")
```

---

## Processes and the shell

### Exec / ExecWait / ExecShell / ExecShellWait

| NSIS            | Installua                                                             |
| --------------- | --------------------------------------------------------------------- |
| `Exec`          | `exec(commandLine)`                                                   |
| `ExecWait`      | `execWait(commandLine)` → `int` (the exit code)                       |
| `ExecShell`     | `execShell(flags, verb, file[, parameters[, showmode[, { … }]]])`     |
| `ExecShellWait` | `execShellWait(flags, verb, file[, parameters[, showmode[, { … }]]])` |

```lua
local code = execWait('"' .. INSTDIR .. '/setup-driver.exe" /S')
if code ~= 0 then abort("driver setup failed") end
```

### CreateShortcut

**Usage** `createShortcut(linkPath, target[, parameters[, iconFile[, iconIndex[, showMode[, hotkey[, comment[, { … }]]]]]]])` → nothing

```lua
createShortcut(SMPROGRAMS .. "/Example/Example.lnk", INSTDIR .. "/Example.exe")
```

### RegDLL / UnRegDLL

Calls `DllRegisterServer` / `DllUnregisterServer` on a DLL already on the target.

**Usage** `regDll(path[, entryPoint])` · `unRegDll(path)`

```lua
regDll(INSTDIR .. "/shell.dll")
```

### GetDLLVersion / GetDLLVersionLocal

The version resource of a DLL — on the target, or on the **build** machine.

**Usage** `getDllVersion(filename[, { … }])` → `int, int` (high, low) ·
`getDllVersionLocal(localFilename)` → `int, int`

### Reboot / SetRebootFlag / IfRebootFlag

**Usage** `reboot()` · `setRebootFlag(true)` · `rebootFlag()` → `boolean`

### Sleep

**Usage** `sleep(milliseconds)` → nothing

---

## Strings and numbers

Arithmetic and comparison are Lua operators, lowered onto `IntOp` and the
`*Cmp` family; width and sign are attributes of the type, so there is no
`Int64Cmp` to write. There are no floats.

| NSIS                                                       | Installua                                                           |
| ---------------------------------------------------------- | ------------------------------------------------------------------- |
| `StrCpy`                                                   | assignment: `x = y`                                                 |
| `StrCmpS`                                                  | `==`, which is **case-sensitive** — the reversal that bites hardest |
| `StrCmp`                                                   | `string.lower(a) == b`, which folds to one case-insensitive compare |
| `StrLen`                                                   | `string.len(s)` → `int`                                             |
| `IntOp`                                                    | `+ - * / % // & \| ~ << >>`                                         |
| `IntCmp` / `Int64Cmp` / `IntPtrCmp` and the unsigned forms | `< <= > >= == ~=`                                                   |
| `IntFmt` / `Int64Fmt`                                      | `string.format`                                                     |

### The string adapters

Hand-written lowerings onto NSIS instructions and `StrFunc` macros — the
`${Using:StrFunc}` lines are collected and emitted for you.

| Installua                                        | Notes                                 |
| ------------------------------------------------ | ------------------------------------- |
| `string.len(s)` → `int`                          | counts UTF-16 code units              |
| `string.sub(s, i[, j])` → `string`               | negative indices follow Lua, not NSIS |
| `string.find(s, needle)` → `int`                 |                                       |
| `string.lower(s)` / `string.upper(s)` → `string` |                                       |
| `string.format(fmt, value)` → `string`           | `IntFmt`, so one integer              |

```lua
func("majorOf", function(version)
	local dot = string.find(version, ".")
	return string.sub(version, 1, dot - 1)
end)
```

### ExpandEnvStrings / ReadEnvStr

| NSIS               | Installua                             |
| ------------------ | ------------------------------------- |
| `ReadEnvStr`       | `readEnvStr(name)` → `string`         |
| `ExpandEnvStrings` | `expandEnvStrings(string)` → `string` |

### ReadMemory

**Usage** `readMemory(address, size)` → `string`

---

## Flow, errors and messages

NSIS's `Goto`-and-label instructions have no spelling. `IfErrors`,
`IfFileExists` and the rest are _predicates_ — they read as questions inside an
ordinary `if`, and the compiler writes the labels.

### Goto / Return / Quit

| NSIS     | Installua                           |
| -------- | ----------------------------------- |
| `Goto`   | `if`, `while`, `break` and `return` |
| `Return` | `return`                            |
| `Quit`   | `os.exit()`                         |

### Abort

Stops the current section or callback. In a page callback it keeps the user on
the page; in `.onInit` it cancels the install.

**Usage** `abort([message])` → nothing

```lua
abort("stopped")
```

### DetailPrint / SetDetailsPrint

Adds a line to the details view — the log the "Show details" button reveals. The
only logging a stock `makensis` has.

**Usage** `detailPrint(message)` · `setDetailsPrint("none" | "listonly" | "textonly" | "both" | "lastused")`

```lua
detailPrint("installing")
```

### IfErrors / ClearErrors / SetErrors

The error flag, which most instructions set on failure instead of raising. It is
sticky: clear it immediately before the thing you mean to test.

**Usage** `errors()` → `boolean` · `clearErrors()` · `setErrors()`

```lua
clearErrors()
writeReg(HKLM, "Software/Example", "Path", INSTDIR)
if errors() then detailPrint("could not write the key") end
```

### SetErrorLevel / GetErrorLevel

The exit code the installer's process returns.

**Usage** `setErrorLevel(n)` · `getErrorLevel()` → `int`

### IfFileExists

True when the path exists on the target at install time. Wildcards allowed; a
directory matches.

**Usage** `fileExists(filename)` → `boolean`

```lua
if fileExists(INSTDIR .. "/app.exe") then detailPrint("present") end
```

### IfSilent / SetSilent

True when the installer was started with `/S`. A silent install must not open a
dialog, which is what this is for.

**Usage** `silent()` → `boolean` · `setSilent("silent" | "normal")`

### IfAbort / GetInstDirError

| NSIS              | Installua                   |
| ----------------- | --------------------------- |
| `IfAbort`         | `aborted()` → `boolean`     |
| `GetInstDirError` | `getInstDirError()` → `int` |

### MessageBox

Shows a dialog and returns the button the user pressed, by name. Written with a
table rather than positionally, because the flags NSIS fuses into one argument
are separate decisions.

**Usage** `messageBox { text = …, buttons = …, icon = …, silentAnswer = … }` → `string`

```lua
local answer = messageBox {
	text = "Remove Example and all of its files?",
	buttons = "YESNO",
	icon = "QUESTION",
	silentAnswer = "NO",
}
if answer == "NO" then
	os.exit()
end
```

### SetAutoClose

**Usage** `setAutoClose(true)` → nothing

---

## Windows facts

### GetWinVer

**Usage** `getWinVer(field)` → `int`

```lua
if getWinVer("MAJOR") >= 10 then detailPrint("modern") end
```

### GetKnownFolderPath

**Usage** `getKnownFolderPath(knownFolderId)` → `string`

### SetShellVarContext / GetShellVarContext / IfShellVarContextAll

Whether `$SMPROGRAMS` and its relatives mean the current user's or every user's.

**Usage** `setShellVarContext("all" | "current" | "lastused")` ·
`getShellVarContext()` → `string` · `shellVarContextAll()` → `boolean`

```lua
setShellVarContext("all")
```

### SetRegView / GetRegView / IfAltRegView

Which half of the registry a 32-bit installer sees on a 64-bit machine. `"32"`
and `"64"` are enum members rather than numbers: the argument names a view
rather than counting anything.

**Usage** `setRegView("32" | "64" | "default" | "lastused")` ·
`getRegView()` → `string` · `altRegView()` → `boolean`

```lua
setRegView("64")
```

---

## Plugins and headers

### plugin

Names a plugin DLL and returns a table whose methods are its calls. The DLL name
**is** the namespace, so nothing has to say where the file is as long as it is
in `NSISDIR/Plugins`; a vendored one adds a `dir` to its declaration. The
compiler reserves the DLL when `.onInit` can reach the call.

**Usage** `local p = plugin(name)` · `p.method(…)` → its outputs

Ships declared: `nsExec.execToStack`, `UserInfo.getAccountType`, `System.call`,
and third-party `nsProcess.findProcess` / `.killProcess` / `.closeProcess`.
Anything else — every third-party DLL — is declared by the project in
[`.installua/headers/*.toml`](#declaring-a-third-party-plugin-or-header), which
is what supplies the output count nothing can ask the DLL for.

```lua
local nsExec = plugin "nsExec"
local userInfo = plugin "UserInfo"

onInit(function()
	if userInfo.getAccountType() ~= "Admin" then
		abort("administrator rights are required")
	end
end)
```

### import

Brings a declared NSIS header's macros into scope. The `!include` and any
`${Using:…}` init lines are emitted for you, once, in the right place.

**Usage** `local h = import(header)` · `h.macro(…)`

Ships declared: `FileFunc.getSize`, `FileFunc.driveSpace`,
`WordFunc.versionCompare`. Any other header's macros are declared by the project
in [`.installua/headers/*.toml`](#declaring-a-third-party-plugin-or-header).
`import` itself needs no declaration: the `!include` is emitted for whatever name
it is given, so a header reached only through `raw` still gets its line.

```lua
local fileFunc = import "FileFunc"
local wordFunc = import "WordFunc"

local freeMib = fileFunc.driveSpace("C:/", "/D=F /S=M")
detailPrint(string.format("%u MiB free", freeMib))

local installed = readRegStr(HKLM, "Software/Example", "Version")
if wordFunc.versionCompare(installed, "1.4.2") == "1" then
	detailPrint("downgrade")
end
```

### raw

Text handed to `makensis` unread. It produces no value, no `local` survives it,
and a failure inside one is reported as _yours_ rather than the compiler's
.

**Usage** `raw [[ … ]]` → nothing

```lua
raw [[
  SetRegView 64
]]
```

#### raw.head and raw.tail

Inside a body, `raw [[ … ]]` means _here_, and where it lands needs no saying.
At the top level there is no _here_: the emitter's slots are fixed and the order
you write declarations in is not the order they come out in. So a top-level
block names its anchor, and the anchor is part of the name — there is no form
that leaves it off.

**Usage** `raw.head [[ … ]]` · `raw.tail [[ … ]]` → nothing

```lua
raw.head [[ !system 'git rev-parse --short HEAD > rev.txt' ]]
raw.tail [[ !packhdr "tmp.dat" '"upx.exe" "tmp.dat"' ]]
```

| Anchor | Where | For |
| ------ | ----- | --- |
| `head` | above every line the compiler writes, including `Unicode` | text producing a value the script then reads: `!system`, `!tempfile`, `!getdllversion` |
| `tail` | below everything | registrations `makensis` acts on when the build ends: `!packhdr`, `!finalize`, `!uninstfinalize` |

Two, and a third arrives when a real script needs one. Each names a **boundary
between numbered slots**, never a region — "before everything" and "after
everything" are boundaries no future slot can move, which is what makes them
safe to promise while the rest of the spine is still settling.

**What may go at an anchor** is text whose meaning is position-independent. Text
whose meaning depends on what the compiler generated is a _declaration the
compiler places_, not an anchor's business: `!addplugindir` written at `head`
would land above `Unicode`, bind to the default target, and silently break every
`unicode = false` build — so it is a slot the compiler owns, reached through
`dir` on a [`[[plugin]]` declaration](#declaring-a-third-party-plugin-or-header).
`$PLUGINSDIR` is the same rule from the other side, and it is diagnosed: the
directory is made by an `InitPluginsDir` the compiler puts above the statement
naming it, and an anchor is outside every body.

### Declaring a third-party plugin or header

One file per plugin or header in `.installua/headers/`, read by the compiler,
the editor stubs and the linter alike. The file name is yours; the extension is
`.toml`.

```toml
# .installua/headers/nsis7z.toml
[[plugin]]
name = "Nsis7z"                       # what `plugin "…"` is given
method = "extractWithDetails"         # what you call it
nsis = "Nsis7z::ExtractWithDetails"   # what NSIS is given
params = ["path", "string"]           # positions, in order
outputs = ["string"]                  # values pushed, in `Pop` order
dir = "vendor/plugins"                # only if the DLL is not in NSISDIR

[[header]]
name = "TextFunc"                     # what `import "…"` is given
method = "trimNewLines"
nsis = "TrimNewLines"                 # the macro name, without `${}`
params = ["string"]
outputs = ["string"]                  # trailing registers, in the order written
```

`name`, `method` and `nsis` are required. `nsis` is not derived from `method`,
because `${StrCase} $0 "text" "L"` puts its destination _first_ and
`${GetSize} "$dir" "" $0 $1 $2` puts it _last_: there is no convention to infer,
and guessing emits NSIS that looks right and is not.

**The types** are `string`, `path`, `int`, `uint`, `int64`, `intptr`, `bool`,
`handle` and `any`. `path` is an input spelling — a `string` whose `/` becomes
`\` on the way in — so it is rejected in `outputs`, where the callee has already
written whatever it wrote. `uint` is worth reaching for on a count or a size:
knowing a value cannot be negative is what elides the sign fixup on `//`.

**`outputs` is the load-bearing line.** NSIS offers no way to ask a DLL how many
values it pushes, so `local rc, out = …` is checked against this list and
nothing else. A count that is too small unbalances the stack, with no diagnostic
from NSIS or from anybody.

**`dir` is for a DLL that does not live in `NSISDIR/Plugins`** — a plugin
vendored into your own repository. It is relative to the project root, and the
compiler emits one `!addplugindir` for it, in the one position the directive is
correct in: under the `Unicode` line and above every call site. You never write
that line yourself, and there is no anchor that would let you — an untagged
`!addplugindir` binds to whichever target is current when it is processed, so
one written at the top of a source would bind to the default target and silently
break every `unicode = false` build. Only a plugin the program actually calls
emits a line.

Redeclaring one of the builtins is allowed and replaces it, so a count that
ships wrong here is not a wall. Declaring the same method twice from two of your
own files is a mistake, and says so.

Run `installua stubs` after adding a declaration: the compiler reads the `.toml`
on every build, but the editor reads the generated stub.

What ships declared lives in `src/headers/*.toml` in this repository, in this
same format and read by this same parser — copy one into `.installua/headers/`
to correct it, or send it back as a pull request so nobody else has to.

---

## Constants

Predefined NSIS constants, as ordinary read-only globals. A `$` inside a string
literal is five dollars, not a variable — join with `..` instead.

**Paths** `INSTDIR` · `OUTDIR` · `PROGRAMFILES` · `PROGRAMFILES64` ·
`COMMONFILES` · `DESKTOP` · `STARTMENU` · `SMPROGRAMS` · `APPDATA` ·
`LOCALAPPDATA` · `TEMP` · `WINDIR` · `SYSDIR` · `EXEDIR` · `EXEPATH` ·
`EXEFILE` · `PLUGINSDIR`

`INSTDIR` and `OUTDIR` are writable; the rest are facts about the machine and
assigning to one is refused.

**Other** `LANGUAGE` · `HWNDPARENT` — the installer's own window

**Registry roots** `HKLM` · `HKCU` · `HKCR` · `HKU` · `HKCC` · `SHCTX`

```lua
setOutPath(PROGRAMFILES64 .. "/Example")
```

---

## Not available

Everything with no Installua spelling, and what to write instead. Nothing here
is pending: these are decisions.

### The preprocessor

Installua has no preprocessor — the script _is_ a program. 36 of the 37 `!`
directives are out:

`!addincludedir` · `!appendfile` · `!appendmemfile` ·
`!assert` · `!cd` · `!define` · `!delfile` · `!echo` · `!else` · `!endif` ·
`!error` · `!execute` · `!finalize` · `!getdllversion` · `!gettlbversion` ·
`!if` · `!ifdef` · `!ifmacrodef` · `!ifmacrondef` · `!ifndef` · `!include` ·
`!insertmacro` · `!macro` · `!macroend` · `!macroundef` · `!makensis` ·
`!packhdr` · `!pragma` · `!searchparse` · `!searchreplace` · `!system` ·
`!tempfile` · `!undef` · `!uninstfinalize` · `!verbose` · `!warning`

Write `local X <const> = …`, `if`, `include(…)` and a function instead.

Four of those are worth naming, because "out" reads harsher than it is:
`!define`, `!undef`, `!ifdef` and `!ifndef` are not withheld, they are
*unnecessary*. A top-level `local X <const> = …` **is** a `!define` — that is
what it emits — and the `!ifndef`/`!define`/`!endif` sandwich that gives one a
settable default is [`param`](#param), which does the same job and additionally
rejects a `-D` nobody declared.

"Out" means there is no Lua spelling, not that the line is unreachable. Six of
them are ones a real script does want and no construct replaces — `!system`,
`!tempfile`, `!getdllversion` and `!packhdr`, `!finalize`, `!uninstfinalize` —
and those go in a [`raw.head` or `raw.tail`](#rawhead-and-rawtail) block, which
is the escape hatch with a position attached.

The one that is not out is `!addplugindir`, and it is not out because the
compiler writes it rather than because you may: it has exactly one legal
position, and a `dir` on a
[`[[plugin]]` declaration](#declaring-a-third-party-plugin-or-header) is how you
ask for it.

```lua
local APP <const> = "Example"
local VERSION <const> = "1.4.2"
```

### Written by the compiler, never by you

These are real NSIS lines in the output — you just do not spell them. Listed
here so a search for the NSIS name lands somewhere.

| NSIS                                                                         | What writes it                    |
| ---------------------------------------------------------------------------- | --------------------------------- |
| `!define`                                                                    | `local X <const> = …`, `param`    |
| `Goto`                                                                       | `if`, `while`, `break`            |
| `Call`                                                                       | a call: `f(x)`                    |
| `Push` / `Pop` / `Exch`                                                      | the calling convention            |
| `StrCpy`                                                                     | assignment                        |
| `StrCmp` / `StrCmpS`                                                         | `==`                              |
| `IntOp` / `IntPtrOp`                                                         | the arithmetic operators          |
| `IntCmp` / `IntCmpU` / `Int64Cmp` / `Int64CmpU` / `IntPtrCmp` / `IntPtrCmpU` | comparison                        |
| `IntFmt` / `Int64Fmt`                                                        | `string.format`                   |
| `LangString`                                                                 | `languages { locales = { … } }`   |
| `LicenseLangString`                                                          | `page.license { file = { … } }`   |
| `InitPluginsDir`                                                             | a body that names `PLUGINSDIR`    |
| `ReserveFile /plugin`                                                        | a plugin `.onInit` can reach      |
| `!addplugindir`                                                              | a plugin declared with a `dir`    |
| `SendMessage`                                                                | a control's `value` / `checked`   |
| `EnableWindow` / `ShowWindow`                                                | a control's `enabled` / `visible` |
| `SetCtlColors` / `CreateFont`                                                | a control's `colors` / `font`     |
| `LoadAndSetImage`                                                            | a `bitmap`'s `image`              |
| `GetFunctionAddress`                                                         | `onClick` / `onChange`            |

### Rejected NSIS commands

| NSIS                                     | Write instead                                           |
| ---------------------------------------- | ------------------------------------------------------- |
| `CallInstDLL`                            | `plugin.method(…)`                                      |
| `ChangeUI`                               | — MUI2 owns the dialog resources                        |
| `DirShow`                                | — NSIS reports this one as not working                  |
| `FindFirst` / `FindNext` / `FindClose`   | `for path in glob("…") do`                              |
| `GetCurrentAddress` / `GetLabelAddress`  | — there are no labels to address                        |
| `LangStringUP`                           | `languages { locales = { … } }`                         |
| `LoadLanguageFile`                       | `languages { locales = { … } }`                         |
| `LogSet` / `LogText`                     | `detailPrint` — logging needs a custom `makensis` build |
| `Nop`                                    | write nothing                                           |
| `PageEx` / `PageExEnd` / `PageCallbacks` | `page.*`                                                |
| `SectionInstType`                        | a section's `installTypes`                              |
| `SetPluginUnload`                        | — NSIS retired it                                       |
| `SubSection` / `SubSectionEnd`           | `group(…)`                                              |
| `Target`                                 | `cpu` and `unicode`                                     |
| `UninstallExeName`                       | `writeUninstaller(…)`                                   |
| `UnsafeStrCpy`                           | `=`                                                     |
| `XPStyle`                                | — MUI2 emits it itself                                  |

### Lua that is not here

Installua is Lua-shaped, not Lua (see [lua-shaped-not-lua.md](lua-shaped-not-lua.md)):
no floats, no closures as values, no metatables, no `pairs`, no `require` as a
runtime load, `#` rejected on strings, and truthiness only for booleans.

---

## Generating this file

This document is hand-written, which is exactly what a correspondence document
must not stay. Most of it is already in the tables:

| Part of an entry          | Where it comes from today                                                                                               |
| ------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| Heading (NSIS name)       | `Row::nsis`                                                                                                             |
| Signature                 | a call renderer over `Instruction`, the shape `src/table/doc.rs` used to print before it was retired with `LANGUAGE.md` |
| Return type               | `Instruction::outputs()` and the `Ty` of each                                                                           |
| Options list              | `Row::options`, the `Offer` names                                                                                       |
| Example                   | `Row::example` — already written, already compiled                                                                      |
| Attribute usage line      | `Class::Attribute(Setting)`                                                                                             |
| "Written by the compiler" | `Class::Lowering(spelling)`                                                                                             |
| "Rejected" reason         | `Class::Rejected(why)`                                                                                                  |
| MUI spelling              | `mui::rows::Row`'s second field                                                                                         |
| **Group**                 | **new: `Row::group`**                                                                                                   |
| **Description**           | **new: `Row::blurb`, one or two sentences**                                                                             |

So the generator is `src/table/reference.rs`, a
`cargo run -q -- generate reference > docs/reference-map.md` arm beside `generate table`, and two
new fields the census can require exactly the way it requires a class today — a
command with no group is a census failure, which is what stops a new NSIS
version quietly adding an undocumented command.

Open questions for the group axis:

1. **One group per command, or several?** `CreateShortcut` is both "files and
   directories" and "processes and the shell". One group keeps the document a
   partition and the census a count; several needs an index instead.
2. **Where do the non-command surfaces go?** `page.*`, `import`, `glob`, the
   control types and `string.*` have no `-CMDHELP` line and so no row, but a
   user reference without them has holes exactly where beginners look. This
   draft writes them by hand; a generator needs rows of their own.
3. **The MUI half is a second table.** `src/mui/rows.rs` carries one spelling
   per name and no group, so the same two fields have to land there too.
