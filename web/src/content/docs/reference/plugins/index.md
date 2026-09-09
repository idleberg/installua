---
title: Plugins
description: Every plugin method Installua ships a declaration for, and every method it deliberately does not.
---

A plugin is a <abbr title="Dynamic-Link Library — a Windows file holding code that a program loads while it runs">DLL</abbr>
that adds a command your installer can call. <abbr title="Nullsoft Scriptable Install System">NSIS</abbr>
has a large collection of them, and Installua ships a ready-made description of
most of what they offer, so you can call a plugin method and have your editor
know what it takes and what it gives back.

## What a declaration is, and what it isn't

Each description carries **one fact**: how many values the plugin leaves behind
for you to collect.

That sounds small, and it is the one thing worth getting right. A plugin hands
its answers back on a stack, and your script takes them off one at a time. If
the expected number is wrong, nothing complains — the build succeeds, and every
value your script picks up from then on is off by one. You find out when a user
does.

**A declaration is not the plugin itself.** Nothing here ships a `.dll`. Getting
the plugin onto your machine, either by installing it where NSIS keeps its
plugins or by pointing `dir` at your own copy, is still up to you.

Elsewhere in the docs:

- [Declaring a third-party plugin or header](/reference/commands/plugins-and-headers/#declaring-a-third-party-plugin-or-header)
  — how to describe a plugin we don't cover
- [`plugin`](/reference/commands/plugins-and-headers/#plugin) — how to call one
- [Headers](/reference/headers/) — the macro libraries that ship with NSIS,
  such as `FileFunc` and `WordFunc`

## Where the numbers come from

Guessing here would be worse than useless, so every number was checked, and each
plugin's page says which of these it came from:

1. **What the plugin says about itself** — its readme, or its wiki page when
   that's all there is.
2. **What real scripts do** — a scan of 984 installers found in the wild,
   counting the values each call actually collects.

When those two disagree, we read the plugin's source code and go with what it
really does. That has happened four times so far, and it was worth the trouble
every time. [AccessControl](/reference/plugins/third-party-plugins/#accesscontrol)
is the clearest case: its readme, its wiki page and the real-world scripts each
suggest a *different* answer, and none of the three is right. Documentation
tends to describe the one path its author had in mind, while a count has to hold
for every path.

The real-world scan is also what caught
[`SimpleSC.getErrorMessage`](/reference/plugins/third-party-plugins/#simplescgeterrormessage-is-not-declarable),
which takes its input in a way no amount of reading would have revealed.

[NScurl](/reference/plugins/third-party-plugins/#nscurl) is the one exception.
It's newer than the scan, and its readme is a tour rather than a list, so its
numbers were read straight out of its source. That's the last resort everywhere
else, and the starting point only here.

Where a method's count still can't be pinned down, we leave it out rather than
guess. You can always
[declare it yourself](/reference/commands/plugins-and-headers/#declaring-a-third-party-plugin-or-header).

---
