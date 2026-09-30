Unicode true

!include "StrFunc.nsh"

Name "Strings"
OutFile "strings-setup.exe"

${Using:StrFunc} StrLoc

Section "Core"
  ${StrLoc} $0 "ftp://host" "ftp" ">"
  StrCmp $0 "" 0 __GENERATED_find_0_hit
  StrCpy $0 -1
__GENERATED_find_0_hit:
  IntOp $0 $0 + 1
  DetailPrint $0
  ${StrLoc} $0 "ftp://host" "://" ">"
  StrCmp $0 "" 0 __GENERATED_find_1_hit
  StrCpy $0 -1
__GENERATED_find_1_hit:
  IntOp $0 $0 + 1
  DetailPrint $0
  ${StrLoc} $0 "ftp.example.com" "://" ">"
  StrCmp $0 "" 0 __GENERATED_find_2_hit
  StrCpy $0 -1
__GENERATED_find_2_hit:
  IntOp $0 $0 + 1
  DetailPrint $0
SectionEnd
