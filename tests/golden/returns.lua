-- Phase 3's golden: parameters, multiple returns, and the caller-saves that
-- fall out of §15.11's clobber fixpoint.
--
-- This is §11's program 4 with the parts Phase 4 owns removed — `import`, the
-- `${GetSize}` macro, `file` and the MUI pages — and nothing else changed. Its
-- shape is the same one program 4's hand-written expectation pins:
--
--   section -> budget -> measure
--   section -> countdown -> countdown
--
-- Four call edges, one of them recursive, and a live value across each.

attributes {
	name = "Returns",
	outFile = "returns-setup.exe",
	unicode = true,
}

-- Two outputs, both derived from one measurement. `string.len` is non-negative
-- by construction, so neither `//` pays §15.4's sign fixup — and the sign
-- travels out through the return type, which is what makes `budget` below free
-- of one too.
func("measure", function(dir)
	local n = string.len(dir)
	return n, n // 2
end)

-- Called from a section that is declared above it, and calling a `func`
-- declared above that: resolution is order-free either way (§15.6).
func("budget", function(dir)
	local size, half = measure(dir)
	return size // 1024, half
end)

-- An SCC of one. The fixpoint saturates in one extra round, which is the whole
-- special case recursion needs (§15.11) — and the depth-cliff warning is the
-- honest response to §3's silent death at ~1300 frames.
func("countdown", function(n)
	if n <= 0 then
		return 0
	end
	local rest = countdown(n - 1)
	return rest + n
end)

installer {
	section("Core", function()
		setOutPath(INSTDIR)

		local kib, files = budget(INSTDIR)

		-- Both are live across this call and `countdown` clobbers registers, so
		-- this is the caller-save the phase exists to get right.
		local total = countdown(4)

		detailPrint("payload: " .. files .. " halves, " .. kib .. " KiB, check " .. total)
	end),
}
