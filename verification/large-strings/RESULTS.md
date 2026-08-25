# The plugin hazard is not real

Phase 0, task 1b. The design recorded, and explicitly flagged as **unverified**:

> A plugin is compiled against `NSIS_MAX_STRLEN`, so a vanilla-built plugin in a
> large-string installer is a genuine hazard — and unlike architecture and charset, NSIS
> ships no directory convention (`Plugins/x86-unicode`) that distinguishes it, so the
> cheap plugin check cannot cover it.

**It is not a hazard, and the missing directory convention is not missing.** The plugin
ABI passes the string size to the plugin at call time, so a plugin built with the shipped
`pluginapi` adapts at runtime and never sees `NSIS_MAX_STRLEN` at all.

## What was run

`probe.nsi`, built twice from one source and run under wine:

| Build | `makensis` | Stubs | Plugins |
| --- | --- | --- | --- |
| control | NSIS 3.12 vanilla, `NSIS_MAX_STRLEN=1024` | 1024 | stock, built against 1024 |
| subject | NSIS 3.12 `strlen_8192` special build | 8192 | **the same stock 1024-built DLLs** |

The subject build is the standard Windows distribution with only `makensis.exe` and
`Stubs/` swapped, so the plugin DLLs are exactly the ones a user would have. Both
assemble clean under `-WX`.

The probe assembles a long string at runtime — 34 characters at a time, so the literal
limit is not what is under test — and then hands it to a plugin in every direction that
exists, reporting a `StrLen` for each rather than relying on a crash to show a problem.

## Results

```
                                  1024 build   8192 build   8192 build
A. pure NSIS StrLen                     1023         1500         6150
B. System reads a variable              1023         1500         6150
C. System copies out to heap            1023         1500         6150
D. System writes a variable             1023         1500         6150
E. System stack round trip              1023         1500         6150
F. nsExec return code                      0            0            0
G. nsExec output StrLen                 1013         1502         6152
H. stack survives a plugin call         1023         1500         6150
```

Every path is intact at 1500 and again at 6150 characters, through two different plugins,
including the two that would break first if the size were baked in: the plugin **writing
into the installer's variable block**, whose stride *is* the string size (D), and the
**shared plugin stack** (E, H).

The control column is a second, independent confirmation of the headline finding:
1024 truncates to **1023**, silently, with no diagnostic.

## Why

`Examples/Plugin/nsis/pluginapi.h`, shipped with NSIS:

```c
#define EXDLL_INIT()           {  \
        g_stringsize=string_size; \
```

`string_size` is passed by the installer on every plugin call. `g_stringsize` is
therefore a runtime value, not a compile-time one, and every `pluginapi` helper
(`popstring`, `pushstring`, `getuservariable`, `setuservariable`) sizes against it.

## Consequences

1. **The plugin check needs no third axis.** Charset and architecture are directory
   conventions because they really are ABI-incompatible; string length is not, which is
   why NSIS never invented a directory for it. The check stays as designed.
2. **"Plugins are the unresolved part" is resolved** and should say so, with
   the residual risk named rather than the general one.
3. **The residual risk is a plugin bug, and it is undetectable.** A plugin that declares
   `TCHAR buf[NSIS_MAX_STRLEN]` and then calls `popstring(buf)` overflows a stack buffer
   in a large-string installer. Nothing about a compiled DLL reveals this, so Installua
   cannot check for it and must not pretend to. One sentence in the `maxStringLength`
   documentation covers it: raising the limit is trusting your plugins to use
   `popstringn`, and a plugin that does not is broken rather than mismatched.
4. **No code changes.** The design already said vanilla and large-string builds get
   byte-identical output; this removes the only reason that might not have held.

## Reproducing

`./run.sh` — needs wine and network access, since the hazard is by construction about two
`makensis` builds disagreeing and Homebrew ships only the vanilla one. Downloads
`nsis-3.12.zip` and `nsis-3.12-strlen_8192.zip` into a scratch directory. Compare its
output against `result-out1024.txt`, `result-out8192.txt` and `result-out7000.txt`.
