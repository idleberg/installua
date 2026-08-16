# Phase 2 — resolve, types, IR

**Status: complete.** The exit criterion is met and asserted.

PLAN's exit criterion: *`a and (b or not c)` allocates **zero temporaries**, asserted as
an IR property rather than inferred from the absence of a `StrCpy` in output text.*

| Task | Where | Result |
| --- | --- | --- |
| Name resolution, order-free (§15.6, §12) | [`src/resolve.rs`](src/resolve.rs) | every top-level name before any body |
| Type lattice (§15.14) | [`src/types.rs`](src/types.rs) | `int{width,sign} \| string \| bool \| handle` |
| CFG with explicit terminators (§8) | [`src/cfg.rs`](src/cfg.rs), [`src/layout.rs`](src/layout.rs) | one pass knows what a label is |
| Condition fusion (§8) | [`src/lower/expr.rs`](src/lower/expr.rs) | three callers, one `branch` |
| The exposed surface, seeded | [`src/builtins.rs`](src/builtins.rs) | 9 builtins, 18 constants, in §15.23's shape |

```
cargo test          # 34 tests; the two makensis ones skip cleanly if it is absent
cargo run -- check examples/*/install.lua
cargo run -- build tests/golden/control-flow.lua --stdout
```

Tiers 0, 2 and 3 all run on this phase's work. The exit criterion is
[`a_fused_condition_allocates_no_temporaries`](tests/cfg.rs), and
[`tests/golden/control-flow.nsi`](tests/golden/control-flow.nsi) is diffed by exact
equality and then assembled under `makensis -WX` with an empty warning allowlist.

---

## What the phase decided

### The exit criterion needed a companion test to mean anything

`body.temps == 0` is only evidence if something can move it. A compiler that emits nothing
allocates no temporaries either. So `a_nested_expression_does_allocate_one` sits directly
beneath it and asserts the counter reaches 1 for
`detailPrint("length " .. string.len(path))` — the same discipline as Phase 1's
`the_whitelist_rejects`, and for the same reason: a measurement with no negative case is a
tautology wearing a test's clothes.

### Reverse postorder, not the depth-first walk that looks equivalent

The obvious layout — walk depth-first from the entry, fallthrough successor first — puts an
`if`'s **entire else-arm after the end of the body**, because visiting the then-arm visits
the join block and everything downstream of it first. The first version of this compiler did
exactly that, and the output had `Return` in the middle with two orphaned arms trailing it.

Reverse postorder places a block after the blocks that reach it, which puts a join after
both arms and a loop's exit after its body. Successors are then walked *in reverse* — the
else-arm first — precisely so that reversing puts the then-arm first, which is what makes
`IfFileExists "…" 0 __GENERATED_endif_0` the shape rather than its mirror.

### A `bool` is `1` and `0`, and it is a `StrCmp`

NSIS's own flag-shaped values are `1` and `0`, so an `IntOp` over a materialised `bool`
needs no conversion. The test is `StrCmpS` rather than `IntCmp` for the simple reason that
`StrCmp` has two arms and a `bool` has two values; `IntCmp`'s third arm would be dead.

### Sign is an attribute, and it earns its place by deleting instructions

`string.len(p) // 2` is **one** `IntOp`. `(string.len(p) - 5) // 2` is nine lines: quotient,
remainder, two compares and an adjustment, because NSIS truncates toward zero and Lua floors
and they disagree exactly when one operand is negative (§15.4).

Both are tested, and the pair is the argument for §15.14's sign axis. `StrLen` is
non-negative *by construction*, so the fixup there is not elided on a hunch — it is provably
unnecessary. Subtraction is the one arithmetic operator that manufactures a negative out of
two non-negatives, and it is the whole difference between the two cases above.

The fixup itself uses one trick worth writing down: exclusive-or of the operands has its
sign bit set exactly when their signs disagree, so `IntOp $s $s ^ $b` plus one `IntCmp`
replaces what would otherwise be four comparisons.

### Concatenation never needs a destination

`"length " .. string.len(path)` allocates **one** register, not two. The first version
lowered a concatenation like any other binary operator — evaluate into a destination — which
meant an outer temporary for the result plus an inner one for the call. But a template is
assembled *in the argument*: only the side that computes something spends a register, and
the join costs nothing at all. This is §5's string model paying for itself, and it is why
`detailPrint("into " .. INSTDIR)` is one line with no `StrCpy` anywhere.

### Two argument shapes are emitted unquoted

`IntOp $3 "$2" / "2"` is valid NSIS and nobody wants to read it. A `Data` argument that is a
single variable, or whose text is a bare integer, is emitted bare. Both are provably safe —
NSIS parses `$0` and `"$0"` identically, and an integer has nothing in it to quote — and the
output is the only debugger anyone has (§9-6).

### `<const>` folds and emits nothing

§12's emission table lists `<const>` → `!define`, and §15.6's worked example says a
`<const>`-guarded `if` puts *"nothing at all"* into the `.nsi`. Folding at every use
satisfies the second and makes the first unnecessary, so no `!define` is emitted. That is a
deliberate reading of a document that says both things, and it is reversible: the moment
`raw` needs to see a constant by name, the `!define` becomes a *use* rather than a
speculative declaration. Recorded here so the choice is visible rather than inferred from
`module.defines` being empty.

---

## Design changes and things worth recording

1. **The generated-label prefix is `_luagen_`, and the documents disagree.** §15.19's
   heading and every example in it say `_luagen_`; §15.25's prose and PLAN's own opening
   note both say `_generated_`. §15.19 is the section that declares itself *"this section is
   the spelling"*, so that is what shipped. It is one constant —
   [`cfg::LABEL_PREFIX`](src/cfg.rs) — and changing it rewrites two golden files, which is
   exactly the churn §15.19 argued against for the *product* name and accepted for this one.
   **Ruled during Phase 3: `__GENERATED_`** — neither spelling, uppercase and
   double-underscored so a generated label is unmistakably not a user's line in a diff. The
   two golden files were regenerated and the constant is unchanged in kind.
2. **`Ty::Unknown` has no producer yet, so it has no dedicated diagnostic.** §15.14 wants
   unknown-at-a-comparison to be a hard error naming both operands, and it is — but as
   `type-mismatch`, whose message names both types. A separate code would fail the registry
   test, because nothing in this phase can *make* an unknown: types come from the builtin
   table, and the only source of a genuinely unknown one is a call whose return needs the
   interprocedural fixpoint. That is Phase 3, and the code should be split then.
3. **A global's type is learned as bodies are lowered, so it is order-free only when the
   value folds.** `state = "installed"` is typed from the literal during resolution and is
   visible everywhere. A global whose first assignment is a computed value is `unknown` to
   any body lowered before that one. The honest fix is a second pass, and it is the same
   fixpoint §15.11 already commits to building.
4. **The unreachable-code warning §8 promises for free is not taken.** Dead blocks disappear
   silently. Taking the warning would fire on every `<const>`-guarded block, which §15.6
   presents as the *intended* idiom, so it needs a way to tell "you folded this deliberately"
   from "this cannot be reached" first.
5. **Loop rotation is not done.** §8 notes that testing at the bottom saves a jump per
   iteration; the loops here test at the top and pay a `Goto` per iteration. Purely an
   optimisation, and it changes both goldens when it lands.
6. **A `for` bound spills to a local that is never reclaimed.** Lua evaluates the bound once,
   and a temporary does not survive the loop, so a non-constant bound costs a register for
   the whole body. Correct, and wasteful in exactly the way §9-3's liveness pass exists to
   fix.
7. **A predicate used as a bare statement is rejected.** `errors()` on its own line has an
   answer nobody reads, and it cannot be lowered as a branch with both arms at the same
   place without emitting a jump to the next line. The diagnostic names `local e = errors()`
   and says the call is never optimised away, which is the part §15.20 verified under wine.

---

## Still open, and named rather than buried

- **§15.14's empirical question is still unanswered.** *How often does the lattice really
  land on `unknown`?* Phase 0 was meant to find out from the five programs, and Phase 2
  cannot: almost none of the v1 command surface is lowered, so the five reach the type
  system barely at all. The measurement arrives with Phase 4.
- **`func` takes no parameters and returns nothing.** Both need the stack ABI and the
  clobber analysis — Phase 3 — and inventing half of either here would be inventing it in
  the wrong place.
- **`and`/`or` as values is checked only where a side resolves without an instruction.**
  Nothing in the current builtin table can produce a non-`bool` from a call, so the hole is
  unreachable rather than latent; it closes when the check moves onto the lowered type
  instead of the syntactic one.
- **No comment carries a source span into the output.** §8 asks for
  `; while i < 10   @ install.installua:42` per construct. That is the readable half of
  §15.22's line map, and both belong to Phase 4 together.
- **Phase 1's note still stands: the clobber fixpoint is exercised once across all five
  programs.** Phase 3 needs synthetic IR-level tests, and now has an IR to write them
  against.
