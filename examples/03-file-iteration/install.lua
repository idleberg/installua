-- Program 3 of five (PLAN Phase 0, §11): a file-iteration loop.
--
-- The point of this one is loops and staging. Three iteration forms appear, and
-- a reader has to be able to tell which machine each runs on:
--
--   `for … in glob(…)`   build machine, unrolled, no NSIS loop at all
--   `for … in lines(f)`  install time, a FileRead loop
--   `for i = 1, n`       install time, a counted loop
--
-- `break` and `continue()` both appear, because they are the two terminators
-- that have no Lua-to-NSIS analogue: NSIS has only `Goto`, and Lua has no
-- `continue` at all (§8).

local APP <const> = "Example3"

attributes {
	name = APP,
	outFile = APP .. "-setup.exe",
	unicode = true,
}

installer {
	installDir = PROGRAMFILES64 .. "/" .. APP,
	pages = { "Directory", "InstFiles" },

	section("Core", function()
		setOutPath(INSTDIR)

		-- Build-machine iteration. The glob runs where `makensis` runs, so this
		-- unrolls into one `File` line per match and there is no loop in the
		-- output. Matches are sorted, so the golden file is stable.
		for path in glob("assets/*.txt") do
			file(path)
		end

		-- Install-time iteration over a file we just wrote. `lines` strips the
		-- line terminator, as Lua's does — which is not free; see README.
		local manifest = fileOpen(INSTDIR .. "/manifest.txt", "r")
		local count = 0
		for line in lines(manifest) do
			if line == "" then
				continue()
			end
			if line == "END" then
				break
			end
			count = count + 1
			detailPrint("entry " .. count .. ": " .. line)
		end
		manifest:close()

		if count == 0 then
			abort("the manifest is empty")
		end
		detailPrint("read " .. count .. " entries")

		-- A counted loop, and the one place `errors()` is read as the impure
		-- predicate it is (§15.20).
		for attempt = 1, 3 do
			clearErrors()
			createDirectory(INSTDIR .. "/cache")
			if not errors() then
				break
			end
			detailPrint("cache directory attempt " .. attempt .. " failed")
			sleep(200)
		end
	end),
}
