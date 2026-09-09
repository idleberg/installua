-- The build-time half: everything the compiler decides before a line is
-- emitted, and the two anchors that let a script talk to `makensis` anyway.
--
-- `param` with each defaulted type, a top-level `if` taken on one of them, a
-- `glob` unrolled against the directory this file lives in, and `raw.head` /
-- `raw.tail` landing either side of the script. Nothing here has a runtime
-- reading, so what the `.nsi` shows is which branch won.
--
-- `raw.head` lands above the `!define`s a `param` becomes, so it is the one
-- place in a script where `${NAME}` is not defined yet.

local NAME    <const> = param("NAME", "Build Time")
local BUILD   <const> = param("BUILD", 41)
local SIGNED  <const> = param("SIGNED", false)

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
		detailPrint("build " .. BUILD + 1)
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
