# Compile-time instructions

Installua has no preprocessor, and `docs/reference-map.md` lists all 37 `!`
directives as rejected. This document is the case for reopening part of that
decision, the evidence it rests on, and the order it should be done in.

The thesis: **most of what the corpus spends `!` directives on is not
compile-time control flow at all.** It is build parameters and path
configuration wearing a directive costume. Retire those two first and the
remaining feature is small enough to add without giving up the emitter's fixed
ordering — which is the property users actually benefit from and the one thing
this plan will not trade.

---

## 1. Evidence

Measured against `/Users/jan/Repositories/_nsis/nsis-corpus`, 984 real-world
`.nsi` files, counting line-initial directives only.

### 1.1 Raw counts

| Directive | occurrences | files | | Directive | occurrences | files |
| --- | ---: | ---: | --- | --- | ---: | ---: |
| `!insertmacro` | 15094 | 876 | | `!system` | 89 | 48 |
| `!define` | 14614 | 934 | | `!addplugindir` | 86 | 66 |
| `!include` | 2845 | 916 | | `!undef` | 65 | 30 |
| `!ifndef` / `!ifdef` / `!if` | 1704 | 228 | | `!error` / `!echo` / `!warning` | 83 | 33 |
| `!macro` … `!macroend` | 374 | 186 | | `!addincludedir` | 41 | 36 |
| | | | | `!packhdr` / `!finalize` / `!uninstfinalize` | 34 | 30 |

Six directives never appear at all: `!execute`, `!assert`, `!elseif`,
`!ifmacrodef`, `!gettlbversion`, `!searchreplace`. Zero files use include
guards.

### 1.2 The counts collapse under classification

The headline numbers describe something Installua already does.

- Of 15094 `!insertmacro`, **11600 (77%) are `MUI_*` / `LANGFILE`** — pages,
  `MUI_LANGUAGE`, `MUI_DESCRIPTION_TEXT`, startmenu. Another 643 are stdlib
  (`GetParameters`, `UnselectSection`, `UpgradeDLL`). 2848 are user macros.
- Of 14614 `!define`, **6278 are `MUI_*` settings** and ~4542 are
  `PRODUCT_NAME`-shaped constants. The top names are `PRODUCT_NAME` (773),
  `MUI_ICON` (677), `PRODUCT_VERSION` (584) — a page setting and a
  `local X <const>`.
- Of 2845 `!include`, the top 15 targets are all stock headers (`MUI2.nsh` 390,
  `LogicLib` 216, `x64` 212, `FileFunc` 166). Custom headers are a long tail led
  by `nsProcess.nsh` (25), `FileAssociation.nsh` (22), `EnvVarUpdate.nsh` (20),
  `UAC.nsh` (16).

### 1.3 Per-file verdict

What each script would need beyond today's Installua:

| | files | share |
| --- | ---: | ---: |
| A — declarative only (defines + stock includes + MUI macros) | 555 | 56% |
| B — + user macros / non-MUI `!insertmacro` | 156 | 16% |
| C — + conditional compilation | 180 | 18% |
| D — + build-side directives (`!system`, `!packhdr`, `!finalize`, `!tempfile`, `!getdllversion`) | 93 | 10% |

### 1.4 The largest single idiom is not conditional compilation

```nsis
!ifndef VERSION
  !define VERSION "1.4.2"
!endif
```

**467 occurrences in 101 files** — over a quarter of every conditional in the
corpus. This is a build parameter with a default, supplied by
`makensis /DVERSION=…`. It wants a CLI flag and a declaration. It has no
ordering semantics whatsoever.

What the rest switch on: feature flags `HAVE_*`/`WITH_*`/`NO_*` (167 occ, 30
files), arch/bitness (151 occ, ~50 files), debug/release (104 occ, 20 files),
`$%ENV%` inside `!if` (122 occ but only **11 files**, essentially all
VirtualBox). Concentration is extreme — the top 10 files hold 26% of all
conditionals, and 5 of those 10 are vendored copies of the same
`VBoxGuestAdditions.nsi`.

### 1.5 Redefinition confirms the hazard Installua removes

`!undef` appears 65 times in 30 files. The most-redefined symbols are
`MUI_PAGE_CUSTOMFUNCTION_PRE` (19 files), `..._LEAVE` (14), `..._SHOW` (10) —
the define/insert/undef dance around each page. That is exactly the ordering
trap the emitter's page interleaving exists to prevent, and it is the top
`!define` hazard in real scripts.

### 1.6 Reproducing these numbers

All figures come from line-initial regex scans over `*.nsi` in the corpus root,
plus a syntactic-context walk that tracks `Section`/`Function`/`!macro` nesting.
Nothing here was sampled; every number is a full-corpus count. Re-derive before
trusting any of it in a later NSIS version.

---

## 2. Where the order problem actually is

It is entirely at **top level**, and the accurate statement is not "Installua
cannot guarantee order" — it is that **Installua deliberately guarantees a
different order than you wrote.**

`src/emit.rs` is a fixed spine. Everything a module contains is bucketed and
emitted in this sequence regardless of source position:

| # | Slot | Ordering rule |
| ---: | --- | --- |
| 1 | `Unicode` | first, so a later `raw` overrides rather than being overridden |
| 1b | `!addplugindir`s | after slot 1: an untagged one binds to the target current at that point |
| 2 | `!define`s | source order |
| 3 | `!include`s | — |
| 4 | header init lines (`${Using:…}`) | — |
| 5 | attributes | **overlay-table order, not source order** |
| 6 | `Var`s | hoisted out of bodies; NSIS rejects a `Var` used before declaration |
| 7 | MUI defines → pages → `MUI_LANGUAGE` → license langstrings → `ReserveFile` | each page's own defines interleave at its insertion point |
| 8 | `InstType`s | declaration order — that order is their identity |
| 9 | Sections | source order — install order is user-visible |
| 9b | descriptions | after sections: each expands a section-index `!define` |
| 10 | Functions | last |

The spine encodes NSIS's real preprocessor constraints as compiler invariants.
Sections precede functions because `Section "Core" SEC_core` is what `!define`s
`${SEC_core}`, and a textual preprocessor means an `.onInit` reading it must be
emitted afterwards. Page defines interleave because MUI2 reads a page-scoped
`!define` at the insertion point. §1.5 is the evidence that this is worth
keeping.

### 2.1 Inside sections and functions, order *is* guaranteed

`src/layout.rs` walks the CFG in source order. It only elides `Goto`-to-next-line
and drops dead blocks. `raw [[ … ]]` lowers to an opaque call site pinned exactly
where it was written.

Two exceptions, both hoists rather than reorderings:

- `Var` leaves the body for slot 6.
- `SectionIn` / `AddSize` float to the top of their section — NSIS treats them
  as declarations spelled as instructions.

### 2.2 The actual gap

`raw` already exists and is already order-exact. It is **body-only**. There is
no top-level escape hatch of any kind. That, not ordering, is what is missing.

---

## 3. Rejected design: `preScript` / `preIncludes` / `preMui` blocks

Blocks that guarantee insertion order for compile-time text, named after regions
of the output. Two reasons not to:

**It answers the wrong question.** Such blocks order user text relative to
*other user text*. Every real ordering hazard in NSIS is user text relative to
*compiler-generated* lines: a `!define MUI_WELCOMEPAGE_TITLE` against the
generated `!insertmacro MUI_PAGE_WELCOME`; a `Var` against a generated `DirVar`.
Three coarse buckets cannot express that, and `preMui` is ambiguous across at
least five distinct positions inside slot 7 alone.

**The names ossify the spine.** `preIncludes` is a public promise about slot 3
that constrains every future change to slots 1–4.

The replacement is the same idea at the right granularity: **named anchors on
the existing spine**, each one a documented boundary between two numbered slots
rather than a vague region. See Phase 2.

---

## 4. The plan

Ordering principle: do the things that *remove* directives from the language
question before the thing that adds language surface.

### Phase 0 — `!addplugindir` needs nothing from the language (done)

`lower::plugins_dir` already turns `plugin` declarations into
`ReserveFile /plugin`, and the DLL name is the namespace — "the same lookup
`!addplugindir` does". The 66 corpus files need nothing from the language.

**A plugin outside `NSISDIR` is not reachable today.** Verified: the `.toml`
declaration fields are `name`/`method`/`nsis`/`params`/`outputs` and none names
a file, `installua.toml` is `name`/`entry`, and `assemble` runs `makensis -WX`
with no further flags. Both halves of what the compiler generates fail, and
differently — `Plugin not found, cannot call nsFoo::ExecToStack` at the call
site, `no files found` at the `ReserveFile /plugin`. So the two passes are
correct as written; the DLL merely has to be findable.

**It is a config key plus one compiler-owned line.** The path goes on the
`[[plugin]]` block in `.installua/headers/*.toml` as `dir`, resolved against the
project root and absolutised — `makensis` resolves a relative plugin directory
against its working directory, and `assemble` sets that to the emitted script's
parent, not the project root. `lower::addplugindir` emits one line per
directory, deduplicated.

**One correction to this plan, found in the writing.** It said `lower::reserved`
should emit the line "in the same pass that writes their `ReserveFile /plugin`",
reusing that pass's reachability walk. That is wrong, and the plan's own §1
evidence says so: the two failures it lists happen at *different* times.
`no files found` at the `ReserveFile` is about the data block, so reachability
from an init callback is the right question there. `Plugin not found, cannot
call Foo::Bar` is about `makensis` locating the DLL at all, and **every call
site** raises it — a plugin only a section calls would have compiled to a hard
error under the pass as planned. So it is a second pass over `body.plugins` for
every body, and the two sets are different on purpose.

`!addplugindir` also moved from `directive(…)` to
`lowering("!addplugindir", …)` in `src/table/overlay.rs` — it is now a real NSIS
line in the output that nobody spells, which is what that class means. The
census reads 36 directives and 29 lowering targets, and `docs/reference-map.md`
now says "36 of the 37".

**The position is forced, and it is slot 1b.** An untagged `!addplugindir` binds
to whichever target is current *when the directive is processed* — not to the
DLL's charset, which `makensis` never inspects, and not lazily at the call site.
The same directory with the same DLL resolves or does not resolve purely on
where the line sits:

| | |
| --- | --- |
| `!addplugindir` then `Unicode false` | plugin not found |
| `Unicode false` then `!addplugindir` | builds |
| `Unicode true` then `!addplugindir` | builds |
| call site, then `!addplugindir` | plugin not found |

So the window is after slot 1 and before any call site, which makes it a new
slot 1b between `Unicode` and the `!define`s — the same reason slot 1 leads at
all. `/target` is only a way to name a target other than the current one, which
a compiler that owns `Unicode` never needs.

Two consequences worth stating. A `makensis -X"!addplugindir …"` flag is *not*
an equivalent: `-X` is processed before the script, therefore before `Unicode`,
therefore binds to the default target and silently breaks every
`unicode = false` build. And the "a later `raw` overrides it" contract on slot 1
now has a second line depending on it — a `raw` that flips `Unicode` to false
after slot 1b desynchronises the target.

### Phase 1 — Build parameters (done)

**Why first:** 467 uses across 101 files, the largest `!`-idiom in the corpus,
and it has *zero* interaction with the spine. Frontend and CLI only. Nothing in
`src/emit.rs` moves.

`BuildArgs` in `src/main.rs` is input + output today, with no `-D` equivalent.

```lua
local VERSION <const> = param("VERSION", "1.4.2")
```

```console
$ installua build install.lua -D VERSION=2.0.0
```

A param is a constant with a settable default. It folds in the frontend exactly
as `<const>` already does, so it is not compile-time control flow and introduces
no ordering question at all. An unknown `-D` is a diagnostic — silent-miss is
precisely the `!ifndef` failure mode being retired.

*Alternative considered:* a `[params]` table in `installua.toml`. Rejected —
`src/stubs.rs` describes that file as "the one file a user has to know about",
and a params table makes it two things. The declaration also belongs next to the
use.

**Docs:** `docs/reference-map.md`. The "all 37 `!` directives are out" paragraph
needs rewording rather than editing: for `!define`/`!ifndef`/`!undef` the answer
becomes "you do not need them", not "they are rejected". Census-visible — those
are `directive(…)` rows in `src/table/overlay.rs`.

**Two things the plan did not say, decided in the writing.**

*The default is the type declaration.* There is nowhere else for one to live,
and without it `-D PORT=abc` against `param("PORT", 8080)` would reach an
`IntOp` as a string and NSIS would read it as zero. So the override text is
coerced against the folded default and an ill-typed one is an error — which is
also why the override is applied inside the `<const>` worklist rather than in
`fold`: that is the first moment the default has a type.

*`param` is a declaration, not an expression.* Legal only as the whole
initialiser of a top-level `local … <const>`. Composition moves one line down
(`local FULL <const> = VERSION .. "-win64"`), which costs nothing because
resolution is order-free — and the gain is that the set of parameter names is
known before anything folds, which is the only reason `-D` can be checked at
all. A `param` in a body or buried in a larger expression gets its own
`param-form` diagnostic rather than "not defined" or "not a build-time
constant", both of which point at the wrong half of the line.

The census did not move: `directive(…)` carries no reason string, so no bucket
changed and `tests/golden/coverage.txt` is untouched. The reference's directive
paragraph was reworded, and the "written by the compiler" table gained a
`!define` row — a top-level `<const>` already emitted one, and a search for the
NSIS name had nowhere to land.

### Phase 2 — Top-level `raw` with anchors (done)

**Why second:** unblocks most of category D's 93 files with no new language
semantics, and de-risks Phase 3 by giving users an exit while the real feature is
designed.

```lua
raw.head [[ !system 'git rev-parse --short HEAD > rev.txt' ]]
raw.tail [[ !packhdr "tmp.dat" '"upx.exe" "tmp.dat"' ]]
```

**Start with exactly two anchors**, `head` (before slot 1) and `tail` (after
slot 10). That covers the corpus honestly: `!system` / `!tempfile` /
`!getdllversion` want `head`; `!packhdr` / `!finalize` / `!uninstfinalize` are
position-independent registrations and `tail` serves them. Add interior anchors
only when a real script demands one, and name each after a **documented boundary
between two numbered slots** — never after a region like "mui".

`head` genuinely precedes slot 1, and that is verified rather than assumed:
`!system` and `!tempfile` above a `Unicode` line assemble clean, while
`!include MUI2.nsh` above one is a hard error — *"Can't change target
architecture after data already got compressed"*. Slot 1 leads because it is the
last point at which the target can still be set, and everything MUI touches is
already past it.

**The rule that governs which anchor an escape hatch may have**, and the reason
Phase 0 does not use one: *anchors are for text whose meaning is
position-independent; text whose meaning depends on compiler-generated state is
a declaration the compiler places.* `!addplugindir` is the first concrete member
of the second class — written by a user at `head` it would land above `Unicode`,
bind to the default target, and silently break every `unicode = false` build.
That is the `!ifndef` silent-miss failure mode wearing a new costume, and an
anchor that permitted it would be selling the spine's guarantee back to the
user. When a directive turns out to be position-sensitive, the answer is a slot,
not an anchor.

**The invariant to preserve:** `lower::plugins_dir` already scans `raw` text for
`$PLUGINSDIR`, because that is "the one place a `$PLUGINSDIR` can arrive without
having passed through `builtins::CONSTANTS`". Top-level `raw` must get the same
scan, or it becomes an escape hatch into the exact failure that pass prevents.

**Testing:** Tier 3 matters more than usual. `raw` at `head` can break `makensis`
in ways no unit test sees.

**Three things the plan did not say, decided in the writing.**

*The anchor is part of the name, not a field in a table.* The plan wrote
`raw{ at = "head", [[ … ]] }`; what shipped is `raw.head [[ … ]]`, on the
`page.welcome {}` precedent. It makes the anchor **un-omittable by
construction** — there is no form of the declaration that carries no anchor, so
"required" is a fact about the grammar rather than a rule with a check behind
it. `at` was also the wrong word: it reads as a coordinate, and coordinates are
what an anchor must never be, since the numbered slots are the compiler's. The
cost, stated plainly: `raw` becomes the first name that is both callable and
namespaced, and the two forms mean different things — which is the point, since
`raw [[ … ]]` in a body means *here* and the top level has no *here*.

*`$PLUGINSDIR` at an anchor is a diagnostic, not a scan.* The plan said
top-level `raw` "must get the same scan". It cannot: that pass works by
inserting an `InitPluginsDir` above the statement that names the directory, and
an anchor is outside every body — there is no statement and nowhere to insert.
So the invariant is held by refusing the text instead, which is the same
guarantee reached the only way this position allows.

*The census did not move.* `directive(…)` rows in `src/table/overlay.rs` carry
no reason string — the staging rule is the reason — and reachability through
`raw` was never what the class meant. Six directives now have a *position* to be
written at, which the reference says; no bucket changed and
`tests/golden/coverage.txt` is untouched.

### Phase 3 — `if` over constants at top level

**Why last:** the only genuinely new language surface, and Phases 1–2 shrink its
job from 180 files to roughly 80.

```lua
if param("ARCH", "x86") == "x64" then
  attributes{ cpu = "amd64" }
  installer{ section("Core64", …) }
else
  installer{ section("Core", …) }
end
```

**The design point that protects the spine: fold before bucketing.** A top-level
`if` over `<const>` / `param` values resolves in the frontend, so by the time
`lower` hands a module to `emit`, the conditional no longer exists — there is
nothing left to order. This is not a directive surviving into the output; it is a
branch the compiler takes.

The constraint to enforce: the condition must fold to a literal. A condition
mentioning a runtime register is an error, with a note pointing at the runtime
`if`. That is the same rule `<const>` already lives under.

**Scope from the corpus:** feature flags (30 files), arch/bitness (~50),
debug/release (20). **Not** `$%ENV%` inside `!if` — 11 files, all VirtualBox, and
`-D` from the build script covers it.

---

## 5. Non-goals

**Text-substitution macros.** The 2848 non-MUI `!insertmacro` sites. Functions
plus `import` already cover the ones expanding to *instructions*; what remains
expands to *declarations*, which is a real preprocessor with real ordering
consequences. Keeping the no here is what protects the spine.

If a specific stock header keeps recurring — `nsProcess.nsh` (25 files),
`FileAssociation.nsh` (22), `EnvVarUpdate.nsh` (20) — declare it in `import`
the way `FileFunc` and `WordFunc` already are. One at a time, on evidence.

**Region-named ordering blocks.** See §3.

**`$%ENV%` interpolation.** See Phase 3.

---

## 6. Open decisions

| # | Decision | Recommendation | Blocks |
| ---: | --- | --- | --- |
| 1 | ~~Params declared in-source or in `installua.toml`?~~ | **closed: in-source.** `param(name, default)`, as the whole initialiser of a top-level `<const>` — which is what gives `-D` a set of names to be checked against | — |
| 2 | ~~Undeclared `-D`: error or ignored?~~ | **closed: error.** `unknown-param`, naming the parameters that do exist | — |
| 3 | ~~Anchor vocabulary — two, or more from the start?~~ | **closed: two.** `raw.head` and `raw.tail`, spelled into the callee so the anchor cannot be left off; a third arrives when a real script needs one | — |
| 4 | ~~Is a non-`NSISDIR` plugin directory reachable today?~~ | **closed: no.** A `dir` key on the `[[plugin]]` declaration, emitted by `lower::reserved` as `!addplugindir` at slot 1b | — |
| 5 | ~~Is the `dir` key per-`[[plugin]]` block, or one project-wide list?~~ | **closed: per-block.** It sits with the declaration it belongs to, and only a plugin the program *calls* emits a line | — |

---

## 7. What this plan will not trade

The fixed spine. It is the reason a user never has to think about
define-before-insert, `Var`-before-use, or section-index-before-`.onInit` — the
three hazards §1.5 shows real scripts getting wrong. Every phase above is
designed so that the compile-time construct is gone before bucketing begins, or
is pinned to a boundary the spine already documents.
