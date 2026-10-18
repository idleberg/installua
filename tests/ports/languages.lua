-- $NSISDIR/Examples/languages.nsi: twelve languages, chosen from a dialog
-- when the installer starts.

-- NSIS's language ids, which the original reads as `${LANG_ENGLISH}`.
local LANG <const> = {
	English = 1033,
	Czech = 1029,
	Dutch = 1043,
	French = 1036,
	German = 1031,
	Korean = 1042,
	Russian = 1049,
	Spanish = 1034,
	Swedish = 1053,
	TradChinese = 1028,
	SimpChinese = 2052,
	Slovak = 1051,
}

attributes {
	-- Set through the normal interface, as a language string.
	name = lang.Name,
	outFile = "languages.exe",
	requestExecutionLevel = "user",
	-- One text for all languages: no language string.
	completedText = "Languages example completed",
}

languages {
	-- The first is the default.
	locales = {
		English = {
			Name = "English",
			ComponentsText = "English component page",
			Sec1Name = "English section #1",
			Message = "English message",
		},
		Czech = {
			Name = "Czech",
			ComponentsText = "Czech component page",
			Sec1Name = "Czech section #1",
			Message = "Czech message",
		},
		Dutch = {
			Name = "Dutch",
			ComponentsText = "Dutch component page",
			Sec1Name = "Dutch section #1",
			Message = "Dutch message",
		},
		French = {
			Name = "French",
			ComponentsText = "French component page",
			Sec1Name = "French section #1",
			Message = "French message",
		},
		German = {
			Name = "German",
			ComponentsText = "German component page",
			Sec1Name = "German section #1",
			Message = "German message",
		},
		Korean = {
			Name = "Korean",
			ComponentsText = "Korean component page",
			Sec1Name = "Korean section #1",
			Message = "Korean message",
		},
		Russian = {
			Name = "Russian",
			ComponentsText = "Russian component page",
			Sec1Name = "Russian section #1",
			Message = "Russian message",
		},
		Spanish = {
			Name = "Spanish",
			ComponentsText = "Spanish component page",
			Sec1Name = "Spanish section #1",
			Message = "Spanish message",
		},
		Swedish = {
			Name = "Swedish",
			ComponentsText = "Swedish component page",
			Sec1Name = "Swedish section #1",
			Message = "Swedish message",
		},
		TradChinese = {
			Name = "Traditional Chinese",
			ComponentsText = "Traditional Chinese component page",
			Sec1Name = "Traditional Chinese section #1",
			Message = "Traditional Chinese message",
		},
		SimpChinese = {
			Name = "Simplified Chinese",
			ComponentsText = "Simplified Chinese component page",
			Sec1Name = "Simplified Chinese section #1",
			Message = "Simplified Chinese message",
		},
		Slovak = {
			Name = "Slovak",
			ComponentsText = "Slovak component page",
			Sec1Name = "Slovak section #1",
			Message = "Slovak message",
		},
	},
	ask = {
		title = "Installer Language",
		info = "Please select the language of the installer",
		allLanguages = true,
	},
}

-- A section named by a language string; the `!` draws it in bold.
local sec1 = section("!" .. lang.Sec1Name, function()
	messageBox { text = lang.Message }
end)

installer {
	-- Not exactly translated, but it shows what's needed.
	page.license {
		file = {
			English = "bigtest.nsi",
			Czech = "WordFuncTest.nsi",
			Dutch = "waplugin.nsi",
			French = "example1.nsi",
			German = "example2.nsi",
			Korean = "gfx.nsi",
			Russian = "languages.nsi",
			Spanish = "LogicLib.nsi",
			Swedish = "makensis.nsi",
			TradChinese = "one-section.nsi",
			SimpChinese = "primes.nsi",
			Slovak = "silent.nsi",
		},
	},
	page.components { topText = lang.ComponentsText },
	page.instFiles {},

	sec1,

	-- The old, slow, wasteful way: see why a language string is so much easier.
	section("Section number two", function()
		if LANGUAGE == LANG.English then messageBox { text = "Installing English stuff" } end
		if LANGUAGE == LANG.Czech then messageBox { text = "Installing Czech stuff" } end
		if LANGUAGE == LANG.Dutch then messageBox { text = "Installing Dutch stuff" } end
		if LANGUAGE == LANG.French then messageBox { text = "Installing French stuff" } end
		if LANGUAGE == LANG.German then messageBox { text = "Installing German stuff" } end
		if LANGUAGE == LANG.Korean then messageBox { text = "Installing Korean stuff" } end
		if LANGUAGE == LANG.Russian then messageBox { text = "Installing Russian stuff" } end
		if LANGUAGE == LANG.Spanish then messageBox { text = "Installing Spanish stuff" } end
		if LANGUAGE == LANG.Swedish then messageBox { text = "Installing Swedish stuff" } end
		if LANGUAGE == LANG.TradChinese then messageBox { text = "Installing Traditional Chinese stuff" } end
		if LANGUAGE == LANG.SimpChinese then messageBox { text = "Installing Simplified Chinese stuff" } end
		if LANGUAGE == LANG.Slovak then messageBox { text = "Installing Slovak stuff" } end
	end),
}
