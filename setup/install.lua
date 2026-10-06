local EnVar = plugin "EnVar"

local APP <const> = "installua"
local VERSION <const> = param("VERSION", "dev")
local BINARY <const> = param("BINARY", "../target/release/installua.exe")

-- Per-user install, so the Add/Remove Programs entry belongs under HKCU. HKLM
-- would need elevation and would advertise the package to users who cannot run it.
local REGKEY <const> = "Software/Microsoft/Windows/CurrentVersion/Uninstall/" .. APP

attributes {
  name = APP,
  outFile = APP .. "-" .. VERSION .. "-windows-x64.exe",
  compressor = "lzma",
  requestExecutionLevel = "user",
  versionInfo = {
    keys = {
      ProductName = APP,
      ProductVersion = VERSION,
      CompanyName = "Jan T. Sott",
      LegalCopyright = "(c) Jan T. Sott",
      FileDescription = "A Lua-shaped language that compiles to NSIS.",
      FileVersion = VERSION,
    },
  },
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

    -- Upgrade in place rather than beside: an earlier install's directory wins
    -- over the default, so a second copy never ends up on PATH alongside the first.
    local prior = readRegStr(HKCU, REGKEY, "InstallLocation")
    if prior ~= "" then
      INSTDIR = prior
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

    writeUninstaller(INSTDIR .. "/uninstall.exe")

    -- What winget reads to know the package is installed, at which version, and
    -- how to remove it. `QuietUninstallString` is why `winget uninstall` needs no
    -- clicking; without it winget falls back to the interactive uninstaller.
    writeReg(HKCU, REGKEY, "DisplayName", APP)
    writeReg(HKCU, REGKEY, "DisplayVersion", VERSION)
    writeReg(HKCU, REGKEY, "DisplayIcon", INSTDIR .. "/installua.exe")
    writeReg(HKCU, REGKEY, "Publisher", "Jan T. Sott")
    writeReg(HKCU, REGKEY, "URLInfoAbout", "https://github.com/idleberg/installua")
    writeReg(HKCU, REGKEY, "InstallLocation", INSTDIR)
    writeReg(HKCU, REGKEY, "UninstallString", '"' .. INSTDIR .. '/uninstall.exe"')
    writeReg(HKCU, REGKEY, "QuietUninstallString", '"' .. INSTDIR .. '/uninstall.exe" /S')
    writeReg(HKCU, REGKEY, "NoModify", 1)
    writeReg(HKCU, REGKEY, "NoRepair", 1)
  end),
}

uninstaller {
  page.confirm {},
  page.instFiles {},

  section("Uninstall", function()
    -- Before the files go, while INSTDIR still means something to EnVar.
    EnVar.setHKCU()
    EnVar.deleteValue("PATH", INSTDIR)

    delete(INSTDIR .. "/installua.exe")
    delete(INSTDIR .. "/uninstall.exe")
    rmDir(INSTDIR)

    deleteRegKey(HKCU, REGKEY)
  end),
}
