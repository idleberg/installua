-- $NSISDIR/Examples/one-section.nsi: two groups of sections, each allowing
-- only one of its options to be ticked.

attributes {
	name = "One Section",
	outFile = "one-section.exe",
	requestExecutionLevel = "user",
}

local g1o1 = section("Group 1 - Option 1", function() end)
local g1o2 = section { "Group 1 - Option 2", optional = true, body = function() end }
local g1o3 = section { "Group 1 - Option 3", optional = true, body = function() end }

local g2o1 = section("Group 2 - Option 1", function() end)
local g2o2 = section { "Group 2 - Option 2", optional = true, body = function() end }
local g2o3 = section { "Group 2 - Option 3", optional = true, body = function() end }

installer {
	page.components {},
	page.instFiles {},

	section { "!Required", required = true, body = function() end },
	g1o1, g1o2, g1o3,
	g2o1, g2o2, g2o3,

	radioButtons { g1o1, g1o2, g1o3 },
	radioButtons { g2o1, g2o2, g2o3 },
}
