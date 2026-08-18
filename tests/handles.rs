//! Section handles: the install-time half of §13's binding.
//!
//! [`tests/components.rs`](components.rs) covers the compile-time half — a
//! section listed by a block, addressed through a define derived from its
//! `local`. What is here is what that define buys: `core.selected` is a
//! `SectionGetFlags` and the bit test after it, and the number NSIS reads
//! appears nowhere in the source.
//!
//! Every assertion below is about an instruction sequence rather than a
//! substring of one line, because the read-modify-write is the whole feature:
//! `Sections.nsh` spells it as a macro that clobbers `$0`, and a compiler with a
//! register allocator has no business including that header.

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
    diags
        .iter()
        .map(|d| (d.code, d.message.clone()))
        .collect::<Vec<_>>()
}

/// A program whose `.onInit` addresses a section the block lists.
fn addressing(body: &str) -> String {
    format!(
        "attributes {{ outFile = \"a.exe\", name = \"a\" }}\n\
         local core = section(\"Core\", function() end)\n\
         local profiler = section(\"Profiler\", function() end)\n\
         local tools = group {{ \"Tools\", sections = {{ profiler }} }}\n\
         installer {{ core, tools, onInit(function() {body} end) }}\n"
    )
}

/// `SF_SELECTED` is bit 0, so the read is one mask and the write is the three
/// instructions ruling 6 promised: read, or, write.
#[test]
fn a_flag_is_read_as_a_bool_and_written_through_its_word() {
    let output = build(&addressing("core.selected = true"));
    assert!(
        output.contains("SectionGetFlags ${SEC_core} $0"),
        "{output}"
    );
    assert!(output.contains("IntOp $0 $0 | 1"), "{output}");
    assert!(
        output.contains("SectionSetFlags ${SEC_core} $0"),
        "{output}"
    );
}

/// Clearing costs one line more than setting, because NSIS's `~` is unary and
/// takes a whole instruction to produce the mask.
#[test]
fn clearing_a_flag_inverts_the_bit_first() {
    let output = build(&addressing("core.selected = false"));
    assert!(output.contains("IntOp $1 1 ~"), "{output}");
    assert!(output.contains("IntOp $0 $0 & $1"), "{output}");
}

/// `SF_RO` is 16, and a `bool` in this compiler is `0` or `1` and nothing else
/// (§15.20) — so a high bit is shifted down before it is masked, rather than
/// handed back as a truth value that happens to be non-zero.
#[test]
fn a_high_bit_normalises_before_it_is_a_bool() {
    let output = build(&addressing("locked = core.readOnly"));
    assert!(output.contains("SectionGetFlags ${SEC_core} $"), "{output}");
    assert!(output.contains(">>> 4"), "{output}");
    assert!(output.contains("& 1"), "{output}");
}

/// A runtime value cannot be or-ed straight in: the bit is cleared, the value
/// masked to `0`/`1`, shifted to the bit's position, and or-ed back.
#[test]
fn a_flag_written_from_a_value_clears_then_sets() {
    let output = build(&addressing("tools.expanded = core.selected"));
    assert!(output.contains("SectionGetFlags ${SEC_core}"), "{output}");
    assert!(output.contains("32 ~"), "{output}");
    assert!(output.contains("<< 5"), "{output}");
    assert!(output.contains("SectionSetFlags ${SEC_tools}"), "{output}");
}

/// `text` and `size` are their own pairs, and neither touches the flags word.
#[test]
fn text_and_size_are_their_own_instruction_pairs() {
    let output = build(&addressing("core.text = \"\"\ncore.size = 4096"));
    assert!(
        output.contains("SectionSetText ${SEC_core} \"\""),
        "{output}"
    );
    assert!(
        output.contains("SectionSetSize ${SEC_core} 4096"),
        "{output}"
    );
    assert!(!output.contains("SectionSetFlags"), "{output}");

    let output = build(&addressing("label = core.text"));
    assert!(
        output.contains("SectionGetText ${SEC_core} $label"),
        "{output}"
    );
}

/// The install types are named and never numbered here too — the write is a bit
/// field, and bit 0 is the first type the block declared.
#[test]
fn install_types_are_written_as_a_bit_field() {
    let source = "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         local core = section(\"Core\", function() end)\n\
         installer {\n\
           installTypes = { \"Full\", \"Minimal\" },\n\
           core,\n\
           onInit(function() core.installTypes = { \"Minimal\" } end),\n\
         }\n";
    let output = build(source);
    assert!(
        output.contains("SectionSetInstTypes ${SEC_core} 2"),
        "{output}"
    );
}

/// Claim rule 4, and the only one of the four about a *use*. One `.nsi` holds
/// both halves, so `${SEC_core}` in `un.onInit` compiles and addresses a section
/// the uninstaller does not contain — which is why the compiler has to say it.
#[test]
fn a_handle_from_the_other_half_is_an_error() {
    let raised = errors(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         local core = section(\"Core\", function() end)\n\
         local removal = section(\"Core\", function() end)\n\
         installer { core }\n\
         uninstaller { removal, onInit(function() core.selected = false end) }\n",
    );
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].0, Code::UnknownField);
    assert!(raised[0].1.contains("is a `section` of the"), "{raised:?}");
}

/// `expanded` is the one field that is not on every handle, and the handle knows
/// which it is.
#[test]
fn expanded_is_a_groups_field_and_size_is_a_sections() {
    let raised = errors(&addressing("core.expanded = true"));
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert!(raised[0].1.contains("is a `group`'s field"), "{raised:?}");

    let raised = errors(&addressing("tools.size = 10"));
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert!(raised[0].1.contains("is a `section`'s field"), "{raised:?}");
}

/// A misspelled field lists the seven, which is the whole gain a closed set of
/// names buys over a shrug.
#[test]
fn a_field_that_is_not_one_names_the_seven() {
    let raised = errors(&addressing("core.selcted = true"));
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].0, Code::UnknownField);
    assert!(
        raised[0].1.contains("is not a field of a section"),
        "{raised:?}"
    );
}

/// A flag is one bit, so there is no third value to assign to it.
#[test]
fn a_flag_takes_a_bool() {
    let raised = errors(&addressing("core.selected = 3"));
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].0, Code::TypeConflict);
    assert!(raised[0].1.contains("is a `bool`"), "{raised:?}");
}

/// A `func` belongs to no half — either may call it — so rule 4 has nothing to
/// compare against and does not run.
#[test]
fn a_func_may_address_either_halfs_sections() {
    let output = build(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         local core = section(\"Core\", function() end)\n\
         func(\"tick\", function() core.selected = true end)\n\
         installer { core, onInit(function() tick() end) }\n",
    );
    assert!(output.contains("SectionGetFlags ${SEC_core}"), "{output}");
}
