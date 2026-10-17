# Program 5 — strings and integers

Assembles clean under `makensis -WX`. Its arithmetic is separately verified under wine
against real `lua` — see `verification/semantics/`, six boundary cases, six matches.

Where the operator and stdlib rules stop being tables and start being arguments.

## What it settled

**The paired sign fixup costs one branch, not two.** With a positive constant divisor,
`//`'s correction condition and `%`'s correction condition are the same test —
`remainder < 0` — so a program computing both shares one compare:

```nsis
  IntOp $4 $3 / 64
  IntOp $5 $3 % 64
  IntCmp $5 0 _generated_sign_8 0 _generated_sign_8
  IntOp $4 $4 - 1
  IntOp $5 $5 + 64
_generated_sign_8:
```

Five instructions and one branch for both operations, against the estimate of "roughly
three instructions and a branch per operation". Since a program that wants a quotient
usually wants the remainder too, the paired case is the common one and the cost
estimate is pessimistic.

**`freeMib // 1024` emits no fixup and `delta // 64` does**, from the same source
construct, because the declaration types `driveSpace`'s output as `uint` and `delta` is an
ordinary `int`. That is the lattice earning its keep on the first real program, and it
is the answer to the open empirical question about how often `unknown` shows up: see
below.

**`lower(a) == lower(b)` really is a bare `StrCmp`.** No `${StrCase}`, no temporary, no
`StrFunc` dependency. And the contrast is visible one line later: `string.upper(channel)`
for its *value* costs `${StrCase} $1 "$0" "U"`. Same function, two lowerings, chosen by
whether the result is compared or used — which is the peephole that was promised.

**`${Using:StrFunc}` lines, collected and emitted once.** `StrCase` is reached from two
different places (a section and a `func`); the pass emits one line, at top level. Verified
load-bearing: removing it aborts the build with `You forgot ${Using:StrFunc} …`, which is
`!error`, not a warning.

**`string.find` is a loop, not `${StrLoc}`.** `${StrLoc}` compares with `StrCmp`, which
ignores case, and Lua's `find` does not. So the compiler walks the subject with `StrCpy`
and `StrCmpS`, and the hit is the offset `+ 1`. `string.sub(v, 1, dot - 1)` then wants
`maxlen = dot - 1`, the offset again: the `+1` and the `-1` would cancel, but the compiler
does not fold them yet.

## The type lattice lands on `unknown` zero times

This was the lattice's open empirical question. Across all five programs, every value
is typed:

| Source of type | Instances |
| --- | --- |
| literal | most |
| declaration output (`getSize`, `driveSpace`, `versionCompare`, `execToStack`) | 6 |
| overlay output (`readRegStr` → `string`, `StrLen` → `uint`) | 5 |
| inferred through a user `func`'s returns | 4 |
| **unresolved** | **0** |

That is a five-program sample and the programs were written by someone who knew the
lattice existed, so it is weak evidence. But the mechanism it points at is real: **types
enter almost entirely through declarations**, not through inference over expressions.
Which means the lattice's accuracy is a property of the overlay's and the headers' `ty`
columns being filled in, and `unknown` at a comparison will in practice mean *"this
command's overlay row is incomplete"* far more often than *"this expression is genuinely
ambiguous"*. The diagnostic should say so.

## What it left open

**`string.format` is `IntFmt` only for the integer directives, and this program only uses
one of them.** `%04d` maps cleanly. `%s` is interpolation, which the adapter rule already says, but
`string.format("%s: %04d", name, n)` mixes the two in one call and has to split into an
`IntFmt` plus a template. Not hard, not designed.

**`driveSpace("C:/")` normalises to `C:\`, and that is a path position on a *drive root*.**
The overlay marks path parameters for `/` → `\`, which works here by luck: a drive root is
a path. A UNC root (`//server/share`) would be normalised to `\\server\share`, which is
also right. No action, but worth a test, because the normalisation is a blind
string replace and `http://` in a `File` position would be silently mangled.

## Exposed commands used

`setOutPath` · `readRegStr` · `detailPrint` · `abort` · `string.len` (`StrLen`) ·
`string.lower`/`upper` (`${StrCase}`, or nothing) · `string.find` (`StrCpy`/`StrCmpS` loop) ·
`string.sub` (`StrCpy`) · `string.format` (`IntFmt`) · `//` · `%` · `-` · `>` · `==`

Headers: `FileFunc.driveSpace` · `WordFunc.versionCompare` · `StrFunc` (init lines only)
