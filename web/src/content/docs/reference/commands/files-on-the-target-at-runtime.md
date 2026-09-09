---
title: Files on the target at runtime
description: "`fileOpen` and the handle methods."
---

`fileOpen` returns a handle with methods on it; NSIS's `FileWrite`-style
instructions are all reached that way.

**Usage** `fileOpen(path, mode)` → `handle`, where mode is `"r"`, `"w"` or `"a"`

| NSIS               | Installua                                 |
| ------------------ | ----------------------------------------- |
| `FileOpen`         | `fileOpen(path, mode)` → `handle`         |
| `FileRead`         | `handle:read([maxlen])` → `string`        |
| `FileWrite`        | `handle:write(text)`                      |
| `FileReadByte`     | `handle:readByte()` → `int`               |
| `FileWriteByte`    | `handle:writeByte(value)`                 |
| `FileReadWord`     | `handle:readWord()` → `int`               |
| `FileWriteWord`    | `handle:writeWord(value)`                 |
| `FileReadUTF16LE`  | `handle:readUtf16Le([maxlen])` → `string` |
| `FileWriteUTF16LE` | `handle:writeUtf16Le(text[, { … }])`      |
| `FileSeek`         | `handle:seek(offset[, mode])` → `int`     |
| `FileClose`        | `handle:close()`                          |

```lua
local f = fileOpen(INSTDIR .. "/install.log", "w")
f:write("installed\r\n")
f:close()
```

---
