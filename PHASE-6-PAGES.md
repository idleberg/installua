# Phase 6 — the page surface

Scoped plan for the `page.*` construct, the MUI2 settings that hang off it, and the
fourteen `todo` rows it retires. Not a new phase: `PLAN.md:218` lists *"the ~70 `MUI_*`
settings"* inside Phase 6's own design-work table. This is that entry, planned.

Everything below the first section was decided in discussion and is settled. The plan is
what to build, not what to choose.

## Built — and what building it changed

Batch 20. All six rulings hold and all six steps landed; `todo` 60 → **46**, as predicted.
Four things the plan did not know:

1. **MUI2's cleanup has two holes, and the plan had the wrong two.** The classification
   rule below is right, but `Directory.nsh` clears `MUI_DIRECTORYPAGE_VARIABLE` and
   `…_VERIFYONLEAVE` after all — through `MUI_UNSET` rather than `!undef`, which a grep
   for `undef` misses. The settings MUI2 really leaves standing are
   `MUI_UNCONFIRMPAGE_VARIABLE` and the two radio button texts, whose `MUI_UNSET` names a
   define that does not exist. Emitting an `!undef` for a define MUI2 already cleared is
   warning 6155 and an error under `-WX`, so tier 3 caught it.
2. **`Var`s moved ahead of the pages.** `page.directory { variable = target }` puts a
   global in a `DirVar`, and `Module`'s field order had the `Var` lines *after* the pages.
   Globals are slot 6 now.
3. **`!define` grew an optional value.** `MUI_DIRECTORYPAGE_VERIFYONLEAVE` is read by
   `!ifdef` and never expanded, so the setting is the define's existence and `false` is its
   absence.
4. **The four once-global settings are `installer {}`-only.** Written in both blocks they
   would define one name twice. `installer.checkBitmap` is the field path in the overlay,
   and the dotted name is also what keeps them out of `attributes {}`, where a raw
   `CheckBitmap` line would lose to MUI2's.

## The rulings

1. **`page.directory { … }`, not `page("Directory", …)`.** Closed set → member access;
   open set → string argument. Pages are seven names fixed by MUI2; sections take any name
   the author picks. This sharpens §15.1's `lang.X` rule, which until now justified itself
   only by "completion works" without saying when it is available.
2. **A page is a positional entry in `installer {}` / `uninstaller {}`**, symmetric with
   `section(…)` (§15.10). Not a top-level declaration: a floating `page {}` cannot know
   which half it belongs to, and giving it a flag or a second `unpage {}` construct
   reintroduces the prefix §15.3 abolished. The block supplies the half; `un.` stays
   unwritten.
3. **`pages = { … }` is removed, not kept as a shorthand.** One construct, no conversion
   cliff when a page grows its first setting, and no ordering question between a named
   field and a positional entry.
4. **`license` leaves the block.** It is page data written at block level — `installer {
   license = … }` is stashed at `lower/mod.rs:1168` and read by exactly one page at
   `lower/mod.rs:1326`, and evaporates silently if no License page is listed. It becomes
   `page.license { file = … }`, where that mistake is unwritable.
5. **`text = { … }` is dropped from `V1_INSTALLER_FIELDS`.** It was reserved for the
   block-level spelling that lost: four of the nine settings are not text, so it needed
   sibling groups and a per-setting sorting rule, and every key had to carry its page as a
   prefix because the block is flat.
6. **`headerText` / `headerSubText` on the page; `caption` stays on the block.** Three
   different strings on one window — the title bar (`Caption`), the bold heading strip
   inside the page (`MUI_PAGE_HEADER_TEXT`/`_SUBTEXT`), and the per-page title-bar
   override (`SubCaption`, which MUI2 blanks at one index). No page field is named
   `caption`, because it would read as either of two and mean neither.

## The classification rule

MUI2's own source draws the line, so no setting needs a judgement call:

```nsis
; PAGE-SCOPED — written inside PageEx, undefined after
PageEx directory
  DirText "${MUI_DIRECTORYPAGE_TEXT_TOP}" "${MUI_DIRECTORYPAGE_TEXT_DESTINATION}"
PageExEnd
!undef MUI_DIRECTORYPAGE_TEXT_TOP

; ONCE-GLOBAL — written inside the INTERFACE macro, !ifndef guarded
!macro MUI_COMPONENTSPAGE_INTERFACE
  !ifndef MUI_COMPONENTSPAGE_INTERFACE     ; runs on the FIRST components page only
    CheckBitmap "${MUI_COMPONENTSPAGE_CHECKBITMAP}"
  !endif
```

**A setting MUI2 `!undef`s is a page field. A setting it guards with `!ifndef` is a block
field.** The second kind is applied once, on the first page of its type, and ignored on
every later one — so putting it on the page would be a lie a second page tells silently.

## The surface

```lua
attributes { name = "Example" }

installer {
  icon        = "app.ico",
  checkBitmap = "check.bmp",
  installColors = "00FF00 000000",

  page.welcome {},
  page.license { file = "LICENSE.txt", bottomText = "Accept to continue." },
  page.components { topText = "Pick what to install." },
  page.directory { topText = "Choose a location.", verifyOnLeave = true },
  page.instFiles {},
  page.finish {},

  section("Main", function() … end),
  onInit(function() … end),
}

uninstaller {
  page.confirm { topText = "Example will be removed." },
  page.instFiles {},

  section("Main", function() … end),
}
```

Page order is source order. `page.finish` exists only in `installer {}` and `page.confirm`
only in `uninstaller {}` — already encoded in `Page::halves` at `lower/mod.rs:135`, and as
a field access an invalid one is an editor-time error rather than a compile-time one.

## Page fields

| NSIS | page | fields |
| ---- | ---- | ------ |
| `ComponentText` | `components` | `topText`, `instTypeText`, `listText` |
| `DirText` | `directory` | `topText`, `destinationText` |
| `DirVar` | `directory`, `confirm` | `variable` |
| `DirVerify` | `directory` | `verifyOnLeave` |
| `LicenseText` | `license` | `bottomText`, `button` |
| `LicenseForceSelection` | `license` | `checkbox`, `radioButtons` |
| `UninstallText` | `confirm` | `topText`, `locationText` |
| `LicenseData` | `license` | `file` |
| *(no NSIS row)* | any | `headerText`, `headerSubText`, `pre`, `show`, `leave` |

## Block fields — the four that look page-scoped and are not

| NSIS | MUI define | field |
| ---- | ---------- | ----- |
| `CheckBitmap` | `MUI_COMPONENTSPAGE_CHECKBITMAP` | `checkBitmap` |
| `InstallColors` | `MUI_INSTFILESPAGE_COLORS` | `installColors` |
| `InstProgressFlags` | `MUI_INSTFILESPAGE_PROGRESSBAR` | `progressBar` |
| `LicenseBkColor` | `MUI_LICENSEPAGE_BGCOLOR` | `licenseBkColor` |

## Rejected

| NSIS | reason |
| ---- | ------ |
| `PageEx` / `PageExEnd` | the classic page block MUI2 generates around every page; writing it means reimplementing MUI2, and §15.7 leaves classic pages to `raw` |
| `PageCallbacks` | legal only inside `PageEx`, and MUI2 fills it with its own generated function names |

Their current reason — *"a page construct; custom pages need a design (nsDialogs) that does
not exist yet"* — is a fourth group reason that does not describe its rows. `Page` and
`UninstPage` keep it and stay `todo`: their signature is an alternation whose `custom` half
is the nsDialogs insertion point, and MUI2 ships no `MUI_PAGE_CUSTOM` macro, so that half
is real work rather than a rejected spelling.

## Steps

1. **`ir::Module.pages` / `.unpages`: `Vec<Instruction>` → `Vec<Page>`**, each page owning
   its own defines. The comment at `ir.rs:38-41` already names the gap this closes — *"a
   page-scoped one would have to sit between two of them"*. `mui_defines` keeps the
   block-level ones (`MUI_ICON` and friends) and loses the page-scoped ones. The ordering
   invariant moves from "one list before another" to "each page owns its defines", which
   is strictly easier to keep right.
2. **`page.*` in the lowerer.** `V1_PAGES` (`lower/mod.rs:128`) gains a field table per
   page; `fn pages` (`lower/mod.rs:1271`) becomes the positional `page.*` handler;
   `body_entry` dispatches it beside `section`.
3. **Field surface.** `V1_INSTALLER_FIELDS` loses `pages`, `license`, `text` and gains
   `checkBitmap`, `installColors`, `progressBar`, `licenseBkColor`.
4. **Table rows.** Seven `todo` → page `Attribute`, four `todo` → block `Attribute`, three
   `todo` → `Rejected`. `LicenseData` moves from block attribute to page field.
5. **Examples and goldens.** Five example scripts use `pages =` (7 lines) plus
   `tests/map.rs`; regenerate `coverage.txt`, `overlay-examples.{lua,nsi}`,
   `overlay-attributes.{lua,nsi}` and `LANGUAGE.md`.
6. **PREPLAN §15.7 amendment note.** Its simple `pages = { … }` form goes away; the
   `page {}` sketch is promoted and re-spelled as `page.*`. An amendment, not a rewrite.

## Census

| move | rows |
| ---- | ---- |
| `todo` → page attribute | 7 |
| `todo` → block attribute | 4 |
| `todo` → rejected | 3 |

`todo` 60 → **46**.

## Out of scope

- **`Page custom` / nsDialogs.** No design exists; `Page` and `UninstPage` stay `todo`.
- **`MUI_PAGE_STARTMENU`.** Absent from `V1_PAGES`. Under a bare string list a page with
  macro arguments was unspellable; under `page.*` it is just another page with fields, so
  it becomes possible here and lands later.
- **`SubCaption` / `UninstallSubCaption`.** MUI2 blanks one index of nine. The blocker is
  the table's shape — a row owned for one argument value and open for the others — not the
  page world.
- **The `include {}` block.** Include order is already guaranteed for MUI2 by `ir::Module`
  being a nine-slot ordered struct (`ir.rs:5-23`): `MUI2.nsh` first and deduplicated, every
  installer page before every uninstaller page whatever order the blocks were written in,
  `MUI_LANGUAGE` last. Third-party macro order (LogicLib, x64.nsh, plugin headers) is a
  rule for that block, and it is unimplemented — `V1_BLOCKS` lists it and the dispatch
  falls through to a `todo` at `lower/mod.rs:461`.
- **`raw` holding `!insertmacro MUI_PAGE_*`.** The one hole in the ordering guarantee,
  unavoidable by definition. A warning when raw text contains `MUI_PAGE` or `MUI_LANGUAGE`
  would close it and is not planned here.
