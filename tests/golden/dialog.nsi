Unicode true

!include "MUI2.nsh"

Name "Dialog"
OutFile "dialog-setup.exe"

Var __GENERATED_ctl_serial
Var __GENERATED_ctl_agree
Var __GENERATED_ctl_flavour
Var __GENERATED_unctl_reason
Var __GENERATED_ctl_badge

!insertmacro MUI_PAGE_WELCOME
Page custom mui.custom.create mui.custom.leave "Registration"
!insertmacro MUI_PAGE_INSTFILES

UninstPage custom un.mui.custom.create
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

Section "Core"
  DetailPrint "installing"
  WriteUninstaller "$INSTDIR\un.exe"
SectionEnd

Section "un.Core"
  DetailPrint "uninstalling"
SectionEnd

Function mui.custom.leave
  System::Call "user32::GetWindowText(p$__GENERATED_ctl_serial,t.s,i${NSIS_MAX_STRLEN})"
  Pop $0
  StrCmpS $0 "" 0 __GENERATED_endif_0
  DetailPrint "no serial"
__GENERATED_endif_0:
  EnableWindow $__GENERATED_ctl_agree 0
  ShowWindow $__GENERATED_ctl_badge 0
FunctionEnd

Function mui.custom.create
  DetailPrint "about to build the dialog"
  !insertmacro MUI_HEADER_TEXT "Serial number" "Enter the key from your invoice."
  nsDialogs::Create 1018
  Pop $0
  StrCmpS $0 "error" __GENERATED_dialog_0_failed 0
  nsDialogs::CreateControl STATIC 0x54000100 0x00000020 0 0u 100% 12u "Serial:"
  Pop $0
  nsDialogs::CreateControl EDIT 0x54010080 0x00000300 0 20u 100% 12u ""
  Pop $__GENERATED_ctl_serial
  nsDialogs::CreateControl BUTTON 0x54012C03 0x00000000 0 36u 100% 12u "I have read the terms"
  Pop $__GENERATED_ctl_agree
  nsDialogs::CreateControl COMBOBOX 0x56210243 0x00000300 0 56u 100% 60u ""
  Pop $__GENERATED_ctl_flavour
  SendMessage $__GENERATED_ctl_flavour 0x0143 0 "STR:Full"
  SendMessage $__GENERATED_ctl_flavour 0x0143 0 "STR:Minimal"
  nsDialogs::CreateControl STATIC 0x54001010 0x00000020 0 120u 100% 2u ""
  Pop $0
  nsDialogs::CreateControl STATIC 0x5400010E 0x00000000 0 90u 100% 20u ""
  Pop $__GENERATED_ctl_badge
  LoadAndSetImage /STRINGID $__GENERATED_ctl_badge 0 0x0010 "check.bmp"
  CreateFont $0 "Tahoma" 8 700
  SendMessage $__GENERATED_ctl_serial 0x0030 $0 1
  SetCtlColors $__GENERATED_ctl_serial 800000 transparent
  SendMessage $__GENERATED_ctl_agree 0x00F1 1 0
  GetDlgItem $0 $HWNDPARENT 2
  EnableWindow $0 0
  nsDialogs::Show
  Return
__GENERATED_dialog_0_failed:
  Abort
FunctionEnd

Function un.mui.custom.create
  !insertmacro MUI_HEADER_TEXT "Before you go" ""
  nsDialogs::Create 1018
  Pop $0
  StrCmpS $0 "error" __GENERATED_dialog_0_failed 0
  nsDialogs::CreateControl STATIC 0x54000100 0x00000020 0 0u 100% 12u "Why are you uninstalling?"
  Pop $0
  nsDialogs::CreateControl EDIT 0x54010080 0x00000300 0 20u 100% 12u ""
  Pop $__GENERATED_unctl_reason
  nsDialogs::CreateControl BUTTON 0x54000007 0x00000020 0 40u 100% 40u "Optional"
  Pop $0
  nsDialogs::Show
  Return
__GENERATED_dialog_0_failed:
  Abort
FunctionEnd
