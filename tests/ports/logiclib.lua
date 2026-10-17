-- $NSISDIR/Examples/LogicLib.nsi: LogicLib's own tests. Each block builds a
-- string through the construct it tests and prints PASSED when the string is
-- the expected one.
--
-- The constants the original compares are in locals here: `1 == 1` would be
-- decided by the compiler, and these test the comparison at run time.

-- Written by the `raw` `FindFirst`/`FindNext`, which have no spelling.
findHandle = ""
found = ""

attributes {
	name = "NSIS LogicLib Example",
	outFile = "LogicLib.exe",
	showInstDetails = "show",
	requestExecutionLevel = "user",
}

func("DoLoop", function(i)
	while true do
		i = i + 1
		if i == 5 then
			return i
		end
	end
end)

local TESTS = section { "Run tests",
	optional = true,
	body = function()
		local one, two, three, four = 1, 2, 3, 4

		-- kinds of if other than "value1 comparison value2"
		clearErrors()
		raw [[
			FindFirst $findHandle $found "$PROGRAMFILES\*"
		]]
		if not errors() then
			while true do
				if found ~= "." and found ~= ".." then
					detailPrint("Found " .. PROGRAMFILES .. "/" .. found)
				end
				raw [[
					FindNext $findHandle $found
				]]
				if errors() then
					break
				end
			end
			raw [[
				FindClose $findHandle
			]]
		end

		if fileExists("logiclib.lua") then
			detailPrint('Source file "logiclib.lua" still exists')
		else
			detailPrint('Source file "logiclib.lua" has gone')
		end

		-- if..endif
		local r = ""
		if one == 1 then r = r .. "A" end
		if one == 2 then r = r .. "B" end
		if one < 2 then r = r .. "C" end
		if one < -2 then r = r .. "D" end
		if one > 2 then r = r .. "E" end
		if one > -2 then r = r .. "F" end
		if one ~= 1 then r = r .. "G" end
		if one ~= 2 then r = r .. "H" end
		if one >= 2 then r = r .. "I" end
		if one >= -2 then r = r .. "J" end
		if one <= 2 then r = r .. "K" end
		if one <= -2 then r = r .. "L" end
		if r == "ACFHJK" then
			detailPrint("PASSED If..EndIf test")
		else
			detailPrint("FAILED If..EndIf test")
		end

		-- if..elseif..else..endif; `ElseUnless x ~= y` is `elseif x == y`
		local a = "A"
		r = ""
		if a == "A" then
			r = r .. "A"
		elseif a == "B" then
			r = r .. "B"
		elseif a == "C" then
			r = r .. "C"
		else
			r = r .. "D"
		end
		if a == "D" then
			r = r .. "D"
		elseif a == "A" then
			r = r .. "A"
		elseif a == "B" then
			r = r .. "B"
		else
			r = r .. "C"
		end
		if a == "C" then
			r = r .. "C"
		elseif a == "D" then
			r = r .. "D"
		elseif a == "A" then
			r = r .. "A"
		else
			r = r .. "B"
		end
		if a == "B" then
			r = r .. "B"
		elseif a == "C" then
			r = r .. "C"
		elseif a == "D" then
			r = r .. "D"
		else
			r = r .. "A"
		end
		if r == a .. a .. a .. a then
			detailPrint("PASSED If..ElseIf..Else..EndIf test")
		else
			detailPrint("FAILED If..ElseIf..Else..EndIf test")
		end

		-- if..andif..orif..endif. LogicLib reads them left to right, so
		-- `AndIf` then `OrIf` is `(a and b) or c` and the reverse needs the
		-- parentheses.
		r = ""
		if one == 1 and two == 2 then r = r .. "A" else r = r .. "B" end
		if one == 1 and two == 3 then r = r .. "B" else r = r .. "A" end
		if one == 2 and two == 2 then r = r .. "B" else r = r .. "A" end
		if one == 2 and two == 3 then r = r .. "B" else r = r .. "A" end

		if one == 1 or two == 2 then r = r .. "A" else r = r .. "B" end
		if one == 1 or two == 3 then r = r .. "A" else r = r .. "B" end
		if one == 2 or two == 2 then r = r .. "A" else r = r .. "B" end
		if one == 2 or two == 3 then r = r .. "B" else r = r .. "A" end

		if one == 1 and two == 2 or three == 3 then r = r .. "A" else r = r .. "B" end
		if one == 1 and two == 3 or three == 3 then r = r .. "A" else r = r .. "B" end
		if one == 2 and two == 2 or three == 3 then r = r .. "A" else r = r .. "B" end
		if one == 2 and two == 3 or three == 3 then r = r .. "A" else r = r .. "B" end
		if one == 1 and two == 2 or three == 4 then r = r .. "A" else r = r .. "B" end
		if one == 1 and two == 3 or three == 4 then r = r .. "B" else r = r .. "A" end
		if one == 2 and two == 2 or three == 4 then r = r .. "B" else r = r .. "A" end
		if one == 2 and two == 3 or three == 4 then r = r .. "B" else r = r .. "A" end

		if (one == 1 or two == 2) and three == 3 then r = r .. "A" else r = r .. "B" end
		if (one == 1 or two == 3) and three == 3 then r = r .. "A" else r = r .. "B" end
		if (one == 2 or two == 2) and three == 3 then r = r .. "A" else r = r .. "B" end
		if (one == 2 or two == 3) and three == 3 then r = r .. "B" else r = r .. "A" end
		if (one == 1 or two == 2) and three == 4 then r = r .. "B" else r = r .. "A" end
		if (one == 1 or two == 3) and three == 4 then r = r .. "B" else r = r .. "A" end
		if (one == 2 or two == 2) and three == 4 then r = r .. "B" else r = r .. "A" end
		if (one == 2 or two == 3) and three == 4 then r = r .. "B" else r = r .. "A" end

		if r == "AAAAAAAAAAAAAAAAAAAAAAAA" then
			detailPrint("PASSED If..AndIf..OrIf..Else..EndIf test")
		else
			detailPrint("FAILED If..AndIf..OrIf..Else..EndIf test")
		end

		-- ifthen..|..|
		r = ""
		if one == 1 then r = r .. "A" end
		if one == 2 then r = r .. "B" end
		if not (one == 1) then r = r .. "C" end
		if not (one == 2) then r = r .. "D" end
		if r == "AD" then
			detailPrint("PASSED IfThen test")
		else
			detailPrint("FAILED IfThen test")
		end

		-- ifcmd..||..| and if/unless cmd
		r = ""
		if messageBox { text = "Please click Yes", buttons = "YESNO" } == "YES" then
			r = r .. "A"
		end
		if messageBox { text = "Please click No", buttons = "YESNO", defaultAnswer = "NO" } ~= "YES" then
			r = r .. "B"
		end
		if r == "AB" then
			detailPrint("PASSED IfCmd/If Cmd test")
		else
			detailPrint("FAILED IfCmd/If Cmd test")
		end

		-- select..case..case2..caseelse..endselect
		r = ""
		if one == 1 then
			r = r .. "A"
		elseif one == 2 then
			r = r .. "B"
		elseif one == 3 or one == 4 then
			r = r .. "C"
		else
			r = r .. "D"
		end
		if one == 2 then
			r = r .. "A"
		elseif one == 3 then
			r = r .. "B"
		elseif one == 4 or one == 5 then
			r = r .. "C"
		else
			r = r .. "D"
		end
		if one == 3 then
			r = r .. "A"
		elseif one == 4 then
			r = r .. "B"
		elseif one == 5 or one == 1 then
			r = r .. "C"
		else
			r = r .. "D"
		end
		if one == 4 then
			r = r .. "A"
		elseif one == 5 then
			r = r .. "B"
		elseif one == 1 or one == 2 then
			r = r .. "C"
		else
			r = r .. "D"
		end
		if r == "ADCC" then
			detailPrint("PASSED Select..Case*..EndSelect test")
		else
			detailPrint("FAILED Select..Case*..EndSelect test")
		end

		-- switch..case..caseelse..endswitch. Lua has no `switch`; where a
		-- `Case` falls through into the next, the next one's test says so.
		r = ""
		for i = 1, 10 do
			if i == 3 then r = r .. "A" end
			if i == 3 or i == 4 then
				r = r .. "B"
			elseif i == 5 then
				r = r .. "C"
			end

			if i == 1 then r = r .. "D" end
			if i ~= 2 then r = r .. "E" else r = r .. "F" end

			if i == 6 or i == 7 or i == 8 then
				if i ~= 8 then r = r .. "G" end
				if i ~= 7 then
					r = r .. "H"
					if i == 6 then r = r .. "I" else r = r .. "J" end
				end
				r = r .. "K"
			else
				if i ~= 9 then r = r .. "L" end
				r = r .. "M"
			end
		end
		if r == "DELMFLMABELMBELMCELMEGHIKEGKEHJKEMELM" then
			detailPrint("PASSED Switch..Case*..EndSwitch test")
		else
			detailPrint("FAILED Switch..Case*..EndSwitch test")
		end

		-- for[each]..exitfor..next
		r = ""
		for i = 1, 5 do r = r .. i end
		for i = 10, 1, -1 do r = r .. i end
		for i = 1, 0 do r = r .. i end
		if r == "1234510987654321" then
			detailPrint("PASSED For[Each]..Next test")
		else
			detailPrint("FAILED For[Each]..Next test")
		end

		-- do..loop
		if DoLoop(0) == 5 then
			detailPrint("PASSED Do..Loop test")
		else
			detailPrint("FAILED Do..Loop test")
		end

		-- do..exitdo..loop
		local n = 0
		r = ""
		while true do
			r = r .. n
			n = n + 1
			if n > 10 then
				break
			end
		end
		if r == "012345678910" then
			detailPrint("PASSED Do..ExitDo..Loop test")
		else
			detailPrint("FAILED Do..ExitDo..Loop test")
		end

		-- do..exitdo..loopuntil; Lua's `repeat … until` is not supported
		n = 0
		r = ""
		while true do
			r = r .. n
			n = n + 1
			if n >= 5 then
				break
			end
		end
		if r == "01234" then
			detailPrint("PASSED Do..ExitDo..LoopUntil test")
		else
			detailPrint("FAILED Do..ExitDo..LoopUntil test")
		end

		-- dountil..exitdo..loop
		n = 0
		r = ""
		while not (n >= 5) do
			r = r .. n
			n = n + 1
		end
		if r == "01234" then
			detailPrint("PASSED DoUntil..ExitDo..Loop test")
		else
			detailPrint("FAILED DoUntil..ExitDo..Loop test")
		end

		-- nested do test
		n = 0
		local m = 0
		r = ""
		while true do
			r = r .. n .. m
			n = n + 1
			if n > 5 then
				break
			end
			m = 0
			while true do
				r = r .. n .. m
				m = m + 1
				if m >= 5 then
					break
				end
			end
		end
		if r == "00101112131415202122232425303132333435404142434445505152535455" then
			detailPrint("PASSED nested Do test")
		else
			detailPrint("FAILED nested Do test")
		end

		-- while..exitwhile..endwhile
		n = 0
		r = ""
		while n < 5 do
			r = r .. n
			n = n + 1
		end
		if r == "01234" then
			detailPrint("PASSED While..ExitWhile..EndWhile test")
		else
			detailPrint("FAILED While..ExitWhile..EndWhile test")
		end

		-- Unsigned integer tests. A value here is its number, not its bits:
		-- `-1 U< 1` and a signed `0xFFFFFFFF > 1` read one as the other, and
		-- have no spelling. The compiler picks `IntCmpU` for the two it can
		-- prove are not negative.
		local minusOne, allOnes = -1, 0xFFFFFFFF
		r = ""
		if minusOne < one then r = r .. "A" end
		if allOnes > one then r = r .. "D" end
		if r == "AD" then
			detailPrint("PASSED unsigned integer test")
		else
			detailPrint("FAILED unsigned integer test")
		end

		-- 64-bit integer tests
		local big, bigger, biggest = 0x100000000, 0x200000000, 0x500000000
		r = ""
		if big == 4294967296 then r = r .. "A" end
		if big < bigger then r = r .. "B" end
		if biggest >= 0x500000000 then r = r .. "C" end
		if r == "ABC" then
			detailPrint("PASSED 64-bit integer test")
		else
			detailPrint("FAILED 64-bit integer test")
		end

		-- Complex string tests. Without the `S`, LogicLib ignores case.
		local abc, abcd = "ABC", "ABCD"
		r = ""
		if string.lower(string.sub(abc, 1, 1)) == "a" then r = "A" .. r end
		if string.sub(abc, 1, 1) == "a" then r = "A!1" .. r end
		if string.lower(string.sub(abc, 1, 1)) == "x" then r = "A!2" .. r end
		if string.lower(string.sub(abc, 1, 3)) == "abc" then r = "B" .. r end
		if string.lower(string.sub(abc, -3)) == "abc" then r = "C" .. r end
		if string.sub(abc, -2) == "BC" then r = "D" .. r end
		if string.sub(abc, -2) == "bc" then r = "D!1" .. r end
		if string.find(string.lower(abcd), "ab") ~= 0 then r = "E" .. r end
		if string.find(string.lower(abcd), "bc") ~= 0 then r = "F" .. r end
		if string.find(abcd, "bc") ~= 0 then r = "F!1" .. r end
		if string.find(string.lower(abcd), "cd") ~= 0 then r = "G" .. r end
		if r == "GFEDCBA" then
			detailPrint("PASSED complex string test")
		else
			detailPrint("FAILED complex string test")
		end

		-- Extra string tests. The original orders strings with `S<` and `S>`
		-- through System.dll; a string has no ordering here, so only `S==`
		-- and `S!=` are left, which are `==` and `~=`.
		local upperA, lowerA = "A", "a"
		r = ""
		if upperA == "A" then r = r .. "E" end
		if upperA == lowerA then r = r .. "F" end
		if upperA ~= lowerA then r = r .. "G" end
		if r == "EG" then
			detailPrint("PASSED extra string test")
		else
			detailPrint("FAILED extra string test")
		end
	end,
}

installer {
	page.components {
		leave = function()
			if not TESTS.selected then
				messageBox { text = "Please select the component" }
				abort()
			end
		end,
	},
	page.instFiles {},

	TESTS,
}
