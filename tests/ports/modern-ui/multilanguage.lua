-- $NSISDIR/Examples/Modern UI/MultiLanguage.nsi: Basic in 67 languages, with
-- the language dialog remembered in the registry.

attributes {
	name = "Modern UI Test",
	outFile = "MultiLanguage.exe",
	installDir = LOCALAPPDATA .. "/Modern UI Test",
	installDirRegKey = { root = HKCU, key = "Software/Modern UI Test", name = "" },
	requestExecutionLevel = "user",
}

languages {
	-- The first is the default.
	locales = {
		English = {},
		French = {},
		German = {},
		Spanish = {},
		SpanishInternational = {},
		SimpChinese = {},
		TradChinese = {},
		Japanese = {},
		Korean = {},
		Italian = {},
		Dutch = {},
		Danish = {},
		Swedish = {},
		Norwegian = {},
		NorwegianNynorsk = {},
		Finnish = {},
		Greek = {},
		Russian = {},
		Portuguese = {},
		PortugueseBR = {},
		Polish = {},
		Ukrainian = {},
		Czech = {},
		Slovak = {},
		Croatian = {},
		Bulgarian = {},
		Hungarian = {},
		Thai = {},
		Romanian = {},
		Latvian = {},
		Macedonian = {},
		Estonian = {},
		Turkish = {},
		Lithuanian = {},
		Slovenian = {},
		Serbian = {},
		SerbianLatin = {},
		Arabic = {},
		Farsi = {},
		Hebrew = {},
		Indonesian = {},
		Mongolian = {},
		Luxembourgish = {},
		Albanian = {},
		Breton = {},
		Belarusian = {},
		Icelandic = {},
		Malay = {},
		Bosnian = {},
		Kurdish = {},
		Irish = {},
		Uzbek = {},
		Galician = {},
		Afrikaans = {},
		Catalan = {},
		Esperanto = {},
		Asturian = {},
		Basque = {},
		Pashto = {},
		ScotsGaelic = {},
		Georgian = {},
		Vietnamese = {},
		Welsh = {},
		Armenian = {},
		Corsican = {},
		Tatar = {},
		Hindi = {},
	},
	ask = {
		allLanguages = true,
		remember = { root = "HKCU", key = "Software/Modern UI Test", value = "Installer Language" },
	},
}

local SecDummy = section { "Dummy Section",
	description = "A test section.",
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
	page.license { file = NSISDIR .. "/Docs/Modern UI/License.txt" },
	page.components {},
	page.directory {},
	page.instFiles {},
	page.finish {},
	section("Uninstall", function()
		delete(INSTDIR .. "/Uninstall.exe")
		rmDir(INSTDIR)
		deleteRegKey(HKCU, "Software/Modern UI Test", { ifEmpty = true })
	end),
}
