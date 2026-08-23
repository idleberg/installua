Unicode true

!include "MUI2.nsh"

Name "Polyglot"
OutFile "polyglot-setup.exe"
InstallDir "$PROGRAMFILES\Polyglot"

!define MUI_LANGDLL_WINDOWTITLE "Installer Language"
!define MUI_LANGDLL_INFO "Please select a language."
!define MUI_LANGDLL_ALWAYSSHOW
!define MUI_LANGDLL_REGISTRY_ROOT "HKCU"
!define MUI_LANGDLL_REGISTRY_KEY "Software\Polyglot"
!define MUI_LANGDLL_REGISTRY_VALUENAME "Installer Language"

!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "German"
!insertmacro MUI_LANGUAGE "PortugueseBR"
!insertmacro MUI_RESERVEFILE_LANGDLL
LangString farewell ${LANG_ENGLISH} "Removing Polyglot"
LangString farewell ${LANG_GERMAN} "Polyglot wird entfernt"
LangString farewell ${LANG_PORTUGUESEBR} "Removendo o Polyglot"
LangString folder ${LANG_ENGLISH} "Files go to"
LangString folder ${LANG_GERMAN} "Dateien landen in"
LangString folder ${LANG_PORTUGUESEBR} "Os arquivos vao para"
LangString greeting ${LANG_ENGLISH} "Installing Polyglot"
LangString greeting ${LANG_GERMAN} "Polyglot wird installiert"
LangString greeting ${LANG_PORTUGUESEBR} "Instalando o Polyglot"

Section "Core"
  DetailPrint $(greeting)
  DetailPrint "$(folder): $INSTDIR"
  SetOutPath $INSTDIR
  WriteUninstaller "$INSTDIR\uninstall.exe"
SectionEnd

Section "un.Uninstall"
  DetailPrint $(farewell)
  Delete "$INSTDIR\uninstall.exe"
  RMDir $INSTDIR
SectionEnd

Function .onInit
  !insertmacro MUI_LANGDLL_DISPLAY
FunctionEnd

Function un.onInit
  !insertmacro MUI_UNGETLANGUAGE
FunctionEnd
