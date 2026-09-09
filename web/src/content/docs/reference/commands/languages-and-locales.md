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
