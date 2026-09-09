---
title: Not available
description: What has no Installua spelling, and what to write instead.
---

Everything with no Installua spelling, and what to write instead. Nothing here
is pending: these are decisions.

## The `!` directives

Installua has no preprocessor — the script _is_ a program — and 36 of the 37 `!`
directives have no spelling here. They are not all out for the same reason,
though, so they are listed under the reason rather than in one heap. Only the
first group is answered by "there is no preprocessor"; the rest are not
preprocessing at all.

### Replaced by the language

`!addincludedir` · `!cd` · `!define` · `!else` · `!endif` · `!if` · `!ifdef` ·
`!ifmacrodef` · `!ifmacrondef` · `!ifndef` · `!include` · `!insertmacro` ·
`!macro` · `!macroend` · `!macroundef` · `!undef`

These are not withheld, they are *unnecessary*. A top-level
`local X <const> = …` **is** a `!define` — that is what it emits — and the rest
have ordinary constructs:

| NSIS                                             | Installua                                          |
| ------------------------------------------------ | -------------------------------------------------- |
| `!define APP "Example"`                          | `local APP <const> = "Example"`                     |
| `!ifndef VERSION` / `!define` / `!endif`         | `local VERSION <const> = param("VERSION", "1.4.2")` |
| `!if ${ARCH} == "x64"` … `!else` … `!endif`      | a [top-level `if`](/reference/commands/program-structure/#build-time-if) |
| `!include "MyHelpers.nsh"` (your own file)       | `include("helpers.lua")`                            |
| `!include "FileFunc.nsh"` (an NSIS header)       | `local fileFunc = import "FileFunc"`                |
| `!macro Banner text` … `!macroend`               | `func("banner", function(text) … end)`              |
| `!insertmacro Banner "1.4.2"`                    | `banner("1.4.2")`                                   |

Two of those are worth more than a row. Conditional compilation is decided *in
the compiler* rather than emitted into the script, and `param` does the
`!ifndef` sandwich's job while additionally rejecting a `-D` nobody declared.

**`!macro` and `!insertmacro` are the ones that stay out**, and they are the
largest bucket by a distance — a scan of 984 real-world scripts finds 15094
`!insertmacro` sites. Most of that number is already answered: 77% are `MUI_*`
and `LANGFILE` names, which are [pages](/reference/modern-ui/) and page settings here,
and another slice is stdlib macros that are ordinary calls. What is left expands
to *declarations* rather than instructions — a `!define` that a later
`!insertmacro` reads, a `Var` that has to precede its use — and admitting that is
admitting a preprocessor with real ordering consequences. The fixed emission
order is what makes define-before-insert, `Var`-before-use and
section-index-before-`.onInit` stop being your problem, and text substitution is
the one feature that cannot coexist with it. Functions plus
[`import`](/reference/commands/plugins-and-headers/#import) cover the rest.

If a particular stock header keeps coming up, the answer is to
[declare it](/reference/commands/plugins-and-headers/#declaring-a-third-party-plugin-or-header) so its macros are calls
— the way `FileFunc` and `WordFunc` already are. One header at a time, on
evidence: the same scan puts `nsProcess.nsh` in 25 files, `FileAssociation.nsh`
in 22 and `EnvVarUpdate.nsh` in 20, and `nsProcess` is declared here because of
it. A `.toml` in your own project does the same thing without waiting for anyone,
and a header you only reach through `raw` needs no declaration at all.

**`!ifdef` used to ask whether a name exists has no spelling at all**, and that
is the one place this group really does lose something. Every name here is
declared: an undeclared one is an error, and a name that may or may not be
supplied is a parameter.

### Computed while building

`!appendfile` · `!appendmemfile` · `!delfile` · `!execute` · `!getdllversion` ·
`!gettlbversion` · `!makensis` · `!searchparse` · `!searchreplace` · `!system` ·
`!tempfile`

These run a program or read a file while the installer is being built, and hand
the answer back to the script as a `!define`. Installua cannot catch that
answer — a [`raw.head`](/reference/commands/plugins-and-headers/#rawhead-and-rawtail) block can run the command, but
the value it produces has no way to become a name your program reads. Giving it
one would mean running things at build time, which is what order-free resolution
costs.

So the work moves to whatever runs the build, and the value arrives as a `-D`:

```nsi
!system 'git rev-parse --short HEAD > rev.txt'
!searchparse /file rev.txt "" REV
DetailPrint "build ${REV}"
```

```lua
local REV <const> = param("REV")   -- no default: the build stops without it

installer {
	section("Core", function()
		detailPrint("build " .. REV)
	end),
}
```

```console
$ installua build install.lua -D REV=$(git rev-parse --short HEAD)
```

A [parameter with no default](/reference/commands/program-structure/#a-parameter-with-no-default-is-required) is what
makes this complete rather than hopeful: a wrapper that forgets to pass the
value fails the build instead of shipping a default nobody chose.

If you only want the side effect and never the value, `raw.head` runs it as
written:

```lua
raw.head [[ !system 'echo building' ]]
```

### Run when the build ends

`!finalize` · `!packhdr` · `!uninstfinalize`

These register a command `makensis` runs once the installer exists. No value
flows anywhere and position is the whole of it, which is what an anchor is for:

```lua
raw.tail [[ !finalize '"sign.exe" "%1"' ]]
```

There is no Installua spelling because one would add nothing — the NSIS line
already says exactly what it does, and `raw.tail` is the position it needs.

### `makensis`'s own output

`!pragma` · `!verbose`

Nothing, and deliberately. `installua build` runs `makensis -WX` with no warning
allowed and rewrites every message back onto the Lua line it came from. A script
turning verbosity down or disabling a warning would be switching off the check
that mapping exists to serve. This is a decision about who owns the `makensis`
invocation, not about staging.

### Build-time messages

`!assert` · `!echo` · `!error` · `!warning`

The common one is the guard that stops a build when a value was not supplied:

```nsi
!ifndef SIGNING_CERT
  !error "pass -DSIGNING_CERT"
!endif
```

```lua
local SIGNING_CERT <const> = param("SIGNING_CERT")
```

That refuses the build the same way and does it from the declaration, so there
is no guard to forget and nowhere to paste it wrongly.

The general form — "this combination of settings makes no sense, stop" — has no
spelling yet. It would be a top-level `error(…)` inside a build-time `if`, and
it waits for a real script that wants one.

### The one that is not out

`!addplugindir` is written by the compiler rather than by you: it has exactly one
legal position, and a `dir` on a
[`[[plugin]]` declaration](/reference/commands/plugins-and-headers/#declaring-a-third-party-plugin-or-header) is how you
ask for it.

## Written by the compiler, never by you

These are real NSIS lines in the output — you just do not spell them. Listed
here so a search for the NSIS name lands somewhere.

| NSIS                                                                         | What writes it                                        |
| ---------------------------------------------------------------------------- | ----------------------------------------------------- |
| `!define`                                                                    | `local X <const> = …`, `param`                        |
| `Goto`                                                                       | `if`, `while`, `for`, `break`, `continue()`, `return` |
| `Call`                                                                       | a call: `f(x)`                                        |
| `Push` / `Pop` / `Exch`                                                      | the calling convention                                |
| `StrCpy`                                                                     | assignment                                            |
| `StrCmp` / `StrCmpS`                                                         | `==`                                                  |
| `IntOp` / `IntPtrOp`                                                         | the arithmetic operators                              |
| `IntCmp` / `IntCmpU` / `Int64Cmp` / `Int64CmpU` / `IntPtrCmp` / `IntPtrCmpU` | comparison                                            |
| `IntFmt` / `Int64Fmt`                                                        | `string.format`                                       |
| `LangString`                                                                 | `languages { locales = { … } }`                       |
| `LicenseLangString`                                                          | `page.license { file = { … } }`                       |
| `InitPluginsDir`                                                             | a body that names `PLUGINSDIR`                        |
| `ReserveFile /plugin`                                                        | a plugin `.onInit` can reach                          |
| `!addplugindir`                                                              | a plugin declared with a `dir`                        |
| `SendMessage`                                                                | a control's `value` / `checked`                       |
| `EnableWindow` / `ShowWindow`                                                | a control's `enabled` / `visible`                     |
| `SetCtlColors` / `CreateFont`                                                | a control's `colors` / `font`                         |
| `LoadAndSetImage`                                                            | a `bitmap`'s `image`                                  |
| `GetFunctionAddress`                                                         | `onClick` / `onChange`                                |

## Plugins with no declaration

Sixteen plugins ship with NSIS. Thirteen are [declared](/reference/commands/plugins-and-headers/#plugin) or reached
through a construct; these three are not, and each one is a decision rather than
a gap. All three remain callable through [`raw`](/reference/commands/plugins-and-headers/#raw), which is where a script
that needs one goes.

| Plugin          | Why not, and what to write instead                                                                                                                                                                                                                       |
| --------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Math`          | Its script string reads and writes `$0`–`$R9` **by name**, and the compiler owns the registers — a declaration would describe one string in and nothing out while the call quietly overwrote whatever the allocator had put there. Use the arithmetic operators. |
| `BgImage`       | Every method returns a value only after `SetReturn on`, which makes the arity a **mode** rather than a signature — the one shape the format cannot carry.                                                                                                 |
| `LangDLL`       | Reached through `languages { ask = … }`, which emits `MUI_LANGDLL_DISPLAY`. A second spelling for one dialog is worse than none.                                                                                                                          |

`InstallOptions` and `nsDialogs` are not in that list because they are not
absences: the first is superseded by [`page.custom`](/reference/commands/windows-and-controls/) and the
second is what `page.custom` emits.

### Third-party plugins that stay out

The same decision, made against a scan of 984 real-world scripts. Each of these
is common enough to have been considered and turned down for a stated reason —
and all but the last are about the **declaration format** rather than about the
plugin, which is what makes the list worth keeping. All remain
callable through [`raw`](/reference/commands/plugins-and-headers/#raw), and any of them can be declared by a project
in five lines of [its own `.toml`](/reference/commands/plugins-and-headers/#declaring-a-third-party-plugin-or-header).

| Plugin                   | Scripts | Why not, and what to write instead                                                                                                                                                                                                                                                        |
| ------------------------ | ------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Registry`               | 40      | Every corpus use is `${registry::…}`, the `Registry.nsh` macro form, not a raw plugin call — and that form needs a trailing `${registry::Unload}`, which is behaviour rather than arity. `readReg`, `writeReg` and `deleteRegKey` already cover 38 of the 40.                                          |
| `SimpleSC.getErrorMessage` | 61    | Takes its argument by `Push` rather than inline: `Push $code` / `SimpleSC::GetErrorMessage` / `Pop $msg`. `params` become the arguments written *after* `Plugin::Method`, so the format has no spelling for it. Three lines of `raw`.                                                        |
| `LockedList`             | 0       | Its surface is a custom **page**, not a call. Declaring only the `Add*` setup calls would ship half a feature.                                                                                                                                                                             |
| `Crypto`                 | 0       | Fails the *common* half of the rule outright.                                                                                                                                                                                                                                             |
| `Nsis7z.extractWithCallback` | 5   | Not the register protocol the six `FileFunc`/`TextFunc` macros use — it pushes its two values on the stack. It stays out a step earlier than that: its second argument is the **address** of a function, `params` has no type for one, and `GetFunctionAddress` has no Lua spelling. See [the plugin reference](/reference/plugins/what-stays-out/#nsis7zextractwithcallback-takes-an-address-not-a-callback). |
| `nsJSON`                 | 3       | Its node path is a **variable number of positional strings** — one to four across 25 call sites — and `params` is a fixed list, so a declaration would have to pick a depth and miscount the `Pop`s at every other one. The repeated `/index` run its readme advertises turns out to appear in no script at all. See [the plugin reference](/reference/plugins/what-stays-out/#nsjsons-blocker-is-its-path-not-its-flags). |
| `Inetc.post`             | 2       | Its POST body is popped **before** the flag loop (`inetc.cpp:1369`), so it has to be written ahead of every switch — and a `params` entry is emitted after the flags. `get`, `head` and `put` ship declared; only this entry point has an argument in front. |
| `nsisFirewall`           | 1       | The only entry turned down over the plugin rather than the format: its Unicode build is a differently *named* DLL. Version 1.2 (2009, the last) added Unicode as **`nsisFirewallW`**, and NSIS resolves `nsisFirewall::…` to `nsisFirewall.dll` — so a declaration spelling the ANSI token cannot load in an Installua installer, which is always `Unicode`. Use [`SimpleFC`](/reference/plugins/third-party-plugins/#simplefc), which ships an ANSI and a Unicode build under one name and reaches `INetFwPolicy2` besides. |

## Rejected NSIS commands

| NSIS                                     | Write instead                                           |
| ---------------------------------------- | ------------------------------------------------------- |
| `CallInstDLL`                            | `plugin.method(…)`                                      |
| `ChangeUI`                               | — MUI2 owns the dialog resources                        |
| `DirShow`                                | — NSIS reports this one as not working                  |
| `FindFirst` / `FindNext` / `FindClose`   | `for path in glob("…") do`                              |
| `GetCurrentAddress` / `GetLabelAddress`  | — there are no labels to address                        |
| `LangStringUP`                           | `languages { locales = { … } }`                         |
| `LoadLanguageFile`                       | `languages { locales = { … } }`                         |
| `LogSet` / `LogText`                     | `detailPrint` — logging needs a custom `makensis` build |
| `Nop`                                    | write nothing                                           |
| `PageEx` / `PageExEnd` / `PageCallbacks` | `page.*`                                                |
| `SectionInstType`                        | a section's `installTypes`                              |
| `SetPluginUnload`                        | — NSIS retired it                                       |
| `SubSection` / `SubSectionEnd`           | `group(…)`                                              |
| `Target`                                 | `cpu` and `unicode`                                     |
| `UninstallExeName`                       | `writeUninstaller(…)`                                   |
| `UnsafeStrCpy`                           | `=`                                                     |
| `XPStyle`                                | — MUI2 emits it itself                                  |

## Lua that is not here

Installua is Lua-shaped, not Lua (see [Lua-shaped, not Lua](/concepts/lua-shaped-not-lua/)):
no floats, no closures as values, no metatables, no `pairs`, no `require` as a
runtime load, `#` rejected on strings, and truthiness only for booleans.

---
