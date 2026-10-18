-- A progress bar, and the timers that move it.
--
-- nsDialogs knows a timer by its callback's address, so a timer has no `Var`:
-- `CreateTimer` and `KillTimer` each take the address again, and both name the
-- function the compiler named from the `local`.
attributes {
	name = "Timer",
	outFile = "timer-setup.exe",
}

local bar = progressBar { y = 0, height = 12 }

-- Declared above its value, because its own callback stops it: a `local` is
-- not in scope inside its own initializer.
local tick
tick = timer {
	interval = 200,
	onTimer = function()
		if sendMessage(bar, PBM_GETPOS, 0, 0) >= 100 then
			tick.running = false
		else
			sendMessage(bar, PBM_DELTAPOS, 2, 0)
		end
	end,
}

installer {
	page.custom {
		controls = {
			bar,
			-- `true` starts it again, at the declared interval.
			button { "Again", y = 20, height = 14,
				onClick = function()
					sendMessage(bar, PBM_SETPOS, 0, 0)
					tick.running = true
				end,
			},
		},
		-- Started just before `nsDialogs::Show`, in the order listed. One
		-- written inline is bound to nothing, so nothing can stop it.
		timers = {
			tick,
			timer { interval = 1000, onTimer = function()
				detailPrint("tick")
			end },
		},
	},

	page.instFiles {},

	section("Core", function() end),
}
