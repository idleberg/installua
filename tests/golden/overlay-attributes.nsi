Unicode true

Caption "caption"
CRCCheck on
AllowRootDirInstall true
FileBufSize 1
Icon "assets\icon.ico"
InstallDirRegKey HKCR "key.out" "name"
InstallDir "installDir.out"
LicenseData "assets\license.txt"
Name "name"
OutFile "outFile.out"
SetCompressor zlib
SetDateSave on
ShowInstDetails hide
ShowUninstDetails hide
SilentInstall normal
SilentUnInstall normal
CPU x86
PEDllCharacteristics 1 1
PESubsysVer "5.1"
RequestExecutionLevel none
ManifestDPIAware true
ManifestDPIAwareness "manifestDpiAwareness"
ManifestLongPathAware true
ManifestMaxVersionTested "10.0.19041.0"
ManifestDisableWindowFiltering true
ManifestGdiScaling true
AllowSkipFiles on

Section "Core"
  DetailPrint "installing"
SectionEnd
