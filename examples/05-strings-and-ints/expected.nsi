; Hand-written expected output for install.lua — PLAN Phase 0, program 5 of five.
;
; The two `${Using:StrFunc}` lines are §15.21's collect-then-emit pass. Nobody
; wrote them and the build aborts without them.
;
; The sign fixups are §15.4. `freeMib` is a `uint` from the declaration, so
; `freeMib // 1024` is a bare `IntOp /`. `delta` has no provable sign, so its
; `//` and `%` are corrected — and because the divisor is a positive constant
; the shared condition collapses to `remainder < 0`, which lets both fixups
; share **one** compare.

Unicode true

!define APP "Example5"
!define VERSION "2.1.0"
!define REGKEY "Software\Example5"
!define BLOCK_MIB 64

!include "MUI2.nsh"
!include "FileFunc.nsh"
!include "StrFunc.nsh"
!include "WordFunc.nsh"

${Using:StrFunc} StrCase
${Using:StrFunc} StrLoc

Name "${APP}"
OutFile "${APP}-setup.exe"
InstallDir "$PROGRAMFILES64\${APP}"

!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

Function majorOf
  Pop $0
  ${StrLoc} $1 "$0" "." ">"
  StrCpy $2 $0 $1 0
  Push $2
FunctionEnd

Section "Core"
  SetOutPath "$INSTDIR"
  ReadRegStr $0 HKLM "${REGKEY}" "DisplayVersion"
  StrCmpS $0 "" _generated_endif_0 0
  ${VersionCompare} "$0" "${VERSION}" $1
  StrCmpS $1 "1" 0 _generated_endif_1
  Abort "version $0 is newer than ${VERSION}"
_generated_endif_1:
  Push $0
  Call majorOf
  Pop $1
  DetailPrint "upgrading from major $1"
_generated_endif_0:
  ReadRegStr $0 HKLM "${REGKEY}" "Channel"
  StrLen $1 $0
  IntCmp $1 32 _generated_endif_3 _generated_endif_3 0
  Abort "the channel name is implausible"
_generated_endif_3:
  StrCmp $0 "beta" 0 _generated_endif_4
  ${StrCase} $1 "$0" "U"
  DetailPrint "banner: $1"
_generated_endif_4:
  ${DriveSpace} "C:\" "/D=F /S=M" $1
  IntOp $2 $1 / 1024
  IntOp $3 $1 - 512
  IntOp $4 $3 / ${BLOCK_MIB}
  IntOp $5 $3 % ${BLOCK_MIB}
  IntCmp $5 0 _generated_sign_8 0 _generated_sign_8
  IntOp $4 $4 - 1
  IntOp $5 $5 + ${BLOCK_MIB}
_generated_sign_8:
  IntFmt $6 "%04d" $4
  DetailPrint "$6 blocks, $5 MiB over"
  DetailPrint "free: $2 GiB"
SectionEnd
