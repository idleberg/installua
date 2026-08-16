!include "WinVer.nsh"

Name "My Installer"
OutFile "demo.exe"

Function greet
  DetailPrint "Hello, world"
FunctionEnd

Section "My Section"
  ${WinVerGetMajor} $0
  IntCmp $0 10 0 else_0 0
  Call greet
  Goto endif_0
else_0:
  DetailPrint "Unsupported Windows version"
endif_0:
  StrCpy $1 2
  DetailPrint $1
  System::Call "kernel32::Beep(i, i) i (1000, 200)"
SectionEnd
