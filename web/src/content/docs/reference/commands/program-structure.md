---
title: Program structure
description: The blocks a file is made of, and the declarations inside them.
---

A file is a Lua program, read top to bottom, with no preprocessor. Declaration
order does not matter: every top-level name is resolved before any body
is lowered, so a `func` may call one declared below it.

The order of the fields _inside_ `attributes {}` does not matter either, and for
a different reason: NSIS has a handful of commands that refuse to run — or, in
one case, crash — once something ahead of them has changed the header, so the
compiler emits the block in an order it chose rather than the one you typed.
Which fields those are was measured against `makensis` rather than reasoned
about; the list is `ORDERED` in `src/lower/mod.rs`.

## Function / FunctionEnd

Declares a function. Parameters and returns are ordinary Lua; the compiler
writes the stack traffic NSIS needs.

**Usage** `func(name, body)` → nothing

```lua
func("majorOf", function(version)
	local dot = string.find(version, ".")
	return string.sub(version, 1, dot - 1)
end)
```

## .onInit / un.onInit

The callback NSIS runs before anything is shown. Written _inside_ the block it
belongs to, so the same spelling covers both halves and the `un.` prefix is the
compiler's business.

**Usage** `onInit(body)`, listed in `installer {}` or `uninstaller {}`

```lua
installer {
	page.instFiles {},

	onInit(function()
		if fileExists(INSTDIR .. "/app.exe") then
			detailPrint("upgrading")
		end
	end),
}
```

## Installer and uninstaller blocks

`installer {}` holds the pages, sections and callbacks of the installer;
`uninstaller {}` the same for the uninstaller, and writing one is what makes
`writeUninstaller` legal. Both also carry the MUI settings whose scope is the
whole half — see [Pages and MUI](/reference/modern-ui/).

**Usage** `installer { … }`, `uninstaller { … }`

```lua
installer {
	installDir = PROGRAMFILES64 .. "/Example",

	page.directory {},
	page.instFiles {},

	section("Core", function()
		setOutPath(INSTDIR)
		file("assets/Example.exe")
		writeUninstaller(INSTDIR .. "/uninstall.exe")
	end),
}

uninstaller {
	page.confirm {},
	page.instFiles {},

	section("Remove", function()
		rmDir(INSTDIR, { recursive = true })
	end),
}
```

## Var

Globals are declared by assigning to one at the top level. A `local`
inside a body is a register the compiler allocates and reuses.

**Usage** assignment at the top level

```lua
gitDescribe = ""
```

## !include

Source layout, not a module system: the named file's declarations are merged
into this one and nothing is emitted. A path is relative to the file
that names it.

**Usage** `include(path)` → nothing

```lua
include("lib/shortcuts.lua")
```

## glob

Walks the **build** machine at compile time and unrolls the result, so the file
list is fixed before anything ships. This is what replaces
`FindFirst`/`FindNext`/`FindClose`, which walk the target's disk instead.

**Usage** `for path in glob(pattern) do … end`

```lua
for path in glob("assets/docs/*.pdf") do
	file(path)
end
```

## param

A build-time constant the invocation may set. This is what replaces
`!ifndef VERSION` / `!define VERSION "1.4.2"` / `!endif`, and `-D` is the same
flag `makensis` spells the same way.

**Usage** `local NAME <const> = param(name, default)` → the value, or
`param(name)` for one the build cannot do without

```lua
local VERSION <const> = param("VERSION", "1.4.2")
local PORT    <const> = param("PORT", 8080)
local SIGNED  <const> = param("SIGNED", false)
```

```console
$ installua build install.lua -D VERSION=2.0.0 -D SIGNED=true
$ installua build install.lua --param VERSION=2.0.0 --param SIGNED=true
```

`-D` is the spelling `makensis -D` teaches; `--param` is the same flag written
out. It is not `--define` or `--declare` because the source is what *declares* a
parameter and the invocation is what *sets* one — a flag named for the declaring
half would read as doing the thing the `unknown-param` error says it does not.

Three things follow from the declaration being written in the source rather
than passed only on the command line:

- **A `-D` naming a parameter the program does not declare is an error.** That
  is the whole point. NSIS's version fails the other way — a misspelt
  `-DVERSOIN` defines a second thing nobody reads, the build succeeds, the
  default ships, and nothing says so.
- **The default is the type.** `param("PORT", 8080)` makes `PORT` an integer, so
  `-D PORT=abc` is rejected instead of arriving at an `IntOp` as a string.
  `param("SIGNED", false)` takes `true` and `false` and nothing else.
- **The name and the binding are separate.** The string is what `-D` sets; the
  `local` is what the program reads. They are usually spelled the same and do
  not have to be.

### A parameter with no default is required

Leave the default out and the invocation has to supply the value. A build
without the flag stops with `missing-param`, at the declaration — the line that
says the value is needed.

```lua
local SIGNING_CERT <const> = param("SIGNING_CERT")
```

```console
$ installua build install.lua
install.lua:1:30: error[missing-param]: `SIGNING_CERT` has no default, so the build needs `-D SIGNING_CERT=…`
```

This is the NSIS `!ifndef SIGNING_CERT` / `!error "..."` / `!endif` guard,
written as the declaration instead of as a line beside it. The guard is
something a script has to remember to include and can be pasted in the wrong
place; the declaration is checked for every build because it *is* the parameter,
and it sits where a reader already looks for what the build takes.

A required parameter is a **string**. The default is what declares a type, so
with no default there is nothing to read the flag's text as but text — `-D
PORT=abc` is accepted by `param("PORT")` and rejected by `param("PORT", 8080)`.
If a required value has to be an integer or a boolean, check it yourself, or
give it a default and let the type do the work.

`param(…)` is a **declaration**, so it stands alone as the whole initialiser of
a top-level `local … <const>` — not inside a body, and not composed into a
larger expression. Compose one line further down instead, which costs nothing
since resolution is order-free:

```lua
local VERSION <const> = param("VERSION", "1.4.2")
local FULL    <const> = VERSION .. "-win64"   -- an ordinary <const>
```

`installua check` takes the same `-D`s, because a program whose parameters are
overridden is a different program to check.

## Build-time if

An `if` at the top level is decided by the compiler, not by the installer. This
is what replaces `!if` / `!ifdef` / `!ifndef` / `!else` / `!endif`.

**Usage** `if <build-time condition> then … end`, at the top level

```lua
local ARCH <const> = param("ARCH", "x86")

if ARCH == "x64" then
	attributes { name = "Example (64-bit)", outFile = "example-x64.exe" }
	installer { section("Core", function() file("bin/x64/app.exe") end) }
else
	attributes { name = "Example", outFile = "example.exe" }
	installer { section("Core", function() file("bin/x86/app.exe") end) }
end
```

**The condition has to fold** — a literal, a `<const>`, a `param`, or an
expression over them. A condition read at install time is an error, because out
here there is nothing to read it from: no register has been written, and the
branch would have to be taken by `makensis` rather than by the installer. That
`if` belongs inside a `section` or a `func`, where it is an ordinary runtime one.
It also has to be a `bool`, for the same reason a runtime condition does.

**Whatever the branch holds is an ordinary top-level declaration.** Sections,
`func`s, `attributes`, `<const>`s, `param`s, globals, `raw.head` / `raw.tail` —
there is no second set of rules, because the selected statements *are* the top
level once the branch is taken. Resolution stays order-free across it: a section
declared inside a branch can be listed by an `installer {}` written outside it,
and a `<const>` declared below the `if` can be what decides it.

The one thing to know is what "not taken" means: a declaration in the branch that
was not taken is not part of the program at all. A `param` declared only there is
not declared, so a `-D` for it is the same `unknown-param` error a misspelling
gets — which is the honest answer, since in that configuration the program really
does not have it.

**Nothing of the conditional reaches the output.** It is not a directive the
compiler emits and then orders against everything else — it is a branch the
compiler takes, before a single line is bucketed. The emitted script is
byte-for-byte the script you would get by writing only the branch that won.
`include` is the one thing that cannot go in a branch: files are merged before
any of this runs.

---
