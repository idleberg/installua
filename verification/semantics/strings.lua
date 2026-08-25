-- Tier 4: the string adapters and the sign fixups, run under wine.
--
-- Assembling proves nothing here. Every value below has a plausible wrong
-- answer that `makensis` accepts without a word:
--
--   * `string.find` is 0-based in NSIS and 1-based in Lua, and `string.sub`
--     takes a length where Lua takes an end position — two off-by-ones that
--     cancel only if both are right;
--   * `//` truncates toward zero in NSIS and floors in Lua, and `%` takes the
--     sign of the dividend rather than the divisor — they disagree exactly when
--     one operand is negative, which is the case a golden file cannot reach
--     without running.

local BLOCK <const> = 64

attributes {
	name = "Strings",
	outFile = "strings.exe",
	unicode = true,
	requestExecutionLevel = "user",
}

func("majorOf", function(v)
	local dot = string.find(v, ".")
	return string.sub(v, 1, dot - 1)
end)

installer {
	section("Core", function()
		local out = fileOpen(EXEDIR .. "/strings-result.txt", "w")

		out:write("major=" .. majorOf("2.1.0") .. "\n")
		out:write("find=" .. string.find("2.1.0", ".") .. "\n")
		out:write("sub=" .. string.sub("abcdef", 2, 4) .. "\n")
		out:write("upper=" .. string.upper("beta") .. "\n")
		out:write("lower=" .. string.lower("BETA") .. "\n")
		out:write("fmt=" .. string.format("%04d", 42) .. "\n")

		-- `string.len` is non-negative by construction, so this subtraction is
		-- the one place the lattice loses the sign — which is what makes the
		-- two lines below carry the fixup rather than eliding it.
		local delta = string.len("abcdefgh") - 512
		out:write("div=" .. delta // BLOCK .. "\n")
		out:write("mod=" .. delta % BLOCK .. "\n")

		out:close()
	end),
}
