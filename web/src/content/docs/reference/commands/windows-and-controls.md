---
title: Windows and controls
description: "`page.custom`: controls, colours, fonts, events."
---

`page.custom` is the eighth page and the only one that is not a MUI2 macro: it
is a `Page custom`, and both functions behind it are the compiler's to write
. A control is a call listed in the page's `controls`; the `local` it is
bound to is how a running program addresses it, and decides nothing about where
it sits.

**Usage** `page.custom { <headerText>, controls = { … }, pre = …, show = …, leave = … }`

## The control types

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

## A control's fields

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

## GetDlgItem / FindWindow / IsWindow / SendMessage

Reaching a window Installua did not draw — MUI2's own Cancel button, or another
process's.

| NSIS         | Installua                                                                |
| ------------ | ------------------------------------------------------------------------ |
| `GetDlgItem` | `getDlgItem(dialog, itemId)` → `handle`                                  |
| `FindWindow` | `findWindow(class[, { title, parent, childAfter }])` → `handle` |
| `IsWindow`   | `isWindow(hwnd)` → `boolean`                                             |

```lua
local cancel = getDlgItem(HWNDPARENT, 2)
cancel.enabled = false
```

## HideWindow / BringToFront / LockWindow / SetBrandingImage / SetDetailsView

| NSIS               | Installua                          |
| ------------------ | ---------------------------------- |
| `HideWindow`       | `hideWindow()`                     |
| `BringToFront`     | `bringToFront()`                   |
| `LockWindow`       | `lockWindow("on" \| "off")`        |
| `SetBrandingImage` | `setBrandingImage(path[, { … }])`  |
| `SetDetailsView`   | `setDetailsView("hide" \| "show")` |

---
