---
title: What stays out
description: The plugin methods turned down on evidence, each for a reason about the format rather than the plugin.
---

Turned down on evidence, and each for a reason about the **format** rather than
about the plugin. All remain callable through `raw`, and any of them can be
declared in five lines of your own `.toml` — where a shape the format cannot
spell can often be handed to a `raw` **argument** instead, keeping the rest of
the declaration.

| Plugin | Scripts | Why |
| ------ | ------- | --- |
| `Registry` | 40 | Every corpus use is `${registry::…}`, the `Registry.nsh` macro form, which needs a trailing `${registry::Unload}` — behaviour, not arity. `readReg`, `writeReg` and `deleteRegKey` already cover 38 of the 40. |
| `nsJSON` | 3 | Its node path is a **variable number of positional strings** — one to four across the corpus — and `params` is a fixed list. [Below](/reference/plugins/what-stays-out/#nsjsons-blocker-is-its-path-not-its-flags). |
| `Inetc.post` | 2 | Its body is popped **before** the flag loop (`inetc.cpp:1369`), so it has to be written ahead of every switch. A `params` entry is emitted after the flags, and there is no spelling for one that comes first. |
| `SimpleSC.getErrorMessage` | 61 | Takes its argument by `Push` — [above](/reference/plugins/third-party-plugins/#simplescgeterrormessage-is-not-declarable). |
| `Nsis7z.extractWithCallback` | 5 | Its second argument is a **function address**, and no `params` type spells one — [below](/reference/plugins/what-stays-out/#nsis7zextractwithcallback-takes-an-address-not-a-callback). |
| `LockedList` | 0 | Its surface is a custom **page**, not a call. Declaring only the `Add*` setup calls would ship half a feature. |
| `nsisunz.unzipToStack` | 2 | Pushes **one value per file in the archive**, so the arity is `1 + n` for an `n` nobody knows at compile time. [Below](/reference/plugins/what-stays-out/#nsisunz-is-the-worked-example-and-its-third-method-could-not-have-shipped). |
| `Crypto` | 0 | Fails the *common* half of the rule outright. |

`nsisunz`'s other two methods are declarable and stay undeclared anyway: the
plugin is the worked example of one you declare yourself, in
[Declaring a third-party plugin or header](/reference/commands/plugins-and-headers/#declaring-a-third-party-plugin-or-header) and in
`tests/declarations.rs`. [Below](/reference/plugins/what-stays-out/#nsisunz-is-the-worked-example-and-its-third-method-could-not-have-shipped).

### nsisunz is the worked example, and its third method could not have shipped

ZIP extraction, 21 corpus scripts. Source:
`nsisunz.cpp`, in the plugin's own distribution.

Three exports over one function, and the count is what decides which:

```cpp
void Unzip(…)        { internal_unzip(0); }
void UnzipToLog(…)   { internal_unzip(1); }
void UnzipToStack(…) { internal_unzip(2); }
```

| Method | Corpus files | Arguments | Returns |
| ------ | ------------ | --------- | ------- |
| `.unzip(zip, dest)` | 1 | `path`, `path` | `string` |
| `.unzipToLog(zip, dest)` | 18 | `path`, `path` | `string` |
| `.unzipToStack(zip, dest)` | 2 | `path`, `path` | **`1 + n`** |

The first two differ only in whether each extracted name is written to the
details log, and both push exactly one value: `"success"` (`szSuccess`,
`nsisunz.cpp:27`) or one of eight error *sentences* — `"Error opening ZIP file"`,
`"File not found in archive"`, `"aborted"` and five more. A sentence rather than
a code, so the comparison is `~= "success"` and the value is worth printing as
it stands, exactly as with [`Inetc`](/reference/plugins/third-party-plugins/#inetc).

**`unzipToStack` is the one that cannot be declared.** Mode 2 pushes the names
as it walks the archive (`nsisunz.cpp:418`):

```cpp
} else if (uselog == 2) {
    if (!first) {
        pushstring("");   // push list terminator (empty string)
        first++;
    }
    pushstring(pfn);
}
```

An empty string first, then one push per file, then the status last — so the
Lua arity is `1 + n` for an `n` that is a property of the *archive*, not of the
program. That is neither a number nor a `tagged`/`more` fork: `tagged` says
*which* of two fixed shapes, and this has no fixed shape. It is
[`nsJSON`'s problem](/reference/plugins/what-stays-out/#nsjsons-blocker-is-its-path-not-its-flags) on the output
side, and the format has no spelling for either.

**The flags are otherwise a textbook case** — a leading loop, order-free, one
bare switch and two valued ones (`nsisunz.cpp:300`):

```cpp
popstring(buf);
while (buf[0] == '/') {
    if (!lstrcmpi(buf+1, "text"))          popstring(g_extract_text), hastext++;
    if (!lstrcmpi(buf+1, "noextractpath")) noextractpath++;
    if (!lstrcmpi(buf+1, "file"))          popstring(filetoextract), usefile++;
    if (popstring(buf)) *buf = 0;
}
```

So `unzipToLog` would declare in a dozen lines and work. It stays out to keep
the worked example honest, which is a decision about the documentation rather
than about the plugin — and the only entry in this file that is.

**Why `unzipToLog` and not `unzip`.** The example named `unzip` for two
releases, and a per-method count was never run: `unzip` appears in **one** of
the 984 scripts, `unzipToLog` in eighteen. A worked example is a page a reader
copies whole, so the method it names is the method they get, and pointing it at
the rare one taught the format correctly and the plugin wrongly.

### nsJSON's blocker is its path, not its flags

The recorded reason was the repeated, ordered flag run its readme advertises:

```nsis
nsJSON::Get /index 0 /index 1 /index 3 /index 0 /end
```

A table has unique keys and no order, so that shape genuinely has no encoding
here. But **no corpus script writes it** — 25 call sites across three files, and
the repeated-`/index` form appears in none of them. It is a documentation
example, not a usage.

What the sites do write is a node path of **positional strings**, and its length
is the problem:

```nsis
nsJSON::Get "tag_name" /end
nsJSON::Get /tree Manifest "builds" "windows" "${ARCH}" "url" /end
nsJSON::Set /tree metrics "Data" "products" "Wii U USB Helper" /value `"$v"` /end
```

One segment to four, chosen per call. `params` is a fixed list, so a declaration
would have to pick a depth and be wrong at every other one — and being wrong
here is not a type error, it is a `Pop` count.

Two of the 25 also interleave — `nsJSON::Get "assets" /index 0 "size" /end`
puts a flag *between* two positionals, in an order the caller chose. That is the
shape flags cannot carry, and it is a path query wearing flag syntax rather than
a flag list.

Two further facts for whoever revisits this. `/tree`, `/value`, `/file` and
`/http` are ordinary valued flags, but `/value` and `/file` come **after** the
path rather than before it — so the leading-only rule does not describe this
plugin either. And `/end` is mandatory for the reason it is mandatory on
`Inetc`: the readme says outright that it *"must be added to the end of the list
to prevent stack corruption"*, and this compiler's caller-saves are exactly the
stack it would corrupt.

A repetition spelling would not unlock this plugin. A variadic `params` tail
would unlock 23 of the 25 sites, and is the thing to design if nsJSON is ever
wanted.

Until then it is `raw` — and this is the plugin the **raw argument** form was
built for. Declare `get` with one `string` parameter in your own `.toml` and
spell the whole path through it:

```lua
local node = nsJSON.get(raw "/index 0 /index 1 /index 3", "$Doc")
```

The path becomes text nothing checks, which is the part a variadic `params`
would fix. What it buys in the meantime is the rest of the declaration: the call
is still one line the compiler writes, `outputs` still says how many `Pop`s
follow, and `local node` still binds one — none of which survives writing the
same call as a `raw` block.

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
consequence is [`nsProcess._Unload`](/reference/plugins/third-party-plugins/#nsprocess), which exists to release a DLL
that was kept loaded and therefore has nothing to do here.
