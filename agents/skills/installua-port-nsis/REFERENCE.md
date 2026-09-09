# Porting reference

## Dissolving the preprocessor

Installua has no preprocessor. The script *is* a program, so the two-stage
`!`-layer collapses into ordinary code. Seven of the directives are not
withheld, they are **unnecessary**:

| NSIS | Installua | Note |
| ---- | --------- | ---- |
| `!define X 5` | `local X <const> = 5` | used as `X`, emitted as `${X}` |
| `!ifndef X` / `!define X` / `!endif` | `local X <const> = param("X", default)` | `-D` is the same flag `makensis` spells the same way; also rejects a `-D` nobody declared |
| `!if` / `!else` / `!endif` | ordinary `if` | the compiler folds what is const |
| `!macro` / `!macroend` / `!insertmacro` | `func("name", function() … end)`, or a plain call | a macro taking arguments is a function taking arguments |
| `!include "X.nsh"` (yours) | `include("x.lua")` | |
| `!include "FileFunc.nsh"` (stdlib header) | `local fileFunc = import "FileFunc"` | |
| `!include "WinVer.nsh"` | nothing — `getWinVer("MAJOR")` is a real instruction since NSIS 3 | returns a number |
| `!include "LogicLib.nsh"` | nothing — `if`, `while` are the language | |
| `!include "nsDialogs.nsh"` | nothing — `page.custom` compiles the same style constants | |
| `!system`, `!execute`, `!searchparse`, … | no spelling | do it in the build that calls `installua`, not in the script |

**The macro trap.** An NSIS macro is textual substitution: it can declare
labels, fall through, and read the caller's registers. A function cannot. When a
macro does something a function cannot express, that is a genuine restructure —
say so and rewrite the logic, rather than reaching for `raw` to preserve the
shape.

**A build-time value is not a runtime one.** `!define`-shaped things are
compile-time constants and the compiler will tell you when you have crossed the
line. `concepts/lua-shaped-not-lua.md` §"Two stages, and you can always tell which"
is the authority.

## Construct by construct

The full table lives in `concepts/nsis-shaped-not-nsis.md` §"Where things you know
live now" — read it once at the start of a port. The entries that catch people:

| NSIS | Installua |
| ---- | --------- |
| `Name`, `OutFile`, `SetCompressor`, … at top level | fields of `attributes {}`, reordered canonically on output |
| `InstallDir`, `Caption`, `Icon` | `attributes {}`, and also `installer {}` entries when scoped to that half |
| `InstallDirRegKey` | `attributes {}` only — it has no `installer {}` spelling |
| `UninstallIcon`, `UninstallSubCaption` | `icon`, `page.*.subCaption` under `uninstaller {}` |
| `UninstallCaption` | `uninstallCaption` in `attributes {}` — not a twin |
| `UninstallText` | `page.confirm { topText = … }` — not a twin |
| `ManifestDPIAwareness`, … | `manifest = { dpiAwareness = … }` |
| `VIProductVersion`, `VIAddVersionKey` | `versionInfo = { product = …, keys = { … } }` |
| `LangString` | the `languages {}` block, keyed locale-first |
| `${If}` / `${While}` | ordinary `if` / `while` |
| `Push` / `Pop` / `Exch` | nothing — the compiler owns the stack |
| `$0`–`$R9` | nothing — the compiler owns the registers, and they are not nameable |
| `Goto`, labels, `Goto +2` | `if`, `while`, `break`, `continue()` |
| `StrCmp`, `IntCmp`, `IfErrors`, `IfFileExists` as commands | `==`, `<`, `errors()`, `fileExists(p)` — expressions, not label-takers |
| `MessageBox` with a jump table | `local answer = messageBox { … }`, then compare `answer` |
| `IntOp` with `!`, `&&`, `\|\|` | ordinary `and` / `or` / `not` |
| `Int64Op` | does not exist in NSIS either; `int64` arithmetic is a hard error naming the reason |

## Pages

MUI2 settings are `!define`s that apply to *the next* `MUI_PAGE_*` macro and are
then undefined, so hand-written MUI2 can attach a header to the wrong page
silently. Grouping them into `page { … }` is what makes that unexpressible:

```lua
page.directory {
  headerText        = "Choose a location",
  directoryVariable = INSTDIR,
  pre = function() … end,
}
```

Include order is never yours to get right — the compiler writes every
`!insertmacro` and `!define` in the order MUI2 requires. A ported script that
carefully preserves the original's include order has preserved nothing.

## What earns a `PORT:` marker

Earns one: a construct dropped because it is `not-yet-implemented`; a setting
that was scoped to a body and is now whole-program; a `-D` flag renamed; a
`StrCmp` whose case-sensitivity you had to choose; a macro whose logic you
rewrote rather than translated; a warning the original also emits, so the next
reader does not re-diagnose it; anything left unported.

Does not earn one: a spelling you looked up and applied. A port where every line
carries a marker has marked nothing.

## Known doc drift, and limits to plan around

The drift found while porting `Examples/bigtest.nsi` has been corrected in
the reference, but the *shape* of it recurs, so check for it rather than trusting a
usage line:

**A row with more than one optional position names all of them.** Exactly one
trailing optional can still be passed positionally (`copyFiles(a, b, 10)`,
`f:seek(0, "END")`); two or more and every one moves into a table written last.
A `**Usage**` line showing nested `[, x[, y]]` past the first optional is stale,
and `error[wrong-arity]` says so with the names.

Structural limits that force a restructure rather than a spelling:

- **A `group` inside a `group` is `not-yet-implemented`.** A nested
  `SectionGroup` has to be flattened, which changes the components tree.
- **A callback written as a dotted `func` goes at the top level**, not inside
  `installer {}` — `func(".onSelChange", …)` is rejected there. Only the
  callbacks with real spellings (`onInit`, `onGUIInit`, `onUserAbort`,
  `onMouseOverSection`) are block entries.
- **`<const>` folding does not go through `and`/`or`.** A build switch has to
  carry the value (`param("COMPRESS", "auto")`) rather than a flag the script
  branches on.
- **`installTypes` on a block wants a literal list**, so an `!ifdef` that
  removed the `InstType` lines has nothing to become; every section naming a
  type would break with it gone.
- **A section's `description` wants a compile-time value**, so the common
  `MUI_DESCRIPTION_TEXT ${Sec} $(DESC_Sec)` pairing cannot carry its
  `LangString` across — the string has to be inlined, and a multi-language
  installer loses that one translation. Always a `PORT:` marker.
- **Whole-program attributes cannot be toggled mid-body.** `SetOverwrite`,
  `SetCompress` and friends are `attributes {}` fields; a script that flips one
  around a single `File` loses that window. Always a `PORT:` marker.
- **`SetCompressor`'s `/SOLID` and `/FINAL` have no spelling.** `compressor`
  carries the algorithm only, so `SetCompressor /SOLID lzma` ports as plain
  `compressor = "lzma"` and the installer gets bigger with no diagnostic.
  Always a `PORT:` marker.
- **`Memento.nsh` is not declared**, so `${MementoSection}` — a component
  selection remembered in the registry across installs — has no equivalent.
  The sections port as ordinary ones and the memory is gone. Same for
  `Sections.nsh`, `Util.nsh` and `Integration.nsh`: undeclared, because their
  value is control flow rather than a signature.

## When there is no spelling

1. **Check `commands/not-available.md`** first. It is a list
   of *decisions*, each naming the replacement. Nothing there is pending.
2. **`raw [[ … ]]`** is the escape hatch: text handed to `makensis` unread. No
   value comes out, no `local` survives it, and a failure inside one is reported
   as yours. A value crosses the boundary through a global, whose NSIS name is
   the one you wrote.
3. Reaching for `raw` because a spelling was not found is almost always a
   failed grep. Search the group headings (`ls commands/`)
   and look under the group the command belongs to before concluding it is absent.

## Verifying a port

- `installua check <file>.lua` — everything `build` would say, writing nothing.
- `installua emit` — the `.nsi`, for diffing against the original. Expect the
  include list, macro expansions and register allocation to differ; expect the
  attributes, section names and file lists not to.
- `installua build` — runs `makensis -WX` and rewrites its diagnostics back onto
  the Lua source. Skip cleanly if `makensis` is absent, and say that you did.
- `installua stubs .` — writes `.installua/meta/*.lua`, so the editor can type
  the ported calls.
