-- The plugins that ship declared, and the one fact a declaration carries: how
-- many values come back.
--
-- Every call below is a `Pop` count the compiler could not have discovered.
-- `Dialer.getConnectedState` pops one, `TypeLib.getLibVersion` pops two, and
-- `Banner.show` pops none -- and a wrong number in `src/declarations/*.toml` would
-- not fail here, it would shift every later `Pop` by one and still assemble.
-- That is what makes this golden worth reading line by line.

local Banner = plugin "Banner"
local Dialer = plugin "Dialer"
local NSISdl = plugin "NSISdl"
local Splash = plugin "Splash"
local TypeLib = plugin "TypeLib"
local VPatch = plugin "VPatch"

attributes {
	name = "Plugins",
	outFile = "plugins-setup.exe",
	unicode = true,
}

installer {
	section("Update", function()
		-- One value out of a call that takes none, and the string is compared
		-- as a string: `Dialer` answers "online" or "offline".
		local state = Dialer.getConnectedState()
		if state == "online" then
			-- The URL is a `string` parameter and stays as written; the file
			-- beside it is a `path` and its `/` becomes `\`. Both in one call
			-- is the reason those are separate types.
			local status = NSISdl.download(
				"http://example.com/data.pat",
				PLUGINSDIR .. "/data.pat"
			)
			detailPrint(status)
		end

		-- Three paths in, one string out. The destination is a new file, which
		-- is why `data.dat` appears twice with two different names.
		local patched = VPatch.patchFile(
			PLUGINSDIR .. "/data.pat",
			INSTDIR .. "/data.dat",
			INSTDIR .. "/data.new"
		)
		detailPrint(patched)

		-- Two values, **minor first**: the plugin pushes the major version and
		-- then the minor, so `Pop` order hands the minor one back first. The
		-- names here are the whole test -- swapping them compiles.
		local minor, major = TypeLib.getLibVersion(INSTDIR .. "/data.tlb")
		detailPrint(major .. "." .. minor)
		TypeLib.register(INSTDIR .. "/data.tlb")
	end),

	section("Show", function()
		-- No outputs at all, so no `Pop` follows either line.
		Banner.show("Working")
		Banner.destroy()

		-- The path is the bitmap without its extension, and the result is a
		-- number rather than a string: `1` closed early, `0` timed out.
		local closed = Splash.show(2000, PLUGINSDIR .. "/logo")
		if closed == 1 then
			detailPrint("impatient")
		end
	end),
}
