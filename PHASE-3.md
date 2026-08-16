# Phase 3 — registers and calls

**Status: complete, with the exit criterion met in the form PLAN allows for it.**

PLAN's exit criterion: *program 4 (multiple returns) emits the hand-written expectation
from Phase 0, **or the expectation is revised with a written reason**.* The reason is
written out in full below, and it is short: program 4 also needs `import`, `${GetSize}`,
`file` and MUI pages, and all four are Phase 4's. Every line of it that Phase 3 owns is
emitted, and the parts that overlap are compared line by line.

| Task | Where | Result |
| --- | --- | --- |
| Liveness-based allocation (§9-3) | [`src/alloc.rs`](src/alloc.rs) | virtual slots, coloured from live ranges |
| Call graph, one traversal (§15.11) | [`src/callgraph.rs`](src/callgraph.rs) | two consumers of three; §15.3 named, not faked |
| Clobber fixpoint on the SCC condensation | [`src/callgraph.rs`](src/callgraph.rs) | recursion needs no special case |
| Caller-saves, `live ∩ clobbered` | [`src/alloc.rs`](src/alloc.rs), [`src/layout.rs`](src/layout.rs) | ascending, restored in reverse |
| Parameters and multiple returns | [`src/lower/`](src/lower/) | the stack ABI, in one place |
| Return-type and parameter inference (§15.14) | [`src/lower/sig.rs`](src/lower/sig.rs) | a whole-program fixpoint |
| Depth-cliff lint (§3) | [`src/callgraph.rs`](src/callgraph.rs) | a warning on every cycle |

```
cargo test          # 45 tests; the makensis one skips cleanly if it is absent
cargo run -- build tests/golden/returns.lua --stdout
```

Tiers 0, 2, 3 **and 4** all run on this phase's work.
[`tests/registers.rs`](tests/registers.rs) asserts the convention as a list of pushes and
pops, [`tests/golden/returns.nsi`](tests/golden/returns.nsi) is diffed by exact equality
and assembled under `makensis -WX`, and
[`verification/semantics/`](verification/semantics/RESULTS.md) runs the compiler's *own
output* under wine and checks the numbers that come back.

That last one is not decoration. Program 4's README says it is the one program where
assembling proves nothing: a caller-save restored one `Exch` out of place gives an
installer that runs and returns a wrong answer. The generated code prints
`kib=0 halves=4 countdown4=10`, and `countdown4=10` is `4+3+2+1` — which only comes out if
the recursive save restores `n` at every depth.

---

## The exit criterion, honestly

Program 4 does not compile today, and nothing in Phase 3 was going to make it. It opens
with `import "FileFunc"`, calls `fileFunc.getSize`, uses `file` and declares MUI pages —
`!include` machinery, the header-declaration format and the overlay, which are Phases 4 and
5 in PLAN's own ordering.

So the exit criterion is checked against the part of program 4 that is *about* Phase 3, as
[`tests/golden/returns.lua`](tests/golden/returns.lua): the same three functions, the same
four call edges, the same recursion, with `${GetSize}` replaced by `string.len`. Against
program 4's hand-written expectation, the generated code differs in exactly three places:

1. **The label prefix**, `__GENERATED_` against the expectation's `_generated_` — ruled
   during this phase, and neither of the two spellings the documents disagreed over.
2. **One register number.** The hand-written `countdown` pops `rest` into `$2`; the
   compiler uses `$1`, because `$1` held the argument and is dead by then. The allocator is
   *tighter* than the hand-written file, which is the outcome §9-3 was aiming at.
3. **The instructions `${GetSize}` would have been**, which is the substitution above.

Everything else is identical, including the whole of the section body:

```nsis
  Push $0        ; save kib
  Push $1        ; save halves
  Push 4         ; argument
  Call countdown
  Pop $2         ; result
  Pop $1         ; restore
  Pop $0         ; restore
```

That is the sequence program 4's README singles out as the one that has to be exactly this
way round rather than merely consistent, and it is emitted line for line.

---

## What the phase decided

### Allocation moved out of lowering entirely, and that is the whole design

Phase 2 handed registers out from both ends of the file and never reclaimed one. Replacing
that with a liveness pass was not an optimisation with a knob: it changed where the decision
is made. Lowering now produces **virtual slots** with no supply to run out of, and
[`alloc`](src/alloc.rs) colours them afterwards from an interference graph.

nsL's issue #5 — a user's variable handed out as a temporary — is not a bug in a particular
call to `setInUse(false)`. It is what tracking ownership by convention *is*. A virtual slot
cannot be handed out twice because it is never handed back, and two slots share a register
only when a computed live range says they may.

The visible payoff is that `control-flow.nsi` now uses `$0`–`$1` where it used `$0`–`$4`
and `$R9`, with no change to the source. The invisible one is that "is this register free"
stopped being a question anybody asks.

### A call is not an instruction, because its instructions are not known when it is lowered

The saves around a call are `live ∩ clobbered`, and neither half exists until colouring has
run and the interprocedural fixpoint has propagated. So a block holds
[`ir::Step`](src/ir.rs)s rather than instructions, and a call is two of them: `Saves(n)`
marks where the pushes go, `Call(n)` marks the call itself, and [`layout`](src/layout.rs)
expands both once `alloc` has filled the site in.

They are two steps rather than one because the saves go in **before the argument
evaluation**, not merely before the argument pushes. That is what keeps the callee's
results on top of the stack when it returns, so the restores fall out underneath them
without a single `Exch` — §15.11 does not say where saves sit relative to arguments, and
program 4 pins it.

### Types are a whole-program fixpoint, because there is nowhere else to get them

§15.14 rules that types are inferred and never annotated. That means a parameter's type
comes from the call sites, which are usually below the declaration, and a return type comes
from the body, which the call sites are above. There is no pass order that resolves both.

So [`lower`](src/lower/mod.rs) runs repeatedly against a signature table until it stops
changing, with the non-final rounds reported into a scratch collector. Three rounds settle
the deepest chain in the five programs. Without it `countdown(n)` cannot know `n` is an
int, and `n - 1` has no lowering at all — this is load-bearing rather than a refinement.

It also closes Phase 2's third open item for free: a global's type is now carried through
the same fixpoint, so a body lowered before the assignment that types a global sees the
right type. §15.6 promised order-freedom and Phase 2 delivered it only for values that
folded.

### The clobber fixpoint, and why recursion is not a special case

Clobber sets propagate over the SCC condensation, callees first, with every member of a
component receiving the component's union. A recursive cycle is then a component whose
members all clobber whatever any of them clobbers — which is exactly §15.11's claim that it
saturates in one extra round, now with an algorithm rather than an assertion behind it.

Tarjan's algorithm emits components in reverse topological order already, so no separate
sort exists to get wrong.

The consequence is visible in `countdown`: the recursive call site saves `$0`, and it does
so *because* the fixpoint went round the cycle and put `$0` into `countdown`'s own set.
Delete that one `Push` and the installer still assembles, still runs, and answers `4`.

### The saved set is an intersection, and the tests say so in both directions

`a_dead_value_is_not_saved` is the one that matters. The alternative every simple NSIS code
generator reaches for — treat `Call` as clobbering all twenty registers — is correct, four
lines, and emits twenty pushes where this emits none. Against a register file of twenty
that is not a constant factor.

[`tests/registers.rs`](tests/registers.rs) checks a value live across a call *is* saved, a
dead one is not, and a value the callee never touches is not — three cases, because a save
set that is always empty and a save set that is always full both pass any one of them.

### A dropped return still costs a slot

`local size, files, _ = …` uses two of three outputs, and `two()` as a statement uses none
of two. The callee pushed them either way, so they have to come off, and the *declaration's*
arity decides rather than the call site's. That is §11's "plural outputs are invisible at
the call site" as a line of code.

The popped-and-ignored register is a genuine definition as far as liveness is concerned, so
it cannot collide with something live across the call — which it would, if the pops were
emitted without the allocator knowing about them.

---

## Design changes and things worth recording

1. **`ir::Piece::Slot` and `ir::Arg::Dest` are new, and the split is what makes liveness
   possible.** Phase 2 wrote register names into strings at lowering time — `Arg::raw("$0")`
   for a destination, `Piece::Var("$0")` for a read — which is legible and unanalysable. A
   `$INSTDIR` spliced into a template and a `$0` the allocator owns are now different
   things in the type, and the whole def/use surface is four short methods on
   [`ir`](src/ir.rs).
2. **`RegisterExhaustion` now reports a fact about the program.** Twenty-one *declarations*
   no longer trigger it; twenty-one values *live at once* do. The diagnostic changed with
   it, because the old wording blamed the allocator and the allocator is no longer at
   fault.
3. **The depth-cliff lint fires on every cycle, including bounded ones.** `countdown(4)`
   is obviously bounded and warns anyway, because nothing here can prove it. §15.11 asked
   for the warning and §3 measured why — a silent process death at ~1300 frames, no dialog,
   no log line, no error level — so the noise is deliberate. What is missing is a way for a
   user to say "I know", and that wants a ruling on suppression syntax before it wants code.
4. **Nested calls save the same register twice.** `twice(twice(3))` pushes the caller's
   live value once for each call, and the inner one is redundant because the outer save is
   already on the stack. Correct, and one push more than necessary; removing it means
   modelling saved copies as spill locations, which is a real allocator feature rather than
   a peephole.
5. **§15.3's uninstaller reachability is named in the call graph and not implemented.** It
   is the third of §15.11's three consumers, and `uninstaller {}` has no lowering yet.
   Writing the consumer first would be inventing the answer to a question nobody has asked.
6. **The three opaque callees have no producer yet.** §15.11 says `plugin`, `System::Call`
   and `raw` clobber everything. None of the three can be lowered in this version, so
   rather than shipping an unreachable `Clobber::All` branch the rule is recorded here and
   lands with the callee that needs it. `builtins.rs` is where it will go, as a column.

   **Landed in Phase 4**, and not as a column: all three became a *call site*
   (`ir::CallKind::Opaque`) rather than an instruction, because the saves around one are
   still `live ∩ clobbered` and still have to be decided after colouring. The clobber half
   is four lines in `alloc::insert_saves`.
7. **`tests/control_flow.rs` became `tests/goldens.rs`.** One harness, a table of programs,
   and each row carries the diagnostics it is expected to raise — exactly, in both
   directions. That is how the depth-cliff warning on `returns` is tested rather than
   tolerated.

---

## Still open, and named rather than buried

- **The fixpoint has a round cap rather than a termination proof.** `Unknown` sits at the
  top of the lattice, so a disagreement can flip a slot back and the iteration is not
  strictly monotone. Eight rounds, then compile against the last table — sound, because an
  unsettled type is `Unknown` and `Unknown` is refused wherever guessing would matter. A
  proof wants the "not yet observed" and "two observations disagreed" states separated in
  `Ty` itself rather than in `Option<Ty>` beside it.
- **Colouring is greedy, in slot order.** Optimal colouring is NP-hard and twenty registers
  is not where the wins are, but greedy means `RegisterExhaustion` can in principle fire on
  a body that a better allocator would fit. No program in the five is anywhere near it.
- **Nothing spills.** When twenty registers are not enough the answer is a diagnostic, not a
  spill to the stack. That is the right first answer — a spilled installer variable is
  invisible in the output, and the output is the only debugger anyone has (§9-6) — but it
  is a ceiling rather than a limit.
- **Phase 1's note is finally retired.** The clobber fixpoint is no longer exercised once
  across all five programs: `a_cycle_shares_one_clobber_set` builds a graph by hand with a
  cycle in it, which is the synthetic IR-level test PLAN asked for and Phase 2 could not
  write.
