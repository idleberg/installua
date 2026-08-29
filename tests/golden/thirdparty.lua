-- The plugins that ship declared, and the one fact each declaration carries:
-- how many values come back.
--
-- Read this the way `plugins.lua` asks to be read -- every call below is a
-- `Pop` count nothing could have discovered, and a wrong number in
-- `src/declarations/*.toml` would not fail here. It would shift every later `Pop` by
-- one and still assemble.
--
-- The last two sections are the second thing a declaration can carry: a count
-- that is not a *number*. A plugin whose first pushed value decides whether
-- more follow is declared with `tagged`, and the two below tag opposite
-- outcomes on purpose.
--
-- Tier 2 only. Most of these DLLs are in `NSISDIR/Plugins` on no machine, so
-- `makensis` would stop at *Plugin not found* long before it judged anything
-- this file is testing. `tests/goldens.rs` keeps it out of the tier-3 list on
-- purpose, and says so there.

local accessControl = plugin "AccessControl"
local enVar = plugin "EnVar"
local nsis7z = plugin "Nsis7z"
local nsisFirewall = plugin "nsisFirewall"
local simpleSC = plugin "SimpleSC"
local startMenu = plugin "StartMenu"

attributes {
	name = "Third Party",
	outFile = "thirdparty-setup.exe",
	unicode = true,
}

installer {
	section("Environment", function()
		-- Stateful and pushes nothing, so no `Pop` follows it: every EnVar call
		-- after this line reads and writes the machine environment.
		enVar.setHKLM()

		-- One code out of two strings. `"NULL"` in the value position is the
		-- plugin's own sentinel for "does this variable exist at all", which is
		-- why the second parameter cannot just be a path.
		local exists = enVar.check("PATH", "NULL")
		if exists == 0 then
			-- The value is a `path`, so the `/` written here becomes a `\`.
			-- The name beside it is a `string` and keeps whatever it was given.
			local added = enVar.addValue("PATH", INSTDIR .. "/bin")
			if added ~= 0 then
				detailPrint("PATH not updated")
			end
		end

		-- Two strings in, one code out -- and the root is a plain string
		-- because `""` is meaningful here rather than missing: it means both
		-- HKCU and HKLM, appended.
		local refreshed = enVar.update("HKLM", "PATH")
		if refreshed ~= 0 then
			detailPrint("PATH not reloaded")
		end
	end),

	section("Service", function()
		-- One value, and `0` is the answer that means yes.
		local present = simpleSC.existsService("Example")
		if present == 0 then
			-- **Two values, code first.** The service's state is the *second*
			-- one; the first only says whether the question could be asked.
			-- Swapping these two names compiles, which is why the count and
			-- the order are both written down.
			local queried, running = simpleSC.serviceIsRunning("Example")
			if queried == 0 and running == 1 then
				simpleSC.stopService("Example", 1, 30)
			end
		end
	end),

	section("Archive", function()
		-- No outputs at all. Nsis7z pushes nothing -- not a status, not an
		-- error -- so no `Pop` follows either line, and a caller who wants to
		-- know whether it worked has to look at the filesystem.
		setOutPath(INSTDIR)
		nsis7z.extract(PLUGINSDIR .. "/payload.7z")
		nsis7z.extractWithDetails(PLUGINSDIR .. "/extras.7z", "Extracting %s...")
	end),

	section("Firewall", function()
		-- Path first, then the name the rule shows under. Removing goes by the
		-- path, so the name is a label rather than a key.
		local allowed = nsisFirewall.addAuthorizedApplication(
			INSTDIR .. "/bin/app.exe",
			"Example"
		)
		if allowed ~= 0 then
			detailPrint("firewall rule not added")
		end
	end),

	section("Permissions", function()
		-- The shape 22 of AccessControl's 25 methods have: `"ok"` alone, or
		-- `"error"` and a message underneath it. Dropping the message would be
		-- a stack leak rather than a lost diagnostic, which is why the second
		-- name is not optional in the emitted `Pop` even when the caller has no
		-- use for it.
		local granted, why = accessControl.grantOnFile(INSTDIR, "(BU)", "FullAccess")
		if granted == "error" then
			detailPrint("ACL not set: " .. why)
		end

		-- The uniform one: `getCurrentUserName` pushes exactly one value on
		-- every path, and `"error"` is that value rather than a tag on top of
		-- one. So there is nothing to test and nothing to default.
		local user = accessControl.getCurrentUserName()

		-- **A tagged arity.** `nameToSid` pushes the SID alone, or a sentence
		-- and `"error"` on top of it -- so the count is one *or* two and the
		-- first value is which. `tagged` is what says so, and the second name
		-- here reads `""` on the path where nothing was pushed.
		--
		-- Binding only `sid` would still be correct: the message comes off the
		-- stack either way, because the plugin put it there and leaving it
		-- would shift every later `Pop` by one.
		local sid, unknown = accessControl.nameToSid(user)
		if sid == "error" then
			detailPrint(unknown)
		else
			detailPrint(user .. " is " .. sid)
		end
	end),

	-- **Flags, the third thing a declaration carries.** Every one below is
	-- written *last*, in a table, and emitted *first*, ahead of the fixed
	-- arguments -- which is the whole reason position is the declaration's
	-- rather than the call site's. StartMenu's own readme puts it plainly:
	-- "the order of the switches doesn't matter but the required parameter
	-- must come after all of them".
	section("Flags", function()
		-- `/sid` alone, and it is a `bool` at the call site because the flag
		-- *is* the value. `false` writes nothing rather than writing an off
		-- switch, since NSIS has no spelling for one.
		local owner = accessControl.getFileOwner(INSTDIR, { sid = true })
		local plain = accessControl.getFileOwner(INSTDIR, { sid = false })
		detailPrint(owner .. plain)

		-- Three flags named in an order nobody chose, emitted in the order
		-- the declaration lists them: `/autoadd`, then `/text`, then
		-- `/lastused`. Two calls naming the same flags differently have to
		-- emit the same line, or this file would be recording which way it
		-- was typed.
		local outcome, folder = startMenu.select("Example", {
			lastused = INSTDIR,
			autoadd = true,
			text = "Pick one",
		})
		if outcome == "success" then
			detailPrint(folder)
		end
	end),

	section("Start Menu", function()
		-- **The opposite polarity, and the reason `tagged` is a list of
		-- literals rather than the word "error".** StartMenu pushes its extra
		-- value on *success*: `"success"` and the folder, or `"cancel"` or an
		-- error message alone. A design that hardcoded the failure spelling
		-- would pop one value too few on every run that worked, and NSIS would
		-- not say a word about it.
		local outcome, folder = startMenu.select("Example")
		if outcome == "success" then
			setOutPath(INSTDIR)
			createShortcut(SMPROGRAMS .. "/" .. folder .. "/Example.lnk", INSTDIR .. "/app.exe")
		end
	end),
}
