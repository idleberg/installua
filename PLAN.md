# Plan

Work for the 0.2.x patch releases: fixes and missing spellings, none of which
changes what 0.2 ships.

## 1. Port the examples NSIS ships

`tests/ports.rs` checks each port in `tests/ports/` against its original by
`makensis -V4` trace: the effect lines have to match. Pages are left out,
because Installua only writes MUI2 pages. A line the original writes inside a
macro is invisible to `-V4`, so `HIDDEN` lists the port's copy.

Ported: `example1`, `example2`, `Memento`, `MultiUser`, `one-section`,
`primes`, `silent` and the five `Modern UI/` examples.

Next, from `$NSISDIR/Examples`: `bigtest`, `LogicLib`, `languages`, `Library`,
`StrFunc`, `unicode`, `VersionInfo`, the `FileFunc`/`TextFunc`/`WordFunc`
ones, and the `nsDialogs/`, `StartMenu/` and `System/` directories.

Not portable: `rtest` tests `GetLabelAddress` and `Call` through an address,
both rejected.

**Rule:** a port that needs a missing feature or hits a bug waits. The fix
goes into Installua first, with its own golden, and the port follows.

## 2. Review the `todo` sites

`src/lower` has 55 `self.todo(` calls (39 in `mod.rs`, 12 in `expr.rs`, 3 in
`handle.rs`, 1 in `library.rs`). Each is a shape the compiler does not
support. For each, decide whether ordinary code reaches it: if so, it is a
missing feature, and it gets a spelling or a real diagnostic; if not, it stays.

## 3. A field on a file handle names the wrong type

`f.size` on a `fileOpen` handle is reported as a field of a control, with the
control fields as the note. Files and windows share one `handle` type
(`src/lower/handle.rs`), so the lowering cannot tell them apart. The fix is a
fifth type rather than a check. `KNOWN` in `tests/positions.rs` holds its two
cells, and a fix fails that test until they leave the list.

## 4. Plain wording in the docs

Words that made sense while building Installua but mean nothing to someone
reading the site cold. Each hit is rewritten to say the thing itself, not
swapped for a synonym. Counts are for `web/src/content/docs/`.

| Word | Hits | Say instead |
| ---- | ---- | ----------- |
| shape | ~25 | what it actually is: "form", "signature", "number of values", "kind of mistake" |
| half, halves | ~25 | "installer and uninstaller", "the case-sensitive version", "the MUI2 part" |
| spelling, spell | 60 | "name", "syntax", "way to write"; "the format has no spelling for it" → "a `.toml` can't describe it" |
| lowering, lowers | 30 | "compiles to", "is turned into" |
| corpus | 28 | say once per page what it is (the scripts surveyed), then "those scripts" |
| the rule, half of the rule | 9 | name the rule in place: "no public script calls it" |
| surface, census, load-bearing, honest, wearing, genuinely, quietly | ~35 | case by case, usually delete the word |

Keep: the tagline and page titles "Lua-shaped, not Lua" / "NSIS-shaped, not
NSIS" (they are URLs and the project's pitch), and literal uses ("two and a half
times", "half-converted", "which half of the registry"), and the generated
`cli.md`.

Order, most hits first: `reference/commands/plugins-and-headers.md`,
`reference/plugins/third-party-plugins.md`, `reference/plugins/what-stays-out.md`,
`reference/commands/not-available.md`, `reference/modern-ui.md`, then the rest.
One file per pass, re-read after, `mise run check` at the end (`tests/docs.rs`
matches names, so removing a name by accident fails it).

Not a word problem, left for later: "rather than" (85 hits) is a tic, not
jargon.

## 5. Bugs

Found while rewriting PimpBot's installers in Installua
(`~/Repositories/_visbot/pimpbot-installua`). Each one has a repro to paste
into a scratch `.lua`, what happens now against 0.2.0 (`581cf0f`), what should
happen, where the fix goes and how to check it. They are listed worst first.
PimpBot works around each one with a `ponytail:` comment, and those workarounds
come out once the fix ships.

The same rule as §1 applies: a fix comes with its own test. That is a
hand-written golden when the fix changes emitted `.nsi`, a `tests/diagnostics.rs`
case when it changes a message, and a Tier-3 `makensis -WX` build when the bug
was that `makensis` rejected the output. `mise run check` passes at the end of
each one.

### 5.1 `${Using:StrFunc}` is emitted above `SetCompressor` — fixed

The init lines now come after the attributes: slots 4 and 5 swapped in
`src/ir.rs` and `src/emit.rs`, since each `${Using:StrFunc}` writes a
`Function` and `SetCompressor` refuses to follow one. Tested by the
`compressor` golden (tiers 2 and 3); example 05 moved its two lines by hand.

### 5.2 A `param` named like an NSIS constant silently becomes the constant — fixed

A top-level `<const>` now shadows a built-in constant. The body lookup in
`src/lower/expr.rs` and the attribute lookup (`constant_arg` in
`src/lower/mod.rs`) check the program's consts before `constant_named`.
Tested by `a_parameter_shadows_the_constant_it_is_named_like` in
`tests/params.rs`, with and without `-D`; `TEMP` undeclared still reads `$TEMP`.

### 5.3 Functions no one calls fail the build — fixed

`callgraph::unreachable` walks from every body the compiler made (sections,
callbacks, page and control hooks, walker bodies) and treats any word of a
`raw` block that names a `func` as a call. The set rides the `lower` fixpoint,
so a dropped `func` is never lowered at all. Tested by
`a_func_nothing_reaches_is_left_out` in `tests/include.rs`.

### 5.4 Parameters of an uncalled `func` are typed `unknown` — fixed

Fell out of 5.3: an unreached `func` is not lowered, so its body is not
type-checked. `newer` in `a_func_nothing_reaches_is_left_out` is the repro.

### 5.5 `execShell` refuses the `runas` verb — fixed

The overlay can now mark a position `open`: its `-CMDHELP` members stay as
completions, and any value passes. `ExecShell`'s verb is the one such position.

### 5.6 `sendMessage` cannot broadcast — fixed

`messages()` in `src/builtins.rs` types `HWND_BROADCAST`, the header's only
`HWND_*`, as a handle; `sendMessage(5, …)` is still an error. Covered by
`send_message_broadcasts_through_hwnd_broadcast` in `tests/overlay.rs`.

### 5.7 An entry cannot be left out of a block's list at build time — fixed

The resolver keeps `Resolved::untaken`, the `local`s declared only in branches
not taken, and the claim pass drops a list entry naming one. A misspelt entry is
still `unknown-field`. Tests in `tests/branches.rs`, `a_list_may_name_what_was_left_out`.

### 5.8 `c and x or y` does not fold to a value — fixed

`fold` in `src/resolve.rs` now folds `and` and `or` by Lua's rule, where
`false` is the only falsy value: the left side decides, and the right is folded
only when it does not. A runtime `and`/`or` still wants `bool`s. Covered by
`and_or_picks_a_value_at_build_time` in `tests/params.rs`, including Lua's
`c and false or y` trap.

### 5.9 `System.call` needs the whole signature at build time — fixed

`signature_outputs` in `src/lower/expr.rs` counts `.s` in the folded pieces
and accepts a runtime piece only as an argument's value: after a space, before
`,` or `)`. The explicit `outputs` count was not needed. Tests in
`tests/declarations.rs`, `system_call`.

### 5.10 `glob`: no recursion, and its error names the wrong path — fixed

The error now names the folder it could not read, as written. `glob` takes
`**` for any depth of folders and a trailing `/` for folders instead of files,
with wildcards in any segment; see `fn glob` in `src/lower/mod.rs` and
`tests/glob.rs`.

### 5.11 The docs promise a `BUILD` namespace that does not exist — fixed

Implemented as `MAKENSIS.echo`, `MAKENSIS.system` and `MAKENSIS.getDllVersion`
(renamed from `BUILD`), in `src/lower/makensis.rs`. Each is its `!` line where
the call stands, and an answer is caught as an install-time value, never a
`<const>`. Two corrections to the plan: `!system` returns a wait status off
Windows (`exit 3` reads 768), so it is shifted unless `NSIS_WIN32_MAKENSIS`; and
3.12's `!getdllversion` does not fail on a file without a version resource, it
defines `""`, so there is no `/noerrors` (that flag only silences a missing
file) and `IntOp … + 0` turns `""` into 0. Tested in the `build-time` golden,
tiers 2 and 3. Ceiling: a `$` in a literal is left alone only when the literal
is the direct argument; inside a `..` it still warns, as it does for `raw`.

### 5.12 `installua stubs` misses every source outside the project root — fixed

`src/project.rs` collects the sources: `installua.toml`'s programs plus their
`include`s, or a recursive walk without one. Tested in `tests/stubs.rs`.

### 5.13 A declaration inside a build-time `if` is invisible to lua-language-server — fixed

A `local` declared in a top-level branch stays visible after `end`, and LuaLS,
scoping as Lua does, reports `undefined-global` at each later use. With §5.8,
`local X <const> = c and a or b` picks the value in a form both tools accept.
`concepts/lua-shaped-not-lua.md` documents it (a table row and the truthiness
section), and `program-structure.md`'s "Build-time if" points there. The
`project_meta` stub was not needed. Verified with `installua check` plus
`lua-language-server --check` on the and/or repro (no problems).

### 5.14 A top-level `local` leaks out of the file that declares it — fixed

A top-level `local` is private to its file; a file shares one with
`return { name = name }`, read as `m.name` after `local m = include "…"`.
`src/frontend/scope.rs` checks it and rewrites `m.name` to the bare name, so
`resolve` and `lower` are unchanged. Tested in `tests/include.rs` and the
`include` golden; PimpBot migrated, its four fixture `.nsi` byte-identical.

Two files may declare the same top-level `local`: the scope pass renames every
copy but the lowest file's (`COLOR` in file 1 → `COLOR_1`), so a program
without a collision emits what it did before. Ceiling: the suffix reaches the
`.nsi`, and a `raw` string spelling the unsuffixed name reads the other file's.

### 5.15 A duplicate `local … <const>` passes `check` and fails the build — fixed

`const_twice` in `src/resolve.rs` reports the second declaration as
`duplicate-block`, naming the first, within one file or across `include`s.
Tested in `tests/include.rs`. Per file since §5.14's rename: across files the
two are separate names.

### 5.16 A top-level `local` is visible above its declaration

`concepts/lua-shaped-not-lua.md` ("Everything hoists") makes the top level
order-free: every top-level name is resolved before any body is compiled. That
holds for `func`s, whose names are strings, but it also holds for `local`s, and
Lua scopes a `local` from its declaration down:

```lua
attributes { name = "b", outFile = "b.exe" }
local random = checkbox {
	"Switch every", x = 8, y = 8, width = 56, height = 13,
	onClick = function() interval.enabled = random.checked end,
}
local interval = number { x = 70, y = 8, width = 20, height = 13 }
installer {
	page.custom { "Options", controls = { random, interval } },
	page.instFiles {},
	section("s", function() end),
}
```

**Now:** `installua check` passes. LuaLS reports `undefined-global` for
`interval` in the `onClick` (and for `random` inside its own initialiser,
which Lua does not yet see either). PimpBot hits this in
`pages/avs-settings/page.lua`: `settingsRandom`'s `onClick` reads
`settingsInterval`, declared below it.

**Expected:** open. Two controls that refer to each other are a real need, so
declaring in order is not enough on its own. Options:
- Lua's own answer: a forward declaration, `local interval` with no value,
  then `interval = number { … }` below. Today a top-level `local` with no
  value is `not-yet-implemented` ("has nowhere to live"), so this means
  accepting it as a declaration whose one later assignment is its value.
  Then read-before-declaration becomes an error, as in Lua.
- Keep hoisting for `local`s and say in the docs that LuaLS reports it.
- A control's callback could be set after both exist
  (`random.onClick = function() … end`), which reads in Lua order without a
  forward declaration. I have not checked whether this compiles today.

`func`s stay hoisted either way: their name is a string, so Lua has nothing to
say about their order.

**Verify:** whichever option: the repro in its fixed form passes both
`installua check` and `lua-language-server --check`, a `tests/diagnostics.rs`
case for a read above the declaration, and PimpBot's `page.lua` migrated.

## Later: random programs

Generate random well-typed programs from the grammar, compile them, and run
`makensis -WX`. A panic, a generic `not-yet-implemented` or a `makensis` error
is a finding. Worth it only once the items above are done.
