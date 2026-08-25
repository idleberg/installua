-- The install-time half of the components tree (Phase 6 batch 26): a section
-- addressed after the compiler has laid it out.
--
-- `components.lua` beside this one is the declaration; this is what a program
-- can do with it once it is running. Every line below reaches a section through
-- the `local` that declared it, and NSIS reads a *number* for every one of them
-- -- the section index and the install type's position, neither of which
-- appears anywhere in this file. Reorder the block's list and the emitted
-- numbers change; this source does not.

attributes {
	name = "Sections",
	outFile = "sections-setup.exe",
}

-- A handle is an ordinary Lua local, and the block below decides the install
-- order. Declaring one here and listing it there is the whole of the binding:
-- there is no name the compiler invents and no define a user has to know.
local core = section { "Core",
	required = true,
	body = function()
		detailPrint("core")
	end,
}

local profiler = section { "Profiler",
	optional = true,
	body = function()
		detailPrint("profiler")
	end,
}

local docs = section("Docs", function()
	detailPrint("docs")
end)

local tools = group { "Tools", sections = { profiler } }

installer {
	installTypes = { "Full", "Minimal" },

	page.components {},
	page.instFiles {},

	core,
	tools,
	docs,

	onInit(function()
		-- The install type the block declared, by name. NSIS wants position 1
		-- and no `1` is written here.
		currentInstType = "Minimal"

		-- A label the components page draws, changed before it draws it. The
		-- name is still "Full": `currentInstType` reads back the *declaration*
		-- rather than the label, so a comparison survives this line.
		instTypes.setText("Full", "Everything")
		detailPrint(instTypes.getText("Full"))

		-- Blanking the text is how a section is hidden: the tree draws no row
		-- for a section with no name, and the section still installs.
		docs.text = ""

		-- Both halves of the flags word, which is one `SectionGetFlags`, one
		-- mask and one `SectionSetFlags` -- the three instructions
		-- `Sections.nsh` spells as a macro that clobbers `$0`.
		profiler.selected = true
		tools.expanded = profiler.selected

		-- `expanded` is a group's bit and `readOnly` is a section's fact, and
		-- both are read as ordinary booleans.
		if core.readOnly then
			detailPrint("core cannot be unticked")
		end

		-- The size an installer cannot know until it runs, in kilobytes, and
		-- the one it already charged.
		docs.size = 4096
		detailPrint("docs charges " .. docs.size .. " KB")

		-- Which install types a section belongs to, by the same names the block
		-- declared: a bit field to NSIS, a list of names here.
		docs.installTypes = { "Full" }
	end),
}
