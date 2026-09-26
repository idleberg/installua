Unicode true

!include "MUI2.nsh"
!include "x64.nsh"

Name "Wide"
OutFile "x64.exe"

!insertmacro MUI_PAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

Section "Core"
  !insertmacro _RunningX64 "" "" 0 __GENERATED_endif_0
  SetRegView 64
__GENERATED_endif_0:
  !insertmacro _IsWow64 "" "" __GENERATED_endif_1 0
  DetailPrint "native"
__GENERATED_endif_1:
  !insertmacro _IsNativeMachineArchitecture "" 43620 0 __GENERATED_false_2
  StrCpy $0 1
  Goto __GENERATED_bool_2
__GENERATED_false_2:
  StrCpy $0 0
__GENERATED_bool_2:
  StrCmpS $0 1 0 __GENERATED_endif_3
  DetailPrint "arm"
__GENERATED_endif_3:
SectionEnd
