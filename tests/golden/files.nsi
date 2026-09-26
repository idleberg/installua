Unicode true

!include "MUI2.nsh"

Name "Files"
OutFile "files-setup.exe"

!insertmacro MUI_PAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

Section ""
  SetOutPath $INSTDIR
  File "LICENSE.txt"
  AllowSkipFiles off
  File /nonfatal "LICENSE.txt"
  AllowSkipFiles on
  File "LICENSE.txt"
SectionEnd
