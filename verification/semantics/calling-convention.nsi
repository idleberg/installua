; Hand-written expected output for install.lua — PLAN Phase 0, program 4 of five.
;
; Calling convention, as this program pins it:
;   * arguments are pushed in **reverse source order**, so the callee's first
;     `Pop` is the first parameter;
;   * returns are pushed in **reverse source order**, so the caller's first
;     `Pop` is the first return value;
;   * caller-saves are pushed **before** the arguments, ascending register
;     number, and restored in reverse — so the callee's result sits on top when
;     it returns and the restores fall out underneath it.
;
; Clobber sets, from the fixpoint over the call graph's SCC condensation:
;   measure   {$0,$1,$2,$3}
;   budget    {$0,$1,$2,$3}   -- its own {$0,$1,$2} plus measure's
;   countdown {$0,$1,$2}      -- the recursive edge adds nothing; one extra round

Unicode true

!define APP "Example4"

!include "FileFunc.nsh"

Name "${APP}"
OutFile "check.exe"
SilentInstall silent
RequestExecutionLevel user
InstallDir "$PROGRAMFILES64\${APP}"



Function measure
  Pop $0
  ${GetSize} "$0" "" $1 $2 $3
  Push $2
  Push $1
FunctionEnd

Function budget
  Pop $0
  Push $0
  Call measure
  Pop $1
  Pop $2
  IntOp $1 $1 / 1024
  Push $2
  Push $1
FunctionEnd

Function countdown
  Pop $0
  IntCmp $0 0 0 0 _generated_endif_0
  Push 0
  Return
_generated_endif_0:
  Push $0
  IntOp $1 $0 - 1
  Push $1
  Call countdown
  Pop $2
  Pop $0
  IntOp $2 $2 + $0
  Push $2
FunctionEnd

Section "Core"
  SetOutPath "$TEMP\p4out"
  File "assets\payload.bin"
  Push "$TEMP\p4out"
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
  FileOpen $4 "$EXEDIR\p4-result.txt" w
  FileWrite $4 "files=$1 kib=$0 countdown4=$2$\r$\n"
  FileClose $4
SectionEnd
