Unicode true

!include "MUI2.nsh"

Name "Files"
OutFile "files-setup.exe"
SetOverwrite ifnewer

!insertmacro MUI_PAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

Section ""
  SetOutPath $INSTDIR
  File "LICENSE.txt"
  AllowSkipFiles off
  File /nonfatal "LICENSE.txt"
  AllowSkipFiles on
  File "LICENSE.txt"
  AllowSkipFiles off
  SetOverwrite off
  File "LICENSE.txt"
  SetOverwrite ifnewer
  AllowSkipFiles on
  File /oname=COPYING "LICENSE.txt"
  File "/oname=$INSTDIR\docs\read me.txt" "LICENSE.txt"
SectionEnd
