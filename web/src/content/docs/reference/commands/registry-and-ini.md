---
title: Registry and INI
description: Reading and writing `HKLM`, `HKCU`, and `.ini` files.
---

Root keys are the predefined globals `HKLM`, `HKCU`, `HKCR`, `HKU`, `HKCC` and
`SHCTX`. Subkey paths take forward slashes and are rewritten to backslashes on
the way out.

## WriteRegStr / WriteRegDWORD

`writeReg` is both: the value's type picks the instruction.

**Usage** `writeReg(rootKey, subKey, entryName, value)` → nothing

```lua
writeReg(HKLM, "Software/Example", "Path", INSTDIR)
writeReg(HKLM, "Software/Example", "Build", 42)
```

## The other write forms

| NSIS                | Installua                                         |
| ------------------- | ------------------------------------------------- |
| `WriteRegExpandStr` | `writeRegExpandStr(root, subKey, name, value)`    |
| `WriteRegBin`       | `writeRegBin(root, subKey, name, hexString)`      |
| `WriteRegMultiStr`  | `writeRegMultiStr(root, subKey, name, hexString)` |
| `WriteRegNone`      | `writeRegNone(root, subKey, name[, hexData])`     |

## ReadRegStr / ReadRegDWORD

A missing key or value gives `""` or `0` and sets the error flag, so the empty
string is the test most scripts want.

**Usage** `readRegStr(root, subKey, entryName)` → `string` ·
`readRegDword(root, subKey, entryName)` → `int`

```lua
local path = readRegStr(HKLM, "Software/Example", "Path")
```

## DeleteRegKey / DeleteRegValue

`deleteRegKey` deletes the key and everything under it — what an uninstaller
wants and a footgun everywhere else.

**Usage** `deleteRegKey(root, subKey[, { … }])` · `deleteRegValue(root, subKey, entryName)`
**Options** `ifEmpty`, `ifNoSubKeys`, `ifNoValues`

```lua
deleteRegKey(HKLM, "Software/Example")
```

## EnumRegKey / EnumRegValue

The name of the _n_-th subkey or value, counting from zero, or `""` past the
end.

**Usage** `enumRegKey(root, subKey, index)` → `string` ·
`enumRegValue(root, subKey, index)` → `string`

```lua
local key = enumRegKey(HKLM, "Software/Example", 0)
```

## ReadINIStr / WriteINIStr / DeleteINIStr / DeleteINISec / FlushINI

| NSIS           | Installua                                            |
| -------------- | ---------------------------------------------------- |
| `ReadINIStr`   | `readIniStr(iniFile, section, entryName)` → `string` |
| `WriteINIStr`  | `writeIniStr(iniFile, section, entryName, value)`    |
| `DeleteINIStr` | `deleteIniStr(iniFile, section, entryName)`          |
| `DeleteINISec` | `deleteIniSection(iniFile, section)`                 |
| `FlushINI`     | `flushIni(iniFile)`                                  |

```lua
writeIniStr(INSTDIR .. "/app.ini", "General", "Path", INSTDIR)
flushIni(INSTDIR .. "/app.ini")
```

---
