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
