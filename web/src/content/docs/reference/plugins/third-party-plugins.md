---
title: Third-party plugins
description: The plugins declared from their own source and a corpus of real scripts, and the methods left out.
---

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

`extractWithCallback` is [not declarable](/reference/plugins/what-stays-out/).

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

That is why it is [`terminator`](/reference/commands/plugins-and-headers/#declaring-a-third-party-plugin-or-header)
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

`post` is [not declarable](/reference/plugins/what-stays-out/).

## NScurl

libcurl in a plugin — HTTP/HTTPS/FTP with TLS, HTTP/2 and HTTP/3, a background
queue, and three hash helpers.
Source: <https://github.com/negrutiu/nsis-nscurl>, `src/nscurl/main.c` for the
exports and `curl.c` / `gui.c` for the parameter loops.

Not in the corpus — it postdates it — and declared on the first half of the
rule: eleven exports, each with its own push count, and no two alike.

| Method | Arguments | Returns |
| ------ | --------- | ------- |
| `.http(method, url, file)` | `string`, `string`, `path` | status (`string`) |
| `.wait()` | — | — |
| `.query(keywords)` | `string` | expansion (`string`) |
| `.cancel()` | — | — |
| `.md5(file)` `.sha1(file)` `.sha256(file)` | `path` | hex digest (`string`) |
| `.escape(s)` `.unescape(s)` | `string` | percent-coded (`string`) |

`http` pushes exactly one value on every path — `GuiWait`'s result when the
request was queued, a formatted Win32 message when a parameter was rejected
(`main.c:290`). By default that value is `"OK"` or an error sentence, and the
`returns` flag replaces it with any `@KEYWORD@` expansion, so the comparison a
call site writes depends on the flag it set.

`wait` and `cancel` push **nothing**. `wait` computes the same string `http`
would push and drops it (`main.c:388`), which is why `/RETURN` is not declared
there — it shapes a value nobody can read.

The third argument to `http` is the output file, or the literal `"Memory"` to
keep the body in the queue for `query` to read back.

### The flags come *after* the arguments here

`CurlParseRequestParam` (`curl.c:369`) takes parameter 0 for the HTTP method, 1
for the URL and 2 for the output path — **by index, not by shape** — and only
starts matching `/SWITCH` tokens from the fourth. A leading `/SILENT` is
therefore not a flag: it is the verb, and the request goes out asking a server
for `SILENT`.

That is the whole of `trailing = true` in
[`NScurl.toml`](/reference/commands/plugins-and-headers/#declaring-a-third-party-plugin-or-header). The
call site is unchanged — the table is still written last and still unordered —
and only the run of flags lands on the other side of the arguments.

`/END` is [not optional](#end-is-not-optional-here) for the same reason it is
not optional on `inetc`: `main.c:238` reads parameters in a loop until it pops
it, and what lies under the last argument is `layout`'s caller-saves.

### Thirty-four flags on `http`, twelve on `wait`

The first block below is the request (`curl.c`), the second the progress UI
(`gui.c`) — `http` takes both, `wait` and `cancel` only what their own loops
match.

| Flag | Value | What it does |
| ---- | ----- | ------------ |
| `resume` `insist` `noredirect` `encoding` `markoftheweb` | — | continue a partial file; retry on failure; don't follow 3xx; accept compression; write the Zone.Identifier stream |
| `http11` `http3` | — | pin the protocol version; the last one written wins |
| `header` `useragent` `referer` `data` | `string` | request headers (`\r\n` separates several), agent, referrer, request body |
| `connecttimeout` `completetimeout` | `string` | a duration with a unit — `"30s"`, `"2m"` — or bare milliseconds |
| `speedcap` `depend` | `uint` | bytes/second ceiling; queue id this one waits on |
| `security` `castore` | `string` | `"weak"`/`"strong"`; `"true"`/`"false"` for the Windows CA store |
| `cacert` | `path` | a bundle, or `"builtin"` or `"none"` |
| `cert` | `string` | a pinned SHA-1 fingerprint or a PEM blob |
| `proxy` `doh` | `string` | proxy URL; DNS-over-HTTPS resolver |
| `cookiejar` `debug` | `path` | cookie file; trace log |
| `tag` | `string` | names this request, for `wait`, `query` and `cancel` |
| `returns` | `string` | the `@KEYWORD@` string `http` pushes instead of the status |
| `background` `page` `popup` `silent` `cancel` | — | return immediately; and the four presentations |
| `titlewnd` `textwnd` `progresswnd` `cancelwnd` | `handle` | drive controls of your own — the handles a `page.custom` hands back |
| `id` (`wait`, `query`, `cancel`) | `uint` | select one queued request |
| `remove` (`cancel`) | — | abort *and* drop it from the queue |

```lua
local status = nscurl.http("GET", url, PLUGINSDIR .. "/tool.zip", {
	connecttimeout = "30s",
	tag = "toolchain",
	silent = true,
})
-- NScurl::http "GET" "…" "$PLUGINSDIR\tool.zip" /CONNECTTIMEOUT "30s" /TAG "toolchain" /SILENT /END
```

### What stays out, and why it is the same reason twice

`/POST`, `/AUTH`, `/PROXYAUTH`, `/TLSAUTH`, `/LOWSPEEDLIMIT` and `/STRING` all
pop **two or more** values after the switch — `/TLSAUTH user pass`,
`/LOWSPEEDLIMIT bps duration`, `/POST [filename=…] [type=…] name data` — and a
`separate` flag carries exactly one. Declaring one would emit the switch and a
single token, and the plugin would take the *next* argument as its second value.
There is no encoding for a multi-value flag, so there is no half-right version
of these to ship.

`echo` is out for the shape it is: a debugging export that pops however many
strings you pushed. `enumerate` is out for the arity — it pushes one queue id
per matching request under an empty-string sentinel, and `tagged` decides
between two fixed counts, not between *n* of them.

Two single-value flags carry a hazard worth naming, because the plugin resolves
it at runtime and the compiler cannot. `/DATA` and `/DEBUG` each peek at the
token they pop and pop **again** if it was a keyword — `-file`, `-string`,
`-memory`, `nodata`. A call site passing one of those words as the value would
unbalance the stack. The keyword forms are not reachable from here, and passing
a keyword by accident is the one way to reach them.

The hash helpers take `path` rather than `string` for the same class of reason.
`IDataParseParam` (`utils.c:1008`) guesses between a file and a literal by
asking whether the path *exists*, and a `$PLUGINSDIR/tool.zip` written the way
every other path in a source is written does not exist under that spelling — it
would silently hash the text. Declaring the position a `path` normalises the
separators first. Hashing a literal string with a `/` in it is what that costs.

## SimpleFC

NSIS Simple Firewall, 24 corpus scripts, and the **only** firewall plugin
declared here — see [`nsisFirewall`](/reference/commands/not-available/#third-party-plugins-that-stay-out)
for the one that was dropped and why. It drives `INetFwPolicy2`, so it reaches
per-profile rules, direction, ports and ICMP types that the older plugin cannot
express, and it ships an ANSI **and** a Unicode build, both at 1.21, both named
`SimpleFC.dll`. Source: `Source/SimpleFC.dpr`, and it is the first of these read from
**Delphi** rather than C++ — the idiom is `PopString` / `PushString` from
`nsis.pas`, but the counting is the same.

All 33 exported methods are declared. Every one of them is straight-line: not a
single `PushString` in the file sits inside a branch, which is why a plugin this
wide needs no [`tagged`](/reference/plugins/tagged-outputs/) entry at all.

### The status is `0`, and it comes off first

The plugin's own `ResultToStr` is the whole vocabulary:

```pascal
function ResultToStr(Value: Boolean): String;
begin
  if Value then result := '0' else result := '1';
end;
```

**`0` is success and `1` is failure** — the opposite of the C convention and the
same as `SimpleSC`'s. The readers push their answer first and the status last,
so the status pops first and the answers follow it:

```pascal
PushString(BoolToStr(Allowed));
PushString(BoolToStr(Restricted));
PushString(FirewallResult);      // popped first
```

That gives `isIcmpTypeAllowed` three outputs — status, allowed, restricted — and
it is the widest fixed arity in any declaration here.

### Three places the readme is wrong

Measured by diffing every synopsis and every worked example in `Readme.txt`
against the pops and pushes in `SimpleFC.dpr`. Three disagreements, all in the
same direction — the prose under-counts:

| Method | Readme says | The DLL does |
| ------ | ----------- | ------------ |
| `enableDisableNotifications` | one `Pop` in one example, **none** in the other | pushes **two**: the status, and an echo of the argument beneath it |
| `enableDisablePort` | synopsis `[port] [protocol]` | pops **three** — the synopsis omits the `[status]` its own examples pass |
| `enableDisableApplication` | synopsis `[path]` | pops **two**, same omission |

The first is the costly one: both of the readme's own examples leak a value, so
a script that follows them has a stack one item deep for the rest of the
section. The second push looks like a copy-paste of the reader's — every other
setter (`enableDisableFirewall`, `startStopFirewallService`,
`allowDisallowExceptionsNotAllowed`) pushes one — but what the DLL does is the
arity, whatever the author meant.

This is the fourth plugin where the source and the documentation disagree, and
the fourth time the source won. The rule stated under
[`AccessControl`](#accesscontrol) needs no restating: **documentation describes
the path its author was thinking about, and an arity is a claim about every
path.**

### Arguments are codes, not names

Most parameters are `int` because the plugin takes Windows' own enumerations
raw: protocol is `6` for TCP and `17` for UDP, scope is `0` for all networks,
IP version, profile, direction and action likewise. The readme's *Parameters*
list is the mapping and is correct there. `advAddRule` takes fifteen of them in
one call — the longest `params` in any declaration, and the one place where
reading the emitted line back is genuinely easier than reading the Lua.

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
format has [`tagged`](/reference/plugins/tagged-outputs/), and the remaining two were misread here
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
