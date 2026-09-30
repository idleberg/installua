-- The texts NSIS expands when it shows them rather than when it builds them:
-- `Name`, `Caption` and the rest are language strings, and a MUI2 page text
-- ends up inside an instruction. So each may name a global, and shows what
-- `.onInit` put in it.
--
-- `outFile` is read while building and still refuses one.

title = ""
site = ""

attributes {
	name = title,
	caption = title .. " Setup",
	brandingText = title,
	outFile = "late-texts.exe",
}

installer {
	onInit(function()
		title = readEnvStr("TITLE")
		site = readEnvStr("SITE")
	end),
	page.instFiles {},
	page.finish {
		readme = { path = "", text = "Visit " .. site },
	},
	section("Core", function()
		detailPrint(title)
	end),
}
