-- Memento.nsh, lowered through the header. `remember` swaps a section's
-- `Section`/`SectionEnd` for the header's pair and names the registry value
-- its box is kept in; the compiler adds the end marker after the last section,
-- the save to an `.onInstSuccess` of its own, and the restore first in
-- `.onInit` — here, where `memento.restore()` places it instead.

attributes {
	name = "Kept",
	outFile = "memento.exe",
}

memento { root = HKLM, key = "Software/Kept/Components" }

-- Listed by a local, so it keeps the index that earns: the id is separate.
local docs = section { "Documentation",
	remember = "docs",
	body = function()
		setOutPath(INSTDIR)
	end,
}

installer {
	page.components {},
	page.instFiles {},

	section("Core", function()
		setOutPath(INSTDIR)
	end),

	docs,

	-- Inline, so its index is minted; `optional` is the `/o` it starts with
	-- on a first run, before anything was stored.
	section { "Samples",
		remember = "samples",
		optional = true,
		body = function()
			setOutPath(INSTDIR)
		end,
	},

	onInit(function()
		writeReg(HKLM, "Software/Kept/Components", "MementoSection_samples", 1)
		memento.restore()
	end),
}
