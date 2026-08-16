-- The thin spine (PLAN Phase 1): the narrowest end-to-end path there is.
--
-- Everything here is one pass wide -- `attributes {}`, one section, and
-- `detailPrint`. Its job is not to be a program anyone would write; it is to
-- make `makensis -WX` a live gate from week one, so that every later phase
-- widens a pipeline that is already verified against the oracle.

attributes {
	name = "Spine",
	outFile = "spine-setup.exe",
	unicode = true,
}

installer {
	section("Core", function()
		detailPrint("the spine assembles")
		-- Constant folding runs before lowering, so this is one `DetailPrint`
		-- and no temporaries (§6).
		detailPrint("built " .. "in " .. 1 .. " pass")
		-- A literal is data, so `$` is doubled on the way out (§15.1) and this
		-- prints a dollar sign rather than expanding anything.
		detailPrint("costs $5")
	end),
}
