---
title: Commands
description: Every NSIS command, grouped by what you are trying to do, with its Installua spelling.
---

<!-- Hand-written. Not generated yet — see §"Generating this file".
     Every signature, spelling and example below is copied from
     `src/table/overlay.rs`, `src/mui/rows.rs` and `src/lower/mod.rs`, so the
     entries are true as of the current tables. -->


The surface reference. Where `installua coverage` is a census — bucket counts
over every NSIS command, answering _is it done_ — this one is a reference:
things grouped by what a person is trying to do, each with a description, a
signature and something you can paste.

**Reading an entry.** The heading is the NSIS or MUI2 name, because that is what
you already know and what search engines have indexed. Everything under it is
Installua. Square brackets are optional positions, `…` a repeated one, `{ … }`
the trailing options table. `→` names what comes back; `→ nothing` means the
call is a statement.

**Coverage.** Every one of the 276 commands `makensis -CMDHELP` prints is
accounted for: either in a group below, or in [Not available](/reference/commands/not-available/).
The 255 MUI2 names are in [the MUI2 reference](/reference/modern-ui/). Nothing is
pending — the `todo` bucket of both censuses is empty.

---

## The groups

| Group                                                             | Covers                                                                        |
| ----------------------------------------------------------------- | ----------------------------------------------------------------------------- |
| [Program structure](/reference/commands/program-structure/)                           | the blocks a file is made of, and the declarations inside them                |
| [Script attributes](/reference/commands/script-attributes/)                           | the `attributes {}` block: name, output, compression, manifest, version info  |
| [Sections and install types](/reference/commands/sections-and-install-types/)         | what the components page offers and what each part costs                      |
| [Pages and MUI](/reference/modern-ui/)                                 | `page.welcome`, `page.license`, … and everything drawn on them — its own file |
| [Windows and controls](/reference/commands/windows-and-controls/)                     | `page.custom`: controls, colours, fonts, events                               |
| [Languages and locales](/reference/commands/languages-and-locales/)                   | `languages { locales = { … } }` and the language dialog                       |
| [Files and directories](/reference/commands/files-and-directories/)                   | packing files in, and what happens to them on the target disk                 |
| [Files on the target at runtime](/reference/commands/files-on-the-target-at-runtime/) | `fileOpen` and the handle methods                                             |
| [Registry and INI](/reference/commands/registry-and-ini/)                             | reading and writing `HKLM`, `HKCU`, and `.ini` files                          |
| [Processes and the shell](/reference/commands/processes-and-the-shell/)               | `exec`, shortcuts, DLLs, reboot                                               |
| [Strings and numbers](/reference/commands/strings-and-numbers/)                       | `string.*`, arithmetic, comparison                                            |
| [Flow, errors and messages](/reference/commands/flow-errors-and-messages/)            | aborting, testing, telling the user                                           |
| [Windows facts](/reference/commands/windows-facts/)                                   | what the machine is: version, shell folders, registry view                    |
| [Plugins and headers](/reference/commands/plugins-and-headers/)                       | `plugin`, `import`, `raw`, declaring a third-party one                        |
| [Constants](/reference/commands/constants/)                                           | `INSTDIR`, `PROGRAMFILES64`, `HKLM`, …                                        |
| [Not available](/reference/commands/not-available/)                                   | what has no Installua spelling, and what to write instead                     |

---

## Generating this file

This document is hand-written, which is exactly what a correspondence document
must not stay. Most of it is already in the tables:

| Part of an entry          | Where it comes from today                                                                                               |
| ------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| Heading (NSIS name)       | `Row::nsis`                                                                                                             |
| Signature                 | a call renderer over `Instruction`, the shape `src/table/doc.rs` used to print before it was retired with `LANGUAGE.md` |
| Return type               | `Instruction::outputs()` and the `Ty` of each                                                                           |
| Options list              | `Row::options`, the `Offer` names                                                                                       |
| Example                   | `Row::example` — already written, already compiled                                                                      |
| Attribute usage line      | `Class::Attribute(Setting)`                                                                                             |
| "Written by the compiler" | `Class::Lowering(spelling)`                                                                                             |
| "Rejected" reason         | `Class::Rejected(why)`                                                                                                  |
| MUI spelling              | `mui::rows::Row`'s second field                                                                                         |
| **Group**                 | **new: `Row::group`**                                                                                                   |
| **Description**           | **new: `Row::blurb`, one or two sentences**                                                                             |

So the generator is `src/table/reference.rs`, a
`cargo run -q -- generate reference > docs/reference-map.md` arm beside `generate table`, and two
new fields the census can require exactly the way it requires a class today — a
command with no group is a census failure, which is what stops a new NSIS
version quietly adding an undocumented command.

Open questions for the group axis:

1. **One group per command, or several?** `CreateShortcut` is both "files and
   directories" and "processes and the shell". One group keeps the document a
   partition and the census a count; several needs an index instead.
2. **Where do the non-command surfaces go?** `page.*`, `import`, `glob`, the
   control types and `string.*` have no `-CMDHELP` line and so no row, but a
   user reference without them has holes exactly where beginners look. This
   draft writes them by hand; a generator needs rows of their own.
3. **The MUI half is a second table.** `src/mui/rows.rs` carries one spelling
   per name and no group, so the same two fields have to land there too.
