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
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!define MUI_UNCONFIRMPAGE_TEXT_TOP "Pages will be removed."
!define MUI_UNCONFIRMPAGE_TEXT_LOCATION "From:"
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

Function mui.welcome.pre
  DetailPrint "about to greet"
FunctionEnd

Function mui.directory.leave
  DetailPrint "leaving the program folder page"
FunctionEnd

Function .onInit
  StrCpy $dataDir ""
FunctionEnd

Section "Core"
  WriteUninstaller "$INSTDIR\un.exe"
SectionEnd

Section "un.Core"
  Delete "$INSTDIR\un.exe"
SectionEnd
