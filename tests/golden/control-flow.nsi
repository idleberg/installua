Unicode true

Name "Control Flow"
OutFile "control-flow-setup.exe"

Var state

Function report
  DetailPrint "state is $state"
FunctionEnd

Section "Core"
  StrCpy $0 "$INSTDIR/app.exe"
  IfFileExists $0 0 _luagen_false_0
  StrCpy $1 1
  Goto _luagen_bool_0
_luagen_false_0:
  StrCpy $1 0
_luagen_bool_0:
  StrCmpS $1 1 0 _luagen_else_1
  IfSilent _luagen_else_1 0
  DetailPrint "upgrading $INSTDIR"
  Goto _luagen_endif_1
_luagen_else_1:
  DetailPrint "installing $INSTDIR"
_luagen_endif_1:
  StrLen $R9 $0
  IntOp $2 $R9 / 2
  StrCpy $3 1
_luagen_while_3_top:
  IntCmpU $3 $2 0 0 _luagen_while_3_end
  IntOp $3 $3 + 1
  IntCmpU $3 3 0 _luagen_endif_4 _luagen_endif_4
  Goto _luagen_while_3_top
_luagen_endif_4:
  IntCmpU $3 8 _luagen_endif_5 _luagen_endif_5 0
  Goto _luagen_while_3_end
_luagen_endif_5:
  DetailPrint "step $3"
  Goto _luagen_while_3_top
_luagen_while_3_end:
  StrCpy $4 1
_luagen_for_6_top:
  IntCmpU $4 3 0 0 _luagen_for_6_end
  DetailPrint "pass $4"
  IntOp $4 $4 + 1
  Goto _luagen_for_6_top
_luagen_for_6_end:
  StrCpy $state "installed"
  Call report
SectionEnd
