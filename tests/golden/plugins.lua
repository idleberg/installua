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
local nsExec = plugin "nsExec"
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
			-- The flags are named *last*, in a table, and emitted *first*: the
			-- position is the declaration's, not the call site's. `noieproxy`
			-- is written before `timeout` here and comes out after it, because
			-- `NSISdl.toml` lists them in the order `nsisdl.cpp` reads them.
			-- `/TIMEOUT` glues its value on with `=`; a bare flag *is* its
			-- value, so `true` writes the token and `false` writes nothing.
			local status = NSISdl.download(
				"http://example.com/data.pat",
				PLUGINSDIR .. "/data.pat",
				{ noieproxy = true, timeout = 30000 }
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

		-- Two values out, and two flags in. `nsexec.c` loops back over its
		-- flag checks, so any order the call site writes would work here --
		-- the emission order is still the declaration's, so that two calls
		-- naming the same flags cannot emit two different lines.
		local code, output = nsExec.execToStack("cmd.exe /c ver", {
			oem = true,
			timeout = 5000,
		})
		detailPrint(code .. output)
	end),
}
