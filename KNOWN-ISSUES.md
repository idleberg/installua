# Known issues

Bugs and rough edges in what 0.2 ships. New features go in `PLAN.md`.

## A field on a file handle names the wrong type

`f.size` on a `fileOpen` handle is reported as a field of a control, with the
control fields as the note. Files and windows share one `handle` type
(`src/lower/handle.rs`), so the lowering cannot tell them apart. The fix is a
fifth type rather than a check. `KNOWN` in `tests/positions.rs` holds its two
cells, and a fix fails that test until they leave the list.

## Generic `todo` diagnostics

`src/lower` has 16 `self.todo(` calls (9 in `mod.rs`, 6 in `expr.rs`, 1 in
`handle.rs`). Each reports `not-yet-implemented` with a generic message. Every
one ordinary code reaches has a real diagnostic now, except `string.rep(…)` and
other library functions the table lacks, where "not supported yet" is true.

The rest stay, because the compiler can't reach them or has caught the mistake
before them. They are the `Handled` and table-shape arms, the operator,
expression and call fallbacks, the non-call `control`, `group` and `for`
iterator arms, and an assignment target that is not a name, field or index.

Not probed: `no_fields` on a handle or an unknown type (`handle.rs`), when
the base is neither a call nor a name.
