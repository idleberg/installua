-- The six macros NSIS calls back into, and the one thing that makes them
-- different from every other declaration: there is no loop in the output.
--
-- NSIS calls the script once per file or per line, so each body below becomes
-- a `Function`, and the loop's two exits become the two strings that function
-- pushes. Falling off the end pushes `""` and carries on; `break` pushes
-- `StopLocate` and the walk ends. Read the generated functions at the bottom
-- of the `.nsi` beside these bodies -- that inversion is the whole feature.
--
-- The arguments arrive in registers NSIS chose (`$R9` down to `$R6` here,
-- `$9` down to `$6` in TextFunc), which is why every prologue pushes them all
-- before popping any: a slot the allocator colours `$R8` would otherwise
-- clobber the second value before the copy that reads it.

local fileFunc = import "FileFunc"
local textFunc = import "TextFunc"

-- Joined at each call rather than bound to a local, because a `path` parameter
-- normalises `/` to `\` at the call and a local would arrive as the string it
-- is.
local LOG <const> = "/install.log"
local CONF <const> = "/app.conf"
local DIST <const> = "/app.conf.new"

attributes {
	name = "Walkers",
	outFile = "walkers-setup.exe",
	unicode = true,
}

installer {
	section("Clean", function()
		-- Four values, all four bound. `/L=F` restricts the walk to files, so
		-- `size` is always a number here -- for a directory the macro leaves
		-- it empty.
		for path, directory, name, size in fileFunc.locate(INSTDIR, "/L=F /M=*.tmp") do
			if size > 1048576 then
				detailPrint("large leftover: " .. name .. " in " .. directory)
			end
			delete(path)
		end

		-- One of four, which is the common call: the names a body does not
		-- want are registers the prologue never reads.
		for path in fileFunc.locate(INSTDIR, "/L=D /M=cache") do
			rmDir(path)
		end
	end),

	section("Survey", function()
		-- `break` is the interesting line: it is not a jump, it is
		-- `Push "StopGetDrives"` followed by `Return`, because ending the walk
		-- is something only NSIS can do.
		for drive, kind in fileFunc.getDrives("ALL") do
			if kind == "CDROM" then
				detailPrint("skipping " .. drive)
				break
			end
			detailPrint(drive .. " is a " .. kind)
		end

		-- Backwards from the end of a file, and a bare `return` means "done
		-- with this line" -- the same thing as falling off the end, and not a
		-- return at all.
		for line, remaining in textFunc.fileReadFromEnd(INSTDIR .. LOG) do
			if line == "" then
				return
			end
			detailPrint(remaining .. ": " .. line)
		end
	end),

	section("Diff", function()
		-- Two files at once. `other` is the second file's line and `match` its
		-- number, `0` where nothing matched -- the pair that makes this a diff
		-- rather than a walk.
		for line, number, other, match in
			textFunc.textCompare(INSTDIR .. CONF, INSTDIR .. DIST, "FastDiff") do
			if match == 0 then
				detailPrint("only ours, line " .. number .. ": " .. line)
			else
				detailPrint("theirs: " .. other)
			end
		end

		-- The one that is not a loop. Its body answers with a **value**, and
		-- three different ones: a string is the line to write, `skip` drops
		-- the line, `stop` ends the walk. A `for` body has nowhere to put the
		-- first of those, which is the whole reason this keeps NSIS's shape.
		textFunc.lineFind(INSTDIR .. CONF, INSTDIR .. DIST, "1:-1", function(line, number)
			if number > 500 then
				return stop
			end
			if line == "DEBUG=1" then
				return skip
			end
			return line
		end)

		-- Case-sensitively, which is a different macro in NSIS and so a
		-- different method here. Nothing about the callback changes: folding
		-- happens before it is called.
		for line in textFunc.textCompareS(INSTDIR .. CONF, INSTDIR .. DIST, "FastEqual") do
			detailPrint("same: " .. line)
		end
	end),
}
