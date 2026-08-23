-- The sections, in the file that owns them, listed by the file that installs
-- them. Splitting code is what `include` exists for; splitting data was always
-- possible (§15.28).

local core = section("Core", function()
  announce("installing")
  setOutPath(INSTDIR)
end)

local docs = section("Docs", function()
  announce("documentation")
end)
