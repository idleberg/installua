//! Install types at run time: §13 applied to a list instead of a section.
//!
//! An install type's identity is its position in the block's `installTypes`
//! list, and every one of these tests is really the same assertion: the number
//! is the compiler's, and the source says the name. What makes the read the
//! interesting one is that the mapping runs *backwards* — a position NSIS
//! chose, turned back into a name the script can compare — and it is built out
//! of comparisons rather than `InstTypeGetText`, because `instTypes.setText`
//! exists and `currentInstType == "Full"` has to keep working after it is
//! called.

use installua::diag::{Code, Diagnostics};

fn build(source: &str) -> String {
    let mut diags = Diagnostics::new();
    let output = installua::build(source, &mut diags);
    assert!(diags.is_empty(), "{}", diags.render("<test>"));
    output.expect("compiles")
}

fn errors(source: &str) -> Vec<(Code, String)> {
    let mut diags = Diagnostics::new();
    installua::build(source, &mut diags);
    diags.iter().map(|d| (d.code, d.message.clone())).collect()
}

/// A program whose `.onInit` runs `body`, in a block declaring two types.
fn typed(body: &str) -> String {
    format!(
        "attributes {{ outFile = \"a.exe\", name = \"a\" }}\n\
         local core = section(\"Core\", function() end)\n\
         installer {{\n\
           installTypes = {{ \"Full\", \"Minimal\" }},\n\
           core,\n\
           onInit(function() {body} end),\n\
         }}\n"
    )
}

/// The write is one instruction, and the `1` in it appears nowhere in the
/// source.
#[test]
fn writing_the_current_type_resolves_the_name_to_a_position() {
    let output = build(&typed("currentInstType = \"Minimal\""));
    assert!(output.contains("SetCurInstType 1"), "{output}");
}

/// The read is `GetCurInstType` and the chain that turns its answer back into a
/// name — one comparison per declared type, in declaration order.
#[test]
fn reading_the_current_type_maps_the_position_back_to_a_name() {
    let output = build(&typed(
        "local chosen = currentInstType\ndetailPrint(chosen)",
    ));
    assert!(output.contains("GetCurInstType $"), "{output}");
    assert!(output.contains("IntCmpU $0 0"), "{output}");
    assert!(output.contains("StrCpy $0 \"Full\""), "{output}");
    assert!(output.contains("IntCmpU $0 1"), "{output}");
    assert!(output.contains("StrCpy $0 \"Minimal\""), "{output}");
    // The custom type, which every list has and no list declares.
    assert!(output.contains("StrCpy $0 \"\""), "{output}");
    // Not `InstTypeGetText`: that is the label, which a script may change.
    assert!(!output.contains("InstTypeGetText"), "{output}");
}

/// Both halves of the table, and both take a name.
#[test]
fn the_label_is_read_and_written_by_name() {
    let output = build(&typed(
        "instTypes.setText(\"Minimal\", \"Just the app\")\n\
         local label = instTypes.getText(\"Full\")\n\
         detailPrint(label)",
    ));
    assert!(
        output.contains("InstTypeSetText 1 \"Just the app\""),
        "{output}"
    );
    assert!(output.contains("InstTypeGetText 0 $"), "{output}");
}

/// The whole point of naming rather than numbering: one misspelling, one error,
/// and the names it could have been.
#[test]
fn a_name_that_is_not_declared_lists_the_ones_that_are() {
    let raised = errors(&typed("currentInstType = \"Tiny\""));
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].0, Code::UnknownField);
    assert!(raised[0].1.contains("is not an install type"), "{raised:?}");

    let raised = errors(&typed("instTypes.setText(\"Tiny\", \"x\")"));
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].0, Code::UnknownField);
}

/// The position is resolved at compile time, so the name has to be too.
#[test]
fn a_runtime_name_is_an_error_rather_than_a_lookup() {
    let raised = errors(&typed("local wanted = \"Full\"\ncurrentInstType = wanted"));
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].0, Code::BadFieldValue);
    assert!(
        raised[0].1.contains("wants a name the block declared"),
        "{raised:?}"
    );
}

/// `instTypes` is a table the compiler owns, not a value and not a slot — which
/// is the point of it not being a global: an unbound assignment target would
/// otherwise become a `Var` and swallow this silently.
#[test]
fn the_table_is_neither_a_value_nor_a_target() {
    let raised = errors(&typed("local all = instTypes"));
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert!(raised[0].1.contains("is not a value"), "{raised:?}");

    let raised = errors(&typed("instTypes = 3"));
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert!(raised[0].1.contains("cannot be assigned to"), "{raised:?}");
}

/// With no types declared the read could only ever answer `""`, so it is said
/// rather than compiled.
#[test]
fn reading_the_current_type_needs_a_declared_list() {
    let raised = errors(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         local core = section(\"Core\", function() end)\n\
         installer { core, onInit(function() detailPrint(currentInstType) end) }\n",
    );
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert!(
        raised[0].1.contains("declares no install types"),
        "{raised:?}"
    );
}

/// The uninstaller declares its own list — `InstType un.` — and its names are
/// the ones its bodies may write.
#[test]
fn the_uninstaller_resolves_against_its_own_list() {
    let output = build(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         local core = section(\"Core\", function() end)\n\
         local removal = section(\"Remove\", function() end)\n\
         installer { installTypes = { \"Full\" }, core }\n\
         uninstaller {\n\
           installTypes = { \"Keep data\", \"Everything\" },\n\
           removal,\n\
           onInit(function() currentInstType = \"Everything\" end),\n\
         }\n",
    );
    assert!(output.contains("SetCurInstType 1"), "{output}");

    let raised = errors(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         local core = section(\"Core\", function() end)\n\
         local removal = section(\"Remove\", function() end)\n\
         installer { installTypes = { \"Full\" }, core }\n\
         uninstaller { removal, onInit(function() currentInstType = \"Full\" end) }\n",
    );
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].0, Code::UnknownField);
}

/// A call is a call: the arities are checked and the message spells both forms.
#[test]
fn the_table_s_two_methods_have_arities() {
    let raised = errors(&typed("instTypes.setText(\"Full\")"));
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].0, Code::WrongArity);
    assert!(raised[0].1.contains("takes 2 argument(s)"), "{raised:?}");
}
