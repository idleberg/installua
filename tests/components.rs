//! The components tree: install types, groups, and a section's four options.
//!
//! The golden in [`tests/goldens.rs`](goldens.rs) proves the whole thing emits
//! and assembles. What is here is the half a golden cannot show — the binding
//! §13 asked for, and the six ways of writing it that are rejected.
//!
//! An install type has no name at run time. NSIS reads a one-based position and
//! nothing else, so the name a user writes is a *compile-time* name for a
//! number the compiler owns. Every test below is about that seam.

use installua::diag::{Code, Diagnostics};

fn build(source: &str) -> String {
    let mut diags = Diagnostics::new();
    let output = installua::build(source, &mut diags);
    assert!(diags.is_empty(), "{}", diags.render("<test>"));
    output.expect("compiles")
}

/// The diagnostics a source raises, as codes and messages.
fn errors(source: &str) -> Vec<(Code, String)> {
    let mut diags = Diagnostics::new();
    installua::build(source, &mut diags);
    diags
        .iter()
        .map(|d| (d.code, d.message.clone()))
        .collect::<Vec<_>>()
}

fn program(installer: &str) -> String {
    format!("attributes {{ outFile = \"a.exe\", name = \"a\" }}\ninstaller {{ {installer} }}\n")
}

/// The whole of §13's compile-time half: a section names an install type, NSIS
/// reads a position, and inserting a type in front of the list renumbers every
/// section that mentions the ones behind it — without a line of the sections
/// changing.
#[test]
fn an_install_type_is_named_and_never_numbered() {
    let before = build(&program(
        "installTypes = { \"Full\", \"Minimal\" },\n\
         section { \"Core\", installTypes = { \"Minimal\" }, body = function() end },",
    ));
    assert!(before.contains("\n  SectionIn 2\n"), "{before}");

    let after = build(&program(
        "installTypes = { \"Custom\", \"Full\", \"Minimal\" },\n\
         section { \"Core\", installTypes = { \"Minimal\" }, body = function() end },",
    ));
    assert!(after.contains("\n  SectionIn 3\n"), "{after}");
}

/// The uninstaller's install types are its own list under an `un.` prefix, and
/// NSIS numbers the two halves separately — the installer's first type is `1`
/// however many the uninstaller declared.
#[test]
fn each_half_numbers_its_own_install_types() {
    let output = build(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         installer {\n\
         installTypes = { \"Full\" },\n\
         section { \"Core\", installTypes = { \"Full\" }, body = function()\n\
         writeUninstaller(INSTDIR .. \"/un.exe\")\n\
         end },\n\
         }\n\
         uninstaller {\n\
         installTypes = { \"Everything\" },\n\
         section { \"Remove\", installTypes = { \"Everything\" }, body = function() end },\n\
         }\n",
    );
    assert!(output.contains("InstType \"Full\"\n"), "{output}");
    assert!(output.contains("InstType un.\"Everything\"\n"), "{output}");
    assert_eq!(output.matches("SectionIn 1").count(), 2, "{output}");
}

/// `optional` says the box starts unticked; `required` says there is no box.
/// Both is a section that can never be selected and can never be deselected,
/// and NSIS resolves it silently — so one of the two words the author wrote is
/// doing nothing and they should hear which.
#[test]
fn a_section_is_not_both_optional_and_required() {
    let raised = errors(&program(
        "section { \"Core\", optional = true, required = true, body = function() end },",
    ));
    assert!(
        raised
            .iter()
            .any(|(code, text)| *code == Code::BadFieldValue
                && text.contains("both `optional` and `required`")),
        "{raised:?}"
    );

    // Either alone is fine, and they emit in two different places.
    let output = build(&program(
        "section { \"A\", optional = true, body = function() end },\n\
         section { \"B\", required = true, body = function() end },",
    ));
    assert!(output.contains("Section /o \"A\""), "{output}");
    assert!(output.contains("  SectionIn RO\n"), "{output}");
}

/// A name that was never declared is the failure the whole binding exists to
/// catch, and the message has to carry the list — the user is choosing from a
/// set they wrote fifteen lines up.
#[test]
fn an_unknown_install_type_names_the_declared_ones() {
    let raised = errors(&program(
        "installTypes = { \"Full\", \"Minimal\" },\n\
         section { \"Core\", installTypes = { \"Typical\" }, body = function() end },",
    ));
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].0, Code::UnknownField);
    assert!(raised[0].1.contains("`Typical` is not an install type"));

    // With nothing declared at all the note has to say so rather than print an
    // empty list, because the fix is a different one.
    let raised = errors(&program(
        "section { \"Core\", installTypes = { \"Full\" }, body = function() end },",
    ));
    assert_eq!(raised.len(), 1, "{raised:?}");
}

/// The list is a set with an order. A repeat in either place is a mistake NSIS
/// accepts and forgets, which is exactly the kind it should not be left to.
#[test]
fn an_install_type_is_named_once() {
    for written in [
        "installTypes = { \"Full\", \"Full\" }, section(\"C\", function() end),",
        "installTypes = { \"Full\" },\n\
         section { \"C\", installTypes = { \"Full\", \"Full\" }, body = function() end },",
    ] {
        let raised = errors(&program(written));
        assert_eq!(raised.len(), 1, "{written}: {raised:?}");
    }
}

/// A heading with nothing under it is not drawn, so the two NSIS lines it
/// becomes do nothing at all — and one level is what the surface offers, since
/// a nested heading has no separate meaning to anything but the tree.
#[test]
fn a_group_holds_at_least_one_section_and_no_group() {
    let raised = errors(&program("group(\"Tools\", {}),"));
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert!(raised[0].1.contains("no sections"), "{raised:?}");

    let raised = errors(&program(
        "group(\"Tools\", { group(\"Inner\", { section(\"C\", function() end) }) }),",
    ));
    assert!(
        raised
            .iter()
            .any(|(code, _)| *code == Code::NotYetImplemented),
        "{raised:?}"
    );
}

/// `AddSize` is a whole number of kilobytes. A section cannot give space back,
/// and a fraction of a kilobyte is not a thing NSIS can be told.
#[test]
fn a_size_is_a_whole_number_of_kilobytes() {
    let output = build(&program(
        "section { \"C\", size = 0, body = function() end },\n\
         section { \"D\", size = 4096, body = function() end },",
    ));
    // Zero is still written: it says the author measured and got nothing,
    // where leaving the option out says they did not measure.
    assert!(output.contains("  AddSize 0\n"), "{output}");
    assert!(output.contains("  AddSize 4096\n"), "{output}");

    for bad in ["size = -1", "size = \"4096\""] {
        let raised = errors(&program(&format!(
            "section {{ \"C\", {bad}, body = function() end }},"
        )));
        assert!(
            raised.iter().any(|(code, _)| *code == Code::BadFieldValue),
            "{bad}: {raised:?}"
        );
    }
}

/// A section that names no install type belongs to none of them, and that is
/// what writing no `SectionIn` means. The option being absent and the option
/// being an empty list are the same thing, so neither emits a line.
#[test]
fn a_section_in_no_install_type_writes_no_line() {
    let output = build(&program(
        "installTypes = { \"Full\" },\n\
         section(\"Core\", function() end),\n\
         section { \"Extra\", installTypes = {}, body = function() end },",
    ));
    assert!(!output.contains("SectionIn"), "{output}");
}

/// §15.23's pair is a short form and a long form, not two spellings of one
/// thing: the moment a section carries an option it takes the table, and the
/// name stays positional there because it is the parameter NSIS is passed.
#[test]
fn a_section_with_options_takes_the_table_form() {
    let short = build(&program(
        "section(\"Core\", function() detailPrint(\"x\") end),",
    ));
    let table = build(&program(
        "section { \"Core\", body = function() detailPrint(\"x\") end },",
    ));
    assert_eq!(short, table);

    // The middle-table form was the surface's only options-between-parameters
    // call, and it is gone rather than kept beside the pair.
    let raised = errors(&program(
        "section(\"Core\", { optional = true }, function() end),",
    ));
    assert!(
        raised
            .iter()
            .any(|(code, _)| *code == Code::NotYetImplemented),
        "{raised:?}"
    );
}

/// The two halves of the table are the two halves of the NSIS command line, so
/// the name is the array part and everything else is a switch. Each way of
/// getting that wrong is caught where it is written.
#[test]
fn a_table_declaration_has_one_name_and_says_what_it_holds() {
    let raised = errors(&program(
        "section { optional = true, body = function() end },",
    ));
    assert!(
        raised
            .iter()
            .any(|(code, text)| *code == Code::MissingAttribute && text.contains("has no name")),
        "{raised:?}"
    );

    let raised = errors(&program("section { \"Core\", optional = true },"));
    assert!(
        raised
            .iter()
            .any(|(code, text)| *code == Code::MissingAttribute && text.contains("has no `body`")),
        "{raised:?}"
    );

    // A second unnamed entry is the mistake the shape invites: a body written
    // positionally, the way the deleted middle-table form took it.
    let raised = errors(&program(
        "section { \"Core\", function() end, optional = true },",
    ));
    assert!(
        raised
            .iter()
            .any(|(code, text)| *code == Code::BadFieldValue && text.contains("takes one name")),
        "{raised:?}"
    );
}

/// A group holds sections rather than running anything, so its contents key is
/// `sections` and the same three rules apply to it.
#[test]
fn a_group_takes_the_same_table_form() {
    let output = build(&program(
        "group { \"Tools\", expanded = true, sections = { section(\"C\", function() end) } },",
    ));
    assert!(output.contains("SectionGroup /e \"Tools\""), "{output}");

    let raised = errors(&program("group { \"Tools\", expanded = true },"));
    assert!(
        raised
            .iter()
            .any(|(code, text)| *code == Code::MissingAttribute
                && text.contains("has no `sections`")),
        "{raised:?}"
    );
}
