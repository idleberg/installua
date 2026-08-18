# Phase 6 — the coverage grind

**Status: the alphabetical grind is done, and every surface gap behind it is closed.
25 → 80 exposed, 14 → 36 attributes, 167 → 84 todo.**

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
| Optional parameters are real | [`src/lower/expr.rs`](src/lower/expr.rs) | `-CMDHELP` brackets are optionality, reached by *name* |
| Tier 3 for the example pairs | [`tests/overlay.rs`](tests/overlay.rs) | all 25 examples assemble under `makensis -WX` |
| The fixtures they need | [`tests/fixtures/`](tests/fixtures/) | one real `.ico`, and a README saying when to add more |

```
cargo test          # 98 tests
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
data entry, so those stay in the backlog with the rest. *(Batch 10 made that decision:
`V1_ATTRIBUTES` is gone and the census answers instead, so all three arrived.)*

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

## Batch 6 — an instruction's outputs are its values

`GetDLLVersion`, `GetFileTime` and their `*Local` twins write **two** registers: a 64-bit
number split across a high and a low half. §3 has no 64-bit type, so the halves stay
halves, and §15.23 already said what that means at the call site — *the output count is
the Lua arity*.

```lua
local high, low = getFileTime(INSTDIR .. "/app.exe")   -- GetFileTime "…" $0 $1
```

Batch 5 taught `place()` **where** the registers go. Nothing taught the lowerer that there
could be more than one of them: `Instruction::returns` reads the first `Dir::Out` and
stops, and `call_multi` — the path a `local a, b = …` takes — knew about `func`s and
namespaces and answered *"binding several values from this call"* for everything else. So
the plural case was never a row. Three rows now exist; one does not, for a reason that is
neither.

### The three questions were already answered, in the wrong place

Two of the emission arms in `call()` differed only in where the destination came from, and
the same pair was copied into `method()` — four arms deciding one thing. They collapse
into [`destinations`](src/lower/expr.rs), which answers it once:

- a **bound** output writes the caller's slot;
- an **unbound required** output writes a temporary, because `GetFileTime` has no
  one-register spelling and Lua adjusting the call to one value does not adjust NSIS;
- an **unbound optional** output is not written at all — batch 5's `fileSeek` rule,
  unchanged.

So `local high = getFileTime(p)` still emits `GetFileTime "…" $0 $1`, and so does
`getFileTime(p)` in statement position. That is the thing an example cannot show, because
an example is one call site, and it is [pinned in `tests/overlay.rs`](tests/overlay.rs)
across all three.

The argument walk and the arity check came out of `call()` at the same time and are now
`positional` and `builtin_arity`, which is what let `call_multi` reuse the whole check
rather than a copy of it. Binding more names than a row writes is a `wrong-arity` error
naming the register count: there is no `nil` to pad with (§3).

### `GetDLLVersionLocal` is a fixture, not a design

The row is written and correct and still `todo`, which has not happened before. The
`*Local` twins read the **build** machine at compile time, so their examples touch this
disk — and `GetDLLVersionLocal` needs a real PE carrying a version resource. Neither the
`.ico` in `tests/fixtures` nor any DLL shipped with NSIS has one; both give *"error
reading version info from …"* under tier 3. `tests/fixtures/README.md` forbids a
placeholder that is not what it claims to be, which is exactly the trap this would be, so
the reason on the row now names the fixture instead of the design.

### §5 is about the target machine

`GetFileTimeLocal` opens its path with `makensis`, not with Windows. Annotated
`Kind::Path` it emitted `assets\icon.ico`, and on macOS a `\` is a filename character:
*"error reading date"*. The same string as `assets/icon.ico` compiles on both hosts. So a
compile-time path is `Kind::Value` — §5's `/`-to-`\` rewrite exists because *Windows*
wants `\` at install time, and this string never reaches Windows. `SearchPath`'s
annotation is the same conclusion from a different direction.

## Batch 7 — a position is named when counting cannot say which one it is

`arity` was a range and the lowerer zipped arguments to `surface()` by index, so which
parameter an argument *meant* was decided by how many arguments there were. That worked
for trailing optionals and failed twice.

**It failed outright on a leading optional.** `ExecShell [flags] verb file [parameters
[showmode]]` puts the optional first, so `execShell("open", url)` zipped `"open"` to
`flags` and the URL to `verb`. Every annotation landed one position left of what the
author meant. The emitted text was *accidentally* correct — NSIS re-parses positionally
and read two tokens as `verb file` — so tier 2 and tier 3 both passed. What was lost was
the checking: the `Kind::Path` annotation belonged to `file` and was applied to `verb`,
and [`stubs.rs`](src/stubs.rs) described the signature shifted by one.

**It was already bad on the trailing ones.** `CreateShortcut` is two required positions
and seven optional. Setting the comment meant passing all nine, four of which the author
does not care about and three of which are enums they would have to look up.

The line is **ambiguity, not optionality**. An argument list is unambiguous exactly when
every optional position is one *trailing* position: an extra argument can then only mean
that position, and nothing is being counted. So `Abort [message]` keeps the spelling it
had, and the rows where the meaning of the fourth argument depends on whether the third
was given take **named fields of one trailing table** instead.

```lua
abort("stopped")                      -- one trailing optional: still an argument
regDll(p, "DllInstall")               -- likewise
execShell("open", url, { showMode = "SW_HIDE" })
execShell("open", url, { invokeIdList = true, parameters = "-q" })
createShortcut(DESKTOP .. "/App.lnk", INSTDIR .. "/app.exe", { comment = "Launch App" })
```

`Instruction::tail_optional` is where the two shapes are told apart, and it answers from
the snapshot alone. Eight of the nine `Exposed` rows with an optional input are the
unambiguous shape and did not change at all; the two that are named are the two the work
was for.

Which positions are named is still not a judgement — it follows from `req: false` and the
order in the snapshot, the same place the arity and the enum members come from. The
overlay adds one thing per optional: a *name*, because `-CMDHELP` calls them `showmode`
and `hex_string_like_12848412AB`. The name is written on every optional whether or not the
row ends up naming it, because the stub and the error message call the position something
either way.

The three consequences predicted before anybody started, and how each landed:

1. **The arity check stopped being an open range.** It is a point for a named row, `n` or
   `n + 1` for an unambiguous trailing optional, and unbounded only for a real repeated
   tail (`File a b c`). An unknown key names the legal ones — `` `execShell` has no option
   `showmode` `` beats *"takes 2 to 5 arguments"*, and that is the whole gain restated as
   an error message.
2. **`flags` became a boolean.** `/INVOKEIDLIST` is spelled by the compiler, so the field
   is `invokeIdList = true`: a ninth instance of *emitted, never written*. A `toggle` is
   also the one optional that never needs a `fill` — NSIS tells a `/FLAG` from the next
   argument lexically rather than by counting.
3. **The table is a literal**, with constant keys checked at compile time, the rule
   `attributes {}` already carries.

Two rows came out of `todo` with it: `ExecShell` and `ExecShellWait`. `ExecShell`'s `file`
is deliberately `Kind::Value` — §5's `/`-to-`\` rewrite is about a Windows *file* path,
and that position is a shell target that may be a URL. Win32 accepts `/` as a separator,
so `INSTDIR .. "/readme.txt"` still opens; `https://example.invalid` put through §5 would
not.

### Naming an option means writing the ones before it

`fill` existed for exactly one row and now carries the whole scheme. NSIS still counts
arguments, so `createShortcut(link, target, { comment = … })` has to write the five
positions the author declined:

```
CreateShortcut "$DESKTOP\App.lnk" "$INSTDIR\app.exe" "" "" 0 SW_SHOWNORMAL "" "Launch App"
```

The census rule that used to say *"an optional input before an **output** needs a fill"*
now says *before **anything emitted***, which is what makes the line above derivable from
the table rather than from this paragraph.

### `-CMDHELP` has a typo, and the parser has to know

`CreateShortcut … [icon_file [icon index [showmode …` — and `icon index` is a *single*
argument missing its underscore. NSIS's own source settles it rather than leaving it to
inference:

```cpp
// Source/script.cpp
ent.offsets[3]=add_string(line.gettoken_str(4));                       // icon_file
ent.offsets[4]=(line.gettoken_int(5,&s) << CS_II_SHIFT) & CS_II_MASK;  // icon index
...
ERROR_MSG(_T("CreateShortcut: cannot interpret icon index\n"));
```

Token 5 is read once, and the error message spells the concept with the space too, so the
missing underscore is consistent within the file. `Source/tokens.cpp` agrees on the count:
`{TOK_CREATESHORTCUT, …, 2, 7, …}` is nine tokens, one of which is the `/NoWorkingDir`
flag `eattoken` removes. **Eight arguments.**

The snapshot records what `makensis` *prints*, which is the point of it, so the correction
cannot live there. It is judgement and it lives in the overlay, as `Kind::Fused` — a
position that is not one. Nine emitted arguments where NSIS takes eight assembles fine and
puts the description in the keyboard shortcut.

`Fused` joins `Kind::Label` as the second kind excluded from `surface()`, and the two are
excluded for opposite reasons: a label is an argument to NSIS and not to Installua, and a
fused half is an argument to neither.

### What it cost

Nothing that was already written. The first cut of this applied the table *uniformly*,
which turned `abort("stopped")` into `abort { message = "stopped" }` in five example
programs and `f:seek(0, "END")` into `f:seek(0, { mode = "END" })` — and neither row has
an ambiguity to remove, so the uniformity was the whole cost and bought nothing. Scoping
the rule to the rows that need it left every existing call site alone.

The objection to scoping it was that adding a second optional to such a row would rewrite
its surface. It would — as a *compile error* on every call site, from a snapshot refresh
the census already gates. That is the loud kind of change, not the silent kind, and it is
not worth an API tax on `abort`.

`messageBox` is hand-lowered and already took a table (§15.18); it is untouched. Six
commands in the table have a leading optional; the four beyond `ExecShell` and
`ExecShellWait` — `InstType`, `LangString`, `PageEx`, `SectionGroup` — are blocked on
§13, locale tables and pages first.

---

## Batch 8 — the flags, which were never positions

`-CMDHELP` prints two kinds of thing and the compiler only read one. The parser has
recorded `Opt { nsis, value, after }` since Phase 5 — thirty-one rows carry at least one —
and `join()` copied them into `Instruction` where nothing looked at them. `Delete
[/REBOOTOK] filespec` was a one-argument command with a flag nobody could write.

A flag is **not a position**, which is why counting could never have reached it: its place
in the line is fixed and arbitrary — leading for `GetDLLVersion`, medial for
`SetCtlColors`, trailing for `SendMessage` — and it is the same place whatever the caller
writes. So it goes in the table batch 7 built, next to the named positions, and the caller
never learns which half a name came from:

```lua
delete(INSTDIR .. "/old.txt", { rebootOk = true })
rmDir(INSTDIR, { recursive = true, rebootOk = true })
copyFiles(src, dst, 100, { silent = true })      -- both halves at once
createShortcut(link, target, { comment = "Launch App", noWorkingDir = true })
```

`copyFiles` is the row that shows they really are two halves: `sizeInKb` is an unambiguous
trailing optional and stays a *counted argument*, and `silent` never was one.

The overlay's judgement is one `Offer` per flag, positional against the snapshot's list
the way `Ann` is against its parameter list — and the census checks the lengths agree on
every `Exposed` row, for the same reason it checks the annotations: a list one short leaves
the *last* flag unoffered and nothing goes wrong loudly.

| `Offer` | Means | Rows |
| --- | --- | --- |
| `Named` | a `boolean` field of the options table | 13 flags across 9 rows |
| `Always` | written on every call | `WriteRegMultiStr`'s `/REGEDIT5` |
| `Unoffered` | not reachable, with the reason | `File`'s `/x`, `MessageBox`'s `/SD` |

**`Always` is a flag with no decision in it.** `WriteRegMultiStr` without `/REGEDIT5` is
an error rather than a different instruction, and there is no second form to choose
between, so there is nothing to ask the caller. That row was the `todo` bucket's own
statement of this gap — *"its `/REGEDIT5` is required rather than optional, and nothing
emits an option that no argument supplies"* — and it is the row that comes out with the
batch.

**`Unoffered` carries its reason** for the argument `Class::Todo` already makes: a flag
that says "not yet" without saying why is indistinguishable from one nobody has read. Both
of the two are the same shape — `/x filespec` and `/SD IDOK` take a **value**, and a
valued flag has no spelling in this scheme yet. `/x` is worse than the other, since it also
repeats, so its field would have to hold a list of exclusions.

### `after` is a place in the line, not an argument index

`GetFullPathName [/SHORT] $(user_var: result) path_or_file` puts the flag in front of the
**output register**:

```
GetFullPathName /SHORT $0 $INSTDIR
```

No count of the caller's arguments could have produced that, which is the second time this
phase that the emitted order and the surface order have turned out to be different lists
(batch 5 was the first). `place` walks the parameters and writes the flags due before each
one, so the rule is read off `Opt::after` and stated nowhere else. A flag flushes anything
`fill` left pending, because an emitted token is an emitted token.

### What it did not touch

`File`'s three boolean flags are named — `nonFatal`, `keepAttributes`, `recursive` — and
its `/oname=` alternation is still a `conflicts` set nothing reads. `MessageBox` is
hand-lowered and its `/SD` belongs with §15.18's table rather than beside it. The
sixteen rows carrying flags that are still `todo` need their command classified first;
the flag is the easy half.

---

## Batch 9 — a flag that carries a value

Batch 8 left two flags on `Exposed` rows unreachable, both for the same stated reason:
`File`'s `/x filespec` and `MessageBox`'s `/SD IDOK` take a **value**, and every flag the
surface could write was a `bool`. They are the batch, and they turn out to want different
things:

```lua
file("assets/icon.ico", { recursive = true, exclude = { "build/*.tmp", "*.log" } })
--> File /r /x "build\*.tmp" /x "*.log" "assets\icon.ico"

messageBox { text = "Restart?", buttons = "YESNO", silentAnswer = "NO" }
--> MessageBox MB_YESNO "Restart?" /SD IDNO
```

**`/x` repeats, so its field is a list and the emitter writes the flag once per element.**
That is not a convenience — it is the only shape in which a caller can say *two*
exclusions, since a field holds one value and the flag is what repeats. Each element is a
`Kind::Path`, so §5 applies to an exclusion as much as to the filespec it excludes, and an
empty list writes nothing the way `false` does. `Offer::List { name, kind }` is the whole
addition, and `Written::flags` went from `Vec<bool>` to a list of occurrences per flag:
empty is not written, one entry carrying nothing is `/REBOOTOK`, and a value per element
is `/x`.

**`/SD` is not in the generic table at all**, and the reason is a check rather than a
shape: the answer has to be one the buttons beside it can give. `silentAnswer = "YES"`
under `buttons = "OKCANCEL"` is a silent install taking a branch nobody wrote, and NSIS
assembles it without complaint. Only §15.18's hand-shaped lowering can see both fields at
once, so `/SD` became a fourth field of `messageBox`'s own table and the table records
that with `Offer::Handled` — a name that is legal in the row's options, written by
someone other than `Instruction::flags`. The answer is spelled `"NO"`, the way
`messageBox` *returns* it, and the compiler writes the `ID`.

**There is no `unoffered` helper in the overlay any more.** Every flag on an `Exposed` row
is now reachable, so the only `Offer::Unoffered` left is the one `join()` hands a row that
has said nothing at all — which is the census's business rather than a judgement anyone
writes. The variant kept its reason string for exactly that one use.

| `Offer` | Means | Rows |
| --- | --- | --- |
| `Named` | a `boolean` field of the options table | 13 flags across 9 rows |
| `List` | a `string[]` field, one flag per element | `File`'s `/x` |
| `Always` | written on every call | `WriteRegMultiStr`'s `/REGEDIT5` |
| `Handled` | a hand-shaped row writes it | `MessageBox`'s `/SD` |
| `Unoffered` | the join's default for an unjudged flag | no `Exposed` row |

### What it did not touch

The five other valued flags in the snapshot — `ReserveFile`'s `/x`, `SendMessage`'s
`/TIMEOUT`, `SetBrandingImage`'s `/IMGID`, `SetFont`'s and `VIAddVersionKey`'s `/LANG` —
are all on `todo` or `attribute` rows, so none of them has a command to hang off yet. Four
of the five take **one** value and do not repeat, which `Offer::List` cannot say; that is a
sibling variant of about ten lines, and writing it now would ship an untested path. It
lands with the first row that needs it.

## Batch 10 — `attributes {}` reads the table

**14 → 23 attributes, 112 → 103 todo, and the second "one table, not two" is done.**

`attributes {}` was the last place in the compiler where adding something took three edits
in two files: an `attribute(…)` row in the overlay, a name in a frozen `V1_ATTRIBUTES`
list, and a match arm in [`src/lower/mod.rs`](src/lower/mod.rs) spelling out the shape.
That is the same bug batch 1 fixed for instructions, one level up — and it had the same
symptom, a hardcoded `boolean()` list in [`src/stubs.rs`](src/stubs.rs) that was a *fourth*
copy of which attributes are `bool`.

The fix is `Class::Attribute(Setting)`. The class no longer says only "this is a block
field"; it says what the field holds:

| `Setting` | Written | NSIS |
| --- | --- | --- |
| `Str { path }` | `name = "MyApp"` | `Name "MyApp"`, `/`→`\` when `path` |
| `Bool { on, off }` | `crcCheck = true` | `CRCCheck on` |
| `Enum` | `silentInstall = "silent"` | `SilentInstall silent` |
| `Int` | `fileBufSize = 8` | `FileBufSize 8` |
| `Handled(ty)` | `unicode = true` | nothing — the emitter reads a module field |

**The enum members are not on the row.** `-CMDHELP` prints `ShowInstDetails
(hide|show|nevershow)` and the snapshot already records those three words, so `Setting::Enum`
names the *shape* and the generated half names the members. That deleted both hardcoded
lists in the lowering — `["zlib", "bzip2", "lzma"]` and `["none", "user", "highest",
"admin"]` — which had been transcriptions of a file sitting in the same repository.

Nine settings landed on that mechanism, and each is one line:

| NSIS | Installua |
| --- | --- |
| `SilentInstall`, `SilentUnInstall` | `silentInstall = "silent"`, `silentUninstall` |
| `ShowInstDetails`, `ShowUninstDetails` | `showInstDetails = "nevershow"` |
| `AllowRootDirInstall`, `AllowSkipFiles` | two `bool`s, and NSIS spells them with different words |
| `FileBufSize` | `fileBufSize = 8`, the only `Int` |
| `CPU` | `cpu = "amd64"` |

### The example is the row

`Exposed` rows carry a hand-written example because a call site has arguments nobody can
guess. An attribute does not: `Setting` says what the field holds and the snapshot says
which keywords an enum takes, so
[`attribute_program()`](tests/overlay.rs) *derives* a value for every `Attribute` row and
gets the same two tiers the examples get — a golden at
[`tests/golden/overlay-attributes.nsi`](tests/golden/overlay-attributes.nsi) and
`makensis -WX` over it.

That paid on its first run, which is the point of writing it before trusting the rows:

```
Error: command DirVerify not valid outside PageEx
```

`DirVerify` is not a script-wide setting at all — it is only legal inside `PageEx`, which
Installua has no shape for. It went back to `todo` with that as its reason, which is a
better reason than the group summary it arrived with. Third row in this phase whose group
reason was a guess, and the first one a *test* un-guessed rather than a person reading a
syntax line.

`LicenseData` also opens its file at compile time, so `tests/fixtures/assets/license.txt`
is a real file for the same reason `icon.ico` is.

### The overlap that was not a mistake

`caption`, `icon`, `installDir` and `license` are `Attribute` rows *and* `installer {}`
fields, and the first cut of this batch rejected them from `attributes {}` — the stub test
caught it, because the generated `installua.Attributes` class offers every `Attribute` row.
They are script-wide NSIS commands that `installer {}` also accepts, so they belong to
both blocks. Only `pages` and `text` are installer-only, and they are the two names that
guard now rejects.

### What it did not touch

The `Manifest*` family (8 rows) and `PE*` (4) are still `todo`, and their reason is now
accurate rather than inherited: `manifest` wants a nested table the way `versionInfo` has
one, and every `Manifest*` setting is `notset|true|false` — a **tri-state**, where `nil`
means "say nothing", which none of the five `Setting`s can spell. `PEAddResource` repeats
and takes four arguments. `DirVar` and `InstallDirRegKey` are one and three arguments
respectively, against a `Setting` model that assumes one value per line.

---

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
all-required. The joined table knows better: a bracketed position is genuine optionality,
and batch 7 made it reachable *by name* rather than by counting.

This is where the rewire pays for itself rather than merely tidying. `createShortcut`'s six
optional positions — parameters, icon file, icon index, start options, keyboard shortcut,
description — were already annotated in the overlay because the census demands an
annotation per parameter. They were simply unreachable. One `exposed(…)` row now delivers
the whole command, which is what makes "one row per command" a true statement about
effort.

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

## Batch 11 — the manifest, and a tri-state that was never one

**23 → 29 attributes, 103 → 97 todo, no new machinery.**

Twelve `Manifest*` and `PE*` rows carried the reason *"a script-wide setting, not an
instruction: it needs a home in `attributes {}` before it needs a row"*. Batch 10 built
that home, so this batch is what those twelve turned out to cost once it existed: seven
are one overlay line each, four want a `Setting` shape that does not exist yet, and one
is a list.

| NSIS | Installua |
| --- | --- |
| `ManifestDPIAware`, `ManifestLongPathAware` | `manifestDpiAware = true` → `true|false` |
| `ManifestDisableWindowFiltering`, `ManifestGdiScaling` | same, but the off-word is `notset` |
| `ManifestDPIAwareness` | `"PerMonitorV2,system"` — NSIS parses the commas, this does not |
| `ManifestMaxVersionTested`, `PESubsysVer` | `"10.0.19041.0"`, `"5.1"` |

### The tri-state dissolved on contact

"Still open" has said since batch 10 that `notset|true|false` needs a spelling `Setting`
does not have, because `notset` means *emit no line* and Lua's `nil` is not a value a
table field can hold. Writing the rows dissolved it: **absence is not a value and does not
need to be one.** A field the user did not write emits nothing already, so `notset` is
never something the compiler has to *store* — it only has to be a word, and only for the
two settings whose syntax line has no `false` in it:

```
ManifestGdiScaling notset|true
```

`makensis` accepts the literal `notset` there, so `Bool { on: "true", off: "notset" }`
spells that field exactly, and eight rows needed nothing built. The item was a design
question invented from a syntax line rather than found in one — worth remembering the
next time a "Still open" bullet describes a problem nobody has hit yet.

### What stayed behind, with better reasons

`PEAddResource` (four arguments, repeats), `PERemoveResource` (three), `PEDllCharacteristics`
(two) and `ManifestAppendCustomString` (two, repeats) are the whole of the multi-argument
setting problem, and `ManifestSupportedOS` is the whole of the repeating one. Their reasons
now name that instead of pointing at a block that exists.

`Target` was in the same group and is not blocked on anything: `Target x86-unicode` is
`cpu` and `unicode` hyphenated together and both are rows already. A third spelling would
be a second way to set `unicode`, which is not a line but a field the emitter reads first.

### `Setting::Str` says less than the syntax line does

`PESubsysVer` takes `major.minor` and `ManifestMaxVersionTested` takes `maj.min.bld.rev`.
Both are `Str`, and `Str` means *a string* — so `peSubsysVer = "hello"` compiles here and
is rejected by `makensis`. That is a real narrowing the table cannot state, and the two
shaped values live in [`attribute_program()`](tests/overlay.rs) rather than on the rows,
because a row is not an example. The derived-example test found both the first time it
ran, which is twice now that it has caught a row nobody could have checked by reading.

## Batch 12 — a setting that is more than one word

**29 → 31 attributes, 97 → 95 todo.**

`Setting` assumed one value per line, which was true of every row batches 10 and 11
landed and false of five they left behind. It is now `Setting::Table`, a list of
[`Part`](src/table/mod.rs)s that each name a Lua key and stand against one position of the
snapshot:

```lua
attributes {
  installDirRegKey = { root = HKLM, key = "Software/Example/App", name = "InstallDir" },
  peDllCharacteristics = { add = 64, remove = 0 },
}
```
```
InstallDirRegKey HKLM "Software\Example\App" "InstallDir"
PEDllCharacteristics 64 0
```

A **table** rather than a list, because the keys are the only thing that tells three
strings apart, and because §12 already says a Lua table has no order — a language that
cannot promise order cannot be asked to supply meaning by counting.

The part carries only its key and its [`Setting`]. Whether the position is required and
which keywords it accepts are the snapshot's, exactly as they are for a top-level enum, so
a row cannot claim a set of registry roots that `-CMDHELP` does not print.

### One function, called twice

The four one-value arms moved out of `setting()` into `value_arg()`, which
[`table_setting()`](src/lower/mod.rs) calls once per part. That is the point of the shape
rather than a tidy-up: a `bool` written on its own line and a `bool` written as a part are
now the same six lines of code, so they cannot come to disagree about what `true` means or
whether a path gets §5's slash conversion. `installDirRegKey.key` is a `PATH` for exactly
that reason — `readRegStr`'s subkey already is one, and a registry path written with `/`
here has to arrive with `\` no matter which row reached it.

### A registry root is `HKLM`, not `"HKLM"`

`readRegStr(HKLM, …)` spells a root as a bare constant, so `root = "HKLM"` would have been
a second spelling of one idea. `Setting::Enum` now accepts a bare name when it is one of
the **sigil-less** constants, which is precisely the registry roots — there is no constant
named `lzma`, so `compressor = lzma` still fails and nothing else moved. The derived
example test derives the spelling from the same two tables, so both paths are compiled
rather than one.

### `FileErrorText` is not blocked on this, and never was

It was on the multi-argument list, and it is the one row `Setting::Table` cannot reach.
`-CMDHELP` prints:

```
FileErrorText [text (can contain $0)] [text without ignore (can contain $0)]
```

and the snapshot parser reads the parenthesised prose as positions — `text`, `can`,
`contain`, `$0`, and again — so the row has **four** optional positions where NSIS has two.
Parts stand against positions, so its parts would stand against sentence fragments. The fix
is in the parser and its reason now says so. Nothing else in the table has prose in its
parameter list; this is one row and one line of `-CMDHELP`.

The other three stayed for a reason that is the same reason three times: `PEAddResource`,
`PERemoveResource` and `ManifestAppendCustomString` all **repeat** to do their job, and a
table field is written once. That is a different gap from the one this batch closed.

### The error paths have their own test

The golden shows a table field written right, and both tiers only ever compile programs
that pass — so a missing part silently emitting a short line, which `makensis` would take
and misread, would have shown up nowhere. `a_table_setting_wants_every_part` writes it
three ways wrong: not a table, a part that is not one, and a part left out.

## Batch 13 — a setting that is written more than once

**31 → 34 attributes, 95 → 92 todo.**

`Setting::Table` gave a field more than one word. It still gave it one *line*, and three
rows are written once per resource or once per string:

```lua
attributes {
  peAddResource = {
    { file = "banner.bmp", restype = "#2", resname = "#200" },
    { file = "logo.ico",   restype = "#3", resname = "#201", reslang = "1033" },
  },
  manifestAppendCustomString = { { path = "/assembly", string = "<x/>" } },
}
```

[`Setting::Each`](src/table/mod.rs) is the field holding a Lua **array**, one whole line
per element. Its elements are *positional* where the parts of a `Table` are named, and for
the mirrored reason: three strings on one line can only be told apart by a key, and two
resources can only be told apart by their order — which is the one order a Lua table keeps
(§12), and the order NSIS adds them in.

A line that repeats is not a position that repeats. `Rep::Many` is the snapshot's word for
*many values on one line*; that a **line** repeats is said only in NSIS's prose, so it is a
variant in the overlay rather than a bit read off the skeleton.

### The elements go through the same function as everything else

`setting()` now dispatches `Each` and hands every element to `setting_line()`, which is the
body it used to have. So the third call in the chain — plain setting, part of a table,
element of a list — is still [`value_arg()`](src/lower/mod.rs), and a `bool` cannot mean
one thing on a line and another inside a list.

### An optional part, at last

`PEAddResource`'s `reslang` is the first optional position a `Part` has stood against, and
the rule is the snapshot's rather than the row's: a part may be left out when its position
is optional *and* nothing after it was written. A gap before something you wrote is the
same error as a part nobody wrote, because NSIS counts arguments and a short line means a
different thing rather than less. The message now ends by naming which parts may be left
out, and the stub types them `reslang?: string`.

### Three narrowings that belong in the example, not the row

`Setting::Str` says "a string" and NSIS means less: `restype` and `resname` are `#N` or a
name it knows, `PEAddResource` opens its file at compile time, `PERemoveResource` names a
resource that must already be in the stub, and `ManifestAppendCustomString`'s path is an
XPath rooted at `/`. All four were found by handing derived values to `makensis` and being
refused. They live in `REAL` in [tests/overlay.rs](tests/overlay.rs) for the reason
`peSubsysVer`'s `"5.1"` does: a row is not an example.

The XPath is the one worth remembering, because the wrong answer type-checks: `path` reads
like a path and is emphatically not one. As `PATH` it would collect §5's `/`-to-`\`
conversion and NSIS would reject the line the compiler built.

### `ManifestSupportedOS` was on this list and is not blocked on it

It takes a repeated *keyword* — `Rep::Many`, one line — and that half is real. What stops
it is the same class of bug `FileErrorText` has: `-CMDHELP` prints
`none|all|…|{GUID} [...]`, and the parser records `GUID` as a member. It is a placeholder,
and `makensis` rejects it as a keyword, so the row would complete to a word that cannot
work. `PERemoveResource` has the same wart in `reslang|ALL`, which is why its language part
is a `STR` and gets no alias: offering `reslang` is worse than offering nothing.

*(Batch 14 fixed the parser and landed the row. `PERemoveResource`'s half is unmarked
notation and stayed as it is — see "Still open".)*

### What the stub cannot say, the compiler does

`{ path: string, string: string }[]` is checked by lua-language-server — `file = 1` is
caught there. Writing the *parts of one entry* where the entries go is not: LuaLS accepts
`peAddResource = { file = … }` silently. `a_repeating_setting_wants_a_list` is where that
is refused, along with a non-list and an element that is not a table;
`a_repeating_setting_repeats` is the only place that two entries become two lines, in
order, is checked at all.

## Batch 14 — notation that is not a parameter list

**34 → 36 attributes, 92 → 90 todo.**

Every earlier batch trusted the snapshot's parameter count. Two rows were blocked because
it was wrong, and both for the same reason: `-CMDHELP` writes prose, names and choices with
the same punctuation it writes positions with, and the parser was reading all of it as
positions. Three rules in [`cmdhelp.rs`](src/table/cmdhelp.rs) now separate them, and
`the_notation_is_not_the_parameter_list` pins each one to the command that decides it.

### A space is both a separator and a letter

`MiscButtonText [back button text]` is one optional caption. `CreateFont … [height weight
/ITALIC /UNDERLINE /STRIKE]` is five positions. Nothing in the notation distinguishes them,
so the parser asks three questions of the bracket, and merges only when all three agree:

| | one name | several positions |
| --- | --- | --- |
| anything but words? | `[back button text]` | `[height weight /ITALIC …]` |
| a nested optional? | `[space required text]` | `[icon index [showmode …]]` |
| an underscore? | `[text without ignore]` | `[return_check label_to_goto_if_equal]` |

The third is the load-bearing one: **every** multi-word parameter NSIS names joins its
words with an underscore — `top_color`, `accept_text`, `pre_function` — so words that use
none are English. Nine groups in 3.12 merge, all of them captions.

`CreateShortcut`'s `[icon index …]` deliberately does not, and keeps its
[`Kind::Fused`](src/table/mod.rs) correction. It opens a further optional, which a name does
not do, so the rule declines and the overlay goes on saying by hand what it always said.
The snapshot keeps printing what `makensis` prints; the judgement stays where judgement
lives.

### A parenthesis is both a choice and a sentence

`FileErrorText [text (can contain $0)] [text without ignore (can contain $0)]` produced
**eight** positions, four of them the words `can`, `contain` and `$0`. A parenthesised
group that follows a *word* annotates it; one that follows a group is a second choice —
`(top|left|bottom|right) (height|width)` — and one that opens the fragment is the first.
Only the first kind is commentary, and only `FileErrorText` has any.

The two survivors are both plain `STR`. `$0` in them is NSIS's own runtime substitution
rather than a §5 sigil, and a path here would be wrong twice over: these are sentences
shown to a user.

### A brace is both a wrapper and a placeholder

`ManifestSupportedOS none|all|…|Win10|{GUID}` recorded eight members, the last of them the
literal word `GUID` — which `makensis` rejects, so the row could not be written at all. But
dropping it is not enough either, because a **real** GUID is legal there and a closed check
would reject it. `Shape` gained one bit:

```
members: &["none", "all", "WinVista", "Win7", "Win8", "Win8.1", "Win10"], open: true
```

An open enum offers the seven and enforces none, in the compiler and in the stub alike —
the alias ends `---| string`, so LuaLS still completes the names and stops marking a GUID
wrong. `compressor` lists three and still means three;
`an_open_enum_takes_what_it_does_not_list` checks both halves, because an open check
everywhere would be no check at all.

`flag={smooth|colored}` proves the two brace uses apart: those wrap the whole list and mean
nothing, so the outer pair is stripped before the members are read.

### A position that repeats, at last

With `{GUID}` out of the way `ManifestSupportedOS` needed only its `[...]`, and it is a
**row of two words**:

```rust
attribute("ManifestSupportedOS", "manifestSupportedOS", Setting::Enum),
```

Everything else is the snapshot's. That the position repeats is `Rep::Many`, so no row says
it and none can be wrong about it — the same division batch 12 drew for `req` and `members`.

`manifestSupportedOS = { "Win7", "Win10" }` and `manifestAppendCustomString = { … }` are
both a Lua list, and the difference between them is invisible in Lua and the whole of the
difference in NSIS: one line here, one line **per element** there. `Setting::Each` is the
row's judgement because only NSIS's prose says a line repeats; `Rep::Many` is read off
`-CMDHELP` because `[...]` says so out loud. `a_repeating_position_fills_one_line` is the
only place the distinction is checked, since the golden writes one element of either.

A *call* spells the same repetition with varargs — `file(a, b, c)` — and a field cannot,
because a field takes one value. The surface forces that asymmetry rather than choosing it.

### The six rows this did not land

`MiscButtonText`, `DetailsButtonText`, `UninstallButtonText`, `InstallButtonText`,
`SpaceTexts` and `CompletedText` all parse correctly now and are all still `todo`. Their
blocker was never the notation: a classic-UI caption needs a home in `installer {}` or
`page {}`, and that home does not exist yet. The parser fix moved them from *unparseable*
to *undesigned*, which is worth having and is not a row.

## Batch 15 — the components tree, and a name for a number

**90 → 84 todo.** Six rows, none of them `Exposed`: five are language constructs and one is
a second name for another command. The bucket they came from was the biggest one left, and
this is its compile-time half.

### The binding §13 asked for

`SectionIn` takes a number. So does `SectionSetText`, and `InstTypeSetText`, and the other
dozen — that is what "addresses a section by index" meant, and it is why eighteen rows sat
in one `todo` reason since batch 1. The numbers are *positions in a declaration list*, so
they are a name binding wearing a number's clothes, and the question was never how to pass
an integer.

The answer at this end is that the list is written once and the number is never written at
all:

```lua
installer {
  installTypes = { "Full", "Minimal" },

  section("Core", { installTypes = { "Full", "Minimal" }, required = true, size = 120 }, function()
    detailPrint("core")
  end),

  group("Tools", { expanded = true }, {
    section("Profiler", { installTypes = { "Full" } }, function() … end),
    section("Debugger", { installTypes = { "Full" }, optional = true, size = 4096 }, function() … end),
  }),
}
```

> **Superseded spelling.** The binding above is unchanged, but the call shape is not: a
> section or group that carries options now takes §15.23's table form —
> `section { "Core", required = true, body = … }` — and the middle-table form shown here
> was deleted. See `PHASE-6-SECTIONS.md` ruling 5.

```
InstType "Full"
InstType "Minimal"

Section "Core"
  SectionIn 1 2 RO
  AddSize 120
  DetailPrint "core"
SectionEnd

SectionGroup /e "Tools"
  Section "Profiler"
    SectionIn 1
  …
```

`an_install_type_is_named_and_never_numbered` is the test that says what this bought:
insert `"Custom"` at the front of the block's list and `SectionIn 2` becomes `SectionIn 3`
without a character of the sections changing. A raw `.nsi` renumbers by hand, and the way
that fails is silent — the wrong sections are ticked, and nothing anywhere is a syntax
error.

The numbering lives in exactly one function, `section_in`, and reaches nothing downstream:
`ir::Section` holds positions, because positions are what NSIS reads.

### Two indexings for one object, and neither is the other

NSIS numbers install types **one**-based for `SectionIn` — `SectionIn 0 out of range 1..32`
— and **zero**-based for the `[index_output]` define that `InstType "Full" it_full` writes.
The same object, counted two ways, in two commands that sit four lines apart. The compiler
that owns both numbers is the only party that can be trusted to keep them straight, which
is a second argument for the surface having none.

### Four things a section is besides a name and a body

| option | NSIS | what it means |
| --- | --- | --- |
| `optional` | `Section /o` | the box starts unticked |
| `required` | `SectionIn RO` | there is no box |
| `installTypes` | `SectionIn 1 2` | which presets tick it |
| `size` | `AddSize 120` | kilobytes beyond the files |

The first two look like opposites and are not, which is the whole reason both exist: one is
about the box's initial state and the other about whether there is a box. Written together
they say a section starts unticked and can never be unticked, NSIS resolves it silently in
favour of `RO`, and `a_section_is_not_both_optional_and_required` says which of the two
words the author wrote is doing nothing.

`size = 0` is emitted rather than skipped. Leaving the option out says nobody measured;
writing zero says somebody did.

### A group is not a scope

```lua
group("Tools", { expanded = true }, { section(…), section(…) })
```

The sections are a **list argument** and not the positional entries of a block, because
nothing runs in a group: it has no body, and the only thing between `SectionGroup` and
`SectionGroupEnd` is other sections. A block would promise a scope that is not there.

One level. NSIS accepts nesting and MUI2's tree draws it, but a nested heading means
nothing to anything except the drawing, so the second level is a `todo` at the point of
use rather than a silent acceptance. An empty group is an error: two NSIS lines that draw
nothing are not worth emitting quietly.

### `SectionInstType`, which is `SectionIn`

Undocumented, absent from the NSIS docs, and byte-for-byte the same command — same
arguments, same "not valid outside Section" error, same effect. It is the batch's only
`rejected` row, and the reason text names `SectionIn` rather than explaining anything: a
second spelling of a command that already has a surface is not a gap.

### What is left of the eighteen

Twelve, all of them the *install-time* half: `SectionSetFlags`, `SectionGetText`,
`GetCurInstType` and the rest read and write a section's state while the installer runs.
They want the other end of the same binding — `Section "Core" sec_core` writing a define
that install-time code can reach — and the surface for that is a separate question from
this one, because it is about how a **running** program names a section rather than how a
declaration does.

## Batch 16 — "classic UI" was the wrong name for thirty rows

Thirty `todo` rows carry one of two reasons: *the classic UI's appearance* (15) and *a
classic-UI caption or button label* (15). Both reasons are wrong, and the project already
decided the thing they were waiting for. `PLAN.md`'s command table says **MUI2 only;
classic pages reach through `raw`** — so "wait until the classic UI has a design" was never
a real blocker. There is no classic UI coming.

What the reasons should have said is a different question, and it has a different answer per
row: *does MUI2 emit this command itself?* That is checkable rather than arguable, so it was
checked, against the shipped `Contrib/Modern UI 2` sources rather than against memory. Four
groups fall out, and only one of them is a rejection.

### MUI2 emits it, so writing it raw is silently overwritten

Ten rows. MUI2 emits every one of these lines from inside its page macros, reading a
`!define` it supplies a default for through `MUI_DEFAULT`. Three of them (`LicenseText`,
`LicenseForceSelection`, `ComponentText`) sit in an `!ifdef` chain, but every branch emits —
there is no path on which MUI2 stays silent and the user's own line survives.

| Row | MUI2 reads | Emitted at |
| --- | --- | --- |
| `CheckBitmap` | `MUI_COMPONENTSPAGE_CHECKBITMAP` | `Pages/Components.nsh:43` |
| `ComponentText` | `MUI_COMPONENTSPAGE_TEXT_TOP`, `…_TEXT_INSTTYPE`, `…_TEXT_COMPLIST` | `Pages/Components.nsh:70` |
| `InstallColors` | `MUI_INSTFILESPAGE_COLORS` | `Pages/InstallFiles.nsh:27` |
| `InstProgressFlags` | `MUI_INSTFILESPAGE_PROGRESSBAR` | `Pages/InstallFiles.nsh:28` |
| `DirText` | `MUI_DIRECTORYPAGE_TEXT_TOP`, `…_TEXT_DESTINATION` | `Pages/Directory.nsh:47` |
| `LicenseText` | `MUI_LICENSEPAGE_TEXT_BOTTOM`, `MUI_LICENSEPAGE_BUTTON` | `Pages/License.nsh:61` |
| `LicenseBkColor` | `MUI_LICENSEPAGE_BGCOLOR` | `Pages/License.nsh:24` |
| `LicenseForceSelection` | `MUI_LICENSEPAGE_CHECKBOX_TEXT`, `…_RADIOBUTTONS_TEXT_ACCEPT`, `…_DECLINE` | `Pages/License.nsh:65-67` |
| `UninstallText` | `MUI_UNCONFIRMPAGE_TEXT_TOP`, `…_TEXT_LOCATION` | `Pages/UninstallConfirm.nsh:42` |
| `UninstallIcon` | `MUI_UNICON` | `Interface.nsh:119` |

This is the batch's finding, and it is the kind tier 3 cannot catch. A user who writes
`CheckBitmap` gets a script that assembles clean under `-WX` and an installer whose
checkboxes are MUI's defaults, because the page macro runs after the attribute and wins.
Emitting the raw command would be *worse than not exposing the row at all*: it would look
like it worked. So every one of the ten lowers to the `!define`, never to its own name.

Three consequences follow:

1. **The surface name is neither the classic one nor the MUI one.** `icon` already lowers to
   `MUI_ICON` or `MUI_UNICON` by half (`src/lower/mod.rs:1142`), and that is the precedent:
   the surface says what the thing *is*, the compiler says what NSIS reads. So not
   `checkBitmap` at top level and not `MUI_COMPONENTSPAGE_CHECKBITMAP` either.
2. **All ten are page-scoped** — most of them literally, inside MUI2's own `PageEx` blocks
   (`Pages/Components.nsh:64-72`). That is §15.7's sequential-`!define` hazard exactly: the
   define has to precede the `!insertmacro MUI_PAGE_*` that reads it, and a user cannot be
   asked to know that. It is the argument for the compiler owning define ordering, and it
   is now backed by ten rows rather than by principle.
3. **`UninstallIcon` may already be done.** `icon` inside `uninstaller {}` emits
   `MUI_UNICON` today. If that is the whole of the row, it is a `language` reclassification
   and not surface work.

### MUI2 emits it and there is nothing to configure

Two rows, and the only genuine rejections of the thirty:

- **`ChangeUI`** — MUI2 calls it five times to install its own dialog resources
  (`all "${MUI_UI}"`, plus the header-image and no-description variants). A user's call
  does not configure MUI; it fights MUI. `rejected`.
- **`XPStyle`** — `Interface.nsh` emits `XPStyle On` unconditionally, with the comment *"XP
  style setting in manifest resource"*. A user writing `XPStyle off` produces a
  last-one-wins race with no diagnostic. Leaning `rejected`, but this is the one row of the
  thirty that wants a decision rather than a lookup: rejecting it removes a real capability,
  and exposing it means promising an ordering the manifest may override anyway.

### MUI2 has no opinion, so these are ordinary surface

Sixteen rows, and the correction the earlier grouping got most wrong. These are absent
from the MUI2 sources entirely — not superseded, not conflicting, just orthogonal:

- **`BGGradient`, `BGFont`** — the full-screen background *window*, which is a separate
  window from the wizard and is unaffected by which page UI is in use.
- **`BrandingText`, `AddBrandingImage`, `SetBrandingImage`** — MUI2 supplies no define for
  any of them. It does style the branding area it finds (`SetCtlColors $mui.Branding.*
  /BRANDING`, `Interface.nsh:269-271`), which is the opposite of owning it: MUI decorates
  whatever the author put there.
- **`SetFont`** — see below; MUI2 reads it rather than replacing it.
- **`WindowIcon`, `UninstallCaption`, `CompletedText`, `SpaceTexts`**
- **`MiscButtonText`, `InstallButtonText`, `DetailsButtonText`, `UninstallButtonText`** —
  button labels come from the NLF language file by default, and these override it. That
  makes them a §15.26 localization interaction rather than a UI one: overriding here
  overrides *every* language at once, which is worth a diagnostic and is not a blocker.
- **`SubCaption`, `UninstallSubCaption`** — partial. MUI2 blanks exactly one index each
  (`SubCaption 4 " "`, `UninstallSubCaption 2 " "`, both `Pages/InstallFiles.nsh:29-30`);
  the other indices are untouched and free. A row that is owned for one argument value and
  free for the rest is a shape the table has no way to say, and this is the second row
  wanting a per-value rule after `PERemoveResource`.

### Four rows where the name misled and the signature did not

`SetCtlColors`, `SetBrandingImage`, `LoadAndSetImage` and `SetFont` were filed together
under *appearance* because of what they affect. Reading the signature rather than the name
splits them three ways — two of them are already counted in the sixteen above, and this is
why:

- **`SetCtlColors hwnd …` and `LoadAndSetImage … ctrl …`** take a control and change it
  while the installer runs. MUI2 calls `SetCtlColors` on twenty-odd of its own controls
  through `$mui.*` handles it does not export, so a user calling either needs a handle of
  their own. These leave the "classic UI" reason and join the `hwnd`/nsDialogs cluster,
  which grows from 14 to 16.
- **`SetBrandingImage [/IMGID=…] bitmap.bmp`** takes no handle: it fills the slot
  `AddBrandingImage` reserved, from inside a page callback. It belongs beside
  `AddBrandingImage` in the group above, not with the handle rows.
- **`SetFont [/LANG=…] face size`** is not an instruction at all — it is the installer-wide
  font attribute, and MUI2 *reads* it: `Interface.nsh:241` builds its bold header font from
  `$(^Font)`/`$(^FontSize)`, which is exactly what `SetFont` sets. Far from being
  superseded by MUI, it is the input MUI derives from. An ordinary attribute row, with
  `/LANG=` making it the repeated-per-language shape batch 13 already built.

### What landed

Thirteen of the thirty moved: eleven attributes, one `language`, one `rejected`. `todo`
goes 84 → 71, `attribute` 36 → 47.

| | Rows |
| --- | --- |
| New attributes | `brandingText`, `uninstallCaption`, `completedText`, `detailsButtonText`, `uninstallButtonText`, `installButtonText`, `windowIcon`, `buttonText`, `brandingImage`, `bgFont`, `font` |
| `language` | `UninstallIcon` — already done as `icon` in `uninstaller {}` |
| `rejected` | `ChangeUI` |

Every one of the eleven is proved by `tests/overlay.rs`, which derives its program from the
rows themselves: a new attribute needs no hand-written example because the `Setting` *is*
the example, and tier 3 hands the result to `makensis -WX`. Which is how the next two
findings arrived — by running the lines rather than by reading them.

### `AddBrandingImage` takes two numbers that have to agree

The row was going to be `edge`, `size`, `padding` with an `Int` padding. `makensis` said
*"Error while adding image branding support: Invalid number!"*, and the reason is not the
one the syntax line suggests:

- `top 20u 2u` assembles. `top 20u 2` is *Invalid number!* — the two arguments must use the
  **same unit**, so an `Int` padding could never be written beside a `u` size.
- `top 20 2` is *"Must use dialog units on non-Win32 platforms!"* — a bare pixel count
  cannot be compiled on the machine this project compiles on at all. So `u` is not one form
  among two; it is the only form that builds here.

Both parts are therefore `Str`. That they agree is a cross-field constraint no `Setting` can
state, and the compiler does not check it — `makensis` does, by name, which is the one case
where deferring beats a worse message.

Separately, `(height|width)` in the snapshot is a **metavariable** and not a pair of
keywords: it names which dimension the edge implies, and the value is a number. Enumerating
it would have rejected every legal value and completed to two illegal ones. Second row to
hit that trap after `PERemoveResource`.

### `SetCompressor` has to come first, and now does

`AddBrandingImage` changes the installer header, and NSIS refuses `SetCompressor` after the
header has changed — *"can't change compressor after data already got compressed or header
already changed!"*. So `attributes { brandingImage = …, compressor = "lzma" }` failed and
the same table written the other way round did not.

A Lua table has no order (§12), so this is the compiler's to own, and it now is:
`compressor` is hoisted to the front of the attribute block, exactly as `VIProductVersion`
is hoisted before `VIAddVersionKey`. One line moved in
`examples/01-mui-uninstaller/generated.nsi`, which is the whole visible effect.

This is the second ordering hazard the attributes block has turned up, and both were found
by tier 3 rather than by reading NSIS's prose. There is no reason to think it is the last.

### The three that did not land, and why the reason changed

Not "blocked on the classic UI" in any of the three cases. The third has since been ruled
and is no longer open:

- **`BGGradient`, `SpaceTexts`** — both are alternations (`off | (top [bottom [text]])`,
  `none | (required [available])`) and `-CMDHELP` flattens each to a single required
  position. `Setting` has no shape for "one keyword *or* a tuple", so the row is blocked on
  a shape rather than on a design.
- **`SubCaption`, `UninstallSubCaption`** — MUI2 blanks exactly one index each and leaves
  the rest free. A row owned for one argument value and open for the others is the same gap
  `PERemoveResource` opened, and the second row to want a per-value rule.
- **`XPStyle`** — wanted a ruling rather than a lookup, and got one: `rejected`. MUI2 emits
  `XPStyle On` unconditionally from `MUI_INTERFACE`, so the only value a user could want is
  `off` and it is the one value that cannot work — whichever line lands last wins, silently,
  and `-WX` sees nothing wrong with either. Exposing it would promise an ordering the
  manifest may override anyway. The capability this removes is one the shell decides, not
  the script; rejecting says so once, where the author wrote it.

`SetCtlColors` and `LoadAndSetImage` moved to the `hwnd` pile as planned, and
`SetBrandingImage` to the page-callback pile beside it.

## Batch 17 — a group whose reason had already retired itself

Five rows, one reason, and the reason was answered by a row that shipped in batch 1.

`SetCompress`, `SetCompressorDictSize`, `SetCompressionLevel`, `SetOverwrite` and
`SetDatablockOptimize` all carried *"compile time and positional: it changes the `file`
calls after it rather than executing, so a call inside an `if` would be a lie"*. That is a
true statement about a **call** and says nothing about an **attribute**. `SetCompressor`
has been `attribute("compressor", …)` since batch 1 on exactly this footing: an
installer-wide default, written once, placed by the compiler. The other five were never
blocked; the group was written before the attribute answer existed and nobody re-read it.

| NSIS | field | holds |
| --- | --- | --- |
| `SetCompress` | `compress` | `off \| auto \| force` |
| `SetCompressorDictSize` | `compressorDictSize` | `Int`, megabytes |
| `SetCompressionLevel` | `compressionLevel` | `Int`, 0–9 |
| `SetOverwrite` | `overwrite` | `on \| off \| try \| ifnewer \| ifdiff` |
| `SetDatablockOptimize` | `datablockOptimize` | `on \| off` |

The per-`file` override is a different feature and still absent — `file` has no
`overwrite = …`. Withholding the installer-wide default until that exists would be
withholding the common case for the rare one.

### The two compression settings exclude each other

Tier 3 found it twice in a row, and neither is in any syntax line:

```
warning 8026: SetCompressorDictSize: compressor is not set to LZMA. Effectively ignored.
warning 8025: SetCompressionLevel: compressor is set to LZMA. Effectively ignored.
```

`compressorDictSize` is read **only** under LZMA and `compressionLevel` **only** under
anything else, so under `-WX` no single script can carry both. The derived program writes
every attribute, which makes it the one script that wants to.

The fix is two-part and the second half is the point. `REAL` now pins
`compressor = "lzma"` so the dictionary size has a compressor that reads it, and a new
`EXCLUSIVE` list drops `SetCompressionLevel` from the derived program. Dropping a row from
tier 3 without checking it elsewhere would leave a `Setting` no `makensis` ever saw, which
is the one failure tier 3 exists to prevent — so
`compression_level_assembles_against_a_compressor_that_reads_it` assembles it on its own,
beside `compressor = "zlib"`.

The dependency is real for authors too, and it is deferred: `makensis` states it by name
and by line, and no `Setting` can say "meaningful only when a sibling holds one value".
That is the third cross-field constraint in two batches, after `AddBrandingImage`'s unit
agreement and `SetCompressor`'s ordering.

## Batch 18 — the same re-reading, one group later

Batch 17 ended by saying every remaining group deserves re-reading before the design work
it claims to need. The next group down the list was *file surface beyond
`file`/`delete`/`fileOpen`: one overlay row each*, four rows, and the reason turns out not
to be a reason. "One overlay row each" is a statement about **cost**. It names no missing
design, no unruled spelling, no shape the table cannot hold — it says only that four rows
would take four rows.

Three of the four are the write halves of reads that had already landed:

| NSIS | installua | shape |
| --- | --- | --- |
| `FileWriteByte` | `f:writeByte` | handle in, number in — `f:readByte` reversed |
| `FileWriteWord` | `f:writeWord` | handle in, number in — `f:readWord` reversed |
| `FileWriteUTF16LE` | `f:writeUtf16Le` | handle in, string in — `f:readUtf16Le` without `maxLen` |

There was never a second thing to decide about any of them. The reader of each pair went in
during the read pass and the writer stayed behind on a note about how many rows the group
had, which is the same failure batch 17 found and not a different one.

`FileWriteUTF16LE` carries `/BOM` at `after: 0`, before the handle — the position
`Opt::after` exists for, first needed by `GetFullPathName`'s `/SHORT`. Only the first write
to a file wants a byte-order mark, so it is a decision and gets `named("bom")` rather than
`always()`; the caller writes `f:writeUtf16Le("done", { bom = true })` and the emitter
decides where the word goes. The golden confirms it: `FileWriteUTF16LE /BOM $0 "done"`.

### The fourth was blocked, and its reason now says so

`ReserveFile` stays `todo`, but with an honest reason instead of an inherited one:

```
ReserveFile [/nonfatal] [/r] [/x filespec [...]] file [file...] | [/nonfatal] /plugin file.dll
```

That is an alternation — `BGGradient`'s problem — and the second alternative is a plugin
DLL, which is §11's. Two open questions, neither of them "one overlay row". A group reason
that lumps a genuinely blocked row in with three unblocked ones hides both facts at once.

## Batch 19 — the reason the row below it already answered

`Exec` and `ExecWait` were `todo` for *"one argument that is part path and part switches;
§5's `/`-to-`\` rule cannot apply to half a string"*. Half of that is true and the other
half does not follow from it.

The true half: `Kind::Path` would turn the `/S` in `setup.exe /S` into `\S`, so the path
kind is wrong. The half that was assumed rather than checked: that `Kind::Value` is
therefore wrong too. §15.2 normalises `/` where **NSIS** demands a backslash — registry
subkeys, `File`, `SetOutPath` — not where Windows does, because *"Windows accepts forward
slashes at the API level"*. `Exec` hands its string to `CreateProcess`, which resolves the
program through that same Win32 parser. Forward slashes go out unchanged and the program
is found.

| installua | emits |
| --- | --- |
| `exec(INSTDIR .. "/app.exe /S")` | `Exec "$INSTDIR/app.exe /S"` |
| `local code = execWait(INSTDIR .. "/app.exe /S")` | `ExecWait "$INSTDIR/app.exe /S" $0` |

`ExecWait`'s exit code is an optional trailing output, which is `FileSeek`'s rule
unchanged: it is emitted only when something reads it, so `execWait(cmd)` alone still
writes two words.

### The ruling was already in the file, two rows down

`ExecShell`'s `file` position is `Kind::Value`, and its comment says why in almost these
words — a shell target may be a URL, Win32 takes `/` as a separator, and §15.2 applied
there would produce `https:\\…`. That comment was written while these two rows sat on a
`todo` that contradicted it. Three batches in a row now, the blocker was a sentence
nobody had read against the rest of the file.

## Batch 20 — the page block, and the rule that decided every setting's home

Fourteen rows, and the only one of the four remaining groups that really was blocked on
design. The design is [`PHASE-6-PAGES.md`](PHASE-6-PAGES.md); this is what building it
found.

A page is a **positional entry** in `installer {}` / `uninstaller {}`, reached by member
access:

```lua
installer {
  checkBitmap = "check.bmp",

  page.license { file = "LICENSE.txt", checkbox = "I accept the terms." },
  page.directory { topText = "Choose where it goes.", verifyOnLeave = true },
  page.instFiles {},

  section("Core", function() … end),
}
```

`page.directory` and not `page("Directory", …)` because the seven pages are a **closed**
set MUI2 picks, where a section's name is an open one the author picks — so one completes
and the other cannot. That sharpens §15.1's `lang.X` rule, which until now justified
itself only by "completion works" without saying when completion is available.

### MUI2's source drew the line, so no setting needed a judgement

The question the last four batches deferred was where a `MUI_*` setting lives. It has a
mechanical answer, and it is in MUI2's own files:

- a setting written inside the `PageEx` MUI2 generates, and `!undef`'d after, is
  **page-scoped**;
- a setting written inside an `!ifndef`-guarded `MUI_*PAGE_INTERFACE` macro runs on the
  **first** page of its type and never again.

The second kind is a block field, because putting it on the page would be a lie a second
page tells silently. Four settings fall on that side despite names that say otherwise:
`CheckBitmap`, `InstallColors`, `InstProgressFlags` and `LicenseBkColor`. Nine rows had
been carrying a reason that said only *"where MUI settings live is unruled"*; reading two
hundred lines of `Contrib/Modern UI 2` answered it for all nine at once.

### The grouping is a correctness property, and MUI2's own cleanup has holes

`ir::Module.pages` is `Vec<Page>` now, each page owning its `!define`s, because MUI2
expands them **at** the insertion point. A shared list up front would give two Directory
pages the last page's text — which is the hazard §15.7 named and the shape that could not
express it.

Then tier 3 found the other half. MUI2 clears most page settings itself, so a second
`!undef` is warning 6155 and an error under `-WX`; but `UninstallConfirm.nsh` never clears
`MUI_UNCONFIRMPAGE_VARIABLE`, and `License.nsh` clears
`MUI_LICENSEPAGE_CHECKBOX_TEXT_ACCEPT`, which is a name nothing defines — the radio button
texts are spelled `…_RADIOBUTTONS_TEXT_ACCEPT`. So each page carries the **difference**
between what it sets and what MUI2 clears, and a first attempt that undefined everything
failed to assemble.

### `license` left the block, and `Var`s moved ahead of the pages

`installer { license = … }` was page data written at block level: exactly one page read it,
and a script naming no License page dropped it without a word. It is `page.license { file =
… }` now, where that mistake is unwritable — the second of the two options
`examples/01-mui-uninstaller/README.md` had named as open since Phase 0.

`page.directory { variable = target }` forced one reordering. `DirVar` takes a *variable*
rather than a value, so the field names a global — and `Module`'s field order had the
`Var` lines after the pages, which NSIS rejects. Globals now come sixth, before the MUI
defines. A `Var` declaration is inert, so moving it earlier is strictly safer.

### What landed

| move | rows |
| ---- | ---- |
| `todo` → page attribute | 7 |
| `todo` → block attribute | 4 |
| `todo` → rejected | 3 |

`todo` 60 → **46**; `attribute` 52 → 63; `rejected` 12 → 15.

`PageEx`, `PageExEnd` and `PageCallbacks` are `Rejected` rather than `Todo`: `PageEx` is
the block MUI2 *generates* around every one of its pages, so writing one is not
configuring the UI but reimplementing MUI2 beside it. `Page` and `UninstPage` keep the old
reason and stay `todo` — their signature is an alternation whose `custom` half is the
nsDialogs insertion point, and MUI2 ships no `MUI_PAGE_CUSTOM` to reach it through.

## Batches 22–26 — a section a running program can name

Eleven rows and five batches, against the question batch 15 left open and stated in
*Still open* as three candidate answers: *"a field on a handle the block never returns, an
addressing function that takes the section's title, or a `sections.core` table"*. The
design is [`PHASE-6-SECTIONS.md`](PHASE-6-SECTIONS.md); the answer is the first one with
its premise removed. **The block returns the handle after all** — or rather `section(…)`
does, and the block merely lists it:

```lua
local core = section { "Core", required = true, body = function() … end }
local tools = group { "Tools", sections = { profiler } }

installer {
  installTypes = { "Full", "Minimal" },
  core, tools,

  onInit(function()
    currentInstType = "Minimal"
    core.text = ""
    tools.expanded = profiler.selected
    docs.size = 4096
  end),
}
```

The other two answers both invent a name: an addressing function has to be given the
section's *title*, which is a string nothing checks and which `SectionSetText` can change
out from under it, and a `sections.core` table is a second namespace keyed by a spelling
the author already wrote as a `local`. The handle is a Lua local and nothing else, and
`${SEC_core}` is derived from that local's name — so a misspelling is an undefined
variable rather than an installer that ticks the wrong box.

### Declaration order stopped being install order

The cost, and the one thing a reader of an Installua program now has to learn: a
`section(…)` at the top level is **deferred**, and the block's positional list decides
where it goes. Four writings became possible that were not, and each has a diagnostic — a
handle no block lists, one listed twice, one listed by both blocks, and one referenced
from the other half.

Sections also had to move ahead of functions in the emitted file, because `${SEC_core}` is
a `!define` and NSIS expands it at *parse* time: a callback that addressed a section it
was written above got the literal text and warning 6000.

### A position the compiler resolves from a name is a fourth kind

`Kind::Bound`, beside the label position §15.20 needed. `SectionSetText ${SEC_core} "…"`
takes its index from the handle and `SetCurInstType 1` its position from the block's
`installTypes` list, so neither is an argument — and the parameter model had no way to say
that. Saying it is what keeps `handle.text(…)` out of the generated stubs and turns the
`LANGUAGE.md` cell into `handle.text = …` rather than a function call. The stub declares
the fields instead, as `installua.Section` and `installua.Group`.

The same ruling twice over: the number NSIS reads exists in exactly one place. A section's
index is its position in the block, an install type's is its position in the block's list,
and an index the surface could write would be a second numbering to keep in step with the
first. So `currentInstType` reads and writes a **name** — and reads it back through a
comparison chain built at compile time rather than through `InstTypeGetText`, because the
label is what `instTypes.setText` changes and `currentInstType == "Full"` has to survive
it.

### What landed

| move | rows |
| ---- | ---- |
| `todo` → exposed | 11 |

`todo` 46 → **35**; `exposed` 85 → 96.

`SectionGetInstTypes` is the twelfth row and did not land. Its write takes a list of names
and its read answers with the bit field, and this language has no list *value* to hand
back — so the row keeps a `todo` whose reason is now about the missing shape rather than
about addressing a section, which is no longer missing.

## Still open

- **`installua stubs` scans one directory** — carried over from Phase 5, unchanged.
- **The `todo` reasons are grouped**, and the grind retired the groups it could. Batch 16
  emptied the two *classic UI* groups, batch 17 the *compile time and positional* one,
  batch 18 the *file surface* one, batch 19 the *part path and part switches* pair,
  batch 20 the *where MUI settings live is unruled* group, and batches 22–26 the
  *addresses a section by index* one. Of the 35 left, the `hwnd` surface is 14, §15.26's
  locale tables are 5, and the rest are one-offs. One group remains, and it is not a
  reason nobody read: it names design that really is missing.
- **A group's reason is written once and never re-read.** Batch 17's five rows were
  unblocked from the moment `SetCompressor` became an attribute, and stayed `todo` for
  sixteen batches because the reason was true of the shape they were rejected as. Batch 18
  found the next group down was worse: *"one overlay row each"* states a cost and no
  blocker at all, and three of its four rows were the write halves of reads already
  exposed. Batch 19's pair was contradicted by the comment on the row two below it. Three
  groups re-read, three groups emptied, and each reason failed differently: true of the
  wrong shape, a cost mistaken for a blocker, a half-truth whose second half was assumed.
  The remaining ones should be re-read before the design work they claim to need, not
  after.
- **A `Setting` cannot say "meaningful only when a sibling holds one value".**
  `compressionLevel` and `compressorDictSize` exclude each other through `compressor`, and
  `makensis` is the only thing that knows. Third cross-field constraint in two batches.
- **`MUI_STARTMENUPAGE` is newly expressible and not yet written.** It was unspellable
  under a bare list of page names, because its macro takes arguments; under `page.*` it is
  another page with fields. The same goes for the settings with no NSIS command behind
  them — `MUI_WELCOMEPAGE_TEXT`, `MUI_FINISHPAGE_*` — which no census row tracks, so they
  are invisible to the burndown and want an inventory of their own.
- **`SubCaption` and `UninstallSubCaption` are still blocked, and not by the page world.**
  MUI2 blanks exactly one index of nine and leaves the rest free. The shape a table has no
  way to say is a row *owned for one argument value and open for the others*.
- **A `Setting` cannot say "either a keyword or a tuple".** `BGGradient` and `SpaceTexts`
  are both `off | (…)`, and the snapshot flattens each to one required position. Two rows,
  one shape, and the shape is the work.
- **The attributes block has ordering hazards and no inventory.** Two are now handled —
  `VIProductVersion` before `VIAddVersionKey`, `SetCompressor` before anything that touches
  the header — and both were found by a golden failing rather than by looking. A third would
  be found the same way, which is to say by luck.
- ~~**A running program cannot name a section.**~~ **Closed by batches 22–26**, with the
  first of the three answers and its premise dropped: `section(…)` returns the handle, so
  there is no name for the compiler to invent. What is left of the family is one row —
  `SectionGetInstTypes`, which would have to answer with a list value this language does
  not have.
- **A metavariable with no marker is still a keyword.** `PERemoveResource restype resname
  reslang|ALL` reads `reslang` as a member beside `ALL`, and unlike `{GUID}` there is
  nothing in the notation that says it is a placeholder — not a brace, not a case, not a
  position. The row pays for it with a plain `STR` and no completion, which is the right
  trade and not a fix. What would fix it is a shape this table does not have: *one of these
  keywords, or any string*, which is `open` at the level of a value rather than of a set.
  One row wants it, so it should arrive with the second.
- **A flag that takes one value and does not repeat has no spelling.** `Offer::List`
  covers `File`'s `/x` because it repeats; `SendMessage`'s `/TIMEOUT=n` and the three
  other single-valued flags are all on `todo` rows, and the variant that spells them
  should arrive with the first of those rows rather than before it.
- **Nothing says where a call is legal.** `SetSilent` is meaningful only in `.onInit`,
  `SetAutoClose` only outside it, and the compiler has no way to state either. Both tiers
  pass a call that is simply dead.
- **No fixture carries a version resource.** `GetDLLVersionLocal` is the only row whose
  blocker is a file rather than a design: its example reads the build machine at compile
  time, and neither the `.ico` in `tests/fixtures` nor any NSIS-shipped DLL has version
  info. A real PE with one, committed there, is the whole task.
