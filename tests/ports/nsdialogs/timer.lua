-- $NSISDIR/Examples/nsDialogs/timer.nsi: three timers, each moving a progress
-- bar, and two buttons that stop the first two.

-- No `page.instFiles`, so no section ever runs.
raw.head [[!pragma warning disable 8000]]

attributes {
	name = "nsDialogs Timer Example",
	outFile = "nsDialogs Timer Example.exe",
	requestExecutionLevel = "user",
}

local bar = progressBar { x = 0, y = 10, height = 12 }
local bar2 = progressBar { x = 0, y = 52, height = 12 }
local bar3 = progressBar { x = 0, y = 114, height = 12 }

local timer1 = timer {
	interval = 1000,
	onTimer = function()
		if sendMessage(bar, PBM_GETPOS, 0, 0) == 100 then
			sendMessage(bar, PBM_SETPOS, 0, 0)
		else
			sendMessage(bar, PBM_DELTAPOS, 10, 0)
		end
	end,
}

local timer2 = timer {
	interval = 100,
	onTimer = function()
		if sendMessage(bar2, PBM_GETPOS, 0, 0) == 100 then
			sendMessage(bar2, PBM_SETPOS, 0, 0)
		else
			sendMessage(bar2, PBM_DELTAPOS, 5, 0)
		end
	end,
}

-- Declared above its value, because its own callback stops it.
local timer3
timer3 = timer {
	interval = 200,
	onTimer = function()
		if sendMessage(bar3, PBM_GETPOS, 0, 0) >= 100 then
			timer3.running = false
			messageBox { text = "Timer 3 killed" }
		else
			sendMessage(bar3, PBM_DELTAPOS, 2, 0)
		end
	end,
}

installer {
	page.custom {
		controls = {
			label { "nsDialogs timer example", x = 0, y = 0, height = 9 },
			bar,
			button {
				"Kill Timer 1",
				x = 0,
				y = 25,
				width = 100,
				height = 14,
				onClick = function()
					timer1.running = false
				end,
			},
			bar2,
			button {
				"Kill Timer 2",
				x = 0,
				y = 67,
				width = 100,
				height = 14,
				onClick = function()
					timer2.running = false
				end,
			},
			bar3,
		},
		timers = { timer1, timer2, timer3 },
		show = function()
			sendMessage(bar2, PBM_SETBARCOLOR, 0, 0x0000FF) -- No visual styles
			sendMessage(bar2, PBM_SETSTATE, PBST_ERROR, 0) -- Vista+
		end,
	},

	section("", function() end),
}
