# Architecture

Which file owns a concern. The pipeline and its invariants are in `AGENTS.md`,
terms are in `GLOSSARY.md`, and how each part works is in its doc comments. A
pass whose file is named in the pipeline diagram has no entry here.

## Codemap

### `src/lib.rs`, `src/main.rs`

The API is `check`, `compile` and `build` with their `_with` variants, plus
`build_mapped`. The CLI is a thin shell over it: argument parsing, file reading
and exit codes, and no decisions. `src/assemble.rs` runs `makensis` and rewrites
its output through the line map in `src/map.rs`.

### `src/frontend/`

Source text to AST. `lift.rs` is the whitelist pass over `full-moon`'s CST, and
it rejects any Lua that Installua lacks. `include.rs` splices files together,
`scope.rs` gives a top-level `local` to its file, and `strings.rs` decodes
literals. Nothing here knows what a name means.

### `src/resolve.rs`

Names, never types: a `func` gets params, block and span. Types come from the
`lower` fixpoint, whose seed is `src/lower/sig.rs`.

### `src/lower/mod.rs`

Declarations, and one CFG per body. The entry points are `lower` and
`lower_once` (a single fixpoint round). `top_level` handles the top-level
statements and `stmt` the ones in a body. Assignment typing is in `assigned`. A
`todo` call site is a construct that is not implemented yet.

### `src/lower/expr.rs`

Expressions. Operators are in `int_op` (Lua's `>>` is emitted as `>>>`). Every
library adapter (`math.*`, `string.*`, `bit32.arshift`, `tostring`, `tonumber`)
is an arm of the name dispatch in `call`, and each has its own fn, such as
`arshift`. Conditions are lowered by `branch` and `compare`, as compare-and-jump.

### `src/lower/` (the rest)

One file per stock header or construct: `pages.rs` (MUI2 page tables),
`handle.rs` (section, window and file handles and their fields), `control.rs`,
`languages.rs`, `multi_user.rs`, `memento.rs`, `radio.rs`, `insttype.rs`,
`library.rs`, `x64.rs`, `makensis.rs` (`MAKENSIS.*`) and `callback.rs`.

### `src/table/`

The overlay. `generated.rs` holds the `-CMDHELP` half, and `overlay.rs`, with
`ROWS`, holds the hand-written half. They are joined in `mod.rs` behind
`table`, `by_nsis` and `by_installua`, and every `Class` is defined there. The
emitter, the stub generator and the census all read it. `src/builtins.rs` holds
the NSIS constants (`CONSTANTS`), not instructions.

### `src/mui/`, `src/headers.rs`, `src/locale.rs`

The other surfaces, each counted the way the overlay is. For `src/mui/`,
`scan.rs` holds the snapshot and `rows.rs` (`ROWS`) the judgement.

### `src/declarations.rs`, `src/declarations/*.toml`

`import "X"` macros and `plugin "X"` methods, with one `.toml` per header or
plugin. A project adds its own under `.installua/declarations/`.

### `src/stubs.rs`

The editor side of `installua stubs` and `installua init`. `meta` generates the
lua-language-server definitions, and `selene_std` generates the selene std. A
library adapter needs an entry in both, written by hand: a `---@param` block in
the meta, and an entry in the selene table.

### `src/diag.rs`, `src/retired.rs`

`Code` is the registry of diagnostic codes. Diagnostics are collected, never
thrown. `retired.rs` maps each NSIS instruction that was deliberately not
ported to its Lua replacement.

### `src/types.rs`

The lattice: four types, where `int` carries width and sign as attributes, and
`Ty::join`.

## Tests

### `tests/goldens.rs`

Runs every `tests/golden/*.{lua,nsi}` pair at tiers 2 and 3. Its `makensis`
fn is the tier-3 runner.

### `tests/ports.rs`, `tests/ports/`

Ports of `$NSISDIR/Examples`, each compared with the original.

### `tests/docs.rs`

Every check the website gets, and the paths named in this file and in
`AGENTS.md`.

### `tests/census.rs`

Every `-CMDHELP` line belongs to exactly one `Class`.

### `tests/mui.rs`, `tests/headers.rs`

The census joins for the other surfaces.

### `tests/positions.rs`

Every kind of value in every position. `KNOWN` lists the cells that are still a
generic `not-yet-implemented`.

### `tests/diagnostics.rs`

`CASES` has a source for every code in `Code::ALL`.

### `tests/stubs.rs`

The stubs, checked against the compiler.

### `tests/common/`

`check_or_update` for snapshot files.

The other `tests/*.rs` are named after the construct they test.
