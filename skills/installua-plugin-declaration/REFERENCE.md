# Declaration reference

The authority is [Declaring a third-party plugin or
header](https://github.com/idleberg/installua/blob/main/docs/reference-map.md#declaring-a-third-party-plugin-or-header).
This file is the working subset plus the part that is not written down there:
how to get the counts out of a plugin's source.

## Fields

One file per plugin in `.installua/declarations/`; the file name is yours, the
extension is `.toml`. One `[[plugin]]` block per method.

| Field | Where | Meaning |
| --- | --- | --- |
| `name` | both | what `plugin "…"` / `import "…"` is given |
| `method` | both | the Installua method name (camelCase) |
| `nsis` | both | `Plugin::Method`, or the macro name without `${}` |
| `params` | both | types of the stack values popped, in order |
| `outputs` | both | types pushed on **every** path, in `Pop` order |
| `flags` | plugin | `/SWITCH` tokens; see below |
| `trailing` | plugin | `true` puts the flag run **after** the arguments |
| `tagged` | plugin | first-popped literals that mean `more` follows |
| `more` | plugin | the values that follow when tagged |
| `terminator` | plugin | a `/TOKEN` emitted after the arguments on every call |
| `dir` | plugin | project-relative directory of a vendored DLL |

Types: `string`, `path`, `int`, `uint`, `int64`, `intptr`, `bool`, `handle`,
`any`, and `callback` (headers only). `path` normalises `/` to `\` on the way in
and is rejected in `outputs`. Reach for `uint` on a count or a size — it elides
the sign fixup on `//`.

A header's outputs are **trailing register arguments**, in the order the macro
writes them, because `!insertmacro` cannot return anything. A plugin's are stack
depth. That is why they are separate ideas.

### tagged / more

For a push count that varies by outcome, where the first pushed value says which:

```toml
outputs = ["string"]     # every path pushes this
tagged  = ["error"]      # when it is one of these …
more    = ["string"]     # … these follow
```

Both lists are part of the Lua arity (`local ok, why = …` binds two), and the
tail reads `""` or `0` on the path that pushed nothing. The polarity is the
plugin's: `AccessControl` tags its failure, `StartMenu::Select` tags its success.
Neither list may appear without the other, and `outputs` must have at least one
entry for `tagged` to test.

### flags

```toml
flags = [
  { name = "autoadd", nsis = "/autoadd" },                             # bare
  { name = "text",    nsis = "/text",    ty = "string", value = "separate" },
  { name = "timeout", nsis = "/TIMEOUT", ty = "int",    value = "joined" },
]
```

Named at the call site in a table written **last**, emitted in the order
declared. A bare flag *is* its value: `false` writes nothing, and it must be a
literal. `ty` and `value` are one field in two halves — `joined` is `/T=5000`,
`separate` is `/text "…"`. Copy `nsis` verbatim; a token NSIS does not recognise
becomes a positional argument silently.

## Reading arity from source

The C/C++ plugin API (`pluginapi.h`, `exdll.h`), inside each exported function:

| Symbol | Meaning |
| --- | --- |
| `popstring`, `popstringn`, `PopString`, `popint`, `popintptr`, `popint64` | one `params` entry, in call order |
| `pushstring`, `PushString`, `pushint`, `pushintptr` | one `outputs` entry, in call order |
| `system_pushstring`, `myPushString`, local wrappers | same — follow the definition |

Delphi/Pascal plugins use `NSIS.pas`: `PopString`, `PushString`.

Traps that decide whether a declaration is right:

- **Count pushes per path, not in total.** Only what every path pushes goes in
  `outputs`; a path-dependent tail is `tagged`/`more`. An early `return` after
  one push and a success path with two is the classic shape.
- **Push order is reverse `Pop` order.** The last `pushstring` is popped first.
- **A loop that pops until a sentinel needs a `terminator`.** `inetc::get` reads
  url/file pairs until `/end`; without it, it pops past its own arguments into
  whatever the caller-save left there. Not optional, not a flag.
- **Arguments read by index rather than by leading `/`** mean `trailing = true`
  (`NScurl::http` takes parameters 0–2 positionally, so a leading `/SILENT` would
  become the HTTP method).
- **An input arriving by `Push` from the caller** rather than as a plugin
  argument is not declarable — skip the method and say so.
- **`/NOUNLOAD` variants and `_Unload` methods** are not declared: Installua
  never emits `/NOUNLOAD`.
- **Different ANSI and Unicode source trees** must agree. If they do not, declare
  the Unicode one and note it.

Read the readme and the wiki page for what a method *means* and what its return
values signify. Do not take a count from either. `AccessControl` is the standing
example: its readme, its wiki page, and the way real installers call it each
imply a different answer, and all three are wrong.

## Headers

```toml
[[header]]
name = "Brand"                # `import "Brand"` → `!include "Brand.nsh"`
method = "applyTheme"
nsis = "BrandApplyTheme"      # macro name, no ${}
params = ["string"]
outputs = ["string"]          # trailing registers, in write order
```

Only for a macro whose value is a **signature**. A macro framework whose value is
control flow or side effects (`Library.nsh`: overwrite mode, reference counting,
reboot flags) does not become a declaration — a correct file describing only its
parameter list documents the wrong thing.

`callback` marks a parameter that takes a function address, but only for macros
whose register map the compiler already knows (`Locate`, `TextCompare`,
`LineFind`, …). There is no way to declare a third-party callback macro.

## Vendored DLLs

`dir = "vendor/plugins"` — project-relative, per block. Installua emits
`!addplugindir` and reserves the file. A declaration is never a bundled DLL:
installing it is still the project's job.
