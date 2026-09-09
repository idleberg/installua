---
title: Windows facts
description: "What the machine is: version, shell folders, registry view."
---

## GetWinVer

**Usage** `getWinVer(field)` → `int`

```lua
if getWinVer("MAJOR") >= 10 then detailPrint("modern") end
```

## GetKnownFolderPath

**Usage** `getKnownFolderPath(knownFolderId)` → `string`

## SetShellVarContext / GetShellVarContext / IfShellVarContextAll

Whether `$SMPROGRAMS` and its relatives mean the current user's or every user's.

**Usage** `setShellVarContext("all" | "current" | "lastused")` ·
`getShellVarContext()` → `string` · `shellVarContextAll()` → `boolean`

```lua
setShellVarContext("all")
```

## SetRegView / GetRegView / IfAltRegView

Which half of the registry a 32-bit installer sees on a 64-bit machine. `"32"`
and `"64"` are enum members rather than numbers: the argument names a view
rather than counting anything.

**Usage** `setRegView("32" | "64" | "default" | "lastused")` ·
`getRegView()` → `string` · `altRegView()` → `boolean`

```lua
setRegView("64")
```

---
