!include "WinVer.nsh"
!include "StrFunc.nsh"

${StrCase}

!define APP "Example"
!define VERSION "1.4.2"
!define TITLE "${APP} ${VERSION}"
!getdllversion "bin\app.dll" BUILD_
!define BUILD "${BUILD_1}.${BUILD_2}.${BUILD_3}.${BUILD_4}"

Name "${TITLE}"
OutFile "${APP}-${VERSION}.exe"
InstallDir "$PROGRAMFILES\Example"

Function reportBudget
  StrCpy $0 98304
  StrCpy $1 250000
  IntOp $R9 $1 - $0
  IntOp $2 $R9 / 1024
  IntOp $3 $1 % 1024
  DetailPrint "${TITLE} needs $0 KiB"
  DetailPrint "free afterwards: $2 MiB, $3 KiB slack"
  ${StrCase} $R9 "${APP}" "U"
  DetailPrint "build ${BUILD} of $R9"
FunctionEnd

Section "Core"
  SetOutPath "$INSTDIR"
  File "build\app.exe"
  ${WinVerGetMajor} $0
  IntCmp $0 10 endif_0 0 endif_0
  MessageBox MB_OK|MB_ICONSTOP "${TITLE} requires Windows 10 or newer."
  Abort "unsupported Windows version"
endif_0:
  MessageBox MB_YESNO|MB_ICONQUESTION "Install the optional tools as well?" /SD IDNO IDYES mb_yes_2 IDNO mb_no_2
mb_yes_2:
  SetOutPath "$INSTDIR\tools"
  File "build\tools\*.exe"
  Goto mb_end_2
mb_no_2:
  DetailPrint "Skipping the optional tools."
mb_end_2:
  Call reportBudget
  nsExec::ExecToLog "cmd /c ver"
SectionEnd

SectionGroup /e "Optional components"
  Section "Documentation"
    SetOutPath "$INSTDIR\docs"
    File "docs\*.pdf"
  SectionEnd

  Section /o "Sample scripts"
    SetOutPath "$INSTDIR\samples"
    File "samples\*.lua"
  SectionEnd
SectionGroupEnd
