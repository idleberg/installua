Unicode true

!include "MUI2.nsh"

Name "Sections"
OutFile "sections-setup.exe"

!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

InstType "Full"
InstType "Minimal"

Section "Core" SEC_core
  SectionIn RO
  DetailPrint "core"
SectionEnd

SectionGroup "Tools" SEC_tools
  Section /o "Profiler" SEC_profiler
    DetailPrint "profiler"
  SectionEnd
SectionGroupEnd

Section "Docs" SEC_docs
  DetailPrint "docs"
SectionEnd

Function .onInit
  SetCurInstType 1
  InstTypeSetText 0 "Everything"
  InstTypeGetText 0 $0
  DetailPrint $0
  SectionSetText ${SEC_docs} ""
  SectionGetFlags ${SEC_profiler} $0
  IntOp $0 $0 | 1
  SectionSetFlags ${SEC_profiler} $0
  SectionGetFlags ${SEC_profiler} $0
  IntOp $1 $0 & 1
  SectionGetFlags ${SEC_tools} $0
  IntOp $2 32 ~
  IntOp $0 $0 & $2
  IntOp $1 $1 << 5
  IntOp $0 $0 | $1
  SectionSetFlags ${SEC_tools} $0
  SectionGetFlags ${SEC_core} $0
  IntOp $0 $0 >>> 4
  IntOp $0 $0 & 1
  StrCmpS $0 1 0 __GENERATED_endif_0
  DetailPrint "core cannot be unticked"
__GENERATED_endif_0:
  SectionSetSize ${SEC_docs} 4096
  SectionGetSize ${SEC_docs} $0
  DetailPrint "docs charges $0 KB"
  SectionSetInstTypes ${SEC_docs} 1
FunctionEnd
