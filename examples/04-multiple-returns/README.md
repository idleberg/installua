# Program 4 — functions returning multiple values

Assembles clean under `makensis -WX`, **and runs correctly under wine** — see
`verification/semantics/`. This is the one program where assembling proves nothing: a
register restored one `Exch` out of place produces an installer that runs and gives a wrong
answer.

```
files=1 kib=0 countdown4=10
```

## What it settled

**The calling convention, in three rules.** `Call` has no argument list, so the stack is
the only convention available, and every choice in it is arbitrary until written down.
These are the ones the golden file pins:

1. **Arguments are pushed in reverse source order**, so the callee's first `Pop` is its
   first parameter.
2. **Returns are pushed in reverse source order**, so the caller's first `Pop` is the first
   return value.
3. **Caller-saves are pushed before the arguments**, ascending register number, restored in
   reverse.

Rule 3 is the one that has to be exactly this way round rather than merely consistent.
Because the saves go on first, the callee's results sit on *top* when it returns, and the
restores fall out underneath them without a single `Exch`:

```nsis
  Push $0        ; save kib
  Push $1        ; save files
  Push 4         ; argument
  Call countdown
  Pop $2         ; result
  Pop $1         ; restore
  Pop $0         ; restore
```

Saving *after* the arguments would need an `Exch` per saved register to get the result out
from underneath, which is both slower and the exact place an off-by-one produces plausible
NSIS. Caller-saves are "push `live ∩ clobbered`, ascending, restored in
reverse" and does not say where they sit relative to the arguments; they sit before.

**Recursion needs no special case.** `countdown` is an SCC of one and its clobber set
`{$0,$1,$2}` saturates in the first extra round, exactly as predicted. The recursive
call site then does an ordinary caller-save of `n`, because `n` is live across it and the
callee clobbers `$0`.

**A dropped output still has to be allocated.** `local size, files, _ = fileFunc.getSize(dir, "")`
uses two of three outputs, but `${GetSize}` writes all three, so the third needs a register
the allocator knows about. The declaration's output count, not the call site's arity, is
what the allocator reads — which is "plural outputs are invisible at the call site",
now with a concrete instance.

## What it left open

**Header declarations need a sign attribute on their outputs.** `budget` computes
`size // 1024`, and the golden emits a bare `IntOp $1 $1 / 1024` with no sign fixup. That is
only correct because `${GetSize}`'s output cannot be negative — a fact that lives nowhere.
The lattice gives `int` a `sign` attribute; the header declaration format has no column
for it. Without one, every `//` on a header result pays the sign fixup for nothing, and
"elided whenever the sign is statically known" quietly stops being the common case.

**The convention needs a name and a home before Phase 3, not during it.** Everything above
is three sentences of documentation and an enormous amount of debugging if it is decided
implicitly by whichever code path is written first.

**Clobber sets are exercised once across all five programs** (see program 2's README for
why). Phase 3's exit criterion — "program 4 emits the hand-written expectation" — is
necessary and nowhere near sufficient.

## Exposed commands used

`setOutPath` · `file` · `detailPrint`

Headers: `FileFunc.getSize` · user `func` with two returns · self-recursion
