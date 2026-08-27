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

**Where the two still disagree, read the source.** That happened once, on
[AccessControl](#accesscontrol), and it was worth the trouble: the readme, the
wiki page and the corpus each imply a *different* set of declarable methods, and
all three are wrong. Documentation describes the path its author was thinking
about, and an arity is a claim about every path.

---

# Plugins that ship with NSIS

Ten, every count read from the plugin's own source under `NSISDIR/Contrib`
rather than from its readme.

## nsExec

| Method | Arguments | Returns |
| ------ | --------- | ------- |
| `.execToStack(command)` | `string` | **exit code, output** |

The exit code comes off first, then the captured output. That is `Pop` order,
and it is the order nothing in the source states — it is the reason
`local rc, out = nsExec.execToStack(…)` is legal at all.

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

Declared anyway, and `StartMenu` is not, on a line worth stating: this arity
varies only on a Windows old enough to lack `InternetAutodial` — older than 98,
or a 95 that never saw IE4 — whereas `StartMenu::Select` pushes one value or two
depending on whether the *user* pressed Cancel, which is an ordinary path on
every run.

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
the one with two declared methods out of twenty-five.

**Source: `AccessControl.cpp`.** This is the one plugin in the third-party set
where the source had to settle it: the readme and the wiki page are both wrong
about the stack, in opposite directions, and either one on its own leads to a
different — wrong — set of declarations.

| Method | Arguments | Returns |
| ------ | --------- | ------- |
| `.getCurrentUserName()` | — | user name |
| `.nameToSid(name)` | `string` | SID |

`getCurrentUserName` gives the bare account name without a domain
(`GetUserName`, not `GetUserNameEx`). Pair the two when a trustee argument needs
a SID. `nameToSid` takes `"Administrator"`, `"Everyone"` or
`"Domain\Administrator"` — the parenthesised `(BU)` and `(S-1-5-32-545)` trustee
spellings are inputs to the methods this file does *not* declare.

**Neither has a usable failure sentinel.** `getCurrentUserName` never checks
`GetUserName`'s result, so a failed lookup pushes an empty string;
`nameToSid` pushes the sentence `Cannot look up name. Error code: N` rather than
`"error"`. Test the `S-1-` prefix, not equality.

### Why only two

The plugin has **two error conventions**, and only one of them keeps the count
fixed.

The mutators and the object readers route every diagnosed failure through
`ABORT_s`/`ABORT_d`, which push a **description** and then jump to a cleanup
that pushes `"error"` on top of it:

```c
#define ABORT_s(x, y) { showerror_s(TEXT(x), y); goto cleanup; }
...
if (ret != 0) pushstring(TEXT("error"));
```

So `GrantOnFile`, `SetOnFile`, `DenyOnFile`, `RevokeOnFile`, `ClearOnFile`,
`SetFileOwner`, the whole `*OnRegKey` family, the inheritance pair, and the
`Get*Owner`/`Get*Group` readers push **one value on success and two on a
diagnosed failure** — and one again on an undiagnosed one, where the allocation
itself failed. That is the **tagged-output** shape: the arity is an outcome
rather than a signature, and `outputs` cannot state it.

The wiki's examples all pop once:

```nsis
AccessControl::SetFileOwner "C:\test.txt" "Waterloo\Mathias"
Pop $0 ; "error" on errors
```

That is a happy-path example, and on the error path it leaks the description
onto the stack. The corpus has the correct idiom:

```nsis
AccessControl::GrantOnFile "$INSTDIR" "(BU)" "FullAccess"
Pop $R0
${If} $R0 == error
    Pop $R0
${EndIf}
```

The three SID helpers use no `ABORT` at all — each has a hand-rolled error path,
and the counts differ per method:

| | Success | Failed lookup | Allocation failure |
| --- | --- | --- | --- |
| `GetCurrentUserName` | 1 (name) | 1 (empty string) | 1 (`"error"`) |
| `NameToSid` | 1 (SID) | 1 (message) | 1 (`"error"`) |
| `SidToName` | **2** (name, domain) | **1** (message) | 1 (`"error"`) |

`SidToName` is the one that looks declarable and is not. On a failed lookup it
writes a message, pushes it, and sets `ret = 0` — so the trailing `"error"`
never fires, and one value comes back where success gives two. The readme
presents it as `Pop $Domain` / `Pop $Username`, which describes the success path
only; both corpus call sites pop twice because both assume the readme.

One more gap that outlives the tagged-output question: every mutator takes an
optional `/NOINHERIT` flag in first position, and the format has no spelling for
a flag — the same absence `Banner` records for `/set` and `nsExec` for
`/TIMEOUT`.

This is the plugin that motivates
[`PLUGINS-PLAN.md` phase 2b](../PLUGINS-PLAN.md): 111 corpus scripts, and 109 of
them call something this file cannot describe.

---

## What stays out

Turned down on evidence, and each for a reason about the **format** rather than
about the plugin. All remain callable through `raw`, and any of them can be
declared in five lines of your own `.toml`.

| Plugin | Scripts | Why |
| ------ | ------- | --- |
| `AccessControl`, all but `.getCurrentUserName` and `.nameToSid` | 111 | Two error conventions, and only the SID helpers keep the count fixed — [above](#why-only-two). `.sidToName` looks declarable and is not. |
| `Registry` | 40 | Every corpus use is `${registry::…}`, the `Registry.nsh` macro form, which needs a trailing `${registry::Unload}` — behaviour, not arity. `readReg`, `writeReg` and `deleteRegKey` already cover 38 of the 40. |
| `SimpleFC` | 24 | The maintained successor to `nsisFirewall`. Not measured yet — the one entry here that is a gap rather than a decision. |
| `SimpleSC.getErrorMessage` | 61 | Takes its argument by `Push` — [above](#simplescgeterrormessage-is-not-declarable). |
| `Nsis7z.extractWithCallback` | 5 | NSIS hands a callback its arguments in registers it names. That map is behaviour and lives in `src/lower/callback.rs`, keyed by macro name — which means a third-party callback cannot be declared at all, here or in a project's own file. The same reason the six `FileFunc`/`TextFunc` callback macros give. |
| `LockedList` | 0 | Its surface is a custom **page**, not a call. Declaring only the `Add*` setup calls would ship half a feature. |
| `Crypto` | 0 | Fails the *common* half of the rule outright. |

`nsisunz` is not in this table and is not declared either: it is the worked
example of a plugin you declare yourself, in
[README.md](../README.md#third-party-plugins-and-headers) and in
`tests/headers.rs`. It stays undeclared so that example stays copy-pasteable.
