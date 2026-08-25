# Installua

A Lua-shaped language that compiles to NSIS.

You write an installer as an ordinary Lua program — real syntax, real scoping,
real editor support — and Installua compiles it to a `.nsi` script and hands
that to `makensis`. The parts of NSIS that are famously easy to get wrong are
not yours to get wrong: MUI2's include order, `un.` prefixes, label
arithmetic, the register stack, `$PLUGINSDIR`. The compiler writes those.

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

## Requirements

- **NSIS 3.x** on your `PATH` (`makensis -VERSION`). Installua targets 3.12.
- **Rust 1.97** to build the compiler.

## Install

Not published yet — build it from this repository:

```sh
git clone <this repo> && cd installua
cargo install --path .
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
| `installua stubs` | regenerate the editor meta files. Run it after adding a `func` or a global. |

`installua --help` lists the rest.

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
cargo install selene --features selene-lib/lua54
```

## Third-party plugins and headers

Installua ships declarations for a handful of plugin methods and header macros —
`nsExec::ExecToStack`, `UserInfo::GetAccountType`, `System::Call`, `${GetSize}`,
`${DriveSpace}`, `${VersionCompare}`, and third-party `nsProcess` — and your
installer will reach past them almost immediately. Anything else is **declared
by your project**, in one small file per plugin or header:

```toml
# .installua/headers/nsis7z.toml
[[plugin]]
name = "Nsis7z"                       # what `plugin "…"` is given
method = "extractWithDetails"         # what you call it
nsis = "Nsis7z::ExtractWithDetails"   # what NSIS is given
params = ["path", "string"]
outputs = ["string"]                  # values pushed, in `Pop` order
```

```lua
local sevenZip = plugin "Nsis7z"

section("Core", function()
	local details = sevenZip.extractWithDetails("data/payload.7z", "")
	detailPrint(details)
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
against it and against nothing else.

A header is the easier half. `import "AnyHeader"` emits the `!include` whatever
the name is, so a header you only reach through `raw` needs no declaration at
all — you only need one to call its macros as `header.macro(…)`.

The full format, the type vocabulary and the rules for redeclaring a builtin are
in [docs/reference-map.md](docs/reference-map.md#declaring-a-third-party-plugin-or-header).
What ships declared is written in that same format, in
[src/headers/](src/headers/) — a plugin worth having here is a pull request
holding one `.toml` file and no Rust.

Two things are still yours where a plugin is concerned: `!addplugindir` for a
DLL outside NSIS's own `Plugins/` tree, written through `raw`, and `raw` itself
for anything with no declaration yet.

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
