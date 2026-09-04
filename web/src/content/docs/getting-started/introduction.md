---
title: Why Installua?
description: What is Installua and why do you want to use it.
---

For more than 20 years, [NSIS](https://nsis.sourceforge.io/) has proven to be a trusted solution for writing custom Windows setup routines, installing millions of apps on computers worldwide. It continues to serve as the foundation for tools such as **CPack**, **Electron Builder** or **Pynsist**. While the scripting language is simple, it has its quirks that may frustrate developers.

## The language


Installua gives you a Lua-shaped language instead, one that taps into a rich ecosystem of well-established tools. Language server support, types, linting – it's all there from the start. You write an ordinary-looking Lua program, and Installua compiles it into a `.nsi` script and hands that to `makensis`.

Let's learn about the Installua's key features.

### Functions without a stack

In NSIS, passing a value to a function means pushing it onto a stack and popping it back off in the right order. Forget one `Pop` and the next function reads someone else's value. In Installua you declare parameters, pass arguments and return values the way you would in any other language. The compiler works out the register juggling and writes the stack code for you.

### No ordering rules

Call a function before you declare it. Reference a section further down the file. Put things in whatever order tells the clearest story, because the compiler reads your whole program before it decides anything – there is no "define it above where you use it" rule to remember.

### MUI2 by default, without the footguns

The Modern UI is what most installers want, so it is simply on. Pages are values you list in the order they should appear, and the settings each page accepts are the settings you can write – a typo, or a setting on the wrong page, is a compile error rather than a surprise at runtime.

### A fully typed API

Every command, page setting and plugin method carries a type. Your editor knows how many arguments a call takes and what comes back, including for the third-party plugins you declare yourself. Mistakes surface as you type instead of after a build.

### Build-time parameters and `if`

Some values belong to the build, not to the source: a version number, a signing flag, whether this one is the nightly. Declare them as parameters, let your CI supply them, and branch on them with a build-time `if`. The branch you don't take never reaches the installer at all.

### Editor support out of the box

`installua init` and `installua stubs` write the config and definition files your editor needs. With [lua-language-server](https://luals.github.io) you get completion, hover and go-to-definition across the whole API. The generated [selene](https://kampfkarren.github.io/selene/) config is the other half: the Lua names Installua doesn't support become lint errors that name their replacement.

### Warnings are errors

Builds run `makensis -WX`, so anything NSIS grumbles about stops the build. That sounds strict, and it is – but it means a green build is genuinely green, and you hear about a problem now rather than from a user later.


