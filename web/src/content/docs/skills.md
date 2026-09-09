---
title: Agent skills
description: Two skills that teach a coding agent the jobs it would otherwise guess at — porting an NSIS script, and declaring a plugin.
---

Installua ships two skills for coding agents, in
[`agents/skills/`](https://github.com/idleberg/installua/tree/main/agents/skills).
They're plain Markdown, so any agent that reads skill files can use them — Claude
Code is what they were written against.

They exist because both jobs go wrong in the same quiet way. An agent that knows
NSIS will happily invent an Installua spelling that looks right and isn't, or
guess how many values a plugin pushes and be off by one. Neither mistake fails
loudly. The skills replace the guess with a lookup.

## installua-port-nsis

For turning an existing `.nsi` or `.nsh` into Installua.

The thing it gets right is that a port isn't a translation. An NSIS script is two
languages stacked — the preprocessor on top, the installer commands underneath —
and Installua has no preprocessor at all, so `!define`, `!macro` and `!if` don't
convert, they dissolve into constants, functions and ordinary `if`. The skill
takes an inventory of the script first and tells you what shape the port will be
before a line of Lua is written.

It also enforces the rule that matters most: every command's spelling gets looked
up in the [command reference](/reference/commands/), never recalled. And it
leaves `PORT NOTES` behind for the code that compiles but no longer behaves the
same as the original — the leftovers nothing downstream will ever raise again.

Reach for it when you're converting a script, or when you just want to know what
one NSIS command is called here.

## installua-plugin-declaration

For a plugin Installua doesn't ship a declaration for.

A declaration carries one fact the compiler can't discover: how many values the
plugin leaves on the stack. Get it wrong and nothing fails — every later `Pop`
shifts by one and the script assembles cleanly. So the skill goes and reads the
plugin's own C source for the count, finds it on the
[NSIS wiki](https://nsis.sourceforge.io/Category:Plugins) and unzips the archive
rather than trusting a download URL, and refuses to write a method it can't pin
down instead of guessing one.

The output is a `.toml` in your project's `.installua/declarations/`, with a
comment naming the file each count came from. See
[Plugins](/reference/plugins/) for the ones that already ship declared.

Reach for it when a `plugin` or `import` name comes back undeclared.

## Installing them

Copy either directory into `.claude/skills/` in your project, or into
`~/.claude/skills/` to have it everywhere:

```sh
git clone https://github.com/idleberg/installua
mkdir -p ~/.claude/skills
cp -r installua/agents/skills/* ~/.claude/skills/
```

Agents load a skill when the work matches its description, so there's nothing to
invoke — start a port, or hit an undeclared plugin, and the right one comes in.
You can also ask for it by name.

:::note
Both skills grep this documentation rather than quoting it, so they stay correct
as the reference changes. Point them at a checkout of the repository, or let them
fetch the pages from GitHub.
:::
