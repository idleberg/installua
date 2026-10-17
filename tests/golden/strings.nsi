Unicode true

Name "Strings"
OutFile "strings-setup.exe"

Section "Core"
  StrCpy $0 "ftp://host"
  StrCpy $1 "ftp.example.com"
  StrCpy $2 0
__GENERATED_find_0:
  StrCpy $3 $0 3 $2
  StrCmpS $3 "ftp" 0 __GENERATED_find_0_end
  IntOp $2 $2 + 1
  Goto __GENERATED_find_0_done
__GENERATED_find_0_end:
  StrCmp $3 "" 0 __GENERATED_find_0_step
  StrCpy $2 0
__GENERATED_find_0_done:
  DetailPrint $2
  StrCpy $2 0
__GENERATED_find_1:
  StrCpy $3 $0 3 $2
  StrCmpS $3 "://" 0 __GENERATED_find_1_end
  IntOp $2 $2 + 1
  Goto __GENERATED_find_1_done
__GENERATED_find_1_end:
  StrCmp $3 "" 0 __GENERATED_find_1_step
  StrCpy $2 0
__GENERATED_find_1_done:
  DetailPrint $2
  StrCpy $2 0
__GENERATED_find_2:
  StrCpy $3 $1 3 $2
  StrCmpS $3 "://" 0 __GENERATED_find_2_end
  IntOp $1 $2 + 1
  Goto __GENERATED_find_2_done
__GENERATED_find_2_end:
  StrCmp $3 "" 0 __GENERATED_find_2_step
  StrCpy $1 0
__GENERATED_find_2_done:
  DetailPrint $1
  StrCpy $1 0
__GENERATED_find_3:
  StrCpy $2 $0 3 $1
  StrCmpS $2 "FTP" 0 __GENERATED_find_3_end
  IntOp $0 $1 + 1
  Goto __GENERATED_find_3_done
__GENERATED_find_3_end:
  StrCmp $2 "" 0 __GENERATED_find_3_step
  StrCpy $0 0
__GENERATED_find_3_done:
  DetailPrint $0
  StrCpy $0 "installua"
  StrCpy $1 $0 3 1
  DetailPrint $1
  StrCpy $1 $0 "" -1
  DetailPrint $1
  StrCpy $1 $0 -1 1
  DetailPrint $1
  StrCpy $1 $0 -1 -3
  DetailPrint $1
  StrLen $1 $0
  IntOp $1 7 - $1
  IntCmp $1 0 0 __GENERATED_sub_4_done 0
  StrCpy $1 ""
__GENERATED_sub_4_done:
  StrCpy $1 $0 $1 -4
  DetailPrint $1
  StrCpy $0 $0 3 0
  DetailPrint $0
  ReadEnvStr $0 "DELAY"
  StrCpy $1 $0
  IntCmp $1 0 __GENERATED_abs_5_done 0 __GENERATED_abs_5_done
  IntOp $1 0 - $1
__GENERATED_abs_5_done:
  Sleep $1
  StrCpy $1 $0
  IntCmp 0 $1 __GENERATED_max_6_next __GENERATED_max_6_next 0
  StrCpy $1 0
__GENERATED_max_6_next:
  Sleep $1
  IntCmp 5 $0 __GENERATED_min_7_next 0 __GENERATED_min_7_next
  StrCpy $0 5
__GENERATED_min_7_next:
  IntCmp 3 $0 __GENERATED_min_8_next 0 __GENERATED_min_8_next
  StrCpy $0 3
__GENERATED_min_8_next:
  DetailPrint $0
  Return
__GENERATED_find_3_step:
  IntOp $1 $1 + 1
  Goto __GENERATED_find_3
__GENERATED_find_2_step:
  IntOp $2 $2 + 1
  Goto __GENERATED_find_2
__GENERATED_find_1_step:
  IntOp $2 $2 + 1
  Goto __GENERATED_find_1
__GENERATED_find_0_step:
  IntOp $2 $2 + 1
  Goto __GENERATED_find_0
SectionEnd
