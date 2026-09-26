-- $NSISDIR/Examples/example1.nsi: one page for the folder, one section that
-- copies this script there.

attributes {
	name = "Example1",
	outFile = "example1.exe",
	requestExecutionLevel = "user",
	installDir = DESKTOP .. "/Example1",
}

installer {
	page.directory {},
	page.instFiles {},

	section("", function()
		setOutPath(INSTDIR)
		file("example1.nsi")
	end),
}
