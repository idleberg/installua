Unicode true

!define MULTIUSER_EXECUTIONLEVEL Standard
!define MULTIUSER_INSTALLMODE_INSTDIR "Mine"
!define MULTIUSER_INSTALLMODE_INSTDIR_REGISTRY_KEY "Software\Mine"
!define MULTIUSER_INSTALLMODE_INSTDIR_REGISTRY_VALUENAME "Path"
!define MULTIUSER_NOUNINSTALL

!include "MultiUser.nsh"

Name "Mine"
OutFile "multiuser-plain.exe"

Section "Core"
  SetOutPath $INSTDIR
  WriteRegStr SHCTX "Software\Mine" "Path" $INSTDIR
SectionEnd

Function .onInit
  !insertmacro MULTIUSER_INIT
FunctionEnd
