use luis_poc::compile;
use luis_poc::diag::Diagnostics;

fn ok(source: &str) -> String {
    let mut diags = Diagnostics::default();
    let out = compile(source, &mut diags);
    assert!(diags.is_empty(), "{}", diags.render("<test>"));
    out.expect("compilation produced no output")
}

fn errors(source: &str) -> String {
    let mut diags = Diagnostics::default();
    let out = compile(source, &mut diags);
    assert!(out.is_none(), "expected failure, got output");
    assert!(diags.has_errors());
    diags.render("<test>")
}

#[test]
fn demo_matches_golden_file() {
    let source = include_str!("../examples/demo.lua");
    let expected = include_str!("golden/demo.nsi");
    assert_eq!(ok(source), expected);
}

#[test]
fn showcase_matches_golden_file() {
    let source = include_str!("../examples/showcase.lua");
    let expected = include_str!("golden/showcase.nsi");
    assert_eq!(ok(source), expected);
}

#[test]
fn name_gets_an_ampersand_doubled_twin() {
    let out = ok(r#"installer { name = "Ben & Jerry", outFile = "a.exe" }"#);
    assert_eq!(
        out,
        "Name \"Ben & Jerry\" \"Ben && Jerry\"\nOutFile \"a.exe\"\n"
    );
}

#[test]
fn nsis_spelling_is_suggested_for_nsis_casing() {
    let rendered = errors(
        r#"installer { name = "a", outFile = "a.exe" }
section("S", function() DetailPrint("x") end)"#,
    );
    assert!(rendered.contains("unknown instruction `DetailPrint`"));
    assert!(rendered.contains("did you mean `detailPrint`?"));
}

#[test]
fn unsupported_constructs_are_rejected() {
    let rendered = errors(
        r#"installer { name = "a", outFile = "a.exe" }
local x = 1"#,
    );
    assert!(rendered.contains("E002"));
}

/// Wraps a section body so the control-flow tests stay readable.
fn section(body: &str) -> String {
    format!(
        "installer {{ name = \"a\", outFile = \"a.exe\" }}\nsection(\"S\", function()\n{body}\nend)"
    )
}

fn body_of(source: &str) -> String {
    let out = ok(source);
    let start = out.find("Section \"S\"\n").expect("no section") + "Section \"S\"\n".len();
    let end = out.find("SectionEnd").expect("no section end");
    out[start..end].to_string()
}

#[test]
fn an_if_without_an_else_jumps_straight_to_the_end() {
    assert_eq!(
        body_of(&section("if 1 < 2 then detailPrint(\"y\") end")),
        "  IntCmp 1 2 endif_0 0 endif_0\n  DetailPrint \"y\"\nendif_0:\n"
    );
}

#[test]
fn comparisons_fuse_into_intcmp_branch_targets() {
    // `~=` is false exactly when the operands are equal, so only the first
    // target jumps; no boolean is ever materialized.
    assert!(
        body_of(&section("if 1 ~= 2 then detailPrint(\"y\") end"))
            .contains("IntCmp 1 2 endif_0 0 0")
    );
    assert!(
        body_of(&section("if 1 >= 2 then detailPrint(\"y\") end"))
            .contains("IntCmp 1 2 0 endif_0 0")
    );
}

#[test]
fn elseif_is_desugared_into_a_nested_if() {
    let body = body_of(&section(
        r#"if 1 == 2 then detailPrint("a") elseif 1 == 3 then detailPrint("b") else detailPrint("c") end"#,
    ));
    assert_eq!(
        body,
        concat!(
            "  IntCmp 1 2 0 else_0 else_0\n",
            "  DetailPrint \"a\"\n",
            "  Goto endif_0\n",
            "else_0:\n",
            "  IntCmp 1 3 0 else_1 else_1\n",
            "  DetailPrint \"b\"\n",
            "  Goto endif_1\n",
            "else_1:\n",
            "  DetailPrint \"c\"\n",
            "endif_1:\n",
            "endif_0:\n",
        )
    );
}

#[test]
fn locals_take_registers_and_temporaries_come_from_the_other_end() {
    // `1 + 2` and `3 + 4` fold before lowering, so the only `IntOp` left is the
    // one with a register operand.
    assert_eq!(
        body_of(&section(
            "local a = 1 + 2\nlocal b = a * (3 + 4)\ndetailPrint(b)"
        )),
        concat!(
            "  StrCpy $0 3\n",
            "  IntOp $1 $0 * 7\n",
            "  DetailPrint $1\n",
        )
    );

    // A temporary is still needed when both ends of an operator compute.
    assert_eq!(
        body_of(&section(
            "local a = 1\nlocal b = 2\nlocal c = (a + b) * (a - b)\ndetailPrint(c)"
        )),
        concat!(
            "  StrCpy $0 1\n",
            "  StrCpy $1 2\n",
            "  IntOp $R9 $0 + $1\n",
            "  IntOp $R8 $0 - $1\n",
            "  IntOp $2 $R9 * $R8\n",
            "  DetailPrint $2\n",
        )
    );
}

#[test]
fn strings_are_not_arithmetic() {
    let rendered = errors(&section("local a = \"x\" + 1"));
    assert!(rendered.contains("expected an integer, found a string"));
}

#[test]
fn a_condition_must_be_a_comparison() {
    let rendered = errors(&section("local a = 1\nif a then detailPrint(\"y\") end"));
    assert!(rendered.contains("must be a comparison"));
}

#[test]
fn operators_that_lie_are_rejected() {
    // Lua's `/` is float division and `^` is exponentiation; NSIS's are integer
    // division and XOR.
    assert!(errors(&section("local a = 4 / 2")).contains("`/` is not supported"));
    assert!(errors(&section("local a = 4 ^ 2")).contains("`^` is not supported"));
}

#[test]
fn floats_are_rejected_outright() {
    assert!(errors(&section("local a = 1.5")).contains("`1.5` is not an integer"));
}

#[test]
fn locals_do_not_escape_the_branch_that_declared_them() {
    let rendered = errors(&section(
        "if 1 == 1 then local inner = 1 end\ndetailPrint(inner)",
    ));
    assert!(rendered.contains("unknown variable `inner`"));
}

/// Like `ok`, but returns the diagnostics instead of forbidding them.
fn with_warnings(source: &str) -> (String, String) {
    let mut diags = Diagnostics::default();
    let out = compile(source, &mut diags);
    assert!(!diags.has_errors(), "{}", diags.render("<test>"));
    (out.expect("no output"), diags.render("<test>"))
}

#[test]
fn a_function_becomes_a_function_block_and_a_call() {
    let out = ok(concat!(
        "installer { name = \"a\", outFile = \"a.exe\" }\n",
        "section(\"S\", function() greet() end)\n",
        // Declared *after* its use: names are resolved before bodies are lowered.
        "function greet() detailPrint(\"hi\") end\n",
    ));
    assert_eq!(
        out,
        concat!(
            "Name \"a\"\n",
            "OutFile \"a.exe\"\n",
            "\n",
            "Function greet\n",
            "  DetailPrint \"hi\"\n",
            "FunctionEnd\n",
            "\n",
            "Section \"S\"\n",
            "  Call greet\n",
            "SectionEnd\n",
        )
    );
}

#[test]
fn function_parameters_need_a_calling_convention_first() {
    let rendered = errors(concat!(
        "installer { name = \"a\", outFile = \"a.exe\" }\n",
        "function greet(who) detailPrint(who) end\n",
    ));
    assert!(rendered.contains("functions take no parameters yet"));
    assert!(rendered.contains("calling convention"));
}

#[test]
fn a_function_call_produces_no_value() {
    let rendered = errors(concat!(
        "installer { name = \"a\", outFile = \"a.exe\" }\n",
        "function f() end\n",
        "section(\"S\", function() local x = f() end)\n",
    ));
    assert!(rendered.contains("does not produce a value"));
}

#[test]
fn an_imported_macro_writes_its_output_into_the_destination() {
    let out = ok(concat!(
        "local winver = import \"WinVer\"\n",
        "installer { name = \"a\", outFile = \"a.exe\" }\n",
        "section(\"S\", function() local b = winver.getBuild() detailPrint(b) end)\n",
    ));
    assert!(out.starts_with("!include \"WinVer.nsh\"\n\nName"));
    // Destination-passing: straight into `$0`, no temporary plus `StrCpy`.
    assert!(out.contains("  ${WinVerGetBuild} $0\n  DetailPrint $0\n"));
}

#[test]
fn a_macro_without_an_output_is_a_statement() {
    let out = ok(concat!(
        "local x64 = import \"x64\"\n",
        "installer { name = \"a\", outFile = \"a.exe\" }\n",
        "section(\"S\", function() x64.disableFSRedirection() end)\n",
    ));
    assert!(out.contains("  ${DisableX64FSRedirection}\n"));

    let rendered = errors(concat!(
        "local x64 = import \"x64\"\n",
        "installer { name = \"a\", outFile = \"a.exe\" }\n",
        "section(\"S\", function() local a = x64.disableFSRedirection() end)\n",
    ));
    assert!(rendered.contains("has no output variable"));
}

#[test]
fn an_unused_import_emits_no_include_but_does_warn() {
    let (out, rendered) = with_warnings(concat!(
        "local x64 = import \"x64\"\n",
        "installer { name = \"a\", outFile = \"a.exe\" }\n",
    ));
    assert!(!out.contains("!include"));
    assert!(rendered.contains("warning[W001]"));
    assert!(rendered.contains("unused import `x64`"));
}

#[test]
fn unknown_headers_and_macros_list_what_exists() {
    assert!(
        errors("local a = import \"Nope\"\ninstaller { name = \"a\", outFile = \"a.exe\" }")
            .contains("unknown header `Nope`")
    );

    let rendered = errors(concat!(
        "local winver = import \"WinVer\"\n",
        "installer { name = \"a\", outFile = \"a.exe\" }\n",
        "section(\"S\", function() winver.getPatch() end)\n",
    ));
    assert!(rendered.contains("`WinVer` has no macro `getPatch`"));
    assert!(rendered.contains("known macros: getMajor, getMinor, getBuild"));
}

#[test]
fn a_plugin_method_is_pascal_cased_not_tabulated() {
    let out = ok(concat!(
        "local system = plugin \"System\"\n",
        "installer { name = \"a\", outFile = \"a.exe\" }\n",
        "section(\"S\", function() system.call(\"kernel32::Beep(i, i) i (1, 2)\") end)\n",
    ));
    assert!(out.contains("  System::Call \"kernel32::Beep(i, i) i (1, 2)\"\n"));
}

#[test]
fn imports_and_plugins_are_top_level_only() {
    let rendered = errors(concat!(
        "installer { name = \"a\", outFile = \"a.exe\" }\n",
        "section(\"S\", function() local w = import \"WinVer\" end)\n",
    ));
    assert!(rendered.contains("only allowed at the top level"));
}

#[test]
fn calling_through_an_unbound_object_is_an_error() {
    let rendered = errors(concat!(
        "installer { name = \"a\", outFile = \"a.exe\" }\n",
        "section(\"S\", function() nope.thing() end)\n",
    ));
    assert!(rendered.contains("`nope` is not an `import` or a `plugin`"));
}

#[test]
fn all_errors_are_reported_not_just_the_first() {
    let mut diags = Diagnostics::default();
    compile(
        r#"installer { name = "a", banana = "b", outFile = 3 }"#,
        &mut diags,
    );
    assert_eq!(diags.iter().count(), 2);
}

// --- Compile-time ---------------------------------------------------

#[test]
fn a_const_becomes_a_define_and_is_used_by_name() {
    let out = ok(concat!(
        "local APP <const> = \"Example\"\n",
        "local TITLE <const> = APP .. \" 1.0\"\n",
        "installer { name = TITLE, outFile = \"a.exe\" }\n",
    ));
    assert_eq!(
        out,
        concat!(
            "!define APP \"Example\"\n",
            "!define TITLE \"${APP} 1.0\"\n",
            "\n",
            "Name \"${TITLE}\"\n",
            "OutFile \"a.exe\"\n",
        )
    );
}

#[test]
fn const_arithmetic_folds_rather_than_reaching_define_math() {
    let out = ok(concat!(
        "local KB <const> = 96 * 1024\n",
        "installer { name = \"a\", outFile = \"a.exe\" }\n",
        "section(\"S\", function() detailPrint(KB) end)\n",
    ));
    assert!(out.contains("!define KB \"98304\"\n"));
    assert!(out.contains("  DetailPrint ${KB}\n"));
}

#[test]
fn a_pre_command_runs_on_the_build_machine_and_defines_its_output() {
    let out = ok(concat!(
        "local V <const> = pre.getDllVersion(\"app.dll\")\n",
        "installer { name = \"a\", outFile = \"a.exe\" }\n",
        "section(\"S\", function() detailPrint(\"v\" .. V) end)\n",
    ));
    assert!(out.contains("!getdllversion \"app.dll\" V_\n"));
    assert!(out.contains("!define V \"${V_1}.${V_2}.${V_3}.${V_4}\"\n"));
    assert!(out.contains("  DetailPrint \"v${V}\"\n"));
}

#[test]
fn a_const_cannot_read_a_register() {
    let rendered = errors(concat!(
        "local X <const> = Y\n",
        "installer { name = \"a\", outFile = \"a.exe\" }\n",
    ));
    assert!(rendered.contains("`Y` is not a build-time constant"));
}

#[test]
fn const_is_top_level_only() {
    let rendered = errors(&section("local X <const> = 1"));
    assert!(rendered.contains("only allowed at the top level"));
}

#[test]
fn an_unknown_pre_command_lists_what_exists() {
    let rendered = errors(concat!(
        "local X <const> = pre.reboot()\n",
        "installer { name = \"a\", outFile = \"a.exe\" }\n",
    ));
    assert!(rendered.contains("unknown build-time command `pre.reboot`"));
}

// --- Strings and arithmetic -------------------------------------

#[test]
fn concatenation_is_a_template_not_an_instruction() {
    // Three operands, one `DetailPrint`, no `StrCpy` anywhere.
    assert_eq!(
        body_of(&section(
            "local n = 1\ndetailPrint(\"a\" .. n .. \"b\" .. 2)"
        )),
        "  StrCpy $0 1\n  DetailPrint \"a$0b2\"\n"
    );
}

#[test]
fn a_computed_operand_materializes_before_the_template() {
    assert_eq!(
        body_of(&section(
            "local a = 3\nlocal b = 4\ndetailPrint(\"sum \" .. a + b)"
        )),
        concat!(
            "  StrCpy $0 3\n",
            "  StrCpy $1 4\n",
            "  IntOp $R9 $0 + $1\n",
            "  DetailPrint \"sum $R9\"\n",
        )
    );
}

#[test]
fn floor_division_and_modulo_are_intop_opcodes() {
    assert_eq!(
        body_of(&section("local a = 7\nlocal b = a // 2\nlocal c = a % 2")),
        concat!(
            "  StrCpy $0 7\n",
            "  IntOp $1 $0 / 2\n",
            "  IntOp $2 $0 % 2\n",
        )
    );
}

#[test]
fn the_rejected_operators_say_what_to_use_instead() {
    assert!(errors(&section("local a = 4 / 2")).contains("use `//`"));
    assert!(errors(&section("local a = 4 ^ 2")).contains("xor"));
}

#[test]
fn an_adapted_stdlib_name_pulls_in_its_header_and_init_once() {
    let out = ok(&section(
        "local a = string.upper(\"x\")\nlocal b = string.lower(\"Y\")\ndetailPrint(a)",
    ));
    assert!(out.starts_with("!include \"StrFunc.nsh\"\n\n${StrCase}\n"));
    // One init for two macros, and the output slot comes first here.
    assert_eq!(out.matches("${StrCase}\n").count(), 1);
    assert!(out.contains("  ${StrCase} $0 \"x\" \"U\"\n"));
    assert!(out.contains("  ${StrCase} $1 \"Y\" \"L\"\n"));
}

#[test]
fn a_rejected_stdlib_name_names_the_adaptations_that_exist() {
    let rendered = errors(&section("local a = string.reverse(\"x\")"));
    assert!(rendered.contains("`string.reverse` has no NSIS lowering"));
    assert!(rendered.contains("adapted from `string`: upper, lower"));
}

// --- messageBox ----------------------------------------------------

#[test]
fn a_message_box_without_handlers_is_one_instruction() {
    assert_eq!(
        body_of(&section(
            "messageBox { text = \"hi\", buttons = \"OK\", icon = \"INFORMATION\" }"
        )),
        "  MessageBox MB_OK|MB_ICONINFORMATION \"hi\"\n"
    );
}

#[test]
fn handlers_become_a_jump_table_with_no_materialized_answer() {
    assert_eq!(
        body_of(&section(concat!(
            "messageBox {\n",
            "  text = \"go?\",\n",
            "  buttons = \"YESNO\",\n",
            "  default = \"NO\",\n",
            "  onNo = function() detailPrint(\"no\") end,\n",
            "  onYes = function() detailPrint(\"yes\") end,\n",
            "}"
        ))),
        concat!(
            // Emitted in button order, not source order.
            "  MessageBox MB_YESNO \"go?\" /SD IDNO IDYES mb_yes_0 IDNO mb_no_0\n",
            "mb_yes_0:\n",
            "  DetailPrint \"yes\"\n",
            "  Goto mb_end_0\n",
            "mb_no_0:\n",
            "  DetailPrint \"no\"\n",
            "mb_end_0:\n",
        )
    );
}

#[test]
fn an_unhandled_button_gets_a_guard_jump() {
    // Without it, `No` would fall out of the `MessageBox` line straight into
    // the `Yes` block.
    let body = body_of(&section(concat!(
        "messageBox {\n",
        "  text = \"go?\",\n",
        "  buttons = \"YESNO\",\n",
        "  onYes = function() detailPrint(\"yes\") end,\n",
        "}"
    )));
    assert_eq!(
        body,
        concat!(
            "  MessageBox MB_YESNO \"go?\" IDYES mb_yes_0\n",
            "  Goto mb_end_0\n",
            "mb_yes_0:\n",
            "  DetailPrint \"yes\"\n",
            "mb_end_0:\n",
        )
    );
}

#[test]
fn handlers_are_checked_against_the_button_set() {
    let rendered = errors(&section(concat!(
        "messageBox { text = \"x\", buttons = \"YESNO\", onCancel = function() end }"
    )));
    assert!(rendered.contains("`onCancel` is not a button of `YESNO`"));
    assert!(rendered.contains("`YESNO` has: onYes, onNo"));

    let rendered = errors(&section(
        "messageBox { text = \"x\", buttons = \"YESNO\", default = \"OK\" }",
    ));
    assert!(rendered.contains("`OK` is not a button of `YESNO`"));

    let rendered = errors(&section("messageBox { text = \"x\", icon = \"SPLAT\" }"));
    assert!(rendered.contains("unknown icon `SPLAT`"));
}

// --- Sections and section groups -----------------------------------

#[test]
fn section_options_become_flags_and_groups_nest() {
    let out = ok(concat!(
        "installer { name = \"a\", outFile = \"a.exe\" }\n",
        "sectionGroup(\"G\", { expanded = true }, function()\n",
        "  section(\"A\", { optional = true }, function() detailPrint(\"a\") end)\n",
        "  section(\"B\", function() detailPrint(\"b\") end)\n",
        "end)\n",
    ));
    assert!(out.ends_with(concat!(
        "SectionGroup /e \"G\"\n",
        "  Section /o \"A\"\n",
        "    DetailPrint \"a\"\n",
        "  SectionEnd\n",
        "\n",
        "  Section \"B\"\n",
        "    DetailPrint \"b\"\n",
        "  SectionEnd\n",
        "SectionGroupEnd\n",
    )));
}

#[test]
fn a_section_group_holds_only_sections() {
    let rendered = errors(concat!(
        "installer { name = \"a\", outFile = \"a.exe\" }\n",
        "sectionGroup(\"G\", function() detailPrint(\"x\") end)\n",
    ));
    assert!(rendered.contains("holds only `section` declarations"));
}

#[test]
fn an_unknown_section_option_is_not_silently_dropped() {
    let rendered = errors(concat!(
        "installer { name = \"a\", outFile = \"a.exe\" }\n",
        "section(\"A\", { optionel = true }, function() end)\n",
    ));
    assert!(rendered.contains("unknown option `optionel`"));
    assert!(rendered.contains("supported options: optional"));
}
