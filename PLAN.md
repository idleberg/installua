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

### 5.35 No Windows binary for a release that has the fixes

The one release is v0.1.0, without any of §5. A program that runs
`installua build` on the user's machine (PimpBot's Compiler) ships
`installua.exe`, and has to cross-build it from a checkout
(`cargo build --release --target x86_64-pc-windows-gnu`, which works and ran
under Wine with a portable NSIS 3.12). Should: each release attaches
`installua-x86_64-pc-windows-gnu.exe` (or a zip) with its SHA-256, so it can be
pinned the way PimpBot pins 7-Zip and curl. The release workflow is the place.

## Later: random programs

Generate random well-typed programs from the grammar, compile them, and run
`makensis -WX`. A panic, a generic `not-yet-implemented` or a `makensis` error
is a finding. Worth it only once the items above are done.
