-- $NSISDIR/Examples/Modern UI/Basic.nsi: one section with a description,
-- a license page and an uninstaller.

attributes {
	name = "Modern UI Test",
	outFile = "Basic.exe",
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
	end,
}

installer {
	abortPrompt = true,
	page.license { file = NSISDIR .. "/Docs/Modern UI/License.txt" },
	page.components {},
	page.directory {},
	page.instFiles {},
	SecDummy,
}

uninstaller {
	page.confirm {},
	page.instFiles {},
	section("Uninstall", function()
		delete(INSTDIR .. "/Uninstall.exe")
		rmDir(INSTDIR)
		deleteRegKey(HKCU, "Software/Modern UI Test", { ifEmpty = true })
	end),
}
