# Phase 6 — the coverage grind

**Status: the preparatory tasks are done and the grind is running. 25 → 55 exposed,
167 → 137 todo, through `R`.**

PLAN describes Phase 6 as *"parallelisable, mechanical … each command is one overlay row
plus its mandatory example pair"*. Before that was true, two things were not: adding a
command took **two** edits in two files, and the example pairs were never handed to
`makensis`. Both are fixed, so the grind is now what PLAN says it is.

Order: **alphabetical** through the `todo` bucket, since nothing about the remaining
commands makes one order cheaper than another and `installua coverage` prints the backlog
in `-CMDHELP` order either way.

| Task | Where | Result |
| --- | --- | --- |
| One table, not two | [`src/builtins.rs`](src/builtins.rs) | the instruction half is gone; `lookup` answers from the census |
| Optional parameters are real | [`src/lower/expr.rs`](src/lower/expr.rs) | `-CMDHELP` brackets became an arity *range* |
| Tier 3 for the example pairs | [`tests/overlay.rs`](tests/overlay.rs) | all 25 examples assemble under `makensis -WX` |
| The fixtures they need | [`tests/fixtures/`](tests/fixtures/) | one real `.ico`, and a README saying when to add more |

```
cargo test          # 83 tests
```

---

## Batch 1 — A through E

Seven rows, and the alphabet is the only thing they have in common:

| NSIS | Installua |
| --- | --- |
| `CopyFiles` | `copyFiles(source, destination [, sizeKb])` |
| `DeleteINISec` / `DeleteINIStr` | `deleteIniSection`, `deleteIniStr` |
| `DeleteRegValue` | `deleteRegValue(root, subkey, entry)` |
| `EnumRegKey` / `EnumRegValue` | `enumRegKey`, `enumRegValue` — both return a string |
| `ExpandEnvStrings` | `expandEnvStrings(s)` |

`INI` is spelled `Ini`, because every other name here is camel case over words and
an acronym is a word; `Sec` became `Section`, because `-CMDHELP` abbreviates and the
surface does not. Those two choices are made once and the rest of the family —
`ReadINIStr`, `WriteINIStr`, `FlushINI` — inherits them when the alphabet reaches it.

**The grind adds `Exposed` rows only.** Six of the alphabetically-earlier `todo`
commands are settings rather than instructions — `AllowSkipFiles`, `DirVar`,
`FileBufSize` — and an `Attribute` row is only half the work: the other half widens
`V1_ATTRIBUTES`, which Phase 0 froze. Widening a frozen surface is a decision, not
data entry, so those stay in the backlog with the rest.

### Four rows the batch could not write, and what they taught

The backlog reasons for these were group summaries. They are now specific, which is
the point of a reason line:

- **`Exec` and `ExecWait`** take *one* argument that is part path and part switches.
  `Kind::Path` turns the `/S` in `setup.exe /S` into `\S`; `Kind::Value` ships the
  forward slashes of `INSTDIR .. "/app.exe"` to a program that will not find it. §5
  gives the surface one rule and a half-path position cannot obey it.
- **`ExecShell` and `ExecShellWait`** are `[flags] verb file [parameters [showmode]]`:
  the optional position is **first**, so `execShell("open", f)` would bind `"open"` to
  `flags`. This is the gap "Still open" predicted — *"it will be a row before it is a
  design"* — arriving as a row, four commands earlier than expected.

## Batch 2 — F through L

Sixteen rows, and eleven of them take no arguments at all:

| NSIS | Installua |
| --- | --- |
| `FlushINI` | `flushIni(path)` |
| `GetErrorLevel`, `GetInstDirError`, `GetRegView`, `GetShellVarContext` | `getErrorLevel()` and friends — one output, no input |
| `GetFullPathName`, `GetKnownFolderPath`, `GetTempFileName` | `getFullPathName(p)`, `getKnownFolderPath(guid)`, `getTempFileName([dir])` |
| `GetWinVer` | `getWinVer("MAJOR")`, typed `nonneg` so `>= 10` is an integer comparison |
| `HideWindow`, `LockWindow` | `hideWindow()`, `lockWindow("on")` |
| `IfAbort`, `IfAltRegView`, `IfRebootFlag`, `IfRtlLanguage`, `IfShellVarContextAll` | five predicates |

**Predicates cost one line each now.** `IfSilent` was the only one when §15.20 was
written and the machinery it needed is all in the join, so five more are five
`predicate(…)` rows: `if rebootFlag() then …` fuses into the branch and `local flag =
rebootFlag()` materialises, from the same bit. The naming rule is drop the `If` and
keep the rest, with one exception — `IfAbort` would be `abort`, which is `Abort`, so
it is `aborted`.

**Two rows were in the wrong bucket, not blocked.** `HideWindow` and `LockWindow` sat
under *"addresses a window by handle; the `hwnd` surface wants nsDialogs designed
first"*, and neither takes a handle: they act on the installer's own window. A group
reason is a guess about every member of the group, and reading the syntax line is
what un-guesses it. Worth remembering for the remaining groups.

### `LogSet` and `LogText`: tier 3, again

Both were written as rows, and tier 3 rejected them before the golden was regenerated:

```
Error: LogSet specified, NSIS_CONFIG_LOG not defined.
```

An **error**, not a warning, so `-WX` is not what refuses it — the stock `makensis`
simply cannot assemble a script containing either. A row would have shipped a call
that works on a machine with a custom build and fails everywhere else, and no amount
of golden-diffing would have said so. They are back in the backlog with that sentence
as their reason, which is a better reason than the group summary they had.

## Batch 3 — M through R

Seven rows, and the letters M through Q contributed none of them:

| NSIS | Installua |
| --- | --- |
| `ReadEnvStr`, `ReadINIStr`, `ReadRegDWORD` | `readEnvStr(n)`, `readIniStr(ini, s, e)`, `readRegDword(root, sub, e)` |
| `ReadMemory` | `readMemory(address, size)` |
| `Reboot`, `Rename`, `RegDLL` | `reboot()`, `rename(a, b)`, `regDll(path [, entry])` |

`readRegDword` is a name rather than a second dispatch of `readReg`, and the asymmetry
with `writeReg` is the reason: `writeReg` picks `WriteRegStr` or `WriteRegDWORD` from
the type of the value it was handed, and a read has no such argument. The name is the
only place the width can be said.

`RegDLL` was filed under §11 with `InitPluginsDir` and does not belong there — it calls
`DllRegisterServer` on a file already on the target and wants nothing from the plugin
directory. Third mis-filed row in three batches; the group reasons were written before
anybody read the syntax lines.

**Twelve `Manifest*` and `PE*` rows had a reason that promised the wrong thing.** They
were *"writes the PE header or the manifest: one overlay row each"*, which reads as data
entry. They are script-wide settings, so a row is the smaller half — the other half is a
home in `attributes {}`, and `manifest` is in `V1_ATTRIBUTES` and unimplemented. Their
reason now says that.

### The type check was an equation, and should have been a bound

`readMemory`'s address is `Ty::int()`. Every integer literal is `nonneg`. The parameter
check compared for **equality**, so `readMemory(0, 4)` was rejected with:

```
error[type-mismatch]: `readMemory` wants a int, and this is a int
```

A message a user cannot act on, for a program that is correct — and it was reachable
from any `Ty::int()` position, which is to say every one the table had not yet written.
Nothing caught it earlier because the existing int-typed rows are all `nonneg`, which
looks like a choice and was an accident.

The lattice already had the answer: `a.join(b) == b` is what "`a` fits where `b` is
wanted" means. The check now asks that, and the other direction still fails — a `nonneg`
position is the one that elides a fixup, so an `int` does not belong in it, and that
case gets a second note, because `int` is what the user's spelling calls both signs.
[`a_narrower_type_fits_a_wider_parameter`](tests/diagnostics.rs) pins both directions.

### An invariant batch 1 needed

`ExecWait command_line [$(user_var: return value)]` has its output **last**, and the
emitter builds `[dest] ++ inputs`. Writing that row would have emitted
`ExecWait $0 "cmd"`: a script that assembles, and does the wrong thing.
[`an_exposed_rows_outputs_come_first`](tests/census.rs) now refuses an `Exposed` row
whose outputs are not leading. `FileRead` is its one exception, and it is one because
nothing generic emits it — `for line in lines(f)` is a hand-written lowering that
places the register itself. A *second* exception is a reason to fix the emitter.

## What the join replaced

`builtins.rs` held a parameter table the lowerer read, written before the `-CMDHELP` join
existed. Phase 5 built the join and left the seed in place, which made every type a thing
written twice — §15.23's exact failure, where a consumer reading two tables sees half an
entry. `builtins::lookup` is now four lines over `table::by_installua`, filtered to
`Class::Exposed`, and [`only_an_exposed_row_is_callable`](tests/census.rs) is the invariant
that keeps it honest: **a row is callable exactly when its bucket says so.** Without the
filter, every `Todo` row becomes a silently working call the moment the join finds it a
shape.

Three fields did not survive the move, and the reasons differ:

- **`returns` was already there.** An output *is* a `Dir::Out` parameter, and its type is
  the annotation the census already requires. `ReadRegStr $(user_var: output) rootkey subkey
  entry` says everything a `returns: Some(Ty::Str)` said.
- **`pure` had no reader at all.** Nothing eliminates dead calls, so it was a field
  documenting a hazard rather than preventing one. The hazard is real — `IfErrors` clears
  the flag it reads, so eliminating an unused call would silently break error handling — and
  it is now a comment on the `IfErrors` row, where whoever writes the elimination pass will
  be standing.
- **`kind: Predicate` became `predicate: bool` on the row.** It cannot be derived from
  `Kind::Label`, which is the obvious guess: `MessageBox` has label positions and is not a
  predicate. The label kind says *the compiler fills this*; the flag says *the call answers
  a question*.

`headers.rs` kept its own `Param`, deliberately. A header macro's argument is a positional
slot in an `!insertmacro` with no direction, optionality or members, so sharing the
instruction struct would mean carrying four fields that can never be anything but their
defaults (§15.27).

## Brackets are optionality, and now they behave like it

The seed table required an exact argument count because every row it had was
all-required. The joined table knows better, and `Instruction::arity` returns a range:
`CreateShortcut` takes two arguments or seven, `Abort` takes none or one, and a `Rep::Many`
tail has no upper bound at all.

This is where the rewire pays for itself rather than merely tidying. `createShortcut`'s five
optional positions — icon file, icon index, start options, keyboard shortcut, description —
were already annotated in the overlay because the census demands an annotation per
parameter. They were simply unreachable. One `exposed(…)` row now delivers the whole
command, which is what makes "one row per command" a true statement about effort.

## Tier 3 found the thing tier 2 cannot

PLAN's tier table marks tier 3 *"always"*, and the example pairs were tier 2 — compiled and
diffed, never assembled. The first run of
[`the_examples_assemble_under_wx`](tests/overlay.rs) failed:

```
Error: no Uninstall section specified, but WriteUninstaller used 1 time(s)
```

The golden had been recording an unbuildable program since the day the `WriteUninstaller`
row was written, and no amount of exact-equality diffing would ever have said so: a golden
answers *"did the output change?"* and only `makensis` answers *"is the output valid
NSIS?"*. Swap `CreateShortcut`'s link and target and the golden passes forever.

The fix is one `uninstaller {}` block in the generated program, and it is worth noting *why*
it belongs there rather than on a row: `WriteUninstaller`'s requirement is a fact about the
whole script, so no per-command example can carry it. That is the general shape of what
tier 3 catches and tier 2 cannot.

## Still open

- **`installua stubs` scans one directory** — carried over from Phase 5, unchanged.
- **The `todo` reasons are grouped**, fifteen sentences over 167 rows — carried over,
  and the grind will retire them a group at a time.
- **`arity` is a range, and nothing yet says which optional position a caller means.**
  Trailing optionals work because they are positional. `ExecShell` is the row that needs
  more: a *leading* optional, where the count of arguments no longer says which position
  each one is. Four commands are waiting on it.
- **The emitter writes destinations first**, which is true of every exposed row and not
  of NSIS — `ExecWait` and `GetTempFileName` put theirs last. A test refuses such a row
  today; placing outputs by position is the fix, and it is the same change as the plural
  case below.
- **Multiple outputs are still one output.** `Instruction::returns` takes the first
  `Dir::Out` parameter, which is every exposed row today. `GetDLLVersion` writes two, and
  §15.23 says the count *is* the Lua arity, so the plural case is a lowering change rather
  than a row.
