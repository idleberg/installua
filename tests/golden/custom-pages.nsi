!pragma warning disable 8000

Unicode true

!include "MUI2.nsh"

OutFile "custom-pages.exe"

!define MUI_DISABLE_INSERT_LANGUAGE_AFTER_PAGES_WARNING

Page custom mui.custom.create "" "One"

!insertmacro MUI_LANGUAGE "English"

Section "-x"
SectionEnd

Function mui.custom.create
  nsDialogs::Create 1018
  Pop $0
  StrCmpS $0 "error" __GENERATED_dialog_0_failed 0
  nsDialogs::Show
  Return
__GENERATED_dialog_0_failed:
  Abort
FunctionEnd
