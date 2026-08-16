Unicode true

!define APP "Example3"

!include "MUI2.nsh"
!include "TextFunc.nsh"

Name "${APP}"
OutFile "${APP}-setup.exe"
InstallDir "$PROGRAMFILES64\${APP}"

!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

Section "Core"
  SetOutPath $INSTDIR
  File "assets\manifest.txt"
  File "assets\notes.txt"
  FileOpen $0 "$INSTDIR\manifest.txt" "r"
  StrCpy $1 0
__GENERATED_for_0_top:
  ClearErrors
  FileRead $0 $2
  IfErrors __GENERATED_for_0_end 0
  ${TrimNewLines} $2 $2
  StrCmpS $2 "" 0 __GENERATED_endif_1
  Goto __GENERATED_for_0_top
__GENERATED_endif_1:
  StrCmpS $2 "END" 0 __GENERATED_endif_2
__GENERATED_for_0_end:
  FileClose $0
  IntCmpU $1 0 0 __GENERATED_endif_3 __GENERATED_endif_3
  Abort "the manifest is empty"
__GENERATED_endif_3:
  DetailPrint "read $1 entries"
  StrCpy $0 1
__GENERATED_for_4_top:
  IntCmpU $0 3 0 0 __GENERATED_for_4_end
  ClearErrors
  CreateDirectory "$INSTDIR\cache"
  IfErrors 0 __GENERATED_then_5
  DetailPrint "cache directory attempt $0 failed"
  Sleep 200
  IntOp $0 $0 + 1
  Goto __GENERATED_for_4_top
__GENERATED_then_5:
__GENERATED_for_4_end:
  Return
__GENERATED_endif_2:
  IntOp $1 $1 + 1
  DetailPrint "entry $1: $2"
  Goto __GENERATED_for_0_top
SectionEnd
