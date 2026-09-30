---
title: Lua-shaped, not Lua
description: What surprises you if you already know Lua, and the one reason behind all of it.
---

For someone who knows Lua. Every row is something that will surprise you, and the reason is
always the same one: there is no runtime. Installua source is valid Lua so that your
editor, formatter and linter work — it is not Lua that runs.

The other table is [NSIS-shaped, not NSIS](/concepts/nsis-shaped-not-nsis/), for the larger
audience coming the other way.

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
| closures as values                   | rejected                    | a `function` may be an argument to `section`, `func`, the callbacks and the page callbacks, and nowhere else                                                        |
| metatables, `setmetatable`, `rawget` | rejected                    | no heap                                                                                                                                                        |
| `coroutine.*`                        | rejected                    | no scheduler                                                                                                                                                   |
| `require`, `load`, `dofile`          | rejected                    | `import` for an NSIS header, `include` for another Installua file — and both are compile-time                                                                  |
| `pcall`, `xpcall`, `error`           | rejected                    | NSIS has no exception model. `abort` is not one — it stops the section                                                                                         |
| `pairs`, `next`, `select`            | rejected                    | iteration is a whitelist: `lines(f)`, `glob(pat)`, `ipairs(t)`, and numeric `for`                                                                              |
| `goto`, `::label::`                  | rejected                    | Installua owns labels; `continue()` is the thing you actually wanted                                                                                           |
| `local x <close>`                    | rejected                    | no runtime to close over                                                                                                                                       |
| `#s` on a string                     | rejected                    | `StrLen` counts UTF-16 code units and Lua's `#` counts UTF-8 bytes — `"café"` is 4 against 5, `"日本語"` is 3 against 9. use `string.len(s)`                   |

## Things that are still here and mean something else

| Lua               | Installua                                   | Note                                                                                                                                                                                              |
| ----------------- | ------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `==` on strings   | **case-sensitive**, compiles to `StrCmpS`   | this is Lua's meaning, and the reversal of NSIS's default. `string.lower(a) == string.lower(b)` is the case-insensitive form and costs nothing — the compiler turns it into a bare `StrCmp`       |
| `//` and `%`      | Lua's meaning, at a cost                    | NSIS truncates toward zero and takes the remainder's sign from the dividend. Installua emits a correction so `-7 // 2` is `-4` and `-7 % 2` is `1`, as Lua says. Elided when the sign is provable |
| `a & b`, `a \| b` | as in Lua                                   |                                                                                                                                                                                                   |
| `a ~ b`           | bitwise **xor**, as in Lua                  | NSIS writes xor as `^`, so the two symbols swap. This is why `^` cannot be quietly remapped                                                                                                       |
| `a >> b`          | logical shift right, as in Lua              | emitted as NSIS's `>>>`. NSIS's arithmetic `>>` has no Lua equivalent; reach for `raw`                                                                                                            |
| `..`              | concatenation                               | **usually free.** It compiles to a string template, not a copy, so `INSTDIR .. "/bin"` is the literal text `"$INSTDIR\bin"` and occupies no register                                              |
| truthiness        | `bool` only                                 | see below                                                                                                                                                                                         |
| declaration order | irrelevant                                  | see below                                                                                                                                                                                         |
| a `local` in a top-level `if` | visible after `end`          | a build-time `if` is not a scope: its branch **is** the top level once taken. `lua-language-server` scopes it as Lua does and reports `undefined-global` at each use below the `end`. To pick a value, write `local X <const> = c and a or b` — see below |
| `include`         | `dofile`'s shape, at build time             | a top-level `local` is its file's own. A file shares names with `return { name = name }` and the includer reads them as `m.name` after `local m = include "…"`. Two files may declare the same top-level name; the `.nsi` suffixes all but the first |

## Truthiness is `bool` and nothing else

Lua says everything but `nil` and `false` is truthy. Installua has neither value, so rather
than invent a rule it narrows: only a `bool` may be a condition.

```lua error
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

At build time the rule is Lua's own, because there `false` is the only falsy value left.
So `c and a or b` picks a value, and is how a `<const>` depends on a `param`:

```lua
local TYPE <const> = param("TYPE", "avs")
local PLUGIN_NAME <const> = TYPE == "avs" and "AVS" or "MilkDrop"
```

Lua's trap comes with it: `c and false or b` is `b` whatever `c` is. Prefer this to
declaring `PLUGIN_NAME` in each branch of an `if`, which the compiler accepts but
`lua-language-server` cannot follow. Keep the `if` for statements — `attributes`,
`installer`, sections.

## A `func` hoists, a `local` does not

In Lua, `function f() end` is an ordered assignment and calling `f` above it is an error.
Installua compiles rather than executes, so there is no build-time execution for an ordering
rule to be _about_: every `func` is resolved before any body is compiled, and the emitter
puts each kind of output where NSIS needs it. Write your functions in whatever order reads
best. The output's order is not yours to choose.

A `local` keeps Lua's rule: it is in scope from its declaration down. Reading one above
that is an error, the same one `lua-language-server` reports as `undefined-global`. When
two controls' callbacks read each other, or one reads itself, declare the names first, as
in Lua:

```lua
local random, interval
random = checkbox {
	"Switch every", x = 8, y = 8, width = 56, height = 13,
	onClick = function() interval.enabled = random.checked end,
}
interval = number { x = 70, y = 8, width = 20, height = 13 }

installer {
	page.custom { "Options", controls = { random, interval } },
	page.instFiles {},
}
```

The assignment below is the declaration's value. It works for what a block or a page lists
by name: a section, a group, a page or a control.

## Two stages, and you can always tell which

The rule is that a reader must never have to guess which machine a line runs on.

| Written as                                                   | Runs                                                       |
| ------------------------------------------------------------ | ---------------------------------------------------------- |
| ordinary code                                                | install time, on the user's machine                        |
| `local X <const> = …`, and any `if` over one                 | build time, folded away — `X` becomes `${X}` in the output |
| `MAKENSIS.system(…)`, `MAKENSIS.getDllVersion(…)`, `MAKENSIS.echo(…)` | build time, as a side effect on your machine |
| `for p in glob("assets/*.txt")`                              | build time, unrolled                                       |
| `for i, x in ipairs(LIST)` over a `<const>` table             | build time, unrolled                                       |
| `import "FileFunc"`, `include "strings/de.lua"`              | build time                                                 |

`print` is the trap worth naming: it is `detailPrint` at install time and `MAKENSIS.echo` at
build time, and they are **two different names** on purpose.

## The standard library, in one table

| Kept, same meaning                           | Kept, adapted                                              | Rejected                                                   |
| -------------------------------------------- | ---------------------------------------------------------- | ---------------------------------------------------------- |
| `tostring`, `tonumber` (casts; emit no code) | `string.len` → `StrLen`                                    | `string.rep`, `.reverse`, `.byte`, `.char`                 |
|                                              | `string.sub` → `StrCpy`, 1-based → 0-based                 | `string.match`/`.gmatch`/`.gsub`                           |
|                                              | `string.upper`/`.lower` → `${StrCase}`                     | `math.*`, `table.*`, `type`, `assert`                      |
|                                              | `string.find` **plain**, two arguments → `${StrLoc}`       | `os.getenv` — use `readEnvStr`                             |
|                                              | `string.format` — `%d %i %u %x %X %c` only                 | `os.remove`, `os.rename` — use `delete`, `rename`          |
|                                              | `os.exit` → `Quit`                                         | `io.*` — use the `fileOpen` handle                         |
|                                              |                                                            | `error`, `pcall`, `xpcall`                                 |
|                                              |                                                            | `require`, `load`, `dofile`                                |
|                                              | `ipairs` → unrolled at build time, over a `<const>` table  | `pairs`, `next`, `select`                                  |
|                                              |                                                            | `setmetatable`, `rawget`, `coroutine.*`                    |

`string.format` takes **one** conversion, and one of `%c %d %i %u %x %X` — `IntFmt` is
`wsprintf` with a single integer argument, and that is `wsprintf`'s set. Flags, width and
precision are yours (`%04d`, `%#x`, `0x%X`), and `%%` is a literal per cent. Everything
else is a compile error, because NSIS is not: `IntFmt $0 "%o" 255` builds clean under
`-WX` and prints a literal `o`, and `%s` reads the number as a pointer. `%I64d` is
rejected too — that is `Int64Fmt`, a different instruction.

Anything not in the table is an unknown global, and the generated `selene` std says so.

## Ten minutes in, you will hit these

**`detailPrint("costs $5")` prints `costs $5`.** Every `$` in a literal is escaped. A
literal is _data_, never a template — see the other table for why.

**`"C:\Program Files"` is an error.** `\` is Lua's escape character, and `\P` is not an
escape. Write `[[C:\Program Files]]` or `"C:\\Program Files"`. `"C:/Program Files"` also
works in a path position, but is left as written in a string you store — see
[paths](/concepts/nsis-shaped-not-nsis/).

**`local f = function() … end` is not a value.** Pass the function literal directly to
`section`, `func` or a callback field.

**`x = 1` with no `local` declares a global**, exactly as in Lua — and emits `Var x`. All
assignments to one global must agree on type, and a conflict names every site, because
"the first one" is meaningless when any body may assign it.

**Recursion works, and has a cliff.** About 1300 frames under wine, and then the process
dies _silently_ — no dialog, no log line, no error level. The compiler warns on unbounded
recursion and names the iterative form.
