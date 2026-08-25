# Lua-shaped, not Lua

For someone who knows Lua. Every row is something that will surprise you, and the reason is
always the same one: there is no runtime. Installua source is valid Lua so that your
editor, formatter and linter work — it is not Lua that runs.

The other table is [NSIS-shaped, not NSIS](nsis-shaped-not-nsis.md), for the larger
audience coming the other way.

---

## The one-sentence version

**There is no heap, no `nil`, no floats, and no execution at build time.** Everything below
follows from those four.

---

## Things that are gone

| Lua                                  | Installua                   | Why                                                                                                                                                            |
| ------------------------------------ | --------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `nil`                                | does not exist              | there is no value to represent it; NSIS variables are strings and `""` is a string                                                                             |
| floats — `1.5`, `1e3`, `/`, `^`      | rejected **at the literal** | NSIS has no float arithmetic at all. `/` says _use `//`_; `^` is rejected outright because NSIS's `^` is xor and silently remapping it ships a wrong installer |
| tables as runtime values             | compile-time only           | a table literal is configuration or a folded constant; there is nothing to allocate it in                                                                      |
| closures as values                   | rejected                    | a `function` may be an argument to `section`, `func`, `onInit` and the page callbacks, and nowhere else                                                        |
| metatables, `setmetatable`, `rawget` | rejected                    | no heap                                                                                                                                                        |
| `coroutine.*`                        | rejected                    | no scheduler                                                                                                                                                   |
| `require`, `load`, `dofile`          | rejected                    | `import` for an NSIS header, `include` for another Installua file — and both are compile-time                                                                  |
| `pcall`, `xpcall`, `error`           | rejected                    | NSIS has no exception model. `abort` is not one — it stops the section                                                                                         |
| `pairs`, `ipairs`, `next`, `select`  | rejected                    | iteration is a whitelist: `lines(f)`, `glob(pat)`, and numeric `for`                                                                                           |
| `goto`, `::label::`                  | rejected                    | Installua owns labels; `continue()` is the thing you actually wanted                                                                                           |
| `local x <close>`                    | rejected                    | no runtime to close over                                                                                                                                       |
| `#s` on a string                     | rejected                    | `StrLen` counts UTF-16 code units and Lua's `#` counts UTF-8 bytes — `"café"` is 4 against 5, `"日本語"` is 3 against 9. `string.len(s)` is the spelling       |

## Things that are still here and mean something else

| Lua               | Installua                                   | Note                                                                                                                                                                                              |
| ----------------- | ------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `==` on strings   | **case-sensitive**, and lowers to `StrCmpS` | this is Lua's meaning, and the reversal of NSIS's default. `string.lower(a) == string.lower(b)` is the case-insensitive form and costs nothing — it peepholes to a bare `StrCmp`                  |
| `//` and `%`      | Lua's meaning, at a cost                    | NSIS truncates toward zero and takes the remainder's sign from the dividend. Installua emits a correction so `-7 // 2` is `-4` and `-7 % 2` is `1`, as Lua says. Elided when the sign is provable |
| `a & b`, `a \| b` | as in Lua                                   |                                                                                                                                                                                                   |
| `a ~ b`           | bitwise **xor**, as in Lua                  | NSIS spells xor `^`, so the spellings swap. This is why `^` cannot be quietly remapped                                                                                                            |
| `a >> b`          | logical shift right, as in Lua              | emitted as NSIS's `>>>`. NSIS's arithmetic `>>` has no Lua spelling; reach for `raw`                                                                                                              |
| `..`              | concatenation                               | **usually free.** It lowers to a string template, not a copy, so `INSTDIR .. "/bin"` is the literal text `"$INSTDIR\bin"` and occupies no register                                                |
| truthiness        | `bool` only                                 | see below                                                                                                                                                                                         |
| declaration order | irrelevant                                  | see below                                                                                                                                                                                         |

## Truthiness is `bool` and nothing else

Lua says everything but `nil` and `false` is truthy. Installua has neither value, so rather
than invent a rule it narrows: only a `bool` may be a condition.

```lua
if fileExists(p) then end        -- fine, and costs no register
local ok = errors()
if ok then end                   -- fine

if count then end                -- error: use `count ~= 0`
if s then end                    -- error: use `s ~= ""`
local dir = customDir or DEFAULT -- error: there is no `nil` to fall through
```

The by-type rule you might expect — `int` truthy when non-zero — is disqualified rather
than merely risky. `if count then` with `count == 0` **prints** in Lua and would not here,
and `lua-language-server` says nothing either way. A construct that means the opposite of
what it means in Lua, silently, is exactly what this language exists to avoid.

`a or b` where both are `bool` is fine, and is the one place a boolean lands in a register.

## Everything hoists, so there are no ordering rules to learn

In Lua, `function f() end` is an ordered assignment and calling `f` above it is an error.
Installua compiles rather than executes, so there is no build-time execution for an ordering
rule to be _about_: every top-level name is resolved before any body is lowered, and the
emitter puts each kind of output where NSIS needs it.

Write your functions in whatever order reads best. The output's order is not yours to
choose.

## Two stages, and you can always tell which

The rule is that a reader must never have to guess which machine a line runs on.

| Spelling                                                     | Runs                                                       |
| ------------------------------------------------------------ | ---------------------------------------------------------- |
| ordinary code                                                | install time, on the user's machine                        |
| `local X <const> = …`, and any `if` over one                 | build time, folded away — `X` becomes `${X}` in the output |
| `BUILD.system(…)`, `BUILD.getDllVersion(…)`, `BUILD.echo(…)` | build time, as a side effect on your machine               |
| `for p in glob("assets/*.txt")`                              | build time, unrolled                                       |
| `import "WinVer"`, `include "strings/de.lua"`                | build time                                                 |

`print` is the trap worth naming: it is `detailPrint` at install time and `BUILD.echo` at
build time, and they are **two different names** on purpose.

## The standard library, in one table

| Kept, same meaning                           | Kept, adapted                                              | Rejected                                                   |
| -------------------------------------------- | ---------------------------------------------------------- | ---------------------------------------------------------- |
| `tostring`, `tonumber` (casts; emit no code) | `string.len` → `StrLen`                                    | `string.rep`, `.reverse`, `.byte`, `.char`                 |
| `type`, `assert` (fold at compile time)      | `string.sub` → `StrCpy`, 1-based → 0-based                 | `string.find`/`.match`/`.gmatch`/`.gsub` **with patterns** |
|                                              | `string.upper`/`.lower` → `${StrCase}`                     | `math.floor`, `.ceil`, `.random`                           |
|                                              | `string.find`/`.gsub` **plain** → `${StrLoc}`, `${StrRep}` | `io.*` as free functions — use the handle form             |
|                                              | `string.format` — `%d %i %u %x %X %c` only                 | `os.exit` differs from `abort`; both exist                 |
|                                              | `math.abs`/`.max`/`.min`                                   | `error`, `pcall`, `xpcall`                                 |
|                                              | `os.getenv` → `ReadEnvStr`                                 | `require`, `load`, `dofile`                                |
|                                              | `os.remove`/`.rename` → `Delete`/`Rename`                  | `pairs`, `ipairs`, `next`, `select`                        |
|                                              | `os.exit` → `Quit`                                         | `setmetatable`, `rawget`, `coroutine.*`                    |
|                                              | `io.open` → a typed handle                                 |                                                            |
|                                              | `table.*` folds at compile time                            |                                                            |

`string.format`'s `%o` is rejected specifically: `IntFmt` emits a literal `o` for it, which
is a wrong answer rather than an error.

Anything not in the table is an unknown global, and the generated `selene` std says so.

## Ten minutes in, you will hit these

**`detailPrint("costs $5")` prints `costs $5`.** Every `$` in a literal is escaped. A
literal is _data_, never a template — see the other table for why.

**`"C:\Program Files"` is an error.** `\` is Lua's escape character, and `\P` is not an
escape. Write `"C:/Program Files"` (normalised for you in path positions), `[[C:\Program Files]]`,
or `"C:\\Program Files"`.

**`local f = function() … end` is not a value.** Pass the function literal directly to
`section`, `func` or a callback field.

**`x = 1` with no `local` declares a global**, exactly as in Lua — and emits `Var x`. All
assignments to one global must agree on type, and a conflict names every site, because
"the first one" is meaningless when everything hoists.

**Recursion works, and has a cliff.** About 1300 frames under wine, and then the process
dies _silently_ — no dialog, no log line, no error level. The compiler warns on unbounded
recursion and names the iterative form.
