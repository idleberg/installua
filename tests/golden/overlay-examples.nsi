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

Section "CopyFiles"
  CopyFiles "$INSTDIR\data" "$INSTDIR\backup"
SectionEnd

Section "CreateDirectory"
  CreateDirectory "$INSTDIR\logs"
SectionEnd

Section "CreateShortcut"
  CreateShortcut "$DESKTOP\App.lnk" "$INSTDIR\app.exe"
SectionEnd

Section "DeleteINISec"
  DeleteINISec "$INSTDIR\app.ini" "Settings"
SectionEnd

Section "DeleteINIStr"
  DeleteINIStr "$INSTDIR\app.ini" "Settings" "Path"
SectionEnd

Section "DeleteRegKey"
  DeleteRegKey HKLM "Software\Example"
SectionEnd

Section "DeleteRegValue"
  DeleteRegValue HKLM "Software\Example" "Path"
SectionEnd

Section "Delete"
  Delete "$INSTDIR\old.txt"
SectionEnd

Section "DetailPrint"
  DetailPrint "installing"
SectionEnd

Section "GetInstDirError"
  GetInstDirError $0
  DetailPrint "instdir $0"
SectionEnd

Section "EnumRegKey"
  EnumRegKey $0 HKLM "Software\Example" 0
  DetailPrint $0
SectionEnd

Section "EnumRegValue"
  EnumRegValue $0 HKLM "Software\Example" 0
  DetailPrint $0
SectionEnd

Section "ExpandEnvStrings"
  ExpandEnvStrings $0 "%TEMP%"
  DetailPrint $0
SectionEnd

Section "File"
  File "assets\icon.ico"
SectionEnd

Section "FlushINI"
  FlushINI "$INSTDIR\app.ini"
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

Section "FileReadByte"
  FileOpen $0 "$INSTDIR\log.txt" "r"
  FileReadByte $0 $1
  DetailPrint "byte $1"
  FileClose $0
SectionEnd

Section "FileReadUTF16LE"
  FileOpen $0 "$INSTDIR\log.txt" "r"
  FileReadUTF16LE $0 $1
  DetailPrint $1
  FileClose $0
SectionEnd

Section "FileReadWord"
  FileOpen $0 "$INSTDIR\log.txt" "r"
  FileReadWord $0 $1
  DetailPrint "word $1"
  FileClose $0
SectionEnd

Section "FileSeek"
  FileOpen $0 "$INSTDIR\log.txt" "r"
  FileSeek $0 0 "END" $1
  DetailPrint "size $1"
  FileClose $0
SectionEnd

Section "GetFullPathName"
  GetFullPathName $0 "$INSTDIR\app.exe"
  DetailPrint $0
SectionEnd

Section "GetTempFileName"
  GetTempFileName $0
  DetailPrint $0
SectionEnd

Section "GetKnownFolderPath"
  GetKnownFolderPath $0 "{374DE290-123F-4565-9164-39C4925E467B}"
  DetailPrint $0
SectionEnd

Section "GetWinVer"
  GetWinVer $0 "MAJOR"
  IntCmpU $0 10 0 __GENERATED_endif_0 0
  DetailPrint "modern"
__GENERATED_endif_0:
SectionEnd

Section "ReadMemory"
  ReadMemory $0 0 4
  DetailPrint $0
SectionEnd

Section "HideWindow"
  HideWindow
SectionEnd

Section "IfAbort"
  IfAbort 0 __GENERATED_endif_0
  DetailPrint "cancelled"
__GENERATED_endif_0:
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

Section "IfRebootFlag"
  IfRebootFlag 0 __GENERATED_endif_0
  DetailPrint "a restart is needed"
__GENERATED_endif_0:
SectionEnd

Section "IfSilent"
  IfSilent 0 __GENERATED_endif_0
  DetailPrint "quiet"
__GENERATED_endif_0:
SectionEnd

Section "IfRtlLanguage"
  IfRtlLanguage 0 __GENERATED_endif_0
  DetailPrint "right to left"
__GENERATED_endif_0:
SectionEnd

Section "MessageBox"
  MessageBox MB_OK "finished"
SectionEnd

Section "Quit"
  Quit
SectionEnd

Section "ReadINIStr"
  ReadINIStr $0 "$INSTDIR\app.ini" "Settings" "Port"
  DetailPrint $0
SectionEnd

Section "ReadRegDWORD"
  ReadRegDWORD $0 HKLM "Software\Example" "Build"
  DetailPrint "build $0"
SectionEnd

Section "ReadRegStr"
  ReadRegStr $0 HKLM "Software\Example" "Path"
  DetailPrint $0
SectionEnd

Section "ReadEnvStr"
  ReadEnvStr $0 "TEMP"
  DetailPrint $0
SectionEnd

Section "Reboot"
  Reboot
SectionEnd

Section "RegDLL"
  RegDLL "$INSTDIR\shell.dll"
SectionEnd

Section "Rename"
  Rename "$INSTDIR\old.txt" "$INSTDIR\new.txt"
SectionEnd

Section "RMDir"
  RMDir $INSTDIR
SectionEnd

Section "SearchPath"
  SearchPath $0 "notepad.exe"
  DetailPrint $0
SectionEnd

Section "SetAutoClose"
  SetAutoClose "true"
SectionEnd

Section "SetDetailsView"
  SetDetailsView "show"
SectionEnd

Section "SetDetailsPrint"
  SetDetailsPrint "listonly"
SectionEnd

Section "SetErrors"
  SetErrors
SectionEnd

Section "SetErrorLevel"
  SetErrorLevel 2
SectionEnd

Section "GetErrorLevel"
  GetErrorLevel $0
  DetailPrint "level $0"
SectionEnd

Section "SetFileAttributes"
  SetFileAttributes "$INSTDIR\readme.txt" "READONLY"
SectionEnd

Section "SetOutPath"
  SetOutPath $INSTDIR
SectionEnd

Section "SetRebootFlag"
  SetRebootFlag "true"
SectionEnd

Section "GetRegView"
  GetRegView $0
  DetailPrint $0
SectionEnd

Section "SetRegView"
  SetRegView 64
SectionEnd

Section "IfAltRegView"
  IfAltRegView 0 __GENERATED_endif_0
  DetailPrint "the other view"
__GENERATED_endif_0:
SectionEnd

Section "GetShellVarContext"
  GetShellVarContext $0
  DetailPrint $0
SectionEnd

Section "SetShellVarContext"
  SetShellVarContext "all"
SectionEnd

Section "IfShellVarContextAll"
  IfShellVarContextAll 0 __GENERATED_endif_0
  DetailPrint "all users"
__GENERATED_endif_0:
SectionEnd

Section "Sleep"
  Sleep 500
SectionEnd

Section "StrLen"
  StrLen $0 "abc"
  DetailPrint "len $0"
SectionEnd

Section "UnRegDLL"
  UnRegDLL "$INSTDIR\shell.dll"
SectionEnd

Section "WriteINIStr"
  WriteINIStr "$INSTDIR\app.ini" "Settings" "Path" $INSTDIR
SectionEnd

Section "WriteRegBin"
  WriteRegBin HKLM "Software\Example" "Blob" "12848412AB"
SectionEnd

Section "WriteRegDWORD"
  WriteRegDWORD HKLM "Software\Example" "Build" 42
SectionEnd

Section "WriteRegStr"
  WriteRegStr HKLM "Software\Example" "Path" $INSTDIR
SectionEnd

Section "WriteRegExpandStr"
  WriteRegExpandStr HKLM "Software\Example" "Data" "%APPDATA%/Example"
SectionEnd

Section "WriteRegNone"
  WriteRegNone HKLM "Software\Example" "Marker"
SectionEnd

Section "WriteUninstaller"
  WriteUninstaller "$INSTDIR\uninstall.exe"
SectionEnd

Section "LockWindow"
  LockWindow "on"
SectionEnd

Section "un.Uninstall"
  RMDir $INSTDIR
SectionEnd
