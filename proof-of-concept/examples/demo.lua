local winver = import "WinVer"
local system = plugin "System"

installer {
  name    = "My Installer",
  outFile = "demo.exe",
}

function greet()
  detailPrint("Hello, world")
end

section("My Section", function()
  local major = winver.getMajor()

  if major >= 10 then
    greet()
  else
    detailPrint("Unsupported Windows version")
  end

  local sum = 1 + 1
  detailPrint(sum)

  system.call("kernel32::Beep(i, i) i (1000, 200)")
end)
