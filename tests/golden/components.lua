-- The components tree (Phase 6 batch 15): install types, groups, and the four
-- things a section is besides a name and a body.
--
-- The point of the program is the *binding*. `installTypes` is declared once as
-- an ordered list, and every section names its types by that name -- NSIS reads
-- a one-based position and no number appears anywhere in this file. Insert
-- "Custom" at the front of the block's list and every `SectionIn` below
-- renumbers itself.

attributes {
	name = "Components",
	outFile = "components-setup.exe",
}

installer {
	installTypes = { "Full", "Minimal" },

	-- `required` is `SectionIn RO`: no box to untick, so it is in every install
	-- type it names and in the custom one too.
	section("Core", { installTypes = { "Full", "Minimal" }, required = true, size = 120 }, function()
		detailPrint("core")
	end),

	group("Tools", { expanded = true }, {
		section("Profiler", { installTypes = { "Full" } }, function()
			detailPrint("profiler")
		end),
		-- `optional` is `Section /o`, which is not the opposite of `required`:
		-- the box is there, it just starts unticked.
		section("Debugger", { installTypes = { "Full" }, optional = true, size = 4096 }, function()
			detailPrint("debugger")
		end),
	}),

	-- A group with no options is a heading and nothing else.
	group("Docs", {
		-- A section that names no install type belongs to none of them, which
		-- is what writing no `SectionIn` line means to NSIS.
		section("Manual", function()
			detailPrint("manual")
		end),
	}),
}
