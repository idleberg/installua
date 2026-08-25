-- Program 1 of five: a MUI installer with an uninstaller.
--
-- The point of this one is the *shape* — four blocks, the page world, uninstaller
-- duality, and the eight things that are emitted rather than written. It stays
-- deliberately shallow on expressions; programs 4 and 5 are where those live.

local APP <const> = "Example1"
local VERSION <const> = "1.4.2"
local REGKEY <const> = "Software/Microsoft/Windows/CurrentVersion/Uninstall/" .. APP

attributes {
	name = APP,
	outFile = APP .. "-" .. VERSION .. "-setup.exe",
	unicode = true,
	compressor = "lzma",
	requestExecutionLevel = "admin",
	versionInfo = {
		product = "1.4.2.0",
		keys = {
			ProductName = APP,
			ProductVersion = VERSION,
			CompanyName = "Example Ltd",
			LegalCopyright = "(c) Example Ltd",
			FileDescription = APP .. " installer",
			FileVersion = VERSION,
		},
	},
}

installer {
	installDir = PROGRAMFILES64 .. "/" .. APP,
	icon = "assets/install.ico",

	page.welcome {},
	page.license { file = "assets/LICENSE.txt" },
	page.directory {},
	page.instFiles {},
	page.finish {},

	-- `.onInit`'s leading dot is emitted, never written.
	onInit(function()
		local prior = readRegStr(HKLM, REGKEY, "InstallLocation")
		if prior ~= "" then
			-- `INSTDIR` is a predefined global and a *writable* one; see README.
			INSTDIR = prior
		end
	end),

	section("Core", function()
		setOutPath(INSTDIR)
		file("assets/Example1.exe")
		file("assets/README.txt")

		writeReg(HKLM, REGKEY, "DisplayName", APP)
		writeReg(HKLM, REGKEY, "DisplayVersion", VERSION)
		writeReg(HKLM, REGKEY, "InstallLocation", INSTDIR)
		writeReg(HKLM, REGKEY, "UninstallString", INSTDIR .. "/uninstall.exe")
		writeReg(HKLM, REGKEY, "NoModify", 1)

		writeUninstaller(INSTDIR .. "/uninstall.exe")
	end),

	-- An option, so the table form: the name stays first and unlabelled
	-- because it is the parameter, and the switch beside it is named.
	section { "Start menu shortcut",
		optional = true,
		body = function()
			createDirectory(SMPROGRAMS .. "/" .. APP)
			createShortcut(SMPROGRAMS .. "/" .. APP .. "/" .. APP .. ".lnk", INSTDIR .. "/Example1.exe")
		end,
	},
}

-- One block, and the `un.` prefix has no spelling at all.
uninstaller {
	icon = "assets/uninstall.ico",

	page.confirm {},
	page.instFiles {},

	onInit(function()
		local answer = messageBox {
			text = "Remove " .. APP .. " and all of its files?",
			buttons = "YESNO",
			icon = "QUESTION",
		}
		if answer == "NO" then
			os.exit()
		end
	end),

	section("Core", function()
		delete(INSTDIR .. "/Example1.exe")
		delete(INSTDIR .. "/README.txt")
		delete(INSTDIR .. "/uninstall.exe")

		delete(SMPROGRAMS .. "/" .. APP .. "/" .. APP .. ".lnk")
		rmDir(SMPROGRAMS .. "/" .. APP)

		rmDir(INSTDIR)
		deleteRegKey(HKLM, REGKEY)
	end),
}
