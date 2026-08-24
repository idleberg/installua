# Phase 5 — the table and the ecosystem

**Status: complete. Every exit criterion is a test.**

PLAN's exit criteria: *census green, `coverage` output is itself a golden file; every
retired-instruction row has a test asserting its diagnostic.* All three are mechanical —
[`tests/census.rs`](tests/census.rs) walks the join in both directions,
[`tests/golden/coverage.txt`](tests/golden/coverage.txt) is diffed by exact equality, and
[`tests/retired.rs`](tests/retired.rs) drives every row of the retired table through the
compiler rather than listing the ones somebody remembered.

| Task | Where | Result |
| --- | --- | --- |
| `-CMDHELP` generator (§9-5) | [`src/table/cmdhelp.rs`](src/table/cmdhelp.rs) | 276 commands parsed from a checked-in snapshot |
| Generated skeletons, checked in | [`src/table/generated.rs`](src/table/generated.rs) | reviewable per NSIS version; drift is a test failure |
| The join (§15.23) | [`src/table/mod.rs`](src/table/mod.rs) | two files, one runtime table, total in both directions |
| The hand-written half | [`src/table/overlay.rs`](src/table/overlay.rs) | 276 rows, every one classified |
| Census (§14) | [`tests/census.rs`](tests/census.rs) | nine tests; an unclassified command fails the build |
| `installua coverage` | [`tests/golden/coverage.txt`](tests/golden/coverage.txt) | the burndown, as a golden |
| `installua stubs` (§1, §15.28) | [`src/stubs.rs`](src/stubs.rs) | `---@meta`, the project meta file, the selene std |
| `installua init` (§15.13) | [`src/stubs.rs`](src/stubs.rs) | `installua.toml`, `.luarc.json`, `selene.toml` |
| The retired-instruction table (§5) | [`src/retired.rs`](src/retired.rs) | derived from the census, not a second copy |
| Example pairs (§14) | [`tests/overlay.rs`](tests/overlay.rs) | one per `Exposed` row, mandatory, one golden |

```
cargo test                                   # 79 tests
cargo run -q -- coverage
cargo run -q -- init my-installer && cargo run -q -- stubs my-installer
cargo run -q -- generate table tables/cmdhelp-3.12.txt > /tmp/g.rs && mv /tmp/g.rs src/table/generated.rs
```

The census as it stands:

| Bucket | Count | |
| --- | ---: | --- |
| `exposed` | 25 | callable, each with an example that compiles |
| `attribute` | 14 | a field of one of the blocks |
| `lowering-target` | 18 | the emitter writes it; the user writes Lua |
| `language` | 6 | `func`, `section`, `return` |
| `directive` | 37 | the preprocessor, out of the surface (§2) |
| `rejected` | 9 | deliberately unavailable, with the text that says so |
| `todo` | 167 | the only honest backlog |

---

## What the phase decided

### The generator parses; the *test* compares structures, not text

PLAN asks for the skeletons "checked in as generated code", and they are — a new NSIS
version is a reviewable diff rather than a silent change in behaviour. But the obvious test,
`generate(snapshot) == read("generated.rs")`, makes `cargo fmt` a failing test: rustfmt owns
the layout of that file and the generator does not.

So [`the_generated_table_matches_the_snapshot`](tests/census.rs) compares
`cmdhelp::parse(snapshot)` against `generated::SKELETONS` field by field. A reformat is not
a failure and a changed parameter is, which is the distinction the test is for.

### `-CMDHELP` has a shorthand, and nine rows of it had to be learned

The syntax lines are the only machine-readable description NSIS ships, and they are written
for a human. Every rule below exists because a row in 3.12 forced it, and each one was wrong
in a way that was invisible until the row was read:

1. **`HKLM[32|64]` is three registry roots**, expanded before the split on `|` — after it,
   the members come out as `HKLM[32` and `64]`, which is how this was wrong the first time.
2. **`OP=(+ - * / % | & ^ ~ ! || && << >> >>>)` separates with spaces**, because `|` is
   itself a member. Splitting it on `|` loses the two operators a user is most likely to
   get wrong.
3. **`[/x filespec [...]]` is a flag with an argument**, not a flag followed by a
   parameter — and reading it the second way shifts every later position by one.
4. **`$(user_var: …)` is an output unless it says `input`.** Keyed on the outputs, three
   commands came out backwards (`ExecWait`, `SendMessage`, `LoadAndSetImage` all spell one
   "return value"); keyed on the inputs, none do.
5. **`InstTypeIdx [InstTypeIdx [...]]` is one repeated parameter**, written twice.
6. **`state(1|0)` is one word**, and splitting at the parenthesis turned it into an
   alternation between two commands.
7. **`mode=modeflag[|modeflag[...]]` describes recursion**, not members: an unparseable
   annotation is no annotation, and the overlay says what `MessageBox`'s flags are.
8. **Five commands print English where the syntax goes** — `SubSection deprecated - use
   SectionGroup` — recognised by marker rather than by name, so a sixth in 3.13 is
   classified rather than mis-parsed.
9. **`/RESIZETOFIT[WIDTH|HEIGHT]` is a suffix that spells a different flag**, not an
   argument.

### `Kind::Label`: the fifth kind §15.23 does not have

§15.23's four kinds are `Value`, `Path`, `Enum` and `Flags`, and the generated stub for
`IfFileExists` came out as `fileExists(path, then, else)` — which is exactly the shape §15.20
exists to remove. A predicate is an ordinary `bool`-valued call; the other two positions are
§8's business and the surface does not have them at all.

So a fifth kind, and it is not a special case for three commands: it is the general fact
that a parameter can belong to the *compiler*. Four rows carry it today (`IfErrors`,
`IfFileExists`, `IfSilent`, `MessageBox`), and every branching command Phase 6 adds will.

### The retired-instruction table is the census, read differently

PLAN describes a curated table feeding `builtins::nearest`'s path. What landed is smaller:
a `LoweringTarget` or `Rejected` row **already carries** the text saying what to write
instead, because §2 requires every rejection to name its replacement and the census requires
every command to carry a reason. [`retired::all`](src/retired.rs) is a filter over the joined
table, keyed on the camelCase of the NSIS name.

That means one static table rather than two, and the same text reaches the user three ways:
the compile-time diagnostic, `installua coverage`, and the selene std's `deprecated` entry.
A row added to the overlay is a lint and a diagnostic on the same day.

```
error[nsis-retired]: `StrCmp` is not a function here
  note: write `string.lower(a) == b`, which folds to one case-insensitive compare (§15.9)
  note: NSIS spells it `StrCmp`; this compiler emits it for you
```

`Goto` is the one row that is *not* retired-diagnosable, and the reason is worth recording:
`goto` is a Lua keyword, so the frontend rejects it long before lowering sees a name. A row
there would be unreachable, and an unreachable row is a diagnostic nobody can test.

### The stubs come from the table, and the one duplicated list has a test

`installua stubs` writes three files from the joined table: the `---@meta` API, the project
meta file §15.28 obliges, and the selene std. The enum members in all three are
`-CMDHELP`'s, so completion *inside* a string literal — `fileOpen(p, "…")` offering `r`,
`w`, `a` — costs nothing to keep correct.

One list is written twice and there is no way around it: the block fields. An `Attribute`
row carries the field path a user writes (`versionInfo.product` — the dot is deliberate),
and the *Lua type* of that field is in `stubs.rs`, because a block's value is an expression
in a table rather than an argument and the parameter model has nothing to say about it. So
[`every_attribute_the_stub_offers_is_one_the_compiler_accepts`](tests/stubs.rs) compiles
every field the stub declares. A field the editor completes and the compiler rejects is
worse than no completion at all.

### An example lives on the row, and Phase 6 depends on it

§14 asks for the granular tests to be *impossible to omit* rather than merely expected. So
the example is a field on the overlay row, the census requires one on every `Exposed` row,
and [`tests/overlay.rs`](tests/overlay.rs) compiles all twenty-five into **one** program —
one section per row — diffed against a single golden.

One program rather than twenty-five, because the section body is the smallest region
containing the behaviour: the golden reads as a table of what each name emits, and a wrong
lowering shows up as three changed lines rather than as a changed file.

This is the mechanism PLAN's Phase 6 rests on. "One overlay row plus its mandatory example
pair" is data entry only if the *mandatory* half is enforced; otherwise every row is a
conversation about whether this one needs a test.

---

## Divergences from §15.23, and why

1. **`LoweringTarget` and `Language` carry text.** §15.23 spells them without a payload.
   They carry one here for the same reason `Rejected` does: a bucket that says "not
   callable" without saying what *is* leaves the reader where the generic unknown-name error
   left them. The retired diagnostic then reads that text rather than keeping a copy.
2. **`Kind::Label` is new** — the section above.
3. **An `Attribute` row's `installua` is a field path**, not a callable name. `Name` is
   `name` and `VIProductVersion` is `versionInfo.product`, which is what a user types.
4. **`conflicts` exists and is empty.** Nothing populates the mutual-exclusion sets yet
   (`File`'s `/oname=` branch against its repeated filespecs), because nothing consumes
   them: the field is in the struct so the row that needs it is a row rather than a
   redesign, and that row is Phase 6's.
5. **Alternation rows record the first alternative only.** `File`, `InstType`, `Exch`,
   `Call`, `SpaceTexts` and eight others have two spellings; the skeleton keeps one and
   marks the row `Note::Alternation`. §15.23 rules that alternation is mutual exclusion
   between *options* rather than a second parameter list, so this is the shape it asked
   for — with (4) still owing the sets.

---

## Still open, and named rather than buried

- **The joined table is not yet the emitter's source of truth.** `builtins.rs` still holds
  the parameter types the lowerer reads, and the overlay holds them again. The census keeps
  the two from disagreeing about *names* — nothing retired is callable, everything exposed
  reaches the stub and the std — but not about types, and §15.23's whole argument is that a
  consumer reading two tables can see half an entry. Rewiring `lower` onto `table::table()`
  is the natural first task of Phase 6, and it is a refactor with a test suite already
  around it rather than a design question.
- **The examples are tier 2, not tier 3.** They compile and diff; they are not assembled,
  because `File "assets\icon.ico"` needs a file to exist and a fixture per row is a
  different kind of test. The five programs are the tier-3 gate and remain so.
- **`installua stubs` scans one directory.** A project with `sections/` beside `install.lua`
  gets stubs for the top level only. Recursion is four lines and a decision about what a
  project root *is* — which `installua.toml` now marks, so the decision has somewhere to
  live.
- **The `todo` reasons are grouped, not individual.** 167 rows carry fifteen distinct
  sentences between them. That is honest — the reason `SectionGetFlags` is not done is the
  same reason `SectionSetText` is not — but it means the burndown's diff shows a name
  leaving a group rather than a reason being retired.
- **No test asserts the census against a *second* NSIS version.** The snapshot-refresh test
  diffs against whatever `makensis` is on `PATH` and skips when there is none, which catches
  drift on a developer's machine and not in CI. That is the same trade §14 makes for
  assembling, and it ages the same way.
