-- The three headers NSIS ships that are worth declaring, and the one fact a
-- header declaration carries: which end the outputs go on.
--
-- A macro cannot return anything, so `!insertmacro` is handed the registers to
-- write into -- and the macros disagree about where in the argument list those
-- registers belong. `${GetSize}` puts three at the end, `${GetTime}` puts
-- seven, and `${FileJoin}` puts none because its result is a file. Every line
-- below is that count and that position, and getting either wrong emits NSIS
-- that assembles and writes the wrong register.

local fileFunc = import "FileFunc"
local textFunc = import "TextFunc"
local wordFunc = import "WordFunc"

-- Written as constants and joined at each call rather than bound to a local
-- once, and that is not style. A `path` parameter normalises `/` to `\` **at
-- the call**, so `INSTDIR .. APP` arrives as a path wherever it is written,
-- while a local holding the joined string would arrive as the string it is --
-- and every splitter below reads `\` and nothing else.
--
-- Each is a `!define` in the output even though constant folding leaves no
-- reference to it: a `<const>` is a build-time name, and the name survives
-- whether or not anything still spells it.
local APP <const> = "/lib/app.dll"
local CONF <const> = "/app.conf"

attributes {
	name = "Headers",
	outFile = "headers-setup.exe",
	unicode = true,
}

installer {
	onInit(function()
		-- No arguments at all, so the only thing on the line is the
		-- destination.
		local params = fileFunc.getParameters()

		-- `/D=` out of that string, and then the same switch
		-- case-sensitively. Two macros rather than one with a flag, which is
		-- why there are two names here: `${GetOptions}` and `${GetOptionsS}`
		-- are different symbols.
		local target = fileFunc.getOptions(params, "/D=")
		local exact = fileFunc.getOptionsS(params, "/D=")
		if target ~= exact then
			detailPrint("switch case differs")
		end
	end),

	section("Core", function()
		setOutPath(INSTDIR)

		-- The path splitters. They split on `\` and nothing else, which is
		-- why the argument is a `path` -- the `/` written here is `\` by the
		-- time the macro sees it.
		local parent = fileFunc.getParent(INSTDIR .. APP)
		local leaf = fileFunc.getFileName(INSTDIR .. APP)

		-- These two take the *result* of that, which is a name and not a
		-- path: `getBaseName` drops the extension and `getFileExt` keeps it,
		-- and neither has a separator left to find.
		local stem = fileFunc.getBaseName(leaf)
		local ext = fileFunc.getFileExt(leaf)
		detailPrint(parent .. " " .. leaf .. " " .. stem .. "." .. ext)

		-- `C:\` rather than the directory, and the drive that root names is
		-- the one the free-space question is about.
		local root = fileFunc.getRoot(INSTDIR .. APP)
		local freeMib = fileFunc.driveSpace(root, "/D=F /S=M")

		-- Three outputs, at the end, in the order the macro writes them.
		local bytes, files, folders = fileFunc.getSize(INSTDIR, "")
		detailPrint(files .. " files, " .. folders .. " folders, " .. bytes // 1024 .. " KiB")
		if bytes // 1048576 > freeMib then
			abort("not enough room on " .. root)
		end

		-- Seven, and the order is the whole reason this line is in a golden:
		-- day, month, year, weekday, hour, minute, second. Every one of them
		-- is a two-digit string, so a swap is invisible at runtime.
		local day, month, year, weekday, hour, minute, second =
			fileFunc.getTime(INSTDIR .. APP, "M")
		detailPrint(weekday .. " " .. day .. "/" .. month .. "/" .. year)
		detailPrint(hour .. ":" .. minute .. ":" .. second)

		local version = fileFunc.getFileVersion(INSTDIR .. APP)
		local readOnly = fileFunc.getFileAttributes(INSTDIR .. APP, "READONLY")
		if readOnly == "1" then
			detailPrint("read-only " .. version)
		end

		-- Shortened for a progress line, which is what the name means -- the
		-- `35A` is a width with the trimming style stuck on the end, so it is
		-- one string and not a number and a letter.
		detailPrint(fileFunc.bannerTrimPath(INSTDIR .. APP, "35A"))
	end),

	section("Config", function()
		-- `1` has files, `0` is empty, `-1` is missing. Signed, and the three
		-- answers are why: an installer treats "empty" and "gone" differently.
		if fileFunc.dirState(INSTDIR) == -1 then
			createDirectory(INSTDIR)
		end

		-- The entry carries its own `=`: the macro appends nothing.
		local port = textFunc.configRead(INSTDIR .. CONF, "Port=")
		if port == "" then
			-- Four arguments, one output, and the output is a word rather
			-- than a status number: "SAME", "CHANGED", "ADDED", "DELETED".
			local wrote = textFunc.configWrite(INSTDIR .. CONF, "Port=", "8080")
			detailPrint(wrote)
		end

		-- The case-sensitive halves, which are separate symbols in NSIS and
		-- so separate methods here.
		local host = textFunc.configReadS(INSTDIR .. CONF, "Host=")
		textFunc.configWriteS(INSTDIR .. CONF, "Host=", host)

		-- One line by number, counting from the end, and then the count of
		-- them. `lineSum` is the only unsigned result in the file.
		local last = textFunc.lineRead(INSTDIR .. CONF, -1)
		local lines = textFunc.lineSum(INSTDIR .. CONF)
		detailPrint(lines .. " lines ending " .. textFunc.trimNewLines(last))

		-- Both of these write a file and answer with nothing, so no register
		-- follows them on the line. `fileJoin` with an empty third argument
		-- appends in place.
		textFunc.fileJoin(INSTDIR .. CONF, INSTDIR .. "/extra.conf", "")
		textFunc.fileRecode(INSTDIR .. CONF, "CharToOem")

		-- The `PATH` idiom: appended only if it is not already a word of the
		-- string, which is the check that makes this worth a macro.
		local path = readRegStr(HKLM, "System/CurrentControlSet/Control/Session Manager/Environment", "Path")
		local widened = wordFunc.wordAdd(path, ";", INSTDIR)
		writeRegExpandStr(HKLM, "System/CurrentControlSet/Control/Session Manager/Environment", "Path", widened)

		-- The option is this macro's whole language, and it decides the shape
		-- of the answer: `#` counts, `+1` selects. One register either way,
		-- which is why the result is a string rather than a number.
		local count = wordFunc.wordFind(widened, ";", "#")
		local first = wordFunc.wordFindS(widened, ";", "+1")
		detailPrint(count .. " entries, first " .. first)

		-- Two delimiters, then the number -- the argument order a caller
		-- writes from memory and gets wrong.
		local quoted = wordFunc.wordFind2X(last, "[", "]", "+1")
		local between = wordFunc.wordFind3X(last, "[", "-", "]", "+1")
		detailPrint(quoted .. " " .. between)

		-- `"+"` is every occurrence rather than one of them, which is the
		-- option letter that turns this from a find into a rewrite.
		local cleaned = wordFunc.wordReplace(last, "  ", " ", "+")
		local numbered = wordFunc.wordInsert(cleaned, " ", "1.", "+1")
		detailPrint(wordFunc.strFilter(numbered, "1", "", "."))
	end),

	section("Upgrade", function()
		local installed = readRegStr(HKLM, "Software/Example", "Version")

		-- `versionCompare` wants two orderable versions, and `versionConvert`
		-- is what makes a suffixed one orderable.
		local ours = wordFunc.versionConvert("1.4.2-beta", "")
		local theirs = wordFunc.versionConvert(installed, "")
		if wordFunc.versionCompare(ours, theirs) == "1" then
			detailPrint("upgrade from " .. installed)
		end

		-- No arguments, one register, and the pair is the reason both are
		-- declared: `getExeName` is the file and `getExePath` the directory.
		detailPrint(fileFunc.getExePath() .. "\\" .. fileFunc.getExeName())

		-- Nothing in and nothing out: the entire call is one word.
		fileFunc.refreshShellIcons()
	end),
}
