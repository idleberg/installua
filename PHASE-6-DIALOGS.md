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
local serial  = text     { y = 20, height = 12, default = "" }
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
| `bitmap`, `link` | `${NSD_CreateBitmap}`, `…Link` | path, or URL |

The rest of nsDialogs' list — rich edit, owner-draw, the timer — is data entry against
this shape rather than more design, and is out of scope below.

## Control fields

| field | type | NSIS |
| --- | --- | --- |
| `value` | `string` | `SendMessage WM_GETTEXT` / `WM_SETTEXT` |
| `checked` | `boolean` | `SendMessage BM_GETCHECK` / `BM_SETCHECK` |
| `enabled` | `boolean` | `EnableWindow`, **write-only** |
| `visible` | `boolean` | `ShowWindow`, **write-only** |
| `textColor`, `backColor` | `string` | `SetCtlColors` |
| `font` | table | `CreateFont` + `WM_SETFONT` |
| `image` | `string` | `LoadAndSetImage` |

Two of the seven have no read instruction in NSIS, so reading them is an error naming the
asymmetry rather than a `SendMessage WM_ENABLE` guess. `checked` on a `label` is an error
the same way `expanded` on a section is: the handle knows which kind it is.

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
2. **`page.custom` as an eighth page**, with the generated creator: `Page custom`
   emission, the `nsDialogs::Create`/`Show` protocol, `MUI_HEADER_TEXT` in place of the
   two header defines, and `$HWNDPARENT` added as a read-only constant so `getDlgItem` has
   a dialog to name. `Page` and `UninstPage` move `todo` → `Class::Language`.
3. **Control declarations, deferred and claimed.** The pass from batch 22–26's step 3,
   generalised from *section claimed by block* to *declaration claimed by construct*. The
   four claim rules and their diagnostics are shared text, not a second wording.
4. **Control fields.** `lower/handle.rs` grows the control side beside the section side:
   the read-modify-write shape is gone (a control field is one `SendMessage` each), and
   the write-only pair gets its own diagnostic.
5. **Events.** `onClick` and `onChange` as declaration options, lowering to
   `GetFunctionAddress` + `nsDialogs::OnClick`, with the callback emitted as an ordinary
   generated function.
6. **Table rows, stubs, golden, docs.** Twelve rows move, `installua.Control` joins
   `installua.Section` in `stubs.rs`, `tests/golden/dialog.lua` goes through tier 3, and
   §15.7 gains an *"Amended in Phase 6"* block saying the eighth page is not MUI2's — plus
   a new §15.32 for the control model, since it is a construct rather than an amendment.

## Census

| move | rows |
| ---- | ---- |
| `todo` → exposed | 11 |
| `todo` → language | 2 |
| `todo` → attribute | 1 |

`todo` 35 → **21**; `exposed` 96 → **107**.

Steps 0 and 1 are five of those: `todo` 35 → **30**, `exposed` 96 → **100**,
`attribute` 63 → **64**.

That leaves one grouped reason in the backlog — §15.26's five locale-table rows — and
sixteen one-offs.

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
