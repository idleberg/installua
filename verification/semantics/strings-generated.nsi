Unicode true

!define BLOCK 64

!include "StrFunc.nsh"

${Using:StrFunc} StrCase
${Using:StrFunc} StrLoc

Name "Strings"
OutFile "strings.exe"
RequestExecutionLevel user
SilentInstall silent

Function majorOf
  Pop $0
  ${StrLoc} $1 $0 "." ">"
  IntOp $1 $1 + 1
  IntOp $1 $1 - 1
  StrCpy $0 $0 $1 0
  Push $0
FunctionEnd

Section "Core"
  FileOpen $0 "$EXEDIR\strings-result.txt" "w"
  Push $0
  Push "2.1.0"
  Call majorOf
  Pop $1
  Pop $0
  FileWrite $0 "major=$1$\n"
  ${StrLoc} $1 "2.1.0" "." ">"
  IntOp $1 $1 + 1
  FileWrite $0 "find=$1$\n"
  StrCpy $1 "abcdef" 3 1
  FileWrite $0 "sub=$1$\n"
  ${StrCase} $1 "beta" "U"
  FileWrite $0 "upper=$1$\n"
  ${StrCase} $1 "BETA" "L"
  FileWrite $0 "lower=$1$\n"
  IntFmt $1 "%04d" 42
  FileWrite $0 "fmt=$1$\n"
  StrLen $1 "abcdefgh"
  IntOp $1 $1 - 512
  IntOp $2 $1 / ${BLOCK}
  IntOp $3 $1 % ${BLOCK}
  IntCmp $3 0 __GENERATED_div_0_done 0 0
  IntOp $3 $3 ^ ${BLOCK}
  IntCmp $3 0 __GENERATED_div_0_done 0 __GENERATED_div_0_done
  IntOp $2 $2 - 1
__GENERATED_div_0_done:
  FileWrite $0 "div=$2$\n"
  IntOp $2 $1 % ${BLOCK}
  IntOp $1 $1 % ${BLOCK}
  IntCmp $1 0 __GENERATED_div_1_done 0 0
  IntOp $1 $1 ^ ${BLOCK}
  IntCmp $1 0 __GENERATED_div_1_done 0 __GENERATED_div_1_done
  IntOp $2 $2 + ${BLOCK}
__GENERATED_div_1_done:
  StrCpy $1 $2
  FileWrite $0 "mod=$1$\n"
  FileClose $0
SectionEnd
