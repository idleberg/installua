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

### 5.1 `${Using:StrFunc}` is emitted above `SetCompressor`

Any program that sets a compressor and uses `string.lower`, `string.upper` or
`string.find` fails to build. Installua reports the failure as its own bug.

```lua
attributes { name = "b", outFile = "b.exe", compressor = { "lzma", solid = true } }
installer { page.instFiles {}, section("s", function() detailPrint(string.lower(INSTDIR)) end) }
```

**Now:** `makensis` stops with `can't change compressor after data already got
compressed or header already changed!`, followed by `this is a compiler bug`.
`${Using:StrFunc} StrCase` defines a function, which changes the header, and
it is emitted before `SetCompressor`.

**Cause:** the spine in `src/ir.rs` (the module doc, slots 4 and 5) puts header
init lines before attributes. `ORDERED` in `src/lower/mod.rs:139` only orders
attributes among themselves.

**Fix:** emit the `${Using:StrFunc}` lines (`src/lower/mod.rs:1558`) after the
attributes, or at least after the `ORDERED` ones (`cpu`, `compressor`,
`brandingImage`). Then update the slot list in `src/ir.rs` and the reason
written beside it.

**Verify:**
- The repro builds.
- Add a golden that sets `compressor` and calls `string.lower`, and build it at Tier 3.
- Search the existing goldens for `Using:StrFunc` and check they still pass `makensis -WX`.

### 5.2 A `param` named like an NSIS constant silently becomes the constant

```lua
local RESOURCES <const> = param("RESOURCES", "assets")
attributes { name = "b", outFile = "b.exe" }
installer { page.instFiles {}, section("s", function() detailPrint(RESOURCES) end) }
```

**Now:**
- It builds with no diagnostic, emitting both `!define RESOURCES "assets"` and
  `DetailPrint $RESOURCES`. The installer prints Windows' resources folder, not
  the parameter.
- `-D RESOURCES=…` is accepted and has no effect.
- In PimpBot, `file(RESOURCES .. "/*")` packed from `$RESOURCES\*`. The same
  applies to `FONTS`, `TEMP`, `DESKTOP` and every other name in `CONSTANTS`
  (`src/builtins.rs`).

**Expected:** the doc on the message constants in `src/builtins.rs` already
states the rule: "a `local` of the same name shadows it as it shadows those".
A `local … <const>`, `param` or not, should shadow the built-in. Every read of
`RESOURCES` then emits `${RESOURCES}`.

**Where:** name resolution checks the built-ins before the top-level `<const>`s.
Find the lookup that resolves a bare name to `constants()` and give the
program's own declarations precedence. If shadowing a built-in is judged too
risky, the alternative is a hard error at the declaration that names the
built-in. It must not stay silent.

**Verify:**
- The repro emits `DetailPrint "${RESOURCES}"`.
- A second case with `-D RESOURCES=x` emits `x`.
- Add both to `tests/params.rs`.
- Check that a plain `RESOURCES` read with no local still emits `$RESOURCES`.

### 5.3 Functions no one calls fail the build

A `func` in an `include`d library that this program never calls is still
emitted. `makensis` answers with `warning 6010: install function "helper" not
referenced`, which fails under `-WX`. This makes a shared library of `func`s,
the reason `include` exists, unusable unless every program calls every function.

```lua
attributes { name = "b", outFile = "b.exe" }
func("helper", function() detailPrint("never called") end)
installer { page.instFiles {}, section("s", function() end) }
```

**Now:** `error[makensis]: warning 6010: install function "helper" not
referenced - zeroing code (40-42) out`.

**Fix:** drop functions that nothing reaches. `src/callgraph.rs` already builds
the graph. The roots are:
- the sections
- the callbacks (`onInit` and the others)
- page and control callbacks (`pre`, `show`, `leave`, `onClick`, `onChange`)
- walker bodies
- `page.finish { run = { call = … } }`

One case needs a decision: a `func` named only inside `raw` (`Call helper`,
`GetFunctionAddress`). Either scan `raw` text for the name and keep the
function, or document that such a function has to be reached from Lua too.
Scanning is the kinder choice.

**Verify:**
- The repro builds and its `.nsi` has no `Function helper`.
- A golden where `raw [[ Call helper ]]` is the only caller keeps the function.
- `tests/callgraph`-style unit coverage for a function reached only through
  another unreachable function (both dropped) and one reached only from a page
  `leave` (kept).

### 5.4 Parameters of an uncalled `func` are typed `unknown`

This has the same root as 5.3. Parameter types come only from call sites, so a
function with none has `unknown` parameters, and the body then fails to type-check.

```lua
attributes { name = "b", outFile = "b.exe" }
func("newer", function(a, b) return a < b end)
installer { page.instFiles {}, section("s", function() end) }
```

**Now:** `error[type-mismatch]: this compares a unknown with a unknown`.

**Fix:** if 5.3 drops unreachable functions before they are type-checked, this
goes away with it. If functions are dropped only at emit time, skip the type
diagnostics for bodies the call graph cannot reach, since nothing will run them.

**Verify:**
- The repro builds.
- `tests/diagnostics.rs` still reports the mismatch when one caller passes a
  string and the other an int.

### 5.5 `execShell` refuses the `runas` verb

```lua
attributes { name = "b", outFile = "b.exe" }
installer { page.instFiles {}, section("s", function() execShell("runas", EXEPATH) end) }
```

**Now:** `error[bad-field-value]: runas is not a verb`, with the note `the
values are open, print`.

**Cause:** the enum comes from `-CMDHELP`
(`tables/cmdhelp-3.12.txt:62`, `verb=(open|print)`), which understates the
real set. `ExecShell` passes the verb straight to `ShellExecuteEx`, so any
registered verb works: `runas` (elevation, the common use), `edit`,
`explore`, `find`, `openas`, `properties`, `printto`. The empty string means
the default verb.

**Fix:** in the `ExecShell` and `ExecShellWait` rows in
`src/table/overlay.rs:872`, change `verb` from `Kind::Enum` to `Kind::Value`,
or widen the enum to the documented Windows verbs. Put a comment beside it
saying why this row departs from the `-CMDHELP` snapshot, the way the
`GetDLLVersionLocal` row explains its own departure. Update the `stubs.rs`
enum (`src/stubs.rs:1458`) and the docs entry in
`reference/commands/processes-and-the-shell.md` to match.

**Verify:**
- The repro emits `ExecShell "runas" "$EXEPATH"` and builds.
- A golden with `execShell("", url)` builds too.
- `tests/census.rs` and `tests/overlay.rs` still pass. Expect the census to need
  a note for the departure.

### 5.6 `sendMessage` cannot broadcast

```lua
attributes { name = "b", outFile = "b.exe" }
installer { page.instFiles {}, section("s", function()
	sendMessage(HWND_BROADCAST, WM_FONTCHANGE, 0, 0, { timeout = 5000 })
end) }
```

**Now:** `error[type-mismatch]: sendMessage wants a handle, and this is a int`.
`HWND_BROADCAST` comes from `tables/winmessages-3.12.txt` like every message
name, so `messages()` in `src/builtins.rs` types it `nonneg`.

**Fix:** type the `HWND_*` pseudo-handles (`HWND_BROADCAST`, `HWND_TOPMOST`,
`HWND_BOTTOM` and any others in the table) as `Ty::Handle` when parsing
`MESSAGES`. The alternative is to let the `sendMessage` hwnd position accept an
int. Typing them as handles is narrower and keeps `sendMessage(5, …)` an error.

**Verify:**
- The repro emits `SendMessage ${HWND_BROADCAST} ${WM_FONTCHANGE} 0 0 /TIMEOUT=5000` and builds.
- `tests/handles.rs` keeps refusing a plain integer.

### 5.7 An entry cannot be left out of a block's list at build time

Everything optional in an installer is an entry in `installer {}`'s list: a
license page only when there is a license, an APE section only when that APE
ships. There is one `installer {}` (`duplicate-block`), a list cannot contain
an `if`, and a name declared only in the branch not taken does not exist:

```lua
local LICENSE <const> = param("LICENSE", "")
attributes { name = "b", outFile = "b.exe" }
if LICENSE ~= "" then
	local licensePage = page.license { file = LICENSE }
end
installer { licensePage, page.instFiles {}, section("s", function() end) }
```

**Now:** `error[unknown-field]: licensePage is not a section or a group`.
PimpBot declares a stand-in in the `else` branch:
- a hidden empty `section { "", … }` for each of 21 APEs, a fonts section and a
  settings section
- a `page.custom { pre = abort }` for the license page

Those stand-ins still reach the `.nsi`.

**Proposed fix:** in a block's list, a bare name that is declared *only* in
branches that were not taken is dropped, not reported. A name declared nowhere
is still `undefined-name`, so misspellings are still caught. The resolver
already knows the untaken branches (`Resolved`, "with every build-time `if`
replaced by the branch it took"). Keep the list of names untaken branches
declared, and consult it where the `Site::Block` diagnostic is raised
(`src/lower/mod.rs:491`). The same rule should apply inside `group(…)`'s list
and a page's `controls`.

**Verify:**
- The repro builds with and without `-D LICENSE=…`, and without it the `.nsi`
  has no license page.
- A golden where a `group` lists one kept and one dropped section.
- `tests/branches.rs` keeps `undefined-name` for a misspelt entry.
- Update the "Build-time if" docs in `reference/commands/program-structure.md`.

### 5.8 `c and x or y` does not fold to a value

```lua
local ON <const> = param("ON", true)
attributes { name = "b", outFile = "b.exe" }
installer { page.instFiles {}, section { ON and "Shown" or "", body = function() end } }
```

**Now:** `error[bad-field-value]: section wants a compile-time value`. `fold`
in `src/resolve.rs:1310` folds `and` and `or` only when both sides are `Bool`,
so Lua's usual `cond and a or b` choice never folds.

**Fix:** add arms that follow Lua's rules when the left side is a folded
`Bool`: `true and x` gives `x`, `false and x` gives `false`, `false or y`
gives `y`, and `v or y` gives `v` when `v` is anything but `false`. Only
build-time folding changes. The runtime rule that a condition must be a `bool`
stays.

**Verify:**
- The repro emits `Section "Shown"`, and `-D ON=false` emits `Section ""`.
- Unit tests in `resolve.rs` for all four arms, plus one showing that
  `cond and false or y` gives `y` (Lua's own trap, documented, not fixed).

### 5.9 `System.call` needs the whole signature at build time

```lua
local system = plugin "System"
attributes { name = "b", outFile = "b.exe" }
installer { page.instFiles {}, section("s", function()
	local id = "26"
	local path = system.call("shell32::SHGetSpecialFolderPath(p 0, t .s, i " .. id .. ", i 0)")
	detailPrint(path)
end) }
```

**Now:** `error[bad-field-value]: System.call needs a build-time signature`
(`src/lower/expr.rs:2091`). The output count is only needed from the parts of
the signature that are known at build time, but the whole string is refused as
soon as one argument is a runtime value. Passing a runtime value is the normal
use of `System::Call`. PimpBot falls back to `raw` and a global.

**Fix:** when the signature is a concatenation, count `.s` in its literal
pieces and accept the call if every runtime piece sits in an argument's value
position. A simpler alternative is an explicit count, `system.call(sig, {
outputs = 1 })`, required whenever the signature does not fold. The explicit
count is easier to get right and easier to document.

**Verify:**
- The repro emits `System::Call 'shell32::…(p 0, t .s, i $0, i 0)'` and a
  single `Pop`.
- A wholly runtime signature without `outputs` still gets today's error, with
  a note pointing at `outputs`.

### 5.10 `glob`: no recursion, and its error names the wrong path

```lua
attributes { name = "b", outFile = "b.exe" }
installer { page.instFiles {}, section("s", function()
	for p in glob("assets/**/*.txt") do file(p) end
end) }
```

**Now:** `error[bad-field-value]: glob cannot read "": No such file or
directory`. There are two problems:
- `src/lower/mod.rs:7296` prints `base.display()`, the source's directory,
  which is empty when the source sits in the working directory. It should
  print `base.join(directory)`, the folder it actually tried.
- `**` is taken as a literal folder name, so a glob cannot walk subfolders or
  yield folders.

PimpBot needs the folder names at build time for the settings page's
preset-folder list. The package's subfolders are only known then.

**Fix:**
- Fix the message first, which is one line.
- Then decide whether `glob` grows `**` (walk subfolders) and a way to ask for
  directories, e.g. a trailing `/` in the pattern. The doc comment on `fn glob`
  (`src/lower/mod.rs:8035`) says anything larger belongs to `BUILD.system`,
  which does not exist (see 5.11). So either grow `glob` or build that.

**Verify:**
- The repro's message names `assets/**`.
- If `**` lands: a golden globbing `assets/**/*.txt` over a two-level fixture,
  with paths returned in a stable order, and `glob("assets/*/")` yielding
  `assets/sub`.

### 5.11 The docs promise a `BUILD` namespace that does not exist

`concepts/lua-shaped-not-lua.md:90` lists `BUILD.system(…)`,
`BUILD.getDllVersion(…)` and `BUILD.echo(…)` as running at build time. Line 94
calls `BUILD.echo` the build-time `print`. Nothing implements them:

```lua
attributes { name = "b", outFile = "b.exe" }
installer { page.instFiles {}, section("s", function() BUILD.echo("hi") end) }
```

**Now:** `error[undefined-name]: BUILD is not defined`. The doc comment on
`fn glob` points at `BUILD.system` as well.

**Fix:** either implement them or take them out of the docs and the comment.

If implementing:
- `BUILD.echo` is `!echo`.
- `BUILD.system` is `!system`, with its exit code available through `!system`'s
  second argument.
- `BUILD.getDllVersion` is `!getdllversion`. It must not fail on a file without
  a version resource the way `GetDLLVersionLocal` does. Use `/noerrors` and
  return zeros, because APEs and other plugins often ship without one.

**Verify:**
- Implemented: a golden per function and a Tier-3 build.
- Removed: `tests/docs.rs` passes and nothing in `web/` mentions `BUILD.`.

### 5.12 `installua stubs` misses every source outside the project root — fixed

`src/project.rs` collects the sources: `installua.toml`'s programs plus their
`include`s, or a recursive walk without one. Tested in `tests/stubs.rs`.

### 5.13 A declaration inside a build-time `if` is invisible to lua-language-server

`program-structure.md` ("Build-time if") says a branch's contents are ordinary
top-level declarations. A `local` declared inside the branch can therefore be
used after its `end`, which is not how Lua scopes locals. LuaLS follows Lua:

```lua
local TYPE <const> = param("TYPE", "avs")
if TYPE == "avs" then
	local PLUGIN_NAME <const> = "AVS"
else
	local PLUGIN_NAME <const> = "MilkDrop"
end
attributes { name = PLUGIN_NAME, outFile = "b.exe" }
installer { page.instFiles {}, section("s", function() end) }
```

**Now:** `installua check` passes. LuaLS reports `unused-local` for both
`PLUGIN_NAME`s and `undefined-global` for the one in `attributes`. PimpBot
hits this for its plugin name and extension and for each stand-in in §5.7.
No form of the program satisfies both tools: the Lua-scoped
`local PLUGIN_NAME <const> = TYPE == "avs" and "AVS" or "MilkDrop"` is §5.8.

**Fix:** keep the compiler's semantics and make the two tools agree:
- Once §5.8 lands, make `cond and a or b` the documented way to pick a value,
  and keep the branch form for statements (`attributes`, `installer`, sections).
- Add the scoping difference to `concepts/lua-shaped-not-lua.md`, along with
  what LuaLS reports for it.
- Maybe: have `project_meta` stub each name declared inside a top-level branch
  as a global. That hides `undefined-global` but not `unused-local`, so only
  do it if the docs alone aren't enough.

**Verify:**
- After §5.8, the `and`/`or` form of the repro passes both `installua check`
  and `lua-language-server --check`.
- `tests/docs.rs` passes with the new paragraph.

### 5.14 A top-level `local` leaks out of the file that declares it

`include` splices a file into the one that names it, so a top-level `local` in
`lib/common.lua` is in scope in every file that includes it. Lua scopes a
`local` to its chunk, and so does LuaLS:

```text
lib/common.lua       local COLOR_VALID <const> = "D8EABD"
packages/a/install.lua
                     include "../../lib/common.lua"
                     installer { page.instFiles {}, section("s", function() detailPrint(COLOR_VALID) end) }
```

**Now:** `installua check` passes, and LuaLS reports `undefined-global` for
`COLOR_VALID` in `install.lua`, plus `unused-local` in `common.lua` when that
file never uses the name. The same happens to a page, section or control bound
with `local` in one file and listed in another. PimpBot has 7 such names:
`COLOR_VALID`, `COLOR_INVALID`, `HEADER_BACKGROUND`, `HEADER_TEXT`,
`apePlugins`, `avsSettingsPage` and `fileFunc`.

**Expected:** a top-level `local` is private to its file. A file shares names
the way a Lua module does, by returning a table, and the includer binds it:

```lua
-- lib/common.lua
local COLOR_VALID <const> = "D8EABD"
return { COLOR_VALID = COLOR_VALID }

-- packages/a/install.lua
local common = include "../../lib/common.lua"
detailPrint(common.COLOR_VALID)
```

The table never exists at run time. It is a build-time namespace, the same
kind of binding `local fileFunc = import "FileFunc"` already makes. Paths stay
relative to the including file. `func`s, globals and `param` names stay
program-wide: `func("name", …)` names its function in a string, and a bare
assignment is a Lua global either way.

**Where:**
- `src/frontend/include.rs`: accept `local m = include "…"` and a top-level
  `return { name = name, … }` in an included file. Rewrite the module doc,
  which rejects this shape because "encapsulation needs values"; `import`
  namespaces already show it doesn't.
- `src/resolve.rs`: key consts, namespaces and deferred declarations by
  (file, name). `Span` already carries the file. An included file's `return`
  becomes a namespace in the including file.
- `src/lower/`: every name lookup takes the file of the span it reads from.
- `!define` names: two files may now declare the same const, so the emitted
  name has to stay unique.
- Docs: `reference/commands/program-structure.md` (`!include`) and
  `concepts/lua-shaped-not-lua.md`.

**Verify:**
- The repro fails with a scope error until it uses `common.COLOR_VALID`, then
  passes `installua check` and `lua-language-server --check`.
- `tests/golden/include.lua` rewritten to the returned-table form.
- Two files declaring the same `local X <const>` both compile, and each reads
  its own value.
- Migrate PimpBot's 8 `include` lines.

### 5.15 A duplicate `local … <const>` passes `check` and fails the build — fixed

`const_twice` in `src/resolve.rs` reports the second declaration as
`duplicate-block`, naming the first, within one file or across `include`s.
Tested in `tests/include.rs`. After §5.14 it has to become per file.

## Later: random programs

Generate random well-typed programs from the grammar, compile them, and run
`makensis -WX`. A panic, a generic `not-yet-implemented` or a `makensis` error
is a finding. Worth it only once the items above are done.
