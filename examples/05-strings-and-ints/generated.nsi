Unicode true

!define APP "Example5"
!define VERSION "2.1.0"
!define REGKEY "Software/Example5"
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
  ${StrLoc} $1 $0 "." ">"
  IntOp $1 $1 + 1
  IntOp $1 $1 - 1
  StrCpy $0 $0 $1 0
  Push $0
FunctionEnd

Section "Core"
  SetOutPath $INSTDIR
  ReadRegStr $0 HKLM "Software\Example5" "DisplayVersion"
  StrCmpS $0 "" __GENERATED_endif_0 0
  ${VersionCompare} $0 "${VERSION}" $1
  StrCmpS $1 1 0 __GENERATED_endif_1
  Abort "version $0 is newer than ${VERSION}"
__GENERATED_endif_1:
  Push $0
  Call majorOf
  Pop $0
  DetailPrint "upgrading from major $0"
__GENERATED_endif_0:
  ReadRegStr $0 HKLM "Software\Example5" "Channel"
  StrLen $1 $0
  IntCmpU $1 32 __GENERATED_endif_2 __GENERATED_endif_2 0
  Abort "the channel name is implausible"
__GENERATED_endif_2:
  StrCmp $0 "beta" 0 __GENERATED_endif_3
  ${StrCase} $1 $0 "U"
  DetailPrint "banner: $1"
__GENERATED_endif_3:
  ${DriveSpace} "C:\" "/D=F /S=M" $0
  IntOp $1 $0 / 1024
  IntOp $0 $0 - 512
  IntOp $2 $0 / ${BLOCK_MIB}
  IntOp $3 $0 % ${BLOCK_MIB}
  IntCmp $3 0 __GENERATED_div_4_done 0 0
  IntOp $3 $3 ^ ${BLOCK_MIB}
  IntCmp $3 0 __GENERATED_div_4_done 0 __GENERATED_div_4_done
  IntOp $2 $2 - 1
__GENERATED_div_4_done:
  IntOp $3 $0 % ${BLOCK_MIB}
  IntOp $0 $0 % ${BLOCK_MIB}
  IntCmp $0 0 __GENERATED_div_5_done 0 0
  IntOp $0 $0 ^ ${BLOCK_MIB}
  IntCmp $0 0 __GENERATED_div_5_done 0 __GENERATED_div_5_done
  IntOp $3 $3 + ${BLOCK_MIB}
__GENERATED_div_5_done:
  StrCpy $0 $3
  IntFmt $2 "%04d" $2
  DetailPrint "$2 blocks, $0 MiB over"
  DetailPrint "free: $1 GiB"
SectionEnd
