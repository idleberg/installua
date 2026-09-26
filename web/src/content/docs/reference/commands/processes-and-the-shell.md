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

## InstallLib / UnInstallLib

Copies in a DLL, type library or COM server through the stock `Library.nsh`,
the whole job and not only the copy. An older version on the target is kept.
A file that is in use is replaced on reboot. The library is registered, and
its shared-DLL count is kept up to date. The source path is read while
`makensis` builds, so it has to be known at build time.

**Usage** `installLib(localFile, destination[, { … }])` ·
`uninstallLib(file[, { … }])` → nothing
**Options** `type` (`"DLL"`, the default, `"REGDLL"`, `"TLB"`, `"REGDLLTLB"`,
`"REGEXE"`), `shared`, `reboot`, `protected`, `x64`, `shellExtension`, `com`;
`installLib` also `tempDir`, `ignoreVersion`, `equalVersion`; `uninstallLib`
also `remove`

- `shared`: for `installLib`, a string that is empty on a first install. The
  count goes up only then, so a reinstall does not count the file twice. For
  `uninstallLib` it is `true`, and the file is removed only when the count
  drops to zero.
- `reboot`: a file in use is replaced (or removed) on reboot instead of failing.
- `protected`: set it for system files, which Windows File Protection guards.
- `tempDir`: where a file in use waits for the reboot. It has to be on the
  destination's drive and defaults to the destination's directory.
- `remove`: `uninstallLib` leaves the file alone unless this is set.

```lua
local previous = readRegStr(HKLM, "Software/Example", "InstallDir")
installLib("bin/shared.dll", SYSDIR .. "/shared.dll", {
  type = "REGDLL", shared = previous, reboot = true, protected = true,
})
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
