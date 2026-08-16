Unicode true

!include "TextFunc.nsh"

Name "Overlay examples"
OutFile "examples.exe"

Section "Abort"
  Abort "stopped"
SectionEnd

Section "ClearErrors"
  ClearErrors
SectionEnd

Section "CreateDirectory"
  CreateDirectory "$INSTDIR\logs"
SectionEnd

Section "CreateShortcut"
  CreateShortcut "$DESKTOP\App.lnk" "$INSTDIR\app.exe"
SectionEnd

Section "DeleteRegKey"
  DeleteRegKey HKLM "Software\Example"
SectionEnd

Section "Delete"
  Delete "$INSTDIR\old.txt"
SectionEnd

Section "DetailPrint"
  DetailPrint "installing"
SectionEnd

Section "File"
  File "assets\icon.ico"
SectionEnd

Section "FileClose"
  FileOpen $0 "$INSTDIR\log.txt" "w"
  FileClose $0
SectionEnd

Section "FileOpen"
  FileOpen $0 "$INSTDIR\log.txt" "w"
  FileClose $0
SectionEnd

Section "FileRead"
  FileOpen $0 "$INSTDIR\log.txt" "r"
__GENERATED_for_0_top:
  ClearErrors
  FileRead $0 $1
  IfErrors 0 __GENERATED_for_0_body
  FileClose $0
  Return
__GENERATED_for_0_body:
  ${TrimNewLines} $1 $1
  DetailPrint $1
  Goto __GENERATED_for_0_top
SectionEnd

Section "FileWrite"
  FileOpen $0 "$INSTDIR\log.txt" "w"
  FileWrite $0 "done"
  FileClose $0
SectionEnd

Section "IfErrors"
  ClearErrors
  IfErrors 0 __GENERATED_endif_0
  DetailPrint "failed"
__GENERATED_endif_0:
SectionEnd

Section "IfFileExists"
  IfFileExists "$INSTDIR\app.exe" 0 __GENERATED_endif_0
  DetailPrint "present"
__GENERATED_endif_0:
SectionEnd

Section "IfSilent"
  IfSilent 0 __GENERATED_endif_0
  DetailPrint "quiet"
__GENERATED_endif_0:
SectionEnd

Section "MessageBox"
  MessageBox MB_OK "finished"
SectionEnd

Section "Quit"
  Quit
SectionEnd

Section "ReadRegStr"
  ReadRegStr $0 HKLM "Software\Example" "Path"
  DetailPrint $0
SectionEnd

Section "RMDir"
  RMDir $INSTDIR
SectionEnd

Section "SetOutPath"
  SetOutPath $INSTDIR
SectionEnd

Section "Sleep"
  Sleep 500
SectionEnd

Section "StrLen"
  StrLen $0 "abc"
  DetailPrint "len $0"
SectionEnd

Section "WriteRegDWORD"
  WriteRegDWORD HKLM "Software\Example" "Build" 42
SectionEnd

Section "WriteRegStr"
  WriteRegStr HKLM "Software\Example" "Path" $INSTDIR
SectionEnd

Section "WriteUninstaller"
  WriteUninstaller "$INSTDIR\uninstall.exe"
SectionEnd

Section "un.Uninstall"
  RMDir $INSTDIR
SectionEnd
