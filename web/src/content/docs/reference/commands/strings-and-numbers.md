---
title: Strings and numbers
description: "`string.*`, arithmetic, comparison."
---

Arithmetic and comparison are Lua operators, compiled to `IntOp` and the
`*Cmp` family; width and sign are attributes of the type, so there is no
`Int64Cmp` to write. There are no floats.

| NSIS                                                       | Installua                                                           |
| ---------------------------------------------------------- | ------------------------------------------------------------------- |
| `StrCpy`                                                   | assignment: `x = y`                                                 |
| `StrCmpS`                                                  | `==`, which is **case-sensitive** — the reversal that bites hardest |
| `StrCmp`                                                   | `string.lower(a) == b`, which folds to one case-insensitive compare |
| `StrLen`                                                   | `string.len(s)` → `int`                                             |
| `IntOp`                                                    | `+ - * / % // & \| ~ << >>`                                         |
| `IntCmp` / `Int64Cmp` / `IntPtrCmp` and the unsigned forms | `< <= > >= == ~=`                                                   |
| `IntFmt` / `Int64Fmt`                                      | `string.format`                                                     |

## The string adapters

Each compiles to NSIS instructions and `StrFunc` macros — the
`${Using:StrFunc}` lines are collected and emitted for you.

| Installua                                        | Notes                                 |
| ------------------------------------------------ | ------------------------------------- |
| `string.len(s)` → `int`                          | counts UTF-16 code units              |
| `string.sub(s, i[, j])` → `string`               | negative indices follow Lua, not NSIS, except that one reaching before the start gives `""` |
| `string.find(s, needle)` → `int`                 | `0` when absent, where Lua says `nil` |
| `string.lower(s)` / `string.upper(s)` → `string` |                                       |
| `string.format(fmt, value)` → `string`           | `IntFmt`, so one integer and one of `%c %d %i %u %x %X` |

```lua
func("majorOf", function(version)
	local dot = string.find(version, ".")
	return string.sub(version, 1, dot - 1)
end)
```

## The math adapters

NSIS has no instruction for these, so each compiles to a compare and a branch.
They matter for the type as much as the value: a check like `if n > 0` does not
make `n` non-negative for the positions that need one, such as `sleep`, and
these are how a script says so.

| Installua                          | Non-negative when                  |
| ---------------------------------- | ---------------------------------- |
| `math.abs(n)` → `int`              | always                             |
| `math.max(a, b, …)` → `int`        | any argument is, so `math.max(n, 0)` |
| `math.min(a, b, …)` → `int`        | every argument is                  |

```lua
func("pause", function(ini)
	local seconds = tonumber(readIniStr(ini, "Settings", "Autoclose"))
	sleep(math.max(seconds, 0) * 1000)
end)
```

## Casts

Every NSIS value is already a string, so `tostring` and `tonumber` emit no code.
They change what the compiler lets a value be passed to, and nothing else.

| Installua                  | Takes            | Notes                                                                  |
| -------------------------- | ---------------- | ---------------------------------------------------------------------- |
| `tostring(v)` → `string`   | `int`, `string`  | concatenation already accepts an `int`: `"step " .. i` needs no cast   |
| `tonumber(s)` → `int`      | `string`, `int`  | NSIS's reading, not Lua's: `"abc"` is `0`, `"0x10"` is 16, `"010"` is 8 |

A `bool` is refused by both, because it is stored as `1` or `0` and Lua's
`tostring(true)` is `"true"`.

```lua
local count = tonumber(readEnvStr("RETRIES")) + 1
messageBox(tostring(count))
```

## ExpandEnvStrings / ReadEnvStr

| NSIS               | Installua                             |
| ------------------ | ------------------------------------- |
| `ReadEnvStr`       | `readEnvStr(name)` → `string`         |
| `ExpandEnvStrings` | `expandEnvStrings(string)` → `string` |

NSIS has no instruction that writes a variable. `setEnv(name, value)` is
`SetEnvironmentVariable` through the `System` plugin. It changes the
installer's own environment, so a program the installer starts afterwards
inherits it, and nothing outside the installer sees it:

```lua
setEnv("NSISDIR", EXEDIR .. "/nsis")
exec('"' .. EXEDIR .. '/nsis/makensis.exe" build.nsi')
```

## ReadMemory

**Usage** `readMemory(address, size)` → `string`

---
