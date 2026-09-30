Unicode true

!include "MUI2.nsh"

Name $title
Caption "$title Setup"
BrandingText $title
OutFile "late-texts.exe"

Var title
Var site

!insertmacro MUI_PAGE_INSTFILES

!define MUI_FINISHPAGE_SHOWREADME ""
!define MUI_FINISHPAGE_SHOWREADME_TEXT "Visit $site"
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_LANGUAGE "English"

Section "Core"
  DetailPrint $title
SectionEnd

Function .onInit
  StrCpy $title ""
  StrCpy $site ""
  ReadEnvStr $title "TITLE"
  ReadEnvStr $site "SITE"
FunctionEnd
