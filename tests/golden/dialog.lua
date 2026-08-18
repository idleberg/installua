-- A custom page, and the controls it draws (§15.32).
--
-- Nothing here is `!include`d. `${NSD_CreateLabel}` is `nsDialogs.nsh`'s way of
-- writing three constants in front of `nsDialogs::CreateControl`, and the
-- compiler writes the constants — so a program with a dialog on it has the same
-- include list as one without, and there is no order to get wrong.
attributes {
	name = "Dialog",
	outFile = "dialog-setup.exe",
}

-- A control is declared like a section and listed like one. The `local` decides
-- nothing about where it sits or when it is drawn; the page's `controls` does.
-- Each of these earns one `Var`, because a handle has to survive from the
-- creator into `leave` — two NSIS functions, with the whole page in between.
local serial = text { "", y = 20, height = 12 }
local agree = checkbox { "I have read the terms", y = 36, height = 12 }
local flavour = dropList { y = 56, height = 60, items = { "Full", "Minimal" } }

-- The uninstaller's own, and a different `local`: a control belongs to one half
-- the way a section does, and the two executables share nothing but the file
-- they are compiled from.
local reason = text { "", y = 20, height = 12 }

installer {
	page.welcome {},

	page.custom { "Registration",
		headerText = "Serial number",
		headerSubText = "Enter the key from your invoice.",

		controls = {
			-- Written where it is used, and bound to nothing: a label the
			-- program never addresses again costs the `Pop` and no `Var`.
			label { "Serial:", y = 0, height = 12 },
			serial,
			agree,
			flavour,
			-- `x` and `width` default to the left edge and the full width,
			-- which are constants. `y` and `height` have no default, because
			-- the only one they could have is *under the last control* — and
			-- that would make every position depend on the order of the list.
			hLine { y = 120, height = 2 },
		},

		pre = function()
			detailPrint("about to build the dialog")
		end,

		leave = function()
			detailPrint("leaving the dialog")
		end,
	},

	page.instFiles {},

	section("Core", function()
		detailPrint("installing")
		writeUninstaller(INSTDIR .. "/un.exe")
	end),
}

uninstaller {
	page.custom {
		headerText = "Before you go",

		controls = {
			label { "Why are you uninstalling?", y = 0, height = 12 },
			reason,
			groupBox { "Optional", y = 40, height = 40 },
		},
	},

	page.instFiles {},

	section("Core", function()
		detailPrint("uninstalling")
	end),
}
