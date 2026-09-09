---
title: Flow, errors and messages
description: Aborting, testing, telling the user.
---

NSIS's `Goto`-and-label instructions have no spelling. `IfErrors`,
`IfFileExists` and the rest are _predicates_ — they read as questions inside an
ordinary `if`, and the compiler writes the labels.

## Goto / Return / Quit

| NSIS     | Installua                                               |
| -------- | ------------------------------------------------------- |
| `Goto`   | `if`, `while`, `for`, `break`, `continue()` and `return` |
| `Return` | `return`                                                |
| `Quit`   | `os.exit()`                                             |

## continue

Skips to the next iteration of the innermost loop — the one jump you do spell,
because Lua has no `continue` and what 5.4 added to write the idiom is `goto`,
which is [rejected](/concepts/lua-shaped-not-lua/). It wears a call's syntax and takes
no arguments; it is a jump, so anything after it in the same block is dead.

**Usage** `continue()` → nothing

```lua
for line in lines(handle) do
	if line == "" then
		continue()
	end
	detailPrint(line)
end
```

Every run-time loop carries a target: `while`, the numeric `for`, `for … in
lines(…)` and a [walker](/reference/commands/plugins-and-headers/#walkers)'s `for`. A `for … in glob(…)` does not — that
loop is unrolled at build time, so it is not a loop `continue()` can see. With
no target at all it is an error.

## Abort

Stops the current section or callback. In a page callback it keeps the user on
the page; in `.onInit` it cancels the install.

**Usage** `abort([message])` → nothing

```lua
abort("stopped")
```

## DetailPrint / SetDetailsPrint

Adds a line to the details view — the log the "Show details" button reveals. The
only logging a stock `makensis` has.

**Usage** `detailPrint(message)` · `setDetailsPrint("none" | "listonly" | "textonly" | "both" | "lastused")`

```lua
detailPrint("installing")
```

## IfErrors / ClearErrors / SetErrors

The error flag, which most instructions set on failure instead of raising. It is
sticky: clear it immediately before the thing you mean to test.

**Usage** `errors()` → `boolean` · `clearErrors()` · `setErrors()`

```lua
clearErrors()
writeReg(HKLM, "Software/Example", "Path", INSTDIR)
if errors() then detailPrint("could not write the key") end
```

## SetErrorLevel / GetErrorLevel

The exit code the installer's process returns.

**Usage** `setErrorLevel(n)` · `getErrorLevel()` → `int`

## IfFileExists

True when the path exists on the target at install time. Wildcards allowed; a
directory matches.

**Usage** `fileExists(filename)` → `boolean`

```lua
if fileExists(INSTDIR .. "/app.exe") then detailPrint("present") end
```

## IfSilent / SetSilent

True when the installer was started with `/S`. A silent install must not open a
dialog, which is what this is for.

**Usage** `silent()` → `boolean` · `setSilent("silent" | "normal")`

## IfAbort / GetInstDirError

| NSIS              | Installua                   |
| ----------------- | --------------------------- |
| `IfAbort`         | `aborted()` → `boolean`     |
| `GetInstDirError` | `getInstDirError()` → `int` |

## MessageBox

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

The answers are the button names in capitals — `OK`, `CANCEL`, `YES`, `NO`,
`RETRY`, `ABORT`, `IGNORE` — and comparing one against a string the dialog
cannot give is a warning, not a dead branch that builds. `answer ~= "yes"` is
the one to watch: `==` is `StrCmpS`, so the case is part of the value.

## SetAutoClose

**Usage** `setAutoClose(true)` → nothing

---
