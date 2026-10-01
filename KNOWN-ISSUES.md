# Known issues

Bugs and rough edges in what 0.2 ships. New features go in `PLAN.md`.

## A field on a file handle names the wrong type

`f.size` on a `fileOpen` handle is reported as a field of a control, with the
control fields as the note. Files and windows share one `handle` type
(`src/lower/handle.rs`), so the lowering cannot tell them apart. The fix is a
fifth type rather than a check. `KNOWN` in `tests/positions.rs` holds its two
cells, and a fix fails that test until they leave the list.

## Generic `todo` diagnostics

`src/lower` has 25 `self.todo(` calls (16 in `mod.rs`, 7 in `expr.rs`, 2 in
`handle.rs`). Each reports `not-yet-implemented` with a generic message. The
ones ordinary code reaches have real diagnostics now. What is left:

- Not probed yet, only read: `mod.rs` 2265, 2587, 3454, 3984, 4555, 4593,
  4597, 5482, 5884, 7155, 7224, 7647, 7659, 7685; `handle.rs` 246, 533;
  `expr.rs` 3384, 3405.
- The `Handled` arms, the operator fallback and the call fallbacks are
  unreachable and stay.
