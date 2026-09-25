Unicode true

!define MULTIUSER_EXECUTIONLEVEL Highest
!define MULTIUSER_INSTALLMODE_COMMANDLINE
!define MULTIUSER_INSTALLMODE_INSTDIR "Shared"
!define MULTIUSER_INSTALLMODE_DEFAULT_REGISTRY_KEY "Software\Shared"
!define MULTIUSER_INSTALLMODE_DEFAULT_REGISTRY_VALUENAME "Installed"

!include "MUI2.nsh"
!include "MultiUser.nsh"

Name "Shared"
OutFile "multiuser.exe"

!insertmacro MUI_PAGE_WELCOME

!define MUI_PAGE_HEADER_TEXT "Who is this for?"
!insertmacro MULTIUSER_PAGE_INSTALLMODE
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

Section "Core"
  SetOutPath $INSTDIR
  WriteUninstaller "$INSTDIR\uninstall.exe"
  WriteRegStr SHCTX "Software\Shared" "Installed" 1
SectionEnd

Section "un.Uninstall"
  DeleteRegKey SHCTX "Software\Shared"
  Delete "$INSTDIR\uninstall.exe"
  RMDir $INSTDIR
SectionEnd

Function .onInit
  !insertmacro MULTIUSER_INIT
  DetailPrint "ready"
FunctionEnd

Function un.onInit
  !insertmacro MULTIUSER_UNINIT
FunctionEnd
