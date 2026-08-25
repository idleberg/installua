Unicode true

!define LOG "/install.log"
!define CONF "/app.conf"
!define DIST "/app.conf.new"

!include "FileFunc.nsh"
!include "TextFunc.nsh"

Name "Walkers"
OutFile "walkers-setup.exe"

Section "Clean"
  ${Locate} $INSTDIR "/L=F /M=*.tmp" __GENERATED_callback_locate_0
  ${Locate} $INSTDIR "/L=D /M=cache" __GENERATED_callback_locate_1
SectionEnd

Section "Survey"
  ${GetDrives} "ALL" __GENERATED_callback_getdrives_2
  ${FileReadFromEnd} "$INSTDIR\install.log" __GENERATED_callback_filereadfromend_3
SectionEnd

Section "Diff"
  ${TextCompare} "$INSTDIR\app.conf" "$INSTDIR\app.conf.new" "FastDiff" __GENERATED_callback_textcompare_4
  ${LineFind} "$INSTDIR\app.conf" "$INSTDIR\app.conf.new" "1:-1" __GENERATED_callback_linefind_5
  ${TextCompareS} "$INSTDIR\app.conf" "$INSTDIR\app.conf.new" "FastEqual" __GENERATED_callback_textcompares_6
SectionEnd

Function __GENERATED_callback_locate_0
  Push $R6
  Push $R7
  Push $R8
  Push $R9
  Pop $0
  Pop $1
  Pop $2
  Pop $3
  IntCmpU $3 1048576 __GENERATED_endif_0 __GENERATED_endif_0 0
  DetailPrint "large leftover: $2 in $1"
__GENERATED_endif_0:
  Delete $0
  Push ""
FunctionEnd

Function __GENERATED_callback_locate_1
  Push $R9
  Pop $0
  RMDir $0
  Push ""
FunctionEnd

Function __GENERATED_callback_getdrives_2
  Push $8
  Push $9
  Pop $0
  Pop $1
  StrCmpS $1 "CDROM" 0 __GENERATED_endif_0
  DetailPrint "skipping $0"
  Push "StopGetDrives"
  Return
__GENERATED_endif_0:
  DetailPrint "$0 is a $1"
  Push ""
FunctionEnd

Function __GENERATED_callback_filereadfromend_3
  Push $8
  Push $9
  Pop $0
  Pop $1
  StrCmpS $0 "" 0 __GENERATED_endif_0
  Goto __GENERATED_continue
__GENERATED_endif_0:
  DetailPrint "$1: $0"
__GENERATED_continue:
  Push ""
FunctionEnd

Function __GENERATED_callback_textcompare_4
  Push $6
  Push $7
  Push $8
  Push $9
  Pop $0
  Pop $1
  Pop $2
  Pop $3
  IntCmpU $3 0 0 __GENERATED_else_0 __GENERATED_else_0
  DetailPrint "only ours, line $1: $0"
  Goto __GENERATED_endif_0
__GENERATED_else_0:
  DetailPrint "theirs: $2"
__GENERATED_endif_0:
  Push ""
FunctionEnd

Function __GENERATED_callback_linefind_5
  Push $R8
  Push $R9
  Pop $0
  Pop $1
  IntCmpU $1 500 __GENERATED_endif_0 __GENERATED_endif_0 0
  Push "StopLineFind"
  Return
__GENERATED_endif_0:
  StrCmpS $0 "DEBUG=1" 0 __GENERATED_endif_1
  Push "SkipWrite"
  Return
__GENERATED_endif_1:
  StrCpy $R9 $0
  Push ""
FunctionEnd

Function __GENERATED_callback_textcompares_6
  Push $9
  Pop $0
  DetailPrint "same: $0"
  Push ""
FunctionEnd
