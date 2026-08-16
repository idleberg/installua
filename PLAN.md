# Installua — build plan

`PREPLAN.md` is the design: 31 rulings, most of them verified against real `makensis`.
`LANGUAGE.md` is the surface reference. **This file is the build order**, and nothing here
re-argues a decision — where a phase depends on a ruling it cites the section and moves on.

Provisional name. §15.19 made generated labels carry a neutral prefix rather than a product
name precisely so a rename churns no golden file, so this costs nothing to defer. Phase 3
settled the spelling as `__GENERATED_`; the `_generated_` in the hand-written `expected.nsi`
oracles is Phase 0's guess, kept as written (PHASE-3.md).

---

## 0. What v1 is

**A vertical slice.** Every pass exists and is correct; the *exposed* instruction set is
only what §11's five programs reach. Everything else is an honest `Class::Todo` in the
census, counted and printed by `installua coverage`.

The alternative — completing the 276-command overlay before the compiler — spends months
on data entry before the first installer builds, and the real risk in this design is not
coverage. It is the interprocedural clobber fixpoint, condition fusion with zero
temporaries, the `//` sign fixups and the line map. Those bugs surface on program 1, not on
command 200. And §15.23's parameter struct plus §14's mandatory example pair make adding a
command afterwards one table row and one test, by construction.

**Done means:** the five programs compile, assemble under `makensis -WX` with an empty
warning allowlist, and the two that warrant it run correctly under wine.

---

## 1. Phases

Each phase states its exit criterion. A phase is not done until its criterion is
mechanically checkable and checked.

### Phase 0 — Specification by example

No Rust. This phase produces the artifacts every later phase is measured against.

1. **Verify §1.** Install `lua-language-server`, `selene`, `stylua` and check every claim
   §1 makes — highest-priority item in the plan, see §4 below.
1b. **Verify the large-string plugin hazard** (§15.31). A plugin is compiled against
   `NSIS_MAX_STRLEN`, and NSIS ships no directory convention distinguishing it the way
   `Plugins/x86-unicode` distinguishes charset — so if the failure is real, §13's cheap
   plugin check cannot cover it and the docs must say so. Currently unverified.
2. **Write the five programs** (§11), source and hand-written expected `.nsi` both:
   MUI installer with uninstaller · plugin-heavy · file-iteration loop · multiple returns ·
   string and integer manipulation.
3. **Assemble all five hand-written `.nsi`** under `makensis -WX`. They are the oracle; if
   the expectation does not assemble, the expectation is wrong.
4. **Two migration tables** (see §5) — the Lua-facing one §11 asks for, and the
   NSIS-facing one. *(Revised after Phase 4: the instruction half of the NSIS-facing table
   is now owned by Phase 5's retired-instruction diagnostic, not by a documentation page.)*

**Falls out of this phase, and is why it comes first:**

- the frozen v1 **exposed-command list** — the definition of scope
- the `MUI_*` subset that is actually needed, out of roughly seventy
- §15.14's open empirical question: how often the type lattice really lands on `unknown`
- the first five goldens

**Exit:** five `.nsi` assemble clean under `-WX`; exposed list frozen; §1 verified or its
failures documented as design changes.

> **This phase is the plan's weak point.** It is days of writing whose payoff is entirely
> front-loaded, and if it slips everything slips. Mitigation: do program 1 first and treat
> "program 1 assembles clean" as a checkpoint worth stopping at to reassess.

### Phase 1 — Skeleton, frontend, and a thin spine

Commit the PoC first so it is recoverable by tag, then rebuild `src/`.

- **Library crate, thin binary** (§15.12). No `static mut`, no thread-local diagnostic
  sink, no `RegisterList::getCurrent()` in any spelling — §9-2 is an architectural
  requirement, because nsL's statics are exactly why it can never be an LSP backend.
  Everything the CLI does must be doable in-process, re-entrantly, against an in-memory
  string.
- **Diagnostics as data** (§9-4): span, code, severity, notes. Collected, never thrown.
  Report all of them.
- **Frontend**: `full-moon` parse → whitelist pass → **escape-sequence validation**
  (§13 — `full-moon` accepts `"C:\Program Files"`, real Lua does not, so the frontend must
  check) → `elseif` desugaring → float-literal rejection.
- **The thin spine.** The narrowest possible end-to-end path — `attributes {}`, one
  `section`, `detailPrint`, and nothing else — emitted and assembled.

**Why the spine is here and not in Phase 4:** without it nothing is assembled between
Phase 0 and Phase 4, which is the longest blind stretch in the plan. With it, `makensis -WX`
is a live gate from week one and every later phase widens a pipeline that is already
verified against the oracle.

**Exit:** `installua check` clean on all five programs; `installua build` produces an
assembling `.nsi` for the spine subset.

### Phase 2 — Resolve, types, IR

- **Name resolution, order-free** (§15.6, §12). Every top-level name resolved before any
  body is lowered.
- **Type lattice** (§15.14): `int | string | bool | handle`, with `int` carrying
  `width` and `sign` as attributes rather than splitting into types. Unknown at a
  comparison is a hard error naming both operands.
- **CFG of basic blocks with explicit terminators** (§8) — `Jump`, `Branch`, `Return`,
  `Unreachable`. One layout pass has sole knowledge of what a label is. Labels reset per
  body (§15.25).
- **Condition fusion** (§8): `emitBranch(expr, Ltrue, Lfalse)`, recursing on `and`/`or`,
  `not` swapping destinations. Three callers, one mechanism — comparisons (§8),
  predicates (§15.20), `messageBox` (§15.18).

**Exit:** `a and (b or not c)` allocates **zero temporaries**, asserted as an IR property
rather than inferred from the absence of a `StrCpy` in output text.

### Phase 3 — Registers and calls

The phase with the most novel work in it.

- **Liveness-based register allocation** (§9-3). Not `setInUse(false)`; nsL's issue #5
  exists purely because ownership was tracked by convention.
- **Call graph, built once, read three times** (§15.11) — clobber sets, uninstaller
  reachability (§15.3), return-type inference (§15.14).
- **Clobber-set fixpoint** on the SCC condensation. A recursive cycle needs no special
  case: it is an SCC whose fixpoint saturates in one extra round.
- **Caller-saves** at each call site: push `live ∩ clobbered`, ascending register number,
  restored in reverse — deterministic because §14 diffs goldens.
- **Three opaque callees clobber everything**: `plugin`, `System::Call`, `raw`.
- **Depth-cliff lint**: warn on unbounded recursion, naming the iterative form. §3 measured
  ~1300 frames under wine, then the process dies **silently** — no dialog, no log line,
  no error level.

**Exit:** program 4 (multiple returns) emits the hand-written expectation from Phase 0, or
the expectation is revised with a written reason.

### Phase 4 — Emit, map, assemble

- **Emission order** (§12): `Unicode`, remaining attributes, `!include`s and their init
  lines, `Var`s, functions, sections. `Unicode` leads so a later `raw` overrides it rather
  than being silently overridden (§15.16).
- **Data versus syntax** (§12): the emitter cannot quote uniformly, and escaping applies
  only to the data half.
- **`StrFunc` init collection** (§15.21): a collect-then-emit pass over `(macro, namespace)`
  pairs, sharing the `!include` dedup machinery. Load-bearing — a missing
  `${Using:StrFunc}` aborts the build.
- **Line map** (§15.22): `Origin::{User, Raw, Emitted}` sidecar. `installua build` owns the
  `makensis` invocation and rewrites diagnostics through it. An `Emitted` line failing is
  by definition a compiler bug and says so, retaining the `.nsi`.

**Exit:** all five programs assemble under `-WX`, empty warning allowlist; one mapping test
per `Origin` kind.

### Phase 5 — The table and the ecosystem

- **`-CMDHELP` generator** → `params`/`options` skeletons, checked in as generated code.
- **The join** (§15.23): generated half plus hand-written half (`installua` name, `class`,
  `ty`, `kind: Path`, `conflicts`) into one `Instruction`. Two files, never two runtime
  tables — a consumer that consults both can silently see half an entry.
- **Census** (§14): every `-CMDHELP` line lands in exactly one `Class`; unclassified fails
  the build. Plus the join test in both directions.
- **`installua stubs`** → the `---@meta` file, **plus the project meta file** that
  `include` requires (§15.28).
- **`installua coverage`**, **`installua init`**, the generated selene std.
- **The retired-instruction table** (see §5) — the NSIS-facing migration table as a
  diagnostic rather than a documentation page.

**Exit:** census green, `coverage` output is itself a golden file; every retired-instruction
row has a test asserting its diagnostic.

#### The retired-instruction table

Some NSIS instructions are deliberately not ported because Lua already has the better
spelling: `StrCmp`, `IntOp`, `StrCpy`, `StrLen`, `IntCmp` and their kin. Today they resolve
to nothing and get the generic unknown-name error, which tells an NSIS user that the
compiler has never heard of the single instruction they use most.

Extend the mechanism that already exists. `src/builtins.rs` has `nearest()`, which turns
`detailprint` into "did you mean `detailPrint`?". Add a curated table feeding the same path,
but producing a rejection that names the *replacement construct* — already §2's rule for
every rejection:

```
error[nsis-retired]: `StrCmp` is not a function here
  note: write `a == b`, which is case-sensitive (§15.9)
  note: for the case-insensitive comparison `StrCmp` does, write
        `string.lower(a) == string.lower(b)`
```

**Keyed on the Installua spelling, matched case-insensitively.** The rows are `strCmp`,
`intOp`, `strCpy` — the camelCase shape the Lua equivalent *would* have had, not NSIS's
`StrCmp`. That is the same convention `nearest()` already uses for the names that do exist,
so one casing rule covers both tables, and the case-insensitive match still catches the NSIS
capitalisation the user actually typed. The diagnostic quotes the user's own spelling.

Zero new language surface, no census entry, no deprecation lifecycle, and it does not
autocomplete — the table feeds diagnostics only, never the stubs or the selene std. One
static table, one diagnostic code, one registry test.

### Phase 6 — Coverage grind

Post-v1. Parallelisable, mechanical, and explicitly not the same job as phases 1–5: each
command is one overlay row plus its mandatory example pair. The `todo` bucket is the
backlog and `installua coverage` is the burndown.

#### What the slice does and does not make cheap

The vertical slice is a bet that v1's mechanisms generalise. They do — but only over
**command surface**, and it is worth being exact about the boundary, because assuming
everything past v1 is transcription is how a `todo` count stops predicting effort.

**Data entry after v1** — one overlay row and one example pair, no compiler change. Any
command whose shape is *n* parameters with directions, optional slots, enums, flags and
repetition. The large majority of the remaining ~220, because §15.23's `Param`/`Opt` model
was designed against the whole `-CMDHELP` set rather than against the twelve the PoC used.

**Still design work after v1:**

| Not just a row | Why |
| --- | --- |
| Commands with their own control-flow shape | `MessageBox` took a whole ruling (§15.18) — statement, flag set and jump table at once |
| The `Section*` family | `SectionGetFlags`/`SetFlags` address sections **by index**, a real compile-time ↔ install-time name binding (§13) |
| The ~70 `MUI_*` settings | data-shaped, but each needs a *home* (`installer {}` vs `page {}`), and page-scoped ones carry §15.7's sequential-`!define` hazard |
| Custom pages / nsDialogs | no design exists — plugin calls, callbacks and a layout model |
| New kind-2 stdlib adapters | `string.sub`/`format`/`math.*` are hand-written lowerings onto instructions, not declarations (§15.21) |
| A fifth lattice type | four is a closed set; a fifth touches every rule in §15.14 |

**And the deferred language features were never covered by the groups at all** — section
index output, code-splitting via `include`, `truncDiv`, the module form of the API. v1 makes
none of those cheaper.

So: **v1 makes command surface cheap and leaves language surface hard.** That is the right
way round, since command surface is the part that is large.

---

## 2. Testing

§14 is the strategy; this is the operational form. **Cheapest first, and tier 3 is live
from Phase 1.**

| Tier | What | When | Needs |
| --- | --- | --- | --- |
| 0 | Pass boundaries — frontend / IR shape / emit text, each asserted separately | always | — |
| 1 | Overlay example pairs, table-driven (§14: omission is unrepresentable) | always | — |
| 2 | Golden `.nsi` diff, exact equality over the smallest region containing the behaviour | always | — |
| 3 | **Assemble with `makensis -WX`**, empty warning allowlist | always | `makensis`, skip cleanly if absent |
| 4 | Run the built installer under wine, assert behaviour | the handful that warrant it | wine, skip cleanly |
| 5 | Census against the checked-in snapshot; snapshot refresh diffed against local `makensis` | always / when present | — |

Rules that are not negotiable, because each has a named failure mode:

- **Warnings are failures.** A `$`-sigil mistake, a mis-ordered `!define` and an unknown
  `${FOO}` are all warning 6000 plus a silently wrong installer. A test checking only the
  exit code passes on precisely the bugs this compiler exists to prevent.
- **`assert!(out.contains(…))` is banned** in favour of exact equality over a scoped
  region. `contains` passes on output carrying an extra spurious line, which is the failure
  mode of a compiler emitting one `StrCpy` too many.
- **Every diagnostic code has a test that produces it**, enforced by a registry-walking
  test, and **every rejection names its replacement** — generated from the same tables that
  generate the stubs and the selene std.
- **Multi-error tests**, since collect-don't-throw is only observable when more than one
  error exists.
- **The §13 verifications are tests**, not footnotes. Each is a claim about NSIS a future
  version can quietly break.

---

## 3. Deferred, with shapes recorded

Deferral is a decision here, not an omission. Each is additive by §15.17's widening
argument, so none closes a door.

| Item | Shape if it lands | Why deferred |
| --- | --- | --- |
| `truncDiv` / `remainder` intrinsics | plain functions, no fixup | §15.14's sign lattice should make the fixup rare; wait for evidence from the five programs |
| `sar(a, b)` | probably never | NSIS's arithmetic `>>` has no Lua operator; `raw` covers it |
| `Name` accelerator opt-out | a second field | nobody has asked; auto-doubling on a constant name covers the real case |
| Section index output | `local id = section("Main", fn)` returning a compile-time symbol | needed only once a program reaches `SectionSetFlags` |
| ~70 `MUI_*` settings | overlay rows in `installer {}` / `page {}` | pure data entry → `Class::Todo`; v1 carries what the five programs reach |
| Multi-file code splitting | `include` (§15.28) | decided, but only `languages {}` forces it in v1 |
| Module form of the API | LuaCATS supports naming a meta file so it is also `require`-able | §15.13 — additive, for the embedded case |
| `System::Call` signature parsing | narrows the opaque-clobber set | §15.11 — a later optimisation, tractable once liveness exists |

---

## 4. Risks

Ordered by expected cost, highest first.

**§1 is unverified, and it is the premise everything rests on.** §13 and §15 verified NSIS
behaviour extensively against real `makensis`. §1 — *stylua works as-is*, *selene's custom
std can flag rejected names*, *`---@alias` gives completion inside string literals*, *LuaLS
warns on an unknown field in `attributes {}`*, *`runtime.builtin = "disable"` turns
`require` back into an unknown global* — has **no** evidence behind it, and none of the
three tools is installed. If `---@alias`-inside-string-literals does not work, §13's "enums
are a gift to the editor" argument weakens across the whole attribute surface. Phase 0,
first task, and cheap.

**Clobber-set correctness.** The fixpoint is the most novel code in the plan and its failure
mode is a wrong installer rather than a crash — a register restored one `Exch` out of place
produces plausible NSIS. Mitigation: assert clobber sets directly at the IR boundary, not
through emitted text, and make program 4's wine run a tier-4 test.

**`string.sub`'s negative arguments.** §15.21 pins the semantics by golden tests over
boundary cases rather than a formula, and explicitly says those tests are the deliverable.
NSIS's negative `maxlen`/`startoffset` are *close to but not identical to* Lua's, so this is
a silent-wrong-answer surface inside an adapter users will reach for constantly.

**`MUI_*` volume.** Roughly seventy defines, each needing a home and an overlay row. Not
design work, but not small, and it is the most likely thing to make v1 feel incomplete.

**Silent string truncation** (§15.31). NSIS truncates over-long strings with **no
diagnostic at all** — verified: an 1100-character literal compiles clean under `-WX` and
measures 1023 at runtime. §14's "warnings are failures" rule cannot help, because there is
no warning to promote. The compiler catches folded literals and constant-folded
concatenations; runtime values are unknowable and stay NSIS's problem. Checked against
1024, the vanilla floor, with `maxStringLength` in `installua.toml` as the opt-in — the
first real key in that file, alongside the plugin search path.

**`makensis` version drift.** The census pins 3.12. That is what makes it meaningful and
what makes it wrong the day 3.13 ships — by design, since the refresh test is what tells
you and the diff is the changelog.

---

## 5. Documentation deliverables

Two migration tables, because Installua has two audiences and PREPLAN only specifies one.

**"Lua-shaped, not Lua"** (§11 asks for this). Divergences a Lua programmer will trip on:
no hoisting rules to worry about because everything hoists · `==` is case-sensitive but
`string.lower(a) == string.lower(b)` is a bare `StrCmp` · `#` rejected on strings · `or`
as a value rejected for non-`bool` · truthiness only for `bool` · no floats, `/` and `^`
rejected · `//` and `%` corrected to Lua's meaning · no closures as values · no
metatables, `pairs`, `require`-as-runtime-load.

**"NSIS-shaped, not NSIS"** — the larger audience. Phase 5's retired-instruction table owns
the half of this that is about *instructions*; the rows below that name a habit rather than
an instruction stay documentation, because no name lookup can fire on them. Making the
instruction half a compiler diagnostic instead of a page is the version that reaches someone
at the moment they need it, with the semantic trap named where it would have bitten.

| NSIS habit | Installua |
| --- | --- |
| `StrCmp` is the one you reach for | `==` is `StrCmpS`; the **case-sensitive** one is the default — the reversal that will bite hardest |
| charset depends on how your `makensis` was built | `Unicode` is always emitted, always first, defaults `true` |
| `\` in paths | `/` accepted and normalised in path positions; `\` needs `\\` or `[[…]]` |
| `Goto` and labels | no spelling at all |
| `$INSTDIR` inside a literal | `INSTDIR` is an ordinary name, joined with `..` |
| `PageEx` / `Page` | MUI2 only; classic pages reach through `raw` |
| `un.` / `Manifest` / `Uninstall` / `.onInit`'s dot | emitted, never written |
| `detailprint` (any casing) | camelCase in, NSIS casing out; the compiler suggests the spelling |

---

## 6. Repo

Same repository. Commit the PoC as-is first so it stays recoverable by tag, then rename the
crate and rebuild `src/`. `examples/`, `tests/golden/`, `PREPLAN.md` and `LANGUAGE.md` stay.
The PoC becomes a reference to diff against, never a foundation to inherit — §9-2's warning
is that nsL's architecture *was* the bug, and incremental refactor tends to preserve the
shape you are trying to escape.
