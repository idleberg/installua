# Phase 6 — the coverage grind

**Status: the alphabetical grind is done and the first design item after it has landed.
25 → 74 exposed, 167 → 118 todo.**

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
cargo test          # 86 tests
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
`an_exposed_rows_outputs_come_first` refused any `Exposed` row whose outputs were not
leading, with `FileRead` as its one exception — and the comment said a *second* exception
would be a reason to fix the emitter rather than extend the list. Batch 5 found four, and
did. The test is gone; see "Outputs go where the table says" below.

## Batch 4 — S through Z

Fifteen rows, and the `Set*` family is most of them:

| NSIS | Installua |
| --- | --- |
| `SearchPath` | `searchPath(name)` |
| `SetAutoClose`, `SetDetailsView`, `SetDetailsPrint` | `setAutoClose(e)`, `setDetailsView(e)`, `setDetailsPrint(e)` |
| `SetErrors`, `SetErrorLevel` | `setErrors()`, `setErrorLevel(n)` |
| `SetFileAttributes`, `SetRebootFlag`, `SetRegView`, `SetShellVarContext` | `setFileAttributes(p, flags)`, `setRebootFlag(e)`, `setRegView(e)`, `setShellVarContext(e)` |
| `UnRegDLL`, `WriteINIStr`, `WriteRegBin`, `WriteRegExpandStr`, `WriteRegNone` | `unRegDll(p)`, `writeIniStr(…)`, `writeRegBin(…)`, `writeRegExpandStr(…)`, `writeRegNone(…)` |

Six of these close a pair whose other half batch 2 or an earlier phase already wrote:
`setErrors`/`clearErrors`, `setErrorLevel`/`getErrorLevel`, `setRegView`/`getRegView`,
`setShellVarContext`/`getShellVarContext`, `unRegDll`/`regDll`, `writeIniStr`/`readIniStr`.
The setters were spread across four different group reasons; the getters were one row away
in the census the whole time.

`writeRegBin` and `writeRegExpandStr` are names rather than further dispatches of
`writeReg`, for the reason `readRegDword` is: `writeReg` picks its instruction from the
*type* of the value, and all three of these take a string. Nothing in the call could tell
them apart.

**`SetAutoClose` was the third row filed under "addresses a window by handle" that takes
no handle**, after `HideWindow` and `LockWindow`. Three batches, three mis-filings, same
cause: the group reasons were written from the command names.

**Five compile-time settings had the wrong reason.** `SetCompress`,
`SetCompressionLevel`, `SetCompressorDictSize`, `SetDatablockOptimize` and `SetOverwrite`
were *"file surface beyond `file`/`delete`/`fileOpen`: one overlay row each"*, which
promised data entry. They are compile-time and **positional** — they change the `file`
calls that follow rather than executing — so a call inside an `if` would be a lie, and
that is a design question rather than a row.

`WriteRegMultiStr`'s `/REGEDIT5` is required rather than optional, and no argument
supplies it; the row waits on the emitter learning to write an option nothing passes.

### The tier both tiers miss

`SetSilent` was written as a row and reverted. `makensis -WX` assembles `SetSilent silent`
inside a section without a word, and NSIS then ignores it at run time: the instruction is
only meaningful from `.onInit`. Tier 2 sees output that did not change unexpectedly; tier
3 sees output that is valid NSIS. Both are right, and the call is still dead.

This is a third failure mode, after "wrong output" and "invalid output": **legal
everywhere, meaningful in one place**. `tests/overlay.rs` puts every example in a section
by design — the section body is the smallest region containing the behaviour — so the row
cannot carry an honest example until something can say where a call is legal. `SetSilent`
is back in the backlog with that as its reason, and it is the first entry in the backlog
that names a missing *check* rather than a missing design.

## Batch 5 — outputs go where the table says

Four rows, and the emitter changed to make them possible:

| NSIS | Installua |
| --- | --- |
| `FileReadByte`, `FileReadWord` | `f:readByte()`, `f:readWord()` |
| `FileReadUTF16LE` | `f:readUtf16Le([maxlen])` |
| `FileSeek` | `f:seek(offset [, mode])` |

### The concatenation that assembled

No output was ever passed as an argument — `surface()` filters `Dir::Out` out, so
`local key = enumRegKey(HKLM, sub, 0)` was already the shape and twenty rows used it. The
call shape was never the problem. The **emitted** line was: the emitter built
`[dest] ++ inputs` unconditionally, and NSIS does not put the register first.

```
local b = fileReadByte(f)        -- FileReadByte handle $(user_var: output)

emitted     FileReadByte $0 $1   -- $0 read as the handle, $1 written as the output
wanted      FileReadByte $1 $0
```

Two registers either way, so `makensis` takes it without a word — verified, both orders
assemble. At run time it reads from the destination and writes **over the handle**, and
the next read on that handle reads from a byte value. Tier 2 sees a golden that changed
as asked; tier 3 sees valid NSIS. Neither tier can see this, which is why the census
carried an invariant refusing such rows instead.

`FileReadUTF16LE` puts its output in the *middle*, so "leading" and "trailing" were never
the two cases. `place()` in [`lower/expr.rs`](src/lower/expr.rs) now walks `params` in
table order and asks each position what goes there.

### Three things the walk has to decide, not two

**Where** the destination goes was the known one. Reading the syntax lines turned up two
more, and `FileSeek` has both:

```
FileSeek $(user_var: handle) offset [mode] [$(user_var: new position)]
```

**Whether to write a destination at all.** The output is optional, so the two call sites
want different token counts:

```lua
f:seek(0, "END")               -- FileSeek $0 0 "END"
local p = f:seek(0, "END")     -- FileSeek $0 0 "END" $1
```

`returns()` hands back the type unconditionally and the emitter always wrote a
destination, so the statement form used to claim a temporary: correct NSIS, and a register
nobody reads still enters the clobber set and can cost a caller a save.

**What to write for a position the caller skipped.** `mode` is optional and sits *before*
the output, and the obvious emission is rejected:

```
FileSeek $0 0 $1     ->  Error in script -- aborting     ($1 is parsed as the mode)
FileSeek $0 0 SET $1 ->  (assembles)
```

So reaching the output means writing a mode nobody named. `Ann::fill` carries it, `SET` is
what NSIS itself uses when the position is absent, and the walk keeps every skipped
position *pending* rather than absent — written only if something after it turns out to be
written. One rule, and `f:seek(0)` still emits `FileSeek $0 0`.

`FileSeek` is the only command in the table that needs a fill, which is exactly why it did
not turn up until somebody read the line.

### Two tests traded

`an_exposed_rows_outputs_come_first` is gone: it existed only to refuse rows the emitter
could not place. In its place,
[`an_optional_input_before_an_output_can_be_filled`](tests/census.rs) checks the narrower
thing — an optional position before an output must carry a `fill`, because a row that
needs one and lacks it emits a line one token short, and NSIS may well accept it.

[`an_optional_position_is_written_only_when_something_after_it_is`](tests/overlay.rs)
covers the three call sites, since an example is one call site and this differs between
them.

### The method table was a second table

`f:readByte` could not be written at all until `method()` stopped being a hardcoded
`match` on `"close"` and `"write"`. The overlay has named these `f:close`, `f:read` and
`f:write` since Phase 5, so the match was a second vocabulary for one set — and it had no
way to express a method with an *output*, which is what all four of these are. Method
dispatch now reads the table: the receiver is the first parameter, the arity is the
surface minus it, and outputs go through the same `place()`. The "a handle has no `x`"
error lists the methods by reading the table, so a new `f:` row appears in it the day it
is written.

### What this did not unblock

`ExecWait` still has the `Exec` family's reason — one argument that is part path and part
switches, where §5's rewrite cannot apply to half a string — and that ruling is the older
of its two blockers. The `GetDLLVersion` and `GetFileTime` families need the plural-output
work as well. The rest of the non-leading-output commands are behind §13, the `hwnd`
surface, the classic UI or §15.19.

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
- **The `todo` reasons are grouped**, and the grind retired the groups it could: what is
  left in the 118 is section-index binding (§13), the `hwnd` surface, the classic UI,
  pages, script-wide settings and a handful of one-offs. Every one of those is a design
  question rather than data entry, which is what the grind was for.
- **Nothing says where a call is legal.** `SetSilent` is meaningful only in `.onInit`,
  `SetAutoClose` only outside it, and the compiler has no way to state either. Both tiers
  pass a call that is simply dead.
- **Multiple outputs are still one output.** `Instruction::returns` takes the first
  `Dir::Out` parameter, which is every exposed row today. `GetDLLVersion`, `GetFileTime`
  and their `*Local` twins write two, and §15.23 says the count *is* the Lua arity, so the
  plural case is a lowering change rather than a row. Batch 5 placed outputs by position,
  which is half of it; the other half is the multi-`dest` binding path.

### Optionals should be named, not counted

`arity` is a range and the lowerer zips arguments to `surface()` by index, so which
parameter an argument *means* is decided by how many arguments there are. That works for
trailing optionals and fails twice.

**It fails outright on a leading optional.** `ExecShell [flags] verb file [parameters
[showmode]]` puts the optional first, so `execShell("open", url)` zips `"open"` to `flags`
and the URL to `verb`. Every annotation lands one position left of what the author meant.
The emitted text is *accidentally* correct — NSIS re-parses positionally and reads two
tokens as `verb file` — so tier 2 and tier 3 both pass. What is lost is the checking: the
`Kind::Path` annotation belongs to `file` and was applied to `verb`, so §5's `/`-to-`\`
rewrite silently does not happen, and [`stubs.rs`](src/stubs.rs) describes the signature
shifted by one.

**It is already bad on the trailing ones.** `CreateShortcut` is two required positions and
seven optional, `arity` `2..=9`. Setting the comment means passing all nine, four of which
the author does not care about and three of which are enums they would have to look up.
That row is exposed and shipped; the leading-optional gap is the narrow case of a defect
already in the table.

The fix: **required positions stay positional, optional positions become named fields of
one trailing table.**

```lua
execShell("open", url, { showmode = "SW_HIDE" })
execShell("open", url, { invokeIdList = true, parameters = "-q" })
createShortcut(DESKTOP .. "/App.lnk", INSTDIR .. "/app.exe", { comment = "Launch App" })
```

Which positions are named is not a judgement — it is `req: false` in the snapshot, the same
place the arity and the enum members already come from. The overlay adds one thing per
optional: a name, because `-CMDHELP` calls them `showmode` and
`hex_string_like_12848412AB`.

Three consequences worth stating before anybody starts.

1. **The arity check stops being a range.** The positional count must *equal* the required
   count, and an unknown key names the legal ones — `` `execShell` has no option
   `showMode` `` beats `takes 2 to 9, and 4 were given`.
2. **`flags` becomes a boolean.** `/INVOKEIDLIST` is spelled by the compiler, so the field
   is `invokeIdList = true`: a ninth instance of *emitted, never written*.
3. **The table must be a literal**, with constant keys checked at compile time — the rule
   `attributes {}` already carries. A computed key is an error, not a fallback.

Eight rows have optional inputs at all: `Abort`, `CopyFiles`, `CreateShortcut`,
`FileRead`, `GetTempFileName`, `MessageBox`, `RegDLL`, `WriteRegNone`. The predicates'
optional *labels* are `Kind::Label` and excluded from `surface()`, so they are untouched.
`messageBox` is hand-lowered and needs its own pass. Six commands in the table have a
leading optional; the other four — `InstType`, `LangString`, `PageEx`, `SectionGroup` —
are blocked on §13, locale tables and pages first.
