-- $NSISDIR/Examples/primes.nsi: writes the first primes to a file, asking
-- after every hundred whether to go on. The original's two loops are labels
-- and `IntCmp`; here they are `while`s.

attributes {
	name = "primes",
	outFile = "primes.exe",
	caption = "Prime number generator",
	showInstDetails = "show",
	allowRootDirInstall = true,
	installDir = EXEDIR,
	requestExecutionLevel = "user",
}

func("doPrimes", function()
	local f = fileOpen(INSTDIR .. "/primes.txt", "w")
	detailPrint("2 is prime!")
	f:write("2 is prime!\r\n")
	detailPrint("3 is prime!")
	f:write("3 is prime!\r\n")
	local pos = 3
	local count = 2
	local more = true
	while more do
		local div = 3
		while pos % div ~= 0 do
			div = div + 2
			if div >= pos then
				detailPrint(pos .. " is prime!")
				f:write(pos .. " is prime!\r\n")
				count = count + 1
				if count == 100 then
					count = 0
					more = messageBox { text = "Process more?", buttons = "YESNO" } == "YES"
				end
				break
			end
		end
		pos = pos + 2
	end
	f:close()
end)

installer {
	page.directory { topText = "Select a directory to write primes.txt." },
	page.instFiles {},

	section("", function()
		setOutPath(INSTDIR)
		doPrimes()
	end),
}
