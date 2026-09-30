-- The build-time half: everything the compiler decides before a line is
-- emitted, and the two anchors that let a script talk to `makensis` anyway.
--
-- `param` with each defaulted type, a top-level `if` taken on one of them, a
-- `glob` unrolled against the directory this file lives in, an `ipairs` over a
-- `<const>` table unrolled the same way, and `raw.head` /
-- `raw.tail` landing either side of the script, and the three `MAKENSIS.*`
-- side effects written where they stand. What the `.nsi` shows is mostly which
-- branch won; the rest is the two `MAKENSIS` answers copied into registers.
--
-- `raw.head` lands above the `!define`s a `param` becomes, so it is the one
-- place in a script where `${NAME}` is not defined yet.

local NAME    <const> = param("NAME", "Build Time")
local BUILD   <const> = param("BUILD", 41)
local SIGNED  <const> = param("SIGNED", false)

-- No `!define`: a table has no text, only fields, and each folds where read.
local DOCS <const> = {
	{ file = "readme.txt", title = "Read me" },
	{ file = "license.txt", title = "License" },
}

raw.head [[ !echo "building" ]]
raw.tail [[ !finalize 'echo done' ]]

attributes {
	name = NAME,
	outFile = "build-time-setup.exe",
}

installer {
	section("Core", function()
		setOutPath(INSTDIR)
		for path in glob("assets/*.bmp") do
			file(path)
		end
		for i, doc in ipairs(DOCS) do
			detailPrint(i .. ". " .. doc.title .. " (" .. doc.file .. ")")
		end
		detailPrint("build " .. BUILD + 1)
		MAKENSIS.echo("building " .. NAME)
		local status = MAKENSIS.system([[test -n "$HOME"]])
		local major, minor = MAKENSIS.getDllVersion("shared.dll")
		detailPrint("status " .. status .. ", shared " .. major .. "." .. minor)
		stamp()
		if SIGNED then
			detailPrint("this line never reaches the .nsi")
		end
	end),
}

if BUILD > 40 then
	func("stamp", function()
		detailPrint("late build")
	end)
else
	func("stamp", function()
		detailPrint("early build")
	end)
end
