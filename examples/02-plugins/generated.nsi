Unicode true

!define APP "Example2"

!include "MUI2.nsh"

Name "${APP}"
OutFile "${APP}-setup.exe"
RequestExecutionLevel admin
InstallDir "$PROGRAMFILES64\${APP}"

!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

Var gitDescribe

Function .onInit
  StrCpy $gitDescribe ""
  UserInfo::GetAccountType
  Pop $0
  StrCmpS $0 "Admin" __GENERATED_endif_0 0
  MessageBox MB_OK|MB_ICONSTOP "Administrator rights are required."
  Quit
__GENERATED_endif_0:
FunctionEnd

Section "Core"
  SetOutPath $INSTDIR
  File "assets\tool.exe"
  StrCpy $0 "$INSTDIR/tool.exe"
  StrCpy $1 "${APP} smoke test"
  Push $0
  Push $1
  nsExec::ExecToStack "$\"$0$\" --version"
  Pop $2
  Pop $3
  Pop $1
  Pop $0
  StrCmpS $2 0 __GENERATED_endif_0 0
  DetailPrint "$1 failed: $3"
  Abort "the bundled tool does not run on this machine"
__GENERATED_endif_0:
  DetailPrint "$1 reported $3"
  Push $0
  Push $1
  Push $3
  System::Call "kernel32::GetTickCount() i .s"
  Pop $2
  Pop $3
  Pop $1
  Pop $0
  DetailPrint "uptime tick $2 for $3 at $0"
  Push $1
  nsExec::ExecToStack '"$INSTDIR\tool.exe" --describe'
  Pop $0
  Pop $gitDescribe
  Pop $1
  DetailPrint "built from $gitDescribe, $1"
SectionEnd
