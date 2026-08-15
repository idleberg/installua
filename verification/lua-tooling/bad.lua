-- Each line here is a §1 claim under test. The expected diagnostic is named in
-- the comment; `expected.txt` is the machine-checked version.

attributes {
	name = "Example",
	outFile = "example.exe",
	instalDir = "typo", -- CLAIM A: unknown field in a block is warned about
}

installer {
	pages = { "Welcom" }, -- CLAIM B: an `---@alias` member is checked inside a string literal
}

detailprint("wrong casing") -- CLAIM C: NSIS muscle memory is an undefined-global
section("Main") -- CLAIM D: arity is checked (body is missing)

local m = require("mymodule") -- CLAIM E: `runtime.builtin` turns `require` into an unknown global
local co = coroutine.create(function() end) -- CLAIM E: ditto for `coroutine`
local f = io.open("x") -- CLAIM E: ditto for `io`
local t = os.time() -- CLAIM E: ditto for `os`
local d = debug.traceback() -- CLAIM E: ditto for `debug`

-- CLAIM F (negative): LuaLS keeps these; flagging them is selene's half, not LuaLS's.
local g = string.gsub("a", "%a", "b")
local p = pcall(function() end)
local n = math.floor(1.5)

return m, co, f, t, d, g, p, n
