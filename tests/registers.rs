//! Phase 3 at the pass boundary (§14 tier 0): liveness, the calling convention
//! and the clobber fixpoint, asserted as properties of the IR.
//!
//! The distinction matters more here than anywhere else in the compiler. A
//! caller-save restored one `Exch` out of place produces an installer that
//! assembles clean, runs, and gives a wrong answer — program 4's README calls
//! this the one program where assembling proves nothing. So the convention is
//! checked as a list of pushes and pops rather than as text that happens to
//! contain them.

use std::collections::{BTreeMap, BTreeSet};

use installua::callgraph::CallGraph;
use installua::cfg::Body;
use installua::diag::{Code, Diagnostics, Span};
use installua::ir;
use installua::layout;
use installua::regs::Slot;

fn module(source: &str) -> ir::Module {
    let mut diags = Diagnostics::new();
    let module = installua::compile(source, &mut diags);
    assert!(!diags.has_errors(), "{}", diags.render("<test>"));
    module.expect("compiles")
}

fn program(statements: &str) -> String {
    format!(
        "attributes {{ outFile = \"a.exe\" }}\n\
         installer {{ section(\"Core\", function()\n{statements}\nend), }}\n"
    )
}

/// The first section's body.
fn section(module: &ir::Module) -> &Body {
    module
        .bodies()
        .find(|(name, _)| *name == "Core")
        .map(|(_, body)| body)
        .expect("the section")
}

fn function<'m>(module: &'m ir::Module, name: &str) -> &'m Body {
    module
        .bodies()
        .find(|(bound, _)| *bound == name)
        .map(|(_, body)| body)
        .unwrap_or_else(|| panic!("no `{name}`"))
}

/// Every register the body writes, in order. The one number liveness is really
/// about, and unreadable from the emitted text without also reading the
/// instruction set.
fn writes(body: &Body) -> Vec<String> {
    let mut out = Vec::new();
    for item in layout::lay_out(body) {
        if let ir::Item::Instruction(instruction) = item {
            for slot in instruction.defs() {
                out.push(slot.nsis());
            }
        }
    }
    out
}

/// The laid-out body as instruction names, with labels as `name:`.
fn shape(body: &Body) -> Vec<String> {
    layout::lay_out(body)
        .into_iter()
        .map(|item| match item {
            ir::Item::Instruction(instruction) => instruction.name,
            ir::Item::Label(label) => format!("{label}:"),
        })
        .collect()
}

fn saves(body: &Body, site: usize) -> Vec<String> {
    body.calls[site].saves.iter().map(Slot::nsis).collect()
}

// -- liveness --------------------------------------------------------------

/// The property nsL's issue #5 is the absence of: a register is reused once
/// the value in it is dead, and *only* then. Both `StrLen`s write `$0` because
/// the first result is finished with by the time the second is computed.
#[test]
fn a_dead_value_frees_its_register() {
    let module = module(&program(
        "\
local a = string.len(INSTDIR)
detailPrint(\"a \" .. a)
local b = string.len(EXEDIR)
detailPrint(\"b \" .. b)",
    ));

    assert_eq!(writes(section(&module)), vec!["$0", "$0"]);
}

/// And the negative case, without which the one above passes on an allocator
/// that hands out `$0` to everything. Here the first value is still live when
/// the second is computed, so they cannot share.
#[test]
fn a_live_value_keeps_its_register() {
    let module = module(&program(
        "\
local a = string.len(INSTDIR)
local b = string.len(EXEDIR)
detailPrint(\"both \" .. a .. b)",
    ));

    assert_eq!(writes(section(&module)), vec!["$0", "$1"]);
}

/// A `Var` is user-visible state whose whole purpose is to survive, so it is
/// not allocated, not coloured, and — the part that is correctness rather than
/// thrift — never saved around a call (§15.11).
#[test]
fn a_global_is_never_allocated() {
    let module = module(
        "attributes { outFile = \"a.exe\" }\n\
         func(\"helper\", function() detailPrint(\"helping\") end)\n\
         installer { section(\"Core\", function()\n\
         state = \"fresh\"\n\
         helper()\n\
         detailPrint(state)\n\
         end), }",
    );

    let body = section(&module);
    assert_eq!(writes(body), vec!["$state"]);
    assert!(
        saves(body, 0).is_empty(),
        "a global was saved around a call"
    );
}

// -- the calling convention ------------------------------------------------

/// The three rules program 4 pins, as one sequence: saves first, arguments in
/// reverse source order, results popped in source order, saves restored in
/// reverse (§11, §15.11).
#[test]
fn the_calling_convention_is_program_fours() {
    let module = module(
        "attributes { outFile = \"a.exe\" }\n\
         func(\"pair\", function(a, b) return a, b end)\n\
         installer { section(\"Core\", function()\n\
         local keep = string.len(INSTDIR)\n\
         local first, second = pair(\"x\", \"y\")\n\
         detailPrint(first .. second .. keep)\n\
         end), }",
    );

    let body = section(&module);
    assert_eq!(
        shape(body),
        vec![
            "StrLen", // keep -> $0
            "Push",   // save $0: live across, and `pair` writes it
            "Push",   // "y"  -- reverse source order
            "Push",   // "x"
            "Call",
            "Pop", // first
            "Pop", // second
            "Pop", // restore $0
            "DetailPrint",
        ]
    );
    assert_eq!(saves(body, 0), vec!["$0"]);

    // The callee's half: arguments come off in source order, results go on in
    // reverse, so neither side needs an `Exch`.
    assert_eq!(
        shape(function(&module, "pair")),
        vec!["Pop", "Pop", "Push", "Push"]
    );
}

/// A save costs a push and a pop, so the saved set is the intersection and not
/// the union: a value that is dead at the call is not saved even though the
/// callee overwrites its register.
#[test]
fn a_dead_value_is_not_saved() {
    let module = module(
        "attributes { outFile = \"a.exe\" }\n\
         func(\"noisy\", function() detailPrint(\"noise\") end)\n\
         installer { section(\"Core\", function()\n\
         local done = string.len(INSTDIR)\n\
         detailPrint(\"len \" .. done)\n\
         noisy()\n\
         end), }",
    );

    assert!(saves(section(&module), 0).is_empty());
}

/// A returned value nobody binds still has to come off the stack: the callee
/// pushed it either way, and leaving it there unbalances every call after it
/// (§11 — "a dropped output still has to be allocated").
#[test]
fn a_dropped_return_still_comes_off_the_stack() {
    let module = module(
        "attributes { outFile = \"a.exe\" }\n\
         func(\"two\", function() return 1, 2 end)\n\
         installer { section(\"Core\", function()\n\
         two()\n\
         end), }",
    );

    assert_eq!(shape(section(&module)), vec!["Call", "Pop", "Pop"]);
}

// -- the clobber fixpoint --------------------------------------------------

/// Recursion needs no special case: `countdown` is an SCC of one, and the
/// recursive call site saves `n` precisely because the fixpoint went round the
/// cycle and put `$0` in `countdown`'s own clobber set (§15.11).
#[test]
fn recursion_saves_across_its_own_call() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/returns.lua"),
    )
    .expect("the golden source");

    let module = module(&source);
    let countdown = function(&module, "countdown");
    assert_eq!(
        saves(countdown, 0),
        vec!["$0"],
        "`n` is live across the call"
    );

    // And a leaf whose caller has nothing live pays nothing at all.
    let budget = function(&module, "budget");
    assert!(saves(budget, 0).is_empty());
}

/// The fixpoint itself, on a graph with a cycle in it and no program around it.
/// PLAN §2 recorded that clobber sets were exercised once across all five
/// programs; this is the synthetic case that says what the algorithm does.
///
/// ```text
/// root → a ⇄ b → leaf
/// ```
///
/// `a` and `b` are one component, so both end up with the union of the whole
/// cycle *and* of everything below it — which is why one extra round saturates.
#[test]
fn a_cycle_shares_one_clobber_set() {
    let names = ["root", "a", "b", "leaf"].map(String::from).to_vec();
    let graph = CallGraph {
        edges: vec![
            BTreeSet::from([1]),
            BTreeSet::from([2]),
            BTreeSet::from([1, 3]),
            BTreeSet::new(),
        ],
        sccs: vec![vec![3], vec![1, 2], vec![0]],
        spans: vec![Span::default(); 4],
        names,
    };

    let direct = BTreeMap::from([
        ("root".to_string(), BTreeSet::from([0u8])),
        ("a".to_string(), BTreeSet::from([1u8])),
        ("b".to_string(), BTreeSet::from([2u8])),
        ("leaf".to_string(), BTreeSet::from([3u8])),
    ]);

    let total = graph.clobbers(&direct);
    assert_eq!(total["leaf"], BTreeSet::from([3]));
    assert_eq!(total["a"], BTreeSet::from([1, 2, 3]));
    assert_eq!(total["b"], BTreeSet::from([1, 2, 3]));
    assert_eq!(total["root"], BTreeSet::from([0, 1, 2, 3]));
}

/// Mutual recursion is one component too, and the depth-cliff warning names
/// both members rather than whichever one happened to be visited first (§3).
#[test]
fn mutual_recursion_warns_once_naming_the_cycle() {
    let mut diags = Diagnostics::new();
    installua::compile(
        "attributes { outFile = \"a.exe\" }\n\
         func(\"ping\", function() pong() end)\n\
         func(\"pong\", function() ping() end)\n\
         installer { section(\"Core\", function() ping() end), }",
        &mut diags,
    );

    let warnings: Vec<&str> = diags
        .iter()
        .filter(|d| d.code == Code::DeepRecursion)
        .map(|d| d.message.as_str())
        .collect();
    assert_eq!(warnings.len(), 1, "one warning per cycle, not per function");
    assert!(
        warnings[0].contains("`ping`") && warnings[0].contains("`pong`"),
        "the warning names only part of the cycle: {}",
        warnings[0]
    );
}

// -- rejections ------------------------------------------------------------

/// `Call` has no arity — the callee pushes and the caller pops — so two paths
/// returning different counts is a stack that unbalances at runtime with NSIS
/// reporting nothing at all (§3).
#[test]
fn returns_must_agree_on_how_many() {
    let mut diags = Diagnostics::new();
    installua::compile(
        "attributes { outFile = \"a.exe\" }\n\
         func(\"maybe\", function()\n\
         if fileExists(\"x\") then return 1 end\n\
         return 2, 3\n\
         end)\n\
         installer { section(\"Core\", function() maybe() end), }",
        &mut diags,
    );

    assert!(diags.contains(Code::ReturnArity));
}

/// A parameter's type comes from the call sites, because there are no
/// annotations to read (§15.14). `n - 1` has no lowering until `countdown(4)`
/// has been seen, and the two are in either order.
#[test]
fn a_parameter_is_typed_by_its_call_sites() {
    let module = module(
        "attributes { outFile = \"a.exe\" }\n\
         installer { section(\"Core\", function() local x = double(21) detailPrint(\"x \" .. x) end), }\n\
         func(\"double\", function(n) return n * 2 end)",
    );

    // An `IntOp` at all means the lattice landed on `int`: a `string` there is
    // a hard error rather than a `StrCpy`.
    assert_eq!(
        shape(function(&module, "double")),
        vec!["Pop", "IntOp", "Push"]
    );
}
