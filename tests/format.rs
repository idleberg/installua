//! `string.format`, against what `IntFmt` can actually perform.
//!
//! The reason this is a file of its own rather than a row in the overlay
//! goldens: `IntFmt` is a `lowering` row, so it has no mandatory example pair,
//! and the failure mode is not a wrong line but an accepted one. `IntFmt $0
//! "%o" 255` and `IntFmt $0 "%s" 255` both build clean under `makensis -WX` on
//! 3.12 — verified — and print a literal `o` and a pointer respectively. NSIS
//! will never object, so a test that only assembles proves nothing here.
//!
//! `Source/exehead/exec.c` is `wsprintf(var0, buf0, val)`: Windows `wsprintf`,
//! exactly one argument. That is the whole specification the accepted set comes
//! from, and the rejected cases below are the three ways to leave it: a
//! conversion outside the set, a count that is not one, and a `%` naming none.

use installua::diag::{Code, Diagnostics};

fn program(statements: &str) -> String {
    format!(
        "attributes {{ outFile = \"a.exe\" }}\n\
         installer {{ section(\"Core\", function()\n{statements}\nend), }}\n"
    )
}

fn build(statements: &str) -> String {
    let mut diags = Diagnostics::new();
    let output = installua::build(&program(statements), &mut diags);
    assert!(diags.is_empty(), "{}", diags.render("<test>"));
    output.expect("it compiles")
}

/// The codes a format raises, so a case asserts the diagnostic rather than
/// merely that something went wrong.
fn codes(statements: &str) -> Vec<Code> {
    let mut diags = Diagnostics::new();
    let _ = installua::build(&program(statements), &mut diags);
    diags.iter().map(|d| d.code).collect()
}

/// Tier 2: the accepted case emits one line, and it is this line.
#[test]
fn a_conversion_intfmt_performs_lowers_to_one_line() {
    let nsi = build("\tdetailPrint(string.format(\"%04d\", 42))");
    assert!(
        nsi.lines()
            .any(|line| line.trim() == r#"IntFmt $0 "%04d" 42"#),
        "{nsi}"
    );
}

/// Flags, width, precision and a literal per cent all sit between the `%` and
/// the letter without adding an argument, so none of them is a second
/// conversion.
#[test]
fn padding_and_a_literal_per_cent_are_not_conversions() {
    for format in ["%x", "%X", "%u", "%i", "%c", "%-8d", "%#x", "%.3d", "%d%%"] {
        assert_eq!(
            codes(&format!("\tdetailPrint(string.format(\"{format}\", 42))")),
            Vec::<Code>::new(),
            "`{format}` should be one conversion"
        );
    }
}

/// The two `wsprintf` will not do with a number: `%o` is not a conversion at
/// all and prints as a literal `o`, and `%s` reads the integer as a pointer.
/// Both are what the language exists to refuse.
#[test]
fn a_conversion_outside_wsprintfs_set_is_rejected() {
    for format in ["%o", "%s", "%S", "%f", "%e", "%p"] {
        assert_eq!(
            codes(&format!("\tdetailPrint(string.format(\"{format}\", 42))")),
            vec![Code::FormatString],
            "`{format}` should be rejected"
        );
    }
}

/// `IntFmt` passes a `UINT`. Reading it as 64 bits is `Int64Fmt`, which is a
/// different instruction, so `%I64d` is rejected rather than silently narrowed.
#[test]
fn the_64_bit_size_prefix_is_not_this_instruction() {
    assert_eq!(
        codes("\tdetailPrint(string.format(\"%I64d\", 42))"),
        vec![Code::FormatString]
    );
}

/// One argument is pushed, so zero conversions drops it and two read past it.
#[test]
fn the_count_has_to_be_exactly_one() {
    for format in ["plain", "%d and %d", "%%"] {
        assert_eq!(
            codes(&format!("\tdetailPrint(string.format(\"{format}\", 42))")),
            vec![Code::FormatString],
            "`{format}` should be rejected"
        );
    }
}

/// A trailing `%` names no conversion and `wsprintf` reads past the end of the
/// string for one.
#[test]
fn a_dangling_per_cent_is_rejected() {
    assert_eq!(
        codes("\tdetailPrint(string.format(\"count: %\", 42))"),
        vec![Code::FormatString]
    );
}

/// The format has to be readable to be checked, and a computed one is not.
/// `todo` rather than an error: this is the honest edge of the check, not a
/// mistake in the program.
#[test]
fn a_computed_format_is_not_yet_implemented() {
    assert_eq!(
        codes(
            "\tlocal width = \"%04d\"\n\
             \tdetailPrint(string.format(width, 42))"
        ),
        vec![Code::NotYetImplemented]
    );
}
