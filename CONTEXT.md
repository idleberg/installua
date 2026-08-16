# Installua

A Lua-shaped language that compiles to NSIS installer scripts. Source is valid Lua so that
`stylua`, `selene` and `lua-language-server` work unmodified; everything past the syntax is
NSIS-specific.

## Language

### The compiler's own vocabulary

**Overlay**:
The single data table describing every NSIS command — its Installua name, census class,
parameter list and options. Half generated from `makensis -CMDHELP`, half hand-written,
joined at build time. Read by the emitter, the stub generator and the census.
_Avoid_: instruction table, command table

**Census**:
The test asserting that every line of a pinned `-CMDHELP` snapshot lands in exactly one
overlay class. An unclassified command fails the build, which makes coverage a number
rather than a document someone remembers to update.
_Avoid_: coverage doc, NSIS-Coverage.md

**Class**:
The census bucket a command falls into: `Exposed`, `Attribute`, `LoweringTarget`,
`Directive`, `Language`, `Rejected`, `Todo`. One enum, three consumers — there is no second
vocabulary for buckets.
_Avoid_: bucket, category

**Lowering target**:
An NSIS command reachable only as the output of a construct, never callable from source —
`IntCmp`, `Goto`, `IfErrors`. The stub generator must not offer them, or the condition
design is bypassed on day one.

**Clobber set**:
The registers a function may write, including transitively through everything it calls.
Computed as a fixpoint over the call graph's SCC condensation, and intersected with
liveness at each call site to decide what the caller saves.

**Emitted, never written**:
The recurring shape where an NSIS prefix or marker is supplied by context rather than typed
by the author — `$` sigils, `un.`, `Manifest`, the `.` on `.onInit`, `Uninstall`/`UN`,
`__GENERATED_` labels, `StrFunc` init lines. Eight instances; it is the shape of the
language rather than a coincidence.

**Adapter**:
A hand-written lowering for a Lua stdlib name that no declaration can express, because it
targets instructions rather than a header macro — `string.sub`, `string.format`,
`math.abs`/`max`/`min`. Distinct from a **delegation**, which is a declaration row and no
compiler code.

**Declaration**:
The data describing a foreign header's macro — include path, arity, output position and
count, one-time init, `un.` twin. Two-thirds functional rather than editor-facing: output
position is not a convention across the shipped headers, so guessing emits plausible-looking
and silently wrong NSIS.
_Avoid_: binding, stub (a stub is the generated `---@meta`, which is downstream)

**Line map**:
The sidecar table giving every emitted line an origin — `User`, `Raw` or `Emitted` — so
`makensis` diagnostics can be rewritten against Installua source. NSIS has no `#line`, which
is what forces Installua to own the `makensis` invocation.
_Avoid_: source map

### The surface

**Block**:
One of the four top-level containers: `attributes {}` (script-global, both executables),
`installer {}`, `uninstaller {}` (scoped attributes plus code), `languages {}`. Each appears
exactly once; a second occurrence is a hard error naming the first, never a merge.

**Attribute**:
A setting legal only outside a section — NSIS's own noun for the set. A field of a block,
never a call.

**Import**:
A declared NSIS header brought into scope, returning a handle whose fields are its macros
and its defines. Emits `!include` into the output.
_Avoid_: include (that is source-level inclusion of another Installua file), require

**Include**:
Source-level inclusion of another Installua file, merging its declarations. Frontend-only —
emits nothing, which is what distinguishes it from an import.

**Raw**:
The visibly-unchecked escape hatch, `raw [[ … ]]`. Emitted verbatim at the position it
appears, drops every live local, and clobbers every register. Unchecked constructs must be
visible in the source, never the silent default.

**Predicate**:
An NSIS condition with no left-hand side, surfaced as an ordinary `bool`-returning call —
`fileExists(p)`, `errors()`. Not a separate condition shape. `errors()` is the one impure
member: `IfErrors` clears the flag it reads.

**Fusion**:
Lowering a condition directly into a compare-and-jump rather than materialising a boolean
into a register. NSIS has no test-then-jump, so this is what keeps output readable and
registers free. Three callers: comparisons, predicates, `messageBox`.

**Read to verify, never to decide**:
The rule governing every property of how `makensis` was built — pointer size, charset,
maximum string length. Such a property may never determine what a source file means, or the
same program means two things on two machines; it may only be checked against an assumption
the source states out loud. Applied three times (§15.15, §15.16, §15.31).

**Staging**:
Which machine a line runs on. Three visibly different spellings: install-time (ordinary
control flow), build-time-folded (`<const>` conditions), and build-machine side effects
(`BUILD.*`). A language whose reader cannot tell which `if` runs when is the trap this
design exists to avoid.
