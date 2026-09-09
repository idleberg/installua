---
title: Processes and the shell
description: "`exec`, shortcuts, DLLs, reboot."
---

## Exec / ExecWait / ExecShell / ExecShellWait

| NSIS            | Installua                                                             |
| --------------- | --------------------------------------------------------------------- |
| `Exec`          | `exec(commandLine)`                                                   |
| `ExecWait`      | `execWait(commandLine)` → `int` (the exit code)                       |
| `ExecShell`     | `execShell(verb, file[, { parameters, showMode, invokeIdList }])`      |
| `ExecShellWait` | `execShellWait(verb, file[, { parameters, showMode, invokeIdList }])` |

```lua
local code = execWait('"' .. INSTDIR .. '/setup-driver.exe" /S')
if code ~= 0 then abort("driver setup failed") end
```

## CreateShortcut

**Usage** `createShortcut(linkPath, target[, { parameters, iconFile, iconIndex, showMode, hotkey, comment, noWorkingDir }])` → nothing

```lua
createShortcut(SMPROGRAMS .. "/Example/Example.lnk", INSTDIR .. "/Example.exe")
```

## RegDLL / UnRegDLL

Calls `DllRegisterServer` / `DllUnregisterServer` on a DLL already on the target.

**Usage** `regDll(path[, entryPoint])` · `unRegDll(path)`

```lua
regDll(INSTDIR .. "/shell.dll")
```

## GetDLLVersion / GetDLLVersionLocal

The version resource of a DLL — on the target, or on the **build** machine.

**Usage** `getDllVersion(filename[, { … }])` → `int, int` (high, low) ·
`getDllVersionLocal(localFilename)` → `int, int`

## Reboot / SetRebootFlag / IfRebootFlag

**Usage** `reboot()` · `setRebootFlag(true)` · `rebootFlag()` → `boolean`

## Sleep

**Usage** `sleep(milliseconds)` → nothing

---
