-- Program 4 of five (PLAN Phase 0, §11): functions returning multiple values.
--
-- This is the program the clobber-set fixpoint exists for. Four call edges,
-- one of them recursive, and a live value across each one:
--
--   section -> budget -> measure -> ${GetSize}
--   section -> countdown -> countdown
--
-- Lua's multiple returns are the feature NSIS was waiting for (§3): `Call` has
-- no argument list, so the stack is the only calling convention there is, and a
-- language with `return a, b` maps onto it exactly.

local APP <const> = "Example4"

local fileFunc = import "FileFunc"

attributes {
	name = APP,
	outFile = APP .. "-setup.exe",
	unicode = true,
}

-- Two outputs out of a header macro's three. The third is dropped, which the
-- declaration has to know about before the allocator can be told anything.
func("measure", function(dir)
	local size, files, _ = fileFunc.getSize(dir, "")
	return size, files
end)

-- Two outputs, one of them derived. `//` on a `${GetSize}` result: see README
-- for why no sign fixup is emitted.
func("budget", function(dir)
	local size, files = measure(dir)
	return size // 1024, files
end)

-- An SCC of one. The fixpoint saturates in one extra round, which is the whole
-- special case recursion needs (§15.11).
func("countdown", function(n)
	if n <= 0 then
		return 0
	end
	local rest = countdown(n - 1)
	return rest + n
end)

installer {
	installDir = PROGRAMFILES64 .. "/" .. APP,
	pages = { "Directory", "InstFiles" },

	section("Core", function()
		setOutPath(INSTDIR)
		file("assets/payload.bin")

		local kib, files = budget(INSTDIR)

		-- Both are live across this call and `countdown` clobbers both, so this
		-- is the caller-save the phase exists to get right.
		local total = countdown(4)

		detailPrint("payload: " .. files .. " files, " .. kib .. " KiB, check " .. total)
	end),
}
