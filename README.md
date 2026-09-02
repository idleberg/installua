# Installua

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

Write an installer as an ordinary Lua program — real syntax, real scoping,
real editor support — and Installua compiles it to a `.nsi` script and hands
that to `makensis`. The parts of NSIS that are famously easy to get wrong are
not yours to get wrong: MUI2's include order, `un.` prefixes, label
arithmetic, the register stack, `$PLUGINSDIR`. The compiler has you covered!

## Features

- **Real variables, real scope.** Locals are locals; the compiler allocates
  `$0`–`$9`/`$R0`–`$R9` and spills to the stack when it runs out. You never name
  a register or count a `Push`/`Pop` pair again.
- **Order-free.** Every name resolves before any body is lowered, so a function
  can call one declared later in the file.
- **MUI2 without the footguns.** Page order, include order, `un.` prefixes and
  the `!define`s that must precede `MUI2.nsh` are the compiler's problem.
  All eight pages and 255 MUI2 names are classified.
- **Typed plugin calls.** Plugin and header macros ship declared with their real
  output counts, taken from plugin sources rather than readmes; add your own
  with a `.toml` file.
- **Build-time parameters and `if`.** The values CI sets live outside the source
  and fold away at compile time.
- **Editor support out of the box.** Generated LuaCATS stubs plus a selene
  config give completion, hover, go-to-definition and arity checks — on your own
  plugin declarations too.
- **`makensis -WX` by default.** A warning from NSIS fails the build.

## Prerequisites

**NSIS 3.x** is installed and on your `PATH` (`makensis -VERSION`). Installua targets 3.12.

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

`installua init --interactive` offers three more things on top of those two
files, as a checklist:

- **Create stubs** — `installua stubs`, run here rather than printed as an
  instruction.
- **VS Code settings** — `.vscode/extensions.json` (recommending
  [sumneko.lua][sumneko]) and `.vscode/tasks.json`, which binds ⇧⌘B /
  Ctrl+Shift+B to `installua build` on the current file with a problem matcher,
  so an error lands on the Lua line. Both are _merged_ into what is already
  there, comments and all. There is no `launch.json`: launching needs a debug
  adapter, and neither Installua nor NSIS has one.
- **Update .gitignore** — the generated files, and the `.nsi` and `.exe` a build
  leaves behind.

`init` refuses a directory it has already been run in, naming every file in the
way and writing none of them. `--force` overwrites; `--interactive` asks per
file, and `--interactive --force` asks nothing.

[sumneko]: https://marketplace.visualstudio.com/items?itemName=sumneko.lua

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
