-- Three files, one program.
--
-- The point of the golden is what the `.nsi` does *not* show: no marker, no
-- ordering artefact, nothing that says which file a line came from. `include`
-- is frontend-only, so the output is the one this program written in a single
-- file would have had.
--
-- `strings.lua` declares a `func` that `sections.lua` calls, and this file
-- lists the sections `sections.lua` returns. A `func` is program-wide, as its
-- name is a string; a `local` is its file's own, as in Lua, so the sections
-- come through the table that file returns.

include "include/strings.lua"
local sections = include "include/sections.lua"

attributes {
  name = "Assembled",
  outFile = "assembled-setup.exe",
  installDir = PROGRAMFILES .. "/Assembled",
}

installer {
  page.directory {},
  page.instFiles {},

  sections.core,
  sections.docs,
}
