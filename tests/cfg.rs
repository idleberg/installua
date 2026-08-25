//! Phase 2 at the pass boundary (tier 0): resolution, the type lattice, and the
//! CFG — asserted as properties of the IR rather than read out of emitted text.
//!
//! The distinction is the point. "There is no `StrCpy` in the output" is an
//! argument from absence, and it passes just as happily when the compiler
//! emitted nothing at all. "This body allocated zero temporaries" is a claim
//! about the thing that was actually decided.

use installua::cfg::{Arm, Body, CmpOp, Terminator, Test};
use installua::diag::{Code, Diagnostics};
use installua::ir;
use installua::layout;

fn module(source: &str) -> ir::Module {
    let mut diags = Diagnostics::new();
    let module = installua::compile(source, &mut diags);
    assert!(diags.is_empty(), "{}", diags.render("<test>"));
    module.expect("compiles")
}

/// The first section's body. Every test here is about one body, because a CFG
/// is per-body and NSIS `Goto` cannot cross the boundary anyway.
fn body(source: &str) -> Body {
    let module = module(source);
    let (_, body) = module.bodies().next().expect("one body");
    body.clone()
}

fn program(statements: &str) -> String {
    format!(
        "attributes {{ outFile = \"a.exe\" }}\n\
         installer {{ section(\"Core\", function()\n{statements}\nend), }}\n"
    )
}

/// The laid-out body as a list of instruction names, with labels as `name:`.
/// Coarse enough to read and exact enough to fail on a spurious line — which
/// `assert!(out.contains(…))` is not.
fn shape(body: &Body) -> Vec<String> {
    layout::lay_out(body)
        .into_iter()
        .map(|(item, _)| match item {
            ir::Item::Instruction(instruction) => instruction.name,
            ir::Item::Label(label) => format!("{label}:"),
        })
        .collect()
}

/// **The Phase 2 exit criterion.**
///
/// `a and (b or not c)` allocates zero temporaries. Three compare-and-jumps and
/// nothing else: `and` and `or` recurse, `not` swaps its two destinations, and
/// no boolean is ever materialised into a register. Evaluating the expression
/// instead would burn registers there are only twenty of, which is nsL's issue
/// #5.
#[test]
fn a_fused_condition_allocates_no_temporaries() {
    let body = body(&program(
        "\
local a = fileExists(\"x\")
local b = fileExists(\"y\")
local c = fileExists(\"z\")
if a and (b or not c) then
	detailPrint(\"yes\")
end",
    ));

    assert_eq!(body.temps, 0, "the condition materialised something");

    // And structurally: the three tests the condition became are *only* tests.
    // A block carrying an instruction alongside its branch would mean something
    // was computed on the way in.
    let tests: Vec<&installua::cfg::BasicBlock> = body
        .blocks
        .iter()
        .filter(|block| {
            matches!(
                &block.terminator,
                Terminator::Branch {
                    test: Test::Str { .. },
                    ..
                }
            )
        })
        .collect();

    assert_eq!(tests.len(), 3, "one compare-and-jump per operand");
    for block in tests {
        assert!(
            block.steps.is_empty(),
            "`{}` computes something before branching: {:?}",
            block.label,
            block.steps
        );
    }
}

/// A metric that is always zero measures nothing. Something has to be able to
/// move it, or the test above passes on a compiler that allocates no registers
/// because it emits nothing.
#[test]
fn a_nested_expression_does_allocate_one() {
    let body = body(&program(
        "\
local path = INSTDIR
detailPrint(\"length \" .. string.len(path))",
    ));

    assert_eq!(body.temps, 1);
}

/// `IntCmp a b <eq> <lt> <gt>` covers all six relational operators in one
/// instruction with no temporaries — the payoff of "no materialised booleans",
/// and not obvious from the NSIS documentation, so it is written down as a
/// table. This is that table.
#[test]
fn int_cmp_fuses_all_six_comparisons() {
    use Arm::{Else, Then};
    assert_eq!(CmpOp::Eq.arms(), [Then, Else, Else]);
    assert_eq!(CmpOp::Ne.arms(), [Else, Then, Then]);
    assert_eq!(CmpOp::Lt.arms(), [Else, Then, Else]);
    assert_eq!(CmpOp::Le.arms(), [Then, Then, Else]);
    assert_eq!(CmpOp::Gt.arms(), [Else, Else, Then]);
    assert_eq!(CmpOp::Ge.arms(), [Then, Else, Then]);
}

/// A label is emitted only for a block something jumps to, and a `Goto` to the
/// next line is never emitted. An `if` with no `else` therefore costs one label
/// and no jump at all.
#[test]
fn an_if_with_no_else_costs_one_label_and_no_goto() {
    let body = body(&program(
        "\
local ok = fileExists(\"x\")
if ok then
	detailPrint(\"yes\")
end
detailPrint(\"after\")",
    ));

    assert_eq!(
        shape(&body),
        vec![
            "IfFileExists",
            "StrCpy",
            "Goto",
            "__GENERATED_false_0:",
            "StrCpy",
            "__GENERATED_bool_0:",
            "StrCmpS",
            "DetailPrint",
            "__GENERATED_endif_1:",
            "DetailPrint",
        ]
    );
}

/// The label counter resets per body, so inserting an `if` in one section
/// renumbers labels in that section only — which is what keeps the whole-file
/// goldens diffable. Both bodies here start at zero.
#[test]
fn the_label_counter_resets_per_body() {
    let module = module(
        "\
attributes { outFile = \"a.exe\" }
installer {
	section(\"One\", function()
		if fileExists(\"x\") then detailPrint(\"a\") end
	end),
	section(\"Two\", function()
		if fileExists(\"y\") then detailPrint(\"b\") end
	end),
}
",
    );

    let labels: Vec<Vec<String>> = module
        .bodies()
        .map(|(_, body)| {
            shape(body)
                .into_iter()
                .filter(|line| line.ends_with(':'))
                .collect()
        })
        .collect();

    assert_eq!(
        labels,
        vec![vec!["__GENERATED_endif_0:"], vec!["__GENERATED_endif_0:"]]
    );
}

/// Dead blocks disappear, and a `<const>` condition never becomes a branch at
/// all — which is also why `!if`/`!ifdef` need no surface spelling. Nothing
/// from the untaken arm reaches the `.nsi`.
#[test]
fn a_const_condition_folds_away_entirely() {
    let body = body(&program(
        "\
local DEBUG <const> = false
if DEBUG then
	detailPrint(\"noisy\")
end
detailPrint(\"always\")",
    ));

    assert_eq!(shape(&body), vec!["DetailPrint"]);
}

/// Resolution is order-free: a section can call a `func` declared below it. Lua
/// does not hoist, and Installua has to, because there is no build-time
/// execution for an ordering rule to be about.
#[test]
fn a_section_calls_a_func_declared_below_it() {
    let module = module(
        "\
installer {
	section(\"Core\", function()
		helper()
	end),
}

func(\"helper\", function()
	detailPrint(\"from the helper\")
end)

attributes { outFile = \"a.exe\" }
",
    );

    assert_eq!(module.functions.len(), 1);
    assert_eq!(module.functions[0].name, "helper");

    let (_, section) = module
        .bodies()
        .find(|(name, _)| *name == "Core")
        .expect("the section");
    assert_eq!(shape(section), vec!["Call"]);
}

/// The sign lattice earns its place by *eliding* work. `StrLen` is non-negative
/// by construction, so `//` on it is one `IntOp` with no fixup — NSIS truncates
/// toward zero and Lua floors, and they only disagree when exactly one operand
/// is negative.
#[test]
fn a_non_negative_division_needs_no_fixup() {
    let body = body(&program(
        "\
local n = string.len(INSTDIR)
local half = n // 2",
    ));

    assert_eq!(shape(&body), vec!["StrLen", "IntOp"]);
}

/// And when the lattice cannot rule out a negative, the fixup is emitted rather
/// than hoped about. Subtraction is the operator that manufactures one.
#[test]
fn an_unknown_sign_division_carries_the_fixup() {
    let body = body(&program(
        "\
local n = string.len(INSTDIR) - 5
local half = n // 2",
    ));

    assert_eq!(
        shape(&body),
        vec![
            "StrLen",
            "IntOp",  // n = len - 5
            "IntOp",  // quotient
            "IntOp",  // remainder
            "IntCmp", // remainder == 0 ?
            "IntOp",  // remainder ^ divisor -- sign bit set iff they disagree
            "IntCmp",
            // The adjustment falls through rather than being jumped to, so it
            // needs no label of its own.
            "IntOp", // quotient - 1
            "__GENERATED_div_0_done:",
            "StrCpy",
        ]
    );
}

/// `==` on strings is `StrCmpS`. Case-sensitive being the *default* is the
/// reversal from NSIS habit that will bite hardest, and `string.lower` is the
/// escape.
#[test]
fn string_equality_is_case_sensitive() {
    let body = body(&program(
        "\
local a = \"one\"
if a == \"ONE\" then detailPrint(\"same\") end",
    ));

    assert_eq!(
        shape(&body),
        vec!["StrCpy", "StrCmpS", "DetailPrint", "__GENERATED_endif_0:"]
    );
}

/// A predicate fuses straight into its branching instruction and spends no
/// register — the case that dissolved a whole second condition shape.
#[test]
fn a_predicate_in_a_condition_spends_no_register() {
    let body = body(&program(
        "if fileExists(INSTDIR) then detailPrint(\"there\") end",
    ));

    assert_eq!(body.temps, 0);
    assert_eq!(
        shape(&body),
        vec!["IfFileExists", "DetailPrint", "__GENERATED_endif_0:"]
    );
}

// -- rejections ------------------------------------------------------------

fn errors(source: &str) -> Diagnostics {
    let mut diags = Diagnostics::new();
    installua::compile(source, &mut diags);
    diags
}

/// Truthiness is `bool` and nothing else. The tempting by-type rule — `int` →
/// `~= 0` — contradicts Lua on `0`, which is the value a reader is most likely
/// to test, and `lua-language-server` reports nothing either way.
#[test]
fn only_a_bool_is_truthy() {
    let diags = errors(&program(
        "local count = 0\nif count then detailPrint(\"x\") end",
    ));
    assert!(diags.contains(Code::NotBool));

    let clean = errors(&program(
        "local ok = fileExists(\"x\")\nif ok then detailPrint(\"y\") end",
    ));
    assert!(clean.is_empty(), "{}", clean.render("<test>"));
}

/// A global has one type for its lifetime, because a `Var` is one slot.
#[test]
fn a_global_has_one_type() {
    let diags = errors(&program("state = \"fresh\"\nstate = 1"));
    assert!(diags.contains(Code::TypeConflict));
}

/// A comparison whose operands disagree is a hard error naming both, rather
/// than a `StrCmp` that makes `"10" < "9"` true and `10 < 9` false with NSIS
/// objecting to neither.
#[test]
fn a_mixed_comparison_names_both_operands() {
    let diags = errors(&program(
        "local n = 1\nlocal s = \"one\"\nif n == s then detailPrint(\"x\") end",
    ));

    let message = diags
        .iter()
        .find(|d| d.code == Code::TypeMismatch)
        .map(|d| d.message.clone())
        .expect("a type mismatch");
    assert!(
        message.contains("int") && message.contains("string"),
        "the message names neither operand: {message}"
    );
}
