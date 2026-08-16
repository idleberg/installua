Unicode true

!define APP "Example4"

!include "MUI2.nsh"
!include "FileFunc.nsh"

Name "${APP}"
OutFile "${APP}-setup.exe"
InstallDir "$PROGRAMFILES64\${APP}"

!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

Function measure
  Pop $0
  ${GetSize} $0 "" $1 $2 $3
  Push $2
  Push $1
FunctionEnd

Function budget
  Pop $0
  Push $0
  Call measure
  Pop $0
  Pop $1
  IntOp $0 $0 / 1024
  Push $1
  Push $0
FunctionEnd

Function countdown
  Pop $0
  IntCmp $0 0 0 0 __GENERATED_endif_0
  Push 0
  Return
__GENERATED_endif_0:
  Push $0
  IntOp $1 $0 - 1
  Push $1
  Call countdown
  Pop $1
  Pop $0
  IntOp $0 $1 + $0
  Push $0
FunctionEnd

Section "Core"
  SetOutPath $INSTDIR
  File "assets\payload.bin"
  Push $INSTDIR
  Call budget
  Pop $0
  Pop $1
  Push $0
  Push $1
  Push 4
  Call countdown
  Pop $2
  Pop $1
  Pop $0
  DetailPrint "payload: $1 files, $0 KiB, check $2"
SectionEnd
