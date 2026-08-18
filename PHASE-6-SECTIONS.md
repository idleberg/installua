# Phase 6 — addressing a section at install time

Scoped plan for the twelve `todo` rows that share one reason — *"addresses a section by
index: a real compile-time to install-time name binding (§13)"* — and for the construct
they need. Not a new phase: `PHASE-6.md`'s Still-open list already names the three
candidate answers and says *"the batch that lands the first row picks one"*. This is that
pick, planned.

Everything below the rulings was decided in discussion and is settled. The plan is what to
build, not what to choose.

## The rulings

1. **The handle is a Lua local, not a name the compiler invents.** Section names are
   author strings, and `PHASE-6-PAGES.md` ruling 1 is explicit about what that means —
   closed set → member access, open set → string argument, *"pages are seven names fixed
   by MUI2; sections take any name the author picks."* A compiler-owned `sections.core`
   would contradict it: reaching `"Desktop shortcut"` needs an identifier rule, and that
   rule needs a collision case (`"Core"` and `"core"` in one block) which is invisible at
   both the declaration and the use. Binding the section to a `local` retires the question
   rather than answering it — the set is never enumerated, because nothing addresses into
   it by name.
2. **A bound `section(…)` is deferred, and lowered where it is claimed.** `fn section`
   (`lower/mod.rs:2028`) takes `half`, and `fn section_in` (`lower/mod.rs:2161`) resolves
   `installTypes = { "Full" }` against the list the *block* declared. Neither is in scope
   above the block, so a section lowered where it is written would guess its half and
   report *"the installer block declares no install types"* on a program that declares two.
   The precedent is in the same struct: `Lowerer.global_inits` (`lower/mod.rs:485`) already
   holds top-level statements *"waiting for the `.onInit` they belong in."*
3. **`group(…)` returns a handle on the same terms.** NSIS gives a section group an index
   output like a section's, and `SectionGetFlags` on it reads `SF_SECGRP`, so the twelve
   rows cover groups too. Nesting is the reason this matters: under a compiler-owned table
   a group and its sections are two levels, and `sections.tools.text` is one syntax meaning
   either a field of the group or a section named `text`. A `local` has no path, so the
   ambiguity has nowhere to arise — and it stays right if `group` inside `group` ever stops
   being a `todo` (`lower/mod.rs:1995`). Writing this plan moved `group`'s own options to
   its last argument, where an optional argument belongs: the two-argument form is now the
   ordinary one instead of an `---@overload`. `section` keeps its options in the middle,
   because its last argument is a function literal and nothing can follow a block.
4. **Install types are addressed by name, not by handle.** The other four rows.
   `installTypes = { "Full", "Minimal" }` is a list of strings with no call to return
   anything, so option 3's mechanism does not reach them and does not need to: the
   name → one-based-position translation already exists in `section_in` and is the same
   §13 binding.
5. **`section` and `group` move to §15.23's positional-or-table pair.** They are the only
   two declarations in the surface that carry configuration through a *middle* table —
   `section(name, options, body)` — and that shape exists nowhere else. §15.23's rule is a
   short form with no options beside a table form with them, where the table's **array
   part is the command's parameters and its hash part is the options**:
   `file { "docs/", recursive = true }` is `File /r "docs\"`, switch for switch. So the
   name stays positional and unlabelled, the switches stay named, and the contents — a
   section's `body`, a group's `sections` — are named keys, because NSIS spells them as
   what sits between the opening line and its `End` rather than as arguments.

   ```lua
   section("Core", function() … end)              -- short form: no options
   section { "Core", required = true, body = … }  -- table form: any options at all
   group  { "Tools", expanded = true, sections = { … } }
   ```

   `Section` is `Class::Language` (`overlay.rs:1241`), so nothing in the compiler forces
   this — the `params`/`options` machinery does not drive it. The reason is that a reader
   who has learned `file { … }` should read `section { … }` without being told anything,
   and an exception whose only justification is an internal representation is invisible
   from the outside. It lands *before* the handles so the option-carrying call sites
   migrate once rather than twice.
6. **`selected` is a field, not a flags integer.** `Sections.nsh` names seven bits and
   NSIS exposes them through one `SectionGetFlags`/`SectionSetFlags` pair. Handing a user
   the integer means handing them `IntOp` and `${SECTION_OFF}`; the fields are the surface
   and the read-modify-write is the compiler's.

## The twelve rows, and the two mechanisms they split across

| row | signature | reached by |
| --- | --- | --- |
| `SectionGetFlags` / `SectionSetFlags` | `section_index [flags]` | `handle.selected`, `.readOnly`, `.bold`, `.expanded` |
| `SectionGetText` / `SectionSetText` | `section_index text` | `handle.text` |
| `SectionGetSize` / `SectionSetSize` | `section_index size` | `handle.size` |
| `SectionGetInstTypes` / `SectionSetInstTypes` | `section_index inst_types` | `handle.installTypes` |
| `GetCurInstType` / `SetCurInstType` | `inst_type_idx` | `currentInstType` |
| `InstTypeGetText` / `InstTypeSetText` | `insttype_index text` | `instTypes.getText(name)` |

Eight are section-indexed and reached through a handle. Four are install-type-indexed and
reached by the name the block declared.

## The surface

```lua
attributes { name = "Example", outFile = "setup.exe" }

local core = section { "Core",
  required = true,
  body = function()
    file("app.exe")
  end,
}

local profiler = section { "Profiler",
  optional = true,
  body = function()
    file("profiler.exe")
  end,
}

-- No options, so the short form: the pair is not two spellings of one thing.
local docs = section("Docs", function()
  file("manual.pdf")
end)

local tools = group { "Tools", expanded = true, sections = { profiler } }

installer {
  installTypes = { "Full", "Minimal" },

  page.components {},
  page.instFiles {},

  core,
  tools,
  docs,

  onInit(function()
    -- Blank a section's text to hide it: the components tree draws no row for
    -- a section with no name.
    if isServerOS() then
      docs.text = ""
    end
    -- The size an installer cannot know until it runs.
    docs.size = downloadKilobytes(url)
  end),

  onSelChange(function()
    -- The mutually exclusive pair, which is the reason most scripts reach for
    -- `SectionSetFlags` at all.
    if profiler.selected then
      docs.selected = false
    end
    tools.expanded = profiler.selected
  end),
}
```

A section's declaration order is no longer its install order: the block's positional list
decides that, and the `local`s above decide nothing. This is the cost ruling 1 accepted,
and it is the one thing a reader of an Installua program has to learn that they did not
before.

## Handle fields

| field | type | NSIS |
| ----- | ---- | ---- |
| `selected` | `boolean` | `SF_SELECTED` (1) |
| `readOnly` | `boolean` | `SF_RO` (16) |
| `bold` | `boolean` | `SF_BOLD` (8) |
| `expanded` | `boolean` | `SF_EXPAND` (32), groups only |
| `text` | `string` | `SectionGetText` / `SectionSetText` |
| `size` | `integer` | `SectionGetSize` / `SectionSetSize`, kilobytes |
| `installTypes` | `string[]` | `SectionGetInstTypes` / `SectionSetInstTypes` |

Every one is readable and writable. A write to a boolean lowers to
`SectionGetFlags` → `IntOp |` or `IntOp & ~bit` → `SectionSetFlags`, which is what
`Sections.nsh`'s `SelectSection` macro does by hand; the compiler emits the three
instructions rather than including the header, because it already owns a register
allocator and the macro clobbers `$0`.

`SF_SECGRP` and `SF_SECGRPEND` are structural — they say what an index *is*, not what a
user may change — so they are not fields. `SF_PSELECTED`, `SF_TOGGLED` and `SF_NAMECHG`
are marked internal in `Sections.nsh` and are not surface either.

`expanded` on a section rather than a group is an error naming the difference. It is the
one field that is not on every handle, and the handle knows which it is.

## Install types

```lua
currentInstType = "Minimal"          -- SetCurInstType, name → position
local chosen = currentInstType       -- GetCurInstType, position → name

instTypes.setText("Full", "Everything")   -- InstTypeSetText
local label = instTypes.getText("Full")      -- InstTypeGetText
```

`currentInstType` reads and writes a **name**, not a number, which is the whole of §13
applied a second time: the position exists in one place, and inserting a type at the front
of the block's list renumbers every use silently and correctly. Reading it when the user
has picked the custom type yields `""` — NSIS returns a position past the end, and there
is no name for it. Not `nil`: this language has no such value, and `""` is the nothing a
`Var` can hold.

`instTypes` is a compiler-owned table addressed by string, per ruling 4. It is the one
place in this plan that takes a name rather than a handle, because there is nothing to
bind.

## The claim rules

A deferred section is claimed by the block that lists it. Four writings become possible
that were not, and each needs a diagnostic:

1. **Never claimed** — a `local` that no block lists. This is the
   `installer { license = … }` bug batch 20 removed: data written at one level that
   evaporates silently if nothing reads it. An error, not a warning.
2. **Claimed twice in one block** — one section, two positions in the tree. An error.
3. **Claimed by both blocks** — the body would be lowered once with `un.` and once
   without, and the two would share a name. An error naming both blocks.
4. **Referenced from the other half** — an installer handle read inside an `uninstaller`
   callback. The section does not exist in that executable, so the reference has no
   meaning; an error, and the only one of the four that is about a *use* rather than a
   claim.

## Emission

Two changes, both about a define existing before it is expanded.

**The index output has to carry the half.** One `.nsi` holds both halves, so two sections
named `"Core"` yield two index defines. `SEC_core` twice is a redefinition warning and an
error under `-WX`; the emitted names are `SEC_core` and `UNSEC_core`.

**Sections move ahead of functions.** `Section "Core" SEC_core` defines `${SEC_core}` at
the point that line is emitted, and `Module`'s field order currently puts functions at
slot 8 and sections at slot 10 (`emit.rs:117-146`), so an `.onInit` naming `${SEC_core}`
is emitted before the line that defines it — warning 6000, an error under `-WX`. Same
class as the `Var`-before-pages fix in batch 20, and the same answer: move the definition
earlier. The comment at `emit.rs:117` says the current order *"is a readability choice,
and that is a sufficient one"* — correctness is a better one, and NSIS resolves a `Call`
to a function defined further down, so nothing else moves.

## Steps

0. **The table shape, first and on its own.** *(done)* `fn section` (`lower/mod.rs:2028`) and
   `fn group` (`lower/mod.rs:1931`) accept `section { "Core", … }` and
   `group { "Tools", … }`; the middle-table forms are deleted rather than kept beside
   them, because a pair is a short form and a long form and not three spellings. Eighteen
   call sites carry options and move — four in Lua files, fourteen in Rust test source —
   and the other eighty-seven are already the short form and do not change a byte.
1. **`ir::Section` and `ir::SectionGroup` gain `index_name: Option<String>`** *(done)*, and the
   emitter writes it as the third word of `Section` / the third of `SectionGroup`. `None`
   for a section nothing addresses, so an unaddressed program's output does not change.
2. **Swap slots 8 and 10** *(done)* in `ir::Module` and `emit.rs`, and update the slot list in
   `Module`'s doc comment. A golden diff of all five examples is the check.
3. **Deferred sections in the lowerer.** *(done)* `section(…)` and `group(…)` at top level record
   the AST against the local's binding rather than lowering it; `body_entry`
   (`lower/mod.rs:1846`) grows a case for a bare `Expr::Name` that resolves to one, and
   lowers it there with the block's `half` in hand. The four claim rules are checked when
   the last block closes.
4. **Handle fields.** *(done, except reading `installTypes` — see Out of scope.)* A field
   read or write on a section binding lowers to its instruction pair (`lower/handle.rs`);
   the boolean fields go through the read-modify-write above. `resolve.rs` needed no new
   binding kind after all: step 3's `Deferred` **is** the compile-time-only one, and what
   a body reads it through is the claim, since the claim is what says which half — and so
   which define — a handle names. Claiming therefore moved to a pass of its own, before
   any body is lowered, because `installer { onInit(…), core }` is ordinary and a body
   cannot wait for an entry written below it (§15.6). Claim rule 4 lands here.
5. **`currentInstType` and `instTypes`.** *(done)* `lower/insttype.rs`. The name → position
   lookup is shared rather than reused: `handle.installTypes` now folds through the same
   `inst_type_position`, so all four writings raise one diagnostic with one wording. Both
   names are `builtins::owned` — not constants, because there is no `$CURINSTTYPE` to
   expand, and not globals, because an unbound assignment target otherwise becomes a `Var`
   and swallows `instTypes = 3` silently. Reading `currentInstType` maps the position back
   through a compile-time comparison chain rather than `InstTypeGetText`, because
   `instTypes.setText` exists and `currentInstType == "Full"` has to survive it; past the
   end of the list is the custom type, which answers `""`.
6. **Table rows, stubs, golden, docs.** Twelve `todo` → `Exposed`; `installua.Section` and
   `installua.Group` classes in `stubs.rs`; a `sections` golden through tier 3; PREPLAN
   §13 amendment; `PHASE-6.md` batch entry.

## Census

| move | rows |
| ---- | ---- |
| `todo` → exposed | 12 |

`todo` 46 → **34**.

## Out of scope

- **`onSelChange` itself.** The surface above uses it and `body_entry` handles only
  `onInit` (`lower/mod.rs:1867`). It is a callback like the one beside it and not a design
  question, but it is a separate row and lands with this batch rather than in it.
- **`SectionIn` at install time.** `handle.installTypes` writes the list; what it cannot
  say is *"and renumber the block's declaration"*, because that is a compile-time list.
- **Group nesting.** Still the `todo` at `lower/mod.rs:1995`. Handles make it harmless
  rather than possible.
- **The flags integer.** No surface exposes it, so a program that wants a bit
  `Sections.nsh` calls internal writes `raw`.
- **Reading `handle.installTypes`.** The write lands in step 4 as a compile-time bit
  field, which is what `SectionSetInstTypes` reads. The *read* has nowhere to go: the
  field's type is `string[]` and there is no list value in this language, so
  `SectionGetInstTypes` waits for one rather than handing back the raw bit field the
  surface never promised.
- **`description = "…"` on a section.** The MUI components-page description is
  `!insertmacro MUI_DESCRIPTION_TEXT ${SEC_core} $(DESC_core)`, inside the
  `MUI_FUNCTION_DESCRIPTION_BEGIN`/`END` pair that builds `Function .onMouseOverSection`.
  It takes a section index, so this plan makes it *possible* — and step 2's
  sections-before-functions move is what makes `${SEC_core}` exist by the time that
  preprocessor comparison expands. It is not in scope because it needs `LangString`,
  which is §15.26's locale tables and unimplemented: without them the description would
  be a literal rather than `$(DESC_core)`, which is the one thing the feature is for.
