Unicode true

!define BANNER "Assembled"

!include "MUI2.nsh"

Name "Assembled"
OutFile "assembled-setup.exe"
InstallDir "$PROGRAMFILES\Assembled"

!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

Section "Core" SEC_core
  Push "installing"
  Call announce
  SetOutPath $INSTDIR
SectionEnd

Section "Docs" SEC_docs
  Push "documentation"
  Call announce
SectionEnd

Function announce
  Pop $0
  DetailPrint "${BANNER}: $0"
FunctionEnd
