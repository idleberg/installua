---
title: Languages and locales
description: "`languages { locales = { … } }` and the language dialog."
---

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

A read also works where the value has to be known while building, such as a
section's `description` or a page's text: it is `$(name)`, and NSIS picks the
locale when the installer runs.

## Built-in language strings

NSIS's own strings, the `$(^…)` ones such as `$(^Name)` and `$(^NameDA)`, are
under `lang.builtin`. Every installer has all of them, translated by the
language file, so none is declared. That is also why `builtin` is not a name a
locale can declare.

**Usage** `lang.builtin.<name>`, with the names from NSIS's `Source/lang.cpp`

```lua
attributes { name = "Example" }

installer {
	section("Core", function()
		writeReg(HKLM, "Software/Microsoft/Windows/CurrentVersion/Uninstall/" .. lang.builtin.Name,
			"DisplayName", lang.builtin.Name)
	end),
}
```

## The language dialog

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

## IfRtlLanguage

True when the running language is written right to left.

**Usage** `rtlLanguage()` → `boolean`

---
