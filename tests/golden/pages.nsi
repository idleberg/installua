Unicode true

!include "MUI2.nsh"

OutFile "pages.exe"
Name "Pages"

Var dataDir

!define MUI_COMPONENTSPAGE_CHECKBITMAP "check.bmp"
!define MUI_INSTFILESPAGE_COLORS "FFFFFF 000000"
!define MUI_INSTFILESPAGE_PROGRESSBAR "smooth"
!define MUI_LICENSEPAGE_BGCOLOR "/windows"

!define MUI_PAGE_CUSTOMFUNCTION_PRE "mui.welcome.pre"
!insertmacro MUI_PAGE_WELCOME

!define MUI_LICENSEPAGE_TEXT_BOTTOM "Read it, then choose."
!define MUI_LICENSEPAGE_BUTTON "Agree"
!define MUI_LICENSEPAGE_CHECKBOX
!define MUI_LICENSEPAGE_CHECKBOX_TEXT "I accept the terms."
!insertmacro MUI_PAGE_LICENSE "LICENSE.txt"

!define MUI_COMPONENTSPAGE_TEXT_TOP "Pick the parts you want."
!define MUI_COMPONENTSPAGE_TEXT_INSTTYPE "Preset:"
!define MUI_COMPONENTSPAGE_TEXT_COMPLIST "Parts:"
!define MUI_PAGE_HEADER_TEXT "Components"
!define MUI_PAGE_HEADER_SUBTEXT "Choose what to install."
!insertmacro MUI_PAGE_COMPONENTS

!define MUI_DIRECTORYPAGE_TEXT_TOP "Choose where the program goes."
!define MUI_DIRECTORYPAGE_TEXT_DESTINATION "Program folder:"
!define MUI_DIRECTORYPAGE_VERIFYONLEAVE
!define MUI_PAGE_CUSTOMFUNCTION_LEAVE "mui.directory.leave"
!insertmacro MUI_PAGE_DIRECTORY

!define MUI_DIRECTORYPAGE_TEXT_TOP "And where the data goes."
!define MUI_DIRECTORYPAGE_VARIABLE $dataDir
!insertmacro MUI_PAGE_DIRECTORY
Page custom mui.custom.create mui.custom.leave "Registration"
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!define MUI_UNCONFIRMPAGE_TEXT_TOP "Pages will be removed."
!define MUI_UNCONFIRMPAGE_TEXT_LOCATION "From:"
!insertmacro MUI_UNPAGE_CONFIRM
UninstPage custom un.mui.custom.create
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

Section "Core"
  WriteUninstaller "$INSTDIR\un.exe"
SectionEnd

Section "un.Core"
  Delete "$INSTDIR\un.exe"
SectionEnd

Function mui.welcome.pre
  DetailPrint "about to greet"
FunctionEnd

Function mui.directory.leave
  DetailPrint "leaving the program folder page"
FunctionEnd

Function mui.custom.leave
  DetailPrint "leaving the dialog"
FunctionEnd

Function mui.custom.create
  DetailPrint "about to build the dialog"
  !insertmacro MUI_HEADER_TEXT "Serial number" "Enter the key from your invoice."
  nsDialogs::Create 1018
  Pop $0
  StrCmpS $0 "error" __GENERATED_dialog_0_failed 0
  DetailPrint "the dialog is up"
  nsDialogs::Show
  Return
__GENERATED_dialog_0_failed:
  Abort
FunctionEnd

Function un.mui.custom.create
  nsDialogs::Create 1018
  Pop $0
  StrCmpS $0 "error" __GENERATED_dialog_0_failed 0
  nsDialogs::Show
  Return
__GENERATED_dialog_0_failed:
  Abort
FunctionEnd

Function .onInit
  StrCpy $dataDir ""
FunctionEnd
