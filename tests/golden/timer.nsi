Unicode true

!include "MUI2.nsh"

Name "Timer"
OutFile "timer-setup.exe"

Var __GENERATED_ctl_bar

Page custom mui.custom.create
!insertmacro MUI_PAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

Section "Core"
SectionEnd

Function mui.control.click
  Pop $0
  SendMessage $__GENERATED_ctl_bar 1026 0 0
  GetFunctionAddress $0 mui.timer.tick
  nsDialogs::CreateTimer $0 200
FunctionEnd

Function mui.timer.tick
  SendMessage $__GENERATED_ctl_bar 1032 0 0 $0
  IntCmp $0 100 0 __GENERATED_else_0 0
  GetFunctionAddress $0 mui.timer.tick
  nsDialogs::KillTimer $0
  Goto __GENERATED_endif_0
__GENERATED_else_0:
  SendMessage $__GENERATED_ctl_bar 1027 2 0
__GENERATED_endif_0:
FunctionEnd

Function mui.timer
  DetailPrint "tick"
FunctionEnd

Function mui.custom.create
  nsDialogs::Create 1018
  Pop $0
  StrCmpS $0 "error" __GENERATED_dialog_0_failed 0
  nsDialogs::CreateControl msctls_progress32 0x54000000 0x00000300 0 0u 100% 12u ""
  Pop $__GENERATED_ctl_bar
  nsDialogs::CreateControl BUTTON 0x54010000 0x00000000 0 20u 100% 14u "Again"
  Pop $0
  GetFunctionAddress $1 mui.control.click
  nsDialogs::OnClick $0 $1
  GetFunctionAddress $0 mui.timer.tick
  nsDialogs::CreateTimer $0 200
  GetFunctionAddress $0 mui.timer
  nsDialogs::CreateTimer $0 1000
  nsDialogs::Show
  Return
__GENERATED_dialog_0_failed:
  Abort
FunctionEnd
