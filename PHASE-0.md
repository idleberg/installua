# Phase 0 — specification by example

**Status: complete.** All four exit criteria met. No Rust was written, as specified.

PLAN's exit criterion: *five `.nsi` assemble clean under `-WX`; exposed list frozen; §1
verified or its failures documented as design changes.*

| Task | Where | Result |
| --- | --- | --- |
| 1. Verify §1 | [`verification/lua-tooling/`](verification/lua-tooling/RESULTS.md) | holds, with three corrections |
| 1b. Verify the plugin hazard (§15.31) | [`verification/large-strings/`](verification/large-strings/RESULTS.md) | **not a hazard** — resolved |
| 2. Write the five programs | [`examples/`](examples/README.md) | done, source and expected `.nsi` |
| 3. Assemble all five under `-WX` | `examples/assemble.sh` | 5/5 clean, empty warning allowlist |
| 4. Two migration tables | [`docs/`](docs/) | both written |

Two extra runtime checks that PLAN did not ask for but the risk register did:
[`verification/semantics/`](verification/semantics/RESULTS.md) — the sign fixups against
real `lua`, and the calling convention under wine. Both are silent-wrong-answer surfaces
that `-WX` cannot see.

One thing verified along the way that is not in PLAN's list, with a correction attached:
**`makensis` is deterministic run-to-run, but its output is not reproducible across a
checkout by default.** It embeds each packed file's mtime, and git does not preserve
mtimes — so every program with a `File` line hashes differently in a fresh clone, and
program 5, which packs nothing, is the only one that does not. `SetDateSave off` fixes it
(verified). `examples/assemble.sh` gates the run-to-run half on every run, which is the
half that catches an embedded build timestamp.

Consequence for this repo: the opaque test fixtures are **generated rather than
committed** (`examples/fixtures.py`), so no vendored binaries and none of NSIS's own
artwork. Pinning their bytes would buy nothing, since the `.exe` hash is checkout-dependent
regardless.

Reproduce everything:

```
verification/lua-tooling/run.sh      # needs a source-built selene; see S1
verification/large-strings/run.sh    # needs wine and network
examples/assemble.sh                 # needs makensis
```

---

## What falls out of the phase

**The frozen v1 exposed-command list** — 24 instructions, 15 attributes, 8 declarations,
7 stdlib adapters, 5 headers, 3 plugins.
[In `examples/README.md`](examples/README.md#the-frozen-v1-exposed-command-list).

**The `MUI_*` subset actually needed** — ten, out of roughly seventy.

**§15.14's open empirical question** — the lattice lands on `unknown` **zero** times across
the five programs, and the reason matters more than the number: types enter through
declarations, not through inference.

**The first five goldens**, plus the two runtime checks.

---

## Design changes this phase produces

Nine, none of them structural. Ordered by how much they cost to act on.

### From §1 verification

1. **`installua init` writes `stylua.toml`.** §1's "stylua, as-is — no work" is wrong:
   the default `call_parentheses = "Always"` rewrites `attributes { … }` into
   `attributes({ … })` on every save. `"Input"` is the only setting that leaves both
   `attributes { … }` and `import "WinVer"` alone.
2. **The generated selene std is YAML with no `base:`.** Not TOML, and a `base:` silently
   overwrites `lua_versions:`, which makes `<const>` a parse error. No base also retires the
   `removed: true` list, since §5 whitelists the stdlib wholesale anyway.
3. **`installua init` must handle selene being unusable.** No released `selene` binary can
   parse Lua 5.4 — the published crate hard-disables `selene-lib`'s `lua54` feature with no
   way to re-enable it. Upstream packaging bug, worth filing; workaround is a one-line
   source build.
4. **The docs must not promise editor detection of a typo'd attribute name.** LuaLS does
   not check the extra-field direction on a table constructor, in any configuration. The
   compiler catches it; the editor does not. The classes stay `(exact)` so it starts
   working the day LuaLS gains the check.
5. **§13's invalid-escape check has an editor half after all** — selene's
   `bad_string_escape`. The compiler still needs its own, since `full-moon` accepts
   `"C:\Program Files"`.

### From the five programs

6. **The emission order gains two steps.** `!define`s before attributes, MUI defines and
   page macros between attributes and `Var`s.
   [Why](examples/README.md#emission-order-as-the-programs-forced-it).
7. **The predefined-globals table needs a `writable` column.** `INSTDIR = prior` must mean
   `StrCpy $INSTDIR $0`; `PROGRAMFILES64 = x` must be an error.
8. **Header declarations need a sign attribute on outputs.** Without one, every `//` on a
   `${GetSize}` result pays §15.4's fixup for nothing and "elided whenever the sign is
   statically known" stops being the common case.
9. **The calling convention has to be written down before Phase 3, not during it.**
   Arguments and returns pushed in reverse source order; caller-saves pushed **before** the
   arguments, ascending, restored in reverse — so the callee's result lands on top and no
   `Exch` is ever needed. [Why that way round](examples/04-multiple-returns/README.md).

### From §15.31

**§15.31's "plugins are the unresolved part" resolves to "no directory convention is
missing".** The plugin ABI passes `string_size` at call time, so a 1024-built plugin
handles 6150 characters in an 8192 installer, in every direction. §13's plugin check needs
no third axis. The residual risk is a plugin with a hardcoded buffer, which is a plugin bug
and undetectable from a DLL — one sentence in the `maxStringLength` docs.

---

## Two things the phase revised downward

**The clobber fixpoint is exercised once across all five programs, not five times.**
§4's string-template model means `..` never occupies a register, so almost nothing is ever
live across a call. That is good for output quality and bad for coverage of the plan's
riskiest code. Phase 3 needs synthetic IR-level tests, which PLAN already says — this is
the evidence for why it is not optional.

**§15.4's fixup cost estimate is pessimistic for the common case.** A paired `//` and `%`
over the same operands share one compare, because with a positive constant divisor both
conditions collapse to `remainder < 0`. Five instructions and one branch for both, against
an estimated "three instructions and a branch per operation".

---

## Still open, and named rather than buried

Each of these is recorded in the relevant program's README with the argument attached.

- **`license` has no home in the parameter model** — `MUI_PAGE_LICENSE` takes its file as a
  macro argument, unlike every other page setting. Probably becomes
  `page { "License", file = … }`.
- **A global's initialising store has no defined position.** `gitDescribe = ""` at top
  level emits `Var` plus a `StrCpy`, and nothing says where the `StrCpy` goes.
- **`glob` is build-machine filesystem access**, which needs to be reconciled explicitly
  with "read to verify, never to decide" rather than left implicit.
- **File handles leak on an early `return`.** Lua's answer is `<close>`, which §4 rejects.
  Either the compiler tracks handles to scope end or the docs say it does not.
- **`string.format` mixing `%s` and `%d` in one call** has to split into an `IntFmt` plus a
  template. Not hard, not designed.
- **Using a local after a `raw` block needs a diagnostic**, not just a documented rule.
- **`dateSave` is an attribute worth exposing**, and worth *not* defaulting. `SetDateSave
  off` is what makes a build reproducible across machines, but it drops the packed files'
  timestamps, which is a real behaviour change. NSIS defaults it on; Installua should
  surface it and let the user choose, rather than quietly picking either side.
