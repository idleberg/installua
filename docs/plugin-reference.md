# Plugin reference

Every plugin method Installua ships a declaration for, and every method it
deliberately does not.

A declaration carries **one fact**: how many values the plugin leaves on the
stack. NSIS offers no way to ask a DLL, and getting it wrong does not fail —
it shifts every later `Pop` by one and assembles cleanly. That is why this file
exists, and why each entry says where its count came from.

**A declaration is not a bundled DLL.** Installing the plugin into
`NSISDIR/Plugins`, or pointing `dir` at a vendored copy, is still yours to do.
Nothing here ships a binary.

For the format itself — how to declare a plugin of your own — see
[Declaring a third-party plugin or header](reference-map.md#declaring-a-third-party-plugin-or-header).
For header macros — `FileFunc`, `TextFunc`, `WordFunc` — see
[header-reference.md](header-reference.md). For the call syntax, see
[`### plugin`](reference-map.md#plugin).

## How the counts were established

Two sources, and where they disagree the method is excluded rather than guessed:

1. The plugin's own documentation — its readme, otherwise its wiki page. Each
   `.toml` says which, and the ones read from a wiki page say so rather than
   borrowing the source-level claim the older first-party files make.
2. A scan of **984 real-world scripts** (`nsis-corpus`), counting the `Pop`s
   that actually follow each call site.

The second is what turns a documented count into a measured one, and it caught
[`SimpleSC.getErrorMessage`](#simplescgeterrormessage-is-not-declarable), whose
argument arrives by `Push` — something no reading of the documentation would
have found.

**Where the two still disagree, read the source.** That happened on
[AccessControl](#accesscontrol), and it was worth the trouble: the readme, the
wiki page and the corpus each imply a *different* set of declarable methods, and
all three are wrong. Documentation describes the path its author was thinking
about, and an arity is a claim about every path.

---

# Tagged outputs

A great many plugins push a number of values that **depends on the outcome**,
and say which by the first value they push. `AccessControl::GrantOnFile` pushes
`"ok"` alone, or `"error"` and a description underneath it;
`StartMenu::Select` pushes `"success"` **and** a folder, or `"cancel"` or an
error message alone.

A flat `outputs` list has to pick one of those and be wrong on the other — and
being wrong is not a wrong type. It is an unbalanced stack: every later `Pop` in
the section shifts by one, and neither Installua nor NSIS says a word. So the
declaration says it instead:

```toml
[[plugin]]
name = "AccessControl"
method = "grantOnFile"
nsis = "AccessControl::GrantOnFile"
params = ["path", "string", "string"]
outputs = ["string"]
tagged = ["error"]
more = ["string"]
```

`outputs` is what comes back on **every** path; `more` is what follows when the
first popped value is one of `tagged`. Both are part of the Lua arity:

```lua
local granted, why = accessControl.grantOnFile(INSTDIR, "(BU)", "FullAccess")
if granted == "error" then
    detailPrint("ACL not set: " .. why)
end
```

which becomes

```nsis
AccessControl::GrantOnFile $INSTDIR "(BU)" "FullAccess"
Pop $0
StrCpy $1 ""
StrCmpS $0 "error" 0 __GENERATED_tail_0
Pop $1
__GENERATED_tail_0:
```

Three things about that emission are deliberate. **The default is written
first**, so `$1` is defined on both paths and the register allocator never sees
a conditional definition — which is what keeps this out of the control-flow
graph entirely. The test is `StrCmpS`, so a payload differing from a tag only in
case is a different value. And the target is a label rather than `+2`, because a
relative jump is correct until a later pass inserts a line.

**`tagged` is a list of literals, not the word "error".** The polarity is the
plugin's to choose, and the two above chose opposite ones: a design that
hardcoded the failure spelling would describe AccessControl and misdescribe
StartMenu by exactly one value, on every run that worked.

A tail the caller does not bind is still popped. The plugin put it there; what
the caller wanted has no bearing on what the stack holds.

## The hazard this cannot cover

The `Pop` is emitted because the declaration says the value is there. A plugin
that pushes its tag and then **fails to push the tail** — AccessControl does
exactly this when `LocalAlloc` fails — leaves the `Pop` to take whatever is
underneath, which is a caller-save, and the stack is corrupt from there on.

Nothing can detect it: NSIS offers no way to ask how deep the stack is, and the
value popped is a perfectly good string. Every hand-written NSIS script that
tests `== error` and pops again has the identical bug. It is an out-of-memory
path on a plugin that just allocated, so it is documented rather than defended
against — the defence would be to not pop at all, which is wrong on every other
run.

---

# Plugins that ship with NSIS

Eleven, every count read from the plugin's own source under `NSISDIR/Contrib`
rather than from its readme.

## nsExec

| Method | Arguments | Returns |
| ------ | --------- | ------- |
| `.execToStack(command)` | `string` | **exit code, output** |

The exit code comes off first, then the captured output. That is `Pop` order,
and it is the order nothing in the source states — it is the reason
`local rc, out = nsExec.execToStack(…)` is legal at all.

All three of its flags are declared, in a table written last:

| Flag | Emits | Effect |
| ---- | ----- | ------ |
| `timeout = 5000` | `/TIMEOUT=5000` | milliseconds to wait *for output*, reset on every byte received |
| `oem = true` | `/OEM` | converts the captured output from OEM to ANSI |
| `mbcs = true` | `/MBCS` | treats the output as ANSI rather than detecting Unicode |

```lua
local code, output = nsExec.execToStack("cmd.exe /c ver", {
	oem = true,
	timeout = 5000,
})
-- nsExec::ExecToStack /TIMEOUT=5000 /OEM "cmd.exe /c ver"
```

`nsexec.c` reads its flags in a loop — `goto params` after each match — so the
order a call site writes them in genuinely does not matter to the plugin. It
matters here anyway: emission follows the declaration, so two calls naming the
same flags emit the same line.

**A timeout is not an error the return value distinguishes by shape.** On one
the plugin pushes the string `"timeout"` where an exit code would be, and on a
failure to launch, `"error"` — both in the same slot as a number, which is why
the first output is a `string`.

## UserInfo

| Method | Arguments | Returns |
| ------ | --------- | ------- |
| `.getAccountType()` | — | `"Admin"`, `"Power"`, `"User"`, `"Guest"` or `""` |

## System

| Method | Arguments | Returns |
| ------ | --------- | ------- |
| `.call(signature)` | `string` | one value per `.s` in the signature |

**The one entry whose count is not in its declaration.** `outputs` is empty on
purpose: `System::Call`'s output count lives in its signature, one per `.s`, and
the lowering counts them there ([`src/lower/expr.rs`](../src/lower/expr.rs)).
That also means the signature has to be a build-time constant — a runtime string
cannot be counted, and is refused rather than guessed.

Parsing the rest of the signature, which would narrow the clobber set from
"everything", is deferred.

## Dialer

| Method | Arguments | Returns |
| ------ | --------- | ------- |
| `.attemptConnect()` | — | `"online"` / `"offline"` |
| `.getConnectedState()` | — | `"online"` / `"offline"` |
| `.autodialOnline()` | — | `"online"` / `"offline"` |
| `.autodialUnattended()` | — | `"online"` / `"offline"` |
| `.autodialHangup()` | — | `"success"` / `"failure"` |

`autodialUnattended` is the unattended twin of `autodialOnline` — same result
strings, no prompt.

**One caveat, and it is why these were nearly excluded.** Each method resolves
its entry point out of `wininet.dll` at call time, and on the branch where
`GetProcAddress` fails it sets the error flag and pushes **nothing at all**. The
`Pop` is emitted before anything could test `errors()`, so on that branch the
value read is whatever was underneath.

Declared anyway, on a line worth stating: this arity varies only on a Windows
old enough to lack `InternetAutodial` — older than 98, or a 95 that never saw
IE4. It is not the [tagged-output](#tagged-outputs) shape, which varies on an
ordinary path and has a first value that says which; here there is no value at
all to test, and the branch is unreachable on anything this century.

## NSISdl

| Method | Arguments | Returns |
| ------ | --------- | ------- |
| `.download(url, file)` | `string`, `path` | `"success"`, `"cancel"`, or a message |
| `.downloadQuiet(url, file)` | `string`, `path` | the same |

**Plain HTTP only — no HTTPS.** That is the fact that decides whether this is
the right call at all in 2026.

The URL is a `string` and deliberately not a `path`: `path` normalises `/` to
`\`, which would turn every URL into a broken one. `downloadQuiet` is the same
function with the progress window suppressed — `download_quiet` in the source
calls straight through to `download`, which is why the two counts cannot drift.

Three of its five flags are declared, on both methods:

| Flag | Emits | Effect |
| ---- | ----- | ------ |
| `timeout = 30000` | `/TIMEOUT=30000` | milliseconds without data before it gives up; 30000 is the plugin's own default |
| `proxy = "host:port"` | `/PROXY "host:port"` | uses that proxy instead of Internet Explorer's |
| `noieproxy = true` | `/NOIEPROXY` | connects direct, ignoring Internet Explorer's proxy |

```lua
local status = NSISdl.download(url, PLUGINSDIR .. "/data.pat", {
	noieproxy = true,
	timeout = 30000,
})
-- NSISdl::download /TIMEOUT=30000 /NOIEPROXY "…" "$PLUGINSDIR\data.pat"
```

**Unlike nsExec, `nsisdl.cpp` checks each flag exactly once, in that order**, so
a call that wrote `/NOIEPROXY` before `/TIMEOUT=` would have the timeout read as
the URL. The declaration lists them in the source's order and emits in
declaration order, which is what makes the call site's order free.

`/TRANSLATE` and `/TRANSLATE2` stay undeclared. Each is a flag followed by eight
or nine further positional strings — the localised progress texts — and that is
not a shape `flags` can spell: a flag carries one value or none.

## Splash

| Method | Arguments | Returns |
| ------ | --------- | ------- |
| `.show(delay, bitmap)` | `uint`, `path` | `1` closed early, `0` timed out, `-1` error |

The path is the bitmap **without its extension**: the plugin appends `.bmp`, and
`.wav` for a sound file of the same name beside it. Still a `path`, because it
is a file position and a `/` would ship into one. The result is `int` rather
than `uint` because of that `-1`.

## AdvSplash

| Method | Arguments | Returns |
| ------ | --------- | ------- |
| `.show(delay, fadeIn, fadeOut, keyColour, bitmap)` | `uint`, `uint`, `uint`, `int`, `path` | as `Splash.show` |

Splash plus fading and a transparent colour. The parameter order is the pop
order and nothing else states it.

**The fades are not counted inside the delay** — that is the mistake this entry
exists to prevent. The key colour is `0xRRGGBB`, or `-1` for no transparency,
which is why it is `int` beside three `uint` durations.

## Banner

| Method | Arguments | Returns |
| ------ | --------- | ------- |
| `.show(text)` | `string` | nothing |
| `.getWindow()` | — | `handle` |
| `.destroy()` | — | nothing |

**The banner is a window, not a page.** Nothing destroys it for you, and a
script that forgets `.destroy()` leaves it on screen for the rest of the
install.

`getWindow` gives the banner's own HWND, written with `%u` and read back by
`GetDlgItem` and friends — `handle` rather than `uint`, because arithmetic on it
is a mistake.

`show` is declared in its **one-string form only**. Its real parameter list is
`[/set id text]... text`, and a repeated flag pair is not a fixed arity; the
format has no spelling for one, and inventing a trailing `any` would let a wrong
call through rather than catch it. The `/set` form is a `raw`.

## TypeLib

| Method | Arguments | Returns |
| ------ | --------- | ------- |
| `.register(file)` | `path` | nothing |
| `.unregister(file)` | `path` | nothing |
| `.getLibVersion(file)` | `path` | **minor, major** |

**Minor first.** The source pushes the major version and then the minor, so
`Pop` order hands back the minor one first:
`local minor, major = typeLib.getLibVersion(path)`. `Library.nsh` reads it in
exactly that order at its `Pop $R3` / `Pop $R2`, and reversing the two compiles
and is wrong.

Two values on **every** branch, failure included: a library that will not load
pushes `"0"` twice rather than nothing, so the count is unconditional and a
`0.0` result *is* the error.

## VPatch

Applying a `GenPat`-built binary patch.

| Method | Arguments | Returns |
| ------ | --------- | ------- |
| `.patchFile(patch, source, destination)` | `path`, `path`, `path` | a status string |
| `.getFileCrc32(file)` | `path` | eight hex characters, or `""` |
| `.getFileMd5(file)` | `path` | 32 hex characters, or `""` |

**The destination is a *new* file.** VPatch does not update in place; renaming
the result over the original is the caller's step, and the reason the plugin
cannot be handed one path twice.

`patchFile` answers `"OK"`, `"OK, new version already installed"`, or one of
four failure sentences — so callers test the `OK` prefix rather than equality,
because the second success string is a success. `getFileCrc32` and `getFileMd5`
give the **empty string** rather than an error sentence when the file will not
open, so check for `""` before comparing.

Those two exist only because the plugin is built with `DLL_CHECKSUMS` defined in
its `SConscript`, which the shipped DLL is. They are undocumented in the readme
and were read from the source.

The `patchFile` export is lower-case `vpatchfile`; the namespace is spelled the
way the DLL is named, so the emitted call and its `ReserveFile /plugin
VPatch.dll` agree. `AdvSplash` is spelled for the same reason — its readme
writes `advsplash::show`, and NSIS matches the namespace case-insensitively.

## StartMenu

The custom page that asks which Start Menu folder to put shortcuts in, and the
[tagged-output](#tagged-outputs) shape with the *unusual* polarity.

| Method | Arguments | Returns |
| ------ | --------- | ------- |
| `.select(defaultFolder)` | `string` | `"success"` + the folder, or `"cancel"` or a message alone |
| `.init(defaultFolder)` | `string` | the page's `HWND`, or an error string |
| `.show()` | — | the same as `.select` |

**Source: `Contrib/StartMenu/StartMenu.c`.** `Select` is `Init`, a `popstring`
that discards the `HWND`, then `Show` — so the three arities are one mechanism:
the dialog procedure pushes the folder and then `"success"` on top of it, or
`"cancel"` alone, and a failed `Init` leaves its error string where `Select`'s
`popstring` never reaches it.

Use `.init` / `.show` in place of `.select` only when the page's controls need
restyling in between; that is the pair's whole purpose, and `GetDlgItem` on the
returned handle is how the readme does it. **Check `.init`'s result before
calling `.show`** — on the path where `Init` failed, `Show` returns having
pushed nothing at all, and the `Pop` Installua emits would take a caller-save.
That is the same hazard [tagged outputs](#the-hazard-this-cannot-cover)
document, reached a different way.

`.select` takes all six of the page's flags, in a table written last:

| Flag | Emits | Effect |
| ---- | ----- | ------ |
| `autoadd = true` | `/autoadd` | appends the program name to the chosen folder |
| `noicon = true` | `/noicon` | drops the icon in the top-left corner |
| `rtl = true` | `/rtl` | lays every control out right-to-left |
| `text = "…"` | `/text "…"` | replaces the page's top text |
| `lastused = "…"` | `/lastused "…"` | seeds the edit box, for remembering a choice |
| `checknoshortcuts = "…"` | `/checknoshortcuts "…"` | adds a checkbox with that label |

```lua
local outcome, folder = startMenu.select("Example", {
	lastused = INSTDIR,
	autoadd = true,
})
-- StartMenu::Select /autoadd /lastused $INSTDIR "Example"
```

The readme is unusually direct about why a table is the right surface here:
*"the order of the switches doesn't matter but the required parameter must come
after all of them"*. That is the declaration's job in one sentence — you name
the flags in any order and it places them, ahead of the argument, every time.

The folder comes back **prefixed with `>`** when the user ticks the
`checknoshortcuts` box, and it is a sub-folder name rather than a full path —
joining it to `$SMPROGRAMS` is the caller's step.

`page.startMenu` is the built-in page that asks the same question through MUI2
and remembers the answer in the registry; reach for this plugin when you want
the dialog somewhere a page cannot go.

---

# Third-party plugins

Six, chosen on a scan of 984 real-world scripts.

## EnVar

Environment variables, 95 corpus scripts — the most-used third-party plugin here
that is uniform all the way through.
Source: <https://github.com/GsNSIS/EnVar>.

`setHKCU` and `setHKLM` are **stateful**: each applies to every later call in
the script, not to one. That is why they are ordinary calls rather than a
parameter on the others. `setHKCU` is the default.

| Method | Arguments | Returns |
| ------ | --------- | ------- |
| `.setHKCU()` | — | nothing |
| `.setHKLM()` | — | nothing |
| `.check(name, value)` | `string`, `path` | code |
| `.addValue(name, value)` | `string`, `path` | code |
| `.addValueEx(name, value)` | `string`, `path` | code |
| `.setValue(name, value)` | `string`, `path` | code |
| `.setValueEx(name, value)` | `string`, `path` | code |
| `.deleteValue(name, value)` | `string`, `path` | code |
| `.delete(name)` | `string` | code |
| `.update(root, name)` | `string`, `string` | code |

The code is a `uint`, and the five values are documented: `0` success, `1`
cannot read, `2` no such variable, `3` no such value, `4` cannot write. `uint`
rather than `int` because the plugin's own table is the proof — no code is
negative, and `if code == 0` compiles to `IntCmpU` rather than a string compare.

Three things the table cannot say:

- **`check` asks three questions.** `"NULL"` in the value position means "does
  this variable exist"; `"NULL"` in both means "is the environment writable at
  all"; neither means "does this variable contain this value".
- **The `Ex` variants write `REG_EXPAND_SZ`**, so a value containing
  `%LOCALAPPDATA%` is expanded on read rather than frozen on write.
  `addValueEx` also converts an existing plain variable to the expandable type —
  reach for it on a new variable, and leave `addValue` alone on `PATH`.
- **`update`'s root is `"HKCU"`, `"HKLM"`, or anything else meaning both**
  (appended, HKLM first). The empty string is *meaningful* here rather than
  missing, which is why it is a plain `string` and not an enum. `update` ignores
  `setHKCU`/`setHKLM` and writes nothing — it reloads the variable into the
  running installer's own environment, which is what a later `execWait` needs.

The value position is a `path` because that is what the plugin's documentation
calls it and what the plugin is for. **The cost is real**: a variable whose
value is not a path gets its forward slashes turned into backslashes. Storing a
URL in an environment variable wants `raw`.

## SimpleSC

Windows services, 61 corpus scripts.
Source: <https://nsis.sourceforge.io/NSIS_Simple_Service_Plugin>.

| Method | Arguments | Returns |
| ------ | --------- | ------- |
| `.installService(name, display, type, start, binary, deps, account, password)` | `string`, `string`, `int`, `int`, `string`, `string`, `string`, `string` | code |
| `.removeService(name)` | `string` | code |
| `.startService(name, arguments, timeout)` | `string`, `string`, `int` | code |
| `.stopService(name, waitForRelease, timeout)` | `string`, `int`, `int` | code |
| `.existsService(name)` | `string` | code |
| `.serviceIsRunning(name)` | `string` | **code, running** |
| `.getServiceStatus(name)` | `string` | **code, status** |
| `.setServiceDescription(name, text)` | `string`, `string` | code |
| `.setServiceStartType(name, type)` | `string`, `int` | code |
| `.setServiceFailure(name, reset, message, command, ×3 type and delay)` | ten, see the `.toml` | code |

Every code is `int`, not `uint`: the documentation says `0` for success and "the
Windows error code" otherwise without ever bounding it below, and `uint` needs
proof rather than an absence of counterexamples.

- **`serviceIsRunning` and `getServiceStatus` push two.** The code comes off
  first and only says whether the question could be asked; the answer is the
  *second* value. `local queried, running = …` — swapping those two names
  compiles, which is the whole reason the order is written down.
- **`existsService` returns `0` for yes**, which reads backwards. One corpus
  script carries the comment `; <> 0 => service exists`, which has it wrong.
- **`installService`'s binary is a `string`, not a `path`**, because it is a
  command line: the corpus passes
  `"$INSTDIR\bin\agent.exe -conf $\"$INSTDIR\conf\cli.conf$\""` there, and
  normalising slashes would rewrite the arguments too.
- **Nothing here is optional.** Three corpus call sites pass fewer arguments
  than documented — `startService "$name"` with no timeout, for instance. A
  plugin reads a fixed number of items off the stack, so a short call reads
  whatever the script happened to leave there. Those are bugs in two scripts,
  not evidence of an optional tail.

The twenty-odd further methods the page documents are left out on the *common*
half of the rule: none has a corpus call site. Each is five lines in your own
`.toml`.

### `SimpleSC.getErrorMessage` is not declarable

Both of its corpus call sites read:

```nsis
Push $0
SimpleSC::GetErrorMessage
Pop $0
```

The error code goes in by **`Push`**, not as an inline argument. `params` become
the arguments written after `Plugin::Method`, so the format has no spelling for
it. Write those three lines as `raw`. The documentation lists it as taking a
parameter, which is what made the corpus the deciding source.

## Nsis7z

7-Zip extraction, 5 corpus scripts.
Source: <https://nsis.sourceforge.io/Nsis7z_plug-in>.

| Method | Arguments | Returns |
| ------ | --------- | ------- |
| `.extract(archive)` | `path` | **nothing** |
| `.extractWithDetails(archive, template)` | `path`, `string` | **nothing** |

**Neither pushes anything at all** — not a status, not an error. Unlike
`nsisunz`, which pushes `"success"` or an error sentence, this plugin is silent,
and an extraction that did nothing looks exactly like one that worked. Check the
result with `fileExists` on something the archive was supposed to contain.

`outputs = []` is what makes `local ok = nsis7z.extract(…)` an error here rather
than a `Pop` that steals somebody else's value.

The archive goes into whatever `setOutPath` last named — there is no destination
argument. `extractWithDetails`'s template has its `%s` replaced with each file
name, which is why it is a `string` rather than a `path`.

Two behaviours that are not arity, so they live here rather than in the
declaration: files whose name does not end in `.7z` are reportedly not
extracted, and `SetOverwrite` is not honoured.

`extractWithCallback` is [not declarable](#what-stays-out).

## Inetc

HTTP and FTP transfer, 46 corpus scripts — the second-most-used third-party
plugin here, and the one with the widest flag surface.
Source: `Contrib/Inetc/inetc.cpp`, checked against
<https://nsis.sourceforge.io/Inetc_plug-in>.

| Method | Arguments | Returns |
| ------ | --------- | ------- |
| `.get(url, file)` | `string`, `path` | status (`string`) |
| `.head(url, file)` | `string`, `path` | status (`string`) |
| `.put(url, file)` | `string`, `path` | status (`string`) |

`"OK"` is success and every other value is an error **sentence** rather than a
code — `"Terminated"`, `"Cancelled"`, a WinInet message with its number spliced
in. So the test is a string comparison and the value is worth printing as it
stands.

`head` requests the headers only and writes the raw response to the file;
`put` uploads the local file to the URL, with the arguments in the same order —
the URL is still first.

### `/END` is not optional here

`inetc.cpp:880` does not count arguments. It reads url/file pairs off the stack
in a loop and stops on `/END`:

```c
while(!popstring(url) && lstrcmpi(url, TEXT("/end")) != 0)
{
    if(popstring(fn) != 0 || lstrcmpi(url, TEXT("/end")) == 0) break;
```

The plugin's own wiki calls `/END` optional, *"required if you stores other vars
in the stack"* — which is a description of every call this compiler emits. A
plugin call sits between `layout`'s caller-saves, so the value under the last
argument is a live register: without the terminator, `inetc` would take it for a
third URL, pop again for its file name, and the restore afterwards would collect
whatever the loop left.

That is why it is [`terminator`](reference-map.md#declaring-a-third-party-plugin-or-header)
in the declaration and not a flag. It is emitted on every call, and a call site
can neither leave it off nor spell it.

### Eighteen flags

All leading, all order-free — `inetc.cpp:1381` loops `while(!popstring(url) &&
*url == TEXT('/'))` and pushes the first non-switch token back — and every
valued one carries its value in a **separate** token.

| Flag | Value | What it does |
| ---- | ----- | ------------ |
| `silent` `weaksecurity` `nocancel` `nocookies` `noproxy` | — | hide the UI; accept a bad certificate; lock Cancel; drop cookies; ignore IE's proxy |
| `caption` `banner` `popup` `canceltext` `question` | `string` | the four progress presentations and the confirm text |
| `proxy` `username` `password` `useragent` `header` | `string` | connection settings; `header` is a raw request header |
| `connecttimeout` `receivetimeout` | `uint` | seconds |
| `resume` | `string` | retry prompt; `""` accepts the default |

**All three entry points take all eighteen**, and the wiki says otherwise — its
`put` synopsis omits `/RESUME`, `/QUESTION` and `/HEADER`, and its `head` entry
is one sentence long. The source settles it the other way: `put`, `head` and
`post` each set a global and then *call `get`*, so there is one flag parser and
one flag set. The three declarations are identical apart from the method name.

```lua
local status = inetc.get(url, PLUGINSDIR .. "/toolchain.zip", {
	caption = "Fetching the toolchain",
	silent = true,
	connecttimeout = 30,
})
-- inetc::get /CONNECTTIMEOUT 30 /SILENT /CAPTION "Fetching the toolchain" … /END
```

Note the emitted order: `Inetc.toml` lists the flags in the order `inetc.cpp`
checks them, and the call site's order is discarded. It has to be — the plugin
accepts them in any order, and two calls naming the same three must not emit two
different lines.

`/TRANSLATE` stays out for the reason it stays out of `NSISdl`: it carries eight
or nine further positional strings. `/TOSTACK` and `/TOSTACKCONV` stay out for a
new one — they push the downloaded body *underneath* the status, so a flag would
be changing `outputs`, and `outputs` is the one thing a declaration is for.

`post` is [not declarable](#what-stays-out).

## nsisFirewall

Firewall exceptions, 1 corpus script.
Source: <https://nsis.sourceforge.io/NsisFirewall_plug-in>.

| Method | Arguments | Returns |
| ------ | --------- | ------- |
| `.addAuthorizedApplication(path, name)` | `path`, `string` | code (`int`) |
| `.removeAuthorizedApplication(path)` | `path` | code (`int`) |

**One corpus script is not "common", and this ships anyway.** It ships on
arity: two methods, fixed positions, one code each, and a wrong count unbalances
the stack with no diagnostic from anywhere. The rule puts the undiscoverable
half first for exactly this case.

**It is not a recommendation.** The plugin drives `INetFwAuthorizedApplications`,
the pre-Vista firewall API, which Windows still honours through a compatibility
shim but which cannot express per-profile rules, direction, or a port. The
maintained alternative is NSIS Simple Firewall (`SimpleFC`, 24 corpus scripts),
which is not declared here only because its methods have not been measured.

The rule name is a label rather than a key — removal goes by path, so two calls
with one path and two names leave one rule, renamed. Removing an application
that was never authorised is not an error, which makes the removal safe to call
unconditionally from an uninstaller.

## nsProcess

Process control, 25 corpus scripts. The original worked example, and the only
one of these read from the plugin's own source.
Source: <https://nsis.sourceforge.io/NsProcess_plugin>.

| Method | Arguments | Returns |
| ------ | --------- | ------- |
| `.findProcess(name)` | `string` | code (`uint`) |
| `.killProcess(name)` | `string` | code (`uint`) |
| `.closeProcess(name)` | `string` | code (`uint`) |

`0` found, `603` not running, and a documented list of failures above 600 —
non-negative throughout, which is what earns `uint`.

`closeProcess` closes the process's windows and waits a few seconds before
terminating it. That is the difference from `killProcess` and the reason to
prefer it.

`_Unload` is deliberately absent: it exists so a script using `/NOUNLOAD` can
release the DLL, and Installua emits no `/NOUNLOAD`.

## AccessControl

ACLs, 111 corpus scripts — the most-used third-party plugin in the corpus, and
all twenty-five of its methods are declared.

**Source: `AccessControl.cpp`.** This is the plugin where the source had to
settle it, twice. The readme and the wiki page are both wrong about the stack in
opposite directions, and either on its own leads to a different — wrong — set of
declarations. Twenty-three of the twenty-five are declarable only because the
format has [`tagged`](#tagged-outputs), and the remaining two were misread here
until the source was read a second time.

### Every method's arity

Twenty-two of them are one shape. The mutators and the object readers route
every diagnosed failure through `ABORT_s`/`ABORT_d`, which push a **description**
and then jump to a cleanup that pushes `"error"` on top of it:

```c
#define ABORT_s(x, y) { showerror_s(TEXT(x), y); goto cleanup; }
...
if (ret) pushstring(TEXT("error"));
```

So `GrantOnFile`, `SetOnFile`, `DenyOnFile`, `RevokeOnFile`, `ClearOnFile`,
`SetFileOwner`, `SetFileGroup`, the inheritance pair, the `Get*Owner` /
`Get*Group` readers and the whole `*OnRegKey` family push **`"ok"` alone, or
`"error"` and a description underneath it** — which is exactly `tagged =
["error"]`, `more = ["string"]`.

| Method | Arguments | Returns |
| ------ | --------- | ------- |
| `.enableFileInheritance(path)` | `path` | `"ok"`, or `"error"` + why |
| `.disableFileInheritance(path)` | `path` | the same |
| `.grantOnFile(path, trustee, permissions)` | `path`, `string`, `string` | the same |
| `.setOnFile(…)` / `.denyOnFile(…)` / `.revokeOnFile(…)` / `.clearOnFile(…)` | the same three | the same |
| `.setFileOwner(path, trustee)` | `path`, `string` | the same |
| `.setFileGroup(path, trustee)` | `path`, `string` | the same |
| `.getFileOwner(path)` | `path` | the **owner**, or `"error"` + why |
| `.getFileGroup(path)` | `path` | the group, or `"error"` + why |
| `.enableRegKeyInheritance(root, key)` | `string`, `string` | `"ok"`, or `"error"` + why |
| `.disableRegKeyInheritance(root, key)` | `string`, `string` | the same |
| `.grantOnRegKey(root, key, trustee, permissions)` | four `string` | the same |
| `.setOnRegKey(…)` / `.denyOnRegKey(…)` / `.revokeOnRegKey(…)` / `.clearOnRegKey(…)` | the same four | the same |
| `.setRegKeyOwner(root, key, trustee)` | three `string` | the same |
| `.setRegKeyGroup(root, key, trustee)` | three `string` | the same |
| `.getRegKeyOwner(root, key)` | `string`, `string` | the owner, or `"error"` + why |
| `.getRegKeyGroup(root, key)` | `string`, `string` | the group, or `"error"` + why |
| `.nameToSid(name)` | `string` | the SID, or `"error"` + why |
| `.sidToName(sid)` | `string` | domain, then name — **always two** |
| `.getCurrentUserName()` | — | user name, **always one** |

Note that `getFileOwner`'s success value is the payload rather than a tag: only
the *failure* side is a fixed string, which is why `tagged` is a list of
first-values to test and never a tag to parse.

The wiki's examples all pop once:

```nsis
AccessControl::SetFileOwner "C:\test.txt" "Waterloo\Mathias"
Pop $0 ; "error" on errors
```

That is a happy-path example, and on the error path it leaks the description
onto the stack. The corpus has the correct idiom, and it is the one Installua
now emits:

```nsis
AccessControl::GrantOnFile "$INSTDIR" "(BU)" "FullAccess"
Pop $R0
${If} $R0 == error
    Pop $R0
${EndIf}
```

### The three SID helpers, read twice

These use no `ABORT` at all — each has a hand-rolled error path — and an
**earlier version of this page got two of them wrong**, in a way worth
recording because it is the same mistake the readme makes. `NameToSid` and
`SidToName` both end with the file's standard trailing line:

```c
  if (ret) pushstring(TEXT("error"));
```

Reading only the body of each function suggests the failure path pushes a
message and stops. It does not: `ret` is still `1` there, so `"error"` goes on
**top** of the message and both come back.

| | Success | Failed lookup | Allocation failure |
| --- | --- | --- | --- |
| `GetCurrentUserName` | 1 (name) | 1 (empty string) | 1 (`"error"`) |
| `NameToSid` | 1 (SID) | **2** (`"error"`, message) | 1 (`"error"`) |
| `SidToName` | **2** (name, domain) | **2** (`"error"`, message) | 1 (`"error"`) |

So `NameToSid` is tagged like the other twenty-two, and `SidToName` is
**uniform at two** — the arity does not vary at all, and the pair is
`(domain, name)` on success and `("error", message)` on failure. The readme
presents it as `Pop $Domain` / `Pop $Username`, which is right about the success
path and silent about the other; both corpus call sites pop twice because both
assume the readme, and both are correct by accident.

`getCurrentUserName` is the genuinely uniform one. It gives the bare account
name without a domain (`GetUserName`, not `GetUserNameEx`) and never checks the
result, so a failed lookup pushes an empty string rather than a sentinel — test
emptiness, not equality.

### The two flags, and the eleven methods that ignore them

`/noinherit` and `/sid` are parsed by `PopFileArgs` and `PopRegKeyArgs`, which
every method but the three SID helpers goes through — so all 22 *accept* both.
They are declared on the 14 that **act** on one:

| Flag | Declared on | Read by |
| ---- | ----------- | ------- |
| `noinherit` | `grant`, `set`, `deny`, `revoke`, `clear` — on file and reg key | `ChangeDACL`, `ClearACL` |
| `sid` | `getFileOwner`, `getFileGroup`, `getRegKeyOwner`, `getRegKeyGroup` | `GetOwner` |

```lua
accessControl.grantOnFile(INSTDIR, "(BU)", "FullAccess", { noinherit = true })
-- AccessControl::GrantOnFile /noinherit $INSTDIR "(BU)" "FullAccess"
```

The other eleven are the interesting half. `setFileOwner` and `setFileGroup`
reach `ChangeOwner`, which reads neither. `enableFileInheritance` and
`disableFileInheritance` are worse than that: `ChangeInheritance` *does* read
`noInherit`, but the dispatcher overwrites it with the enable-or-disable choice
one line before the call, so a `/noinherit` on those two is parsed, stored and
discarded. Declaring it there would have been a flag that compiles, assembles
and does nothing — which is the same class of mistake as a wrong output count,
and caught only by reading `AccessControl.cpp`.

---

## What stays out

Turned down on evidence, and each for a reason about the **format** rather than
about the plugin. All remain callable through `raw`, and any of them can be
declared in five lines of your own `.toml`.

| Plugin | Scripts | Why |
| ------ | ------- | --- |
| `Registry` | 40 | Every corpus use is `${registry::…}`, the `Registry.nsh` macro form, which needs a trailing `${registry::Unload}` — behaviour, not arity. `readReg`, `writeReg` and `deleteRegKey` already cover 38 of the 40. |
| `Inetc.post` | 2 | Its body is popped **before** the flag loop (`inetc.cpp:1369`), so it has to be written ahead of every switch. A `params` entry is emitted after the flags, and there is no spelling for one that comes first. |
| `SimpleFC` | 24 | The maintained successor to `nsisFirewall`. Not measured yet — the one entry here that is a gap rather than a decision. |
| `SimpleSC.getErrorMessage` | 61 | Takes its argument by `Push` — [above](#simplescgeterrormessage-is-not-declarable). |
| `Nsis7z.extractWithCallback` | 5 | Its second argument is a **function address**, and no `params` type spells one — [below](#nsis7zextractwithcallback-takes-an-address-not-a-callback). |
| `LockedList` | 0 | Its surface is a custom **page**, not a call. Declaring only the `Add*` setup calls would ship half a feature. |
| `Crypto` | 0 | Fails the *common* half of the rule outright. |

`nsisunz` is not in this table and is not declared either: it is the worked
example of a plugin you declare yourself, in
[README.md](../README.md#third-party-plugins-and-headers) and in
`tests/declarations.rs`. It stays undeclared so that example stays copy-pasteable.

### `Nsis7z.extractWithCallback` takes an address, not a callback

The obvious reading is that it is an eighth member of the family
`src/lower/callback.rs` covers, turned down for the reason the six
`FileFunc`/`TextFunc` macros give: NSIS hands a callback its arguments in
registers it names, that map is behaviour, and behaviour stays out of a `.toml`.

**`nsis7z.cpp` does not do that.** Its handler pushes both values on the stack
and runs the code segment:

```c
pushint((int)totalSize);
pushint((int)completedSize);
g_pluginExtra->ExecuteCodeSegment(progressCallback-1, 0);
```

So the body pops completed first and total second — which is what the plugin's
own example does, and the only place that order is written down — and pushes
nothing back. There is no sentinel, so no way to cancel the extraction, and no
register protocol to get wrong.

The reason it stays out is a step earlier than the protocol. The plugin does not
take a function; it takes the **address** of one, which the script obtains
separately:

```nsis
GetFunctionAddress $R9 CallbackTest
Nsis7z::ExtractWithCallback "Test.7z" $R9
```

`params` has no type for an address, and `GetFunctionAddress` has no Lua
spelling — it is a `todo` row that the compiler emits only for the nsDialogs
event handlers it generates itself, where the function's address exists in
exactly one place. Declaring this method would mean giving a plugin argument the
address of a user-written function, which is the surface that row exists to
withhold.

Worth recording for whoever revisits it: the body **must** pop exactly two, and
the plugin fires the callback once per progress tick. A body that pops one
leaves an int on the stack on every tick of every extraction.

### `/NOUNLOAD` belongs to nobody

It is the most common flag in the corpus — **99 sites**, two and a half times
the next one — and it is not in any `flags` list here, nor can a project put it
in one.

It was never a plugin's option. `Source/script.cpp:5151` reads it off the front
of *any* plugin call, before the method's own arguments, and passes it to
`EW_REGISTERDLL` as the bit that decides whether the DLL is freed after the
call. So it describes NSIS's loader, not the method — there is no signature for
it to be part of, and a declaration that named it would be claiming the plugin
parses a token the plugin never sees.

It is also **deprecated**, and has been since 2.42 (December 2008): *"Deprecated
/NOUNLOAD and SetPluginsUnload to make scripts simpler and safer"*. The plugin
API that replaced it lets a DLL that must stay resident say so itself. `makensis`
still accepts the token, and warns only when it is written in the wrong place
(`DW_PLUGIN_NOUNLOAD_PLACEMENT`) — the case where a plugin has a `/NOUNLOAD`
parameter of its own is the one that warning exists to catch.

So Installua emits none, offers no spelling for it, and treats unloading as the
compiler's business the way it treats register allocation. The one visible
consequence is [`nsProcess._Unload`](#nsprocess), which exists to release a DLL
that was kept loaded and therefore has nothing to do here.
