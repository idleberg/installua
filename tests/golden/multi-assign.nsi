Unicode true

Name "Multi"
OutFile "multi-assign.exe"

Var last

Section "Core"
  nsExec::Exec "a.exe"
  Pop $0
  StrCpy $1 ""
  nsExec::ExecToStack "b.exe $0"
  Pop $0
  Pop $1
  DetailPrint "$0$1"
  nsExec::ExecToStack "c.exe"
  Pop $0
  Pop $last
  DetailPrint $0
SectionEnd

Function .onInit
  StrCpy $last ""
FunctionEnd
