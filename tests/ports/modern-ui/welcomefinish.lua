-- $NSISDIR/Examples/Modern UI/WelcomeFinish.nsi: Basic with welcome and
-- finish pages in both halves.

attributes {
	name = "Modern UI Test",
	outFile = "WelcomeFinish.exe",
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
	page.welcome {},
	page.license { file = NSISDIR .. "/Docs/Modern UI/License.txt" },
	page.components {},
	page.directory {},
	page.instFiles {},
	page.finish {},
	SecDummy,
}

uninstaller {
	page.welcome {},
	page.confirm {},
	page.instFiles {},
	page.finish {},
	section("Uninstall", function()
		delete(INSTDIR .. "/Uninstall.exe")
		rmDir(INSTDIR)
		deleteRegKey(HKCU, "Software/Modern UI Test", { ifEmpty = true })
	end),
}
