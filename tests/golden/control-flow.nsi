Unicode true

Name "Control Flow"
OutFile "control-flow-setup.exe"

Var state

Function report
  DetailPrint "state is $state"
FunctionEnd

Section "Core"
  StrCpy $0 "$INSTDIR/app.exe"
  IfFileExists $0 0 __GENERATED_false_0
  StrCpy $1 1
  Goto __GENERATED_bool_0
__GENERATED_false_0:
  StrCpy $1 0
__GENERATED_bool_0:
  StrCmpS $1 1 0 __GENERATED_else_1
  IfSilent __GENERATED_else_1 0
  DetailPrint "upgrading $INSTDIR"
  Goto __GENERATED_endif_1
__GENERATED_else_1:
  DetailPrint "installing $INSTDIR"
__GENERATED_endif_1:
  StrLen $0 $0
  IntOp $0 $0 / 2
  StrCpy $1 1
__GENERATED_while_3_top:
  IntCmpU $1 $0 0 0 __GENERATED_while_3_end
  IntOp $1 $1 + 1
  IntCmpU $1 3 0 __GENERATED_endif_4 __GENERATED_endif_4
  Goto __GENERATED_while_3_top
__GENERATED_endif_4:
  IntCmpU $1 8 __GENERATED_endif_5 __GENERATED_endif_5 0
  Goto __GENERATED_while_3_end
__GENERATED_endif_5:
  DetailPrint "step $1"
  Goto __GENERATED_while_3_top
__GENERATED_while_3_end:
  StrCpy $0 1
__GENERATED_for_6_top:
  IntCmpU $0 3 0 0 __GENERATED_for_6_end
  DetailPrint "pass $0"
  IntOp $0 $0 + 1
  Goto __GENERATED_for_6_top
__GENERATED_for_6_end:
  StrCpy $state "installed"
  Call report
SectionEnd
