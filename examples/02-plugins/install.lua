-- Program 2 of five: plugin-heavy.
--
-- The point of this one is the stack ABI and the opaque-clobber rule. Three
-- callees clobber everything — `plugin`, `System::Call` and `raw` — and this
-- program uses all three, with live values across each of them.
--
-- Note what is *not* here: `Push`, `Pop` and `Exch` have no Installua spelling.
-- A plugin's output count comes from its declaration in `.installua/declarations/`,
-- which is what makes `local rc, out = …` legal at all.

local APP <const> = "Example2"

local nsExec = plugin "nsExec"
local userInfo = plugin "UserInfo"
local system = plugin "System"

attributes {
	name = APP,
	outFile = APP .. "-setup.exe",
	unicode = true,
	requestExecutionLevel = "admin",
}

-- A global, because no local survives a `raw` block, so a
-- value that has to cross one goes through the honest channel.
gitDescribe = ""

installer {
	installDir = PROGRAMFILES64 .. "/" .. APP,

	page.directory {},
	page.instFiles {},

	onInit(function()
		local account = userInfo.getAccountType()
		if account ~= "Admin" then
			messageBox {
				text = "Administrator rights are required.",
				buttons = "OK",
				icon = "STOP",
			}
			os.exit()
		end
	end),

	section("Core", function()
		setOutPath(INSTDIR)
		file("assets/tool.exe")

		-- Two locals live across every call below, so the caller saves them.
		local target = INSTDIR .. "/tool.exe"
		local label = APP .. " smoke test"

		local rc, out = nsExec.execToStack('"' .. target .. '" --version')
		if rc ~= "0" then
			detailPrint(label .. " failed: " .. out)
			abort("the bundled tool does not run on this machine")
		end
		detailPrint(label .. " reported " .. out)

		-- `System::Call` is the second opaque callee, and it is opaque because
		-- `.r0` inside that string writes a register nothing in the AST records.
		-- `.s` pushes instead, which the declaration can describe.
		-- `out` is still live here, so this is the one call site in the five
		-- programs where a caller-save actually fires.
		local ticks = system.call("kernel32::GetTickCount() i .s")
		detailPrint("uptime tick " .. ticks .. " for " .. out .. " at " .. target)

		-- The third. Nothing is hoisted, nothing survives, and it is visibly
		-- unchecked at the position it appears.
		raw [[
			nsExec::ExecToStack '"$INSTDIR\tool.exe" --describe'
			Pop $0
			Pop $gitDescribe
		]]

		detailPrint("built from " .. gitDescribe .. ", " .. label)
	end),
}
