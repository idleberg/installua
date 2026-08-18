-- The page surface (§15.7): every MUI2 setting that belongs to a page rather
-- than to the block, and the two block settings that look page-scoped and are
-- not.
--
-- The interesting part is the *second* Directory page. MUI2 reads a page-scoped
-- `!define` at the point the macro is inserted and undefines it after, so each
-- page's settings have to sit between two insertions rather than in one list up
-- front — and the settings MUI2 forgets to clear (`MUI_DIRECTORYPAGE_VARIABLE`,
-- `…_VERIFYONLEAVE`) are undefined here instead.

attributes {
	outFile = "pages.exe",
	name = "Pages",
}

-- A global, because `DirVar` takes a *variable* and not a value: NSIS stores
-- the chosen directory into it (§15.24).
dataDir = ""

installer {
	-- The four settings MUI2 writes inside an `!ifndef` interface guard, which
	-- runs on the first page of its type and never again. They are the block's
	-- for that reason, not the page's.
	checkBitmap = "check.bmp",
	installColors = "FFFFFF 000000",
	progressBar = "smooth",
	licenseBkColor = "/windows",

	page.welcome {
		pre = function()
			detailPrint("about to greet")
		end,
	},

	page.license {
		file = "LICENSE.txt",
		bottomText = "Read it, then choose.",
		button = "Agree",
		checkbox = "I accept the terms.",
	},

	page.components {
		headerText = "Components",
		headerSubText = "Choose what to install.",
		topText = "Pick the parts you want.",
		instTypeText = "Preset:",
		listText = "Parts:",
	},

	page.directory {
		topText = "Choose where the program goes.",
		destinationText = "Program folder:",
		verifyOnLeave = true,
		leave = function()
			detailPrint("leaving the program folder page")
		end,
	},

	-- The second page of the same type: none of the settings above reach it.
	page.directory {
		topText = "And where the data goes.",
		variable = dataDir,
	},

	-- The eighth page, and the only one whose body is the compiler's: `Page
	-- custom` names a creator and a leave function, so `pre` and `show` are
	-- inlined either side of the dialog and `leave` becomes the second name on
	-- the line (§15.32). The header is a `MUI_HEADER_TEXT` call rather than the
	-- `!define` the other five write, because MUI2 never reads a define for a
	-- page it did not insert.
	page.custom { "Registration",
		headerText = "Serial number",
		headerSubText = "Enter the key from your invoice.",
		pre = function()
			detailPrint("about to build the dialog")
		end,
		show = function()
			detailPrint("the dialog is up")
		end,
		leave = function()
			detailPrint("leaving the dialog")
		end,
	},

	page.instFiles {},
	page.finish {},

	section("Core", function()
		writeUninstaller(INSTDIR .. "/un.exe")
	end),
}

uninstaller {
	page.confirm {
		topText = "Pages will be removed.",
		locationText = "From:",
	},

	-- The same page with nothing on it, in the other half: no caption, no
	-- `leave`, no header. `UninstPage custom` with one name after it is the
	-- shortest line this construct can produce, and `un.` leads the function
	-- because NSIS spells the half in the name (§15.3).
	page.custom {},

	page.instFiles {},

	section("Core", function()
		delete(INSTDIR .. "/un.exe")
	end),
}
