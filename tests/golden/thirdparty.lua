-- The five third-party plugins that ship declared, and the one fact each
-- declaration carries: how many values come back.
--
-- Read this the way `plugins.lua` asks to be read -- every call below is a
-- `Pop` count nothing could have discovered, and a wrong number in
-- `src/headers/*.toml` would not fail here. It would shift every later `Pop` by
-- one and still assemble.
--
-- Tier 2 only. None of these DLLs is in `NSISDIR/Plugins` on any machine, so
-- `makensis` would stop at *Plugin not found* long before it judged anything
-- this file is testing. `tests/goldens.rs` keeps it out of the tier-3 list on
-- purpose, and says so there.

local accessControl = plugin "AccessControl"
local enVar = plugin "EnVar"
local nsis7z = plugin "Nsis7z"
local nsisFirewall = plugin "nsisFirewall"
local simpleSC = plugin "SimpleSC"

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

		-- The two AccessControl methods whose count does not depend on the
		-- outcome -- one value on every path through each. Everything else the
		-- plugin exports pushes one on success and two on a diagnosed failure,
		-- which is an outcome rather than a signature.
		--
		-- `nameToSid` is also where the absent sentinel shows: a failed lookup
		-- comes back as a sentence, not as `"error"`, so there is nothing to
		-- compare against and the prefix is what a caller tests.
		local user = accessControl.getCurrentUserName()
		local sid = accessControl.nameToSid(user)
		detailPrint(user .. " is " .. sid)
	end),
}
