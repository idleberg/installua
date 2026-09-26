-- $NSISDIR/Examples/silent.nsi: a message box that answers itself when the
-- installer is silent, and a locked file that `file()` cannot write, skipped
-- with Ignore and then, with `allowSkip = false`, cancelled.

attributes {
	name = "Silent",
	outFile = "silent.exe",
	requestExecutionLevel = "user",
}

installer {
	page.components {},
	page.directory {},
	page.instFiles {},

	onInit(function()
		local answer = messageBox {
			text = "Would you like the installer to be silent from now on?",
			buttons = "YESNO",
			icon = "QUESTION",
			silentAnswer = "YES",
		}
		if answer == "YES" then
			setSilent("silent")
		else
			setSilent("normal")
		end
	end),

	section("", function()
		if silent() then
			messageBox { text = 'This is a "silent" installer', icon = "INFORMATION" }
		end
		messageBox { text = "This is not a silent installer", icon = "INFORMATION", silentAnswer = "OK" }

		local f = fileOpen(TEMP .. "/silentOverwrite", "w")
		file("silent.nsi", { outName = TEMP .. "/silentOverwrite" })
		f:close()

		messageBox { text = "This message box always shows if the installer is silent", icon = "INFORMATION" }

		f = fileOpen(TEMP .. "/silentOverwrite", "w")
		file("silent.nsi", { outName = TEMP .. "/silentOverwrite", allowSkip = false })
		f:close()
	end),
}
