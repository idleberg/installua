---
title: Editor Setup
description: Set your editor up for completion, hover and linting on Installua code.
---

Installua code is Lua code, so the Lua tooling you may already have works on it. If you write Lua for a living you can probably skip this page – otherwise, two tools are worth installing once, and every Installua project you ever start will benefit.

## Lua Language Server

[lua-language-server](https://github.com/LuaLS/lua-language-server) is what gives your editor completion, hover documentation, go-to-definition and a warning when a call gets the wrong number of arguments. It reads the type definitions Installua generates for you, so this works across the whole API – commands, page settings and the plugins you declare yourself included.

Most editors have a ready-made extension that bundles it:

| Editor    | Extension     |
|------------|------|
|VS Code     | [sumneko.lua](https://marketplace.visualstudio.com/items?itemName=sumneko.lua) (Marketplace)
|VSCodium, Cursor, Antigravity & friends     | [sumneko.lua](https://open-vsx.org/extension/sumneko/lua) (OpenVSX)
| Sublime Text | [LSP](https://packagecontrol.io/packages/LSP) and [LSP-lua](https://packagecontrol.io/packages/LSP-lua)
| Neovim | [nvim-lspconfig](https://github.com/neovim/nvim-lspconfig)
| Zed | built in, no setup needed

Anything else – the [releases](https://github.com/LuaLS/lua-language-server/releases) page has binaries, and the project documents how to wire them up

## selene

[selene](https://kampfkarren.github.io/selene/) is a separate tool: a Lua linter. Installua uses it for the things a language server won't tell you – above all, the Lua names Installua doesn't support, which selene reports as errors that name the replacement to use instead.

One caveat: the published selene binary is built for Lua 5.1, which can't parse the `<const>` that build-time constants are declared with. Build it for Lua 5.4 instead:

```sh
cargo install selene --features selene-lib/lua54
```

selene is optional. Skip it and everything still compiles – you just lose the friendly nudge when you reach for a Lua feature that isn't there.
