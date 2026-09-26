# Finding bugs on purpose

Steps 1 and 3 are done, and step 3's findings are fixed except one left on
purpose. Step 2 is under way; steps 4 and 5 are deferred.

## Why

Every issue in `INSTALLUA-HANDOFF.md` was found by porting one real installer.
That worked because a real installer combines features in ways no test
does, but finding them was luck. Almost all of them had the same shape: a
value worked in one position and not in another.

- a page could be listed in a block but not bound to a `local` (10)
- a control worked through a `local` but not through a global (9)
- a field write worked on a name but not on a call (11)

## What the tests cover today, and what they don't

- **The census covers the vocabulary.** Each of the 276 NSIS commands, 255 MUI2
  names and the locales has an Installua spelling or a stated reason. `todo 0`
  measures that. `tests/docs.rs` checks the same list against the site.
- **It does not cover the grammar.** A word is checked only in the form written
  in its own test. The 32 test files are hand-written, one behaviour at a time.
  Some loop over variants of one feature (`tests/controls.rs`, the field list).
  None loops over positions. So `todo 0` looks finished while combinations
  are still broken.
- **Doc examples are not compiled.** `tests/docs.rs` matches names on the site
  and never builds the ` ```lua ` blocks.

## Approaches, cheapest and most productive first

### 1. Compile every Lua block in the docs

Extract each ` ```lua ` block from `web/src/content/docs/`. Wrap a fragment in a
minimal program, build it, and run `makensis -WX` when it is available. A
failure is either a compiler bug or a doc that has drifted. Mark a block that
is deliberately wrong (an error example) so the test skips it. Runs in CI and
keeps finding things.

**Done:** `every_lua_example_compiles` in `tests/docs.rs`.

- **Wrappers:** each block is tried as a section body, at the top level, and
  as `installer {}` entries.
- **Fence words:** `lua skip` (needs files on disk) and `lua error` (shown
  because it is rejected).
- **Examples:** about 20 were made self-contained.

A `-->` comment must now begin a line of the output; there are three, and
all three match. `makensis -WX` stays out of the test. A one-off run failed
39 of the 95 examples that compile:

- 21 need a file or a plugin that is not on disk;
- 17 are fragments that are not a whole installer (no sections, no
  `instfiles` page, a `func` nothing calls, MUI section descriptions with no
  pages);
- 1 is a doc finding: the `versionInfo` example in script-attributes.md has
  no `FileDescription`, which is a warning, so `-WX` rejects it.

No compiler bug among them.

Found by it, and fixed:

- **Wrong diagnostic for an undefined base.** `nope.run()`, `nope.x = 1` and
  `local a, b = nope.run()` report `not-yet-implemented` instead of
  `undefined-name`. Only `detailPrint(nope)` gets the right error. — fixed
  (`an_undefined_base_is_undefined`).
- **`os.getenv`, `os.remove`, `os.rename` do not exist.**
  `concepts/lua-shaped-not-lua.md` lists them as kept, but the spellings are
  `readEnvStr`, `delete` and `rename`. Nothing checks that page's stdlib
  table. — fixed: `type`, `assert`, `math.*`, `table.*`,
  `io.open` and plain `string.gsub` were wrong too, and `os.exit` was listed
  as rejected though it works.
- **`include` is spelled two ways.** `include("lib/x.lua")` in
  program-structure.md, `include "strings/de.lua"` in lua-shaped-not-lua.md.
  — fixed: examples use the string form, like `import` and
  `plugin`; the `**Usage**` signatures keep the call form.

Found and fixed in the docs:

- **Missing commas.** Installer entries were shown without them, so they did
  not parse (sections-and-install-types, modern-ui license).
- **Stale page syntax.** nsis-shaped-not-nsis showed `page { "Directory", … }`
  with a `directoryVariable` field; neither exists.
- **Scanner false positive.** The method-call check read `inetc.cpp` in prose
  as a call; it now requires `(`.

### 2. Port the examples NSIS ships

`$NSISDIR/Examples` has about 40 scripts written to show features:
`bigtest.nsi`, `LogicLib.nsi`, `languages.nsi`, `MultiUser.nsi`,
`Memento.nsi`, `Library.nsi`, `StrFunc.nsi`, `unicode.nsi`, and the
`nsDialogs/`, `StartMenu/`, `System/`, `Modern UI/` directories. This is the
process that found the handoff issues, aimed at every feature. Keep each port
as a hand-written golden pair, and log every workaround a port needs as an
issue. One-off, not a guard.

**Started:** `tests/ports.rs` checks each port in `tests/ports/` against its
original by `makensis -V4` trace: the effect lines have to match, and pages
are left out because Installua only writes MUI2 pages. `example1`,
`example2` and `primes` port with no workaround.

**Rule:** a port that needs a missing feature or hits a bug waits. The fix
goes into Installua first, and the port follows once it is in.

Waiting on a fix:

- **`one-section`:** callbacks. `installer {}` takes `onInit` and the three
  MUI2 hooks, and nothing else: `.onSelChange`, `.onInstSuccess`,
  `.onInstFailed`, `.onVerifyInstDir`, `.onGUIEnd`, `.onRebootFailed` and
  their `un.` twins have no spelling. No census counts callbacks, so `todo 0`
  never saw them.
- **`silent`:** `file()` has no `/oname=`. The census keeps only the first
  alternative of `File`'s syntax, so the rename form was never offered.
- **`silent`:** `AllowSkipFiles` is only an `attributes {}` field, but NSIS
  lets it change between `File` lines; the example turns it off halfway
  through a section. — fixed: `file(…, { allowSkip = false })` writes it
  around that one `File` and puts the attribute's value back (golden `files`).

Not portable: `rtest` tests `GetLabelAddress` and `Call` through an
address, both rejected.

### 3. A position matrix: every kind of value in every position

A census of the grammar, not the vocabulary.

- **Value kinds:** declared control, found window (`getDlgItem`, `findWindow`),
  section, group, page, start menu page, string, int, bool, file handle.
- **Positions:** `local`, global, field read, field write, method call, argument,
  call base (`f().x`), after crossing `raw`, in the uninstaller half, inside a
  page callback.
- **Table:** one cell per kind and position, classed as accepted, rejected with a
  stated reason, or `todo`, like `src/mui/` classes the MUI2 names.
- **Test:** generate a small program for each cell and require that the result
  matches its class. A generic `not-yet-implemented` passes only in a cell
  classed `todo`. A new value kind or position fails the build until every
  cell for it is classified.

Expect the first run to turn up another batch of issues. Runs in CI.

**Done:** `tests/positions.rs`, 10 kinds × 10 positions.

- **Shape:** each position passes the value through, then uses it the way its
  kind is used. A rejection names its code and a phrase from the message, and
  that phrase is the stated reason. There are no `todo` cells: every generic
  `not-yet-implemented` turned out to be a bug.
- **Result:** 59 cells accepted, and all of them pass `makensis -WX`. 41 are
  rejected, and 2 of those are open findings, listed in `KNOWN`. A fix fails
  the test until its cells leave that list.
- **Stale note:** passing a control straight to a `func` works; handoff issue 9
  said it still errored.

Found by it:

1. **A declaration used as a value errors twice.** `local x = core` rightly
   says `core` is a section, not a value, then errors again where the value
   lands: `x` is not defined, `selected` is not a field *of a control*, or
   `make` returns 0 values. Sections, groups, pages and start menu pages; 14
   cells. — fixed: not only declarations, any failed value (`local x =
   nope`) did it. The target is now marked as already reported (a `local`, a
   global, a `func` parameter or result) and its uses stay quiet
   (`a_failed_value_is_reported_once`).
2. **A field on a file handle is reported as a field of a control**, with the
   control field list as the note (`f.size`). A known limit, stated in
   `src/lower/handle.rs`: files and windows share one `handle` type, and
   telling them apart "is a fifth type rather than a check". 2 cells. Left
   as is unless files and windows get separate types.
3. **A field on a string, int or bool is `not-yet-implemented`.** — fixed: "a
   string has no fields", in all four places a field is reached.
4. **A method on a section, group or page is `not-yet-implemented`.** — fixed:
   "`nope` is not a method of `core`".
5. **"a int has no methods"**: should be "an int". — fixed in every message
   that names a type, not only this one: `{:#}` on a `Ty` writes the article.

### 4. Review the `todo` sites

`src/lower` has 58 `self.todo(` calls (41 in `mod.rs`, 12 in `expr.rs`, 5 in
`handle.rs`), and each is a shape the compiler does not support. For each,
decide whether ordinary code would reach it: if yes, it is a missing feature;
if no, it can stay. This turns the unknown gaps into a list, and item 3
should absorb most of them. One-off.

### 5. Later: random programs

Generate random well-typed programs from the grammar, compile them, and run
`makensis -WX`. Any panic, generic `todo` or makensis error is a finding.
More work to build and noisier than 1–3, so worth it only after them.

## Suggested order

Start with 1, then 3. Both keep running in CI. 2 and 4 are one-off reviews to
do when there is time.
