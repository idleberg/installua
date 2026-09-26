---
title: Sections and install types
description: What the components page offers, and what each part costs.
---

A section is a unit of work the user can turn off. Sections are written inside
`installer {}` or `uninstaller {}`; the components page lists them in the order
they appear.

## Section / SectionEnd

Declares a section: a name and a body. The table form takes options beside the
name — `optional` starts it unticked, `required` makes it untickable,
`installTypes` puts it in the presets, `description` is its hover text.

**Usage** `section(name, body)` or
`section { <name>, body = …, optional = …, required = …, installTypes = { … }, size = …, description = … }`

```lua
installer {
	section("Core", function()
		setOutPath(INSTDIR)
		file("assets/Example.exe")
		writeUninstaller(INSTDIR .. "/uninstall.exe")
	end),

	section { "Start menu shortcut",
		optional = true,
		description = "Adds Example to the Start menu.",
		body = function()
			createDirectory(SMPROGRAMS .. "/Example")
			createShortcut(SMPROGRAMS .. "/Example/Example.lnk", INSTDIR .. "/Example.exe")
		end,
	},
}
```

## SectionGroup / SectionGroupEnd

Groups sections under one collapsible heading on the components page. Ticking
the heading ticks the children. A group can hold groups too. Takes
`description` like a section does.

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

## InstType

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

## SectionIn

Which install types a section belongs to, by name, plus `required` for the
untickable one.

**Usage** a `section`'s `installTypes = { <name>, … }` and `required = <boolean>`

```lua
installer {
	installTypes = { "Typical", "Full" },
	section { "Documentation",
		installTypes = { "Full" },
		body = function() file("assets/manual.pdf") end,
	},
}
```

## AddSize

Adds kilobytes to a section's reported size, for things the compiler cannot
weigh — a download, a generated file, a database built on first run.

**Usage** a `section`'s `size = <int>` (kilobytes)

```lua
section { "Offline map data",
	size = 240000,
	body = function() exec(INSTDIR .. "/fetch-maps.exe") end,
}
```

## Remembering what was ticked

The components page can start the way the user left it last time, through
the `Memento.nsh` header that ships with NSIS. The script names a registry key,
and each section to remember gets an id. The id is the name of a registry
value, so keep it the same from one version to the next, or that choice is
forgotten.

**Usage** a top-level `memento { root = <root>, key = <key> }`, and a
`section`'s `remember = <id>` (letters, digits and `_`)

```lua
memento { root = HKLM, key = "Software/Example/Components" }

installer {
	page.components {},
	page.instFiles {},

	section { "Documentation",
		remember = "docs",
		body = function() file("assets/manual.pdf") end,
	},

	section { "Samples",
		remember = "samples",
		optional = true,
		body = function() file("assets/samples/*.example") end,
	},
}
```

On the first run `optional` decides, as it always does. On a later run the
stored choice does, and a section that is new since then is drawn in bold.
The choice is saved only when the install succeeds. Beside
[`multiUser {}`](/reference/commands/script-attributes/#per-machine-or-per-user),
write `root = SHCTX`, so each mode keeps its own choice.

Only installer sections can remember. The compiler writes the header's lines:
`MementoSectionEx` and `MementoSectionEnd` around each remembered section,
`MementoSectionDone` after the last one, `MementoSectionRestore` at the start
of `.onInit` and `MementoSectionSave` at the start of `.onInstSuccess`, which
it writes itself when the block has no `onInstSuccess`.

## One of several

Sections the user picks between, as radio buttons: ticking one unticks the one
that was ticked, and unticking it ticks it again. The first section listed is
the one ticked at the start, so mark the others `optional`. Lowered through the
`Sections.nsh` header that ships with NSIS.

**Usage** a `radioButtons { <section>, … }` entry of `installer {}` or
`uninstaller {}`, listing two or more of that block's sections

```lua
local small = section("Small", function() end)
local large = section { "Large", optional = true, body = function() end }

installer {
	page.components {},
	page.instFiles {},

	small,
	large,
	radioButtons { small, large },
}
```

The compiler writes the header's `StartRadioButtons`, `RadioButton` and
`EndRadioButtons` at the start of `.onSelChange`, which it writes itself when
the block has no `onSelChange`, and the starting section into a `Var` in
`.onInit`.

## Reading and changing a section while it runs

Your installer reaches a section through the handle `section(…)` gives back,
rather than by counting positions. `group(…)` gives back the same kind of
handle. These eight NSIS commands all become fields on it:

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

## The install type the user picked

By name, rather than by the number NSIS uses. Covers `GetCurInstType`,
`SetCurInstType`, `InstTypeGetText` and `InstTypeSetText`.

**Usage** `currentInstType` → `string`, `currentInstType = <name>`,
`instTypes.getText(name)` → `string`, `instTypes.setText(name, text)`

```lua
installer {
	installTypes = { "Typical", "Full" },
	onInit(function()
		currentInstType = "Full"
		instTypes.setText("Full", "Everything")
	end),
}
```

## WriteUninstaller

Writes the uninstaller executable, built from the `uninstaller {}` block. Call
it from a section — usually the first — after `setOutPath`.

**Usage** `writeUninstaller(path)` → nothing

```lua
writeUninstaller(INSTDIR .. "/uninstall.exe")
```

---
