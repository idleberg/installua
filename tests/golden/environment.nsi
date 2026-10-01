Unicode true

Name "Environment"
OutFile "environment.exe"

Section "Core"
  ReadEnvStr $0 "USERPROFILE"
  Push $0
  Push $EXEDIR
  Push "NSISDIR"
  System::Call "kernel32::SetEnvironmentVariable(t s, t s)"
  Pop $0
  Push $0
  Push $0
  Push "HOME"
  System::Call "kernel32::SetEnvironmentVariable(t s, t s)"
  Pop $0
  Exec "makensis.exe"
  DetailPrint $0
SectionEnd
