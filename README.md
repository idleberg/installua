# Installua

![Crates.io License](https://img.shields.io/crates/l/installua?style=for-the-badge)
[![Crates.io Version](https://img.shields.io/crates/v/installua?style=for-the-badge)](https://crates.io/crates/installua)
[![CI](https://img.shields.io/github/actions/workflow/status/idleberg/installua/ci.yml?style=for-the-badge)](https://github.com/idleberg/installua/actions)

A Lua-shaped language that compiles to NSIS.

**Example**

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

Write an installer as an ordinary Lua program, and Installua compiles it to a
`.nsi` script and hands that to `makensis`.

## Features

- Write functions without managing a stack
- No ordering rules
- MUI2 by default, without the footguns
- Fully typed API, including plugins
- Build-time parameters and `if`
- Editor support out of the box
- `makensis -WX` by default

## Prerequisites

**NSIS 3.x** is installed and on your `PATH` – Installua targets version 3.12.

## Install

### Cargo

```sh
cargo install installua
```

### Homebrew

```sh
brew install idleberg/asahi/installua
```

## Getting started

```sh
mkdir my-app && cd my-app
installua init .        # installua.toml, .luarc.json, selene.toml
installua stubs         # .installua/meta/*.lua — the editor's half
```

Write `setup.lua`, then build the installer:

```sh
installua build setup.lua
```

That compiles to `setup.nsi` and runs `makensis -WX` on it, so a warning from
NSIS is a failed build.

### The commands you will use

| Command                         | What it does                                                                                               |
| ------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| `installua build <file.lua>`    | compile, then run `makensis -WX`. Exits non-zero on any error.                                             |
| `installua check <file.lua>...` | everything `build` would say, writing nothing and running no `makensis` — the fast gate for CI and editors |
| `installua emit <file.lua>`     | compile to `.nsi` and stop — for wiring into an existing build                                             |
| `installua stubs`               | regenerate the editor meta files. Run it after adding a `func` or a global.                                |

`installua --help` lists the rest.

### Editor support

`installua init` writes `.luarc.json` and `selene.toml`, and `installua stubs`
writes the LuaCATS definitions they point at. With
[lua-language-server](https://luals.github.io) installed you get completion,
hover, go-to-definition and arity checking on the whole API — including your own
plugin declarations. The generated
[selene](https://kampfkarren.github.io/selene/) config is the other half: the
Lua names Installua rejects are lint errors naming their replacement.

selene has to be built for Lua 5.4, or it cannot parse the `<const>` a
build-time constant is declared with — the published binary is not:

```sh
mise run install:selene
```

## Documentation

|                                                              |                                                                                                             |
| ------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------- |
| [docs/reference-map.md](docs/reference-map.md)               | **Start here.** Every NSIS command and what to write instead, grouped by what you are trying to do.         |
| [docs/mui-reference.md](docs/mui-reference.md)               | The eight MUI2 pages and every setting on them.                                                             |
| [docs/plugin-reference.md](docs/plugin-reference.md)         | Every plugin method that ships declared, where its output count came from, and what is deliberately absent. |
| [docs/header-reference.md](docs/header-reference.md)         | The same for `FileFunc`, `TextFunc` and `WordFunc` macros.                                                  |
| [docs/nsis-shaped-not-nsis.md](docs/nsis-shaped-not-nsis.md) | If you know NSIS: the habits that do not carry over.                                                        |
| [docs/lua-shaped-not-lua.md](docs/lua-shaped-not-lua.md)     | If you know Lua: what this language does not have, and why.                                                 |
| [examples/](examples/)                                       | Five complete programs, each with the `.nsi` it must produce.                                               |

Some starting points in the reference: [build parameters and build-time
`if`](docs/reference-map.md#param) for the values CI sets rather than the
source, and [declaring a third-party plugin or
header](docs/reference-map.md#declaring-a-third-party-plugin-or-header) for the
plugins beyond the ones that ship declared — a `.toml` file per plugin, since
NSIS offers no way to ask a DLL how many values it pushes.

The reference answers _how do I write this_; `installua coverage` answers _is
this done yet_, one line per bucket over all 276 NSIS commands.

## Status

Pre-1.0, and the coverage question is answered: all 276 commands `makensis
-CMDHELP` prints and all 255 MUI2 names are classified, with nothing pending in
either census. Run `installua coverage` to see the counts.

What that does **not** mean is that every command has a Lua spelling — a good
number are deliberately rejected, or written by the compiler on your behalf.
Both categories are listed with their reasons in the reference.
