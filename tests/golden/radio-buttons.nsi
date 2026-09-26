Unicode true

!include "MUI2.nsh"
!include "Sections.nsh"

Name "Radio"
OutFile "radio-buttons.exe"

Var __GENERATED_radio_small

!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

Section "Small" SEC_small
SectionEnd

Section /o "Large" SEC_large
SectionEnd

Function .onSelChange
  !insertmacro StartRadioButtons $__GENERATED_radio_small
  !insertmacro RadioButton ${SEC_small}
  !insertmacro RadioButton ${SEC_large}
  !insertmacro EndRadioButtons
  SectionGetFlags ${SEC_large} $0
  IntOp $0 $0 & 1
  StrCmpS $0 1 0 __GENERATED_endif_0
  DetailPrint "large"
__GENERATED_endif_0:
FunctionEnd

Function .onInit
  StrCpy $__GENERATED_radio_small ${SEC_small}
FunctionEnd
