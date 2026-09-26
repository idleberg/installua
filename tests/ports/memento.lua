-- $NSISDIR/Examples/Memento.nsi: the components page remembers what was
-- ticked, and can be loaded with an example state first.

attributes {
	name = "Memento",
	outFile = "Memento.exe",
	showInstDetails = "show",
	requestExecutionLevel = "user",
}

memento { root = HKCU, key = "Software/NSIS/Memento Test" }

local sec_horse = section { "horse", remember = "sec_horse", body = function() end }
local sec_donkey = section { "donkey", remember = "sec_donkey", body = function() end }
local sec_chicken = section { "chicken", remember = "sec_chicken", body = function() end }
local sec_croc = section { "croc", remember = "sec_croc", body = function() end }
local sec_cow = section { "cow", remember = "sec_cow", body = function() end }
local sec_dinosaur = section { "dinosaur", remember = "sec_dinosaur", optional = true, body = function() end }

installer {
	page.components {},
	page.instFiles {},

	sec_horse,
	sec_donkey,
	sec_chicken,

	group { "group", expanded = true, sections = {
		group { "group", expanded = true, sections = {
			sec_croc,
			sec_cow,
		} },
	} },

	sec_dinosaur,

	onInit(function()
		if messageBox { text = "Would you like to load an example state?", buttons = "YESNO" } == "YES" then
			deleteRegKey(HKCU, "Software/NSIS/Memento Test")
			writeReg(HKCU, "Software/NSIS/Memento Test", "MementoSectionUsed", "")
			writeReg(HKCU, "Software/NSIS/Memento Test", "MementoSection_sec_horse", 1)
			writeReg(HKCU, "Software/NSIS/Memento Test", "MementoSection_sec_chicken", 1)
			writeReg(HKCU, "Software/NSIS/Memento Test", "MementoSection_sec_donkey", 0)
			writeReg(HKCU, "Software/NSIS/Memento Test", "MementoSection_sec_croc", 0)
		end
		memento.restore()
	end),
}
