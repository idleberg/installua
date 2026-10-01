-- `setEnv` sets the installer's own environment, so a program it starts
-- afterwards inherits it. Both values go on the stack and `t s` pops them,
-- the name first: a runtime value is never spliced into the signature.
--
-- `home` is live across both calls, and `System::Call` is opaque, so each
-- call saves it.

attributes {
	name = "Environment",
	outFile = "environment.exe",
}

installer {
	section("Core", function()
		local home = readEnvStr("USERPROFILE")
		setEnv("NSISDIR", EXEDIR)
		setEnv("HOME", home)
		exec("makensis.exe")
		detailPrint(home)
	end),
}
