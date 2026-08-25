# Program 3 — file iteration

Assembles clean under `makensis -WX`.

The staging program. Three iteration forms, running on two different machines, and the
reader has to be able to tell which is which from the source alone.

## What it settled

**`lines(f)` needs `TextFunc.nsh`, which the user never wrote.** `FileRead` returns the
line terminator; Lua's `lines()` strips it. Keeping the Lua meaning therefore costs a
`${TrimNewLines}` per iteration and an `!include` nobody asked for. This is the *same*
shape as the `StrFunc` init lines — a header pulled in by an adapter rather than by an
`import` — but it is a whole include rather than a setup line, and the adapter rule only describes
the setup-line half. The collect-then-emit pass has to cover both.

The alternative, leaving the terminator on, is rejected for the reason the operator rules reject
inheriting NSIS's `//`: an operator or a stdlib name that Installua spells like Lua must
behave like Lua, or the divergence is discovered in a shipped installer.

**EOF is `IfErrors`, so the loop header is three instructions, not one.**

```nsis
_generated_for_0_top:
  ClearErrors
  FileRead $0 $2
  IfErrors _generated_for_0_end 0
```

The `ClearErrors` is not optional: the error flag is sticky, and any instruction before the
loop that set it makes the first `FileRead` look like EOF. Note this is the compiler
reading the flag; `errors()` in source stays an impure predicate.

**One label counter per body, incremented per construct in source order.** Program 3 has
six constructs and emits labels for three of them — constructs 1, 2 and 5 fuse into jumps
at labels the enclosing loop already owns. So the numbering is sparse
(`_generated_endif_3`, `_generated_for_4_top`) and that is correct: numbering the *emitted*
labels consecutively instead would make the golden files churn whenever a construct starts
or stops needing one.

**`break` and `continue()` are just the loop's two labels.** `continue()` jumps to
`_generated_for_0_top`, `break` to `_generated_for_0_end`. Both already exist, so neither
terminator costs anything — which is the argument for having `continue()` at all when Lua
does not.

**`glob` unrolls, and the match order has to be sorted.** Two `File` lines, in filename
order. If the compiler emitted them in readdir order the golden file would differ between
machines, which the test strategy cannot tolerate. Cheap to fix, easy to forget.

## What it left open

**`glob` is build-machine filesystem access, and "compiles, never executes" does not obviously permit it.**
"Installua compiles; it never executes Lua at build time" is about *Lua* execution, and a
glob is the compiler reading a directory, the same class of thing as `!system`. But it
means the same source produces different output on two machines, which is the exact shape
"read to verify, never to decide" forbids. The distinction that saves it: the glob's
*result* is data the source is asking for, not a property of the toolchain. Worth writing
down as a fourth application of the rule rather than leaving it implicit.

**`for attempt = 1, 3` allocates a register for a counter that is also printed.** Nothing
wrong with it, but it is the first case where a loop induction variable is user-visible,
which means the allocator cannot treat induction variables as a private class.

**`manifest:close()` after a `break`.** The handle is closed on both loop exits here
because the close is after the loop, but a `return` inside the loop would leak it. Lua's
answer is `<close>`, which the language rejects outright. So either the compiler tracks handles to
their scope end, or leaked handles are the user's problem and the docs say so. Not
decided anywhere.

## Exposed commands used

`setOutPath` · `file` · `fileOpen` · `fileRead` (via `lines`) · `fileClose` ·
`detailPrint` · `abort` · `createDirectory` · `sleep` · `clearErrors` · `errors`

Iterators: `glob` (build) · `lines` (install) · numeric `for` (install)
