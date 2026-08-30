# Installua

A Lua-shaped language that compiles to NSIS.

```lua
attributes {
	name = "Example",
	outFile = "Example-setup.exe",
	compressor = "lzma",
}

installer {
	installDir = PROGRAMFILES64 .. "/Example",

	page.welcome {},
	page.directory {},
	page.instFiles {},
	page.finish {},

	section("Core", function()
		setOutPath(INSTDIR)
		file("README.txt")
		writeUninstaller(INSTDIR .. "/uninstall.exe")
	end),
}
```

Write an installer as an ordinary Lua program — real syntax, real scoping,
real editor support — and Installua compiles it to a `.nsi` script and hands
that to `makensis`. The parts of NSIS that are famously easy to get wrong are
not yours to get wrong: MUI2's include order, `un.` prefixes, label
arithmetic, the register stack, `$PLUGINSDIR`. The compiler has you covered!

## Requirements

- **NSIS 3.x** on your `PATH` (`makensis -VERSION`). Installua targets 3.12.
- **[mise](https://mise.jdx.dev)** and **[rustup](https://rustup.rs)**. The Rust
  version is pinned in `rust-toolchain.toml` (1.98) and `mise run install`
  installs it; you never name it yourself.

## Install

Not published yet — build it from this repository:

```sh
git clone <this repo> && cd installua
mise run install        # pinned toolchain, then `cargo install --path .`
```

## Getting started

```sh
mkdir my-installer && cd my-installer
installua init .        # installua.toml, .luarc.json, selene.toml
installua stubs         # .installua/meta/*.lua — the editor's half
```

Write `install.lua` (the block above is a complete one), then:

```sh
installua build install.lua
```

That compiles to `install.nsi` and runs `makensis -WX` on it, so a warning from
NSIS is a failed build. When something is wrong you get the Lua line, not the
generated one:

```
install.lua:3:15: warning[dollar-in-literal]: `$INSTDIR` is emitted as literal text here
  note: did you mean `.. INSTDIR ..`? a string literal is data, never a template:
        every `$` is emitted as `$$`
```

### The commands you will use

| Command | What it does |
| --- | --- |
| `installua build <file.lua>` | compile, then run `makensis -WX`. Exits non-zero on any error. |
| `installua check <file.lua>...` | everything `build` would say, writing nothing and running no `makensis` — the fast gate for CI and editors |
| `installua emit <file.lua>` | compile to `.nsi` and stop — for wiring into an existing build |
| `-D NAME=VALUE` | on any of the three: set a build parameter the source declared with `param(…)`. `--param NAME=VALUE` is the same flag spelled out. |
| `installua stubs` | regenerate the editor meta files. Run it after adding a `func` or a global. |

`installua --help` lists the rest.

### Build parameters

The version, the channel, the feature flag — the values a CI job sets rather
than the source:

```lua
local VERSION <const> = param("VERSION", "1.4.2")
local SIGNED  <const> = param("SIGNED", false)
```

```sh
installua build install.lua -D VERSION=2.0.0 -D SIGNED=true
```

It replaces NSIS's `!ifndef VERSION` / `!define VERSION "1.4.2"` / `!endif`, and
fixes that idiom's failure mode: because the parameter is *declared*, a misspelt
`-D VERSOIN=2.0.0` is an error naming the parameters that do exist, instead of a
build that quietly ships the default. The default is also the type — `-D
PORT=abc` against `param("PORT", 8080)` is rejected rather than handed to an
`IntOp` as a string.

A top-level `if` over those values is the other half, and it replaces `!if` /
`!ifdef` / `!else` / `!endif`:

```lua
local ARCH <const> = param("ARCH", "x86")

if ARCH == "x64" then
	installer { section("Core", function() file("bin/x64/app.exe") end) }
else
	installer { section("Core", function() file("bin/x86/app.exe") end) }
end
```

The condition has to fold at build time — that is the same rule `<const>` lives
under — and what it selects is ordinary top-level declarations, resolved
order-free with everything around them. Nothing of the conditional reaches the
output: it is a branch the compiler takes rather than a directive it emits, so
the script is the one you would get by writing only the branch that won.

### Editor support

`installua init` writes `.luarc.json` and `selene.toml`, and `installua stubs`
writes the LuaCATS definitions they point at. With
[lua-language-server](https://luals.github.io) installed you get completion,
hover, go-to-definition and arity checking on the whole API — including inside
string literals, where most of an installer's interesting values live.

The generated [selene](https://kampfkarren.github.io/selene/) config is the
other half: the Lua names Installua rejects are lint errors that each name
their replacement, rather than names that silently do nothing.

selene has to be built for Lua 5.4, or it cannot parse the `<const>` a
build-time constant is declared with — the published binary is not:

```sh
mise run install:selene
```

In VS Code, `installua init --vscode` writes the workspace half of that:

```sh
installua init . --vscode   # + .vscode/extensions.json, .vscode/tasks.json
```

`extensions.json` recommends [sumneko.lua][sumneko] — the extension that reads
`.luarc.json` and the generated stubs — and is the one file `init` will add to
rather than skip when it already exists, since a recommendation takes nothing
away. `tasks.json` binds ⇧⌘B / Ctrl+Shift+B to `installua build` on the current
file and carries a problem matcher for the compiler's diagnostics, so an error
lands on the Lua line in the editor instead of in the terminal only.

There is no `launch.json`: launching needs a debug adapter, and neither
Installua nor NSIS has one.

[sumneko]: https://marketplace.visualstudio.com/items?itemName=sumneko.lua

## Third-party plugins and headers

Installua ships declarations for the plugin methods and header macros a real
installer reaches for first — `nsExec::ExecToStack`, `UserInfo::GetAccountType`,
`System::Call`, `${GetSize}`, `${DriveSpace}`, `${VersionCompare}`, and eight
third-party plugins picked on a scan of 984 real-world scripts: `EnVar`,
`SimpleSC`, `AccessControl`, `Inetc`, `nsProcess`, `Nsis7z`, `nsisFirewall` and
`SimpleFC` ([the full list](docs/plugin-reference.md)). A declaration is
not a bundled DLL — installing the plugin is still yours — and your installer
will reach past the set almost immediately. Anything else is **declared by your
project**, in one small file per plugin or header:

```toml
# .installua/declarations/nsisunz.toml
[[plugin]]
name = "nsisunz"                      # what `plugin "…"` is given
method = "unzipToLog"                 # what you call it
nsis = "nsisunz::UnzipToLog"          # what NSIS is given
params = ["path", "path"]
outputs = ["string"]                  # values pushed, in `Pop` order
dir = "vendor/plugins"                # only if the DLL is not in NSISDIR
```

```lua
local nsisunz = plugin "nsisunz"

section("Core", function()
	local result = nsisunz.unzipToLog("data/payload.zip", INSTDIR)
	if result ~= "success" then
		detailPrint(result)
	end
end)
```

That is the whole of it. The declaration is read by the compiler on every build,
so the call is arity-checked and emitted like any other; run `installua stubs`
and the editor gets it too, with completion, hover and the same argument
checking it gives `detailPrint`. A method nobody declared is an error naming the
directory that would declare it, rather than a stack imbalance NSIS finds no
fault with.

**Why a declaration and not a scan.** NSIS offers no way to ask a DLL how many
values it pushes, and an `!insertmacro` parameter list carries no directions —
`${StrCase} $0 "text" "L"` puts its destination first and
`${GetSize} "$dir" "" $0 $1 $2` puts it last. There is nothing to discover and
nothing to infer, so the one thing that cannot be guessed is the one thing you
write down. `outputs` is the load-bearing line: `local rc, out = …` is checked
against it and against nothing else. `dir` is the one field that is about a file
rather than a signature — a DLL vendored into your repository instead of
installed into `NSISDIR` — and the compiler turns it into an `!addplugindir` in
the one position that directive is correct in, which is not a position you could
write it in yourself.

A header is the easier half. `import "AnyHeader"` emits the `!include` whatever
the name is, so a header you only reach through `raw` needs no declaration at
all — you only need one to call its macros as `header.macro(…)`.

The full format, the type vocabulary and the rules for redeclaring a builtin are
in [docs/reference-map.md](docs/reference-map.md#declaring-a-third-party-plugin-or-header).
What ships declared is written in that same format, in
[src/declarations/](src/declarations/) — a plugin worth having here is a pull request
holding one `.toml` file and no Rust.

What is still yours is `raw` — for a plugin with no declaration yet, and for the
handful of NSIS lines no construct replaces. Inside a body `raw [[ … ]]` lands
where it is written; at the top level it takes an anchor, because the emitter's
slots are fixed and declaration order is not emission order:

```lua
raw.head [[ !system 'git rev-parse --short HEAD > rev.txt' ]]
raw.tail [[ !packhdr "tmp.dat" '"upx.exe" "tmp.dat"' ]]
```

`raw` also works as an *argument* of a declared plugin method, for the argument
shape a `params` list cannot describe — a path whose length the caller picks,
say. The text is spliced into that call's line, and the call stays declared, so
its outputs still bind:

```lua
local node = nsJSON.get(raw "/index 0 /index 1 /index 3", "$Doc")
```

## Documentation

| | |
| --- | --- |
| [docs/reference-map.md](docs/reference-map.md) | **Start here.** Every NSIS command and what to write instead, grouped by what you are trying to do. |
| [docs/mui-reference.md](docs/mui-reference.md) | The eight MUI2 pages and every setting on them. |
| [docs/nsis-shaped-not-nsis.md](docs/nsis-shaped-not-nsis.md) | If you know NSIS: the habits that do not carry over. |
| [docs/lua-shaped-not-lua.md](docs/lua-shaped-not-lua.md) | If you know Lua: what this language does not have, and why. |
| [examples/](examples/) | Five complete programs, each with the `.nsi` it must produce. |

The reference answers *how do I write this*; `installua coverage` answers *is
this done yet*, one line per bucket over all 276 NSIS commands.

## Status

Pre-1.0, and the coverage question is answered: all 276 commands `makensis
-CMDHELP` prints and all 255 MUI2 names are classified, with nothing pending in
either census. Run `installua coverage` to see the counts.

What that does **not** mean is that every command has a Lua spelling — a good
number are deliberately rejected, or written by the compiler on your behalf.
Both categories are listed with their reasons in the reference.
