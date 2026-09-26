Unicode true

!define MEMENTO_REGISTRY_ROOT HKLM
!define MEMENTO_REGISTRY_KEY "Software\Called\Components"

!include "MUI2.nsh"
!include "Memento.nsh"

Name "Called"
OutFile "callbacks.exe"

!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES

!insertmacro MUI_UNPAGE_DIRECTORY
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

Section "Core"
  SetOutPath $INSTDIR
  WriteUninstaller "$INSTDIR\uninstall.exe"
SectionEnd

!insertmacro MementoSectionEx "" "Documentation" docs SEC_docs
  SetOutPath $INSTDIR
!insertmacro MementoSectionEnd

Section "un.Uninstall"
  Delete "$INSTDIR\uninstall.exe"
  RMDir $INSTDIR
SectionEnd

!insertmacro MementoSectionDone

Function .onInit
  !insertmacro MementoSectionRestore
  SectionGetFlags ${SEC_docs} $0
  IntOp $1 1 ~
  IntOp $0 $0 & $1
  SectionSetFlags ${SEC_docs} $0
FunctionEnd

Function .onInstSuccess
  !insertmacro MementoSectionSave
  DetailPrint "installed"
FunctionEnd

Function .onInstFailed
  DetailPrint "failed"
FunctionEnd

Function .onVerifyInstDir
  IfFileExists "$INSTDIR\.." 0 __GENERATED_then_0
  Return
__GENERATED_then_0:
  Abort
FunctionEnd

Function .onGUIEnd
  DetailPrint "closing"
FunctionEnd

Function .onSelChange
  SectionGetFlags ${SEC_docs} $0
  IntOp $0 $0 & 1
  StrCmpS $0 1 0 __GENERATED_endif_0
  DetailPrint "documentation"
__GENERATED_endif_0:
FunctionEnd

Function .onRebootFailed
  DetailPrint "reboot by hand"
FunctionEnd

Function un.onInit
  DetailPrint "starting"
FunctionEnd

Function un.onUninstSuccess
  DetailPrint "removed"
FunctionEnd

Function un.onUninstFailed
  DetailPrint "failed"
FunctionEnd

Function un.onVerifyInstDir
  IfFileExists "$INSTDIR\uninstall.exe" 0 __GENERATED_then_0
  Return
__GENERATED_then_0:
  Abort
FunctionEnd

Function un.onGUIEnd
  DetailPrint "closing"
FunctionEnd

Function un.onSelChange
  DetailPrint "changed"
FunctionEnd

Function un.onRebootFailed
  DetailPrint "reboot by hand"
FunctionEnd
