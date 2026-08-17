Unicode true

Name "Components"
OutFile "components-setup.exe"

InstType "Full"
InstType "Minimal"

Section "Core"
  SectionIn 1 2 RO
  AddSize 120
  DetailPrint "core"
SectionEnd

SectionGroup /e "Tools"
  Section "Profiler"
    SectionIn 1
    DetailPrint "profiler"
  SectionEnd

  Section /o "Debugger"
    SectionIn 1
    AddSize 4096
    DetailPrint "debugger"
  SectionEnd
SectionGroupEnd

SectionGroup "Docs"
  Section "Manual"
    DetailPrint "manual"
  SectionEnd
SectionGroupEnd
