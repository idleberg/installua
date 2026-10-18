-- $NSISDIR/Examples/Library.nsi: every type of library, installed and
-- uninstalled in every mode. It tests what Library.nsh writes, more than what
-- the installer does.

attributes {
	name = "Library Test",
	outFile = "Library Test.exe",
	installDir = TEMP .. "/Library Test",
	requestExecutionLevel = "user",
}

installer {
	page.directory {},
	page.instFiles {},

	section("", function()
		-- Empty, which is a first install of a shared library.
		local previous = ""

		installLib(NSISDIR .. "/Plugins/x86-unicode/LangDLL.dll", INSTDIR .. "/test.dll", { reboot = true, protected = true })
		installLib(NSISDIR .. "/Plugins/x86-unicode/LangDLL.dll", INSTDIR .. "/test.dll", { protected = true })
		installLib(NSISDIR .. "/Plugins/x86-unicode/LangDLL.dll", INSTDIR .. "/test.dll", { reboot = true })
		installLib(NSISDIR .. "/Plugins/x86-unicode/LangDLL.dll", INSTDIR .. "/test.dll")

		installLib(NSISDIR .. "/Plugins/x86-unicode/LangDLL.dll", INSTDIR .. "/test.dll", { type = "REGDLL", reboot = true, protected = true })
		installLib(NSISDIR .. "/Plugins/x86-unicode/LangDLL.dll", INSTDIR .. "/test.dll", { type = "REGDLL", protected = true })
		installLib(NSISDIR .. "/Plugins/x86-unicode/LangDLL.dll", INSTDIR .. "/test.dll", { type = "REGDLL", reboot = true })
		installLib(NSISDIR .. "/Plugins/x86-unicode/LangDLL.dll", INSTDIR .. "/test.dll", { type = "REGDLL" })

		installLib(NSISDIR .. "/Plugins/x86-unicode/LangDLL.dll", INSTDIR .. "/test.dll", { type = "TLB", reboot = true, protected = true })
		installLib(NSISDIR .. "/Plugins/x86-unicode/LangDLL.dll", INSTDIR .. "/test.dll", { type = "TLB", protected = true })
		installLib(NSISDIR .. "/Plugins/x86-unicode/LangDLL.dll", INSTDIR .. "/test.dll", { type = "TLB", reboot = true })
		installLib(NSISDIR .. "/Plugins/x86-unicode/LangDLL.dll", INSTDIR .. "/test.dll", { type = "TLB" })

		installLib(NSISDIR .. "/Plugins/x86-unicode/LangDLL.dll", INSTDIR .. "/test.dll", { type = "REGDLLTLB", reboot = true, protected = true })
		installLib(NSISDIR .. "/Plugins/x86-unicode/LangDLL.dll", INSTDIR .. "/test.dll", { type = "REGDLLTLB", protected = true })
		installLib(NSISDIR .. "/Plugins/x86-unicode/LangDLL.dll", INSTDIR .. "/test.dll", { type = "REGDLLTLB", reboot = true })
		installLib(NSISDIR .. "/Plugins/x86-unicode/LangDLL.dll", INSTDIR .. "/test.dll", { type = "REGDLLTLB" })

		installLib(NSISDIR .. "/Plugins/x86-unicode/LangDLL.dll", INSTDIR .. "/test.dll", { shared = previous, reboot = true, protected = true })
		installLib(NSISDIR .. "/Plugins/x86-unicode/LangDLL.dll", INSTDIR .. "/test.dll", { shared = previous, protected = true })
		installLib(NSISDIR .. "/Plugins/x86-unicode/LangDLL.dll", INSTDIR .. "/test.dll", { shared = previous, reboot = true })
		installLib(NSISDIR .. "/Plugins/x86-unicode/LangDLL.dll", INSTDIR .. "/test.dll", { shared = previous })

		installLib(NSISDIR .. "/Plugins/x86-unicode/LangDLL.dll", INSTDIR .. "/test.dll", { type = "REGDLL", shared = previous, reboot = true, protected = true })
		installLib(NSISDIR .. "/Plugins/x86-unicode/LangDLL.dll", INSTDIR .. "/test.dll", { type = "REGDLL", shared = previous, protected = true })
		installLib(NSISDIR .. "/Plugins/x86-unicode/LangDLL.dll", INSTDIR .. "/test.dll", { type = "REGDLL", shared = previous, reboot = true })
		installLib(NSISDIR .. "/Plugins/x86-unicode/LangDLL.dll", INSTDIR .. "/test.dll", { type = "REGDLL", shared = previous })

		installLib(NSISDIR .. "/Plugins/x86-unicode/LangDLL.dll", INSTDIR .. "/test.dll", { type = "TLB", shared = previous, reboot = true, protected = true })
		installLib(NSISDIR .. "/Plugins/x86-unicode/LangDLL.dll", INSTDIR .. "/test.dll", { type = "TLB", shared = previous, protected = true })
		installLib(NSISDIR .. "/Plugins/x86-unicode/LangDLL.dll", INSTDIR .. "/test.dll", { type = "TLB", shared = previous, reboot = true })
		installLib(NSISDIR .. "/Plugins/x86-unicode/LangDLL.dll", INSTDIR .. "/test.dll", { type = "TLB", shared = previous })

		installLib(NSISDIR .. "/Plugins/x86-unicode/LangDLL.dll", INSTDIR .. "/test.dll", { type = "REGDLLTLB", shared = previous, reboot = true, protected = true })
		installLib(NSISDIR .. "/Plugins/x86-unicode/LangDLL.dll", INSTDIR .. "/test.dll", { type = "REGDLLTLB", shared = previous, protected = true })
		installLib(NSISDIR .. "/Plugins/x86-unicode/LangDLL.dll", INSTDIR .. "/test.dll", { type = "REGDLLTLB", shared = previous, reboot = true })
		installLib(NSISDIR .. "/Plugins/x86-unicode/LangDLL.dll", INSTDIR .. "/test.dll", { type = "REGDLLTLB", shared = previous })

		installLib(NSISDIR .. "/Contrib/UIs/default.exe", INSTDIR .. "/test.exe", { type = "REGEXE", shared = previous, reboot = true, protected = true })
		installLib(NSISDIR .. "/Contrib/UIs/default.exe", INSTDIR .. "/test.exe", { type = "REGEXE", shared = previous, protected = true })
		installLib(NSISDIR .. "/Contrib/UIs/default.exe", INSTDIR .. "/test.exe", { type = "REGEXE", shared = previous, reboot = true })
		installLib(NSISDIR .. "/Contrib/UIs/default.exe", INSTDIR .. "/test.exe", { type = "REGEXE", shared = previous })

		writeUninstaller(INSTDIR .. "/uninstall.exe")
	end),
}

uninstaller {
	section("uninstall", function()
		uninstallLib(INSTDIR .. "/test.dll")
		uninstallLib(INSTDIR .. "/test.dll", { remove = true, reboot = true, protected = true })
		uninstallLib(INSTDIR .. "/test.dll", { remove = true, protected = true })
		uninstallLib(INSTDIR .. "/test.dll", { remove = true, reboot = true })
		uninstallLib(INSTDIR .. "/test.dll", { remove = true })

		uninstallLib(INSTDIR .. "/test.dll", { type = "REGDLL" })
		uninstallLib(INSTDIR .. "/test.dll", { type = "REGDLL", remove = true, reboot = true, protected = true })
		uninstallLib(INSTDIR .. "/test.dll", { type = "REGDLL", remove = true, protected = true })
		uninstallLib(INSTDIR .. "/test.dll", { type = "REGDLL", remove = true, reboot = true })
		uninstallLib(INSTDIR .. "/test.dll", { type = "REGDLL", remove = true })

		uninstallLib(INSTDIR .. "/test.dll", { type = "TLB" })
		uninstallLib(INSTDIR .. "/test.dll", { type = "TLB", remove = true, reboot = true, protected = true })
		uninstallLib(INSTDIR .. "/test.dll", { type = "TLB", remove = true, protected = true })
		uninstallLib(INSTDIR .. "/test.dll", { type = "TLB", remove = true, reboot = true })
		uninstallLib(INSTDIR .. "/test.dll", { type = "TLB", remove = true })

		uninstallLib(INSTDIR .. "/test.dll", { type = "REGDLLTLB" })
		uninstallLib(INSTDIR .. "/test.dll", { type = "REGDLLTLB", remove = true, reboot = true, protected = true })
		uninstallLib(INSTDIR .. "/test.dll", { type = "REGDLLTLB", remove = true, protected = true })
		uninstallLib(INSTDIR .. "/test.dll", { type = "REGDLLTLB", remove = true, reboot = true })
		uninstallLib(INSTDIR .. "/test.dll", { type = "REGDLLTLB", remove = true })

		uninstallLib(INSTDIR .. "/test.dll", { shared = true })
		uninstallLib(INSTDIR .. "/test.dll", { shared = true, remove = true, reboot = true, protected = true })
		uninstallLib(INSTDIR .. "/test.dll", { shared = true, remove = true, protected = true })
		uninstallLib(INSTDIR .. "/test.dll", { shared = true, remove = true, reboot = true })
		uninstallLib(INSTDIR .. "/test.dll", { shared = true, remove = true })

		uninstallLib(INSTDIR .. "/test.dll", { type = "REGDLL", shared = true })
		uninstallLib(INSTDIR .. "/test.dll", { type = "REGDLL", shared = true, remove = true, reboot = true, protected = true })
		uninstallLib(INSTDIR .. "/test.dll", { type = "REGDLL", shared = true, remove = true, protected = true })
		uninstallLib(INSTDIR .. "/test.dll", { type = "REGDLL", shared = true, remove = true, reboot = true })
		uninstallLib(INSTDIR .. "/test.dll", { type = "REGDLL", shared = true, remove = true })

		uninstallLib(INSTDIR .. "/test.dll", { type = "TLB", shared = true })
		uninstallLib(INSTDIR .. "/test.dll", { type = "TLB", shared = true, remove = true, reboot = true, protected = true })
		uninstallLib(INSTDIR .. "/test.dll", { type = "TLB", shared = true, remove = true, protected = true })
		uninstallLib(INSTDIR .. "/test.dll", { type = "TLB", shared = true, remove = true, reboot = true })
		uninstallLib(INSTDIR .. "/test.dll", { type = "TLB", shared = true, remove = true })

		uninstallLib(INSTDIR .. "/test.dll", { type = "REGDLLTLB", shared = true })
		uninstallLib(INSTDIR .. "/test.dll", { type = "REGDLLTLB", shared = true, remove = true, reboot = true, protected = true })
		uninstallLib(INSTDIR .. "/test.dll", { type = "REGDLLTLB", shared = true, remove = true, protected = true })
		uninstallLib(INSTDIR .. "/test.dll", { type = "REGDLLTLB", shared = true, remove = true, reboot = true })
		uninstallLib(INSTDIR .. "/test.dll", { type = "REGDLLTLB", shared = true, remove = true })

		uninstallLib(INSTDIR .. "/test.exe", { type = "REGEXE", shared = true })
		uninstallLib(INSTDIR .. "/test.exe", { type = "REGEXE", shared = true, remove = true, reboot = true, protected = true })
		uninstallLib(INSTDIR .. "/test.exe", { type = "REGEXE", shared = true, remove = true, protected = true })
		uninstallLib(INSTDIR .. "/test.exe", { type = "REGEXE", shared = true, remove = true, reboot = true })
		uninstallLib(INSTDIR .. "/test.exe", { type = "REGEXE", shared = true, remove = true })
	end),
}
