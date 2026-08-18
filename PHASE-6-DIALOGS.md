# Phase 6 — the window surface, and the custom page it needs

Scoped plan for the fourteen `todo` rows filed under *"addresses a window by handle; the
`hwnd` surface wants nsDialogs designed first"* and its two neighbours. It is the last
group in `PHASE-6.md`'s Still-open list — 14 of the 35 rows left — and the third one whose
written reason turns out to be a guess about every member (`overlay.rs:943`: *"A group
reason is a guess about every member; the syntax line un-guesses it"*, written when
`HideWindow` and `LockWindow` walked out of it).

Read `PHASE-6-SECTIONS.md` first. This plan reuses its machinery rather than inventing a
second copy: a control is a handle bound to a `local` and claimed by the construct that
lists it, which is §13 one construct over.

## The rulings

1. **Five of the fourteen are not blocked by anything, and the group reason is wrong for
   the third time.** The syntax lines un-guess them, one at a time:

   | row | syntax | why it is not a handle question |
   | --- | --- | --- |
   | `AutoCloseWindow` | `(false\|true)` | a compile-time attribute, whose runtime twin `SetAutoClose` is already exposed |
   | `BringToFront` | *(no arguments)* | raises the installer's own window |
   | `FindWindow` | `$var class [title] …` | *produces* a handle; nsDialogs is about windows this installer creates |
   | `IsWindow` | `hwnd jump jump` | a fusable predicate over one, which §8 already knows how to lower |
   | `SetBrandingImage` | `[/IMGID=…] bitmap.bmp` | its reason says *"there is no way to write a page callback yet"*, and batch 20 gave every page `pre`, `show` and `leave` |

   These land first, in a batch that needs none of the design below. `brandingImage` is
   already an attribute (`overlay.rs:331`), so the fifth has both halves.

2. **A custom page is a page, reached by the same member access.** `page.custom { … }` —
   an eighth member of the set §15.7 calls closed, and it stays closed: `custom` is a name
   NSIS fixes, and what is open lives *inside* it as controls. Not a new top-level
   construct and not a second page world; a floating dialog cannot know which half it
   belongs to, which is the same argument that made a page a positional entry.

3. **A control is a `local`, claimed by the page that lists it.** Ruling 1 of
   `PHASE-6-SECTIONS.md`, verbatim, one construct over: control names are author strings,
   so a compiler-owned `controls.serial` would need an identifier rule and a collision
   case, and binding to a `local` retires the question instead of answering it. The four
   claim rules — never claimed, claimed twice in one page, claimed by both halves,
   referenced from the other half — are the same four, checked by the same pass. This is
   the payoff for having done sections first, and the reason this plan is short.

4. **The creator function is the compiler's, and the controls are declarative.** A user
   writes neither `nsDialogs::Create`, nor the `Pop`, nor the `== error` check, nor
   `nsDialogs::Show`. Those four steps are a protocol whose order is invariant and whose
   omissions are silent — a missing `Show` yields a page that flashes past — which is the
   same reason `page {}` owns its `!define`s rather than trusting the author to sequence
   them (§15.7).

5. **Nothing is `!include`d.** `${NSD_CreateLabel}` is a macro that expands to
   `nsDialogs::CreateControl` with a style word inlined; the compiler emits that plugin
   call with the style word inlined and includes no header, exactly as it emits
   `SectionGetFlags`/`IntOp`/`SectionSetFlags` rather than including `Sections.nsh`. The
   plugin DLL ships with NSIS and §11's `plugin` already knows how to call one. **A
   program that uses a custom page includes nothing, so there is no include order to get
   wrong** — the standing requirement, made structural rather than documented.

6. **Geometry is explicit, and there is no layout engine.** `x`, `y`, `width`, `height`
   are `integer|string`: an integer is dialog units, a string passes through so `"100%"`
   works. A control with no geometry is an error rather than a stacked default. Auto-flow
   is a design of its own and a cheap one to add later; adding it now would mean every
   control's position depends on the declaration order of the ones above it, which is the
   coupling ruling 3 just spent a whole arc removing from sections.

7. **A claimed control's handle is a `Var`, not a register.** A plugin call clobbers
   everything (§15.11), and a handle lives from the creator through `leave` — across two
   NSIS functions. So each claimed control gets one compiler-owned `Var`, named from the
   `local` the way `${SEC_core}` is. The register allocator never sees it.

8. **Fields, not `SendMessage` — and `SendMessage` stays anyway.** `serial.value` rather
   than `sendMessage(h, 0x000C, 0, buf)`, on ruling 6 of the sections plan: handing a user
   the message number is handing them the `System::Call` documentation. But `SendMessage`
   is *also* exposed as itself, because `findWindow` returns handles to windows this
   program did not create and there is no field surface for those.

## The fourteen rows, and the three groups they split across

| group | rows | reached by |
| --- | --- | --- |
| not blocked (ruling 1) | `AutoCloseWindow`, `BringToFront`, `FindWindow`, `IsWindow`, `SetBrandingImage` | an attribute and four calls |
| the page construct | `Page`, `UninstPage` | `page.custom { … }` |
| the control surface | `GetDlgItem`, `SendMessage`, `EnableWindow`, `ShowWindow`, `SetCtlColors`, `CreateFont`, `LoadAndSetImage` | a field on a control handle, or a call taking one |

`Page` and `UninstPage` become `Class::Language`, not `Exposed`: the construct is the
Installua spelling, the way `Section` is `section`. Everything else is `Exposed`, and the
field rows carry `Kind::Bound` — the kind batch 26 added for exactly this, a position the
compiler resolves from a name.

## The surface

```lua
attributes { name = "Example", outFile = "setup.exe" }

-- A control is declared like a section and listed like one. The `local` decides
-- nothing about where it sits; `controls` does.
local serial  = text     { "", y = 20, height = 12 }
local agree   = checkbox { "I have read the terms", y = 40, height = 12 }
local proceed = button   { "Check", x = 0, y = 60, width = 60, height = 14 }

installer {
  page.welcome {},

  page.custom { "Registration",
    headerText    = "Serial number",
    headerSubText = "Enter the key from your invoice.",

    controls = {
      label { "Serial:", y = 0, height = 12 },
      serial,
      agree,
      proceed,
    },

    show = function()
      -- A stock control, addressed the only way Windows offers: by id. The ids
      -- are Microsoft's and MUI2's, not ours, so they are not renamed.
      local cancel = getDlgItem(HWNDPARENT, 2)
      cancel.enabled = false
    end,

    leave = function()
      if serial.value == "" then
        messageBox { text = "Enter your serial number." }
        abort()
      end
    end,
  },

  page.instFiles {},
}
```

`proceed.onClick = …` is written as an option on the declaration rather than assigned at
install time, because the address of a function is a compile-time fact:

```lua
local proceed = button { "Check", x = 0, y = 60, width = 60, height = 14,
  onClick = function()
    agree.enabled = serial.value ~= ""
  end,
}
```

## Control kinds

Each is a declaration in §15.23's table form: array part is the positional argument (the
text, where the control has one), hash part is the options.

| kind | nsDialogs | positional |
| --- | --- | --- |
| `label` | `${NSD_CreateLabel}` | text |
| `text`, `password`, `number` | `${NSD_CreateText}` and friends | initial text |
| `button` | `${NSD_CreateButton}` | caption |
| `checkbox`, `radioButton` | `${NSD_CreateCheckBox}` | caption |
| `groupBox`, `hLine` | `${NSD_CreateGroupBox}`, `…HLine` | caption, or none |
| `dropList`, `listBox` | `${NSD_CreateDropList}` | none; `items = { … }` |
| `fileRequest`, `dirRequest` | `${NSD_CreateFileRequest}` | initial path |
| `bitmap`, `link` | `${NSD_CreateBitmap}`, `…Link` | none; `image = …`, or URL |

The rest of nsDialogs' list — rich edit, owner-draw, the timer — is data entry against
this shape rather than more design, and is out of scope below.

## Control fields

| field | type | NSIS |
| --- | --- | --- |
| `value` | `string` | `System::Call user32::GetWindowText` / `SendMessage WM_SETTEXT` |
| `checked` | `boolean` | `SendMessage BM_GETCHECK` / `BM_SETCHECK` |
| `enabled` | `boolean` | `EnableWindow`, **write-only** |
| `visible` | `boolean` | `ShowWindow`, **write-only** |
| `colors` | table | `SetCtlColors`, **write-only** |
| `font` | table | `CreateFont` + `WM_SETFONT`, **write-only** |
| `image` | `string` | `LoadAndSetImage`, **write-only** |

*Amended in step 4.* `value`'s read was written here as `WM_GETTEXT` and is not one: NSIS's
`SendMessage` cannot be handed a buffer. And `textColor`/`backColor` were written as two
fields and are one, because `SetCtlColors` is one instruction that writes both.

Five of the seven have no read instruction in NSIS, so reading them is an error naming the
setter rather than a `SendMessage WM_ENABLE` guess. `checked` on a `label` is an error the
same way `expanded` on a section is: the handle knows which kind it is — unless it came
from `getDlgItem`, in which case nothing does, and the two kind-dependent fields say so.

## Emission

```nsis
Page custom __GENERATED_page_2 __GENERATED_page_2_leave "Registration"

Var __GENERATED_ctl_serial

Function __GENERATED_page_2
  !insertmacro MUI_HEADER_TEXT "Serial number" "Enter the key from your invoice."
  nsDialogs::Create 1018
  Pop $0
  StrCmp $0 error 0 +2
    Abort
  nsDialogs::CreateControl STATIC ${__NSD_Label_STYLE} ${__NSD_Label_EXSTYLE} 0 0u 100% 12u "Serial:"
  Pop $1
  nsDialogs::CreateControl EDIT ${__NSD_Text_STYLE} ${__NSD_Text_EXSTYLE} 0 20u 100% 12u ""
  Pop $__GENERATED_ctl_serial
  …
  nsDialogs::Show
FunctionEnd
```

Three things worth naming, because each is a place the generated form is not the obvious
one:

- **The header strip is a macro call, not a `!define`.** Every other page carries
  `headerText` as `MUI_PAGE_HEADER_TEXT`; a custom page is not a MUI2 page, so MUI2 never
  reads that define for it. `MUI_HEADER_TEXT` inside the creator is how MUI2's own
  documentation writes it, and it is the one field that changes shape between
  `page.custom` and its seven siblings.
- **Only claimed controls get a `Var`.** A `label` listed inline and bound to no `local`
  pops into a scratch register and is never named again, so a ten-control page costs the
  handful of `Var`s the program actually addresses. Same rule as `index_name: Option<…>`
  on a section.
- **`onClick` takes the function's address, and the user never writes that.**
  `GetFunctionAddress` stays a `todo` — its reason is §3's *"`Call`-by-address has no Lua
  shape"* and that is still true of the *surface* — but the compiler emits it, in the same
  spirit as `Kind::Bound`: the address exists in exactly one place.

## Steps

0. **Land the four that are not blocked.** *(done)* `AutoCloseWindow` became an attribute
   beside `brandingImage`; `bringToFront()`, `findWindow(…)` and `isWindow(…)` became
   exposed rows. No new machinery at all — the generic call, predicate and attribute
   lowerings took them as written, which is what "not blocked" turned out to mean.
1. **`Offer` gains the single-valued flag, and `SetBrandingImage` with it.** *(done)*
   The fifth row of ruling 1 could not land in step 0 after all: every flag on an
   `Exposed` row has to be reachable (`overlay.rs:145`), and `/IMGID=` had no spelling.
   `Offer::Valued { name, ty, kind }` is that spelling, and the `=` is the whole reason it
   is not `Offer::List` with a count of one — the snapshot keeps neither the `=` of
   `/IMGID=id` nor the space of `/x filespec`, recording `value: true` for both, so which
   shape a flag has is judgement and lives in the overlay. It is also the first `Offer` to
   carry a `Ty`: a flag value is checked against `str` everywhere else, and an id is a
   number. Emission glues a literal into one `Arg::Raw` token and keeps the pieces of
   anything holding a register, because `Arg::Raw` reads nothing and hiding a register
   from liveness is not a formatting decision. `SendMessage`'s `/TIMEOUT=` and two other
   flags now have a spelling waiting for them.
2. **`page.custom` as an eighth page**, with the generated creator. *(done)* `Page custom`
   emission, the `nsDialogs::Create`/`Show` protocol, `MUI_HEADER_TEXT` in place of the
   two header defines, and `HWNDPARENT` added as a read-only constant so `getDlgItem` has
   a dialog to name. `Page` and `UninstPage` moved `todo` → `Class::Language`.

   Three things the ruling did not say, decided here:

   - **`Page custom` has two function slots and the page has three hooks**, so `pre` and
     `show` are inlined into the creator on either side of the dialog — `pre` before
     `nsDialogs::Create`, early enough that `abort()` skips the page, and `show` after the
     controls and before `nsDialogs::Show`. Only `leave` is a name on the line. `Call`ing
     them instead was the alternative and was dropped: whether `Abort` propagates out of a
     `Call` is not a question generated code should depend on.
   - **The caption is the array part**, matching §15.23: `page.custom { "Registration", … }`.
     The other seven reject a positional entry outright, because MUI2 names them.
   - **`body()` split into `body_with(span, half, build)`**, since a generated body is
     compiler instructions with user blocks between them and there is no single `Block` to
     hand the old signature.
3. **Control declarations, deferred and claimed.** *(done)* The pass from batch 22–26's
   step 3, generalised from *section claimed by block* to *declaration claimed by
   construct*: `DeferredKind::Control` carries the row it matched, `Site` says what a bare
   name is being listed by, and the four claim rules are the same four in the same words —
   only the construct they name changes. `page.custom` gained `controls`, and the creator
   writes one `nsDialogs::CreateControl` per entry, with the style words folded to numbers
   so that nothing is `!include`d (ruling 5).

   Four things the rulings did not say, decided here:

   - **`Site::accepts` is a new check rather than new wording.** The other three rules
     generalised as text; this one is the question the section plan never had to ask,
     because a section had only one kind of place to be listed.
   - **`y` and `height` are required and `x` and `width` default to constants** — the left
     edge and the full width. Ruling 6 rejects auto-flow, and the only default a `y` could
     have is "under the last control", which is the coupling sections just lost.
   - **An integer is dialog units, emitted with the `u` that says so**, because nsDialogs
     reads a bare number as pixels. A string passes through for `"100%"` and `"-13u"`, and
     anything that is not a measurement is an error rather than the 0 nsDialogs reads it
     as.
   - **Thirteen kinds, not fifteen.** `bitmap` and `link` need a field and an event to
     mean anything, so they land in steps 4 and 5 rather than as controls that draw
     nothing. `default = "…"` from the surface sketch above is not a thing: the array part
     is the text, per §15.23 and the kind table.

   One thing is deliberately still wrong until step 4: a control handle read anywhere at
   all is `NotYetImplemented`, which shadows claim rule 4's cross-half message. The rule is
   checked for sections and its control half returns with the fields.
4. **Control fields.** *(done)* `lower/handle.rs` grew the control side beside the section
   side: `addressed()` decides once what the base of `a.b` is, and the two halves never ask
   each other. The read-modify-write shape is gone — a control field is one instruction —
   and the write-only *five* get a diagnostic naming their setter.

   Four things the table above got wrong or left open, decided here:

   - **`value` is not `SendMessage WM_GETTEXT`.** NSIS's `SendMessage` has nowhere to put
     a string it is handed back, so the read is `System::Call user32::GetWindowText`,
     which is what `nsDialogs.nsh` does. Ruling 5 holds: `System` is a plugin and
     `${NSIS_MAX_STRLEN}` is makensis' own define, so the output still includes nothing.
   - **`textColor` and `backColor` became one `colors` field.** `SetCtlColors` writes both
     in one instruction, so a pair of fields would mean a write to either replacing the
     other with a colour this compiler invented. Both halves required; `back =
     "transparent"` leaves the background unpainted.
   - **A window from `getDlgItem` has the fields every window has.** `checked` and `image`
     need a declaration, because the class behind an `HWND` is not readable from NSIS;
     the other five are true of any window. §15.14's single `handle` type means this
     accepts a `fileOpen` handle too, which a fifth type — not a check — would fix.
   - **`image` is an option as well as a field, and `bitmap` landed with it.** A `bitmap`
     that draws nothing until a callback runs is a declaration that declares an empty
     rectangle. `Created`'s `items` generalised into `post`: the instructions a creator
     runs against a control it has just made.

   `GetDlgItem` moved `todo` → exposed here rather than in step 6, because it is the one
   window row a program calls rather than reaches. Claim rule 4's control half is no longer
   shadowed.
5. **Events.** *(done)* `onClick` and `onChange` as declaration options, lowering to
   `GetFunctionAddress` + `nsDialogs::OnClick`/`OnChange` beside the control's
   `CreateControl`, with the callback emitted as an ordinary generated function named
   `mui.control.<local>.<event>`. `link` landed here, which is what step 3 deferred it for.

   Three things decided here:

   - **The callback pops, and the program never sees what it popped.** nsDialogs pushes the
     control's `HWND` before calling, and a callback that leaves it there corrupts the
     stack for everything after — a wrong string in an unrelated instruction rather than a
     crash. The compiler writes the `Pop`; the program has no use for the value, because it
     wrote the callback *on* the control it belongs to.
   - **`url` is an `onClick` the compiler writes.** A `link` is an owner-drawn button that
     looks like one and opens nothing, so `link { "Terms", url = "…" }` generates a
     callback whose body is `ExecShell "open"` — a shortcut's behaviour, so the browser is
     the user's. Writing both `url` and `onClick` is an error: one control has one click.
   - **Which kinds have which event is a table, from nsDialogs' own line.** *"There is
     nothing to notify about label changes, only clicks"*: `onClick` is on the six that are
     clicked and `onChange` on the seven a user edits, and `hLine` and `groupBox` have
     neither.

   `GetFunctionAddress` stays `todo` while being emitted, for the reason `SendMessage` did
   in step 3: §3's *"`Call`-by-address has no Lua shape"* is a statement about the surface,
   and the address of a generated function exists in exactly one place.
6. **Table rows, stubs, golden, docs.** *(done)* `installua.Control` and
   `tests/golden/dialog.lua` landed early, in step 3, because a construct with no golden
   is a construct nothing assembles, and the fields on `installua.Control` landed with
   them in step 4. What was left is the rows and the two documents, and the rows moved
   somewhere the plan did not name:

   - **Seven rows became `lowering-target`, not `exposed`.** `SendMessage`,
     `EnableWindow`, `ShowWindow`, `SetCtlColors`, `CreateFont` and `LoadAndSetImage`,
     plus `GetFunctionAddress` from step 5. Every one of them needs a handle, and every
     handle there is belongs to a control this compiler drew — so a call would be a
     second spelling of a field, without the kind check or the spill. `lowering-target`
     is the bucket that says *the compiler writes it and here is what you write*, so the
     move buys a §5 diagnostic: `sendMessage(…)` now answers with `agree.checked = true`
     rather than with the generic unknown-name error.
   - **`GetLabelAddress` keeps its `todo`.** It shared `GetFunctionAddress`'s reason and
     does not share its resolution: §8 owns labels, and there is nothing to take the
     address of.

   §15.7 gained its *"Amended in Phase 6"* block — the eighth page is a classic
   `Page custom` line between MUI2's `!insertmacro`s, and it includes nothing — and
   §15.32 is written as a construct rather than an amendment: the declaration, the `Var`
   and the `Pop`, the seven fields as one instruction each, the events, `getDlgItem`, and
   the census consequence above.

## Census

| move | rows |
| ---- | ---- |
| `todo` → exposed | 5 |
| `todo` → lowering-target | 7 |
| `todo` → language | 2 |
| `todo` → attribute | 1 |

`todo` 35 → **20**; `exposed` 96 → **101**; `lowering-target` 18 → **25**.

The plan predicted eleven rows to `exposed` and got five, because the twelve it counted as
callable turned out to be seven the compiler writes: see step 6.

Steps 0 and 1 are five of those: `todo` 35 → **30**, `exposed` 96 → **100**,
`attribute` 63 → **64**. Step 2 is the two `language` rows: `todo` 30 → **28**,
`language` 12 → **14**. Step 3 moves none: the twelve remaining rows are reached through
control *fields*, and what it landed is the construct they hang off. Step 4 moves one —
`GetDlgItem`, the only window row a program *calls* — leaving the six it reaches through
fields, plus `SendMessage`, for step 6: `todo` 28 → **27**, `exposed` 100 → **101**. Step 5
moves none, and adds one to step 6's list: `GetFunctionAddress`, which the events emit and
the surface still has no spelling for. Step 6 moves those seven to `lowering-target`:
`todo` 27 → **20**, `lowering-target` 18 → **25**.

That leaves twenty rows: fourteen in six groups — §15.26's three locale-table rows, the
three `Find*`, the two `Log*`, the two remaining address rows, the two flattened
alternations and the two `*SubCaption` — and six one-offs.

## Out of scope

- **A layout engine.** Ruling 6. Auto-flow, tab order and resize behaviour are one design
  and none of them is a row.
- **The rest of nsDialogs' control list.** Rich edit, owner-draw, `CreateTimer`,
  `SelectFolderDialog`. Each is a table entry against the shape step 3 lands, and none
  changes it.
- **`${NSD_*}` for `raw` users.** A program that drops into `raw` and writes
  `${NSD_CreateLabel}` gets no header included on its behalf, because ruling 5 is that
  nothing is included. `raw` already carries this cost everywhere else (§13).
- **Custom page reordering at install time.** MUI2 pages skip themselves by `Abort` in
  `pre`, and so does this one. Nothing new is needed and nothing new is offered.
- **`SetBrandingImage /IMGID`.** Step 0 lands the form that targets the control
  `brandingImage` creates. Targeting an arbitrary control by dialog id is `getDlgItem`
  plus `image`, which step 4 gives for free — so the switch would be a second spelling of
  a thing already spelled.
