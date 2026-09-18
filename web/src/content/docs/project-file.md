---
title: The installua.toml file
description: The file that marks a workspace, lists its installers, and lets a build run without naming one.
---

`installua.toml` is useful for two things:

- **building without naming a file**, so `installua build` is the whole command
- **several installers in one checkout** sharing the same plugin and header
  declarations instead of each keeping its own copy

You don't need one. Without it, you name the `.lua` file on every command and
each installer reads only its own declarations — which is how most projects
work.

## What goes in it

A list of projects. Each one is a name and the `.lua` file it builds:

```toml
[[project]]
name = "pro"
entry = "installers/pro/install.lua"

[[project]]
name = "lite"
entry = "installers/lite/install.lua"
```

| Key | What it does |
| --- | ------------ |
| `entry` | The `.lua` file to build, relative to `installua.toml`. Required. |
| `name` | What `-p` picks the project by. Optional while the file lists one project; required, and unique, once it lists two. |

That's all the file may say. **A key we don't recognise stops the build.** That
looks unfriendly for a file this small, and it's deliberate: a typo like
`entyr = …` would otherwise leave that project with nothing to build, and you'd
find out from a confusing error somewhere else.

An empty file is valid too. It lists no projects, but it still marks the top of
a workspace, which is what the shared declarations below need.

## Building without naming a file

With one project listed, `build`, `emit` and `check` don't need a file. Run them
from the folder holding `installua.toml` or from anywhere below it:

```console
$ installua build
$ cd installers/pro && installua build     # the same thing
```

With several, pick one with `-p`:

```console
$ installua build -p pro
$ installua emit -p lite -o dist/lite.nsi
```

Leave `-p` off with several projects and, at a terminal, `build` and `emit` ask
which one, from a list you can search by name or entry. Without a terminal
(a script, CI) it's an error that lists the names instead. Either way it won't
guess, because building the wrong installer is worse than building none.

`check` is the exception: with nothing named, it checks **every** project, since
it's what you'd run in CI and one broken installer should fail it. `-p` works
there too, as often as you like:

```console
$ installua check              # all of them
$ installua check -p pro -p lite
```

Naming a file still works as it always did, and then `installua.toml` isn't
consulted for which program to build. Naming a file *and* `-p` is an error.

## Sharing declarations

When you build, Installua reads declarations from a `.installua/declarations/`
folder next to your `.lua` file. With `installua.toml` above it, it also reads
the one next to `installua.toml`:

```text
myapp/
  installua.toml              # the workspace
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

## Where the search stops

Installua walks up from your `.lua` file — or, with no file named, from the
folder you're in — and stops at the first of these:

- **an `installua.toml`.** The nearest one is the workspace, and nothing above
  it is read. Keep one per workspace, at the top: a second one in
  `installers/pro/` would become that installer's workspace and hide the shared
  declarations. (A project's own `.installua/declarations/` is always read, so
  it never needs a file of its own for that.)
- **any folder holding `.git`**, because a checkout is a boundary whether or not
  anyone put an `installua.toml` at the top of it
- **the root of the drive**, so a stray `installua.toml` in your home folder
  can't quietly join every build on the machine

## Coming from 0.1

Installua 0.1's file looked like this, and `installua init --workspace` wrote
it:

```toml
[project]
root = true
```

Both lines are gone: the file itself marks the workspace now, so there is
nothing for `root` to say. Delete them, and add `[[project]]` entries if you
want to build without naming a file. The old form is reported as an error that
says so, rather than read the old way, and `init --workspace` no longer exists.

0.1 also cascaded: an `installua.toml` without `root = true` added its folder's
declarations and let the search keep climbing. That's gone too. Two levels — the
workspace's and the installer's own — is what the layout above needs.

## What it is not

**It is not a settings file.** Your installer's name, its output file, its
version — all of that belongs in `attributes {}` in your `.lua` source, where
you can see it beside the rest of the program. See
[Script attributes](/reference/commands/script-attributes/).

**It is not where build values go either.** A version number or a signing flag
that changes per build is a
[parameter](/reference/commands/program-structure/#param), passed on the command
line, so the same source can produce a nightly and a release.
