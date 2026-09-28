-- The sections, in the file that owns them, listed by the file that installs
-- them. The `return` is what makes that possible: a top-level `local` is this
-- file's own, and the table hands the two names on.

local core = section("Core", function()
  announce("installing")
  setOutPath(INSTDIR)
end)

local docs = section("Docs", function()
  announce("documentation")
end)

return { core = core, docs = docs }
