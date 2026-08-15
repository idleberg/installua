; Hand-written expected output for install.lua — PLAN Phase 0, program 2 of five.

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
  UserInfo::GetAccountType
  Pop $0
  StrCmpS $0 "Admin" _generated_endif_0 0
  MessageBox MB_OK|MB_ICONSTOP "Administrator rights are required."
  Quit
_generated_endif_0:
FunctionEnd

Section "Core"
  StrCpy $gitDescribe ""
  SetOutPath "$INSTDIR"
  File "assets\tool.exe"
  nsExec::ExecToStack '"$INSTDIR\tool.exe" --version'
  Pop $0
  Pop $1
  StrCmpS $0 "0" _generated_endif_0 0
  DetailPrint "${APP} smoke test failed: $1"
  Abort "the bundled tool does not run on this machine"
_generated_endif_0:
  DetailPrint "${APP} smoke test reported $1"
  Push $1
  System::Call "kernel32::GetTickCount() i .s"
  Pop $0
  Pop $1
  DetailPrint "uptime tick $0 for $1 at $INSTDIR\tool.exe"
  nsExec::ExecToStack '"$INSTDIR\tool.exe" --describe'
  Pop $0
  Pop $gitDescribe
  DetailPrint "built from $gitDescribe, ${APP} smoke test"
SectionEnd
