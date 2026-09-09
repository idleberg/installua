---
title: Create project
description: Set up a new Installua project and compile your first installer.
---

With all previous steps completed, let's start our first project.

## Create the folder

From the command-line or file manager of choice:

```sh
mkdir my-app && cd my-app
```

## Initialize the project

First time users should stick to interactive mode:

```sh
installua init --interactive
```

Confirm the directory when it asks, then pick everything on the menu – the config files, the stubs your editor reads, the VS Code settings and the `.gitignore` entries are the recommended defaults for a new project.

## Write the installer

`installua init` names `install.lua` as the entry file, so that's what to write next:

```lua
attributes {
	name = "MyApp",
	outFile = "MyApp-setup.exe",
	compressor = "lzma",
}

installer {
	installDir = PROGRAMFILES64 .. "/MyApp",

	page.welcome {},
	page.directory {},
	page.instFiles {},
	page.finish {},

	section("Core", function()
		setOutPath(INSTDIR)
		file("myapp.exe")
		writeUninstaller(INSTDIR .. "/uninstall.exe")
	end),
}
```

## Build it

```sh
installua build install.lua
```

That compiles your program and hands it to `makensis`, leaving `MyApp-setup.exe` behind – your first installer.
