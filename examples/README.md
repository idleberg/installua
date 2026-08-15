# The five programs

PLAN Phase 0, tasks 2 and 3. Five complete Installua programs, each with the exact `.nsi`
wanted out of it, hand-written. **The `.nsi` files are the oracle** — `./assemble.sh` runs
all five under `makensis -WX` with an empty warning allowlist, and if one does not
assemble the expectation is wrong, not the compiler.

```
makensis v3.12
PASS 01-mui-uninstaller     <hash>
PASS 02-plugins             <hash>
PASS 03-file-iteration      <hash>
PASS 04-multiple-returns    <hash>
PASS 05-strings-and-ints    <hash>

all five assemble clean
```

The hash is the second half of the check: each program is assembled **twice** and the two
`.exe` files must be byte-identical.

**The hashes are not stable across checkouts, and that is a finding rather than a
nuisance.** `makensis` embeds each packed file's mtime, git does not preserve mtimes, so a
fresh clone produces a different `.exe` for every program that has a `File` line —
verified: touching one fixture changes the hash, and program 5, which packs nothing, is
the only one that matches across checkouts.

`SetDateSave off` makes it mtime-independent and the output identical again — also
verified. That is an NSIS script attribute, so by §13's rule it belongs in `attributes {}`
as `dateSave`, and it is **not** defaulted to `false`: dropping the packed files'
timestamps is a real behaviour change and the user's call to make. It is recorded in
`PHASE-0.md` as an attribute worth exposing with the reproducible-builds argument
attached.

So the gate asserts **run-to-run determinism in a fixed tree**, which is what catches an
`.nsi` that embeds a build timestamp. Cross-machine reproducibility is available, opt-in,
and not claimed here.

## Fixtures

Since the `.exe` hash is checkout-dependent anyway, pinning the fixtures' *bytes* buys
nothing — so `examples/fixtures.py` writes the payloads nothing reads — the two `.ico` files and the two
`.exe` payloads — and `assemble.sh` runs it first. Committed instead are the fixtures whose
*content* is read: `LICENSE.txt` (shown on the license page), `manifest.txt` (parsed by
program 3's loop), `notes.txt`, `README.txt`, and `payload.bin` (whose size program 4's
runtime check asserts).

So the repo carries no vendored binaries and none of NSIS's own artwork. Running
`makensis expected.nsi` by hand in an example directory needs `python3 ../fixtures.py`
first.

Two of them are additionally checked for *behaviour* rather than assembly, under wine —
see [`../verification/semantics/`](../verification/semantics/RESULTS.md). Assembling
proves nothing about a calling convention or a sign fixup.

All five parse as real Lua 5.4 and are stable under `stylua` with the project config.

| | Program | What it is for | README |
| --- | --- | --- | --- |
| 1 | MUI installer with uninstaller | the shape: four blocks, both page families, uninstaller duality | [notes](01-mui-uninstaller/README.md) |
| 2 | plugin-heavy | the stack ABI and the three opaque callees | [notes](02-plugins/README.md) |
| 3 | file-iteration loop | staging: build-machine vs install-time iteration | [notes](03-file-iteration/README.md) |
| 4 | multiple returns | the calling convention and the clobber fixpoint | [notes](04-multiple-returns/README.md) |
| 5 | strings and integers | the sign fixups, the adapters, the `StrFunc` init lines | [notes](05-strings-and-ints/README.md) |

Each README records what its program **settled** and what it **left open**. Read those
before Phase 1; between them they are the phase's real output.

---

## The frozen v1 exposed-command list

This is the definition of scope. Everything not here is `Class::Todo` in the census,
counted and printed by `installua coverage` — not missing, *scheduled*.

**Instructions — 24**

`abort` · `clearErrors` · `createDirectory` · `createShortcut` · `delete` · `detailPrint` ·
`errors` · `execWait` · `file` · `fileClose` · `fileOpen` · `fileRead` · `fileWrite` ·
`fileExists` · `messageBox` · `readEnvStr` · `readRegStr` · `rmDir` · `setOutPath` ·
`sleep` · `strLen` (as `string.len`) · `writeReg` · `writeUninstaller` · `os.exit`
(`Quit`)

`execWait`, `fileWrite`, `readEnvStr` and `fileExists` are not reached by any of the five
programs but are in for a reason: each is the *only* member of a shape that would otherwise
have no coverage — an optional `Out`, a handle write, an environment read, and a predicate
that is pure where `errors()` is not.

**Attributes — 15**

`name` · `outFile` · `unicode` · `compressor` · `requestExecutionLevel` · `installDir` ·
`icon` · `license` · `pages` · `caption` · `text` · `manifest.*` · `versionInfo.product` ·
`versionInfo.keys` · `crcCheck`

**Declarations — 7**

`attributes` · `installer` · `uninstaller` · `languages` · `section` · `sectionGroup` ·
`func` · `onInit`

**Operators — all of §6**, since they are the compiler rather than the overlay.

**Stdlib adapters — 7**

`string.len` · `string.sub` · `string.upper` · `string.lower` · `string.find` (plain) ·
`string.format` (integer directives) · `math.abs`/`max`/`min`

**Headers reached — 5**

`MUI2` · `FileFunc` (`getSize`, `driveSpace`) · `WordFunc` (`versionCompare`) ·
`StrFunc` (`StrCase`, `StrLoc`) · `TextFunc` (`TrimNewLines`, emitted not written)

**Plugins reached — 3**

`nsExec` · `UserInfo` · `System`

## The MUI subset that is actually needed

Ten, out of roughly seventy:

`MUI_ICON` · `MUI_UNICON` · `MUI_PAGE_WELCOME` · `MUI_PAGE_LICENSE` · `MUI_PAGE_DIRECTORY` ·
`MUI_PAGE_INSTFILES` · `MUI_PAGE_FINISH` · `MUI_UNPAGE_CONFIRM` · `MUI_UNPAGE_INSTFILES` ·
`MUI_LANGUAGE`

PLAN calls the `MUI_*` volume the most likely thing to make v1 feel incomplete. Ten of
seventy is the number behind that.

## §15.14's open empirical question, answered weakly

*How often does the type lattice land on `unknown`?* Across all five programs: **zero
times**. The reason is more useful than the number — types enter almost entirely through
**declarations**, not through inference over expressions. See
[program 5's README](05-strings-and-ints/README.md#the-type-lattice-lands-on-unknown-zero-times)
for the breakdown and the caveat.

---

## Emission order, as the programs forced it

PLAN's Phase 4 list is *"`Unicode`, remaining attributes, `!include`s and their init lines,
`Var`s, functions, sections"*. Program 1 breaks it immediately: `Name "${APP}"` is an
attribute that references a `!define`, and `!define` has to come first. The order all five
goldens actually use:

1. `Unicode` — first, so a later `raw` overrides it rather than being silently overridden
2. `!define`s, in source order
3. `!include`s
4. `${Using:StrFunc}` and other init lines
5. attributes, in overlay order
6. MUI `!define`s, page macros, `MUI_LANGUAGE`
7. `Var`s
8. functions
9. sections, in source order — a sequence, never reordered

Steps 2 and 6 are new. Step 6 has to be after step 5 and before step 7 for a reason worth
recording: `MUI_ICON` and `MUI_UNICON` are read at MUI's interface-init time, which the
*first* page macro triggers — so an `uninstaller {}` field emits into the installer's
define region. The block is a scope for names, not a region of the output.
