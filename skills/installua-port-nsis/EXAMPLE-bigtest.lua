-- bigtest.lua — a port of NSIS's Examples/bigtest.nsi.
--
-- The original exercises most of the exehead. Three things did not survive as
-- written, and each is a restructure rather than a spelling:
--   * every label and `Goto` (the two backwards jumps are loops),
--   * the ASSERT macro, which worked by a relative jump `+2`,
--   * `$8`, which the original passed from a section body into a function.
--
-- PORT NOTES — everything below compiles, so none of it is a build error. These
-- are the places the port is not behaviour-identical to the original. Grep
-- `PORT:` for each one in context.
--
--   1. `overwrite` is whole-program, so the `SetOverwrite ifnewer` window the
--      original opened around one `File` is gone. BEHAVIOUR CHANGE.
--   2. `SectionGroup Group2` was nested inside `SectionGroup1`; a group inside
--      a group is not-yet-implemented, so the tree is flattened. The components
--      page shows five siblings where the original showed three plus a subtree.
--   3. `!ifndef NOINSTTYPES` is dropped: `installTypes` wants a literal list,
--      and every section names a type, so the switch had nothing coherent to do.
--   4. `-D NOCOMPRESS` is now `-D COMPRESS=off`, and `-D NSISDIR=` is now
--      `-D NSIS_TREE=`. INVOCATION CHANGE.
--   5. `File /a` (`keepAttributes`) makes makensis warn 5050 on a non-Win32
--      build machine. The upstream .nsi warns identically; not a port defect.

local NSIS_TREE <const> = param("NSIS_TREE", "/opt/homebrew/Cellar/makensis@3.12/3.12/share/nsis")

-- PORT: (4) `!ifdef NOCOMPRESS` was a build-time switch, which is what `param` replaces
local COMPRESS <const> = param("COMPRESS", "auto")

local HAVE_UPX <const> = param("HAVE_UPX", false)
if HAVE_UPX then
	raw.tail [[
  !packhdr tmp.dat "upx\upx -9 tmp.dat"
]]
end

attributes {
	name = "BigNSISTest",
	caption = "NSIS Big Test",
	icon = NSIS_TREE .. "/Contrib/Graphics/Icons/nsis1-install.ico",
	outFile = "bigtest.exe",

	dateSave = true,
	datablockOptimize = true,
	crcCheck = true,
	silentInstall = "normal",
	bgGradient = { top = "000000", bottom = "800000", text = "FFFFFF" },

	installDir = PROGRAMFILES .. "/NSISTest/BigNSISTest",
	installDirRegKey = { root = HKLM, key = "Software/NSISTest/BigNSISTest", name = "Install_Dir" },

	requestExecutionLevel = "admin",
	manifest = { supportedOS = { "all" } },

	autoCloseWindow = false,
	showInstDetails = "show",

	-- PORT: (1) The original toggles SetOverwrite twice inside a section body.
	compress = COMPRESS,
	overwrite = "try",
}

iniTestValue = ""

shifted = 0
zero = 0

func("assertThat", function(ok, expr)
	if not ok then
		messageBox { text = "ASSERT: " .. expr, icon = "STOP" }
	end
end)

func("cscTest", function()
	createDirectory(SMPROGRAMS .. "/Big NSIS Test")
	setOutPath(INSTDIR)
	createShortcut(SMPROGRAMS .. "/Big NSIS Test/Uninstall BIG NSIS Test.lnk", INSTDIR .. "/bt-uninst.exe")
	createShortcut(SMPROGRAMS .. "/Big NSIS Test/silent.nsi.lnk", INSTDIR .. "/silent.nsi", {
		iconFile = WINDIR .. "/notepad.exe",
		iconIndex = 0,
		showMode = "SW_SHOWMINIMIZED",
		hotkey = "CONTROL|SHIFT|Q",
	})
	createShortcut(SMPROGRAMS .. "/Big NSIS Test/TheDir.lnk", INSTDIR .. "/", {
		iconIndex = 0,
		showMode = "SW_SHOWMAXIMIZED",
		hotkey = "CONTROL|SHIFT|Z",
	})
end)

func("myfunc", function(marker)
	messageBox { text = "myfunc: MyTestVar=" .. marker }
end)

func("myFunctionTest", function()
	local read = readIniStr(INSTDIR .. "/test.ini", "MySectionIni", "Value1")
	if read ~= iniTestValue then
		messageBox { text = "WriteINIStr failed" }
	end
end)

local hidden = section("", function()
	local greeting = "Hello World"
	detailPrint("I like to be able to see what is going on (debug) " .. greeting)
	writeReg(HKLM, "SOFTWARE/NSISTest/BigNSISTest", "Install_Dir", INSTDIR)

	writeReg(HKLM, "Software/Microsoft/Windows/CurrentVersion/Uninstall/BigNSISTest",
		"DisplayName", "BigNSISTest (remove only)")
	writeReg(HKLM, "Software/Microsoft/Windows/CurrentVersion/Uninstall/BigNSISTest",
		"UninstallString", '"' .. INSTDIR .. '/bt-uninst.exe"')

	setOutPath(INSTDIR)
	-- PORT: (5) `File /a` warns 5050 off Win32; the original does too.
	file("silent.nsi", { keepAttributes = true })
	createDirectory(INSTDIR .. "/MyProjectFamily/MyProject")
	writeUninstaller(INSTDIR .. "/bt-uninst.exe")
end)

local tempTest = section { "TempTest",
	installTypes = { "Most", "Full", "More" },
	body = function()
		while true do
			messageBox { text = "Start:" }

			if messageBox { text = "Goto MyLabel", buttons = "YESNO" } ~= "YES" then
				messageBox { text = "Right before MyLabel:" }
			end

			messageBox { text = "MyLabel:" }
			messageBox { text = "Right after MyLabel:" }

			if messageBox { text = "Goto Start:?", buttons = "YESNO" } ~= "YES" then
				break
			end
		end
	end,
}

local registryIni = section { "Test Registry/INI functions",
	installTypes = { "Most", "Base", "More" },
	body = function()
		writeReg(HKLM, "SOFTWARE/NSISTest/BigNSISTest", "StrTest_INSTDIR", INSTDIR)
		writeReg(HKLM, "SOFTWARE/NSISTest/BigNSISTest", "DwordTest_0xDEADBEEF", 0xdeadbeef)
		writeReg(HKLM, "SOFTWARE/NSISTest/BigNSISTest", "DwordTest_123456", 123456)
		writeReg(HKLM, "SOFTWARE/NSISTest/BigNSISTest", "DwordTest_0123", 123)
		writeRegBin(HKLM, "SOFTWARE/NSISTest/BigNSISTest",
			"BinTest_deadbeef01f00dbeef", "DEADBEEF01F00DBEEF")

		iniTestValue = SYSDIR .. "/IniTest"
		writeIniStr(INSTDIR .. "/test.ini", "MySection", "Value1", iniTestValue)
		writeIniStr(INSTDIR .. "/test.ini", "MySectionIni", "Value1", iniTestValue)
		writeIniStr(INSTDIR .. "/test.ini", "MySectionIni", "Value2", iniTestValue)
		writeIniStr(INSTDIR .. "/test.ini", "IniOn", "Value1", iniTestValue)

		myFunctionTest()

		deleteIniStr(INSTDIR .. "/test.ini", "IniOn", "Value1")
		deleteIniSection(INSTDIR .. "/test.ini", "MySectionIni")

		if readIniStr(INSTDIR .. "/test.ini", "MySectionIni", "Value1") ~= "" then
			messageBox { text = "DeleteINISec failed" }
		end

		clearErrors()
		local missing = readRegStr(HKCR, "software/microsoft", "xyz_cc_does_not_exist")
		if errors() then
			messageBox { text = "could not read from HKCR/software/microsoft/xyz_cc_does_not_exist" }
		else
			messageBox { text = "read '" .. missing .. "' from HKCR/software/microsoft/xyz_cc_does_not_exist" }
		end
	end,
}

local shortcuts = section { "Test CreateShortcut",
	installTypes = { "Most", "Full", "More" },
	body = function()
		cscTest()
	end,
}

local integer = section("Integer", function()
	clearErrors()
	raw [[
  IntOp $shifted 0xffffffff >> 31
]]
	assertThat(shifted == -1, "IntCmpU $0 -1")

	local logical = 0xffffffff >> 31
	assertThat(logical == 1, "IntCmpU $0 1")

	local shiftedLeft = 1 << 31
	assertThat(shiftedLeft == 0x80000000, "IntCmpU $0 0x80000000")

	local xored = 0x80000000 ~ 0x40000000
	assertThat(xored == 0xC0000000, "IntCmpU $0 0xC0000000")

	clearErrors()
	local quotient = 1 // zero
	assertThat(errors(), "IfErrors")
	assertThat(quotient == 0, "IntCmpU $0 0")
end)

local branching = section { "Test Branching",
	installTypes = { "Most", "Full", "More" },
	body = function()
		while true do
			setOutPath(INSTDIR)

			local overwriteIt = true
			if fileExists(INSTDIR .. "/LogicLib.nsi") then
				overwriteIt = messageBox {
					text = "Would you like to overwrite " .. INSTDIR .. "/LogicLib.nsi?",
					buttons = "YESNO",
					icon = "QUESTION",
				} ~= "NO"
			end
			if overwriteIt then
				file("LogicLib.nsi")
			end

			if messageBox {
				text = "Would you like to skip the rest of this section?",
				buttons = "YESNO",
				icon = "QUESTION",
			} == "YES" then
				break
			end

			if messageBox {
				text = "Would you like to go back to the beginning of this section?",
				buttons = "YESNO",
				icon = "QUESTION",
			} == "YES" then
				continue()
			end

			if messageBox {
				text = "Would you like to hide the installer and wait five seconds?",
				buttons = "YESNO",
				icon = "QUESTION",
			} ~= "NO" then
				hideWindow()
				sleep(5000)
				bringToFront()
			end

			if messageBox {
				text = "Would you like to call the function 5 times?",
				buttons = "YESNO",
				icon = "QUESTION",
			} ~= "NO" then
				local marker = "x"
				while true do
					myfunc(marker)
					marker = "x" .. marker
					if marker == "xxxxxx" then
						break
					end
				end
			end

			break
		end
	end,
}

local copyFilesTest = section { "Test CopyFiles",
	installTypes = { "Most", "Full", "More" },
	body = function()
		setOutPath(INSTDIR .. "/cpdest")
		copyFiles(WINDIR .. "/*.ini", INSTDIR .. "/cpdest", 0)
	end,
}

local execTest = section { "Test Exec functions",
	installTypes = { "Most", "Full", "More" },
	body = function()
		local notepad = searchPath("notepad.exe")
		messageBox { text = "notepad.exe=" .. notepad }
		exec('"' .. notepad .. '"')
		execShell("open", INSTDIR)
		sleep(500)
		bringToFront()
	end,
}

func(".onSelChange", function()
	if execTest.text == "" then
		execTest.text = "TextInSection"
	else
		execTest.text = ""
	end
end)

local activeX = section { "Test ActiveX control registration",
	installTypes = { "Full" },
	body = function()
		unRegDll(SYSDIR .. "/spin32.ocx")
		sleep(1000)
		regDll(SYSDIR .. "/spin32.ocx")
		sleep(1000)
	end,
}

installer {
	-- PORT: (3) The original wraps these in `!ifndef NOINSTTYPES`.
	installTypes = { "Most", "Full", "More", "Base" },

	checkBitmap = NSIS_TREE .. "/Contrib/Graphics/Checks/classic-cross.bmp",
	installColors = "FF8080 000030",

	page.license {
		file = "bigtest.nsi",
		topText = "A test text, make sure it's all there",
	},
	page.components {},
	page.directory {},
	page.instFiles {},

	hidden,
	tempTest,
	-- PORT: (2) `Group2` was nested here; nesting is unimplemented, so it is flattened.
	group("SectionGroup1", {
		registryIni,
		shortcuts,
		integer,
		branching,
		copyFilesTest,
	}),
	execTest,
	activeX,

}

uninstaller {
	icon = NSIS_TREE .. "/Contrib/Graphics/Icons/nsis1-uninstall.ico",

	page.confirm { topText = "This will uninstall example2. Hit next to continue." },
	page.instFiles {},

	section("Uninstall", function()
		deleteRegKey(HKLM, "Software/Microsoft/Windows/CurrentVersion/Uninstall/BigNSISTest")
		deleteRegKey(HKLM, "SOFTWARE/NSISTest/BigNSISTest")
		delete(INSTDIR .. "/silent.nsi")
		delete(INSTDIR .. "/LogicLib.nsi")
		delete(INSTDIR .. "/bt-uninst.exe")
		delete(INSTDIR .. "/test.ini")
		delete(SMPROGRAMS .. "/Big NSIS Test/*.*")
		rmDir(SMPROGRAMS .. "/BiG NSIS Test")

		if messageBox {
			text = "Would you like to remove the directory " .. INSTDIR .. "/cpdest?",
			buttons = "YESNO",
			icon = "QUESTION",
		} ~= "NO" then
			delete(INSTDIR .. "/cpdest/*.*")
			rmDir(INSTDIR .. "/cpdest")
		end

		rmDir(INSTDIR .. "/MyProjectFamily/MyProject")
		rmDir(INSTDIR .. "/MyProjectFamily")
		rmDir(INSTDIR)

		if fileExists(INSTDIR) then
			messageBox { text = "Note: " .. INSTDIR .. " could not be removed!" }
		end
	end),
}
