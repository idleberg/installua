-- Every callback NSIS calls by name, in both halves, besides the three MUI2
-- writes itself. The uninstaller's outcomes say `Uninst` where the
-- installer's say `Inst`, as NSIS spells them. `memento {}` puts its save at
-- the top of the `.onInstSuccess` written here, rather than a second one.

attributes {
	name = "Called",
	outFile = "callbacks.exe",
}

memento { root = HKLM, key = "Software/Called/Components" }

local docs = section { "Documentation",
	remember = "docs",
	body = function()
		setOutPath(INSTDIR)
	end,
}

installer {
	page.components {},
	page.directory {},
	page.instFiles {},

	section("Core", function()
		setOutPath(INSTDIR)
		writeUninstaller(INSTDIR .. "/uninstall.exe")
	end),

	docs,

	onInit(function()
		docs.selected = false
	end),

	onInstSuccess(function()
		detailPrint("installed")
	end),

	onInstFailed(function()
		detailPrint("failed")
	end),

	onVerifyInstDir(function()
		if not fileExists(INSTDIR .. "/..") then
			abort()
		end
	end),

	onGUIEnd(function()
		detailPrint("closing")
	end),

	onSelChange(function()
		if docs.selected then
			detailPrint("documentation")
		end
	end),

	onRebootFailed(function()
		detailPrint("reboot by hand")
	end),
}

uninstaller {
	page.directory {},
	page.instFiles {},

	section("Uninstall", function()
		delete(INSTDIR .. "/uninstall.exe")
		rmDir(INSTDIR)
	end),

	onInit(function()
		detailPrint("starting")
	end),

	onUninstSuccess(function()
		detailPrint("removed")
	end),

	onUninstFailed(function()
		detailPrint("failed")
	end),

	onVerifyInstDir(function()
		if not fileExists(INSTDIR .. "/uninstall.exe") then
			abort()
		end
	end),

	onGUIEnd(function()
		detailPrint("closing")
	end),

	onSelChange(function()
		detailPrint("changed")
	end),

	onRebootFailed(function()
		detailPrint("reboot by hand")
	end),
}
