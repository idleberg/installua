---
title: Strings and numbers
description: "`string.*`, arithmetic, comparison."
---

Arithmetic and comparison are Lua operators, lowered onto `IntOp` and the
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

Hand-written lowerings onto NSIS instructions and `StrFunc` macros — the
`${Using:StrFunc}` lines are collected and emitted for you.

| Installua                                        | Notes                                 |
| ------------------------------------------------ | ------------------------------------- |
| `string.len(s)` → `int`                          | counts UTF-16 code units              |
| `string.sub(s, i[, j])` → `string`               | negative indices follow Lua, not NSIS |
| `string.find(s, needle)` → `int`                 |                                       |
| `string.lower(s)` / `string.upper(s)` → `string` |                                       |
| `string.format(fmt, value)` → `string`           | `IntFmt`, so one integer and one of `%c %d %i %u %x %X` |

```lua
func("majorOf", function(version)
	local dot = string.find(version, ".")
	return string.sub(version, 1, dot - 1)
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

## ReadMemory

**Usage** `readMemory(address, size)` → `string`

---
