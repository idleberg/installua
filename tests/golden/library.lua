-- Library.nsh, lowered through the header. Each call is one `InstallLib` or
-- `UnInstallLib`; the table spells its keyword arguments as fields, and a
-- `LIBRARY_*` switch is defined around the one call that asked for it.

attributes {
	name = "Shared",
	outFile = "library.exe",
}

installer {
	page.instFiles {},

	section("Core", function()
		-- Empty on a first install, which is when the shared count goes up.
		local previous = readRegStr(HKLM, "Software/Shared", "Path")
		installLib("shared.dll", SYSDIR .. "/shared.dll", {
			type = "REGDLL",
			shared = previous,
			reboot = true,
			protected = true,
		})
		-- No table: an unregistered DLL, replaced now or not at all, with its
		-- temporary directory taken from the destination.
		installLib("shared.dll", INSTDIR .. "/shared.dll")
		installLib("shared.dll", INSTDIR .. "/shared.dll", { tempDir = TEMP, x64 = true })
		writeUninstaller(INSTDIR .. "/uninstall.exe")
	end),
}

uninstaller {
	section("Core", function()
		uninstallLib(SYSDIR .. "/shared.dll", {
			type = "REGDLL",
			shared = true,
			remove = true,
			reboot = true,
		})
		uninstallLib(INSTDIR .. "/shared.dll")
	end),
}
