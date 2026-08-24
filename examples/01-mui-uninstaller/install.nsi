Unicode true

!define APP "Example1"
!define VERSION "1.4.2"
!define REGKEY "Software/Microsoft/Windows/CurrentVersion/Uninstall/Example1"

!include "MUI2.nsh"

SetCompressor lzma
Name "${APP}"
OutFile "${APP}-${VERSION}-setup.exe"
RequestExecutionLevel admin
VIProductVersion 1.4.2.0
VIAddVersionKey CompanyName "Example Ltd"
VIAddVersionKey FileDescription "${APP} installer"
VIAddVersionKey FileVersion "${VERSION}"
VIAddVersionKey LegalCopyright "(c) Example Ltd"
VIAddVersionKey ProductName "${APP}"
VIAddVersionKey ProductVersion "${VERSION}"
InstallDir "$PROGRAMFILES64\${APP}"

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

Section "Core"
  SetOutPath $INSTDIR
  File "assets\Example1.exe"
  File "assets\README.txt"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Example1" "DisplayName" "${APP}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Example1" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Example1" "InstallLocation" $INSTDIR
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Example1" "UninstallString" "$INSTDIR/uninstall.exe"
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Example1" "NoModify" 1
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
  RMDir $INSTDIR
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Example1"
SectionEnd

Function .onInit
  ReadRegStr $0 HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Example1" "InstallLocation"
  StrCmpS $0 "" __GENERATED_endif_0 0
  StrCpy $INSTDIR $0
__GENERATED_endif_0:
FunctionEnd

Function un.onInit
  MessageBox MB_YESNO|MB_ICONQUESTION "Remove ${APP} and all of its files?" IDNO __GENERATED_mb_0_no
  StrCpy $0 "YES"
  Goto __GENERATED_mb_0_end
__GENERATED_mb_0_no:
  StrCpy $0 "NO"
__GENERATED_mb_0_end:
  StrCmpS $0 "NO" 0 __GENERATED_endif_1
  Quit
__GENERATED_endif_1:
FunctionEnd
