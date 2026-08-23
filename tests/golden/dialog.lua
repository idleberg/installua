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

-- An event is an option and not a field, because the address of a function is a
-- build-time fact: there is no install-time moment at which one could be
-- assigned that is not already inside a callback. nsDialogs pushes the control's
-- handle before calling, and the `Pop` that clears it is the compiler's.
local proceed = button { "Check", x = 0, y = 140, width = 60, height = 14,
	onClick = function()
		detailPrint("checking")
	end,
}

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
			proceed,
			-- A `url` is an `onClick` the compiler writes: `ExecShell "open"`,
			-- which is what a shortcut to an address does, so the browser is
			-- the user's rather than one this installer picks.
			link { "Terms and conditions", url = "https://example.invalid/terms", y = 160, height = 12 },
		},

		pre = function()
			detailPrint("about to build the dialog")
		end,

		-- Fields, which are the controls made addressable (§15.32). Each is one
		-- instruction: a control has no flags word to read, edit and write back
		-- the way a section does.
		show = function()
			serial.font = { face = "Tahoma", size = 8, bold = true }
			serial.colors = { text = "800000", background = "transparent" }
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
