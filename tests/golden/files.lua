-- `AllowSkipFiles` for one `file` call. NSIS's command is build-time state that
-- holds for every `File` after it in the script, and bodies here are laid out by
-- the compiler, so the option is written before its one `File` and put back to
-- the `attributes {}` value after it.

attributes {
	name = "Files",
	outFile = "files-setup.exe",
}

installer {
	page.instFiles {},

	section("", function()
		setOutPath(INSTDIR)
		file("LICENSE.txt")
		-- A locked target has no Ignore button, so Cancel is the only way out.
		file("LICENSE.txt", { allowSkip = false, nonFatal = true })
		-- Already the default, so nothing is written around it.
		file("LICENSE.txt", { allowSkip = true })
	end),
}
