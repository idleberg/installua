-- `string.find` counts from 1 as Lua does, and answers `0` for a miss where
-- Lua answers `nil`: a hit at the very start, a hit further on, and a miss.

attributes {
	name = "Strings",
	outFile = "strings-setup.exe",
}

installer {
	section("Core", function()
		detailPrint(tostring(string.find("ftp://host", "ftp")))
		detailPrint(tostring(string.find("ftp://host", "://")))
		detailPrint(tostring(string.find("ftp.example.com", "://")))
	end),
}
