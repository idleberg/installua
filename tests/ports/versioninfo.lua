-- $NSISDIR/Examples/VersionInfo.nsi: the version information Windows shows on
-- the installer's Properties tab.

attributes {
	name = "Version Info",
	outFile = "VersionInfo.exe",
	versionInfo = {
		product = "1.2.3.4",
		keys = {
			ProductName = "Test Application",
			Comments = "A test comment",
			CompanyName = "Fake company",
			LegalTrademarks = "Test Application is a trademark of Fake company",
			LegalCopyright = "Copyright Fake company",
			FileDescription = "Test Application",
			FileVersion = "1.2.3",
		},
	},
}

installer {
	section("", function() end),
}
