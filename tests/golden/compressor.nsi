Unicode true

!include "MUI2.nsh"
!include "StrFunc.nsh"

SetCompressor /SOLID lzma
Name "Compressor"
OutFile "compressor-setup.exe"

${Using:StrFunc} StrCase

!insertmacro MUI_PAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

Section "Core"
  ${StrCase} $0 $INSTDIR "L"
  DetailPrint $0
SectionEnd
