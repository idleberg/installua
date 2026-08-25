# Two runtime checks, because assembling is not correctness

Phase 0. Both of these produce plausible-looking NSIS when wrong, which is exactly
the failure mode `makensis -WX` cannot see. They are tier-4 tests — run under wine,
skip cleanly without it.

## `//` and `%` match Lua, including the signs

An operator Installua *spells* like Lua must *behave* like Lua, and the design pays
for it with a fixup: NSIS's `IntOp /` truncates toward zero and its `%` takes the sign of
the dividend, where Lua floors and takes the sign of the divisor.

`sign-fixup.nsi` runs the exact pattern program 5 emits over six boundary cases, and the
answers are compared against real `lua` 5.5:

| | `a // b` | `a % b` | Lua |
| --- | --- | --- | --- |
| `7 // 2` | 3 | 1 | ✅ |
| `-7 // 2` | -4 | 1 | ✅ |
| `-8 // 2` | -4 | 0 | ✅ |
| `0 // 2` | 0 | 0 | ✅ |
| `-1 // 64` | -1 | 63 | ✅ |
| `65 // 64` | 1 | 1 | ✅ |

Six for six. `-8 // 2` is the case that catches a fixup applied unconditionally rather
than only when the remainder is non-zero, and `-1 // 64` is the one that catches a fixup
testing the *quotient's* sign instead of the remainder's.

**And both fixups share one compare.** With a positive constant divisor the condition
`remainder ≠ 0 and sign(remainder) ≠ sign(divisor)` collapses to `remainder < 0`, which is
the same test for `//` and for `%`. So the pair costs one `IntCmp` and two `IntOp`s
between them, not two branches:

```nsis
  IntOp $4 $3 / 64
  IntOp $5 $3 % 64
  IntCmp $5 0 _generated_sign_8 0 _generated_sign_8
  IntOp $4 $4 - 1
  IntOp $5 $5 + 64
_generated_sign_8:
```

The estimate was "roughly three instructions and a branch per operation". For the paired
case it is five instructions and one branch for **both**, which is cheaper than the
estimate — and the pairing is common, since a program computing `a // b` usually wants
`a % b` too.

## The calling convention round-trips

`calling-convention.nsi` is program 4 with the UI stripped and the result written to a
file. It exercises the stack ABI end to end: two-value returns, a nested call, a
self-recursive call, and a caller-save of two live registers across the recursion.

```
files=1 kib=0 countdown4=10
```

`countdown(4)` is 10, so the recursive caller-save restores `n` correctly at every depth —
the failure this checks for is a register restored one `Exch` out of place, which produces
an installer that runs and gives a wrong number. `files=1` and `kib=0` are the `${GetSize}`
outputs for a 14-byte payload, arriving in the right order through two levels of
stack-passed returns.

## Reproducing

Needs `makensis` and `wine`.

```
cd "$(mktemp -d)" && makensis -WX <path>/sign-fixup.nsi && wine fixup.exe && cat fixup-result.txt
```

Compare against `sign-fixup-result.txt` and `calling-convention-result.txt`.
`calling-convention.nsi` needs `assets/payload.bin` from `examples/04-multiple-returns/`
beside it.

## The *generated* convention round-trips too — Phase 3

`calling-convention.nsi` above is hand-written: it proves the convention is sound, not
that the compiler implements it. `returns-generated.nsi` is `installua build`'s own output
for the equivalent program, with three lines changed so the answers land in a file:

```lua
attributes { name = "Returns", outFile = "returns.exe" }

func("measure", function(dir)
	local n = string.len(dir)
	return n, n // 2
end)

func("budget", function(dir)
	local size, half = measure(dir)
	return size // 1024, half
end)

func("countdown", function(n)
	if n <= 0 then
		return 0
	end
	local rest = countdown(n - 1)
	return rest + n
end)

installer {
	section("Core", function()
		local kib, halves = budget("abcdefgh")
		local total = countdown(4)
		detailPrint("kib=" .. kib .. " halves=" .. halves .. " countdown4=" .. total)
	end),
}
```

```
kib=0 halves=4 countdown4=10
```

All three numbers are load-bearing. `halves=4` is the *second* of two stack-passed returns
arriving through two levels of call, so an argument or return pushed in the wrong order
swaps it with `kib`. `countdown4=10` is `4+3+2+1`, which only comes out right if the
recursive caller-save restores `n` at every depth — the compiler emits `Push $0` before the
recursive call because the fixpoint put `$0` in `countdown`'s own clobber set, and
dropping that one line gives `countdown4=4` on an installer that still runs.

`kib=0` is `8 // 1024`, and it carries the sign lattice: `string.len` is non-negative by
construction, that travels out through the return type, and neither `//` in the program
pays the sign fixup. The generated `budget` is one `IntOp`.

### Reproducing

```
cd "$(mktemp -d)" && makensis -WX <path>/returns-generated.nsi && wine returns.exe \
  && cat returns-result.txt
```

Compare against `returns-generated-result.txt`.

## Phase 4 — the string adapters and the sign fixups

`strings.lua` → `strings-generated.nsi` → wine. One hand-edited line, `SilentInstall
silent`, because no attribute exposes it yet; everything else is the compiler's output
unmodified.

```
major=2
find=2
sub=bcd
upper=BETA
lower=beta
fmt=0042
div=-8
mod=8
```

Every line has a plausible wrong answer that `makensis` accepts without a word, which is
why this is tier 4 and not tier 3.

**`find=2` and `major=2` are the two off-by-ones, and they are separate facts.**
`${StrLoc}` answers `1` for the `.` in `2.1.0`, counting from zero; Lua's `string.find`
counts from one, so the adapter adds one and `find=2` is the *Lua* answer. `major=2` then
goes the other way: `string.sub(v, 1, dot - 1)` becomes `StrCpy $0 $0 <length> <offset>`,
where a length and an offset are not the two positions Lua wrote. Getting one of the two
right and the other wrong yields `"2."` or `""`, and both assemble.

**`div=-8` and `mod=8` are the Lua answers, and they are the numbers NSIS gets wrong.** `-504 // 64`
truncates to `-7` and `-504 % 64` to `-56` under `IntOp`; Lua floors and takes the sign of
the divisor, giving `-8` and `8`. The compiler emits the fixup here — and does not emit it
for `string.len(s) // 1024` in program 4 — because `string.len` is non-negative by
construction and the subtraction on line 44 is where the lattice loses that.

**`fmt=0042` is `IntFmt`**, which is the whole of `string.format` that NSIS has: one
integer and one specifier. `%s` and several arguments are `Class::Todo`.

### Reproducing

```
cd "$(mktemp -d)" && makensis -WX <path>/strings-generated.nsi \
  && cp <path>/strings.exe . && wine strings.exe && cat strings-result.txt
```

Compare against `strings-generated-result.txt`.
