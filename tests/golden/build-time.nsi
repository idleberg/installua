!echo "building"

Unicode true

!define NAME "Build Time"
!define BUILD 41
!define SIGNED 0

Name "${NAME}"
OutFile "build-time-setup.exe"

Section "Core"
  SetOutPath $INSTDIR
  File "assets\check.bmp"
  DetailPrint "build 42"
  Call stamp
SectionEnd

Function stamp
  DetailPrint "late build"
FunctionEnd

!finalize 'echo done'
