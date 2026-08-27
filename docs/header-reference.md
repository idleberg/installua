# Header reference

Every header macro Installua ships a declaration for: `FileFunc`, `TextFunc` and
`WordFunc`, reached with `import "FileFunc"` and called as
`fileFunc.getParent(path)`.

For plugins, see [plugin-reference.md](plugin-reference.md). For the call
syntax and the walker forms, see [`### import`](reference-map.md#import).

## Why a header needs a declaration at all

**Every macro puts its outputs last.** A header macro cannot return anything, so
`!insertmacro` is handed the registers to write into — and there is no
convention to infer, because `${GetSize} "$dir" "" $0 $1 $2` puts the
destination last while `${StrCase} $0 "text" "L"` puts it first. Guessing emits
NSIS that looks right and is not.

That is why an output here is a `Param` position rather than a `returns` field,
and it is the opposite of a plugin, whose outputs are on the stack in `Pop`
order.

Every `params` and `outputs` list was read from the macro body in
`NSISDIR/Include/*.nsh` and checked against the matching `Examples/*.nsi`, which
is the file NSIS itself keeps honest.

## The `S` suffix, and which way round it is

NSIS ships several of these as **two macros** — `WordFind` and `WordFindS` call
two different artificial functions with identical arguments — so they are two
methods here. The format has no flag that could join them, and inventing one
would be inventing a parameter NSIS does not take.

Note which way round it is: **the unsuffixed name is case-insensitive**, the
opposite of `==`, which lowers to `StrCmpS`. That is not an inconsistency to
fix. `==` is case-sensitive because Lua's `==` is, and `import` is the
NSIS-shaped surface, where the name a reader arrives with is the one that should
work.

## Callbacks

Six macros take a **function address**: `Locate` and `GetDrives` in `FileFunc`,
and `LineFind`, `FileReadFromEnd`, `TextCompare`, `TextCompareS` in `TextFunc`.
Their `params` end in `callback`, which says a function goes there and nothing
else — which register each argument arrives in is *behaviour*, and behaviour
lives in [`src/lower/callback.rs`](../src/lower/callback.rs) where a test can
reach it. A third-party callback macro therefore cannot be declared.

Five of the six are `for … in` loops, because a loop body says only "keep going"
or "stop". `lineFind` is not, and that is the one interesting difference: its
callback returns the line to *write*, and "and this is the new value" is not
something a loop body can say.

---

# FileFunc

## Path splitting

| Method | Arguments | Returns |
| ------ | --------- | ------- |
| `.getParent(path)` | `path` | `string` |
| `.getFileName(path)` | `path` | `string` |
| `.getBaseName(path)` | `path` | `string` |
| `.getFileExt(path)` | `path` | `string` |
| `.getRoot(path)` | `path` | `string` |
| `.bannerTrimPath(path, width)` | `path`, `string` | `string` |

All five splitters take `path` rather than `string`, and that is load-bearing
rather than tidy: they split on `\` and nothing else, so a program that wrote
`INSTDIR .. "/lib/app.dll"` and got `string` here would be handed the whole
thing back as if it had no parent at all. Their results are `string`, because
`path` is an *input* spelling — it normalises on the way in, and what a macro
already wrote is whatever it wrote.

- `getBaseName` is the file name without its extension. The NSIS name says "base
  name" and means the last component, not the stem of a path.
- `getFileExt` gives the extension **without** the dot, and empty for a name with
  none.
- `getRoot` gives `C:\` from `C:\dir\file`, and `\\server\share\` from a UNC
  path — the root a path is relative to, which is why it keeps its trailing
  separator where `getParent` drops it.
- `bannerTrimPath` shortens the middle to `C:\dir\...\file` so the result fits a
  width, for a progress line. The width is a length with an optional trailing
  `A`–`D` choosing which end goes — `"35A"` — so it is a `string`, not an `int`.

## The command line

| Method | Arguments | Returns |
| ------ | --------- | ------- |
| `.getParameters()` | — | `string` |
| `.getOptions(params, switch)` | `string`, `string` | `string` |
| `.getOptionsS(params, switch)` | `string`, `string` | `string` |
| `.getExeName()` | — | `string` |
| `.getExePath()` | — | `string` |

`getParameters` is everything after the executable's own name, unparsed — what
an `onInit` reads before anything else, and it takes no arguments at all.

`getOptions(params, "/D=")` answers with what followed `/D=`, and **sets the
error flag when the switch is absent**, which is the difference from an empty
value that was written. Its parameters are a `string` and not a `path`:
normalising `/` in a command line would turn every switch into a directory
separator.

`getExeName` and `getExePath` are the running executable's own file name and the
directory holding it. They differ from `$EXEPATH`/`$EXEDIR` only in being
available to a header written before those existed.

## Walking the target's disk

| Walker | Binds | Sentinel |
| ------ | ----- | -------- |
| `.locate(path, options)` | `path`, `directory`, `name`, `size` | `StopLocate` |
| `.getDrives(types)` | `drive`, `kind` | `StopGetDrives` |

Both are `for … in` loops, because both answer only "keep going" or "stop".

`locate`'s options select what is walked: `/L=F` files only, `/L=D` directories,
`/M=*.tmp` a mask, `/S=1M-` a size bound, `/G=0` no recursion. The default walks
everything under the path, recursively.

**`locate` is the one macro with no other spelling in the language.** `glob`
walks the *build* machine and unrolls before anything ships; `locate` walks the
disk the installer is standing on.

`getDrives`' `kind` is one of `FDD`, `HDD`, `NET`, `CDROM` or `RAM`, and the
argument is a `+`-joined list of those same words — or `ALL` for every drive
letter regardless of type.

## Facts about a file on the target

| Method | Arguments | Returns |
| ------ | --------- | ------- |
| `.getSize(path, options)` | `path`, `string` | `uint` ×3 — size, files, directories |
| `.driveSpace(drive, options)` | `path`, `string` | `uint` |
| `.getTime(path, option)` | `path`, `string` | `string` ×7 |
| `.getFileVersion(path)` | `path` | `string` |
| `.getFileAttributes(path, which)` | `path`, `string` | `string` |
| `.dirState(path)` | `path` | `int` |
| `.refreshShellIcons()` | — | nothing |

`getSize` answers size, files, directories. A byte count cannot be negative, and
that `uint` is what makes `size // 1024` a bare `IntOp` with no sign fixup.

**`getTime` returns seven values, in this order: day, month, year, weekday,
hour, minute, second.** That is the order `Examples/FileFunc.nsi` prints them as
`$0/$1/$2 ($3)` and `$4:$5:$6`. Getting it wrong is undetectable — every one is
a two-digit string — which is exactly the kind of fact a declaration is for. The
weekday is a name (`"Monday"`), not a number, so all seven are `string` and the
numeric ones keep their leading zero: `IntFmt "%.2u"` in the macro body is what
puts it there, and `"08"` compared as an int would be fine while `"08"` printed
as a date is the point. The option chooses the clock: `"L"` local now, `"LS"`
UTC now — both ignore the first argument — and `"A"`/`"C"`/`"M"` the file's
access, creation or modification time, with `"AS"`/`"CS"`/`"MS"` the UTC forms.

`getFileVersion` gives `"1.4.2.0"` from the file's version resource — a `string`
and not four numbers, because the macro writes one register. A program that
wants the parts has `string.find` or `wordFind`.

`getFileAttributes(path, "READONLY")` answers `"1"` or `"0"`, and
`getFileAttributes(path, "ALL")` answers the whole list joined by `|`. One call
with two return shapes chosen by a string, so `string` is the only honest type —
the same reason `versionCompare` is not an `int`.

`dirState` answers `1` has files, `0` is empty, `-1` does not exist. Signed
because of that last one, and the three-way answer is why it is not a `bool`:
"empty" and "missing" are the two cases an installer has to tell apart before
calling `rmDir`.

`refreshShellIcons` tells the shell to re-read its icons after a file
association changed. Nothing in, nothing out.

---

# TextFunc

Reading and rewriting files on the target at install time — the job `glob`
cannot do.

## Walkers and the rewriter

| Macro | Binds | Sentinel |
| ----- | ----- | -------- |
| `.fileReadFromEnd(file)` | `line`, `remaining`, `number` | `StopFileReadFromEnd` |
| `.textCompare(a, b, option)` | `line`, `number`, `other`, `match` | `StopTextCompare` |
| `.textCompareS(a, b, option)` | the same, case-sensitively | `StopTextCompare` |
| `.lineFind(input, output, range, body)` | **not a loop** — see below | — |

`lineFind` has **three answers rather than two**, which is why it is not a loop:
returning a string writes it, returning `skip` drops the line, and returning
`stop` ends the walk. The range is `first:last`, negative counting from the end,
and `/NUL` as the output writes nothing at all — a read-only pass over the file.

`fileReadFromEnd` walks backwards from the last line. `remaining` counts down
and `number` counts up from the start of the file, so "the last ten" and "line
90" both have a name.

`textCompare`'s `other` is the second file's line and `match` its number, `0`
when the line matched nothing — the pair that makes this a diff and not a walk.
The option picks what the callback is called for: `FastDiff` and `SlowDiff` fire
on lines that differ, `FastEqual` and `SlowEqual` on lines that agree. Slow
compares every line against every line; fast walks both in step.

## Reading and joining

| Method | Arguments | Returns |
| ------ | --------- | ------- |
| `.lineRead(file, number)` | `path`, `int` | `string` |
| `.lineSum(file)` | `path` | `uint` |
| `.fileJoin(first, second, output)` | `path`, `path`, `path` | **nothing** |

`lineRead`'s number is negative-from-the-end, so `-1` is the last line — that is
why it is a signed `int`; the macro runs `IntOp $1 $1 + 0` on it and compares
against zero. **Line 0 does not exist**: numbering starts at 1, and 0 sets the
error flag.

`lineSum` is `uint` for the same reason `getSize` is: a count of lines cannot go
negative, so `lines // 2` needs no sign fixup. Its error path writes an empty
string rather than a number, and the error flag is what says so.

`fileJoin` writes `first` then `second` into `output`. Three paths in and
**nothing out** — the result is the file, and a program that tries to read a
return value here is reading the stack. An empty third argument appends `second`
onto `first` in place, which is the common call and the one that makes the
missing output easy to overlook.

## Config files and recoding

| Method | Arguments | Returns |
| ------ | --------- | ------- |
| `.configRead(file, entry)` | `path`, `string` | `string` |
| `.configReadS(file, entry)` | `path`, `string` | `string` |
| `.configWrite(file, entry, value)` | `path`, `string`, `string` | `string` |
| `.configWriteS(file, entry, value)` | `path`, `string`, `string` | `string` |
| `.fileRecode(file, direction)` | `path`, `string` | nothing |
| `.trimNewLines(text)` | `string` | `string` |

These are the `name=value` file without an INI's sections —
`configRead(file, "Port=")` answers with what followed. **The entry carries its
own separator**, so the `=` is part of the string a caller passes rather than
something the macro adds.

`configWrite` answers `"SAME"`, `"CHANGED"`, `"ADDED"` or `"DELETED"` — deleted
because an empty value *removes* the line rather than blanking it, which is the
one behaviour here a caller will not guess. A `string` and not four cases the
compiler knows: it is the word the macro leaves in the register, and the error
path leaves an empty one.

`fileRecode` works in place and has only two spellings: `"OemToChar"` or
`"CharToOem"`. Anything else sets the error flag, which is why the type is a
`string` rather than something the declaration could narrow — the macro checks
it at install time.

**`trimNewLines` is the odd one.** Despite NSIS naming the argument `_FILE`, it
trims a **string**, not a file: it strips trailing `$\r` and `$\n` from a value,
and it exists because `fileRead` hands back the line ending along with the line.

---

# WordFunc

String surgery on the target at install time. Every one of these writes a single
register that `!insertmacro` is handed last, and every result is a `string`.

**Why every result is a `string`:** several of these answer with a number under
one option letter and with text under another — `wordFind(s, " ", "#")` counts
words while `wordFind(s, " ", "+1")` returns one — and a type that depended on
the *value* of an argument is not something a declaration can say.

Each has an `S` twin that is the case-sensitive half; see
[the note above](#the-s-suffix-and-which-way-round-it-is).

## Finding and counting

| Method | Arguments | Returns |
| ------ | --------- | ------- |
| `.wordFind(string, delimiter, option)` | `string` ×3 | `string` |
| `.wordFind2X(string, open, close, number)` | `string` ×4 | `string` |
| `.wordFind3X(string, open, centre, close, number)` | `string` ×5 | `string` |

`wordFind`'s option is the whole language of the macro: `"+1"` the first word,
`"-1"` the last, `"#"` the number of words, `"*"` the number of delimiters, and
an `E` prefix on any of them sets the error flag instead of returning the input
unchanged when the word is not there.

`wordFind2X` gives what lies between two *different* delimiters —
`wordFind2X(s, "[", "]", "+1")` takes the first bracketed run. **The number goes
last, after both delimiters**, which is the argument order a caller gets wrong
from memory. `wordFind3X` adds a third marker in the middle: delimiter, centre,
delimiter, then the number — five arguments, and the centre is the one that is
easy to read as another delimiter.

## Rewriting

| Method | Arguments | Returns |
| ------ | --------- | ------- |
| `.wordReplace(string, word, replacement, number)` | `string` ×4 | `string` |
| `.wordAdd(string, delimiter, word)` | `string` ×3 | `string` |
| `.wordInsert(string, delimiter, word, number)` | `string` ×4 | `string` |
| `.strFilter(string, filter, include, exclude)` | `string` ×4 | `string` |

`wordReplace`'s number says *which* occurrence: `"+1"` the first, `"-1"` the
last, `"+"` or `"-"` all of them, and `"{}"` around any of those keeps the
surrounding delimiters.

`wordAdd` appends a word unless the string already holds it — the `PATH` idiom,
and the reason to prefer it over `..` is that "already there" is what it checks.
The word may carry a leading `-` to mean remove instead.

`wordInsert` puts a word at a position rather than at the end:
`wordInsert(string, delimiter, word, "+2")` makes it the second word.

`strFilter` has four arguments and only the first is obvious. The filter is a
digit combination naming the character classes to keep — `"1"` digits, `"2"`
upper case, `"3"` lower case, and pairs like `"12"` — with a leading `-` to drop
them instead; `include` and `exclude` are literal character lists that override
the filter either way.

## Versions

| Method | Arguments | Returns |
| ------ | --------- | ------- |
| `.versionCompare(a, b)` | `string`, `string` | `"0"`, `"1"` or `"2"` |
| `.versionConvert(version, letters)` | `string`, `string` | `string` |

`versionCompare` answers `"0"` equal, `"1"` the first is newer, `"2"` the second
is — a `string`, because that is what the macro leaves in the register and
comparing it as an int would be a guess the lattice has no evidence for.

`versionConvert` turns `"1.4.2-beta"` into something `versionCompare` can order,
by mapping the letters in its second argument onto digits. That argument is the
character list, and defaults to the alphabet when empty.
