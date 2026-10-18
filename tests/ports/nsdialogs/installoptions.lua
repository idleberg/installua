-- $NSISDIR/Examples/nsDialogs/InstallOptions.nsi: an InstallOptions INI file
-- drawn by nsDialogs, and the fields it filled in read back.

-- `NSD_FUNCTION_INIFILE` writes `CreateDialogFromINI` and `UpdateINIState`,
-- which have no spelling.
raw.tail [[
!include nsDialogs.nsh
!insertmacro NSD_FUNCTION_INIFILE
]]

attributes {
	name = "nsDialogs IO",
	outFile = "nsDialogs IO.exe",
	showInstDetails = "show",
}

installer {
	page.custom {
		dialog = false,
		show = function()
			file("../InstallOptions/test.ini", { outName = PLUGINSDIR .. "/io.ini" })

			if
				messageBox {
					text = "Test the right-to-left version?",
					buttons = "YESNO",
					icon = "QUESTION",
					defaultAnswer = "NO",
				} == "YES"
			then
				writeIniStr(PLUGINSDIR .. "/io.ini", "Settings", "RTL", "1")
			end

			raw [[
				StrCpy $0 $PLUGINSDIR\io.ini
				Call CreateDialogFromINI
			]]
		end,
		leave = function()
			raw [[Call UpdateINIState]]
		end,
	},
	page.instFiles {},

	section("", function()
		detailPrint("Install X=" .. readIniStr(PLUGINSDIR .. "/io.ini", "Field 2", "State"))
		detailPrint("Install Y=" .. readIniStr(PLUGINSDIR .. "/io.ini", "Field 3", "State"))
		detailPrint("Install Z=" .. readIniStr(PLUGINSDIR .. "/io.ini", "Field 4", "State"))
		detailPrint("File=" .. readIniStr(PLUGINSDIR .. "/io.ini", "Field 5", "State"))
		detailPrint("Dir=" .. readIniStr(PLUGINSDIR .. "/io.ini", "Field 6", "State"))
		detailPrint("Info=" .. readIniStr(PLUGINSDIR .. "/io.ini", "Field 8", "State"))
	end),
}
