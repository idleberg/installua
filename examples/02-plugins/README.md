# Program 2 — plugin-heavy

Assembles clean under `makensis -WX`.

The stack-ABI program. All three opaque callees appear — `plugin`, `System::Call` and
`raw` — with a live value across each.

## What it settled

**`Push`, `Pop` and `Exch` need no Installua spelling, and that is a decision the plugin
declarations pay for.** `local rc, out = nsExec.execToStack(cmd)` is only expressible
because the declaration in `.installua/headers/` says the call leaves two values on the
stack. Without it the surface degenerates to a `pop()` builtin, which reintroduces exactly
the "output position is not a convention" problem §11 identifies for header macros. So
plugin declarations carry an **output count** and are not optional for any plugin whose
result is used.

**`System::Call`'s `.r0` versus `.s` is a surface decision, not just a clobber question.**
`.r0` writes a register the AST does not know about — that is why §15.11 calls
`System::Call` opaque. But `.s` pushes to the stack instead, and a declaration *can*
describe that. So the honest spelling routes results through `.s`, and `.r0` stays
legal-but-opaque for people who want it. Worth saying out loud in the docs; it turns a
"this clobbers everything" warning into an avoidable one.

**The one caller-save in the five programs is here.** `out` is live across the
`System::Call`, so:

```nsis
  Push $1
  System::Call "kernel32::GetTickCount() i .s"
  Pop $0
  Pop $1
```

Note how rare that is, and why — see below.

## What it left open

**§4's string-template model makes caller-saves much rarer than expected.** `target` and
`label` look like two live locals across four opaque calls. They are not: `..` lowers to a
template, so `target` is the literal text `"$INSTDIR\tool.exe"` and `label` is
`"${APP} smoke test"`, and neither ever occupies a register. Only values that come *out*
of an instruction — a `Pop`, a `ReadRegStr` — are ever live in the sense the allocator
cares about.

That is good news for output quality and bad news for test coverage: the clobber fixpoint
is the riskiest code in the plan (PLAN §4) and the five programs exercise it **once**.
Phase 3 needs synthetic tests that assert clobber sets at the IR boundary directly, which
is what PLAN already says — this is the evidence for why.

**A global's initialiser has no obvious home.** `gitDescribe = ""` at top level becomes
`Var gitDescribe` plus a `StrCpy`, and the `StrCpy` has to go *somewhere*. Here it is the
first line of the first section, which is wrong the moment a second section runs first or
`.onInit` reads it. §15.24 decides that assignment declares a global but does not say where
the initialising store lands. Candidates: the top of `.onInit` (and `un.onInit`), or
nowhere at all if the initialiser is a constant, since NSIS zero-initialises `Var`s. The
second is cheaper and correct for `""`; a non-constant initialiser needs the first.

**Nothing checks the `raw` block's `Pop $gitDescribe`.** That is the design working as
intended — `raw` is the visibly-unchecked escape hatch — but it means the "no local
survives a `raw` block" rule needs a *diagnostic*, not just a documented convention. Using
a local after a `raw` block must name the rule and suggest the global.

## Exposed commands used

`setOutPath` · `file` · `detailPrint` · `abort` · `messageBox` · `os.exit` (`Quit`)

Plugins: `nsExec.execToStack` · `UserInfo.getAccountType` · `System.call`
