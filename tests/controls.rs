//! The control surface (§15.32): what a `page.custom` draws, and the four claim
//! rules it shares with `section`.
//!
//! The golden in [`tests/goldens.rs`](goldens.rs) proves a page of controls
//! emits and assembles. What is here is the half a golden cannot show: the
//! writings that are refused, and the three places the generated form is not the
//! obvious one — a claimed control's `Var`, an unclaimed one's `Pop`, and the
//! style word folded at compile time so that nothing has to be `!include`d.
//!
//! The second half of the file is the *fields* — `serial.value`,
//! `agree.checked` — which are to a control what [`tests/handles.rs`](handles.rs)
//! covers for a section, and differ from it in one way worth testing: none of
//! them is a read-modify-write, and two of them cannot be read at all.

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

/// A program whose installer holds one custom page and one section.
fn program(declarations: &str, controls: &str) -> String {
    format!(
        "attributes {{ outFile = \"a.exe\", name = \"a\" }}\n\
         {declarations}\n\
         installer {{\n\
         page.custom {{ controls = {{ {controls} }} }},\n\
         page.instFiles {{}},\n\
         section(\"Core\", function() detailPrint(\"x\") end),\n\
         }}\n"
    )
}

/// Ruling 5, which is the standing requirement made structural: a program that
/// uses a custom page includes nothing, so there is no include order for it to
/// get wrong. `${__NSD_Label_STYLE}` would need `nsDialogs.nsh`; the number it
/// expands to needs nothing.
#[test]
fn a_control_is_created_with_no_header_of_any_kind() {
    let output = build(&program("", "label { \"Hi\", y = 0, height = 12 },"));

    assert!(
        output
            .contains("nsDialogs::CreateControl STATIC 0x54000100 0x00000020 0 0u 100% 12u \"Hi\""),
        "{output}"
    );
    assert!(!output.contains("nsDialogs.nsh"), "{output}");
    assert!(!output.contains("${__NSD"), "{output}");
}

/// Ruling 7, and the reason it is not a register: a handle is popped in the
/// creator and read in `leave`, which is a different NSIS function, and every
/// plugin call in between clobbers every register (§15.11).
#[test]
fn only_a_claimed_control_earns_a_var() {
    let output = build(&program(
        "local serial = text { \"\", y = 0, height = 12 }",
        "label { \"Serial:\", y = 0, height = 12 }, serial,",
    ));

    assert!(output.contains("Var __GENERATED_ctl_serial"), "{output}");
    assert!(output.contains("Pop $__GENERATED_ctl_serial"), "{output}");
    // The label is bound to no `local`, so its handle is popped into whatever
    // the allocator had spare and named nowhere. Popped all the same: the
    // plugin pushes a handle whether or not the program wants one.
    assert_eq!(output.matches("Var __GENERATED_ctl").count(), 1, "{output}");
    assert_eq!(
        output.matches("nsDialogs::CreateControl").count(),
        2,
        "{output}"
    );
    assert_eq!(output.matches("\n  Pop ").count(), 3, "{output}");
}

/// The list's order is the drawing order and the tab order, and it is the one
/// thing a control's declaration does not decide. Same shape as a section: the
/// `local` above says what, the list below says where.
#[test]
fn the_list_decides_the_order_and_the_declaration_does_not() {
    let output = build(&program(
        "local second = text { \"\", y = 20, height = 12 }\n\
         local first = label { \"Serial:\", y = 0, height = 12 }",
        "first, second,",
    ));

    let first = output.find("\"Serial:\"").expect("the label");
    let second = output.find("Pop $__GENERATED_ctl_second").expect("the box");
    assert!(first < second, "{output}");
}

/// `items` is filled by message rather than by a create argument, because
/// `CreateControl` has one text and a list has many. `STR:` is `SendMessage`'s
/// spelling for "a string, not a number", and it is the compiler's to write.
#[test]
fn a_list_control_is_filled_by_message() {
    let output = build(&program(
        "local flavour = dropList { y = 0, height = 60, items = { \"Full\", \"Minimal\" } }",
        "flavour,",
    ));

    assert!(
        output.contains("SendMessage $__GENERATED_ctl_flavour 0x0143 0 \"STR:Full\""),
        "{output}"
    );
    assert!(
        output.contains("SendMessage $__GENERATED_ctl_flavour 0x0143 0 \"STR:Minimal\""),
        "{output}"
    );
}

/// Ruling 6. `x` and `width` default to constants — the left edge and the full
/// width — and `y` and `height` have no default at all, because the only one
/// they could have is *under the control written above*, which is the auto-flow
/// this surface does not have.
#[test]
fn a_control_says_where_it_sits() {
    let raised = errors(&program("", "label { \"Hi\", height = 12 },"));
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].0, Code::MissingAttribute);
    assert!(raised[0].1.contains("`y`"), "{raised:?}");

    // And the two that do default, do so without reading any other control.
    let output = build(&program("", "label { \"Hi\", y = 4, height = 12 },"));
    assert!(output.contains(" 0 4u 100% 12u "), "{output}");
}

/// A bare number is **pixels** to nsDialogs, and a page laid out in pixels comes
/// apart at a different font size or DPI. An integer is therefore written with
/// the `u` that makes it dialog units, and a string passes through for the two
/// things a number cannot say.
#[test]
fn a_measurement_is_dialog_units_unless_it_says_otherwise() {
    let output = build(&program(
        "",
        "label { \"Hi\", x = 2, y = 0, width = \"100%\", height = \"-13u\" },",
    ));
    assert!(output.contains(" 2u 0u 100% -13u "), "{output}");

    // Anything nsDialogs cannot read it treats as zero, which is a control that
    // is there, is the right size, and sits in the corner.
    let raised = errors(&program(
        "",
        "label { \"Hi\", y = 0, height = 12, width = \"wide\" },",
    ));
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].0, Code::BadFieldValue);
}

/// Claim rule 1, in the words a control needs: a `local` no page lists is data
/// written at one level that evaporates if nothing reads it.
#[test]
fn a_control_no_page_lists_is_an_error() {
    let raised = errors(&program(
        "local orphan = label { \"Hi\", y = 0, height = 12 }",
        "",
    ));
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].0, Code::MissingAttribute);
    assert!(raised[0].1.contains("no page lists"), "{raised:?}");
}

/// Claim rules 2 and 3, which are the section's two rules over the construct one
/// step in: one declaration is one control, in one place.
#[test]
fn a_control_is_listed_once() {
    let raised = errors(&program(
        "local serial = text { \"\", y = 0, height = 12 }",
        "serial, serial,",
    ));
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].0, Code::DuplicateBlock);
    assert!(raised[0].1.contains("twice among `controls`"), "{raised:?}");

    let both = errors(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         local serial = text { \"\", y = 0, height = 12 }\n\
         installer { page.custom { controls = { serial } }, page.instFiles {} }\n\
         uninstaller { page.custom { controls = { serial } }, page.instFiles {} }\n",
    );
    assert!(
        both.iter()
            .any(|(code, message)| *code == Code::DuplicateBlock
                && message.contains("both `installer` and `uninstaller`")),
        "{both:?}"
    );
}

/// The declaration and the construct that lists it have to agree. A control in a
/// block would be a window with no dialog to sit in, and a section among
/// `controls` would be an install-time thing among drawing ones.
#[test]
fn a_declaration_is_listed_by_the_construct_it_belongs_to() {
    let control_in_a_block = errors(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         local serial = text { \"\", y = 0, height = 12 }\n\
         installer { serial, page.instFiles {} }\n",
    );
    assert!(
        control_in_a_block
            .iter()
            .any(|(code, message)| *code == Code::BadFieldValue
                && message.contains("this lists a section or a group")),
        "{control_in_a_block:?}"
    );

    let section_in_a_page = errors(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         local core = section(\"Core\", function() detailPrint(\"x\") end)\n\
         installer { core, page.custom { controls = { core } }, page.instFiles {} }\n",
    );
    assert!(
        section_in_a_page
            .iter()
            .any(|(code, message)| *code == Code::BadFieldValue
                && message.contains("this lists a control")),
        "{section_in_a_page:?}"
    );
}

/// The kind knows what it holds. `items` on a label is the same error as
/// `expanded` on a section: a field of a different thing, named on this one.
#[test]
fn a_control_takes_only_its_own_options() {
    let raised = errors(&program(
        "",
        "label { \"Hi\", y = 0, height = 12, items = { \"a\" } },",
    ));
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].0, Code::UnknownField);
    assert!(raised[0].1.contains("holds no items"), "{raised:?}");

    let unknown = errors(&program(
        "",
        "label { \"Hi\", y = 0, height = 12, colour = \"red\" },",
    ));
    assert_eq!(unknown.len(), 1, "{unknown:?}");
    assert_eq!(unknown[0].0, Code::UnknownField);
}

/// A control belongs to one half the way a section does. The `Var` carries the
/// half in its name for the same reason the define does: one `.nsi` holds both
/// executables, and one name declared twice is an error under `-WX`.
#[test]
fn each_half_names_its_own_controls() {
    let output = build(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         local before = text { \"\", y = 0, height = 12 }\n\
         local after = text { \"\", y = 0, height = 12 }\n\
         installer {\n\
         page.custom { controls = { before } },\n\
         page.instFiles {},\n\
         section(\"Core\", function() writeUninstaller(INSTDIR .. \"/un.exe\") end),\n\
         }\n\
         uninstaller {\n\
         page.custom { controls = { after } },\n\
         page.instFiles {},\n\
         section(\"Core\", function() detailPrint(\"x\") end),\n\
         }\n",
    );

    assert!(output.contains("Var __GENERATED_ctl_before"), "{output}");
    assert!(output.contains("Var __GENERATED_unctl_after"), "{output}");
}

// -- fields ---------------------------------------------------------------

/// A program whose custom page runs `body` in its `leave` callback.
fn page(declarations: &str, controls: &str, body: &str) -> String {
    format!(
        "attributes {{ outFile = \"a.exe\", name = \"a\" }}\n\
         {declarations}\n\
         installer {{\n\
         page.custom {{ controls = {{ {controls} }},\n\
         leave = function()\n{body}\nend }},\n\
         page.instFiles {{}},\n\
         section(\"Core\", function() detailPrint(\"x\") end),\n\
         }}\n"
    )
}

/// The one field whose read is not its write with a different message number.
///
/// `SendMessage` in NSIS has nowhere to put a string it is handed back, so
/// `WM_GETTEXT` cannot be written at all — which is why `nsDialogs.nsh` reads
/// text through `GetWindowText` and why this does too. Ruling 5 survives it:
/// `System` is a plugin and `${NSIS_MAX_STRLEN}` is makensis' own define.
#[test]
fn text_is_read_by_a_call_and_written_by_a_message() {
    let output = build(&page(
        "local serial = text { \"\", y = 0, height = 12 }",
        "serial,",
        "if serial.value == \"\" then detailPrint(\"empty\") end\n\
         serial.value = \"filled\"",
    ));

    assert!(
        output.contains(
            "System::Call \"user32::GetWindowText(p$__GENERATED_ctl_serial,t.s,i\
             ${NSIS_MAX_STRLEN})\""
        ),
        "{output}"
    );
    assert!(
        output.contains("SendMessage $__GENERATED_ctl_serial 0x000C 0 \"STR:filled\""),
        "{output}"
    );
    assert!(!output.contains("nsDialogs.nsh"), "{output}");
}

/// Five of the seven fields have a setter and no getter, and the diagnostic says
/// which instruction the setter is rather than inventing a message number whose
/// answer would be whatever the control does with a message it does not know.
#[test]
fn a_field_windows_will_not_report_is_a_write_only() {
    let raised = errors(&page(
        "local agree = checkbox { \"ok\", y = 0, height = 12 }",
        "agree,",
        "local on = agree.enabled",
    ));
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].0, Code::UnknownField);
    assert!(raised[0].1.contains("written and not read"), "{raised:?}");

    // And the two that Windows does report, do.
    let output = build(&page(
        "local agree = checkbox { \"ok\", y = 0, height = 12 }",
        "agree,",
        "if agree.checked then detailPrint(\"ticked\") end",
    ));
    assert!(
        output.contains("SendMessage $__GENERATED_ctl_agree 0x00F0 0 0 $"),
        "{output}"
    );
}

/// `checked` and `image` depend on how the window was drawn, and the two are the
/// only fields that do. A tick on a label is not refused by Windows — it answers
/// 0 — which is what makes this worth a compile-time error.
#[test]
fn a_field_that_needs_a_kind_needs_a_declaration() {
    let raised = errors(&page(
        "local tag = label { \"hi\", y = 0, height = 12 }",
        "tag,",
        "tag.checked = true",
    ));
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].0, Code::UnknownField);
    assert!(raised[0].1.contains("a `checkbox`"), "{raised:?}");

    let on_a_found_window = errors(&page(
        "",
        "label { \"hi\", y = 0, height = 12 },",
        "local cancel = getDlgItem(HWNDPARENT, 2)\ncancel.image = \"a.bmp\"",
    ));
    assert_eq!(on_a_found_window.len(), 1, "{on_a_found_window:?}");
    assert!(
        on_a_found_window[0].1.contains("a `bitmap`"),
        "{on_a_found_window:?}"
    );
}

/// The other half of that rule: a window this compiler did not draw still
/// answers to the fields every window has, which is what makes `getDlgItem` a
/// way to reach MUI2's own buttons rather than a handle with nothing to do.
#[test]
fn a_window_the_program_did_not_draw_has_the_fields_every_window_has() {
    let output = build(&page(
        "",
        "label { \"hi\", y = 0, height = 12 },",
        "local cancel = getDlgItem(HWNDPARENT, 2)\ncancel.enabled = false",
    ));

    assert!(output.contains("GetDlgItem $0 $HWNDPARENT 2"), "{output}");
    assert!(output.contains("EnableWindow $0 0"), "{output}");
}

/// `ShowWindow`'s two states are 0 and 5, not 0 and 1, so a literal picks one
/// and anything else is multiplied — which is exact, because a `bool` in this
/// language is 0 or 1 and nothing else (§15.20).
#[test]
fn visible_is_hide_and_show_rather_than_false_and_true() {
    let literal = build(&page(
        "local agree = checkbox { \"ok\", y = 0, height = 12 }",
        "agree,",
        "agree.visible = false",
    ));
    assert!(
        literal.contains("ShowWindow $__GENERATED_ctl_agree 0"),
        "{literal}"
    );

    let computed = build(&page(
        "local agree = checkbox { \"ok\", y = 0, height = 12 }",
        "agree,",
        "agree.visible = agree.checked",
    ));
    assert!(computed.contains("IntOp $0 $0 * 5"), "{computed}");
    assert!(
        computed.contains("ShowWindow $__GENERATED_ctl_agree $0"),
        "{computed}"
    );
}

/// One instruction sets both colours, so one field holds both. `textColor` and
/// `backColor` as a pair would mean a write to either replacing the other with
/// something this compiler chose, and the evidence would be on the screen rather
/// than in the output.
#[test]
fn colours_are_one_field_because_they_are_one_instruction() {
    let output = build(&page(
        "local tag = label { \"hi\", y = 0, height = 12 }",
        "tag,",
        "tag.colors = { text = \"FF0000\", background = \"transparent\" }",
    ));
    assert!(
        output.contains("SetCtlColors $__GENERATED_ctl_tag FF0000 transparent"),
        "{output}"
    );

    let half = errors(&page(
        "local tag = label { \"hi\", y = 0, height = 12 }",
        "tag,",
        "tag.colors = { text = \"FF0000\" }",
    ));
    assert_eq!(half.len(), 1, "{half:?}");
    assert_eq!(half[0].0, Code::MissingAttribute);

    // `SetCtlColors` reads anything it does not understand as black, so a colour
    // that is not one is an error rather than a label that disappears.
    let named = errors(&page(
        "local tag = label { \"hi\", y = 0, height = 12 }",
        "tag,",
        "tag.colors = { text = \"red\", background = \"transparent\" }",
    ));
    assert!(
        named.iter().any(|(code, _)| *code == Code::BadFieldValue),
        "{named:?}"
    );
}

/// A font is two instructions and one temporary: `CreateFont` makes the object
/// and `WM_SETFONT` hands it over. `bold` is a weight rather than a flag,
/// because that is what `CreateFont` takes.
#[test]
fn a_font_is_a_face_and_a_size() {
    let output = build(&page(
        "local tag = label { \"hi\", y = 0, height = 12 }",
        "tag,",
        "tag.font = { face = \"Tahoma\", size = 10, bold = true }",
    ));
    assert!(
        output.contains("CreateFont $0 \"Tahoma\" 10 700"),
        "{output}"
    );
    assert!(
        output.contains("SendMessage $__GENERATED_ctl_tag 0x0030 $0 1"),
        "{output}"
    );

    let plain = build(&page(
        "local tag = label { \"hi\", y = 0, height = 12 }",
        "tag,",
        "tag.font = { face = \"Tahoma\", size = 10 }",
    ));
    assert!(plain.contains("CreateFont $0 \"Tahoma\" 10 400"), "{plain}");

    let sizeless = errors(&page(
        "local tag = label { \"hi\", y = 0, height = 12 }",
        "tag,",
        "tag.font = { face = \"Tahoma\" }",
    ));
    assert_eq!(sizeless.len(), 1, "{sizeless:?}");
    assert_eq!(sizeless[0].0, Code::MissingAttribute);
}

/// `image` is the one field that is also an option, because a `bitmap` whose
/// picture arrives only from a callback is a declaration that declares an empty
/// rectangle.
#[test]
fn a_bitmap_says_what_it_draws_where_it_is_declared() {
    let output = build(&program(
        "",
        "bitmap { image = \"check.bmp\", y = 0, height = 20 },",
    ));
    assert!(
        output.contains("LoadAndSetImage /STRINGID $0 0 0x0010 \"check.bmp\""),
        "{output}"
    );

    let elsewhere = errors(&program(
        "",
        "label { \"hi\", y = 0, height = 12, image = \"check.bmp\" },",
    ));
    assert_eq!(elsewhere.len(), 1, "{elsewhere:?}");
    assert_eq!(elsewhere[0].0, Code::UnknownField);
}

/// Claim rule 4, restored for controls: the `Var` exists in the file either way,
/// it is empty in the half that did not draw the dialog, and `SendMessage 0` is
/// a silent no-op rather than a crash.
#[test]
fn a_control_of_the_other_half_is_not_addressable() {
    let raised = errors(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         local serial = text { \"\", y = 0, height = 12 }\n\
         installer {\n\
         page.custom { controls = { serial } },\n\
         page.instFiles {},\n\
         section(\"Core\", function() writeUninstaller(INSTDIR .. \"/un.exe\") end),\n\
         }\n\
         uninstaller {\n\
         page.instFiles {},\n\
         section(\"Core\", function() detailPrint(serial.value) end),\n\
         }\n",
    );

    assert!(
        raised
            .iter()
            .any(|(code, message)| *code == Code::UnknownField
                && message.contains("`serial` is a `text` of the `installer`")),
        "{raised:?}"
    );
}

// -- events ---------------------------------------------------------------

/// An event is an option and not a field: the address of a function is a
/// build-time fact, so there is no install-time moment at which one could be
/// assigned that is not already inside a callback.
///
/// `GetFunctionAddress` stays a `todo` row while the compiler emits it, for the
/// same reason `SectionGetFlags` did — §3's *"`Call`-by-address has no Lua
/// shape"* is a statement about the surface, and the address of a generated
/// function exists in exactly one place.
#[test]
fn a_callback_is_registered_where_the_control_is_made() {
    let output = build(&program(
        "local proceed = button { \"Check\", y = 0, height = 14,\n\
         onClick = function() detailPrint(\"checked\") end }",
        "proceed,",
    ));

    assert!(
        output.contains("GetFunctionAddress $0 mui.control.proceed.click"),
        "{output}"
    );
    assert!(
        output.contains("nsDialogs::OnClick $__GENERATED_ctl_proceed $0"),
        "{output}"
    );
}

/// The one line of protocol a callback owes: nsDialogs pushes the control's
/// handle before calling, and a callback that leaves it there corrupts the stack
/// for everything after — which shows up as a wrong string somewhere unrelated
/// rather than as a crash.
#[test]
fn a_callback_pops_the_handle_nsdialogs_pushed() {
    let output = build(&program(
        "local proceed = button { \"Check\", y = 0, height = 14,\n\
         onClick = function() detailPrint(\"checked\") end }",
        "proceed,",
    ));

    let body = output
        .split("Function mui.control.proceed.click")
        .nth(1)
        .expect("the callback");
    let popped = body.find("Pop $").expect("the pop");
    let first = body.find("DetailPrint").expect("the body");
    assert!(popped < first, "{output}");
}

/// Not every control supports both, and nsDialogs' own documentation is where
/// the line is drawn: *"there is nothing to notify about label changes, only
/// clicks"*. Registering one anyway is a callback that never runs.
#[test]
fn an_event_is_on_the_kinds_that_have_it() {
    let raised = errors(&program(
        "",
        "label { \"Hi\", y = 0, height = 12, onChange = function() abort() end },",
    ));
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].0, Code::UnknownField);
    assert!(raised[0].1.contains("`onChange`"), "{raised:?}");

    // And a `hLine` is a rule drawn on the page: it has neither.
    let rule = errors(&program(
        "",
        "hLine { y = 0, height = 2, onClick = function() abort() end },",
    ));
    assert_eq!(rule.len(), 1, "{rule:?}");
    assert_eq!(rule[0].0, Code::UnknownField);
}

/// A `link` is an owner-drawn button that looks like one and opens nothing, so
/// `url` is the click written for you — and writing both is asking for two
/// things to happen on one click.
#[test]
fn a_url_is_the_click_the_compiler_writes() {
    let output = build(&program(
        "",
        "link { \"Terms\", url = \"https://example.invalid\", y = 0, height = 12 },",
    ));
    assert!(
        output.contains("nsDialogs::CreateControl LINK 0x5401000B"),
        "{output}"
    );
    assert!(
        output.contains("ExecShell \"open\" \"https://example.invalid\""),
        "{output}"
    );
    assert!(output.contains("nsDialogs::OnClick $0 $1"), "{output}");

    let both = errors(&program(
        "",
        "link { \"Terms\", url = \"https://example.invalid\", y = 0, height = 12,\n\
         onClick = function() abort() end },",
    ));
    assert_eq!(both.len(), 1, "{both:?}");
    assert_eq!(both[0].0, Code::DuplicateBlock);
}

/// A callback belongs to the half whose page drew the control, and NSIS says so
/// with a prefix: an uninstaller function is `un.`-something, and calling one
/// from the installer is not possible rather than merely wrong.
#[test]
fn a_callback_carries_the_half_that_owns_it() {
    let output = build(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         local bye = button { \"Bye\", y = 0, height = 14,\n\
         onClick = function() detailPrint(\"bye\") end }\n\
         installer {\n\
         page.instFiles {},\n\
         section(\"Core\", function() writeUninstaller(INSTDIR .. \"/un.exe\") end),\n\
         }\n\
         uninstaller {\n\
         page.custom { controls = { bye } },\n\
         page.instFiles {},\n\
         section(\"Core\", function() detailPrint(\"x\") end),\n\
         }\n",
    );

    assert!(
        output.contains("Function un.mui.control.bye.click"),
        "{output}"
    );
    assert!(
        output.contains("GetFunctionAddress $0 un.mui.control.bye.click"),
        "{output}"
    );
}

/// The seven instructions the fields and the events write are `lowering-target`
/// rows and not `todo` ones, which is §5's retired-instruction diagnostic
/// rather than a census entry: a user who reaches for `SendMessage` is told the
/// field to write, and gets it in the compiler rather than in `LANGUAGE.md`.
///
/// None of them could have been `exposed`. A call needs a handle, and every
/// handle there is belongs to a control this compiler drew — so the field *is*
/// the call, with the kind checked and the register spilled.
#[test]
fn the_instructions_behind_the_fields_are_retired_rather_than_missing() {
    for (name, field) in [
        ("sendMessage", "`agree.checked = true`"),
        ("enableWindow", "`agree.enabled = false`"),
        ("showWindow", "`badge.visible = false`"),
        ("setCtlColors", "a control's `colors`"),
        ("createFont", "a control's `font`"),
        ("loadAndSetImage", "a `bitmap`'s `image`"),
        ("getFunctionAddress", "an event"),
    ] {
        let mut diags = Diagnostics::new();
        installua::build(
            &format!(
                "attributes {{ outFile = \"a.exe\", name = \"a\" }}\n\
                 installer {{\n\
                 page.instFiles {{}},\n\
                 section(\"Core\", function() {name}(1) end),\n\
                 }}\n"
            ),
            &mut diags,
        );

        assert!(
            diags.iter().any(|d| d.code == Code::NsisRetired),
            "{name}: {}",
            diags.render("<test>")
        );
        let rendered = diags.render("<test>");
        assert!(rendered.contains(field), "{name}: {rendered}");
    }
}
