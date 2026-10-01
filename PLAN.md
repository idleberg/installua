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

### 5.1 `${Using:StrFunc}` is emitted above `SetCompressor` — fixed

The init lines now come after the attributes: slots 4 and 5 swapped in
`src/ir.rs` and `src/emit.rs`, since each `${Using:StrFunc}` writes a
`Function` and `SetCompressor` refuses to follow one. Tested by the
`compressor` golden (tiers 2 and 3); example 05 moved its two lines by hand.

### 5.2 A `param` named like an NSIS constant silently becomes the constant — fixed

A top-level `<const>` now shadows a built-in constant. The body lookup in
`src/lower/expr.rs` and the attribute lookup (`constant_arg` in
`src/lower/mod.rs`) check the program's consts before `constant_named`.
Tested by `a_parameter_shadows_the_constant_it_is_named_like` in
`tests/params.rs`, with and without `-D`; `TEMP` undeclared still reads `$TEMP`.

### 5.3 Functions no one calls fail the build — fixed

`callgraph::unreachable` walks from every body the compiler made (sections,
callbacks, page and control hooks, walker bodies) and treats any word of a
`raw` block that names a `func` as a call. The set rides the `lower` fixpoint,
so a dropped `func` is never lowered at all. Tested by
`a_func_nothing_reaches_is_left_out` in `tests/include.rs`.

### 5.4 Parameters of an uncalled `func` are typed `unknown` — fixed

Fell out of 5.3: an unreached `func` is not lowered, so its body is not
type-checked. `newer` in `a_func_nothing_reaches_is_left_out` is the repro.

### 5.5 `execShell` refuses the `runas` verb — fixed

The overlay can now mark a position `open`: its `-CMDHELP` members stay as
completions, and any value passes. `ExecShell`'s verb is the one such position.

### 5.6 `sendMessage` cannot broadcast — fixed

`messages()` in `src/builtins.rs` types `HWND_BROADCAST`, the header's only
`HWND_*`, as a handle; `sendMessage(5, …)` is still an error. Covered by
`send_message_broadcasts_through_hwnd_broadcast` in `tests/overlay.rs`.

### 5.7 An entry cannot be left out of a block's list at build time — fixed

The resolver keeps `Resolved::untaken`, the `local`s declared only in branches
not taken, and the claim pass drops a list entry naming one. A misspelt entry is
still `unknown-field`. Tests in `tests/branches.rs`, `a_list_may_name_what_was_left_out`.

### 5.8 `c and x or y` does not fold to a value — fixed

`fold` in `src/resolve.rs` now folds `and` and `or` by Lua's rule, where
`false` is the only falsy value: the left side decides, and the right is folded
only when it does not. A runtime `and`/`or` still wants `bool`s. Covered by
`and_or_picks_a_value_at_build_time` in `tests/params.rs`, including Lua's
`c and false or y` trap.

### 5.9 `System.call` needs the whole signature at build time — fixed

`signature_outputs` in `src/lower/expr.rs` counts `.s` in the folded pieces
and accepts a runtime piece only as an argument's value: after a space, before
`,` or `)`. The explicit `outputs` count was not needed. Tests in
`tests/declarations.rs`, `system_call`.

### 5.10 `glob`: no recursion, and its error names the wrong path — fixed

The error now names the folder it could not read, as written. `glob` takes
`**` for any depth of folders and a trailing `/` for folders instead of files,
with wildcards in any segment; see `fn glob` in `src/lower/mod.rs` and
`tests/glob.rs`.

### 5.11 The docs promise a `BUILD` namespace that does not exist — fixed

Implemented as `MAKENSIS.echo`, `MAKENSIS.system` and `MAKENSIS.getDllVersion`
(renamed from `BUILD`), in `src/lower/makensis.rs`. Each is its `!` line where
the call stands, and an answer is caught as an install-time value, never a
`<const>`. Two corrections to the plan: `!system` returns a wait status off
Windows (`exit 3` reads 768), so it is shifted unless `NSIS_WIN32_MAKENSIS`; and
3.12's `!getdllversion` does not fail on a file without a version resource, it
defines `""`, so there is no `/noerrors` (that flag only silences a missing
file) and `IntOp … + 0` turns `""` into 0. Tested in the `build-time` golden,
tiers 2 and 3. Ceiling: a `$` in a literal is left alone only when the literal
is the direct argument; inside a `..` it still warns, as it does for `raw`.

### 5.12 `installua stubs` misses every source outside the project root — fixed

`src/project.rs` collects the sources: `installua.toml`'s programs plus their
`include`s, or a recursive walk without one. Tested in `tests/stubs.rs`.

### 5.13 A declaration inside a build-time `if` is invisible to lua-language-server — fixed

A `local` declared in a top-level branch stays visible after `end`, and LuaLS,
scoping as Lua does, reports `undefined-global` at each later use. With §5.8,
`local X <const> = c and a or b` picks the value in a form both tools accept.
`concepts/lua-shaped-not-lua.md` documents it (a table row and the truthiness
section), and `program-structure.md`'s "Build-time if" points there. The
`project_meta` stub was not needed. Verified with `installua check` plus
`lua-language-server --check` on the and/or repro (no problems).

### 5.14 A top-level `local` leaks out of the file that declares it — fixed

A top-level `local` is private to its file; a file shares one with
`return { name = name }`, read as `m.name` after `local m = include "…"`.
`src/frontend/scope.rs` checks it and rewrites `m.name` to the bare name, so
`resolve` and `lower` are unchanged. Tested in `tests/include.rs` and the
`include` golden; PimpBot migrated, its four fixture `.nsi` byte-identical.

Two files may declare the same top-level `local`: the scope pass renames every
copy but the lowest file's (`COLOR` in file 1 → `COLOR_1`), so a program
without a collision emits what it did before. Ceiling: the suffix reaches the
`.nsi`, and a `raw` string spelling the unsuffixed name reads the other file's.

### 5.15 A duplicate `local … <const>` passes `check` and fails the build — fixed

`const_twice` in `src/resolve.rs` reports the second declaration as
`duplicate-block`, naming the first, within one file or across `include`s.
Tested in `tests/include.rs`. Per file since §5.14's rename: across files the
two are separate names.

### 5.16 A top-level `local` is visible above its declaration — fixed

Lua's rule, and Lua's answer. A top-level `local` is in scope from its
declaration down: a read above it is `undefined-name` ("`x` is read above its
declaration"), reported by the globals walk in `src/resolve.rs`, which already
bound each `local` after its initialiser. `local a, b` with no value is a
forward declaration, and a later top-level `a = <section/group/page/control>`
is its value; one nothing assigns is the old "nowhere to live" error. `func`s
stay hoisted.

The repro's fixed form (`local random, interval`, then both assigned) passes
`installua check`, `makensis -WX` and `lua-language-server --check` with no
problems; the unfixed form gets the same two positions from both tools.
Case in `tests/diagnostics.rs`. Two things read above their declaration and
moved: `tests/ports/modern-ui/startmenu.lua`'s `StartMenuFolder`, and
`tests/params.rs`'s parameter-below-its-use test, now a rejection.

PimpBot's `pages/avs-settings/page.lua` is migrated (`local settingsRandom,
settingsInterval` above both), and every `mise run build` profile passes.
`avs-full` also needed a `glob` fix found on the way: an absolute pattern lost
its leading `/` (`tests/glob.rs`, `an_absolute_pattern_starts_at_the_root`).

### 5.17 `string.find` returns 1 when the needle is absent — fixed

A miss is `0`. After `${StrLoc}`, a `StrCmp` on `""` sets the slot to `-1`
before the `+ 1` (`src/lower/expr.rs`, `"string.find"`), and the comment there
now says so. Golden `tests/golden/strings.{lua,nsi}` has a hit at 1, a later
hit and a miss, and passes Tier 3. `examples/05-strings-and-ints` gained the
same three lines. The migration table in `concepts/lua-shaped-not-lua.md` has
the row, and `strings-and-numbers.md` notes the `0`.

Found on the way: `string.sub(v, 1, dot - 1)` with `dot == 0` is
`StrCpy … -1`, which drops the last character, where Lua's `string.sub(v, 1,
-1)` is the whole string. A negative index known only at run time is not
converted.

### 5.18 A declaration's `dir` is resolved against the program, not the declaration — fixed

`Declarations::absorb` (`src/declarations.rs`) now joins a loaded `dir` onto
the folder holding its `.installua/` and makes it absolute. `addplugindir`
(`src/lower/mod.rs`) turns it back into a path from `base`, which is where
`assemble` runs `makensis`. A `dir` given to `Declarations::parse`, which has
no folder, is relative to the source and goes out as written. So the output
is the same from any working directory, and it holds no absolute path. The
comments on `PluginMethod::dir` and `addplugindir` say so.
`tests/cli.rs`, `a_workspace_plugin_dir_is_the_same_from_anywhere`, builds a
workspace declaration from the root and from the program's folder. Both get
`!addplugindir "../../plugins/x86-unicode"`, and the build passes Tier 3 with
a copy of `nsExec.dll` in that folder. `tests/plugins.rs` now expects
`vendor/plugins` instead of `/project/vendor/plugins`. `plugins-and-headers.md`
says what `dir` is relative to.

PimpBot can drop its workaround: `dir = "plugins/x86-unicode"`, built from
anywhere.

### 5.19 `installua stubs` writes a selene std that selene cannot read — fixed

`REJECTED`'s `message` and `replace` now go through `escape()`, like the
retired-instruction rows, so the apostrophe in the `pairs` message is `''`.
`every_quoted_scalar_in_the_selene_std_is_well_formed` in `tests/stubs.rs`
checks every single-quoted scalar in the generated `.yml`, with no YAML
dependency and no selene. selene 0.31 reads the std again.

### 5.20 selene 0.31 cannot parse `<const>`, whatever the std says — fixed

It can: the published binary is built for Lua 5.1, and the one
`cargo install selene --features selene-lib/lua54` builds (`mise run
install:selene`, and what the docs already say) parses `<const>`. It reports
0 parse errors on PimpBot.

Running it then showed 412 errors on PimpBot, nearly all the std's:

- a `func` is named by a string and selene follows no `include`, so every call
  to one and every global was `undefined_variable`. `selene_std` now takes the
  project's sources and lists both, from the walk `project_meta` uses
  (`project_names`);
- `string.sub`, `.find`, `.lower`, `.upper`, `.format`, `ipairs` and
  `continue` were missing, and are in `LANGUAGE`;
- `messageBox` was generated in its NSIS shape, two strings, so every
  `messageBox { … }` was a wrong arity. It is hand-shaped here as in the meta;
- `selene.toml` allows `unscoped_variables`: a global is the language's.

What is left on PimpBot is 72 errors and 67 warnings, all its own: locals
bound in a top-level `if` (`local fonts = section {…}` in each branch), which
the docs already say both linters report, unused variables, and one
`if_same_then_else`. PimpBot's `selene.toml` predates the
`unscoped_variables` line and needs it by hand, or `installua init --force`, since `init`
does not overwrite one otherwise.

### 5.21 A plugin that needs `/NOUNLOAD` cannot be declared — fixed

`nounload = true` on a `[[plugin]]` block writes `/NOUNLOAD` as the first token
after `Plugin::Method` on every call, ahead of the flags: `script.cpp` reads it
only there. `nxs::Show` declares as `params = []`, `outputs = []`, its options
as `separate` string flags, `terminator = "/end"` and `nounload = true`, and
compiles to `nxs::Show /NOUNLOAD /top "…" /sub "…" /end`.
`nounload_is_emitted_before_the_flags` in `tests/declarations.rs` checks it. The
field is documented in `plugins-and-headers.md`, `what-stays-out.md` and the
skill's `REFERENCE.md`.

PimpBot's `nxs.toml` can now declare `show` and `update` instead of the `raw`
workaround its comment describes.

### 5.22 A value proven positive by an `if` is still "not known to be non-negative" — fixed

The range check stays flow-insensitive; the adapters `CONTEXT.md` listed now
ship, so the fact can be stated. `math.abs(n)` is non-negative, `math.max(…)`
is when any argument is, `math.min(…)` when every one is — so
`sleep(math.max(seconds, 0) * 1000)` compiles. Each lowers to a compare and a
branch (`math_adapter` in `src/lower/expr.rs`); `max`/`min` keep a scratch
register so `dest` may be a later argument. The selene std lists all three.
Golden: `strings`. Docs: `reference/commands/strings-and-numbers.md` and the
stdlib table in `concepts/lua-shaped-not-lua.md`.

PimpBot's `for _ = 1, seconds do sleep(1000) end` workaround can become
`sleep(math.max(seconds, 0) * 1000)`.

### 5.23 Smaller gaps met on the way — fixed

- `string.sub` takes a negative constant index. `i = -k` is `StrCpy`'s offset
  `-k`, `j = -k` a `maxlen` of `-(k - 1)`, and `j = -1` the empty "to the end";
  `i = 0` reads as `1`, where it used to become offset `-1`, the last character.
  A negative `i` with a positive or non-constant `j` stays `not-yet-implemented`,
  since the length would be needed, and a negative `i` reaching before the start
  gives `""` where Lua clamps: both in the doc comment and on
  `strings-and-numbers.md`. `strings` has the first golden for `string.sub`, and
  every case was run under Wine.
- `flow-errors-and-messages.md` says that `os.exit()` in `onInit` exits with 2
  and that `setErrorLevel(0)` goes first when that is not an error. Checked
  under Wine.

PimpBot writes `string.sub(server, -1)`, and its `/help` in `pack` now exits
with 0.

### 5.24 `name`, `caption` and the finish page's texts refuse a run-time value — fixed

`attributes { name = packTitle }`, `caption = "… " .. packTitle` and
`page.finish { readme = { text = "Visit " .. packWebName } }` failed with
`bad-field-value: … wants a compile-time value`. NSIS stores `Name`, `Caption`
and the other `SetInnerString` lines as language strings and expands them when
they are shown, after `.onInit`, and MUI2 writes every page text into an
instruction.

`Setting::Str` has a `late` bit now, set on `name`, `caption`,
`uninstallCaption`, `brandingText`, `installButtonText`, `uninstallButtonText`,
`detailsButtonText` and `completedText`. Those fields and every MUI2 page string
go through `late_arg`, which also takes a global, or one concatenated with
strings, and writes it as `$name`. The `Var` stays below the attributes: that
is legal, because a language string is resolved when the tables are written,
and a Wine run showed `Name "$t"` with the value `.onInit` stored. The
`late-texts` golden covers it.

PimpBot's Runtime can drop its `raw.tail` `Name` line and the `WM_SETTEXT` on
the readme checkbox.

### 5.25 A project `func` is stubbed as returning nothing — fixed

```lua
func("packValue", function(section, key) return readIniStr(ini, section, key) end)
packType = string.lower(packValue("Installer", "PresetType"))
```

`project_meta` wrote `function packValue(section, key) end`, with no
`---@return`, and LuaLS took the empty body to return `nil`: about 30 of the
warnings on PimpBot, none of them PimpBot's. Every global was `---@type string`.

`lower::infer` runs the signature fixpoint on its own, and `project_types` in
`src/stubs.rs` runs it on every source as a root, with `include` followed from
memory, joining what each program observed. So a `func` in `lib/` is typed by
the packages that call it. Each `func` gets `---@param` (`any` when no program
calls it) and one `---@return` per value, and each global gets its inferred type
(`any` for a handle). `project_meta` takes the declarations now, so a plugin's
result is typed as well. Test: `the_project_meta_carries_the_inferred_types`.
`lua-language-server --check` on PimpBot shows no `assign-type-mismatch` or
`cast-local-type`.

### 5.26 Only `onInit` is in the LuaLS meta — fixed

```lua
installer { onGUIInit(function() end), onVerifyInstDir(function() end) }
```

`undefined-global` on both. The selene std listed all twelve callbacks by hand,
but `declarations()` wrote only `function onInit(body) end` into
`installua.lua`.

`lower::callback_words` is both halves of `CALLBACKS` and the `MUI_HOOKS`, each
once, and both generators loop over it: `installua.lua` writes each as
`---@param body fun()`, the selene std as one `function` argument. Test:
`every_callback_the_compiler_has_is_in_both_stubs`.

### 5.27 `sendMessage` rejects an integer `wParam`/`lParam` — fixed

```lua
sendMessage(hwnd, BM_SETCHECK, 0, 0)
```

`param-type-mismatch: Cannot assign integer to "wparam"|"STR:wParam"`.
`-CMDHELP` writes the parameter as `wparam|STR:wParam`, which the table reads
as an enum of two words, so `alias_table` generated
`installua.Wparamorstrwparam`.

The overlay marks both positions `spelled`, and the join drops a spelled
position's members, so nothing reads them as an enum. The stub types the
table's `unknown` positions `integer|string`, as `coerce` takes them. Test:
`send_message_takes_a_number_or_a_string`. PimpBot no longer calls
`sendMessage`, so LuaLS was run on the example above instead: clean.

### 5.28 Smaller stub gaps — fixed

- `dropList` and `listBox` return `installua.List`, a `Control` with
  `add fun(text: string)`; every other control still returns `Control`, so
  `label.add` stays `undefined-field`.
- `foreign_method` takes the method's flags and writes a trailing
  `options? { name?: type, … }`, every key optional as at the call site.
  Macros pass none.
- A meta-file `tonumber` does not override LuaLS's: the two definitions merge
  and the call stays `number?`. So `.luarc.json` disables `basic` as it does
  `os`, and the meta declares the three names kept from it — `tostring`,
  `tonumber` (`---@return integer`) and `ipairs`. The rest of `basic`
  (`pairs`, `error`, `type`, …) is now an unknown global, as the docs say it
  is. An existing project picks this up with `installua init --force`.

Tests: `a_list_control_is_offered_add`, `a_method_with_flags_takes_an_options_table`
and `tonumber_is_an_integer` in `tests/stubs.rs`. LuaLS on a scratch project:
`list.add`, `inetc.get(…, { silent = true })` and `math.max(tonumber(s), 0)` are
clean, `label.add` and `pairs` are flagged.

### 5.29 `glob` is case-sensitive on Windows — fixed

`matches_pattern`, `glob`'s matcher in `src/lower`, folds case when Installua is
built for Windows, as `FindFirstFile` does, so `res/*.bmp` takes `A.BMP` too.
Elsewhere it keeps case, the file system's rule there. A segment with no
wildcard is joined rather than matched and so was never affected.
`a_wildcard_ignores_case_on_windows` and `a_wildcard_keeps_case_elsewhere` in
`tests/glob.rs` check both; the first was run under Wine, cross-compiled for
`x86_64-pc-windows-gnu`.

PimpBot's pack installer still needs its second `*.BMP` glob when built on
macOS or Linux.

### 5.30 A string function on a build-time value does not fold

```lua
for path in glob("res/*") do
	if string.lower(string.sub(path, -4)) == ".bmp" then file(path) end
end
```

compiles to `StrCpy $0 "res/A.BMP" "" -4` / `StrCmp $0 ".bmp"` around each
`File`: a run-time test, so every file is embedded whatever its type, and once
per branch when the `if` sits in an `ipairs` loop. `path`, the `-4` and the
literal are all known at build time. Should: `string.sub`, `.lower`, `.upper`,
`.find` and `..` fold when every argument is constant, and a constant `if`
drops its dead branch as `c and x or y` already does (§5.8). Then a `glob` can
be filtered by extension, which is the only way to filter it now that `*.BMP`
and `*.bmp` differ (§5.29). The fix goes where §5.8 folds. Check: a golden
whose `.nsi` holds one `File` and no `StrCmp`.

### 5.31 `build` finds `makensis` only through `$MAKENSIS` or the PATH

`src/main.rs` reads `MAKENSIS`, and no page says so. A program that builds
installers on the user's machine (PimpBot's Compiler, with a portable NSIS in
its own folder) has to set the variable for its child, and Installua has no
way to do that either: `SetEnvironmentVariable` is a `raw` `System::Call`.
Should: a `--makensis <path>` flag on `build`, which wins over the variable;
both in `cli.md`. Separately, `setEnv(name, value)` beside `getEnv`
(`ReadEnvStr`), compiling to `System::Call 'kernel32::SetEnvironmentVariable(t, t)'`,
since the child-process case is common. Check: a `tests/cli.rs` case with a
fake `makensis` that records it was called; a golden for `setEnv`.

### 5.32 A relative `-D` path is relative to the source, not the shell

```console
$ installua build packages/pack/install.lua -D PRESETS=fixtures/avs -o build/pack.nsi
error[makensis]: File: "fixtures\avs\*.avs" -> no files found.
```

`assemble` runs `makensis` in the source's folder, so `fixtures/avs` means
`packages/pack/fixtures/avs`. That is consistent with every other path in a
program, and with `makensis -D`, which also changes into the script's folder;
but a value typed on the command line reads as relative to where it was
typed, and nothing says otherwise. Should: one sentence under `param` in
`program-structure.md` and on `cli.md`. Changing the behaviour would need to
know which parameters are paths, which `param` does not say.

### 5.33 A multi-value call cannot assign to existing variables

```lua
local code = nsExec.exec("a.exe")
local output = ""
code, output = nsExec.execToStack("b.exe")
```

`not-yet-implemented: an assignment with a different number of targets and
values`. `local code, output = nsExec.execToStack(…)` works, so the values are
there; only the plain assignment is missing. Should: `a, b = f()` pops into
the targets as the `local` form does, and `installua coverage` loses the
entry. Check: a golden beside the one for the `local` form.

PimpBot's Compiler declares a second local (`buildCode`).

### 5.34 A program whose pages are all `page.custom` never sets MUI2 up

```lua
installer { page.custom { "One", controls = {} }, section("-x", function() end) }
```

`makensis -WX` fails on `MUI_LANGUAGE[EX] should be inserted after the
MUI_[UN]PAGE_* macros`: `Page custom` is not a MUI2 macro, so `MUI_INSERT`
never runs, and without it the header text, header image and colours a
custom page asks for are not set up either. Should: the compiler writes
`!insertmacro MUI_INSERT` before `MUI_LANGUAGE` when no MUI2 page did, the
way `MUI_PAGE_INIT` would. Check: the repro as a Tier-3 `-WX` build, and a
golden holding the line.

Such a program also has sections no `instFiles` page runs, which is makensis's
warning 8000 and fails `-WX` as well. That one is the program's to silence
(`raw.head [[!pragma warning disable 8000]]`), but the docs for `page.custom`
could say so.

PimpBot's Compiler adds a welcome page that skips itself.

### 5.35 No Windows binary for a release that has the fixes

The one release is v0.1.0, without any of §5. A program that runs
`installua build` on the user's machine (PimpBot's Compiler) ships
`installua.exe`, and has to cross-build it from a checkout
(`cargo build --release --target x86_64-pc-windows-gnu`, which works and ran
under Wine with a portable NSIS 3.12). Should: each release attaches
`installua-x86_64-pc-windows-gnu.exe` (or a zip) with its SHA-256, so it can be
pinned the way PimpBot pins 7-Zip and curl. The release workflow is the place.

### 5.36 An optional section or page has no form that lua-language-server accepts — fixed

`local fonts` above the `if` and `fonts = section { … }` inside it now builds
either way. An assignment in a branch not taken is that name's declaration not
taken (`locals` in `src/resolve.rs`), so the name joins `Resolved::untaken`
rather than being `nowhere to live`. Install-time code addressing such a name
is `Addressed::Untaken` in `src/lower/handle.rs`: a field write lowers to
nothing, and a read is the field's empty value. The dummy `else` is
unnecessary, and the docs now show the forward form. Checks:
`a_forward_declared_section_the_build_left_out`, `…_when_the_build_keeps_it`
and `a_forward_declaration_no_branch_assigns_is_still_an_error` in
`tests/branches.rs`.

PimpBot's `install.lua` rewritten into this form, without the two dummy
`else`s, builds all three pack profiles through `makensis`. LuaLS reports none
of the eight `undefined-global`s. A ninth warning showed up behind them, at
`fonts.selected = fontsMissing()`: `installua stubs` gave `fontsMissing` no
`@return`. That had two causes. `install.lua` has required params, so
`project_types` dropped it whole on `missing-param`. And its one call sits
inside `if FONT_DIR ~= ""`, so the fixpoint pruned it. Both are fixed in
`stubs.rs`: `missing-param` no longer drops a file, and a `func` left without a
return gets one from an unpruned pass, if that pass types it fully.

## Later: random programs

Generate random well-typed programs from the grammar, compile them, and run
`makensis -WX`. A panic, a generic `not-yet-implemented` or a `makensis` error
is a finding. Worth it only once the items above are done.
