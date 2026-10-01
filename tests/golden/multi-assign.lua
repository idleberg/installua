-- `a, b = f()` into names that already exist: the `local a, b = f()` form,
-- popping into the registers the names hold rather than into new ones. A
-- global target is its `Var`.
--
-- `code` is read by the second call and written by it: the arguments are read
-- before the outputs are popped, so the second `exec` sees the first one's code.

local nsExec = plugin "nsExec"

attributes {
	name = "Multi",
	outFile = "multi-assign.exe",
}

installer {
	section("Core", function()
		local code = nsExec.exec("a.exe")
		local output = ""
		code, output = nsExec.execToStack("b.exe " .. code)
		detailPrint(code .. output)
		code, last = nsExec.execToStack("c.exe")
		detailPrint(code)
	end),
}

last = ""
