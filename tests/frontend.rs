//! Frontend pass boundaries (§14 tier 0) and the Phase 1 exit criterion:
//! `installua check` clean on all five of Phase 0's programs.

use std::path::{Path, PathBuf};

use installua::ast::{Block, Expr, Stmt};
use installua::diag::Diagnostics;

fn examples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples")
}

/// **The Phase 1 exit criterion.** Not "no errors" but *no diagnostics at all*:
/// §14's rule is that warnings are failures, and the five programs are the
/// specification, so a warning on one of them means either the program or the
/// check is wrong.
#[test]
fn the_five_programs_check_clean() {
    let programs = [
        "01-mui-uninstaller",
        "02-plugins",
        "03-file-iteration",
        "04-multiple-returns",
        "05-strings-and-ints",
    ];

    for program in programs {
        let path = examples().join(program).join("install.lua");
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));

        let mut diags = Diagnostics::new();
        let checked = installua::check(&source, &mut diags);

        assert!(
            diags.is_empty(),
            "{program} should check clean:\n{}",
            diags.render(program)
        );
        assert!(checked.is_some(), "{program} should produce a tree");
    }
}

/// A whitelist that accepts everything is not a whitelist. The five programs
/// passing is only evidence once something is known to fail.
#[test]
fn the_whitelist_rejects() {
    let mut diags = Diagnostics::new();
    installua::check("for k, v in pairs(t) do end", &mut diags);
    assert!(diags.has_errors());
}

fn parse(source: &str) -> Block {
    let mut diags = Diagnostics::new();
    let program = installua::check(source, &mut diags).expect("parses");
    assert!(!diags.has_errors(), "{}", diags.render("<test>"));
    program.block
}

/// The `elseif` desugaring (§7): after the frontend, every `If` has exactly one
/// condition, so the layout pass has one shape to lay out rather than a chain.
#[test]
fn elseif_desugars_into_a_nested_if() {
    let block = parse(
        "\
if a == 1 then
	detailPrint(\"one\")
elseif a == 2 then
	detailPrint(\"two\")
else
	detailPrint(\"other\")
end
",
    );

    let [
        Stmt::If {
            then_block,
            else_block,
            ..
        },
    ] = block.as_slice()
    else {
        panic!("expected a single `if`, got {block:#?}");
    };

    assert_eq!(then_block.len(), 1, "the `then` arm is untouched");

    // The `elseif` is now the whole of the `else` branch.
    let else_block = else_block.as_ref().expect("an else branch exists");
    let [
        Stmt::If {
            then_block: inner_then,
            else_block: inner_else,
            ..
        },
    ] = else_block.as_slice()
    else {
        panic!("the `else` branch should hold exactly one nested `if`, got {else_block:#?}");
    };

    assert_eq!(inner_then.len(), 1);
    assert_eq!(
        inner_else.as_ref().map(Vec::len),
        Some(1),
        "the original `else` is threaded through to the innermost arm"
    );
}

/// A chain with no `else` desugars to a nested `if` with no `else`, rather than
/// to an empty block that a later pass would have to recognise as empty.
#[test]
fn elseif_without_else_leaves_no_empty_block() {
    let block = parse("if a then f() elseif b then g() end");

    let [Stmt::If { else_block, .. }] = block.as_slice() else {
        panic!("expected one `if`");
    };
    let [Stmt::If { else_block, .. }] = else_block.as_ref().expect("nested").as_slice() else {
        panic!("expected a nested `if`");
    };
    assert!(else_block.is_none());
}

/// Escapes are decoded once, in the frontend, so no later pass re-reads a
/// literal — and a long string is decoded not at all (§5).
#[test]
fn escapes_are_decoded_once() {
    let block = parse(r#"local a, b = "one\ttwo", [[C:\Tools]]"#);

    let [Stmt::Local { values, .. }] = block.as_slice() else {
        panic!("expected a local");
    };
    let [Expr::Str(short), Expr::Str(long)] = values.as_slice() else {
        panic!("expected two string literals");
    };

    assert_eq!(short.value, "one\ttwo");
    assert!(!short.long);
    assert_eq!(
        long.value, r"C:\Tools",
        "a long string processes no escapes"
    );
    assert!(long.long);
}

/// `raw` takes NSIS source rather than data, so the `$` check does not run
/// inside it — the sigils in there are the point (§13).
#[test]
fn raw_suppresses_the_dollar_check() {
    let mut diags = Diagnostics::new();
    installua::check("raw [[ Pop $INSTDIR ]]", &mut diags);
    assert!(diags.is_empty(), "{}", diags.render("<test>"));

    let mut diags = Diagnostics::new();
    installua::check(r#"detailPrint("Pop $INSTDIR")"#, &mut diags);
    assert_eq!(diags.len(), 1, "the same text outside `raw` is diagnosed");
}

/// Hexadecimal is an integer; a fraction and an exponent are not. The check is
/// at the literal, since there is no float type for it to be at (§6).
#[test]
fn integer_literals_are_integers() {
    let block = parse("local a, b, c = 42, 0x20, -7");
    let [Stmt::Local { values, .. }] = block.as_slice() else {
        panic!("expected a local");
    };
    assert_eq!(values.len(), 3);

    for source in ["local x = 1.5", "local x = 1e3", "local x = 0x1p4"] {
        let mut diags = Diagnostics::new();
        installua::check(source, &mut diags);
        assert!(diags.has_errors(), "{source} should be rejected");
    }
}
