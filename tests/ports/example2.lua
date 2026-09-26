-- $NSISDIR/Examples/example2.nsi: example1 plus a remembered folder, an
-- uninstaller and optional Start menu shortcuts.

local uninstallKey <const> = "Software/Microsoft/Windows/CurrentVersion/Uninstall/Example2"

attributes {
	name = "Example2",
	outFile = "example2.exe",
	requestExecutionLevel = "admin",
	installDir = PROGRAMFILES .. "/Example2",
	installDirRegKey = { root = HKLM, key = "Software/NSIS_Example2", name = "Install_Dir" },
}

installer {
	page.components {},
	page.directory {},
	page.instFiles {},

	section { "Example2 (required)",
		required = true,
		body = function()
			setOutPath(INSTDIR)
			file("example2.nsi")
			writeReg(HKLM, "SOFTWARE/NSIS_Example2", "Install_Dir", INSTDIR)
			writeReg(HKLM, uninstallKey, "DisplayName", "NSIS Example2")
			writeReg(HKLM, uninstallKey, "UninstallString", '"' .. INSTDIR .. '/uninstall.exe"')
			writeReg(HKLM, uninstallKey, "NoModify", 1)
			writeReg(HKLM, uninstallKey, "NoRepair", 1)
			writeUninstaller(INSTDIR .. "/uninstall.exe")
		end,
	},

	section("Start Menu Shortcuts", function()
		createDirectory(SMPROGRAMS .. "/Example2")
		createShortcut(SMPROGRAMS .. "/Example2/Uninstall.lnk", INSTDIR .. "/uninstall.exe")
		createShortcut(SMPROGRAMS .. "/Example2/Example2 (MakeNSISW).lnk", INSTDIR .. "/example2.nsi")
	end),
}

uninstaller {
	page.confirm {},
	page.instFiles {},

	section("Uninstall", function()
		deleteRegKey(HKLM, uninstallKey)
		deleteRegKey(HKLM, "SOFTWARE/NSIS_Example2")
		delete(INSTDIR .. "/example2.nsi")
		delete(INSTDIR .. "/uninstall.exe")
		delete(SMPROGRAMS .. "/Example2/*.lnk")
		rmDir(SMPROGRAMS .. "/Example2")
		rmDir(INSTDIR)
	end),
}
