-- Three files, one program (§15.28).
--
-- The point of the golden is what the `.nsi` does *not* show: no marker, no
-- ordering artefact, nothing that says which file a line came from. `include`
-- is frontend-only, so the output is the one this program written in a single
-- file would have had.
--
-- `strings.lua` declares a `func` that `sections.lua` calls, and this file
-- lists the sections both of them declare. Three files, and no ordering rule
-- between them beyond the one Lua already has for a `local`.

include "include/strings.lua"
include "include/sections.lua"

attributes {
  name = "Assembled",
  outFile = "assembled-setup.exe",
  installDir = PROGRAMFILES .. "/Assembled",
}

installer {
  page.directory {},
  page.instFiles {},

  core,
  docs,
}
