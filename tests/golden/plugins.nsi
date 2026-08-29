Unicode true

Name "Plugins"
OutFile "plugins-setup.exe"

Section "Update"
  InitPluginsDir
  Dialer::GetConnectedState
  Pop $0
  StrCmpS $0 "online" 0 __GENERATED_endif_0
  NSISdl::download /TIMEOUT=30000 /NOIEPROXY "http://example.com/data.pat" "$PLUGINSDIR\data.pat"
  Pop $0
  DetailPrint $0
__GENERATED_endif_0:
  VPatch::vpatchfile "$PLUGINSDIR\data.pat" "$INSTDIR\data.dat" "$INSTDIR\data.new"
  Pop $0
  DetailPrint $0
  TypeLib::GetLibVersion "$INSTDIR\data.tlb"
  Pop $0
  Pop $1
  DetailPrint "$1.$0"
  TypeLib::Register "$INSTDIR\data.tlb"
SectionEnd

Section "Show"
  InitPluginsDir
  Banner::show "Working"
  Banner::destroy
  Splash::show 2000 "$PLUGINSDIR\logo"
  Pop $0
  IntCmp $0 1 0 __GENERATED_endif_0 __GENERATED_endif_0
  DetailPrint "impatient"
__GENERATED_endif_0:
  nsExec::ExecToStack /TIMEOUT=5000 /OEM "cmd.exe /c ver"
  Pop $0
  Pop $1
  DetailPrint "$0$1"
SectionEnd
