-- `SetCompressor` has to precede anything that writes installer data, and
-- `${Using:StrFunc} StrCase` writes a `Function`. So the init line goes below
-- the attributes, and `makensis` accepting this is the whole assertion.

attributes {
	name = "Compressor",
	outFile = "compressor-setup.exe",
	compressor = { "lzma", solid = true },
}

installer {
	page.instFiles {},
	section("Core", function()
		detailPrint(string.lower(INSTDIR))
	end),
}
