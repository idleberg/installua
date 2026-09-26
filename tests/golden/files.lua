-- `AllowSkipFiles` and `SetOverwrite` for one `file` call. NSIS's commands are
-- build-time state that holds for every `File` after it in the script, and
-- bodies here are laid out by the compiler, so each option is written before its
-- one `File` and put back to the `attributes {}` value after it. `outName` is
-- `File /oname=`, which unpacks one file under another name.

attributes {
	name = "Files",
	outFile = "files-setup.exe",
	overwrite = "ifnewer",
}

installer {
	page.instFiles {},

	section("", function()
		setOutPath(INSTDIR)
		file("LICENSE.txt")
		-- A locked target has no Ignore button, so Cancel is the only way out.
		file("LICENSE.txt", { allowSkip = false, nonFatal = true })
		-- Already the default, so nothing is written around it.
		file("LICENSE.txt", { allowSkip = true, overwrite = "ifnewer" })
		-- Both, put back in the reverse order.
		file("LICENSE.txt", { allowSkip = false, overwrite = "off" })
		file("LICENSE.txt", { outName = "COPYING" })
		-- The flag and the name are one token, quoted as one.
		file("LICENSE.txt", { outName = INSTDIR .. "/docs/read me.txt" })
	end),
}
