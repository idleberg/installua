-- A program whose only page is `page.custom`, which is `Page custom` and not a
-- MUI2 macro. `MUI_LANGUAGE` `!warning`s unless a MUI2 page macro ran above it,
-- so the compiler writes the define MUI2 offers for this case. Nothing else
-- changes: `MUI_LANGUAGE` still runs `MUI_INSERT`, which sets up the header a
-- custom page draws.
--
-- No `instFiles` page means no section ever runs, which is makensis's warning
-- 8000. That one is the program's to silence, and this one does.

attributes { outFile = "custom-pages.exe" }

raw.head [[!pragma warning disable 8000]]

installer {
	page.custom { "One", controls = {} },
	section("-x", function() end),
}
