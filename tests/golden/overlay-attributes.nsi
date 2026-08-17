Unicode true

SetCompressor zlib
AddBrandingImage top "20u" "2u"
BGFont "face" 1 1
BrandingText "brandingText"
Caption "caption"
CRCCheck on
AllowRootDirInstall true
FileBufSize 1
FileErrorText "text" "withoutIgnore"
Icon "assets\icon.ico"
InstallDirRegKey HKCR "key.out" "name"
InstallDir "installDir.out"
LicenseData "assets\license.txt"
Name "name"
OutFile "outFile.out"
SetDateSave on
SetFont "face" 1
ShowInstDetails hide
ShowUninstDetails hide
SilentInstall normal
SilentUnInstall normal
CPU x86
UninstallCaption "uninstallCaption"
WindowIcon on
PEAddResource "assets\icon.ico" "#100" "#1" 1033
PERemoveResource "#5" "#105" "ALL"
PEDllCharacteristics 1 1
PESubsysVer "5.1"
RequestExecutionLevel none
ManifestAppendCustomString "/assembly" "string"
ManifestDPIAware true
ManifestDPIAwareness "manifestDpiAwareness"
ManifestLongPathAware true
ManifestSupportedOS none
ManifestMaxVersionTested "10.0.19041.0"
ManifestDisableWindowFiltering true
ManifestGdiScaling true
MiscButtonText "back" "next" "cancel" "close"
DetailsButtonText "detailsButtonText"
UninstallButtonText "uninstallButtonText"
InstallButtonText "installButtonText"
CompletedText "completedText"
AllowSkipFiles on

Section "Core"
  DetailPrint "installing"
SectionEnd
