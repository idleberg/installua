-- Program 5 of five: mostly string and integer work.
--
-- This is where the operator and stdlib rules stop being tables and start being
-- arguments. Four
-- things are under test and each has a silent-wrong-answer failure mode:
--
--   * `//` and `%` corrected to Lua's meaning where the sign is not provable
--   * `string.sub`'s 1-based → 0-based conversion, and `string.find`'s
--   * `lower(a) == lower(b)` peepholing to a bare `StrCmp`
--   * `StrFunc` init lines, which abort the build when missing

local APP <const> = "Example5"
local VERSION <const> = "2.1.0"
local REGKEY <const> = "Software/Example5"
local BLOCK_MIB <const> = 64

local fileFunc = import "FileFunc"
local wordFunc = import "WordFunc"

attributes {
	name = APP,
	outFile = APP .. "-setup.exe",
	unicode = true,
}

-- "2.1.0" -> "2". Two off-by-one conversions meet here and cancel; see README.
func("majorOf", function(v)
	local dot = string.find(v, ".")
	return string.sub(v, 1, dot - 1)
end)

installer {
	installDir = PROGRAMFILES64 .. "/" .. APP,

	page.directory {},
	page.instFiles {},

	section("Core", function()
		setOutPath(INSTDIR)

		local installed = readRegStr(HKLM, REGKEY, "DisplayVersion")
		if installed ~= "" then
			if wordFunc.versionCompare(installed, VERSION) == "1" then
				abort("version " .. installed .. " is newer than " .. VERSION)
			end
			detailPrint("upgrading from major " .. majorOf(installed))
		end

		local channel = readRegStr(HKLM, REGKEY, "Channel")
		if string.len(channel) > 32 then
			abort("the channel name is implausible")
		end

		-- Case-insensitive equality: a bare `StrCmp`, no `${StrCase}`, no
		-- temporary. Case conversion for its *value* does cost a macro.
		if string.lower(channel) == "beta" then
			detailPrint("banner: " .. string.upper(channel))
		end

		-- The lattice knows `driveSpace` is a `uint`, and knows nothing about
		-- `delta`. So the first `//` needs no fixup and the second pair does.
		local freeMib = fileFunc.driveSpace("C:/", "/D=F /S=M")
		local freeGib = freeMib // 1024

		local delta = freeMib - 512
		local blocks = delta // BLOCK_MIB
		local slack = delta % BLOCK_MIB

		detailPrint(string.format("%04d", blocks) .. " blocks, " .. slack .. " MiB over")
		detailPrint("free: " .. freeGib .. " GiB")
	end),
}
