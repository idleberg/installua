# NSIS-shaped, not NSIS

For someone who has written `.nsi` files. You know what an installer needs to do; this is
where Installua puts each of those things, and where it deliberately does not let you do
what you are used to.

The other table is [Lua-shaped, not Lua](lua-shaped-not-lua.md), for people arriving from
the other side.

---

## The reversal that will bite hardest

**`==` on strings is case-sensitive.**

```lua
if channel == "beta" then end          --> StrCmpS $0 "beta" …
if string.lower(channel) == "beta" then end  --> StrCmp $0 "beta" …
```

`StrCmp` is the one your fingers know, and it is _not_ what `==` gives you. Installua's `==`
is Lua's `==`, which is case-sensitive, so it lowers to `StrCmpS`. The case-insensitive
form is `string.lower(a) == string.lower(b)` and it costs nothing — the compiler peepholes
it to a bare `StrCmp` with no `${StrCase}` and no temporary.

Getting this backwards is the one mistake in this document that produces a working
installer with wrong behaviour.

---

## Where things you know live now

| Your habit                                           | Installua                                                                                            |
| ---------------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| `Name`, `OutFile`, `SetCompressor`, … at top level   | fields of `attributes {}` — a set, reordered into a canonical order on output                        |
| `InstallDir`, `Caption`, `Icon`                      | fields of `installer {}`; the uninstaller's twins are the **same field names** in `uninstaller {}`   |
| `UninstallCaption`, `UninstallIcon`                   | `uninstallCaption` in `attributes {}`; `icon` in `uninstaller {}` — the `Uninstall` prefix is emitted, never written |
| `UninstallText`                                      | `page.confirm { topText = … }` — it is that page's text, not a block field                           |
| `ManifestDPIAwareness`, `ManifestSupportedOS`, …     | `manifest = { dpiAwareness = …, supportedOS = … }`                                                   |
| `VIProductVersion`, `VIAddVersionKey`                | `versionInfo = { product = …, keys = { … } }`                                                        |
| `Section` / `SectionEnd`                             | `section("Name", function() … end)`                                                                  |
| `SetCompressor /SOLID lzma`                          | `compressor = { "lzma", solid = true }` — an attribute's flags go in a table with it, the way a call's do |
| `Section /o`                                         | `section("Name", { optional = true }, function() … end)`                                             |
| `SectionGroup`                                       | `group("Name", { … })` — a list of sections, not a body; nesting one is not yet implemented          |
| `Function` / `FunctionEnd`                           | `func("name", function() … end)`                                                                     |
| `Function .onInit`                                   | `onInit(function() … end)` — the leading `.` is emitted, never written                               |
| `Section un.Main`, `Function un.Foo`                 | declare them inside `uninstaller {}`; `un.` has no spelling at all                                   |
| `LangString`                                         | the `languages {}` block, keyed locale-first; `un.` is applied for you where it helps                |
| `!include "WinVer.nsh"`                              | nothing — `getWinVer("MAJOR")` is a real instruction since NSIS 3, and returns a number              |
| `!include "FileFunc.nsh"`                            | `local fileFunc = import "FileFunc"`, then `fileFunc.driveSpace("C:/", "/D=F /S=M")`                 |
| `!define X 5`                                        | `local X <const> = 5`, used as `X`, emitted as `${X}`                                                |
| `!insertmacro MUI_PAGE_DIRECTORY`                    | `page.directory {}`, listed inline among `installer {}`'s entries — there is no `pages` field        |
| `!define MUI_PAGE_HEADER_TEXT` before a page macro   | a field of that page's `page.<name> { … }` table                                                     |
| `nsExec::ExecToStack`                                | `local nsExec = plugin "nsExec"`, then `local rc, out = nsExec.execToStack(cmd)`                     |
| `${If}` / `${While}` (LogicLib)                      | ordinary `if` and `while`; LogicLib is reachable through `raw` if you insist                         |
| `Push` / `Pop` / `Exch`                              | nothing — the compiler owns the stack. A plugin's output count comes from its declaration            |
| `$0`–`$R9`                                           | nothing — the compiler owns the registers, and they are not nameable                                 |
| `Goto`, labels                                       | nothing — `if`, `while`, `break`, `continue()`                                                       |

---

## `$` does not exist inside a string

This is the biggest single change and it is not negotiable, because the failure mode it
prevents is silent. An unknown `${NOPE}` or `$x` in real NSIS is _warning 6000_ and the
literal text ships into the installer.

So a literal is **data**. Every `$` in one is escaped to `$$` on the way out, and NSIS's
four sigil forms become ordinary Lua names joined with `..`:

| NSIS                                      | Installua                               | Emitted        |
| ----------------------------------------- | --------------------------------------- | -------------- |
| `$INSTDIR`, `$DESKTOP`, `$PROGRAMFILES64` | `INSTDIR`, `DESKTOP`, `PROGRAMFILES64`  | `$INSTDIR`     |
| `${MYDEF}`                                | `MYDEF`, from `local MYDEF <const> = …` | `${MYDEF}`     |
| `$(MyString)`                             | `lang.MyString`                         | `$(MyString)`  |
| `$0`–`$R9`                                | —                                       | compiler-owned |
| `$$`                                      | a plain `$` in any literal              | `$$`           |

```lua
detailPrint("Installing to " .. INSTDIR .. "/bin")
--> DetailPrint "Installing to $INSTDIR\bin"
```

It looks more verbose and costs nothing: `..` lowers to a string template, so that is one
line and no registers. What you get back is that `INSTDR` is an **error in your editor**,
where `$INSTDR` was a warning in a build log — if you read build logs.

If you type `$INSTDIR` inside a literal out of habit, the compiler says so by name and
suggests `.. INSTDIR`.

---

## Paths, and the backslash

`\` is Lua's escape character now. So:

| Write               | Get                                                       |
| ------------------- | --------------------------------------------------------- |
| `"assets/icon.ico"` | `"assets\icon.ico"` — `/` is normalised in path positions |
| `[[C:\Tools]]`      | `"C:\Tools"` — long strings process no escapes            |
| `"C:\\Tools"`       | `"C:\Tools"`                                              |
| `"C:\Tools"`        | **error** — `\T` is not an escape                         |

Forward slashes are the recommended form. The overlay marks which parameters are path
positions — filesystem paths _and_ registry subkeys — and normalises only those, so
`detailPrint("a/b")` is left alone.

`$\n`, `$\r`, `$\t` and `$\"` are written `\n`, `\r`, `\t` and `\"`.

---

## Charset, always written, always first

`Unicode` is emitted unconditionally and defaults to `true`. NSIS's behaviour without it
depends on how your `makensis` was built, which means the same script means two things on
two machines — the thing this language exists to stop.

`Target` has no Installua spelling, because `Target` and `Unicode` are last-one-wins with
each other and silently so. If you need it, `raw` gives it to you, and putting `Unicode`
first is what lets your `raw` line override it rather than the other way round.

The same rule covers pointer size and maximum string length: a property of how `makensis`
was built may **verify** an assumption your source states, and may never **decide** what
your source means.

---

## Pages are MUI2, and only MUI2

`PageEx`, `Page` and `UninstPage` have no Installua spelling. MUI2 is what gets generated.

The reason page settings group into a `page { … }` table rather than sitting loose is
correctness, not tidiness: MUI2's settings are `!define`s that apply to _the next_
`!insertmacro MUI_PAGE_*` and are then undefined. Hand-written MUI2 can attach a header
text to the wrong page and nothing will tell you. A `page { … }` table cannot express the
mistake.

```lua
page {
  "Directory",
  headerText        = "Choose a location",
  directoryVariable = INSTDIR,
  pre = function() … end,
}
```

Custom pages are `page.custom`, and the controls on one are declarations listed in
its `controls`. nsDialogs is compiled, not `!include`d: the compiler writes the
style constants `nsDialogs.nsh`'s `${NSD_Create*}` macros would have written, so a
program with a dialog has the same include list as one without. See
`tests/golden/dialog.lua`.

---

## Things NSIS lets you do that Installua will not

| NSIS                                                       | Why not                                                                  | Instead                                                                                           |
| ---------------------------------------------------------- | ------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------- |
| relative jumps, `Goto +2`                                  | correct until a later pass inserts an instruction, then silently wrong   | never emitted, never writable                                                                     |
| `IntOp` with `!`, `&&`, `\|\|`                             | they materialise a boolean; conditions fuse into jumps instead           | ordinary `and`/`or`/`not`                                                                         |
| `Int64Op`                                                  | **it does not exist in NSIS.** `Int64Cmp` and `Int64Fmt` do              | arithmetic on an `int64` is a hard error naming the reason, never a truncation                    |
| `StrCmp`, `IntCmp`, `IfErrors`, `IfFileExists` as commands | they take labels; the language owns labels                               | `==`, `<`, `errors()`, `fileExists(p)` — all ordinary expressions                                 |
| `MessageBox` with a jump table                             | it is a statement, a flag set and a branch at once                       | `local answer = messageBox { … }`; the jump table comes back for free when you compare the answer |
| a config file for `Name`, `OutFile`, …                     | anything NSIS expresses as a script attribute belongs in `attributes {}` | `installua.toml` holds lint policy and search paths, and nothing NSIS has a command for           |

---

## What is still yours

`raw [[ … ]]` emits verbatim at the position it appears. It is deliberately conspicuous,
and it costs exactly what you would expect:

- nothing is hoisted — position is load-bearing;
- **no local survives it** — raw NSIS can write any register (`System::Call '…i.r0'` does
  it from inside a string literal) and nothing in the AST records which, so values cross a
  `raw` block through a global;
- labels you declare in it join the enclosing body's namespace;
- a long string is required, not merely preferred: a `"…"` literal would be lexed as Lua
  and mangle your `\` and `$\n`.

`raw` has one other position, where it costs less: an argument of a declared plugin
method, `plugin.method(raw "/index 0 /index 1", "$Doc")`. There the text is spliced into
that call's line and everything else stays declared — one argument however many words it
holds, and the outputs still bind to `local`s.

Third-party headers and plugins are declared in `.installua/declarations/*.toml`, which the
compiler, the editor stubs and the linter all read — five lines per method, and the call
is then as ordinary as `detailPrint`. Declaring one is not ceremony:
`${StrCase} $0 "text" "L"` puts its destination _first_ and `${GetSize} "$dir" "" $0 $1 $2`
puts it _last_, so there is no convention to infer, and guessing emits NSIS that looks
right and is not. The format is in
[the reference](reference-map.md#declaring-a-third-party-plugin-or-header); what stays
`raw`'s work is `!addplugindir` and anything you have not declared yet.
