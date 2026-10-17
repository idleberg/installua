-- Phase 2's golden: everything the middle of the compiler does, once each.
--
-- Order-free resolution (the section calls `report`, declared below it), the
-- type lattice picking an instruction family, condition fusion, a `<const>`
-- folding away, `break` and `continue()` as terminators, and a global declared
-- by assigning to it.
--
-- Three numeric `for`s: counting up, counting down — whose test is signed,
-- since the counter steps past `1` to `-1` — and a step read at run time,
-- whose sign is tested at the top of every turn, a zero step ending the loop.
--
-- And a `while` that counts down: `remaining` starts non-negative and the
-- subtraction widens it, so its test is signed too, though the test is lowered
-- above the subtraction.

attributes {
	name = "Control Flow",
	outFile = "control-flow-setup.exe",
}

local VERBOSE <const> = false

installer {
	section("Core", function()
		local target = INSTDIR .. "/app.exe"
		local present = fileExists(target)

		if present and not silent() then
			detailPrint("upgrading " .. INSTDIR)
		else
			detailPrint("installing " .. INSTDIR)
		end

		if VERBOSE then
			detailPrint("this line never reaches the .nsi")
		end

		local width = string.len(target) // 2
		local i = 1
		while i <= width do
			i = i + 1
			if i == 3 then
				continue()
			end
			if i > 8 then
				break
			end
			detailPrint("step " .. i)
		end

		for pass = 1, 3 do
			detailPrint("pass " .. pass)
		end

		for left = 3, 1, -2 do
			detailPrint("left " .. left)
		end

		local stride = tonumber(readEnvStr("STRIDE"))
		for at = 1, width, stride do
			detailPrint("at " .. at)
		end

		local remaining = 3
		while remaining > 0 do
			remaining = remaining - 1
		end

		state = "installed"
		report()
	end),
}

func("report", function()
	detailPrint("state is " .. state)
end)
