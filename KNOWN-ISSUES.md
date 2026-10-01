# Known issues

Bugs and rough edges in what 0.2 ships. New features go in `PLAN.md`.

## A field on a file handle names the wrong type

`f.size` on a `fileOpen` handle is reported as a field of a control, with the
control fields as the note. Files and windows share one `handle` type
(`src/lower/handle.rs`), so the lowering cannot tell them apart. The fix is a
fifth type rather than a check. `KNOWN` in `tests/positions.rs` holds its two
cells, and a fix fails that test until they leave the list.

## Generic `todo` diagnostics

`src/lower` has 39 `self.todo(` calls (25 in `mod.rs`, 10 in `expr.rs`, 3 in
`handle.rs`, 1 in `library.rs`). Each reports `not-yet-implemented` with a
generic message. The ones ordinary code reaches have real diagnostics now.
What is left:

- `todo_at`'s note says the shape is "scheduled" and that `installua coverage`
  counts it. Neither is true for these sites.
- About 15 are one typo: a positional entry where named fields are expected,
  or the other way round. One shared `bad-field-value` would cover them all.
  The sites: `expr.rs` 1154, 1514, 3213; `library.rs` 85; `handle.rs` 1514;
  `mod.rs` 1947 (`attributes "x"`), 1992, 2801, 2937, 2984, 3037, 3764, 3897,
  6260. Line numbers are from before the fix above and have moved.
- Not probed yet, only read: `mod.rs` 2217, 2533, 3358, 3883, 4449, 4487,
  4491, 5376, 5755, 7000, 7065, 7431, 7443, 7469; `handle.rs` 246, 533;
  `expr.rs` 3262, 3283. Same caveat on line numbers.
- The `Handled` arms, the operator fallback and the call fallbacks are
  unreachable and stay.

## Two `not-yet-implemented` shapes

Both are real errors with a workaround in the note, not features:

- `string.sub(s, -k, j)` with `j >= 0` needs the string's length at run time.
- A `for` step that is not a build-time constant: its sign picks the loop's
  test.

## `string.gmatch` names the wrong thing

`for w in string.gmatch(…)` reports "`string` is not a header" instead of
saying `gmatch` is not an iterator.
