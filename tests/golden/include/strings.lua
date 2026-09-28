-- One file's worth of shared text, and the `func` that prints it.
--
-- Nothing here is returned, because nothing needs to be: `BANNER` is only read
-- in this file, and a `func` is program-wide already.

local BANNER <const> = "Assembled"

func("announce", function(what)
  detailPrint(BANNER .. ": " .. what)
end)
