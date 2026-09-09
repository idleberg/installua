---
title: Installation
description: How to install Installua and everything it needs.
---

Installua is a single command-line program, so getting set up takes a minute or two. Pick whichever of the routes below fits the tools you already have.

:::caution
Installua writes NSIS scripts and passes them to the NSIS compiler, so you need [NSIS 3.0](https://nsis.sourceforge.io/Download) or later installed and reachable on your `PATH`.
:::

Various installations methods are at your disposal, including a variety of package managers.

### Cargo

If you have the Rust toolchain around, this builds Installua from source and puts it on your `PATH`:

```sh
cargo install installua
```

### Winget

On Windows, the package manager that ships with the system will do:

```powershell
winget install --id idleberg.installua
```

### Homebrew

On macOS and Linux, from our own tap:

```sh
brew install idleberg/asahi/installua
```

### GitHub

No package manager? Download the installer straight from the [releases](https://github.com/idleberg/installua/releases) page.
