---
title: Script attributes
description: "The `attributes {}` block: name, output, compression, manifest, version info."
---

Everything in this group is a field of the top-level `attributes {}` block, not
a call. It is set once, read by the compiler, and never executed. `outFile` is
the only one with no default.

```lua
attributes {
	name = "Example",
	outFile = "Example-setup.exe",
	unicode = true,
	compressor = "lzma",
	requestExecutionLevel = "admin",
}
```

## Identity and output

| NSIS                  | Installua                                            | Holds               |
| --------------------- | ---------------------------------------------------- | ------------------- |
| `Name`                | `name`                                               | string              |
| `OutFile`             | `outFile`                                            | path — **required** |
| `Caption`             | `caption`                                            | string              |
| `UninstallCaption`    | `uninstallCaption`                                   | string              |
| `Icon`                | `icon`                                               | path                |
| `UninstallIcon`       | `icon`, inside `uninstaller {}`                      | path                |
| `WindowIcon`          | `windowIcon`                                         | boolean             |
| `BrandingText`        | `brandingText`                                       | string              |
| `InstallDir`          | `installDir`                                         | path                |
| `InstallDirRegKey`    | `installDirRegKey = { root = …, key = …, name = … }` | table               |
| `AllowRootDirInstall` | `allowRootDirInstall`                                | boolean             |

## Build and compression

| NSIS                    | Installua            | Holds                                                      |
| ----------------------- | -------------------- | ---------------------------------------------------------- |
| `Unicode`               | `unicode`            | boolean — defaults `true`, always emitted first            |
| `CPU`                   | `cpu`                | `"x86"` \| `"amd64"`                                       |
| `SetCompressor`         | `compressor`         | `"zlib"` \| `"bzip2"` \| `"lzma"` — or that keyword in a table beside its flags, `{ "lzma", solid = true, final = true }` |
| `SetCompressionLevel`   | `compressionLevel`   | int — read only when `compressor` is `"zlib"` or `"bzip2"` |
| `SetCompressorDictSize` | `compressorDictSize` | int (MB) — read only when `compressor` is `"lzma"`         |
| `SetCompress`           | `compress`           | `"off"` \| `"auto"` \| `"force"` — `"off"` is refused beside a solid `compressor`, which NSIS answers with warning 8021 |
| `SetDatablockOptimize`  | `datablockOptimize`  | boolean                                                    |
| `SetDateSave`           | `dateSave`           | boolean                                                    |
| `CRCCheck`              | `crcCheck`           | boolean                                                    |
| `FileBufSize`           | `fileBufSize`        | int                                                        |
| `SetOverwrite`          | `overwrite`          | `"on"` \| `"off"` \| `"try"` \| `"ifnewer"` \| `"ifdiff"`  |
| `AllowSkipFiles`        | `allowSkipFiles`     | boolean                                                    |

```lua
attributes {
	compressor = "lzma",
	compressorDictSize = 32,
}
```

The two compression settings are the one place an `attributes {}` field depends
on another. NSIS reads a dictionary size only under LZMA and a level only under
the other two, and it accepts the wrong pairing and silently ignores it — so
this compiler refuses it instead, `error[ignored-setting]`. An absent
`compressor` counts as `"zlib"`, which is NSIS's own default, so
`compressorDictSize` written on its own is refused too.

## The manifest

The `Manifest*` commands are one table rather than eight prefixed fields: NSIS
puts the word on every command, and this language puts it on the table once.

**Usage** `manifest = { … }`

| NSIS                             | Installua                                                          |
| -------------------------------- | ------------------------------------------------------------------ |
| `RequestExecutionLevel`          | `requestExecutionLevel = "none" \| "user" \| "highest" \| "admin"` |
| `ManifestSupportedOS`            | `manifest.supportedOS = { … }`                                     |
| `ManifestMaxVersionTested`       | `manifest.maxVersionTested`                                        |
| `ManifestDPIAware`               | `manifest.dpiAware`                                                |
| `ManifestDPIAwareness`           | `manifest.dpiAwareness`                                            |
| `ManifestLongPathAware`          | `manifest.longPathAware`                                           |
| `ManifestGdiScaling`             | `manifest.gdiScaling`                                              |
| `ManifestDisableWindowFiltering` | `manifest.disableWindowFiltering`                                  |
| `ManifestAppendCustomString`     | `manifest.customStrings = { { path = …, string = … }, … }`         |

`requestExecutionLevel` lands in the manifest too and is written flat, because
its NSIS name says nothing about a manifest: the groups are the prefixes NSIS
itself uses, so the map from one to the other stays mechanical.

```lua
attributes {
	requestExecutionLevel = "admin",
	manifest = {
		supportedOS = { "Win7", "Win10" },
		dpiAwareness = "PerMonitorV2,system",
		longPathAware = true,
	},
}
```

## Version info

`VIProductVersion` is the four-part version Explorer shows on the Properties
tab, `file` is `VIFileVersion` beside it, and `keys` are the free-form pairs.

**Usage** `versionInfo = { product = <string>, file = <string>, keys = { … } }`

```lua
attributes {
	versionInfo = {
		product = "1.4.2.0",
		keys = {
			ProductName = "Example",
			CompanyName = "Example Ltd",
			FileVersion = "1.4.2",
		},
	},
}
```

## The PE image

One table, for the reason [the manifest](#the-manifest) is one — and the group
is spelled out, because `pe` is an abbreviation only someone who already works
on Windows executables reads at a glance.

**Usage** `portableExecutable = { … }`

| NSIS                   | Installua                                                                                     |
| ---------------------- | --------------------------------------------------------------------------------------------- |
| `PEAddResource`        | `portableExecutable.addResource = { { file = …, restype = …, resname = …, reslang = … }, … }` |
| `PERemoveResource`     | `portableExecutable.removeResource = { { restype = …, resname = …, reslang = … }, … }`        |
| `PEDllCharacteristics` | `portableExecutable.dllCharacteristics = { add = …, remove = … }`                             |
| `PESubsysVer`          | `portableExecutable.subsystemVersion`                                                         |

```lua
attributes {
	portableExecutable = {
		addResource = { { file = "assets/app.ico", restype = "#100", resname = "#1" } },
		subsystemVersion = "5.1",
	},
}
```

## Classic UI text and colours

These configure the window MUI2 draws into. The per-page text lives on the page
instead — see [Pages and MUI](/reference/modern-ui/).

| NSIS                  | Installua                                                    |
| --------------------- | ------------------------------------------------------------ |
| `AddBrandingImage`    | `brandingImage = { edge = …, size = …, padding = … }`        |
| `AutoCloseWindow`     | `autoCloseWindow`                                            |
| `BGFont`              | `bgFont = { face = …, height = …, weight = … }`              |
| `BGGradient`          | `bgGradient = false` or `{ top = …, bottom = …, text = … }`  |
| `SetFont`             | `font = { face = …, size = … }`                              |
| `MiscButtonText`      | `buttonText = { back = …, next = …, cancel = …, close = … }` |
| `InstallButtonText`   | `installButtonText`                                          |
| `UninstallButtonText` | `uninstallButtonText`                                        |
| `DetailsButtonText`   | `detailsButtonText`                                          |
| `CompletedText`       | `completedText`                                              |
| `FileErrorText`       | `fileErrorText = { text = …, withoutIgnore = … }`            |
| `SpaceTexts`          | `spaceTexts = false` or `{ required = …, available = … }`    |
| `ShowInstDetails`     | `showInstDetails = "hide" \| "show" \| "nevershow"`          |
| `ShowUninstDetails`   | `showUninstDetails`                                          |
| `SilentInstall`       | `silentInstall = "normal" \| "silent" \| "silentlog"`        |
| `SilentUnInstall`     | `silentUninstall = "normal" \| "silent"`                     |

---
