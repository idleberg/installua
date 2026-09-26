Unicode true

!define MULTIUSER_EXECUTIONLEVEL Highest
!define MULTIUSER_INSTALLMODE_COMMANDLINE
!define MULTIUSER_INSTALLMODE_INSTDIR "Shared"
!define MULTIUSER_INSTALLMODE_DEFAULT_REGISTRY_KEY "Software\Shared"
!define MULTIUSER_INSTALLMODE_DEFAULT_REGISTRY_VALUENAME "InstallMode"
!define MULTIUSER_INSTALLMODE_INSTDIR_REGISTRY_KEY "Software\Shared"
!define MULTIUSER_INSTALLMODE_INSTDIR_REGISTRY_VALUENAME "InstallDir"
!define MEMENTO_REGISTRY_ROOT SHCTX
!define MEMENTO_REGISTRY_KEY "Software\Shared\Components"

!include "MUI2.nsh"
!include "Memento.nsh"
!include "MultiUser.nsh"

Name "Shared"
OutFile "multiuser.exe"

!insertmacro MUI_PAGE_WELCOME

!define MUI_PAGE_HEADER_TEXT "Who is this for?"
!insertmacro MULTIUSER_PAGE_INSTALLMODE
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

Section "Core"
  SetOutPath $INSTDIR
  WriteUninstaller "$INSTDIR\uninstall.exe"
  DetailPrint $MultiUser.InstallMode
SectionEnd

!insertmacro MementoSectionEx "" "Documentation" docs SEC_docs
  SetOutPath $INSTDIR
!insertmacro MementoSectionEnd

Section "un.Uninstall"
  DeleteRegKey SHCTX "Software\Shared"
  Delete "$INSTDIR\uninstall.exe"
  RMDir $INSTDIR
SectionEnd

!insertmacro MementoSectionDone

Function .onInit
  !insertmacro MULTIUSER_INIT
  !insertmacro MementoSectionRestore
  DetailPrint "ready"
FunctionEnd

Function un.onInit
  !insertmacro MULTIUSER_UNINIT
FunctionEnd

Function .onInstSuccess
  !insertmacro MementoSectionSave
  WriteRegStr SHCTX "Software\Shared" "InstallMode" $MultiUser.InstallMode
  WriteRegStr SHCTX "Software\Shared" "InstallDir" $INSTDIR
FunctionEnd

Function un.onUninstSuccess
  DeleteRegValue SHCTX "Software\Shared" "InstallMode"
  DeleteRegValue SHCTX "Software\Shared" "InstallDir"
  DeleteRegKey /ifempty SHCTX "Software\Shared"
FunctionEnd
