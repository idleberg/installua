-- One construct from each language group. `demo.lua` is the minimal
-- input; this one exercises everything the PoC lowers.

local winver = import "WinVer"
local nsExec = plugin "nsExec"

-- Compile-time. `<const>` is `!define`; `pre.*` ran on the build machine,
-- and the namespace is what makes the stage legible in the source.
local APP     <const> = "Example"
local VERSION <const> = "1.4.2"
local TITLE   <const> = APP .. " " .. VERSION
local BUILD   <const> = pre.getDllVersion([[bin\app.dll]])

installer {
  name       = TITLE,
  outFile    = APP .. "-" .. VERSION .. ".exe",
  installDir = [[$PROGRAMFILES\Example]],
}

-- Arithmetic and strings. `//` is NSIS's `/` and `%` follows the
-- dividend; Lua's `/` and `^` mean something else again and are rejected.
-- `..` costs no instructions: it becomes one NSIS string template.
function reportBudget()
  local needed = 96 * 1024
  local free   = 250000
  local spare  = (free - needed) // 1024
  local slack  = free % 1024

  detailPrint(TITLE .. " needs " .. needed .. " KiB")
  detailPrint("free afterwards: " .. spare .. " MiB, " .. slack .. " KiB slack")
  detailPrint("build " .. BUILD .. " of " .. string.upper(APP))
end

section("Core", function()
  setOutPath("$INSTDIR")
  file([[build\app.exe]])

  local major = winver.getMajor()
  if major < 10 then
    messageBox {
      text    = TITLE .. " requires Windows 10 or newer.",
      buttons = "OK",
      icon    = "STOP",
    }
    abort("unsupported Windows version")
  end

  -- The jump-table half of MessageBox, as handler blocks rather than labels.
  messageBox {
    text    = "Install the optional tools as well?",
    buttons = "YESNO",
    icon    = "QUESTION",
    default = "NO",
    onYes = function()
      setOutPath([[$INSTDIR\tools]])
      file([[build\tools\*.exe]])
    end,
    onNo = function()
      detailPrint("Skipping the optional tools.")
    end,
  }

  reportBudget()
  nsExec.execToLog([[cmd /c ver]])
  -- No `writeUninstaller` here: uninstaller duality is a group of its own, and
  -- `makensis` rejects the call without an `un.` section.
end)

sectionGroup("Optional components", { expanded = true }, function()
  section("Documentation", function()
    setOutPath([[$INSTDIR\docs]])
    file([[docs\*.pdf]])
  end)

  section("Sample scripts", { optional = true }, function()
    setOutPath([[$INSTDIR\samples]])
    file([[samples\*.lua]])
  end)
end)
