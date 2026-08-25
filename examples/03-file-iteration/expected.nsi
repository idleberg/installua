; Hand-written expected output for install.lua — PLAN Phase 0, program 3 of five.
;
; Label numbering is one counter per body, incremented per construct in source
; order, as `_generated_while_7_top` implies. Constructs 1, 2 and 5 fuse
; into jumps at labels the enclosing loop already owns and emit none themselves.

Unicode true

!define APP "Example3"

!include "MUI2.nsh"
!include "TextFunc.nsh"

Name "${APP}"
OutFile "${APP}-setup.exe"
InstallDir "$PROGRAMFILES64\${APP}"

!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

Section "Core"
  SetOutPath "$INSTDIR"
  File "assets\manifest.txt"
  File "assets\notes.txt"
  FileOpen $0 "$INSTDIR\manifest.txt" r
  StrCpy $1 0
_generated_for_0_top:
  ClearErrors
  FileRead $0 $2
  IfErrors _generated_for_0_end 0
  ${TrimNewLines} $2 $2
  StrCmpS $2 "" _generated_for_0_top 0
  StrCmpS $2 "END" _generated_for_0_end 0
  IntOp $1 $1 + 1
  DetailPrint "entry $1: $2"
  Goto _generated_for_0_top
_generated_for_0_end:
  FileClose $0
  IntCmp $1 0 0 _generated_endif_3 _generated_endif_3
  Abort "the manifest is empty"
_generated_endif_3:
  DetailPrint "read $1 entries"
  StrCpy $3 1
_generated_for_4_top:
  IntCmp $3 3 0 0 _generated_for_4_end
  ClearErrors
  CreateDirectory "$INSTDIR\cache"
  IfErrors 0 _generated_for_4_end
  DetailPrint "cache directory attempt $3 failed"
  Sleep 200
  IntOp $3 $3 + 1
  Goto _generated_for_4_top
_generated_for_4_end:
SectionEnd
