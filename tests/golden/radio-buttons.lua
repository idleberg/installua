-- One of a group of sections ticked at a time. The compiler's lines go first
-- in the `onSelChange` written here, and the first section listed is the one
-- ticked at the start, so the others are `optional`.

attributes {
	name = "Radio",
	outFile = "radio-buttons.exe",
}

local small = section("Small", function() end)
local large = section { "Large", optional = true, body = function() end }

installer {
	page.components {},
	page.instFiles {},

	small,
	large,

	radioButtons { small, large },

	onSelChange(function()
		if large.selected then
			detailPrint("large")
		end
	end),
}
