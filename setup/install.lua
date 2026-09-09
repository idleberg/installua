local EnVar = plugin "EnVar"

local APP <const> = "installua"
local VERSION <const> = param("VERSION", "dev")
local BINARY <const> = param("BINARY", "../target/release/installua.exe")

attributes {
  name = APP,
  outFile = APP .. "-" .. VERSION .. "-windows-x64.exe",
  unicode = true,
  compressor = "lzma",
  requestExecutionLevel = "user",
}

installer {
  installDir = LOCALAPPDATA .. "/Programs/" .. APP,

  page.license { file = "../LICENSE" },
  page.directory {},
  page.instFiles {},

  onInit(function()
    -- Rust's tier-1 Windows floor is Windows 10; 7/8.1 are tier 3 and the
    -- binary will not load there.
    if getWinVer("MAJOR") < 10 then
      messageBox {
        text = APP .. " requires Windows 10 or later.",
        buttons = "OK",
        icon = "STOP",
      }
      abort()
    end
  end),

  section("Core", function()
    setOutPath(INSTDIR)
    file(BINARY)

    EnVar.setHKCU()
    local code = EnVar.addValue("PATH", INSTDIR)

    if code ~= 0 then
      detailPrint("could not add " .. INSTDIR .. " to PATH (code " .. code .. ")")
    end
  end),
}
