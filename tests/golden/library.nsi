Unicode true

!include "MUI2.nsh"
!include "Library.nsh"

Name "Shared"
OutFile "library.exe"

!insertmacro MUI_PAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

Section "Core"
  ReadRegStr $0 HKLM "Software\Shared" "Path"
  !insertmacro InstallLib REGDLL $0 REBOOT_PROTECTED "shared.dll" "$SYSDIR\shared.dll" $SYSDIR
  !insertmacro InstallLib DLL NOTSHARED NOREBOOT_NOTPROTECTED "shared.dll" "$INSTDIR\shared.dll" $INSTDIR
  !define LIBRARY_X64
  !insertmacro InstallLib DLL NOTSHARED NOREBOOT_NOTPROTECTED "shared.dll" "$INSTDIR\shared.dll" $TEMP
  !undef LIBRARY_X64
  WriteUninstaller "$INSTDIR\uninstall.exe"
SectionEnd

Section "un.Core"
  !insertmacro UnInstallLib REGDLL SHARED REBOOT_NOTPROTECTED "$SYSDIR\shared.dll"
  !insertmacro UnInstallLib DLL NOTSHARED NOREMOVE "$INSTDIR\shared.dll"
SectionEnd
