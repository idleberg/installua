Unicode true

Name "Third Party"
OutFile "thirdparty-setup.exe"

Section "Environment"
  EnVar::SetHKLM
  EnVar::Check "PATH" "NULL"
  Pop $0
  IntCmpU $0 0 0 __GENERATED_endif_0 __GENERATED_endif_0
  EnVar::AddValue "PATH" "$INSTDIR\bin"
  Pop $0
  IntCmpU $0 0 __GENERATED_endif_1 0 0
  DetailPrint "PATH not updated"
__GENERATED_endif_1:
__GENERATED_endif_0:
  EnVar::Update "HKLM" "PATH"
  Pop $0
  IntCmpU $0 0 __GENERATED_endif_2 0 0
  DetailPrint "PATH not reloaded"
__GENERATED_endif_2:
SectionEnd

Section "Service"
  SimpleSC::ExistsService "Example"
  Pop $0
  IntCmp $0 0 0 __GENERATED_endif_0 __GENERATED_endif_0
  SimpleSC::ServiceIsRunning "Example"
  Pop $0
  Pop $1
  IntCmp $0 0 0 __GENERATED_endif_1 __GENERATED_endif_1
  IntCmp $1 1 0 __GENERATED_endif_1 __GENERATED_endif_1
  SimpleSC::StopService "Example" 1 30
  Pop $0
__GENERATED_endif_1:
__GENERATED_endif_0:
SectionEnd

Section "Archive"
  InitPluginsDir
  SetOutPath $INSTDIR
  Nsis7z::Extract "$PLUGINSDIR\payload.7z"
  Nsis7z::ExtractWithDetails "$PLUGINSDIR\extras.7z" "Extracting %s..."
SectionEnd

Section "Firewall"
  nsisFirewall::AddAuthorizedApplication "$INSTDIR\bin\app.exe" "Example"
  Pop $0
  IntCmp $0 0 __GENERATED_endif_0 0 0
  DetailPrint "firewall rule not added"
__GENERATED_endif_0:
SectionEnd

Section "Permissions"
  AccessControl::GrantOnFile $INSTDIR "(BU)" "FullAccess"
  Pop $0
  StrCpy $1 ""
  StrCmpS $0 "error" 0 __GENERATED_tail_0
  Pop $1
__GENERATED_tail_0:
  StrCmpS $0 "error" 0 __GENERATED_endif_0
  DetailPrint "ACL not set: $1"
__GENERATED_endif_0:
  AccessControl::GetCurrentUserName
  Pop $0
  Push $0
  AccessControl::NameToSid $0
  Pop $1
  StrCpy $2 ""
  StrCmpS $1 "error" 0 __GENERATED_tail_2
  Pop $2
__GENERATED_tail_2:
  Pop $0
  StrCmpS $1 "error" 0 __GENERATED_else_1
  DetailPrint $2
  Goto __GENERATED_endif_1
__GENERATED_else_1:
  DetailPrint "$0 is $1"
__GENERATED_endif_1:
SectionEnd

Section "Start Menu"
  StartMenu::Select "Example"
  Pop $0
  StrCpy $1 ""
  StrCmpS $0 "success" 0 __GENERATED_tail_0
  Pop $1
__GENERATED_tail_0:
  StrCmpS $0 "success" 0 __GENERATED_endif_0
  SetOutPath $INSTDIR
  CreateShortcut "$SMPROGRAMS\$1\Example.lnk" "$INSTDIR\app.exe"
__GENERATED_endif_0:
SectionEnd
