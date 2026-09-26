-- $NSISDIR/Examples/MultiUser.nsi: installs for everyone or for the current
-- user, and the uninstaller finds out which.

attributes {
	name = "MultiUser example",
	outFile = "MultiUser.exe",
}

multiUser {
	executionLevel = "highest",
	commandLine = true,
	folder = lang.builtin.Name,
	modeRegistry = { key = "Software/Microsoft/Windows/CurrentVersion/Uninstall/" .. lang.builtin.Name, value = "CurrentUser" },
}

installer {
	page.welcome {},
	page.installMode {},
	page.directory {},
	page.instFiles {},
	page.finish {},

	section("", function()
		setOutPath(INSTDIR)
		writeUninstaller(INSTDIR .. "/Uninstall.exe")
		writeReg(SHCTX, "Software/Microsoft/Windows/CurrentVersion/Uninstall/" .. lang.builtin.Name, "DisplayName", lang.builtin.Name)
		writeReg(SHCTX, "Software/Microsoft/Windows/CurrentVersion/Uninstall/" .. lang.builtin.Name, "UninstallString", '"' .. INSTDIR .. '/Uninstall.exe"')
		-- The value `modeRegistry` looks for, so the uninstaller finds the mode.
		writeReg(SHCTX, "Software/Microsoft/Windows/CurrentVersion/Uninstall/" .. lang.builtin.Name, multiUser.installMode, "1")
		file("AppGen.nsi", { outName = INSTDIR .. "/MyApp.exe" })
	end),

	section("Start Menu shortcut", function()
		createShortcut(SMPROGRAMS .. "/" .. lang.builtin.Name .. ".lnk", INSTDIR .. "/MyApp.exe", { noWorkingDir = true })
	end),
}

uninstaller {
	page.confirm {},
	page.instFiles {},

	section("Uninstall", function()
		delete(SMPROGRAMS .. "/" .. lang.builtin.Name .. ".lnk")
		delete(INSTDIR .. "/MyApp.exe")
		delete(INSTDIR .. "/Uninstall.exe")
		deleteRegKey(SHCTX, "Software/Microsoft/Windows/CurrentVersion/Uninstall/" .. lang.builtin.Name)
		rmDir(INSTDIR)
	end),
}
