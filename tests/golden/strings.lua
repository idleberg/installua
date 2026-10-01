-- `string.find` counts from 1 as Lua does, and answers `0` for a miss where
-- Lua answers `nil`: a hit at the very start, a hit further on, and a miss.
--
-- `string.sub` takes Lua's positions and folds them to `StrCpy`'s length and
-- offset: a middle, the last character, a negative end, both negative, a
-- negative start with a positive end, which measures the string first, and a
-- start of `0`, which Lua reads as `1`.
--
-- Each subject is a plain `local`: a literal would fold at build time, and
-- this golden is about the run-time adapters.
--
-- `math.abs` and `math.max(n, 0)` are non-negative, which is what lets them
-- into `sleep`; `math.min` is only when every argument is.

attributes {
	name = "Strings",
	outFile = "strings-setup.exe",
}

installer {
	section("Core", function()
		local url, host = "ftp://host", "ftp.example.com"
		detailPrint(tostring(string.find(url, "ftp")))
		detailPrint(tostring(string.find(url, "://")))
		detailPrint(tostring(string.find(host, "://")))

		local word = "installua"
		detailPrint(string.sub(word, 2, 4))
		detailPrint(string.sub(word, -1))
		detailPrint(string.sub(word, 2, -2))
		detailPrint(string.sub(word, -3, -2))
		detailPrint(string.sub(word, -4, 7))
		detailPrint(string.sub(word, 0, 3))

		local n = tonumber(readEnvStr("DELAY"))
		sleep(math.abs(n))
		sleep(math.max(n, 0))
		detailPrint(tostring(math.min(n, 5, 3)))
	end),
}
