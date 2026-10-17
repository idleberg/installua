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

## Two ints that differ in sign are a type conflict

`local x = 0` followed by `x = x - 5` is rejected: "`x` holds a int and is
assigned a int". A global gets the same, "is assigned a int here and a int
elsewhere". `assigned` in `src/lower/mod.rs` compares whole `Ty`s, so the
sign attribute counts. A literal is `nonneg`, while a subtraction or
`1 << 31` has unknown sign. So any counter that counts down fails. The
message also says "a int" because it writes `a {ty}` and not `{ty:#}`.

For a global, the fix is to store the join and let the fixpoint carry it
through `Inferred.globals`. For a local, the binding keeps the type of its
`local`, so a read above a widening assignment, such as a loop body, needs that
join too, kept per declaration across rounds. The bigtest port works around it
with `raw` `IntOp`s on a global that Lua only ever assigns `0`.

## NSIS's arithmetic `>>` has no spelling

Lua's `>>` is emitted as NSIS's `>>>`, and the docs send the arithmetic shift to
`raw`. bigtest's "Integer" section needs `IntOp $n 0xffffffff >> 31`.

## `keepAttributes` cannot build off Windows

`file(…, { keepAttributes = true })` is `File /a`. makensis on any other
platform warns 5050, "File /a is disabled for non Win32 platforms", and
`installua build` runs `-WX`, so the build fails. The compiler could drop the
flag with its own warning when the host is not Windows, or the docs could say
so. The bigtest port leaves the flag out.
