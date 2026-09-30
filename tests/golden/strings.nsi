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
  ReadEnvStr $0 "DELAY"
  StrCpy $1 $0
  IntCmp $1 0 __GENERATED_abs_3_done 0 __GENERATED_abs_3_done
  IntOp $1 0 - $1
__GENERATED_abs_3_done:
  Sleep $1
  StrCpy $1 $0
  IntCmp 0 $1 __GENERATED_max_4_next __GENERATED_max_4_next 0
  StrCpy $1 0
__GENERATED_max_4_next:
  Sleep $1
  IntCmp 5 $0 __GENERATED_min_5_next 0 __GENERATED_min_5_next
  StrCpy $0 5
__GENERATED_min_5_next:
  IntCmp 3 $0 __GENERATED_min_6_next 0 __GENERATED_min_6_next
  StrCpy $0 3
__GENERATED_min_6_next:
  DetailPrint $0
SectionEnd
