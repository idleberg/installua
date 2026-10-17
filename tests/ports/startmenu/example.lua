-- $NSISDIR/Examples/StartMenu/Example.nsi: StartMenu.dll's folder picker on a
-- custom page, and shortcuts to MakeNSISw in the folder it returns.

local startMenu = plugin "StartMenu"

folder = ""

attributes {
	name = "StartMenu.dll test",
	outFile = "StartMenu Test.exe",
	installDir = NSISDIR,
}

installer {
	page.directory {
		topText = "This installer will create some shortcuts to MakeNSIS in the start menu.\nFor this it needs NSIS's path.",
		destinationText = "Please specify the path in which you have installed NSIS:",
	},
	onVerifyInstDir(function()
		if not fileExists(INSTDIR .. "/makensis.exe") then
			abort()
		end
	end),

	page.custom { ": Start Menu Folder",
		dialog = false,
		show = function()
			local outcome, chosen = startMenu.select("StartMenu.dll test", {
				checknoshortcuts = "Don't create a start menu folder",
				autoadd = true,
				lastused = folder,
			})
			if outcome == "success" then
				folder = chosen
			elseif outcome ~= "cancel" then
				messageBox { text = outcome }
				folder = "StartMenu.dll test" -- use default
			end
		end,
	},
	page.instFiles {},

	section("", function()
		-- this part is only necessary if you used `checknoshortcuts`
		if string.sub(folder, 1, 1) ~= ">" then
			createDirectory(SMPROGRAMS .. "/" .. folder)
			createShortcut(SMPROGRAMS .. "/" .. folder .. "/MakeNSISw.lnk", INSTDIR .. "/makensisw.exe")

			setShellVarContext("all")
			createDirectory(SMPROGRAMS .. "/" .. folder)
			createShortcut(SMPROGRAMS .. "/" .. folder .. "/All users MakeNSISw.lnk", INSTDIR .. "/makensisw.exe")
		end
	end),
}
