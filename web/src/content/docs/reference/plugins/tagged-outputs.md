---
title: Tagged outputs
description: How a plugin whose output count depends on the outcome is declared, and what that emits.
---

A great many plugins push a number of values that **depends on the outcome**,
and say which by the first value they push. `AccessControl::GrantOnFile` pushes
`"ok"` alone, or `"error"` and a description underneath it;
`StartMenu::Select` pushes `"success"` **and** a folder, or `"cancel"` or an
error message alone.

A flat `outputs` list has to pick one of those and be wrong on the other — and
being wrong is not a wrong type. It is an unbalanced stack: every later `Pop` in
the section shifts by one, and neither Installua nor NSIS says a word. So the
declaration says it instead:

```toml
[[plugin]]
name = "AccessControl"
method = "grantOnFile"
nsis = "AccessControl::GrantOnFile"
params = ["path", "string", "string"]
outputs = ["string"]
tagged = ["error"]
more = ["string"]
```

`outputs` is what comes back on **every** path; `more` is what follows when the
first popped value is one of `tagged`. Both are part of the Lua arity:

```lua
local granted, why = accessControl.grantOnFile(INSTDIR, "(BU)", "FullAccess")
if granted == "error" then
    detailPrint("ACL not set: " .. why)
end
```

which becomes

```nsis
AccessControl::GrantOnFile $INSTDIR "(BU)" "FullAccess"
Pop $0
StrCpy $1 ""
StrCmpS $0 "error" 0 __GENERATED_tail_0
Pop $1
__GENERATED_tail_0:
```

Three things about that emission are deliberate. **The default is written
first**, so `$1` is defined on both paths and the register allocator never sees
a conditional definition — which is what keeps this out of the control-flow
graph entirely. The test is `StrCmpS`, so a payload differing from a tag only in
case is a different value. And the target is a label rather than `+2`, because a
relative jump is correct until a later pass inserts a line.

**`tagged` is a list of literals, not the word "error".** The polarity is the
plugin's to choose, and the two above chose opposite ones: a design that
hardcoded the failure spelling would describe AccessControl and misdescribe
StartMenu by exactly one value, on every run that worked.

A tail the caller does not bind is still popped. The plugin put it there; what
the caller wanted has no bearing on what the stack holds.

## The hazard this cannot cover

The `Pop` is emitted because the declaration says the value is there. A plugin
that pushes its tag and then **fails to push the tail** — AccessControl does
exactly this when `LocalAlloc` fails — leaves the `Pop` to take whatever is
underneath, which is a caller-save, and the stack is corrupt from there on.

Nothing can detect it: NSIS offers no way to ask how deep the stack is, and the
value popped is a perfectly good string. Every hand-written NSIS script that
tests `== error` and pops again has the identical bug. It is an out-of-memory
path on a plugin that just allocated, so it is documented rather than defended
against — the defence would be to not pop at all, which is wrong on every other
run.

---
