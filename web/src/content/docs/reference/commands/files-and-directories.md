---
title: Files and directories
description: Packing files in, and what happens to them on the target disk.
---

The two halves of this group do different things at different times. `file`
packs a file from the **build** machine into the installer; everything else
moves, copies or deletes files on the **target** machine while the installer
runs. Paths take forward slashes and are normalised on the way out.

## File

Packs one or more files into the installer and unpacks them into the current
output directory. A missing file is a build error unless `nonFatal` says
otherwise.

**Usage** `file(filespec, …[, { … }])` → nothing
**Options** `recursive`, `exclude = { … }`, `keepAttributes`, `nonFatal`

```lua
setOutPath(INSTDIR)
file("assets/icon.ico", { exclude = { "*.tmp", "*.log" } })
```

## ReserveFile

Puts a file at the head of the data block, so it can be extracted before the
rest. Reserving a **plugin** is the compiler's job and has no spelling: it works
out which plugins `.onInit` can reach and writes `ReserveFile /plugin` itself.

**Usage** `reserveFile(filespec, …[, { … }])` → nothing
**Options** `nonFatal`, `recursive`, `exclude = { … }`

## SetOutPath

Sets the directory that following `file` calls unpack into, creating it if it
does not exist, and the working directory for shortcuts made afterwards.

**Usage** `setOutPath(path)` → nothing

```lua
setOutPath(INSTDIR)
```

## InitPluginsDir

No spelling, and nothing to remember: any body that names `PLUGINSDIR` gets the
line at the top of it, because `$PLUGINSDIR` expands to nothing until something
creates it and NSIS never says so.

```lua
setOutPath(PLUGINSDIR)   -- InitPluginsDir is written above this
file("assets/splash.bmp")
```

## CreateDirectory

Creates a directory and every missing parent. Unlike `setOutPath` it does not
change where later `file` calls land.

**Usage** `createDirectory(directoryName)` → nothing

```lua
createDirectory(INSTDIR .. "/logs")
```

## Delete

Deletes a file, or every file matching a wildcard. Deleting something that is
not there is not an error — check with `fileExists` if you care.

**Usage** `delete(filespec[, { … }])` → nothing
**Options** `rebootOk`

```lua
delete(INSTDIR .. "/old.txt", { rebootOk = true })
```

## RMDir

Removes a directory. It must be empty unless `recursive` is set, and `recursive`
on a directory the user chose is how an uninstaller eats a disk.

**Usage** `rmDir(directoryName[, { … }])` → nothing
**Options** `recursive`, `rebootOk`

```lua
rmDir(INSTDIR, { recursive = true, rebootOk = true })
```

## Rename

Moves a file, across directories as well as within one. With `rebootOk`, a
rename blocked by a lock is queued for the next restart.

**Usage** `rename(sourceFile, destinationFile[, { … }])` → nothing
**Options** `rebootOk`

```lua
rename(INSTDIR .. "/old.txt", INSTDIR .. "/new.txt")
```

## CopyFiles

Copies files already on the target disk, with the shell's copy progress dialog.
For files coming out of the installer, use `file`.

**Usage** `copyFiles(sourcePath, destinationPath[, sizeInKb[, { … }]])` → nothing
**Options** `silent`, `filesOnly`

```lua
copyFiles(INSTDIR .. "/data", INSTDIR .. "/backup")
```

## SetFileAttributes

**Usage** `setFileAttributes(file, attribute)` → nothing

```lua
setFileAttributes(INSTDIR .. "/config.ini", "READONLY")
```

## Asking the file system a question

| NSIS               | Installua                                                          |
| ------------------ | ------------------------------------------------------------------ |
| `GetFullPathName`  | `getFullPathName(pathOrFile[, { … }])` → `string`                  |
| `GetTempFileName`  | `getTempFileName([baseDir])` → `string`                            |
| `SearchPath`       | `searchPath(filename)` → `string`                                  |
| `GetFileTime`      | `getFileTime(file)` → `int, int` (high, low)                       |
| `GetFileTimeLocal` | `getFileTimeLocal(localFile)` → `int, int` — the **build** machine |

```lua
local temp = getTempFileName()
detailPrint(temp)
```

---
