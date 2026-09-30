-- `string.find` counts from 1 as Lua does, and answers `0` for a miss where
-- Lua answers `nil`: a hit at the very start, a hit further on, and a miss.
--
-- `string.sub` takes Lua's positions and folds them to `StrCpy`'s length and
-- offset: a middle, the last character, a negative end, both negative, and a
-- start of `0`, which Lua reads as `1`.
--
-- `math.abs` and `math.max(n, 0)` are non-negative, which is what lets them
-- into `sleep`; `math.min` is only when every argument is.

attributes {
	name = "Strings",
	outFile = "strings-setup.exe",
}

installer {
	section("Core", function()
		detailPrint(tostring(string.find("ftp://host", "ftp")))
		detailPrint(tostring(string.find("ftp://host", "://")))
		detailPrint(tostring(string.find("ftp.example.com", "://")))

		detailPrint(string.sub("installua", 2, 4))
		detailPrint(string.sub("installua", -1))
		detailPrint(string.sub("installua", 2, -2))
		detailPrint(string.sub("installua", -3, -2))
		detailPrint(string.sub("installua", 0, 3))

		local n = tonumber(readEnvStr("DELAY"))
		sleep(math.abs(n))
		sleep(math.max(n, 0))
		detailPrint(tostring(math.min(n, 5, 3)))
	end),
}
