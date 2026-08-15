-- Every construct here is legal Installua and must produce **zero** LuaLS
-- diagnostics. If this file is not clean, the meta file is wrong, not the source.

attributes {
	name = "Example",
	outFile = "example.exe",
	unicode = true,
	requestExecutionLevel = "admin",
	crcCheck = "force",
	xpStyle = true,
	manifest = { gdiScaling = true },
}

installer {
	installDir = PROGRAMFILES64 .. "/Example",
	pages = { "Welcome", "Directory", "InstFiles", "Finish" },
}

uninstaller {
	pages = { "Confirm", "InstFiles" },
}

section("Main", function()
	setOutPath(INSTDIR)
	file("build/app.exe")
	detailPrint("Installed to " .. INSTDIR)

	local answer = messageBox {
		text = "Install the optional tools?",
		buttons = "YESNO",
	}
	if answer == "YES" then
		detailPrint("yes")
	end
end)
