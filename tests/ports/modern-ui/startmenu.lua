-- $NSISDIR/Examples/Modern UI/StartMenu.nsi: Basic with a Start menu folder
-- page, remembered in the registry.

attributes {
	name = "Modern UI Test",
	outFile = "StartMenu.exe",
	installDir = LOCALAPPDATA .. "/Modern UI Test",
	installDirRegKey = { root = HKCU, key = "Software/Modern UI Test", name = "" },
	requestExecutionLevel = "user",
}

languages {
	locales = {
		English = { DESC_SecDummy = "A test section." },
	},
}

local SecDummy = section { "Dummy Section",
	description = lang.DESC_SecDummy,
	body = function()
		setOutPath(INSTDIR)
		writeReg(HKCU, "Software/Modern UI Test", "", INSTDIR)
		writeUninstaller(INSTDIR .. "/Uninstall.exe")
		StartMenuFolder.write(function()
			createDirectory(SMPROGRAMS .. "/" .. StartMenuFolder.folder)
			createShortcut(SMPROGRAMS .. "/" .. StartMenuFolder.folder .. "/Uninstall.lnk", INSTDIR .. "/Uninstall.exe")
		end)
	end,
}

local StartMenuFolder = page.startMenu {
	registry = { root = "HKCU", key = "Software/Modern UI Test", value = "Start Menu Folder" },
}

installer {
	abortPrompt = true,
	page.license { file = NSISDIR .. "/Docs/Modern UI/License.txt" },
	page.components {},
	page.directory {},
	StartMenuFolder,
	page.instFiles {},
	SecDummy,
}

uninstaller {
	page.confirm {},
	page.instFiles {},
	section("Uninstall", function()
		delete(INSTDIR .. "/Uninstall.exe")
		rmDir(INSTDIR)
		delete(SMPROGRAMS .. "/" .. StartMenuFolder.folder .. "/Uninstall.lnk")
		rmDir(SMPROGRAMS .. "/" .. StartMenuFolder.folder)
		deleteRegKey(HKCU, "Software/Modern UI Test", { ifEmpty = true })
	end),
}
