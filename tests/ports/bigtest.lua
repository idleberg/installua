-- $NSISDIR/Examples/bigtest.nsi: most of what the installer can do at run
-- time, a little of each, in four install types.

local regKey <const> = "SOFTWARE/NSISTest/BigNSISTest"
local uninstallKey <const> = "Software/Microsoft/Windows/CurrentVersion/Uninstall/BigNSISTest"

-- Globals: `n` so the `raw` in "Integer" can write it, and `iniTest` because
-- the original's `MyFunctionTest` reads `$8` without being passed it.
n = 0
iniTest = ""

attributes {
	name = "BigNSISTest",
	caption = "NSIS Big Test",
	icon = NSISDIR .. "/Contrib/Graphics/Icons/nsis1-install.ico",
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
}

func("CSCTest", function()
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

func("myfunc", function(x)
	local text = "MyTestVar=" .. x
	messageBox { text = "myfunc: " .. text }
end)

func("MyFunctionTest", function()
	if readIniStr(INSTDIR .. "/test.ini", "MySectionIni", "Value1") ~= iniTest then
		messageBox { text = "WriteINIStr failed" }
	end
end)

local TESTIDX = section { "Test Exec functions",
	installTypes = { "Most", "Full", "More" },
	body = function()
		local notepad = searchPath("notepad.exe")
		messageBox { text = "notepad.exe=" .. notepad }
		exec('"' .. notepad .. '"')
		execShell("open", '"' .. INSTDIR .. '"')
		sleep(500)
		bringToFront()
	end,
}

installer {
	installColors = "FF8080 000030",
	checkBitmap = NSISDIR .. "/Contrib/Graphics/Checks/classic-cross.bmp",
	installTypes = { "Most", "Full", "More", "Base" },

	page.license { file = "bigtest.nsi", bottomText = "A test text, make sure it's all there" },
	page.components {},
	page.directory {},
	page.instFiles {},

	section("", function()
		local hello = "Hello World"
		detailPrint("I like to be able to see what is going on (debug) " .. hello)
		writeReg(HKLM, regKey, "Install_Dir", INSTDIR)

		writeReg(HKLM, uninstallKey, "DisplayName", "BigNSISTest (remove only)")
		writeReg(HKLM, uninstallKey, "UninstallString", '"' .. INSTDIR .. '/bt-uninst.exe"')

		setOutPath(INSTDIR)
		-- The original's `File /a`: makensis warns that it does nothing off Windows.
		file("silent.nsi")
		createDirectory(INSTDIR .. "/MyProjectFamily/MyProject")
		writeUninstaller(INSTDIR .. "/bt-uninst.exe")
	end),

	section { "TempTest",
		installTypes = { "Most", "Full", "More" },
		body = function()
			local again = true
			while again do
				messageBox { text = "Start:" }
				if messageBox { text = "Goto MyLabel", buttons = "YESNO" } ~= "YES" then
					messageBox { text = "Right before MyLabel:" }
				end
				messageBox { text = "MyLabel:" }
				messageBox { text = "Right after MyLabel:" }
				again = messageBox { text = "Goto Start:?", buttons = "YESNO" } == "YES"
			end
		end,
	},

	group { "SectionGroup1", expanded = true, sections = {
		section { "Test Registry/INI functions",
			installTypes = { "Most", "Base", "More" },
			body = function()
				writeReg(HKLM, regKey, "StrTest_INSTDIR", INSTDIR)
				writeReg(HKLM, regKey, "DwordTest_0xDEADBEEF", 0xdeadbeef)
				writeReg(HKLM, regKey, "DwordTest_123456", 123456)
				writeReg(HKLM, regKey, "DwordTest_0123", 83) -- NSIS reads `0123` as octal
				writeRegBin(HKLM, regKey, "BinTest_deadbeef01f00dbeef", "DEADBEEF01F00DBEEF")
				iniTest = SYSDIR .. "/IniTest"
				writeIniStr(INSTDIR .. "/test.ini", "MySection", "Value1", iniTest)
				writeIniStr(INSTDIR .. "/test.ini", "MySectionIni", "Value1", iniTest)
				writeIniStr(INSTDIR .. "/test.ini", "MySectionIni", "Value2", iniTest)
				writeIniStr(INSTDIR .. "/test.ini", "IniOn", "Value1", iniTest)

				MyFunctionTest()

				deleteIniStr(INSTDIR .. "/test.ini", "IniOn", "Value1")
				deleteIniSection(INSTDIR .. "/test.ini", "MySectionIni")

				if readIniStr(INSTDIR .. "/test.ini", "MySectionIni", "Value1") ~= "" then
					messageBox { text = "DeleteINISec failed" }
				end

				clearErrors()
				local value = readRegStr(HKCR, "software/microsoft", "xyz_cc_does_not_exist")
				if errors() then
					messageBox { text = "could not read from HKCR\\software\\microsoft\\xyz_cc_does_not_exist" }
				else
					messageBox { text = "read '" .. value .. "' from HKCR\\software\\microsoft\\xyz_cc_does_not_exist" }
				end
			end,
		},

		section { "Test CreateShortcut",
			installTypes = { "Most", "Full", "More" },
			body = function()
				CSCTest()
			end,
		},

		group("Group2", {
			section("Integer", function()
				-- These are `raw` because `n = 1 << 31` would compile to
				-- `StrCpy $n -2147483648`: an expression whose operands are all
				-- constants is computed by the compiler, and this section tests
				-- NSIS's arithmetic at run time. `1 // 0` needs none: division
				-- by zero is never computed ahead.
				-- NSIS's arithmetic `>>` also has no spelling (see KNOWN-ISSUES.md).
				raw [[
					IntOp $n 0xffffffff >> 31
				]]
				if n ~= -1 then
					messageBox { text = "ASSERT: IntCmpU " .. n .. " -1 (bigtest.nsi:160)", icon = "STOP" }
				end
				raw [[
					IntOp $n 0xffffffff >>> 31
				]]
				if n ~= 1 then
					messageBox { text = "ASSERT: IntCmpU " .. n .. " 1 (bigtest.nsi:162)", icon = "STOP" }
				end
				raw [[
					IntOp $n 1 << 31
				]]
				if n ~= 0x80000000 then
					messageBox { text = "ASSERT: IntCmpU " .. n .. " 0x80000000 (bigtest.nsi:164)", icon = "STOP" }
				end
				raw [[
					IntOp $n 0x80000000 ^ 0x40000000
				]]
				if n ~= 0xC0000000 then
					messageBox { text = "ASSERT: IntCmpU " .. n .. " 0xC0000000 (bigtest.nsi:166)", icon = "STOP" }
				end
				clearErrors()
				n = 1 // 0
				-- Division by zero must set the error flag.
				if not errors() then
					messageBox { text = "ASSERT: IfErrors  (bigtest.nsi:169)", icon = "STOP" }
				end
				-- Unspecified result, hopefully it's zero.
				if n ~= 0 then
					messageBox { text = "ASSERT: IntCmpU " .. n .. " 0 (bigtest.nsi:170)", icon = "STOP" }
				end
			end),

			section { "Test Branching",
				installTypes = { "Most", "Full", "More" },
				body = function()
					while true do
						setOutPath(INSTDIR)
						if fileExists(INSTDIR .. "/LogicLib.nsi") then
							messageBox {
								text = "Would you like to overwrite " .. INSTDIR .. "/LogicLib.nsi?",
								buttons = "YESNO",
								icon = "QUESTION",
							}
						end
						file("LogicLib.nsi", { overwrite = "ifnewer" })

						if messageBox { text = "Would you like to skip the rest of this section?", buttons = "YESNO", icon = "QUESTION" } == "YES" then
							return
						end
						if messageBox { text = "Would you like to go back to the beginning of this section?", buttons = "YESNO", icon = "QUESTION" } ~= "YES" then
							break
						end
					end
					if messageBox { text = "Would you like to hide the installer and wait five seconds?", buttons = "YESNO", icon = "QUESTION" } == "YES" then
						hideWindow()
						sleep(5000)
						bringToFront()
					end
					if messageBox { text = "Would you like to call the function 5 times?", buttons = "YESNO", icon = "QUESTION" } == "YES" then
						local x = "x"
						while x ~= "xxxxxx" do
							myfunc(x)
							x = "x" .. x
						end
					end
				end,
			},
		}),

		section { "Test CopyFiles",
			installTypes = { "Most", "Full", "More" },
			body = function()
				setOutPath(INSTDIR .. "/cpdest")
				copyFiles(WINDIR .. "/*.ini", INSTDIR .. "/cpdest", 0)
			end,
		},
	} },

	TESTIDX,

	section { "Test ActiveX control registration",
		installTypes = { "Full" },
		body = function()
			unRegDll(SYSDIR .. "/spin32.ocx")
			sleep(1000)
			regDll(SYSDIR .. "/spin32.ocx")
			sleep(1000)
		end,
	},

	onSelChange(function()
		if TESTIDX.text ~= "" then
			TESTIDX.text = ""
		else
			TESTIDX.text = "TextInSection"
		end
	end),
}

uninstaller {
	icon = NSISDIR .. "/Contrib/Graphics/Icons/nsis1-uninstall.ico",

	page.confirm { topText = "This will uninstall example2. Hit next to continue." },
	page.instFiles {},

	section("Uninstall", function()
		deleteRegKey(HKLM, uninstallKey)
		deleteRegKey(HKLM, regKey)
		delete(INSTDIR .. "/silent.nsi")
		delete(INSTDIR .. "/LogicLib.nsi")
		delete(INSTDIR .. "/bt-uninst.exe")
		delete(INSTDIR .. "/test.ini")
		delete(SMPROGRAMS .. "/Big NSIS Test/*.*")
		rmDir(SMPROGRAMS .. "/BiG NSIS Test")

		if messageBox { text = "Would you like to remove the directory " .. INSTDIR .. "/cpdest?", buttons = "YESNO", icon = "QUESTION" } == "YES" then
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
