//! The diagnostic registry, as a test rather than a promise.
//!
//! PLAN §2: *every diagnostic code has a test that produces it, enforced by a
//! registry-walking test, and every rejection names its replacement.* All three
//! are checked here, and the table is the mechanism — a code added to
//! `Code::ALL` without a case in `CASES` fails the build, so omission is
//! unrepresentable (§14).

use installua::diag::{Code, Diagnostics, Severity};

/// One source per code, each the smallest thing that raises it.
///
/// A case may raise *other* codes too — `detailPrint` at the top level is not a
/// declaration, so the `dollar-in-literal` case also collects a
/// `not-yet-implemented`. That is fine and deliberately not asserted away:
/// pinning the exact diagnostic set of every snippet would make this table a
/// second golden file with none of the leverage.
const CASES: &[(Code, &str)] = &[
    (Code::ParseError, "attributes {"),
    (Code::FloatLiteral, "local x = 1.5"),
    (Code::InvalidEscape, r#"local p = "C:\Program Files""#),
    (Code::DollarInLiteral, r#"detailPrint("into $INSTDIR")"#),
    (Code::OverlongLiteral, OVERLONG),
    (Code::FloatDivision, "local x = 1 / 2"),
    (Code::Exponentiation, "local x = 2 ^ 8"),
    (Code::LengthOperator, "local n = #name"),
    (Code::Goto, "::done::\ngoto done"),
    (Code::NilValue, "local x = nil"),
    (Code::Varargs, "local x = ..."),
    (Code::RepeatLoop, "repeat until true"),
    (Code::FunctionStatement, "function f() end"),
    (Code::ClosureValue, "local f = function() end"),
    (Code::CloseAttribute, "local f <close> = handle"),
    (Code::UnknownAttribute, "local x <mutable> = 1"),
    (Code::IndexExpression, "local x = pages[1]"),
    (Code::UnsupportedIterator, "for k, v in pairs(t) do end"),
    (Code::RuntimeRequire, r#"require("WinVer")"#),
    (Code::NotYetImplemented, "uninstaller {}"),
    (Code::UnknownField, r#"attributes { nope = 1 }"#),
    (Code::BadFieldValue, r#"attributes { unicode = "yes" }"#),
    (
        Code::DuplicateBlock,
        "attributes { outFile = \"a.exe\" }\nattributes { name = \"b\" }",
    ),
    (Code::MissingAttribute, r#"attributes { name = "Spine" }"#),
];

/// 1100 characters: verified in Phase 0 to compile clean under `-WX` and
/// measure 1023 at runtime, with no `makensis` diagnostic at any point.
const OVERLONG: &str = concat!(
    r#"detailPrint(""#,
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    r#"")"#,
);

fn compile(source: &str) -> Diagnostics {
    let mut diags = Diagnostics::new();
    installua::build(source, &mut diags);
    diags
}

#[test]
fn every_code_has_a_case() {
    let missing: Vec<&str> = Code::ALL
        .iter()
        .filter(|code| !CASES.iter().any(|(cased, _)| cased == *code))
        .map(|code| code.slug())
        .collect();

    assert!(
        missing.is_empty(),
        "these codes have no case in CASES, so nobody has read their wording: {missing:?}"
    );
}

#[test]
fn every_case_raises_its_code() {
    for (code, source) in CASES {
        let diags = compile(source);
        assert!(
            diags.contains(*code),
            "`{}` was not raised by its own case; got {:?}",
            code.slug(),
            diags.iter().map(|d| d.code.slug()).collect::<Vec<_>>()
        );
    }
}

/// PLAN §2: *every rejection names its replacement.* A diagnostic that says
/// only "not supported" sends the user back to the NSIS documentation, which is
/// the thing this compiler exists to stand in front of.
#[test]
fn every_rejection_names_its_replacement() {
    for (code, source) in CASES {
        if matches!(*code, Code::ParseError) {
            // `full-moon` writes these, and its wording is its own.
            continue;
        }

        let diags = compile(source);
        for diagnostic in diags.iter().filter(|d| d.code == *code) {
            assert!(
                !diagnostic.notes.is_empty(),
                "`{}` was raised with no note explaining what to write instead",
                code.slug()
            );
        }
    }
}

/// Collect-don't-throw is only observable when more than one error exists
/// (PLAN §2). Three unrelated mistakes, one run, three diagnostics.
#[test]
fn errors_are_collected_not_thrown() {
    let diags = compile(
        "\
local a = 1 / 2
local b = 2 ^ 8
local c = nil
",
    );

    let codes: Vec<&str> = diags.iter().map(|d| d.code.slug()).collect();
    assert_eq!(
        codes,
        vec!["float-division", "exponentiation", "nil-value"],
        "all three errors should be reported, in source order"
    );
}

/// The two literal checks are warnings rather than errors: both describe output
/// that is legal and probably not what was meant, and neither blocks lowering.
#[test]
fn literal_checks_are_warnings() {
    for code in [Code::DollarInLiteral, Code::OverlongLiteral] {
        let (_, source) = CASES
            .iter()
            .find(|(cased, _)| *cased == code)
            .expect("covered by every_code_has_a_case");
        let diags = compile(source);
        let severity = diags
            .iter()
            .find(|d| d.code == code)
            .map(|d| d.severity)
            .expect("covered by every_case_raises_its_code");
        assert_eq!(severity, Severity::Warning, "{}", code.slug());
    }
}
