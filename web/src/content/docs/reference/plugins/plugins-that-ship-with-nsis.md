---
title: Plugins that ship with NSIS
description: The eleven plugins under NSISDIR/Contrib, every count read from their own source.
---

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
IE4. It is not the [tagged-output](/reference/plugins/tagged-outputs/) shape, which varies on an
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
[tagged-output](/reference/plugins/tagged-outputs/) shape with the *unusual* polarity.

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
That is the same hazard [tagged outputs](/reference/plugins/tagged-outputs/#the-hazard-this-cannot-cover)
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
