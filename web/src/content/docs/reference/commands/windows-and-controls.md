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

A number is dialog units: `x = 25` is `25u`. A string passes through as
nsDialogs reads it — `"25u"`, `"100%"`, `"-13u"`, or plain digits, which are
**pixels**: `x = "25"` is 25 pixels, the unit most nsDialogs scripts use.

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

Either colour may be left out and keeps the control's own. The same holds for
`headerColors` and a directory page's `colors`, where MUI2's default fills in.

On a `dropList` or `listBox`, `value` is the selected row: `""` when none is,
and a write selects the first row that starts with the text, ignoring case — what
`NSD_CB_SelectString` does. `handle.add("text")` appends a row at install time,
where `items` holds the rows known at build time.

`handle.focus()` moves the keyboard focus to the control — `${NSD_SetFocus}`. Call
it in `show`, since focus set in `pre` is lost when the page is drawn. A
`getDlgItem` handle has it too: `local next = getDlgItem(HWNDPARENT, 1)` then
`next.focus()` focuses MUI2's Next button.

A `raw` block reaches a control through a global: `h = serial`, then `$h` in the
`raw`. The global keeps the fields a `getDlgItem` handle has, so `h.enabled` and
`h.focus()` work in any function.

```lua
local agree = checkbox { "I have read the notes", y = 60, height = 12 }
local serial = text { x = 60, y = 90, width = 120, height = 14 }

installer {
	page.custom { "Details",
		controls = { agree, serial },
		show = function()
			serial.font = { face = "Tahoma", size = 8 }
			serial.colors = { text = "800000", background = "transparent" }
			agree.enabled = false
		end,
	},
}
```

## GetDlgItem / FindWindow / IsWindow / SendMessage

Reaching a window Installua did not draw — MUI2's own Cancel button, or another
process's.

| NSIS          | Installua                                                           |
| ------------- | ------------------------------------------------------------------- |
| `GetDlgItem`  | `getDlgItem(dialog, itemId)` → `handle`                             |
| `FindWindow`  | `findWindow(class[, { title, parent, childAfter }])` → `handle`     |
| `IsWindow`    | `isWindow(hwnd)` → `boolean`                                        |
| `SendMessage` | `sendMessage(hwnd, message, wParam, lParam[, { timeout }])` → `int` |

```lua
local cancel = getDlgItem(HWNDPARENT, 2)
cancel.enabled = false
```

A one-off write needs no `local`: `getDlgItem(HWNDPARENT, 2).enabled = false`.

`sendMessage` is every message a field is not. The message is a
[window message constant](/reference/commands/constants/) or a number, and
`wParam` and `lParam` are each a number, a handle or a string. A string is
passed as a pointer to its text, which is what `STR:` means in NSIS. `timeout`
is in milliseconds.

The directory page's path field is item 1019 of the dialog inside the
installer window, and this caps it at 100 characters:

```lua
page.directory {
	show = function()
		local inner = findWindow("#32770", { parent = HWNDPARENT })
		sendMessage(getDlgItem(inner, 1019), EM_LIMITTEXT, 100, 0)
	end,
},
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
