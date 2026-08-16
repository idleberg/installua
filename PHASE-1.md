# Phase 1 — skeleton, frontend, and a thin spine

**Status: complete.** Both exit criteria met.

PLAN's exit criterion: *`installua check` clean on all five programs; `installua build`
produces an assembling `.nsi` for the spine subset.*

| Task | Where | Result |
| --- | --- | --- |
| Commit the PoC, recoverable by tag | tag `poc` | done, `.gitignore` un-ignores it |
| Library crate, thin binary (§15.12) | [`src/lib.rs`](src/lib.rs), [`src/main.rs`](src/main.rs) | no statics of any kind |
| Diagnostics as data (§9-4) | [`src/diag.rs`](src/diag.rs) | span, code, severity, notes; a walkable registry |
| Frontend | [`src/frontend/`](src/frontend/) | parse → whitelist → escapes → `elseif` → floats |
| The thin spine | [`src/lower.rs`](src/lower.rs), [`src/emit.rs`](src/emit.rs) | `attributes {}`, one `section`, `detailPrint` |

```
cargo test          # 18 tests; the makensis one skips cleanly if it is absent
cargo run -- check examples/*/install.lua
cargo run -- build tests/golden/spine.lua --stdout
```

Tiers 0, 2 and 3 are live from this phase, as §14 asks. The spine's `.nsi` is compared by
exact equality against [`tests/golden/spine.nsi`](tests/golden/spine.nsi) and then assembled
under `makensis -WX` with an empty warning allowlist.

---

## What the phase decided

### `check` is the frontend, and only the frontend

`installua check` runs the four frontend steps and stops. It does not lower, so it does not
report `nayme = "x"` in an `attributes {}` block — `build` does. The split is what makes
"clean on all five programs" a meaningful criterion at all: the five reach roughly the whole
language and almost none of the v1 *command surface* is lowered yet, so a `check` that
lowered would have to fail on all five by construction.

It is also the shape an editor wants. `check` is what an LSP would call on a keystroke, and
it is re-entrant against an in-memory string with no file system involved (§9-2).

### The whitelist pass is syntactic

It decides **which Lua forms exist in this language** and nothing else. `detailPrint(nope)`
passes it and `goto done` does not. No name is resolved, no type is inferred, and no
instruction is distinguished from a declaration — all Phase 2.

Three rules are about a *spelling* rather than a shape, and they are here anyway because the
sets they check are closed and syntax is enough to see them:

- `require` → named as `import`/`include`
- the `for … in` iterators — `glob`, `lines`, `range`, with `pairs` getting its own note
- `raw`'s string arguments, which are NSIS source rather than data

### `NotYetImplemented` and `UnknownField` are different diagnostics

`uninstaller {}` is **scheduled**; `attributes { nayme = … }` is a **typo**. Collapsing them
into one "unsupported" message is the difference between a five-second fix and a search
through the NSIS documentation, and it is also what makes the `todo` bucket countable — this
is the code `installua coverage` will burn down in Phase 6.

### `Module`'s field order *is* the emission order

The nine steps Phase 0 settled on are written down once, as the order of the fields in
[`ir::Module`](src/ir.rs), rather than in the emitter's control flow. Phase 1 fills three of
them. The other six exist empty, because a widening is a smaller change than a reordering.

### The registry test paid for itself on its first run

PLAN §2 asks for a registry-walking test and for every rejection to name its replacement.
Both are in [`tests/diagnostics.rs`](tests/diagnostics.rs), as one table with one row per
code — so a code added without a case fails the build (§14: omission is unrepresentable).

`every_rejection_names_its_replacement` failed the first time it ran, on `bad-field-value`,
which had been written with no note. That is the entire argument for the test: the wording of
a diagnostic is not something anybody checks by remembering to.

---

## Design changes and things worth recording

1. **A byte escape above 127 becomes the matching Latin-1 code point.** `\xNN` and `\ddd`
   name a *byte* and this compiler carries *text*. §5 says these are folded into the literal
   rather than rejected, and they round-trip exactly over ASCII, which is every real
   installer. Recorded rather than solved.
2. **The `$`-in-literal check is a warning, not an error.** `detailPrint("cost $USD")` is
   legal and correct — the `$` is doubled on output — so an error would be wrong. But §14
   promotes warnings to failures, and there is no opt-out spelling yet. Phase 5's
   `installua.toml` is the obvious home for one.
3. **`raw` suppresses the `$` check on its direct string arguments only.** `raw(a .. b)` is
   not special-cased, because nothing can concatenate into a `raw` block until Phase 2 gives
   `a` a meaning.
4. **A function expression is allowed in any argument position.** `Position::Argument`
   enforces "closures are declaration bodies, never values" (§3), which is the half that
   needs no resolution. That `foo(function() end)` is only legal when `foo` is a declaration
   is the other half, and it needs the resolver.
5. **`0xFFFFFFFF` reads as a bit pattern rather than an error.** NSIS integers are 32-bit and
   wrap, and a hexadecimal literal is written for its bits.

---

## Still open, and named rather than buried

- **Where the `$`-warning opt-out lives**, per change 2 above.
- **`installua build` does not invoke `makensis`.** Phase 4 owns that, together with the line
  map that rewrites its diagnostics back through `Origin::{User, Raw, Emitted}` — building
  the invocation now without the map would produce exactly the raw `.nsi` line numbers §15.22
  exists to hide.
- **The spine accepts `section(name, body)` only.** The `{ optional = true }` form is one
  overlay row in Phase 5, and adding it here would mean inventing half of the parameter model
  in the wrong place.
- **Phase 0's finding that the clobber fixpoint is exercised once across all five programs
  still stands, and nothing in this phase changes it.** Phase 3 needs synthetic IR-level
  tests.
