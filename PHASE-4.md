# Phase 4 — emit, map, assemble

**Status: complete. The exit criterion is met as written and checked by a test.**

PLAN's exit criterion: *all five programs assemble under `-WX`, empty warning allowlist;
one mapping test per `Origin` kind.* Both halves are mechanical:
[`tests/examples.rs`](tests/examples.rs) compiles each of the five and hands the result to
real `makensis` with warnings promoted to errors, and [`tests/map.rs`](tests/map.rs) has one
test per origin kind plus both of the `makensis` message syntaxes §15.22 verified.

| Task | Where | Result |
| --- | --- | --- |
| Emission order (§12) | [`src/ir.rs`](src/ir.rs), [`src/emit.rs`](src/emit.rs) | the `Module`'s field order **is** the order |
| Data versus syntax (§12) | [`src/emit.rs`](src/emit.rs) | quoting decided per piece, not per argument |
| `!include` dedup and `StrFunc` init (§15.21) | [`src/lower/mod.rs`](src/lower/mod.rs) | one `Requirements` set, collected then emitted |
| Line map (§15.22) | [`src/map.rs`](src/map.rs), [`src/layout.rs`](src/layout.rs) | one origin per output line, no drift |
| `makensis` invocation and translation (§15.22) | [`src/assemble.rs`](src/assemble.rs) | `build` owns it; `emit` stops before it |
| The five programs | [`examples/`](examples/) | all five compile, all five assemble |

```
cargo test                       # 54 tests
cargo run -- emit  examples/04-multiple-returns/install.lua --stdout
cargo run -- build examples/01-mui-uninstaller/install.lua
```

Tiers 0, 2, 3 **and 4** all run on this phase's work.

---

## The five programs, and what each one forced

Getting them to assemble was most of the phase, and each needed a different thing. This is
the honest inventory, because "the exposed set is what the five programs reach" (PLAN §0) is
only a scope rule if somebody writes down what that turned out to be.

| Program | What it forced |
| --- | --- |
| 1 — MUI installer with uninstaller | `installer`/`uninstaller` as one function with a `Half` threaded through; MUI pages, icons and `MUI_LANGUAGE`; `versionInfo`; `Section /o`; `messageBox` (§15.18); the eight things emitted rather than written |
| 2 — plugin-heavy | plugin declarations, `System::Call`, `raw` — and the caller-save machinery extended to all three (§15.11) |
| 3 — file iteration | `for … in glob` unrolled on the build machine, `for … in lines` as a `FileRead` loop, `fileOpen`/`f:close()` |
| 4 — multiple returns | `import` and header macros with **trailing output registers** (§15.27) |
| 5 — strings and integers | the `string.*` adapters, `StrFunc` init collection (§15.21), §15.9's case-folding peephole |

---

## What the phase decided

### A `<const>` is a `!define`, and a path is where that stops

§7-1 rules that `local X <const> = …` is a `!define` and `${X}` is the spelling in the
output. That is now true, and it has one exception that had to be invented here: **a `${…}`
cannot appear in a path.**

`/` is normalised to `\` in text pieces (§5), and a macro expansion is opaque to that — the
preprocessor substitutes after this compiler has stopped looking. So a define holding
`Software/Example5` spliced into a registry key would ship the separator NSIS does not
accept. [`Arg::into_path`](src/ir.rs) therefore substitutes a constant's **value** at the
parameter boundary, and only when the value contains a `/`: a value with no separator
normalises to itself, so the reference survives and the output still says `${APP}`.

The result is that `InstallDir "$PROGRAMFILES64\${APP}"` reads as a hand-written script
would, while `WriteRegStr HKLM "Software\Microsoft\…\Example1"` is spliced. The hand-written
oracle spells the second `${REGKEY}` and normalises the define instead — which is what a
human does when they can see every use at once, and is not available to a rule.

### `installer` and `uninstaller` are the same function

§15.3 says `un.` has no surface spelling. The whole of that is a two-variant enum threaded
through one code path: it picks the section-name prefix, the page-macro prefix, the
callback's name, and the `MUI_ICON`/`MUI_UNICON` define. Writing the uninstaller as a second
lowering would have made it a second thing to keep in step, which is exactly the shape §15.3
was avoiding.

### Opaque callees are call sites, not instructions

§15.11's three — `plugin`, `System::Call`, `raw` — clobber everything, and Phase 3 recorded
that rule with no producer for it. The producers arrived here, and the interesting part is
that a plugin call is a **call site** in the IR ([`ir::CallKind::Opaque`](src/ir.rs)) rather
than an instruction. It has to be: the saves around it are `live ∩ clobbered`, and the fact
that the right-hand side is "everything" does not change where the pushes go or that they
have to be decided after colouring.

So the whole of the opaque rule is four lines in
[`alloc::insert_saves`](src/alloc.rs) — the clobber set is `0..COUNT` instead of a fixpoint
lookup — and program 2 gets its `Push $1` / `Pop $1` around `System::Call` from the same
machinery that gave program 4 its `Push $0` around `countdown`.

`raw` differs from a plugin in exactly one respect, and it is a reporting one: its lines are
the user's text and nothing checked them, so they carry [`Origin::Raw`](src/map.rs).

### A macro's inputs interfere with its outputs

`${GetSize} "$0" "" $1 $2 $3` writes three trailing registers, and nothing in this compiler
has read `FileFunc.nsh`. The allocator would happily colour the first output the same as the
input, since the input is dead after the call — and `${GetSize} $0 "" $0 $1 $2` *happens* to
work, because that macro pushes its arguments before it pops its results.

The next macro may not, and the difference is invisible at this level. So
[`ir::Instruction::atomic`](src/ir.rs) marks a line whose inputs are still live while its
outputs are written, and the interference graph gets the edge. Every macro and adapter sets
it; no NSIS instruction does, because an NSIS destination is a whole write.

### The map is built with the text, not from it

[`Out::line`](src/emit.rs) takes a line and its origin together. There is no second pass
that walks the output and decides where each line came from, because a second pass can
disagree with the first about how many lines there are — and a map that drifts by one is
worse than no map, since every message it rewrites points at the wrong line.
`every_emitted_line_has_an_origin` asserts the two counts are equal, which is the cheapest
possible guard against exactly that.

The attribution itself is a span stamped on each `ir::Instruction` by
[`BodyLowerer::emit`](src/lower/mod.rs) from the statement being lowered. An instruction
with **no** span is the compiler's own line, and that is not a fallback: it is the whole
distinction §15.22 needs, since a generated line failing under `makensis` is by definition a
compiler bug and gets told to the user as one.

### `build` owns the invocation, and `emit` exists so that it does not have to

§15.22's cost is real: it makes Installua a build tool rather than a program that emits a
file. The mitigation is the two entry points, and both are now in the CLI —
`installua emit` writes the `.nsi` and stops, `installua build` additionally runs
`makensis -WX` and rewrites its diagnostics through the map.

`-WX` rather than plain is deliberate at the CLI and not only in tests: §14's "warnings are
failures" is not a testing policy, it is the rule that keeps a silently wrong installer from
shipping, and the three most common NSIS mistakes are all warning 6000.

---

## Divergences from the hand-written oracles, and why

The five `expected.nsi` files are Phase 0's oracles and remain unedited. What is diffed is
`generated.nsi` beside each of them, so both are visible and neither is silently rewritten.
Every difference is one of these five:

1. **Register numbering.** The allocator is tighter than the hand-written files in several
   places and looser in one (a `local` whose value is a template still spends a register;
   see below). Freezing a hand-written numbering would be freezing an arbitrary one.
2. **`${REGKEY}` in path positions**, spliced by value — the ruling above.
3. **`messageBox` is materialised rather than fused.** §15.18 describes fusing the answer
   into the comparison when the value is dead after one comparison against a constant;
   program 1 binds it to a `local` first, so the compiler pays the "honest price" form the
   ruling also describes — two `StrCpy` and a `StrCmp`. Correct, three lines longer, and the
   fusion is a liveness-driven peephole rather than a lowering.
4. **`string.find` + `string.sub` do not cancel algebraically.** The oracle collapses `+ 1`
   and `- 1` into nothing because a human can see both; the compiler emits both. Verified
   correct under wine.
5. **`SetOutPath $INSTDIR` is unquoted** where the oracle quotes it. NSIS parses `$0` and
   `"$0"` identically and the unquoted form is what a reader flinches at less (§9-6).

---

## Still open, and named rather than buried

- **A `local` holding a template still spends a register.** `local target = INSTDIR ..
  "/tool.exe"` is a `StrCpy` into `$0` and then `$0` everywhere, where the oracle splices
  the template at each use. Binding such a `local` as a *value* rather than a slot would
  remove the register and the caller-saves around it — but it is only sound while nothing
  assigns to the name, so it wants a per-binding mutability fact the resolver does not
  currently compute.
- **`string.sub` with a negative index is `Class::Todo`.** §15.21 says the semantics are
  pinned by goldens over boundary cases rather than by a formula, and the boundary cases are
  not written. A provably-negative constant is rejected; a runtime value is assumed
  non-negative, which is the assumption the five programs happen to satisfy and the one that
  will need the goldens first.
- **`System::Call`'s signature is read for `.s` and nothing else.** Counting the pushes is
  what makes `local ticks = system.call(…)` legal at all; parsing the rest — which would
  narrow the clobber set from "everything" — is PLAN §3's deferred item, unchanged.
- **Top-level global initialisers go into `.onInit`, and only the installer's.** A global
  assigned at the top level is initialised in `.onInit`, which is invented if the program
  has none. The uninstaller is a separate executable with its own `Var` storage, so a global
  read in `uninstaller {}` starts empty — which is §15.3's reachability question, still the
  third consumer of the call graph and still unimplemented.
- **A registry *value* is data, so its separators are the user's.** `writeReg(HKLM, key,
  "UninstallString", INSTDIR .. "/uninstall.exe")` emits a forward slash: the key is a path
  position and the value is not, and nothing can tell that this particular value is a path.
  Windows accepts it; the oracle writes `\`.
