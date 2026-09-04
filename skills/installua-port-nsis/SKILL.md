---
name: installua-port-nsis
description: Port an existing NSIS `.nsi` script to Installua — take an inventory of the script, dissolve its preprocessor layer into ordinary program structure, look each command up in the reference map rather than guessing a spelling, and verify by compiling. Use when a `.nsi`, `.nsh` or NSIS installer script is being converted, rewritten or migrated to Installua, when the user asks how an NSIS command, MUI2 macro, `!define`, `!macro`, LogicLib `${If}` or `$0`-style register is written in Installua, or when a ported script fails to compile.
---

# Porting an NSIS script to Installua

A port is **not a line-by-line translation**. NSIS scripts are two languages
stacked: a preprocessor (`!define`, `!macro`, `!insertmacro`, `!if`) and the
installer commands underneath. Installua has no preprocessor — 36 of the 37 `!`
directives have no spelling — so the top layer does not translate, it
**dissolves** into constants, functions and `if`. Read
[REFERENCE.md](REFERENCE.md#dissolving-the-preprocessor) before touching a
script that uses macros.

## The lookup rule

Never write a spelling from memory. `docs/reference-map.md` is ~1900 lines and
`docs/mui-reference.md` ~250 — **grep them, never read them whole**:

```bash
grep -n -A20 '^### .*\bWriteRegStr\b' docs/reference-map.md   # a command
grep -n -A12 'MUI_PAGE_DIRECTORY' docs/mui-reference.md       # a MUI2 name
grep -n 'SetRegView' docs/reference-map.md                    # not found above? check Not available
```

Headings are the **NSIS** name (grouped, e.g. `### SectionGetText / SectionSetText / …`),
because that is what the source script has. Everything under one is Installua.
No hit anywhere means the command is in [Not
available](#not-available) — that section names what to write instead. If the
repo is not to hand, both files are in `docs/` of
<https://github.com/idleberg/installua>.

**The compiler outranks the docs**, which are hand-written and can drift. Where
a `**Usage**` line shows trailing optional positions, the instruction almost
always takes a **named options table** instead. Known drift:
[REFERENCE.md](REFERENCE.md#known-doc-drift-and-limits-to-plan-around).

## Workflow

1. **Inventory first.** Grep the source for what decides the shape of the port:
   `!macro` / `!insertmacro`, `!include`, `!define`, plugin calls (`::`),
   `Push`/`Pop`, `Goto`, `${If}`, `Var`, `$0`-`$R9`. Report what you found and
   what each becomes before writing any Lua — a macro-heavy script is a
   restructure, not a transcription.
2. **Shell.** `attributes {}` (name, outFile, compressor, manifest,
   versionInfo), then `installer {}` and `uninstaller {}`. Most `Uninstall*`
   twins are the *same field names* under `uninstaller {}` (`icon`,
   `subCaption`) — the prefix is emitted, never written. Two are not:
   `uninstallCaption` is an `attributes {}` field, and `UninstallText` is
   `page.confirm { topText = … }`.
3. **Pages.** Every `!insertmacro MUI_PAGE_*` becomes a `page.<name> { … }`
   listed **inline among the block's entries** — there is no `pages = { … }`
   field. Each preceding `!define MUI_PAGE_*` becomes a field of that page's
   table; the four MUI2 writes once per block (`checkBitmap`, `installColors`,
   `licenseBkColor`, `progressBar`) go on `installer {}` itself.
   `Page`/`PageEx`/`UninstPage` have no spelling — a classic-page script is
   still a page port.
4. **Sections and functions.** `Section`→`section("N", function() … end)`,
   `Section /o`→`{ optional = true }`, `Function`→`func`, `Function .onInit`→
   `onInit`. Anything `un.`-prefixed is declared inside `uninstaller {}` instead;
   `un.` has no spelling.
5. **Bodies.** Look each command up per the lookup rule. `Goto`/labels become
   `if`/`while`/`break`/`continue()`; `Push`/`Pop` and `$0`-`$R9` disappear —
   the compiler owns the stack and the registers.
6. **Plugins and headers.** `nsExec::ExecToStack` → `local nsExec = plugin "nsExec"`.
   `!include "FileFunc.nsh"` → `local fileFunc = import "FileFunc"`. If the
   compiler does not know the plugin, that is the
   `installua-plugin-declaration` skill's job, not a guess here.
7. **Verify.** `installua check <file>.lua`, then `installua build` when
   `makensis` is available. `check` writes nothing, so re-run `emit` before
   reading the `.nsi` — grepping a stale one invents findings.
8. **Mark what changed behaviour.** See below.

## `PORT:` markers

A port has two kinds of leftovers, and only one needs a marker. Anything that
*fails to compile* needs no note — `check` already blocks it. What deserves a
marker is the opposite: **the code that compiles and is not behaviour-identical**
to the original, because nothing downstream will ever raise it again.

Write a numbered `PORT NOTES` block at the top of the file and a matching
`-- PORT: (n)` comment at each site, so the risk surface reads at a glance and
greps in context. Label the loud ones `BEHAVIOUR CHANGE` / `INVOCATION CHANGE`:

```lua
-- PORT NOTES — everything below compiles; none of it is a build error.
--   1. `overwrite` is whole-program, so the `SetOverwrite ifnewer` window the
--      original opened around one `File` is gone. BEHAVIOUR CHANGE.
--   2. `-D NOCOMPRESS` is now `-D COMPRESS=off`. INVOCATION CHANGE.
```

What earns one, and what does not:
[REFERENCE.md](REFERENCE.md#what-earns-a-port-marker).
`EXAMPLE-bigtest.lua` beside this file is a full worked port.

**`PORT:` notes are the only comments a port ships by default.** The reasoning
that belongs beside the *compiler's* source does not belong beside a user's
installer: a comment explaining what `group` is, or why `==` is case-sensitive,
teaches Installua in the margins of a script whose author is about to maintain
it. Carry the original's own comments across, add `PORT:` notes, and stop there.

Before writing the file, ask once whether they want more than that — a
walkthrough version annotating each NSIS construct with what it became is
genuinely useful for a first port and noise for a tenth. Default to lean.

## Traps that produce a *working* installer with wrong behaviour

- **`==` on strings is case-sensitive** — it lowers to `StrCmpS`, not the
  `StrCmp` your fingers know. Case-insensitive is
  `string.lower(a) == string.lower(b)`, which peepholes back to a bare `StrCmp`.
  Every ported `StrCmp` needs this decision made deliberately.
- **`$` in a literal is data.** `detailPrint("costs $5")` prints `costs $5`.
  Interpolation is concatenation: `INSTDIR .. "/app.exe"`.
- **`"C:\Program Files"` is an error** — `\` is Lua's escape. Write `C:/…`,
  `[[C:\…]]` or `\\`.
- **A top-level `<const>` is emitted as a `!define` of the same name**, so one
  named after a makensis predefine (`NSISDIR`, `__FILE__`, …) collides and the
  build dies in the header. Rename it.

More at [REFERENCE.md](REFERENCE.md#construct-by-construct).

## Checklist

- [ ] Inventory reported before any Lua was written
- [ ] Every command's spelling traced to a `docs/` heading, not memory
- [ ] Every `StrCmp` decided case-sensitive or not, on purpose
- [ ] No `raw` used where a real spelling exists (grep before reaching for it)
- [ ] Unknown plugins declared, not guessed
- [ ] `installua check` clean; `installua build` run if `makensis` exists
- [ ] A `PORT NOTES` block lists every behaviour and invocation change
- [ ] No commentary beyond `PORT:` notes and the original's own comments,
      unless the user asked for the annotated version
- [ ] What you deliberately did not port is listed, with the reason
