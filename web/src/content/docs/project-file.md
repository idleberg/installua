---
title: The installua.toml file
description: The marker that tells a build where your project starts, and the only three things it may say.
---

The project file may become useful when you have **several installers in one checkout** that
should share the same plugin and header declarations, instead of each keeping
its own copy.

## What it actually does

It marks a starting line. Nothing else.

When you build, Installua looks for declarations in a `.installua/declarations/`
folder next to your `.lua` file. With this marker in place, it keeps walking up
through the parent folders and reads every `.installua/declarations/` it passes
on the way, stopping at the folder that holds the marker.

```text
myapp/
  installua.toml              # root = true — the walk stops here
  .installua/declarations/
    acme.toml                 # both installers below can use this
  installers/
    pro/install.lua
    lite/install.lua
```

Both installers now see `acme.toml` without a copy each. If one of them wants a
different version of the same declaration, it puts its own in
`installers/pro/.installua/declarations/` — the nearer folder wins, so what's at
the top is a default rather than a rule.

## Making one

```console
$ installua init --workspace .
```

That writes the whole file:

```toml
[project]
root = true
```

You can also just write those two lines yourself. There's nothing else to it.

## What you may put in it

Three keys, and only one of them does anything today:

| Key | What it does |
| --- | ------------ |
| `root` | `true` or `false`. `true` means the walk stops here. This is the one that matters. |
| `entry` | A quoted path. Accepted, but nothing reads it yet. |
| `name` | A quoted name. Accepted, but nothing reads it yet. |

`entry` and `name` are there for commands that don't exist yet — building a
project without naming a file, picking one project out of several. They're
checked for spelling and type now so a file you write today doesn't turn out to
be wrong on the day they start counting.

**A key we don't recognise stops the build.** That looks unfriendly for a two-line
file, and it's deliberate: a typo like `rooot = true` would otherwise do nothing
quietly, and your build would compile against a different set of declarations
than you meant, with nothing to tell you.

## Where a build stops walking, even without a marker

Three things end the search, so a file in the wrong place can't reach into
builds it has nothing to do with:

- the marker that says `root = true` — the one you chose
- any folder holding `.git`, because a checkout is a boundary whether or not
  anyone wrote a marker at the top of it
- the root of the drive, so a stray `installua.toml` in your home folder can't
  quietly join every build on the machine

## What it is not

**It is not a settings file.** Your installer's name, its output file, its
version — all of that belongs in `attributes {}` in your `.lua` source, where
you can see it beside the rest of the program. See
[Script attributes](/reference/commands/script-attributes/).

**It is not where build values go either.** A version number or a signing flag
that changes per build is a
[parameter](/reference/commands/program-structure/#param), passed on the command
line, so the same source can produce a nightly and a release.

---
