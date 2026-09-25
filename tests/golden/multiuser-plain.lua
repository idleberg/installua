-- MultiUser.nsh with neither an uninstaller nor MUI2. Without an
-- `uninstaller {}` the header is told so, or it writes `un.` functions nothing
-- calls; and with no `.onInit` written, the compiler writes one to hold the
-- init macro.

attributes {
	name = "Mine",
	outFile = "multiuser-plain.exe",
}

multiUser {
	executionLevel = "standard",
	folder = "Mine",
	folderRegistry = { key = "Software/Mine", value = "Path" },
}

installer {
	section("Core", function()
		setOutPath(INSTDIR)
		writeReg(SHCTX, "Software/Mine", "Path", INSTDIR)
	end),
}
