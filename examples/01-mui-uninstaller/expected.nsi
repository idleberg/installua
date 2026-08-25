; Hand-written expected output for install.lua — Phase 0, program 1 of five.
; This file is the oracle: if it does not assemble under `makensis -WX`, the
; expectation is wrong, not the compiler.
;
; Emission order, as this program forced it (see README — the original list was
; incomplete): Unicode, !defines, !includes, attributes, MUI defines and pages,
; Vars, functions, sections.

Unicode true

!define APP "Example1"
!define VERSION "1.4.2"
!define REGKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP}"

!include "MUI2.nsh"

Name "${APP}"
OutFile "${APP}-${VERSION}-setup.exe"
SetCompressor lzma
RequestExecutionLevel admin
InstallDir "$PROGRAMFILES64\${APP}"

VIProductVersion 1.4.2.0
VIAddVersionKey CompanyName "Example Ltd"
VIAddVersionKey FileDescription "${APP} installer"
VIAddVersionKey FileVersion "${VERSION}"
VIAddVersionKey LegalCopyright "(c) Example Ltd"
VIAddVersionKey ProductName "${APP}"
VIAddVersionKey ProductVersion "${VERSION}"

!define MUI_ICON "assets\install.ico"
!define MUI_UNICON "assets\uninstall.ico"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "assets\LICENSE.txt"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

Function .onInit
  ReadRegStr $0 HKLM "${REGKEY}" "InstallLocation"
  StrCmpS $0 "" _generated_endif_0 0
  StrCpy $INSTDIR $0
_generated_endif_0:
FunctionEnd

Function un.onInit
  MessageBox MB_YESNO|MB_ICONQUESTION "Remove ${APP} and all of its files?" IDYES _generated_endif_0
  Quit
_generated_endif_0:
FunctionEnd

Section "Core"
  SetOutPath "$INSTDIR"
  File "assets\Example1.exe"
  File "assets\README.txt"
  WriteRegStr HKLM "${REGKEY}" "DisplayName" "${APP}"
  WriteRegStr HKLM "${REGKEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "${REGKEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKLM "${REGKEY}" "UninstallString" "$INSTDIR\uninstall.exe"
  WriteRegDWORD HKLM "${REGKEY}" "NoModify" 1
  WriteUninstaller "$INSTDIR\uninstall.exe"
SectionEnd

Section /o "Start menu shortcut"
  CreateDirectory "$SMPROGRAMS\${APP}"
  CreateShortcut "$SMPROGRAMS\${APP}\${APP}.lnk" "$INSTDIR\Example1.exe"
SectionEnd

Section "un.Core"
  Delete "$INSTDIR\Example1.exe"
  Delete "$INSTDIR\README.txt"
  Delete "$INSTDIR\uninstall.exe"
  Delete "$SMPROGRAMS\${APP}\${APP}.lnk"
  RMDir "$SMPROGRAMS\${APP}"
  RMDir "$INSTDIR"
  DeleteRegKey HKLM "${REGKEY}"
SectionEnd
