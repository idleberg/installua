---
name: installua-plugin-declaration
description: Write a project-level Installua declaration for a third-party NSIS plugin or header that Installua does not ship — find the plugin on the NSIS wiki, fetch its archive, read its source for the real argument and push counts, and write the .toml into .installua/declarations/. Use when an Installua project needs a plugin the compiler does not know, when `plugin "X"` or `import "X"` errors as undeclared, when the user mentions declaring/adding an NSIS plugin, plugin arity, output counts, Pop counts, or nsis.sourceforge.io plugin pages.
---

# Declaring a third-party NSIS plugin for Installua

A declaration carries **one fact the compiler cannot discover**: how many values
the plugin leaves on the stack. A wrong count does not fail — it shifts every
later `Pop` by one and assembles cleanly. Get it from the source, not the readme.

## Quick start

```toml
# .installua/declarations/nsisunz.toml
# nsisunz, third-party: <https://nsis.sourceforge.io/Nsisunz_plug-in>.
# Counts read from nsisunz/nsisunz.cpp.

[[plugin]]
name = "nsisunz"                   # what `plugin "…"` is given
method = "unzipToLog"              # what you call it
nsis = "nsisunz::UnzipToLog"       # what NSIS is given
params = ["path", "path"]          # stack values it pops, in order
outputs = ["string"]               # values it pushes, in `Pop` order
```

```lua
local unz = plugin("nsisunz")
local result = unz.unzipToLog(zip, INSTDIR)
```

## Workflow

1. **Check it isn't already shipped.** `installua stubs .` then read
   `.installua/meta/`, or the [plugin
   reference](https://github.com/idleberg/installua/blob/main/docs/plugin-reference.md).
   22 plugins and headers ship declared; a project file redeclaring one wins, so
   correcting a shipped count is legitimate — say so in the file's comment.
2. **Get the source.** Find the plugin's page under
   <https://nsis.sourceforge.io/Category:Plugins>, follow the download link on
   that page, and unzip the archive into a temporary directory. Never guess a
   download URL — read it off the page. If the archive ships no source, say so
   and stop at what the documentation settles; a guessed count is worse than no
   declaration.
3. **Read the source for each method.** Pops are the params, pushes are the
   outputs — see [REFERENCE.md](REFERENCE.md#reading-arity-from-source) for the
   symbols and the traps. Read the readme only for *meaning*; where readme and
   source disagree, the source wins.
4. **Write one `.toml` per plugin** into `<project>/.installua/declarations/`.
   Header comment: plugin name, wiki URL, which file the counts came from, and
   which methods you deliberately left out and why.
5. **Skip what you cannot pin down.** A method whose push count depends on
   something the source does not settle is left undeclared, not guessed. If it
   varies by outcome and the first pushed value says which, that is `tagged` /
   `more`, not a guess.
6. **Verify**: `installua check <program>.lua` (a malformed declaration is
   reported before compiling), then `installua stubs .` to type the calls in the
   editor, then a real `installua build` if `makensis` is available.

## The fields

`name`, `method`, `nsis` are required; `nsis` is never derived from `method`.
Plugin-only: `flags`, `trailing`, `tagged`, `more`, `terminator`, `dir`.
Types: `string` `path` `int` `uint` `int64` `intptr` `bool` `handle` `any`
(`callback` for headers only). `path` is input-only.

Full field semantics, the `tagged`/`more` pair, flag carrying, `dir` for a
vendored DLL, and header (`[[header]]`) declarations: [REFERENCE.md](REFERENCE.md).

## Checklist

- [ ] Counts traced to a source file, and the file named in a comment
- [ ] `outputs` covers only what is pushed on **every** path; the conditional
      tail is `tagged` + `more`
- [ ] Every flag's `nsis` token copied verbatim, case and all
- [ ] `dir` set if the DLL is not in `NSISDIR/Plugins`
- [ ] `installua check` clean; a call site written and compiled
- [ ] Methods you skipped are listed in the comment with the reason
