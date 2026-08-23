-- One file's worth of shared text, and the `func` that prints it.
--
-- Nothing here is exported, because there is nothing to export from: the
-- declarations are spliced into the program that named this file, which is
-- what makes `include` source layout rather than a module system.

local BANNER <const> = "Assembled"

func("announce", function(what)
  detailPrint(BANNER .. ": " .. what)
end)
