---
title: Plugins and headers
description: "`plugin`, `import`, `raw`, and declaring a third-party one."
---

## plugin

Names a plugin DLL and returns a table whose methods are its calls. The DLL name
**is** the namespace, so nothing has to say where the file is as long as it is
in `NSISDIR/Plugins`; a vendored one adds a `dir` to its declaration. The
compiler reserves the DLL when `.onInit` can reach the call.

**Usage** `local p = plugin(name)` · `p.method(…)` → its outputs

Eleven that ship with NSIS, read from each plugin's own source rather than its
wiki page:

| Plugin      | Methods                                                                                    |
| ----------- | ------------------------------------------------------------------------------------------ |
| `nsExec`    | `.execToStack` → exit code, output                                                         |
| `UserInfo`  | `.getAccountType`                                                                          |
| `System`    | `.call` — outputs come from the signature's `.s`, not from a count                          |
| `Dialer`    | `.attemptConnect`, `.getConnectedState`, `.autodialOnline`, `.autodialUnattended`, `.autodialHangup` |
| `NSISdl`    | `.download`, `.downloadQuiet` — plain HTTP only, no HTTPS                                   |
| `VPatch`    | `.patchFile`, `.getFileCrc32`, `.getFileMd5`                                                |
| `TypeLib`   | `.register`, `.unregister`, `.getLibVersion` → **minor, major**                             |
| `Banner`    | `.show`, `.getWindow`, `.destroy`                                                           |
| `Splash`    | `.show` → `1` closed early, `0` timed out, `-1` error                                       |
| `AdvSplash` | `.show` — Splash plus fades and a transparent colour                                        |
| `StartMenu` | `.select`, `.init`, `.show` — the folder follows `"success"`, and only then                 |

And eight third-party plugins, seven of them on the evidence of a scan of 984
real-world scripts. These are **declarations, not bundled DLLs** — the plugin is
still yours to install, and the file here only supplies the count:

| Plugin           | Scripts | Methods                                                                                    |
| ---------------- | ------- | ------------------------------------------------------------------------------------------ |
| `EnVar`          | 95      | `.setHKCU`, `.setHKLM`, `.check`, `.addValue`, `.addValueEx`, `.setValue`, `.setValueEx`, `.deleteValue`, `.delete`, `.update` |
| `SimpleSC`       | 61      | `.installService`, `.removeService`, `.startService`, `.stopService`, `.existsService`, `.serviceIsRunning`, `.getServiceStatus`, `.setServiceDescription`, `.setServiceStartType`, `.setServiceFailure` |
| `nsProcess`      | 25      | `.findProcess`, `.killProcess`, `.closeProcess`                                            |
| `Inetc`          | 46      | `.get`, `.head`, `.put` — eighteen flags and a mandatory `/END`; `.post` stays out          |
| `AccessControl`  | 111     | all 25 — every mutator and reader on files and registry keys, plus the three SID helpers    |
| `Nsis7z`         | 5       | `.extract`, `.extractWithDetails` — neither pushes anything at all                          |
| `SimpleFC`       | 24      | all 33 — ports, applications, ICMP types and advanced rules; `0` is success and `1` is failure |
| `NScurl`         | —       | `.http`, `.wait`, `.query`, `.cancel`, `.md5`, `.sha1`, `.sha256`, `.escape`, `.unescape` — libcurl, and the one method whose flags trail |

`NScurl` has no script count because it postdates the corpus. It ships on the
same half of the rule `Nsis7z` does: eleven exports and no two push the same
number of values, two of them pushing none at all. Its counts were read out of
`main.c`.

`Nsis7z`'s count of 5 is not a typo. It ships on **arity** rather than
popularity — two methods that push *nothing at all*, which is the one count no
reader guesses — because the rule that governs this list puts the undiscoverable
half first. Popularity only breaks the tie.

`AccessControl` is the other end of the same rule. Twenty-three of its
twenty-five methods push a number of values that depends on the outcome, which
is a fact no reading of its documentation supplies and no `outputs` list could
state; they are declared with [`tagged`](/reference/plugins/tagged-outputs/),
which says what the first popped value has to be for the rest to follow.

Three plugins that ship with NSIS are **deliberately not declared**; the reasons
are in
[Plugins with no declaration](/reference/commands/not-available/#plugins-with-no-declaration), and
[the plugin reference](/reference/plugins/) is the per-method detail for the
third-party set. Anything else is declared by the project in
[`.installua/declarations/*.toml`](#declaring-a-third-party-plugin-or-header), which
is what supplies the output count nothing can ask the DLL for.

```lua
local nsExec = plugin "nsExec"
local userInfo = plugin "UserInfo"

onInit(function()
	if userInfo.getAccountType() ~= "Admin" then
		abort("administrator rights are required")
	end
end)
```

## import

Brings a declared NSIS header's macros into scope. The `!include` and any
`${Using:…}` init lines are emitted for you, once, in the right place.

**Usage** `local h = import(header)` · `h.macro(…)`

Three headers ship declared, and between them they are the whole of what NSIS
provides that a fixed argument list can describe:

| Header | Methods | Worth knowing |
| ------ | ------- | ------------- |
| `FileFunc` | `getParameters`, `getOptions`, `getOptionsS`, `getParent`, `getFileName`, `getBaseName`, `getFileExt`, `getRoot`, `bannerTrimPath`, `getExeName`, `getExePath`, `getSize`, `driveSpace`, `getTime`, `getFileVersion`, `getFileAttributes`, `dirState`, `refreshShellIcons` | `getTime` returns **day, month, year, weekday, hour, minute, second** — seven strings, and a swapped pair is invisible at runtime. `dirState` is `-1` missing, `0` empty, `1` has files |
| `WordFunc` | `wordFind`, `wordFind2X`, `wordFind3X`, `wordReplace`, `wordAdd`, `wordInsert`, `strFilter`, `versionCompare`, `versionConvert`, and the `S` half of the first seven | Every result is a `string`: the option argument decides whether the answer is a word or a count, so no narrower type is available |
| `TextFunc` | `lineRead`, `lineSum`, `fileJoin`, `configRead`, `configReadS`, `configWrite`, `configWriteS`, `fileRecode`, `trimNewLines` | `fileJoin` and `fileRecode` return **nothing** — their result is the file. `trimNewLines` takes a string, not a path, despite what NSIS calls the argument |

**The `S` names are the case-sensitive halves**, and NSIS ships each as its own
macro rather than as an option, so each is its own method here. Note which way
round it is: the unsuffixed name is the case-**in**sensitive one, the opposite
of [`==`](/reference/commands/strings-and-numbers/). That is not an inconsistency — `==` is
case-sensitive because Lua's is, and `import` is the NSIS-shaped surface, where
the name you arrive with should be the one that works.

**Six of them call back into the script**, and those are written as loops
rather than as calls — see [walkers](#walkers) below.

Any other header's macros are declared by the project in
[`.installua/declarations/*.toml`](#declaring-a-third-party-plugin-or-header). A
declaration ships when the fact it records is undiscoverable *and* the caller is
common; the argument order of a macro that writes its outputs into trailing
registers is exactly that, which is why these three arrived together.
`import` itself needs no declaration: the `!include` is emitted for whatever name
it is given, so a header reached only through `raw` still gets its line.

```lua
local fileFunc = import "FileFunc"
local wordFunc = import "WordFunc"

local freeMib = fileFunc.driveSpace("C:/", "/D=F /S=M")
detailPrint(string.format("%u MiB free", freeMib))

local installed = readRegStr(HKLM, "Software/Example", "Version")
if wordFunc.versionCompare(installed, "1.4.2") == "1" then
	detailPrint("downgrade")
end
```

## walkers

Six declared macros do not return a value — NSIS calls the script back, once
per file or per line. Five of them are written as `for … in` loops:

**Usage** `for a, b in header.method(…) do … end`

```lua
local fileFunc = import "FileFunc"

for path, directory, name, size in fileFunc.locate(INSTDIR, "/L=F /M=*.tmp") do
	if size > 1048576 then
		detailPrint("large: " .. name .. " in " .. directory)
	end
	delete(path)
end
```

| Walker | Yields | Ends with |
| ------ | ------ | --------- |
| `fileFunc.locate(path, options)` | `path`, `directory`, `name`, `size` | `StopLocate` |
| `fileFunc.getDrives(types)` | `drive`, `kind` | `StopGetDrives` |
| `textFunc.fileReadFromEnd(file)` | `line`, `remaining`, `number` | `StopFileReadFromEnd` |
| `textFunc.textCompare(a, b, option)` | `line`, `number`, `other`, `match` | `StopTextCompare` |
| `textFunc.textCompareS(a, b, option)` | the same, case-sensitively | `StopTextCompare` |

Bind as few names as the body wants — `for path in fileFunc.locate(…)` is the
common call, and the registers nothing bound are never read.

**`break` is not a jump.** The walk belongs to NSIS, so ending it means pushing
the sentinel in the table above and returning; falling off the end pushes the
empty string and the walk carries on. A bare `return` means the same thing as
falling off the end, and returning a *value* is an error — a walker's body has
no third answer for one to carry.

**The body sees no enclosing local.** `${Locate}` uses `$0`–`$9` for its own
bookkeeping while the walk runs, so a register holding a section's local does
not survive to the callback. Globals do, and are the way out.

### lineFind

The sixth is not a loop, because its body answers with a **value**: the line to
write. Three answers, where a loop has two.

**Usage** `textFunc.lineFind(input, output, range, body)` → nothing

```lua
textFunc.lineFind(INSTDIR .. "/app.ini", INSTDIR .. "/app.new", "1:-1", function(line, number)
	if number > 500 then
		return stop      -- ends the walk
	end
	if line == "DEBUG=1" then
		return skip      -- the line is not written
	end
	return line          -- what gets written; a changed string rewrites it
end)
```

`stop` and `skip` are bare words **in this position only**, so a local called
`stop` elsewhere is unaffected. `/NUL` as the output writes nothing at all,
which makes the call a read-only pass over the file. The range is `first:last`,
counting from the end when negative.

### Why these are not ordinary declarations

Every other macro is described entirely by its `.toml`. These six are not: NSIS
hands the callback its arguments in **registers it names** — `$R9` down to
`$R6` in `FileFunc`, `$9` down to `$6` in `TextFunc` — and reads the answer off
the stack, with `lineFind` reading `$R9` again on the way out. That is behaviour
rather than arity, so the declaration says only `callback` and the register map
lives in `src/lower/callback.rs`.

The consequence is worth stating plainly: **a third-party callback macro cannot
be declared.** `params = [… , "callback"]` naming a macro that table does not
know is an error rather than a guess. A register map written into a `.toml` by
hand would compile, assemble, and hand a caller a directory where it asked for
a file name, with no diagnostic possible from anywhere — and that is the one
shape of mistake this compiler exists to prevent. `raw` remains for anyone who
needs it and is willing to write `$R9` themselves.

## raw

Text handed to `makensis` unread. As a statement it produces no value, no
`local` survives it, and a failure inside one is reported as _yours_ rather than
the compiler's. (There is one other position — see
[raw in an argument](#raw-in-an-argument) below.)

**Usage** `raw [[ … ]]` → nothing

```lua
raw [[
  SetRegView 64
]]
```

A value crosses the boundary in a [global](/reference/commands/program-structure/#var), whose NSIS name is the one
you wrote — so the raw text names it directly and the Lua on either side reads
and writes it as an ordinary variable:

```lua
outVar = ""

raw [[
  nsExec::ExecToStack '"cmd.exe" /c ver'
  Pop $outVar
]]
detailPrint("got " .. outVar)
```

A `local` is not that: registers belong to the allocator, which colours them
and computes each call site's save list, so a name pinned to `$R0` is a promise
it has no way to keep and the failure is wrong data rather than a diagnostic.
The global costs one `Var` line and one zero-init in `.onInit`.

### raw in an argument

The one position where `raw` is not a statement. As an argument of a
[declared plugin method](#declaring-a-third-party-plugin-or-header) it splices
its text into that call's line — unquoted, with no path conversion and no type
check — and everything else about the call stays declared:

```lua
local node = nsJSON.get(raw "/index 0 /index 1 /index 3", "$Doc")
```

**Usage** `plugin.method(…, raw "…", …)` → the method's own outputs

It is for the argument shape a `params` list cannot describe — most often a
count the caller picks per call. However many words it spells, a spliced
argument is **one** argument, so the position count and the number of `Pop`s
after the line both remain the declaration's. That is the whole difference from
writing the call in a `raw` block, where no `local` survives and the outputs
have to cross into the rest of the section through a global with the `Pop`s
written by hand.

The cost is that one position: its declared type and its `path` flag both
describe a value, and there is no value there — what you write is what
`makensis` sees.

### raw.head and raw.tail

Inside a body, `raw [[ … ]]` means _here_, and where it lands needs no saying.
At the top level there is no _here_: the emitter's slots are fixed and the order
you write declarations in is not the order they come out in. So a top-level
block names its anchor, and the anchor is part of the name — there is no form
that leaves it off.

**Usage** `raw.head [[ … ]]` · `raw.tail [[ … ]]` → nothing

```lua
raw.head [[ !system 'git rev-parse --short HEAD > rev.txt' ]]
raw.tail [[ !packhdr "tmp.dat" '"upx.exe" "tmp.dat"' ]]
```

| Anchor | Where | For |
| ------ | ----- | --- |
| `head` | above every line the compiler writes, including `Unicode` | text producing a value the script then reads: `!system`, `!tempfile`, `!getdllversion` |
| `tail` | below everything | registrations `makensis` acts on when the build ends: `!packhdr`, `!finalize`, `!uninstfinalize` |

Two, and a third arrives when a real script needs one. Each names a **boundary
between numbered slots**, never a region — "before everything" and "after
everything" are boundaries no future slot can move, which is what makes them
safe to promise while the rest of the spine is still settling.

**What may go at an anchor** is text whose meaning is position-independent. Text
whose meaning depends on what the compiler generated is a _declaration the
compiler places_, not an anchor's business: `!addplugindir` written at `head`
would land above `Unicode`, bind to the default target, and silently break every
`unicode = false` build — so it is a slot the compiler owns, reached through
`dir` on a [`[[plugin]]` declaration](#declaring-a-third-party-plugin-or-header).
`$PLUGINSDIR` is the same rule from the other side, and it is diagnosed: the
directory is made by an `InitPluginsDir` the compiler puts above the statement
naming it, and an anchor is outside every body.

## Declaring a third-party plugin or header

One file per plugin or header in `.installua/declarations/`, read by the compiler,
the editor stubs and the linter alike. The file name is yours; the extension is
`.toml`.

```toml
# .installua/declarations/nsisunz.toml
[[plugin]]
name = "nsisunz"                      # what `plugin "…"` is given
method = "unzipToLog"                 # what you call it
nsis = "nsisunz::UnzipToLog"          # what NSIS is given
params = ["path", "path"]             # positions, in order
outputs = ["string"]                  # values pushed, in `Pop` order
dir = "vendor/plugins"                # only if the DLL is not in NSISDIR

[[header]]
name = "Brand"                        # what `import "…"` is given, and the
method = "applyTheme"                 # `!include "Brand.nsh"` that follows
nsis = "BrandApplyTheme"              # the macro name, without `${}`
params = ["string"]
outputs = ["string"]                  # trailing registers, in the order written
```

The header here is one of your own, beside the script, because everything NSIS
ships that a fixed argument list can describe [ships
declared](#import) — the headers left over are macro frameworks whose value is
control flow, and those are not signatures at all.

`name`, `method` and `nsis` are required. `nsis` is not derived from `method`,
because `${StrCase} $0 "text" "L"` puts its destination _first_ and
`${GetSize} "$dir" "" $0 $1 $2` puts it _last_: there is no convention to infer,
and guessing emits NSIS that looks right and is not.

**The types** are `string`, `path`, `int`, `uint`, `int64`, `intptr`, `bool`,
`handle` and `any`. `path` is an input spelling — a `string` whose `/` becomes
`\` on the way in — so it is rejected in `outputs`, where the callee has already
written whatever it wrote. `uint` is worth reaching for on a count or a size:
knowing a value cannot be negative is what elides the sign fixup on `//`.

**`outputs` is the load-bearing line.** NSIS offers no way to ask a DLL how many
values it pushes, so `local rc, out = …` is checked against this list and
nothing else. A count that is too small unbalances the stack, with no diagnostic
from NSIS or from anybody.

**`tagged` and `more` are for a count that is not a number.** A great many
plugins push a different number of values depending on the outcome and say
which by the first value they push:

```toml
[[plugin]]
name = "AccessControl"
method = "grantOnFile"
nsis = "AccessControl::GrantOnFile"
params = ["path", "string", "string"]
outputs = ["string"]                  # every path pushes this
tagged = ["error"]                    # when it is one of these …
more = ["string"]                     # … these follow it
```

Both lists are part of the Lua arity — `local ok, why = …` binds two — and the
tail reads `""` (or `0`, for a numeric type) on the path where the plugin
pushed nothing. `tagged` is a **list of literals** rather than a fixed spelling
because the polarity is the plugin's to choose: `AccessControl` tags its
failure, `StartMenu::Select` tags its *success*. The two are declared the same
way and mean opposite things.

The pair is plugin-only — a macro writes its outputs into registers on every
path, so there is no first value to test — and `outputs` must declare at least
one value for `tagged` to be about. See
[the plugin reference](/reference/plugins/tagged-outputs/) for the lines this
emits and for the one failure it cannot cover.

**`flags` is for the `/SWITCH` tokens a plugin takes alongside its arguments.**
They are named at the call site, in a table written **last**, and emitted
**first** — position is the declaration's, not yours, which is also what lets
`trailing` below move the whole run behind the arguments instead:

```toml
[[plugin]]
name = "StartMenu"
method = "select"
nsis = "StartMenu::Select"
params = ["string"]
outputs = ["string"]
tagged = ["success"]
more = ["string"]
flags = [
  { name = "autoadd", nsis = "/autoadd" },                            # on or off
  { name = "text", nsis = "/text", ty = "string", value = "separate" },
  { name = "timeout", nsis = "/TIMEOUT", ty = "int", value = "joined" },
]
```

```lua
startMenu.select("Example", { lastused = INSTDIR, autoadd = true })
-- StartMenu::Select /autoadd /lastused $INSTDIR "Example"
```

`name` is the table key and `nsis` is the token, written out rather than derived
— `/NOINHERIT` is upper and `/checknoshortcuts` is lower, and a token NSIS does
not recognise as a flag becomes a positional argument without complaint.

A flag with no `ty` **is** its value: the table field is `true` or `false`, and
`false` writes nothing, because NSIS has no spelling for an off switch. It has
to be a literal rather than a variable — the call line is assembled before
anything runs, so nothing at runtime can decide whether a token was written.

`ty` and `value` are one field in two halves, like `tagged` and `more`. `value`
is `joined` for `/TIMEOUT=5000` or `separate` for `/text "…"`, and the value
itself is an ordinary expression, register and all: `/FLAGS=$R0` is a shape the
corpus uses.

The order flags are emitted in is the order they are **declared**, never the
order the table names them: a table has no order, and two calls naming the same
flags have to emit the same line. They are plugin-only — `!insertmacro` takes
its arguments by position, so a macro's option string is one of its `params`.

**`trailing = true` moves the whole run of flags behind the fixed arguments.**
One plugin needs it. `NScurl::http` reads its first three stack values by
*index* — method, URL, output path — and only starts testing for a leading `/`
from the fourth, so a leading `/SILENT` there is not a flag, it is the HTTP verb
and the request goes out asking for `SILENT`:

```toml
[[plugin]]
name = "NScurl"
method = "http"
nsis = "NScurl::http"
params = ["string", "string", "path"]
outputs = ["string"]
trailing = true
terminator = "/END"
```

```lua
nscurl.http("GET", url, PLUGINSDIR .. "/tool.zip", { silent = true })
-- NScurl::http "GET" "…" "$PLUGINSDIR\tool.zip" /SILENT /END
```

Nothing else changes: the table is still written last, still unordered, and the
flags still come out in declaration order. It is plugin-only, and `false` — a
leading run — is the default every other declaration takes.

A table has unique keys and no order, so a flag written **more than once** —
`nsJSON::Get /index 0 /index 1` — has no encoding here and is not declarable.
Neither is one written *between* two positional arguments. Both belong to
`nsJSON`; see [the plugin reference](/reference/plugins/what-stays-out/).

**`terminator` is for a plugin that reads its arguments in a loop.** `inetc`
takes url/file pairs until it pops the token `/END`, and a call that omits it
keeps popping — past its own arguments and into whatever lies underneath, which
under this compiler is a caller-saved register:

```toml
[[plugin]]
name = "Inetc"
method = "get"
nsis = "inetc::get"
params = ["string", "path"]
outputs = ["string"]
terminator = "/END"
```

```lua
inetc.get(url, PLUGINSDIR .. "/tool.zip", { silent = true })
-- inetc::get /SILENT "…" "$PLUGINSDIR\tool.zip" /END
```

So it is not a flag and not an option: it is emitted after the fixed arguments
on **every** call, and the call site can neither set it nor leave it off. It
must begin with `/`, because a token that does not is one the plugin will read
as an argument — a fixed trailing argument belongs in `params`.

**`dir` is for a DLL that does not live in `NSISDIR/Plugins`** — a plugin
vendored into your own repository. It is relative to the project root, and the
compiler emits one `!addplugindir` for it, in the one position the directive is
correct in: under the `Unicode` line and above every call site. You never write
that line yourself, and there is no anchor that would let you — an untagged
`!addplugindir` binds to whichever target is current when it is processed, so
one written at the top of a source would bind to the default target and silently
break every `unicode = false` build. Only a plugin the program actually calls
emits a line.

Redeclaring one of the builtins is allowed and replaces it, so a count that
ships wrong here is not a wall. Declaring the same method twice from two files
in the *same* `.installua/declarations/` is a mistake, and says so.

## Sharing declarations across a monorepo

Several installers in one checkout can read one declaration instead of a copy
each. Write `installua.toml` at the top — `installua init --workspace .` is the
whole of it — and a compile walks up from the source's own directory reading
every `.installua/declarations/` it passes:

```text
myapp/
  installua.toml              # root = true — the walk stops here
  .installua/declarations/
    acme.toml                 # shared by both installers below
  installers/
    pro/install.lua
    lite/install.lua
```

The nearer directory wins where two declare the same method, by the same rule a
project's own file wins over a builtin — so the shared one is a default, not a
wall. The walk also stops at a directory holding `.git`, marker or no marker,
and the file holds nothing but `root`: it says where the search ends and
nothing about what the compiler does.

Projects with no `installua.toml` anywhere are unaffected, which is most of
them: a single installer reads its own `.installua/declarations/` and no other.

Run `installua stubs` after adding a declaration: the compiler reads the `.toml`
on every build, but the editor reads the generated stub.

What ships declared lives in `src/declarations/*.toml` in this repository, in this
same format and read by this same parser — copy one into `.installua/declarations/`
to correct it, or send it back as a pull request so nobody else has to.

---
