-- $NSISDIR/Examples/unicode.nsi: text outside Latin-1 in the log and in a
-- UTF-16LE file, written and read back.

attributes {
	name = "Unicode Games",
	outFile = "unicode.exe",
	requestExecutionLevel = "user",
	showInstDetails = "show",
}

installer {
	page.instFiles {},

	section("Unicode in UI", function()
		detailPrint("Hello World!")
		detailPrint("שלום עולם!")
		detailPrint("مرحبا العالم!")
		detailPrint("こんにちは、世界！")
		detailPrint("你好世界！")
		detailPrint("привет мир!")
		detailPrint("안녕하세요!")

		detailPrint("\u{A9}") -- arbitrary unicode chars
	end),

	section("Unicode in Files", function()
		local f = fileOpen(PLUGINSDIR .. "/Test.txt", "w")
		if errors() then
			return
		end
		f:writeUtf16Le("Hello World ", { bom = true })
		f:writeWord(0xD83C) -- Manually write U+1F30D
		f:writeWord(0xDF0D) -- as surrogate-pair
		f:writeUtf16Le(" and Sun \u{2600}\r\n")
		f:close()

		f = fileOpen(PLUGINSDIR .. "/Test.txt", "r")
		if errors() then
			return
		end
		local message = f:readUtf16Le()
		f:close()

		detailPrint("Message: " .. message)
	end),
}
