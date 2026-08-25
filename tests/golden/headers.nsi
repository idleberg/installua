Unicode true

!define APP "/lib/app.dll"
!define CONF "/app.conf"

!include "FileFunc.nsh"
!include "TextFunc.nsh"
!include "WordFunc.nsh"

Name "Headers"
OutFile "headers-setup.exe"

Section "Core"
  SetOutPath $INSTDIR
  ${GetParent} "$INSTDIR\lib\app.dll" $0
  ${GetFileName} "$INSTDIR\lib\app.dll" $1
  ${GetBaseName} $1 $2
  ${GetFileExt} $1 $3
  DetailPrint "$0 $1 $2.$3"
  ${GetRoot} "$INSTDIR\lib\app.dll" $0
  ${DriveSpace} $0 "/D=F /S=M" $1
  ${GetSize} $INSTDIR "" $2 $3 $4
  IntOp $5 $2 / 1024
  DetailPrint "$3 files, $4 folders, $5 KiB"
  IntOp $2 $2 / 1048576
  IntCmpU $2 $1 __GENERATED_endif_0 __GENERATED_endif_0 0
  Abort "not enough room on $0"
__GENERATED_endif_0:
  ${GetTime} "$INSTDIR\lib\app.dll" "M" $0 $1 $2 $3 $4 $5 $6
  DetailPrint "$3 $0/$1/$2"
  DetailPrint "$4:$5:$6"
  ${GetFileVersion} "$INSTDIR\lib\app.dll" $0
  ${GetFileAttributes} "$INSTDIR\lib\app.dll" "READONLY" $1
  StrCmpS $1 1 0 __GENERATED_endif_1
  DetailPrint "read-only $0"
__GENERATED_endif_1:
  ${BannerTrimPath} "$INSTDIR\lib\app.dll" "35A" $0
  DetailPrint $0
SectionEnd

Section "Config"
  ${DirState} $INSTDIR $0
  IntCmp $0 -1 0 __GENERATED_endif_0 __GENERATED_endif_0
  CreateDirectory $INSTDIR
__GENERATED_endif_0:
  ${ConfigRead} "$INSTDIR\app.conf" "Port=" $0
  StrCmpS $0 "" 0 __GENERATED_endif_1
  ${ConfigWrite} "$INSTDIR\app.conf" "Port=" 8080 $0
  DetailPrint $0
__GENERATED_endif_1:
  ${ConfigReadS} "$INSTDIR\app.conf" "Host=" $0
  ${ConfigWriteS} "$INSTDIR\app.conf" "Host=" $0 $1
  ${LineRead} "$INSTDIR\app.conf" -1 $0
  ${LineSum} "$INSTDIR\app.conf" $1
  ${TrimNewLines} $0 $2
  DetailPrint "$1 lines ending $2"
  ${FileJoin} "$INSTDIR\app.conf" "$INSTDIR\extra.conf" ""
  ${FileRecode} "$INSTDIR\app.conf" "CharToOem"
  ReadRegStr $1 HKLM "System\CurrentControlSet\Control\Session Manager\Environment" "Path"
  ${WordAdd} $1 ";" $INSTDIR $2
  WriteRegExpandStr HKLM "System\CurrentControlSet\Control\Session Manager\Environment" "Path" $2
  ${WordFind} $2 ";" "#" $1
  ${WordFindS} $2 ";" "+1" $3
  DetailPrint "$1 entries, first $3"
  ${WordFind2X} $0 "[" "]" "+1" $1
  ${WordFind3X} $0 "[" "-" "]" "+1" $2
  DetailPrint "$1 $2"
  ${WordReplace} $0 "  " " " "+" $1
  ${WordInsert} $1 " " "1." "+1" $0
  ${StrFilter} $0 1 "" "." $1
  DetailPrint $1
SectionEnd

Section "Upgrade"
  ReadRegStr $0 HKLM "Software\Example" "Version"
  ${VersionConvert} "1.4.2-beta" "" $1
  ${VersionConvert} $0 "" $2
  ${VersionCompare} $1 $2 $3
  StrCmpS $3 1 0 __GENERATED_endif_0
  DetailPrint "upgrade from $0"
__GENERATED_endif_0:
  ${GetExePath} $0
  ${GetExeName} $1
  DetailPrint "$0\$1"
  ${RefreshShellIcons}
SectionEnd

Function .onInit
  ${GetParameters} $0
  ${GetOptions} $0 "/D=" $1
  ${GetOptionsS} $0 "/D=" $2
  StrCmpS $1 $2 __GENERATED_endif_0 0
  DetailPrint "switch case differs"
__GENERATED_endif_0:
FunctionEnd
