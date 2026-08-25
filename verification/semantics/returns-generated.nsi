; **Compiler output**, not a hand-written expectation — Phase 3.
;
; Produced by `installua build` from the source in this file's RESULTS.md entry,
; with three lines changed by hand so the answers land somewhere readable:
; `SilentInstall`/`RequestExecutionLevel` added, and the final `DetailPrint`
; replaced by a `FileOpen`/`FileWrite`/`FileClose`. Everything else — every
; `Push`, every `Pop`, every register number — is generated.
;
; That is the whole point of running it. Program 4's README calls this the one
; program where assembling proves nothing: a caller-save restored one `Exch` out
; of place produces an installer that runs and gives a wrong answer.

Unicode true

Name "Returns"
OutFile "returns.exe"
SilentInstall silent
RequestExecutionLevel user

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

Section "Core"
  Push "abcdefgh"
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
  FileOpen $3 "$EXEDIR\returns-result.txt" w
  FileWrite $3 "kib=$0 halves=$1 countdown4=$2$\r$\n"
  FileClose $3
SectionEnd
