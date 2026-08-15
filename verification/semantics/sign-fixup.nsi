; The exact fixup pattern from program 5, over the boundary cases.
Unicode true
Name "fixup"
OutFile "fixup.exe"
SilentInstall silent
RequestExecutionLevel user

!macro DIVMOD a b
  IntOp $4 ${a} / ${b}
  IntOp $5 ${a} % ${b}
  IntCmp $5 0 skip_${a}_${b} 0 skip_${a}_${b}
  IntOp $4 $4 - 1
  IntOp $5 $5 + ${b}
skip_${a}_${b}:
  FileWrite $9 "${a} // ${b} = $4    ${a} %% ${b} = $5$\r$\n"
!macroend

Section
  FileOpen $9 "$EXEDIR\fixup-result.txt" w
  !insertmacro DIVMOD 7 2
  !insertmacro DIVMOD -7 2
  !insertmacro DIVMOD -8 2
  !insertmacro DIVMOD 0 2
  !insertmacro DIVMOD -1 64
  !insertmacro DIVMOD 65 64
  FileClose $9
SectionEnd
