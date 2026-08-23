attributes {
  name = "Polyglot",
  outFile = "polyglot-setup.exe",
  installDir = PROGRAMFILES .. "/Polyglot",
}

languages {
  ask = {
    title = "Installer Language",
    info = "Please select a language.",
    alwaysShow = true,
    remember = { root = "HKCU", key = "Software\\Polyglot", value = "Installer Language" },
  },

  locales = {
    English = {
      greeting = "Installing Polyglot",
      farewell = "Removing Polyglot",
      folder = "Files go to",
    },
    German = {
      greeting = "Polyglot wird installiert",
      farewell = "Polyglot wird entfernt",
      folder = "Dateien landen in",
    },
    PortugueseBR = {
      greeting = "Instalando o Polyglot",
      farewell = "Removendo o Polyglot",
      folder = "Os arquivos vao para",
    },
  },
}

installer {
  page.directory {},
  page.instFiles {},

  section("Core", function()
    detailPrint(lang.greeting)
    detailPrint(lang.folder .. ": " .. INSTDIR)
    setOutPath(INSTDIR)
    writeUninstaller(INSTDIR .. "/uninstall.exe")
  end),
}

uninstaller {
  page.confirm {},
  page.instFiles {},

  section("Uninstall", function()
    detailPrint(lang.farewell)
    delete(INSTDIR .. "/uninstall.exe")
    rmDir(INSTDIR)
  end),
}
