; PLAN Phase 0, task 1b — the large-string plugin hazard.
;
; Built twice from this one file: once with vanilla makensis
; (NSIS_MAX_STRLEN=1024) and once with the official strlen_8192 special build.
; Both link the *same* stock plugin DLLs from the standard distribution, which
; are compiled against 1024. That is the hazard this probe exists to measure.
;
; Every probe reports a length, so silent truncation is visible as a number
; rather than needing a crash.

Unicode true
Name "strlen probe"
OutFile "$%PROBE_OUT%"
RequestExecutionLevel user
SilentInstall silent

!include "LogicLib.nsh"

Var LONG

!macro LOG text
  FileWrite $9 "${text}$\r$\n"
!macroend

Section
  FileOpen $9 "$EXEDIR\probe-result.txt" w
  !insertmacro LOG "NSIS_MAX_STRLEN at build = ${NSIS_MAX_STRLEN}"

  ; ---------------------------------------------------------------- build 1500
  ; Assembled at runtime, 34 chars at a time, so the *literal* limit is not what
  ; is under test. Pure NSIS instructions only.
  StrCpy $LONG ""
  StrCpy $0 0
  ${While} $0 < 50
    StrCpy $LONG "$LONGabcdefghijklmnopqrstuvwxyz0123"
    IntOp $0 $0 + 1
  ${EndWhile}
  StrLen $1 $LONG
  !insertmacro LOG "A. pure NSIS StrLen             = $1"

  ; ------------------------------------------- System.dll reads an NSIS variable
  ; `w r0` marshals $0 out to a wide C string. If System.dll sized that buffer
  ; from a hardcoded 1024 rather than from extra_parameters->string_size, this
  ; is where it truncates or overruns.
  StrCpy $0 $LONG
  System::Call 'kernel32::lstrlenW(w r0) i .r2'
  !insertmacro LOG "B. System reads var, lstrlenW   = $2"

  ; ------------------------------------------ System.dll writes an NSIS variable
  ; The other direction, and the more dangerous one: the plugin writes into the
  ; installer's variable block, whose stride *is* g_stringsize.
  System::Alloc 40000
  Pop $3
  System::Call 'kernel32::lstrcpyW(p r3, w r0)'
  System::Call 'kernel32::lstrlenW(p r3) i .r4'
  !insertmacro LOG "C. System copy-out, lstrlenW    = $4"

  System::Call '*$3(&w19999 .r5)'
  StrLen $6 $5
  !insertmacro LOG "D. System writes var, StrLen    = $6"
  System::Free $3

  ; ---------------------------------------------------- the same, via the stack
  StrCpy $R5 ""
  Push $LONG
  System::Store "s"
  System::Store "l"
  Pop $R6
  StrLen $R7 $R6
  !insertmacro LOG "E. System stack round trip      = $R7"

  ; ------------------------------------------------------------- nsExec on stack
  ; A second plugin, different vintage, and its output crosses the stack.
  nsExec::ExecToStack 'cmd /c echo $LONG'
  Pop $7
  Pop $8
  StrLen $R0 $8
  !insertmacro LOG "F. nsExec rc                    = $7"
  !insertmacro LOG "G. nsExec output StrLen         = $R0"

  ; ------------------------------------------------------------------ stack ABI
  ; The plugin stack is shared with the installer: push long, run a plugin that
  ; uses the stack itself, and check the entry underneath survived.
  Push $LONG
  nsExec::ExecToStack 'cmd /c ver'
  Pop $R1
  Pop $R2
  Pop $R3
  StrLen $R4 $R3
  !insertmacro LOG "H. stack survives a plugin      = $R4"

  !insertmacro LOG "done"
  FileClose $9
SectionEnd
