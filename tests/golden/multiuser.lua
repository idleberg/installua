-- MultiUser.nsh, lowered through the header the way MUI2 is. The block's
-- fields are `MULTIUSER_*` defines above its `!include`; the init macros are
-- the first lines of each half's `.onInit`, ahead of what the script wrote
-- there; and the install-mode page is a MUI2 page among the others, taking the
-- same header text and hooks.

attributes {
	name = "Shared",
	outFile = "multiuser.exe",
}

multiUser {
	executionLevel = "highest",
	commandLine = true,
	folder = "Shared",
	-- The uninstaller finds its mode again by where this value landed:
	-- `SHCTX` below is `HKLM` for all users and `HKCU` for one.
	modeRegistry = { key = "Software/Shared", value = "Installed" },
}

installer {
	onInit(function()
		detailPrint("ready")
	end),

	page.welcome {},
	page.installMode { headerText = "Who is this for?" },
	page.directory {},
	page.instFiles {},

	section("Core", function()
		setOutPath(INSTDIR)
		writeUninstaller(INSTDIR .. "/uninstall.exe")
		writeReg(SHCTX, "Software/Shared", "Installed", "1")
	end),
}

uninstaller {
	page.confirm {},
	page.instFiles {},

	section("Uninstall", function()
		deleteRegKey(SHCTX, "Software/Shared")
		delete(INSTDIR .. "/uninstall.exe")
		rmDir(INSTDIR)
	end),
}
