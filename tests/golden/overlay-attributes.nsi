Unicode true

CPU x86
SetCompressor lzma
AddBrandingImage top "20u" "2u"
AutoCloseWindow true
BGFont "face" 1 1
BGGradient 000000 "0000FF" "FFFFFF"
BrandingText "brandingText"
Caption "caption"
CRCCheck on
SetDatablockOptimize on
AllowRootDirInstall true
FileBufSize 1
FileErrorText "text" "withoutIgnore"
Icon "assets\icon.ico"
InstallDirRegKey HKCR "key.out" "name"
InstallDir "installDir.out"
Name "name"
OutFile "outFile.out"
SetCompress off
SetCompressorDictSize 1
SetDateSave on
SetFont "face" 1
SetOverwrite on
ShowInstDetails hide
ShowUninstDetails hide
SilentInstall normal
SilentUnInstall normal
UninstallCaption "uninstallCaption"
WindowIcon on
RequestExecutionLevel none
MiscButtonText "back" "next" "cancel" "close"
DetailsButtonText "detailsButtonText"
UninstallButtonText "uninstallButtonText"
InstallButtonText "installButtonText"
SpaceTexts "required" "available"
CompletedText "completedText"
AllowSkipFiles on
PEAddResource "assets\icon.ico" "#100" "#1" 1033
PERemoveResource "#5" "#105" "ALL"
PEDllCharacteristics 1 1
PESubsysVer "5.1"
ManifestAppendCustomString "/assembly" "string"
ManifestDPIAware true
ManifestDPIAwareness "dpiAwareness"
ManifestLongPathAware true
ManifestSupportedOS none
ManifestMaxVersionTested "10.0.19041.0"
ManifestDisableWindowFiltering true
ManifestGdiScaling true

Section "Core"
  DetailPrint "installing"
SectionEnd
