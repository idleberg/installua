!pragma warning disable 8000

Unicode true

!include "MUI2.nsh"

OutFile "custom-pages.exe"

!define MUI_DISABLE_INSERT_LANGUAGE_AFTER_PAGES_WARNING

Page custom mui.custom.create "" "One"
Page custom mui.custom.create.2 "" "Two"

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

Function mui.custom.create.2
  StartMenu::Select "Example"
  Pop $0
  StrCpy $1 ""
  StrCmpS $0 "success" 0 __GENERATED_tail_0
  Pop $1
__GENERATED_tail_0:
FunctionEnd
