# §1 verified

PLAN Phase 0, task 1 — *"the highest-priority item in the plan"*, because §1 is the
premise the whole design rests on and had no evidence behind it.

Run `./run.sh` to reproduce. Versions under test:

| Tool | Version | Source |
| --- | --- | --- |
| `lua-language-server` | 3.19.1 | Homebrew |
| `stylua` | 2.5.2 | Homebrew |
| `selene` | 0.31.0 | Homebrew **and** crates.io — see S1 |
| `makensis` | 3.12 | Homebrew |

**Verdict: §1 holds, with three corrections.** The claim the design leans hardest on —
`---@alias` giving completion *inside* a string literal — is true, and so is every
rejection claim. What failed is smaller than it could have been, but one of the three
(L1) removes a diagnostic §1 promised the editor would give.

---

## What holds

| § claim | Result | Evidence |
| --- | --- | --- |
| `---@alias` gives completion **inside string literals** | ✅ | `completion_probe.py`, 4/4 positions — array element, scalar field, nested table arg, and block field completion. A real LSP session, not `--check`. |
| An `---@alias` member is **type-checked** inside a string literal | ✅ | `bad.lua:11` → `assign-type-mismatch`, listing every legal member |
| `runtime.builtin = "disable"` turns `require` back into an unknown global | ✅ | `bad.lua:17–21` → `undefined-global` for `require`, `coroutine`, `io`, `os`, `debug` |
| A generated meta file silences the unknown-global noise | ✅ | `good.lua` is clean at `checklevel=Information` |
| LuaLS checks arity on `section` | ✅ | `bad.lua:15` → `missing-parameter` |
| `detailprint` typed from muscle memory is an unknown global with no suggestion | ✅ | `bad.lua:14` → `undefined-global`, exactly as §1 predicted, and no fix-it |
| LuaLS **keeps** `string.gsub`, `pcall`, `math.floor` — flagging them is selene's half | ✅ | none of `bad.lua:24–26` produce a LuaLS diagnostic |
| A selene custom std can flag §5's rejected names | ✅ | see S2 — and it does better than §1 hoped |
| selene names a **replacement**, satisfying §14 | ✅ | `deprecated: { message, replace }` prints `= try: include("mymodule")` with argument substitution |
| `stylua` leaves already-formatted source alone | ✅ | `good.lua` round-trips under `stylua.toml` |

Two bonus findings, neither of which §1 claimed:

- **selene catches the §13 backslash hazard.** `"C:\Program Files"` → `bad_string_escape`.
  LuaLS does **not** flag it. So the editor half of §13's invalid-escape check is selene's,
  and the compiler still has to do it independently because `full-moon` accepts it.
- **LuaLS `missing-fields` works.** Omitting a required field of a block *is* reported
  (`Missing required fields in type 'Attributes': outFile`). It is only the *extra*-field
  direction that fails — see L1.

---

## L1 — LuaLS does **not** warn on an unknown field in a table constructor

§1 promised *"a warning on an unknown field in `attributes {}`"*. It does not happen, in
any configuration tried.

```lua
attributes {
  name    = "Example",
  outFile = "example.exe",
  instalDir = "typo",   -- no diagnostic, at any checklevel
}
```

Reduced to a self-contained repro with no Installua involvement — the behaviour is the
same for `---@param o Opt` and for `---@type Opt`, with and without `---@class (exact)`,
and with `type.checkTableShape`, `diagnostics.neededFileStatus.inject-field` and
`undefined-field` forced on. LuaLS 3.19.1 simply does not check the extra-field direction
on a table constructor.

What *does* fire, on the same class:

| Direction | Diagnostic |
| --- | --- |
| required field **missing** from the constructor | `missing-fields` ✅ |
| unknown field **read** off a typed value (`o.bogus`) | `undefined-field` ✅ |
| unknown field **written** to a typed value (`o.bogus = 1`) | `inject-field` ✅ |
| unknown field **present in the constructor** | *nothing* ❌ |

**Consequence.** A typo'd attribute name is the single most likely mistake in
`attributes {}`, and the editor will not catch it. It is a hard compiler error regardless
(an unknown attribute has no overlay row), so nothing is *unsafe* — but the feedback
arrives at build time rather than as-you-type, and the docs must not promise otherwise.
Marking the block classes `(exact)` costs nothing and is kept, so the day LuaLS gains the
check it starts working; `run.sh` fails if that day arrives, so the docs get updated.

*This is the one §1 claim that failed outright.* Nothing in the design depends on it.

## S1 — no released `selene` binary can lint Installua

`local X <const> = …` (§7) and `//` (§15.4) are Lua 5.3/5.4 syntax. Getting selene to
parse them needs `selene-lib`'s `lua53`/`lua54` cargo features, and **the published
`selene` crate hard-disables them**:

```toml
# selene 0.31.0, selene/Cargo.toml
[dependencies.selene-lib]
version = "=0.31.0"
default-features = false      # selene-lib's own default is ["lua52","lua53","lua54",…]
```

There is no `selene` feature that turns them back on, so `cargo install selene` and
`brew install selene` both produce a binary that reports

```
ERROR: lua version lua54 in standard library, but feature for it is not enabled
error[parse_error]: unexpected token `<`
```

on every Installua file that uses `<const>`. Verified against both.

**Workaround, and it works:** build from source with one line changed —

```
features = ["lua52", "lua53", "lua54"]   # on the selene-lib dependency
```

— after which everything in S2 below passes. This is an upstream packaging bug and worth
filing; until it is fixed, `installua init` must say so rather than assuming `selene` on
`PATH` is usable.

## S2 — the std is YAML, and it must declare **no base**

Two corrections to §1's *"a custom std TOML"*:

1. **selene 0.31 reads YAML**, not TOML. TOML is the pre-0.20 format and is rejected.
2. **Setting `base:` silently overwrites `lua_versions:`.** From
   `selene-lib/src/standard_library/mod.rs`:

   ```rust
   // Intentionally not a merge, didn't seem valuable
   if !other.lua_versions.is_empty() {
       self.lua_versions = other.lua_versions;
   }
   ```

   and there is **no `lua54` base std at all** — selene ships `lua51`, `lua52`, `lua53`
   and `luau`. So `base: lua53` + `lua_versions: [lua54]` parses as 5.3 and rejects
   `<const>`; `base: lua54` fails to resolve.

   The generated std therefore declares **no base** and lists the whole surface itself.
   That is not a workaround so much as the right shape: §5 whitelists the stdlib
   wholesale, so a base std would only be there to be subtracted from, and dropping it
   retires the entire `removed: true` list. `gen_selene_std.py` is the prototype.

With no base, `lua_versions: [lua54]` alone, everything works:

```
error[undefined_variable]: `detailprint` is not defined
error[incorrect_standard_library_use]: standard library function `section` requires 2 parameters, 1 passed
error[deprecated]: standard library function `string.gsub` is deprecated
  = Lua patterns have no NSIS lowering
  = try: string.replace("a", "%a", "b")
```

That last shape is worth naming: `deprecated` takes a `message` *and* a `replace`
template with `%1`-style argument substitution, so §14's *"every rejection names its
replacement"* is generated data rather than compiler code. `[lints] deprecated = "deny"`
promotes it from warning to error, which §14's *"warnings are failures"* requires.

## T1 — `stylua` needs a config, and the default one is destructive

§1 said *"stylua, as-is — no work"*. As-is, stylua's default
`call_parentheses = "Always"` rewrites every block:

```lua
attributes { name = "Example" }   -->   attributes({ name = "Example" })
installer { … }                   -->   installer({ … })
messageBox { text = "Go?" }       -->   messageBox({ text = "Go?" })
```

Still valid Lua and still compiles, but it defaces the surface syntax §15.10 exists to
provide, on every save.

The four settings, against the two idioms that matter:

| `call_parentheses` | `attributes { … }` | `import "WinVer"` | `detailPrint("x")` |
| --- | --- | --- | --- |
| `Always` (default) | ❌ `attributes({ … })` | ❌ `import("WinVer")` | `detailPrint("x")` |
| `NoSingleTable` | ✅ | ❌ `import("WinVer")` | `detailPrint("x")` |
| `None` | ✅ | ✅ | ⚠️ `detailPrint "x"` |
| **`Input`** | ✅ | ✅ | ✅ |

`Input` — preserve what the author wrote — is the only one that leaves all three alone,
and is what `stylua.toml` uses. `None` is defensible (§1 itself likes the `f "string"`
sugar) and rewrites nothing incorrectly; it is a house-style call rather than a
correctness one, and `Input` is the choice that never rewrites working code.

**So `installua init` must write `stylua.toml`.** One more generated file, no design
change beyond that.

---

## Design changes this produces

1. `installua init` writes **`stylua.toml`** (`call_parentheses = "Input"`) alongside
   `.luarc.json` and `installua.toml`. §1's *"no work"* for stylua is wrong.
2. The generated selene std is **`installua.yml`**, YAML, **no `base:`**, with
   `lua_versions: [lua54]` — and it enumerates the kept stdlib rather than subtracting
   from a base. Note the near-collision with the `installua.toml` project marker; they
   are different files.
3. §14's *"every rejection names its replacement"* is satisfied for the **stdlib** half by
   selene `deprecated: { message, replace }`, which is generated data. The compiler still
   owns the other half (unknown commands, rejected syntax).
4. The docs must **not** claim the editor catches a typo'd attribute name (L1). The
   compiler does.
5. `installua init` must handle **selene being unusable** (S1) rather than assuming it
   works: detect the `lua version lua54 … feature is not enabled` error and point at the
   source build.
6. The **invalid-escape check has an editor half after all** — selene's
   `bad_string_escape`. §13 assumed only the compiler could catch it. The compiler still
   must, since `full-moon` accepts it, but the editor gets there first.

## Files

| File | What |
| --- | --- |
| `run.sh` | the whole verification, re-runnable; `SELENE=` overrides the binary |
| `meta/installua.lua` | prototype of the generated `---@meta` |
| `.luarc.json` | prototype of what `installua init` writes |
| `stylua.toml` | ditto, and the T1 fix |
| `gen_selene_std.py` → `installua.yml` | prototype of the generated selene std |
| `selene.toml` | ditto, with `deprecated = "deny"` |
| `good.lua` | legal Installua; must be clean under all three tools |
| `bad.lua` | one claim per line, each with its expected diagnostic |
| `escapes.lua` | the §13 backslash hazard |
| `completion_probe.py` | a real LSP session; completion has no headless mode |
