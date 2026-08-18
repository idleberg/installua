Unicode true

Name "Returns"
OutFile "returns-setup.exe"

Section "Core"
  SetOutPath $INSTDIR
  Push $INSTDIR
  Call budget
  Pop $0
  Pop $1
  Push $0
  Push $1
  Push 4
  Call countdown
  Pop $2
  Pop $1
  Pop $0
  DetailPrint "payload: $1 halves, $0 KiB, check $2"
SectionEnd

Function measure
  Pop $0
  StrLen $0 $0
  IntOp $1 $0 / 2
  Push $1
  Push $0
FunctionEnd

Function budget
  Pop $0
  Push $0
  Call measure
  Pop $0
  Pop $1
  IntOp $0 $0 / 1024
  Push $1
  Push $0
FunctionEnd

Function countdown
  Pop $0
  IntCmp $0 0 0 0 __GENERATED_endif_0
  Push 0
  Return
__GENERATED_endif_0:
  Push $0
  IntOp $1 $0 - 1
  Push $1
  Call countdown
  Pop $1
  Pop $0
  IntOp $0 $1 + $0
  Push $0
FunctionEnd
