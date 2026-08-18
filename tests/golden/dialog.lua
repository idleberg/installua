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

-- A picture, and the one control whose declaration says what it draws: a
-- `bitmap` with no `image` is an empty rectangle, so the field that gives it one
-- is also an option.
local badge = bitmap { image = "check.bmp", y = 90, height = 20 }

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
			badge,
		},

		pre = function()
			detailPrint("about to build the dialog")
		end,

		-- Fields, which are the controls made addressable (§15.32). Each is one
		-- instruction: a control has no flags word to read, edit and write back
		-- the way a section does.
		show = function()
			serial.font = { face = "Tahoma", size = 8, bold = true }
			serial.colors = { text = "800000", back = "transparent" }
			agree.checked = true

			-- MUI2's own Cancel button, addressed the only way Windows offers.
			-- The kind of window this is cannot be read back, so it answers to
			-- the fields every window has and not to `checked`.
			local cancel = getDlgItem(HWNDPARENT, 2)
			cancel.enabled = false
		end,

		leave = function()
			-- The one read that is not a `SendMessage`: NSIS cannot be handed a
			-- buffer, so the text comes back through `System::Call`.
			if serial.value == "" then
				detailPrint("no serial")
			end
			agree.enabled = false
			badge.visible = false
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
