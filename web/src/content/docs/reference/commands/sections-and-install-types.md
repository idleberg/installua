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

## SectionGroup / SectionGroupEnd

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
section { "Documentation",
	installTypes = { "Full" },
	body = function() file("assets/manual.pdf") end,
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
onInit(function()
	currentInstType = "Full"
	instTypes.setText("Full", "Everything")
end)
```

## WriteUninstaller

Writes the uninstaller executable, built from the `uninstaller {}` block. Call
it from a section — usually the first — after `setOutPath`.

**Usage** `writeUninstaller(path)` → nothing

```lua
writeUninstaller(INSTDIR .. "/uninstall.exe")
```

---
