---
title: Constants
description: "`INSTDIR`, `PROGRAMFILES64`, `HKLM`, and the rest of the predefined names."
---

Predefined NSIS constants, as ordinary read-only globals. A `$` inside a string
literal is five dollars, not a variable — join with `..` instead.

**Program files** `PROGRAMFILES` · `PROGRAMFILES32` · `PROGRAMFILES64` ·
`COMMONFILES` · `COMMONFILES32` · `COMMONFILES64`

**Shell folders** `DESKTOP` · `STARTMENU` · `SMPROGRAMS` · `SMSTARTUP` ·
`QUICKLAUNCH` · `DOCUMENTS` · `MUSIC` · `PICTURES` · `VIDEOS` · `FAVORITES` ·
`SENDTO` · `RECENT` · `NETHOOD` · `PRINTHOOD` · `FONTS` · `TEMPLATES` ·
`ADMINTOOLS` · `INTERNET_CACHE` · `COOKIES` · `HISTORY` · `PROFILE` ·
`RESOURCES` · `RESOURCES_LOCALIZED` · `CDBURN_AREA` · `APPDATA` ·
`LOCALAPPDATA`

Which folder each of those names depends on `setShellVarContext`. To name one
side outright: `USERAPPDATA` · `USERLOCALAPPDATA` · `USERTEMPLATES` ·
`USERSTARTMENU` · `USERSMPROGRAMS` · `USERDESKTOP` · `COMMONLOCALAPPDATA` ·
`COMMONPROGRAMDATA` · `COMMONTEMPLATES` · `COMMONSTARTMENU` ·
`COMMONSMPROGRAMS` · `COMMONDESKTOP`

**Paths** `INSTDIR` · `OUTDIR` · `TEMP` · `WINDIR` · `SYSDIR` · `EXEDIR` ·
`EXEPATH` · `EXEFILE` · `PLUGINSDIR`

`INSTDIR` and `OUTDIR` are writable; the rest are facts about the machine and
assigning to one is refused.

**Other** `CMDLINE` · `LANGUAGE` · `HWNDPARENT` — the installer's own window

**Registry roots** `HKLM` · `HKLM32` · `HKLM64` · `HKLMANY` · `HKCU` ·
`HKCU32` · `HKCU64` · `HKCUANY` · `HKCR` · `HKCR32` · `HKCR64` · `HKCRANY` ·
`HKU` · `HKCC` · `HKDD` · `HKPD` · `SHCTX` · `SHCTX32` · `SHCTX64` ·
`SHCTXANY`

The `32`/`64` suffix forces a view of the registry regardless of
`setRegView`; `ANY` restores the default of following it.

```lua
setOutPath(PROGRAMFILES64 .. "/Example")
```

---
