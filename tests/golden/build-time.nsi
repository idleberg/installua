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
  DetailPrint "1. Read me (readme.txt)"
  DetailPrint "2. License (license.txt)"
  DetailPrint "build 42"
  !echo "building ${NAME}"
  !system "test -n $\"$HOME$\"" _MAKENSIS
  !ifndef NSIS_WIN32_MAKENSIS
  !define /redef /math _MAKENSIS ${_MAKENSIS} >> 8
  !endif
  StrCpy $0 "${_MAKENSIS}"
  !getdllversion "shared.dll" _MAKENSIS
  IntOp $1 "${_MAKENSIS1}" + 0
  IntOp $2 "${_MAKENSIS2}" + 0
  DetailPrint "status $0, shared $1.$2"
  Call stamp
SectionEnd

Function stamp
  DetailPrint "late build"
FunctionEnd

!finalize 'echo done'
