-- x64.nsh's conditions, as calls. Each is LogicLib's `_Name _a _b _t _f`
-- macro used as a predicate: fused into the `if`, with no `${If}`.

attributes {
	name = "Wide",
	outFile = "x64.exe",
}

installer {
	page.instFiles {},

	section("Core", function()
		if runningX64() then setRegView("64") end
		if not wow64() then detailPrint("native") end
		-- As a value: the branch writes the `bool`.
		local arm = nativeMachine("ARM64")
		if arm then detailPrint("arm") end
	end),
}
