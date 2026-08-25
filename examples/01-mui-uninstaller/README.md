# Program 1 — MUI installer with an uninstaller

Assembles clean: `makensis -WX -V4 expected.nsi` produces zero warnings.

The shape program, deliberately shallow on expressions. What it exercises: all four
declaration forms, the MUI2 page world in both directions, uninstaller duality, and six of
the eight *emitted, never written* prefixes — `$` sigils, `un.`, the `.` on `.onInit`,
`${…}` on defines, `MUI_UNPAGE_`, and `_generated_` labels.

## What it settled

**`INSTDIR` is writable and `PROGRAMFILES64` is not.** `INSTDIR = prior` has to mean
`StrCpy $INSTDIR $0`, and `PROGRAMFILES64 = x` has to be an error. So the predefined-globals
table needs a **`writable` column** — it is not one uniform kind of name. The stdlib survey lists them all
together and does not say this.

**Label scope is per body, verified.** `_generated_endif_0` appears in both `.onInit` and
`un.onInit` of the assembling output. The counter resets per body; this
confirms `makensis` agrees, rather than the decision resting on inference.

**`writeReg`'s variant selection works on an integer literal.** `NoModify = 1` emits
`WriteRegDWORD`, the other five emit `WriteRegStr`, from operand type alone.

**`messageBox` fusion spends no register.** `if answer == "NO" then os.exit() end` is one
`MessageBox` line with one jump target, `IDYES` skipping the body. The claim that the
jump table is recovered by fusion holds on the first real use.

## What it left open

**`license` had no home in the model — Phase 6 gave it one.** `MUI_PAGE_LICENSE` takes
its file as a **macro argument**, not a `!define`, unlike every other page setting. As
`installer { license = … }` it was page data written at block level: exactly one page read
it, and a script naming no License page dropped it without a word. It is
`page.license { file = … }` now, where the file is required and forgetting it is a
diagnostic — the second of the two options this section named, and it did cost a line of
source.

**`uninstaller { icon = … }` does not emit inside the uninstaller region.** `MUI_UNICON`
is a `!define` that MUI reads at interface-init time, so it must precede the **installer's**
first page macro. The block is a scoping construct for *names*, not a region of the output —
worth stating in the docs, because "the block is the context" invites the opposite
reading.

**`createShortcut` arity.** NSIS's `CreateShortcut` takes up to seven arguments, five of
them optional and trailing. Used here with two. It is the first command in the set where
`req: false` on a long tail of `In` parameters matters, and it argues for the options-table
form rather than positional trailing `nil`s.

## Exposed commands used

`readRegStr` · `writeReg` (`WriteRegStr`, `WriteRegDWORD`) · `deleteRegKey` ·
`setOutPath` · `file` · `delete` · `rmDir` · `createDirectory` · `createShortcut` ·
`writeUninstaller` · `messageBox` · `os.exit` (`Quit`)

Attributes: `name` · `outFile` · `unicode` · `compressor` · `requestExecutionLevel` ·
`versionInfo.product` · `versionInfo.keys` · `installDir` · `icon`

Pages: `page.welcome` · `page.license` (`file`) · `page.directory` · `page.instFiles` ·
`page.finish` · `page.confirm`

MUI subset needed: `MUI_ICON` · `MUI_UNICON` · `MUI_PAGE_WELCOME` · `MUI_PAGE_LICENSE` ·
`MUI_PAGE_DIRECTORY` · `MUI_PAGE_INSTFILES` · `MUI_PAGE_FINISH` · `MUI_UNPAGE_CONFIRM` ·
`MUI_UNPAGE_INSTFILES` · `MUI_LANGUAGE` — ten, out of roughly seventy.
