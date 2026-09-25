Unicode true

!define MEMENTO_REGISTRY_ROOT HKLM
!define MEMENTO_REGISTRY_KEY "Software\Kept\Components"

!include "MUI2.nsh"
!include "Memento.nsh"

Name "Kept"
OutFile "memento.exe"

!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

Section "Core"
  SetOutPath $INSTDIR
SectionEnd

!insertmacro MementoSectionEx "" "Documentation" docs SEC_docs
  SetOutPath $INSTDIR
!insertmacro MementoSectionEnd

!insertmacro MementoSectionEx "/o" "Samples" samples SEC.memento.0
  SetOutPath $INSTDIR
!insertmacro MementoSectionEnd

!insertmacro MementoSectionDone

Function .onInit
  !insertmacro MementoSectionRestore
FunctionEnd

Function .onInstSuccess
  !insertmacro MementoSectionSave
FunctionEnd
