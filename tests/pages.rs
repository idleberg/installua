//! The page surface: where a MUI2 setting lives, and what it costs to put it in
//! the wrong place.
//!
//! The golden in [`tests/goldens.rs`](goldens.rs) proves the whole thing emits
//! and assembles. What is here is the half a golden cannot show — the scoping
//! MUI2's preprocessor imposes, and the writings that are refused.
//!
//! MUI2's own source draws the line between a page setting and a block setting,
//! so neither is a judgement: a setting written inside the generated `PageEx`
//! and undefined after is the page's, and one written inside an
//! `!ifndef`-guarded `MUI_*PAGE_INTERFACE` macro runs on the **first** page of
//! its type and never again. Every test below is about that seam.

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

fn program(installer: &str) -> String {
    format!("attributes {{ outFile = \"a.exe\", name = \"a\" }}\ninstaller {{ {installer} }}\n")
}

/// The whole reason a page owns its defines rather than the module owning all
/// of them. MUI2 expands `${MUI_DIRECTORYPAGE_TEXT_TOP}` at the point the macro
/// is inserted, so two pages of one type need their settings *between* the two
/// insertions — a single list up front would give both pages the last value.
#[test]
fn each_page_carries_its_own_settings() {
    let output = build(&program(
        "page.directory { topText = \"first\" },\n\
         page.directory { topText = \"second\" },",
    ));

    let first = output.find("\"first\"").expect("the first setting");
    let insert = output.find("MUI_PAGE_DIRECTORY").expect("the first page");
    let second = output.find("\"second\"").expect("the second setting");
    assert!(
        first < insert && insert < second,
        "the second page's setting must come after the first page's macro:\n{output}"
    );
}

/// The two holes in MUI2's own cleanup. `UninstallConfirm.nsh` clears its text
/// settings and never `MUI_UNCONFIRMPAGE_VARIABLE`, so without an `!undef` here
/// a second Confirm page would silently inherit the first one's variable.
#[test]
fn a_setting_mui2_forgets_to_clear_is_undefined_here() {
    let output = build(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         target = \"\"\n\
         uninstaller {\n\
         page.confirm { variable = target, topText = \"gone\" },\n\
         }\n",
    );
    assert!(
        output.contains("!undef MUI_UNCONFIRMPAGE_VARIABLE"),
        "{output}"
    );
    // And the one MUI2 does clear is left to it: a second `!undef` is warning
    // 6155, which `-WX` makes an error.
    assert!(
        !output.contains("!undef MUI_UNCONFIRMPAGE_TEXT_TOP"),
        "{output}"
    );
}

/// A `bool` MUI2 reads with `!ifdef` has no off-word to emit, so `false` is the
/// define's absence rather than a define with a false value.
#[test]
fn a_flag_setting_is_the_defines_existence() {
    let on = build(&program("page.directory { verifyOnLeave = true },"));
    assert!(
        on.contains("!define MUI_DIRECTORYPAGE_VERIFYONLEAVE\n"),
        "{on}"
    );

    let off = build(&program("page.directory { verifyOnLeave = false },"));
    assert!(!off.contains("MUI_DIRECTORYPAGE_VERIFYONLEAVE"), "{off}");
}

/// `DirVar` stores *into* its argument, so the field takes the global and not
/// its value — and the `Var` line has to precede the page that names it, which
/// is why `Module`'s field order puts the globals ahead of the pages.
#[test]
fn a_variable_field_names_a_global_and_is_declared_before_it_is_used() {
    let output = build(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         target = \"\"\n\
         installer { page.directory { variable = target } }\n",
    );
    let declaration = output.find("Var target").expect("the declaration");
    let use_site = output
        .find("!define MUI_DIRECTORYPAGE_VARIABLE $target")
        .expect("the define");
    assert!(declaration < use_site, "{output}");

    // A name nothing declared is caught here rather than by `makensis`, which
    // reports it from inside a macro expansion with no line to point at.
    let raised = errors(&program("page.directory { variable = nope },"));
    assert!(
        raised
            .iter()
            .any(|(code, message)| *code == Code::UnknownField && message.contains("not a global")),
        "{raised:?}"
    );
}

/// A page's hooks are functions the compiler names, because nothing in the
/// source names them: the hook is written where it runs. Two pages of one type
/// each get their own.
#[test]
fn a_page_hook_becomes_a_function_the_compiler_names() {
    let output = build(&program(
        "page.directory { leave = function() detailPrint(\"one\") end },\n\
         page.directory { leave = function() detailPrint(\"two\") end },",
    ));
    assert!(
        output.contains("Function mui.directory.leave\n"),
        "{output}"
    );
    assert!(
        output.contains("Function mui.directory.leave.2\n"),
        "{output}"
    );
}

/// The uninstaller's half has no spelling of its own: the block the page is
/// written in supplies both the `MUI_UNPAGE_` prefix and the `un.` on the
/// functions MUI2 will call.
#[test]
fn the_block_supplies_the_half() {
    let output = build(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         uninstaller { page.confirm { pre = function() detailPrint(\"x\") end } }\n",
    );
    assert!(
        output.contains("!insertmacro MUI_UNPAGE_CONFIRM"),
        "{output}"
    );
    assert!(output.contains("Function un.mui.confirm.pre\n"), "{output}");
}

/// `confirm` exists only as `MUI_UNPAGE_CONFIRM`, and `page.finish` in the
/// uninstaller is a different question — MUI2 defines that one. So the refusal
/// is a fact about MUI2 rather than a policy here.
#[test]
fn a_page_that_exists_in_one_half_only_says_so() {
    let raised = errors(&program("page.confirm {},"));
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].0, Code::UnknownField);
    assert!(
        raised[0].1.contains("no installer `confirm` page"),
        "{raised:?}"
    );
}

/// The licence file is the `MUI_PAGE_LICENSE` macro's *argument*, so it is
/// required at the page. As `installer { license = … }` it was page data at
/// block level: exactly one page read it, and a script naming no License page
/// dropped it without a word.
#[test]
fn a_license_page_needs_its_file() {
    let raised = errors(&program("page.license { bottomText = \"read it\" },"));
    assert!(
        raised
            .iter()
            .any(|(code, _)| *code == Code::MissingAttribute),
        "{raised:?}"
    );

    // And the block no longer takes it at all.
    let raised = errors(&program("license = \"LICENSE.txt\","));
    assert!(
        raised.iter().any(|(code, _)| *code == Code::UnknownField),
        "{raised:?}"
    );
}

/// MUI2 reads `!ifdef MUI_LICENSEPAGE_CHECKBOX` first and the radio buttons
/// only in its `!else`, so a page with both is not a page with two controls —
/// it is a page whose second setting does nothing.
#[test]
fn a_license_page_takes_one_kind_of_forced_selection() {
    let raised = errors(&program(
        "page.license { file = \"L.txt\", checkbox = \"yes\", \
         radioButtons = { accept = \"a\", decline = \"d\" } },",
    ));
    assert!(
        raised
            .iter()
            .any(|(code, message)| *code == Code::BadFieldValue && message.contains("not both")),
        "{raised:?}"
    );
}

/// The four settings that look page-scoped and are not. MUI2 writes each inside
/// an `!ifndef` interface guard, which runs once — so the block is the only
/// home where a second page's copy cannot be written.
#[test]
fn a_once_global_setting_belongs_to_the_block() {
    let output = build(&program("checkBitmap = \"check.bmp\", page.components {},"));
    assert!(
        output.contains("!define MUI_COMPONENTSPAGE_CHECKBITMAP \"check.bmp\""),
        "{output}"
    );

    let raised = errors(&program("page.components { checkBitmap = \"check.bmp\" },"));
    assert!(
        raised.iter().any(|(code, _)| *code == Code::UnknownField),
        "{raised:?}"
    );

    // And both blocks writing it would define one name twice, which is a
    // redefinition warning and so an error under `-WX`.
    let raised = errors(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         uninstaller { checkBitmap = \"check.bmp\" }\n",
    );
    assert!(
        raised
            .iter()
            .any(|(code, message)| *code == Code::UnknownField
                && message.contains("not an `uninstaller` field")),
        "{raised:?}"
    );
}

/// The header strip is not the title bar and not the page's body text, and two
/// of the seven pages draw no header at all: `Pages.nsh` calls
/// `MUI_HEADER_TEXT_PAGE` from five of them and not from `welcome` or `finish`.
#[test]
fn only_a_page_with_a_header_takes_header_text() {
    let output = build(&program("page.directory { headerText = \"Where\" },"));
    assert!(
        output.contains("!define MUI_PAGE_HEADER_TEXT \"Where\""),
        "{output}"
    );

    let raised = errors(&program("page.welcome { headerText = \"Hello\" },"));
    assert!(
        raised
            .iter()
            .any(|(code, message)| *code == Code::UnknownField
                && message.contains("not a `welcome` page field")),
        "{raised:?}"
    );
}

/// The custom page's header is the one setting that changes *mechanism* between
/// it and its seven siblings, and the reason is MUI2's: `Page custom` is a
/// stock NSIS line MUI2 never sees, so a `!define` it reads at insertion time
/// would do nothing here and then leak onto the next page that does read it.
#[test]
fn a_custom_page_calls_the_header_macro_instead_of_defining_it() {
    let output = build(&program(
        "page.custom { headerText = \"Serial\", headerSubText = \"From the invoice.\" },",
    ));
    assert!(
        output.contains("!insertmacro MUI_HEADER_TEXT \"Serial\" \"From the invoice.\""),
        "{output}"
    );
    assert!(!output.contains("MUI_PAGE_HEADER_TEXT"), "{output}");

    // The macro takes two arguments and there is no spelling for omitting one,
    // so the half that was not written is empty rather than absent.
    let output = build(&program("page.custom { headerText = \"Serial\" },"));
    assert!(
        output.contains("!insertmacro MUI_HEADER_TEXT \"Serial\" \"\""),
        "{output}"
    );
}

/// `Page custom` has two function slots and the page has three hooks, so `pre`
/// and `show` are inlined into the creator on either side of the dialog and
/// only `leave` becomes a name on the line.
#[test]
fn a_custom_page_inlines_two_of_its_three_hooks() {
    let output = build(&program(
        "page.custom {\n\
         \tpre = function() detailPrint(\"before\") end,\n\
         \tshow = function() detailPrint(\"after\") end,\n\
         \tleave = function() detailPrint(\"leaving\") end,\n\
         },",
    ));
    assert!(
        output.contains("Page custom mui.custom.create mui.custom.leave"),
        "{output}"
    );
    // One function for the page and one for `leave`, and no third.
    assert_eq!(output.matches("\nFunction ").count(), 2, "{output}");

    let before = output
        .find("DetailPrint \"before\"")
        .expect("the `pre` body");
    let create = output.find("nsDialogs::Create").expect("the dialog");
    let after = output
        .find("DetailPrint \"after\"")
        .expect("the `show` body");
    let show = output.find("nsDialogs::Show").expect("the show call");
    assert!(
        before < create && create < after && after < show,
        "{output}"
    );
}

/// A trailing argument is only omissible while the ones after it are too, which
/// is why a captioned page with no `leave` writes the empty string NSIS reads
/// as "none" — and a page with neither writes one name and stops.
#[test]
fn a_custom_page_writes_only_the_arguments_it_needs() {
    let output = build(&program("page.custom {},"));
    assert!(
        output.contains("Page custom mui.custom.create\n"),
        "{output}"
    );

    let output = build(&program("page.custom { \"Registration\" },"));
    assert!(
        output.contains("Page custom mui.custom.create \"\" \"Registration\""),
        "{output}"
    );
}

/// The array part of the table is the caption, and there is one of those. A
/// second positional is not a page with two names — it is a table written by
/// mistake, and every other page rejects the array part outright.
#[test]
fn a_custom_page_takes_one_caption() {
    let raised = errors(&program("page.custom { \"One\", \"Two\" },"));
    assert!(
        raised
            .iter()
            .any(|(code, message)| *code == Code::WrongArity && message.contains("one caption")),
        "{raised:?}"
    );
}

/// A page name outside the closed set is caught at the page rather than at
/// `makensis`, and the message lists the seven — which is the other half of
/// what member access buys: an editor completes them, and a compiler that has
/// them can say what was meant.
#[test]
fn the_pages_are_a_closed_set() {
    let raised = errors(&program("page.summary {},"));
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].0, Code::UnknownField);
    assert!(
        raised[0].1.contains("`summary` is not a page"),
        "{raised:?}"
    );
}

/// `colors` is one field holding two, one and two levels up from the control
/// field of the same name: MUI2 spends both halves on a single `SetCtlColors`,
/// so a script that could write one alone would be writing a colour this
/// compiler invented over the one the theme chose.
#[test]
fn a_colour_pair_is_one_field_at_every_level() {
    let output = build(&program(
        "headerColors = { text = \"000000\", background = \"FFFFFF\" },\n\
         page.directory { colors = { text = \"112233\", background = \"445566\" } },",
    ));

    assert!(output.contains("!define MUI_TEXTCOLOR 000000"), "{output}");
    assert!(output.contains("!define MUI_BGCOLOR FFFFFF"), "{output}");
    assert!(
        output.contains("!define MUI_DIRECTORYPAGE_BGCOLOR 445566"),
        "{output}"
    );
    assert!(
        output.contains("!define MUI_DIRECTORYPAGE_TEXTCOLOR 112233"),
        "{output}"
    );
}

/// `Directory.nsh` clears neither of its colours, so they are the third and
/// fourth holes in MUI2's own cleanup — without the `!undef` a second directory
/// page is painted in the first one's colours.
#[test]
fn the_directory_colours_do_not_survive_their_page() {
    let output = build(&program(
        "page.directory { colors = { text = \"112233\", background = \"445566\" } },",
    ));

    assert!(
        output.contains("!undef MUI_DIRECTORYPAGE_BGCOLOR"),
        "{output}"
    );
    assert!(
        output.contains("!undef MUI_DIRECTORYPAGE_TEXTCOLOR"),
        "{output}"
    );
}

/// Half a pair is refused, at both levels, in the same words.
#[test]
fn half_a_colour_pair_is_refused() {
    let page = errors(&program(
        "page.directory { colors = { background = \"445566\" } },",
    ));
    assert_eq!(page.len(), 1, "{page:?}");
    assert_eq!(page[0].0, Code::MissingAttribute);
    assert!(page[0].1.contains("`colors` wants both"), "{page:?}");

    let block = errors(&program("headerColors = { text = \"000000\" },"));
    assert_eq!(block.len(), 1, "{block:?}");
    assert!(
        block[0].1.contains("`headerColors` wants both"),
        "{block:?}"
    );
}

/// A colour is checked rather than passed through: `SetCtlColors` reads
/// anything it does not understand as black.
#[test]
fn a_colour_that_is_not_six_hex_digits_is_refused() {
    let raised = errors(&program(
        "page.directory { colors = { text = \"red\", background = \"445566\" } },",
    ));
    assert_eq!(raised[0].0, Code::BadFieldValue);
    assert!(raised[0].1.contains("`text` wants"), "{raised:?}");
}

/// The header colours are read inside MUI2's `!ifndef`-guarded macro, so they
/// are the block's and the uninstaller cannot hold a second copy.
#[test]
fn the_header_colours_belong_to_the_installer_block() {
    let raised = errors(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         uninstaller { headerColors = { text = \"000000\", background = \"FFFFFF\" } }\n",
    );
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].0, Code::UnknownField);
    assert!(
        raised[0]
            .1
            .contains("`headerColors` is not an `uninstaller` field"),
        "{raised:?}"
    );
}

/// `abortPrompt` is one field holding three defines, for the reason `colors`
/// holds two: MUI2 reads the text and the default button only inside an
/// `!ifdef MUI_ABORTWARNING`, so the field's presence has to be the enable.
#[test]
fn the_abort_prompt_is_one_field_holding_three_defines() {
    let output = build(&program(
        "abortPrompt = { text = \"Really quit?\", default = \"cancel\" },",
    ));
    assert!(output.contains("!define MUI_ABORTWARNING\n"), "{output}");
    assert!(
        output.contains("!define MUI_ABORTWARNING_TEXT \"Really quit?\""),
        "{output}"
    );
    assert!(
        output.contains("!define MUI_ABORTWARNING_CANCEL_DEFAULT\n"),
        "{output}"
    );
}

/// The plain case: on, with the wording MUI2 ships translated in every language
/// file. Nothing else is written, because a literal would be one language's
/// wording in all of them.
#[test]
fn an_abort_prompt_can_be_switched_on_alone() {
    let output = build(&program("abortPrompt = true,"));
    assert!(output.contains("!define MUI_ABORTWARNING\n"), "{output}");
    assert!(!output.contains("MUI_ABORTWARNING_TEXT"), "{output}");
    assert!(!output.contains("CANCEL_DEFAULT"), "{output}");
}

/// `false` writes nothing at all — the same as leaving the field out, so a
/// build that switches the prompt off has a spelling that is not deleting a
/// line.
#[test]
fn an_abort_prompt_that_is_off_writes_nothing() {
    let output = build(&program("abortPrompt = false,"));
    assert!(!output.contains("MUI_ABORTWARNING"), "{output}");
}

/// Both halves, unlike `headerColors`: MUI2 reads `MUI_UNABORTWARNING` in a
/// second place, so the uninstaller's prompt is its own.
#[test]
fn the_uninstaller_has_its_own_abort_prompt() {
    let output = build(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         installer { abortPrompt = true }\n\
         uninstaller { abortPrompt = { text = \"Stop uninstalling?\" } }\n",
    );
    assert!(output.contains("!define MUI_ABORTWARNING\n"), "{output}");
    assert!(output.contains("!define MUI_UNABORTWARNING\n"), "{output}");
    assert!(
        output.contains("!define MUI_UNABORTWARNING_TEXT \"Stop uninstalling?\""),
        "{output}"
    );
}

/// The default button is named, not numbered: MUI2's define says which button
/// Enter presses, and `"ok"` is what it already does.
#[test]
fn an_abort_prompt_default_is_one_of_two_buttons() {
    let ok = build(&program("abortPrompt = { default = \"ok\" },"));
    assert!(!ok.contains("CANCEL_DEFAULT"), "{ok}");

    let raised = errors(&program("abortPrompt = { default = \"yes\" },"));
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].0, Code::BadFieldValue);
    assert!(raised[0].1.contains("`default` wants"), "{raised:?}");
}

/// A misspelt member is caught rather than ignored, because the field it would
/// have set has no other spelling and a silently dropped one is a prompt that
/// shows the wrong words.
#[test]
fn an_unknown_abort_prompt_member_is_refused() {
    let raised = errors(&program("abortPrompt = { message = \"…\" },"));
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].0, Code::UnknownField);
    assert!(
        raised[0]
            .1
            .contains("`message` is not an `abortPrompt` field"),
        "{raised:?}"
    );
}

/// The finish page's plain spelling, which has to stay plain: the string is the
/// setting, and the table beside it is the exception rather than the form.
#[test]
fn a_finish_page_title_is_a_string() {
    let output = build(&program(
        "page.finish { title = \"Foo is installed\", text = \"Thanks.\", button = \"Done\" },",
    ));
    assert!(
        output.contains("!define MUI_FINISHPAGE_TITLE \"Foo is installed\""),
        "{output}"
    );
    assert!(
        output.contains("!define MUI_FINISHPAGE_TEXT \"Thanks.\""),
        "{output}"
    );
    assert!(
        output.contains("!define MUI_FINISHPAGE_BUTTON \"Done\""),
        "{output}"
    );
    // Geometry nobody asked for is geometry MUI2 keeps.
    assert!(!output.contains("TITLE_3LINES"), "{output}");
    assert!(!output.contains("TEXT_LARGE"), "{output}");
}

/// One field holding two, because the second is the room the first is drawn
/// in: a `lines` with no `text` beside it would make space for MUI2's own
/// wording, which is the one case worth having and the only one.
#[test]
fn a_title_can_ask_for_a_third_line() {
    let output = build(&program(
        "page.finish { title = { text = \"Setup is done\", lines = 3 } },",
    ));
    assert!(
        output.contains("!define MUI_FINISHPAGE_TITLE \"Setup is done\""),
        "{output}"
    );
    assert!(
        output.contains("!define MUI_FINISHPAGE_TITLE_3LINES"),
        "{output}"
    );

    // Two is MUI2's own height, so it writes nothing rather than a second name.
    let two = build(&program(
        "page.finish { title = { text = \"x\", lines = 2 } },",
    ));
    assert!(!two.contains("TITLE_3LINES"), "{two}");
}

/// The title box has two heights and no others, so a number MUI2 cannot draw
/// is an error rather than the nearest one it can.
#[test]
fn a_title_has_two_heights_and_no_more() {
    let raised = errors(&program(
        "page.finish { title = { text = \"x\", lines = 4 } },",
    ));
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].0, Code::BadFieldValue);
    assert!(raised[0].1.contains("`lines` wants"), "{raised:?}");
}

/// The body text's second half is a `bool` where the title's is a number,
/// because that is what MUI2 has: one taller box, not a count of lines.
#[test]
fn the_finish_text_can_take_the_taller_box() {
    let output = build(&program(
        "page.finish { text = { text = \"Two things left to do.\", large = true } },",
    ));
    assert!(
        output.contains("!define MUI_FINISHPAGE_TEXT_LARGE"),
        "{output}"
    );

    let small = build(&program(
        "page.finish { text = { text = \"x\", large = false } },",
    ));
    assert!(!small.contains("TEXT_LARGE"), "{small}");
}

/// `reboot` is `Nested` inverted: the table is the on state and MUI2's define
/// is the opt-out, so a page that words the reboot question cannot also have
/// turned the reboot question off.
#[test]
fn the_reboot_half_is_on_unless_it_is_switched_off() {
    let output = build(&program(
        "page.finish { reboot = { text = \"Windows must restart.\", now = \"Restart now\", \
         later = \"Restart later\", default = \"later\" } },",
    ));
    assert!(!output.contains("NOREBOOTSUPPORT"), "{output}");
    assert!(
        output.contains("!define MUI_FINISHPAGE_TEXT_REBOOT \"Windows must restart.\""),
        "{output}"
    );
    assert!(
        output.contains("!define MUI_FINISHPAGE_TEXT_REBOOTNOW \"Restart now\""),
        "{output}"
    );
    assert!(
        output.contains("!define MUI_FINISHPAGE_TEXT_REBOOTLATER \"Restart later\""),
        "{output}"
    );
    assert!(
        output.contains("!define MUI_FINISHPAGE_REBOOTLATER_DEFAULT"),
        "{output}"
    );

    let off = build(&program("page.finish { reboot = false },"));
    assert!(
        off.contains("!define MUI_FINISHPAGE_NOREBOOTSUPPORT"),
        "{off}"
    );
    assert!(!off.contains("TEXT_REBOOT"), "{off}");
}

/// Which radio button starts chosen is named rather than flagged, and the word
/// for MUI2's own choice writes nothing — so a build script can pass either
/// without branching.
#[test]
fn the_reboot_default_is_one_of_two_words() {
    let now = build(&program("page.finish { reboot = { default = \"now\" } },"));
    assert!(!now.contains("REBOOTLATER_DEFAULT"), "{now}");

    let raised = errors(&program(
        "page.finish { reboot = { default = \"maybe\" } },",
    ));
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].0, Code::BadFieldValue);
    assert!(raised[0].1.contains("`default` wants"), "{raised:?}");
}

/// `autoClose` is a block field and not a `page.finish` one: MUI2 reads it in
/// `MUI_FINISHPAGE_GUIINIT`, once per half, so a second finish page could not
/// differ even if it asked. Each half writes its own name.
#[test]
fn auto_close_is_a_block_field_with_a_name_per_half() {
    let output = build(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         installer { autoClose = false, page.finish {} }\n\
         uninstaller { autoClose = false, page.finish {} }\n",
    );
    assert!(
        output.contains("!define MUI_FINISHPAGE_NOAUTOCLOSE"),
        "{output}"
    );
    assert!(
        output.contains("!define MUI_UNFINISHPAGE_NOAUTOCLOSE"),
        "{output}"
    );

    // The default is MUI2's, so the word for it writes nothing.
    let closing = build(&program("autoClose = true,"));
    assert!(!closing.contains("NOAUTOCLOSE"), "{closing}");

    let raised = errors(&program("page.finish { autoClose = false },"));
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].0, Code::UnknownField);
}

/// The short spelling, which is the one most scripts want: a program to start
/// and nothing said about it.
#[test]
fn the_run_checkbox_takes_a_path_on_its_own() {
    let output = build(&program("page.finish { run = INSTDIR .. \"/foo.exe\" },"));
    assert!(
        output.contains("!define MUI_FINISHPAGE_RUN \"$INSTDIR/foo.exe\""),
        "{output}"
    );
    // Nothing else: MUI2 words the checkbox itself and ticks it itself.
    assert!(!output.contains("MUI_FINISHPAGE_RUN_TEXT"), "{output}");
    assert!(
        !output.contains("MUI_FINISHPAGE_RUN_NOTCHECKED"),
        "{output}"
    );
}

#[test]
fn the_run_checkbox_takes_a_path_with_its_trimmings() {
    let output = build(&program(
        "page.finish { run = { path = INSTDIR .. \"/foo.exe\", parameters = \"--first-run\",\n\
         text = \"Run Foo now\", checked = false } },",
    ));
    for expected in [
        "!define MUI_FINISHPAGE_RUN \"$INSTDIR/foo.exe\"",
        "!define MUI_FINISHPAGE_RUN_PARAMETERS \"--first-run\"",
        "!define MUI_FINISHPAGE_RUN_TEXT \"Run Foo now\"",
        "!define MUI_FINISHPAGE_RUN_NOTCHECKED",
    ] {
        assert!(output.contains(expected), "missing {expected}:\n{output}");
    }
}

/// `checked` is MUI2's opt-out read the other way round, so the *ticked* case
/// is the one that writes nothing.
#[test]
fn a_ticked_box_is_the_absence_of_the_define() {
    let output = build(&program(
        "page.finish { run = { path = \"a.exe\", checked = true } },",
    ));
    assert!(!output.contains("NOTCHECKED"), "{output}");

    let raised = errors(&program(
        "page.finish { run = { path = \"a.exe\", checked = \"yes\" } },",
    ));
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].0, Code::BadFieldValue);
}

/// The function spelling writes the checkbox's own define empty, because MUI2
/// draws the box from an `!ifdef` on it and calls the function instead of
/// `Exec`ing anything.
#[test]
fn the_run_checkbox_takes_a_function_instead() {
    let output = build(&program(
        "page.finish { run = { call = function() messageBox(\"hi\") end } },",
    ));
    assert!(
        output.contains("!define MUI_FINISHPAGE_RUN \"\""),
        "{output}"
    );
    assert!(
        output.contains("!define MUI_FINISHPAGE_RUN_FUNCTION \"mui.finish.run\""),
        "{output}"
    );
    assert!(output.contains("Function mui.finish.run"), "{output}");
}

/// The whole point of two part lists rather than one: `parameters` is a member
/// of the path spelling and of no other, so the combination MUI2 ignores has no
/// spelling at all.
#[test]
fn parameters_are_not_a_field_of_the_function_spelling() {
    let raised = errors(&program(
        "page.finish { run = { call = function() end, parameters = \"--x\" } },",
    ));
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].0, Code::UnknownField);
    assert!(raised[0].1.contains("parameters"), "{raised:?}");
}

#[test]
fn a_checkbox_says_one_thing_to_do_or_the_compiler_asks() {
    let neither = errors(&program("page.finish { run = { text = \"Run it\" } },"));
    assert_eq!(neither.len(), 1, "{neither:?}");
    assert_eq!(neither[0].0, Code::BadFieldValue);

    let both = errors(&program(
        "page.finish { run = { path = \"a.exe\", call = function() end } },",
    ));
    assert_eq!(both.len(), 1, "{both:?}");
    assert_eq!(both[0].0, Code::BadFieldValue);
}

/// The two checkboxes are the same shape, and their functions are two
/// functions: `call` is one word under both, and the names must still differ.
#[test]
fn the_readme_checkbox_is_the_run_one_without_parameters() {
    let output = build(&program(
        "page.finish { run = { call = function() end },\n\
         readme = { call = function() end, text = \"View the readme\" } },",
    ));
    assert!(
        output.contains("!define MUI_FINISHPAGE_SHOWREADME_FUNCTION \"mui.finish.readme\""),
        "{output}"
    );
    assert!(
        output.contains("!define MUI_FINISHPAGE_RUN_FUNCTION \"mui.finish.run\""),
        "{output}"
    );
    assert!(
        output.contains("!define MUI_FINISHPAGE_SHOWREADME_TEXT \"View the readme\""),
        "{output}"
    );

    // `ExecShell open` takes no arguments, so this half has no `parameters`.
    let raised = errors(&program(
        "page.finish { readme = { path = \"r.txt\", parameters = \"--x\" } },",
    ));
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].0, Code::UnknownField);
}

/// A label and the place it goes, together — MUI2's click handler execs the
/// location unconditionally, so a link without one is a link to nowhere.
#[test]
fn a_link_is_a_label_and_a_url_and_will_not_be_one_alone() {
    let output = build(&program(
        "page.finish { link = { text = \"Visit foo.org\", url = \"https://foo.org\",\n\
         color = \"0000FF\" } },",
    ));
    for expected in [
        "!define MUI_FINISHPAGE_LINK \"Visit foo.org\"",
        "!define MUI_FINISHPAGE_LINK_LOCATION \"https://foo.org\"",
        "!define MUI_FINISHPAGE_LINK_COLOR \"0000FF\"",
    ] {
        assert!(output.contains(expected), "missing {expected}:\n{output}");
    }

    let missing = errors(&program("page.finish { link = { text = \"Visit\" } },"));
    assert_eq!(missing.len(), 1, "{missing:?}");
    assert_eq!(missing[0].0, Code::MissingAttribute);

    // And no short spelling, because one string cannot be both halves.
    let short = errors(&program("page.finish { link = \"https://foo.org\" },"));
    assert_eq!(short.len(), 1, "{short:?}");
    assert_eq!(short[0].0, Code::BadFieldValue);
}

/// The welcome page's title is the finish page's, and its text is not: MUI2
/// draws this box at a fixed height and has no `…_LARGE` for it.
#[test]
fn the_welcome_page_is_a_title_and_a_body() {
    let output = build(&program(
        "page.welcome { title = { text = \"Welcome to Foo\", lines = 3 },\n\
         text = \"This wizard will install Foo.\" },",
    ));
    for expected in [
        "!define MUI_WELCOMEPAGE_TITLE \"Welcome to Foo\"",
        "!define MUI_WELCOMEPAGE_TITLE_3LINES",
        "!define MUI_WELCOMEPAGE_TEXT \"This wizard will install Foo.\"",
    ] {
        assert!(output.contains(expected), "missing {expected}:\n{output}");
    }

    let raised = errors(&program(
        "page.welcome { text = { text = \"…\", large = true } },",
    ));
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].0, Code::BadFieldValue);
}

/// The fourth hook, and the reason it is not in `COMMON_FIELDS`: only the
/// full-window pages insert `MUI_PAGE_FUNCTION_CUSTOM DESTROYED`.
#[test]
fn the_destroyed_hook_belongs_to_the_pages_that_call_it() {
    let output = build(&program(
        "page.finish { destroyed = function() messageBox(\"gone\") end },",
    ));
    assert!(
        output.contains("!define MUI_PAGE_CUSTOMFUNCTION_DESTROYED \"mui.finish.destroyed\""),
        "{output}"
    );
    assert!(output.contains("Function mui.finish.destroyed"), "{output}");

    let raised = errors(&program("page.directory { destroyed = function() end },"));
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].0, Code::UnknownField);
}

#[test]
fn the_license_page_has_a_top_text_now() {
    let output = build(&program(
        "page.license { file = \"LICENSE\", topText = \"Press Page Down.\" },",
    ));
    assert!(
        output.contains("!define MUI_LICENSEPAGE_TEXT_TOP \"Press Page Down.\""),
        "{output}"
    );
}

/// The install log's two endings, and MUI2's own hole under them: it never
/// unsets the abort pair, so the compiler does.
#[test]
fn the_install_log_is_headed_by_how_it_ended() {
    let output = build(&program(
        "page.instFiles { finishHeaderText = \"Installation complete\",\n\
         finishHeaderSubText = \"Foo is installed.\",\n\
         abortHeaderText = \"Installation aborted\",\n\
         abortHeaderSubText = \"Setup was not completed.\" },",
    ));
    for expected in [
        "!define MUI_INSTFILESPAGE_FINISHHEADER_TEXT \"Installation complete\"",
        "!define MUI_INSTFILESPAGE_FINISHHEADER_SUBTEXT \"Foo is installed.\"",
        "!define MUI_INSTFILESPAGE_ABORTHEADER_TEXT \"Installation aborted\"",
        "!define MUI_INSTFILESPAGE_ABORTHEADER_SUBTEXT \"Setup was not completed.\"",
        "!undef MUI_INSTFILESPAGE_ABORTHEADER_TEXT",
        "!undef MUI_INSTFILESPAGE_ABORTHEADER_SUBTEXT",
    ] {
        assert!(output.contains(expected), "missing {expected}:\n{output}");
    }

    // MUI2 clears the finish pair itself, so the compiler must not.
    assert!(
        !output.contains("!undef MUI_INSTFILESPAGE_FINISHHEADER_TEXT"),
        "{output}"
    );
}

/// The field's presence is `MUI_HEADERIMAGE`, because MUI2 reads every other
/// name in the group inside an `!ifdef` on it.
#[test]
fn a_header_image_is_the_switch_and_the_bitmap_at_once() {
    let output = build(&program("headerImage = \"header.bmp\","));
    assert!(output.contains("!define MUI_HEADERIMAGE\n"), "{output}");
    assert!(
        output.contains("!define MUI_HEADERIMAGE_BITMAP \"header.bmp\""),
        "{output}"
    );

    // `true` is the enable alone: MUI2 then draws the bitmap it ships.
    let bare = build(&program("headerImage = true,"));
    assert!(bare.contains("!define MUI_HEADERIMAGE\n"), "{bare}");
    assert!(!bare.contains("MUI_HEADERIMAGE_BITMAP"), "{bare}");
}

/// The half picks the name, the way it does for `icon` — and the enable is
/// neither half's, so it is written once however many blocks ask for it.
#[test]
fn the_uninstallers_header_image_is_the_un_bitmap() {
    let output = build(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         installer { headerImage = \"header.bmp\" }\n\
         uninstaller { headerImage = \"header-un.bmp\" }\n",
    );
    assert!(
        output.contains("!define MUI_HEADERIMAGE_BITMAP \"header.bmp\""),
        "{output}"
    );
    assert!(
        output.contains("!define MUI_HEADERIMAGE_UNBITMAP \"header-un.bmp\""),
        "{output}"
    );
    assert_eq!(
        output.matches("!define MUI_HEADERIMAGE\n").count(),
        1,
        "the enable is script-wide, so twice is a redefinition:\n{output}"
    );
}

#[test]
fn the_right_to_left_bitmap_carries_its_own_stretch() {
    let output = build(&program(
        "headerImage = { file = \"header.bmp\", stretch = \"AspectFitHeight\",\n\
         rtl = { file = \"header-rtl.bmp\", stretch = \"NoStretchNoCrop\" } },",
    ));
    for expected in [
        "!define MUI_HEADERIMAGE_BITMAP_STRETCH \"AspectFitHeight\"",
        "!define MUI_HEADERIMAGE_BITMAP_RTL \"header-rtl.bmp\"",
        "!define MUI_HEADERIMAGE_BITMAP_RTL_STRETCH \"NoStretchNoCrop\"",
    ] {
        assert!(output.contains(expected), "missing {expected}:\n{output}");
    }

    // A stretch with no bitmap under it is read by nothing.
    let raised = errors(&program(
        "headerImage = { rtl = { stretch = \"FitControl\" } },",
    ));
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].0, Code::MissingAttribute);
}

/// MUI2 warns and falls back on an unknown mode, which is a wrong image at
/// build time. Here it is an error.
#[test]
fn stretch_takes_one_of_muis_four_words() {
    let raised = errors(&program(
        "headerImage = { file = \"h.bmp\", stretch = \"Stretched\" },",
    ));
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].0, Code::BadFieldValue);
    assert!(raised[0].1.contains("stretch"), "{raised:?}");

    for mode in [
        "FitControl",
        "AspectFitHeight",
        "NoStretchNoCrop",
        "NoStretchNoCropNoAlign",
    ] {
        let output = build(&program(&format!(
            "headerImage = {{ file = \"h.bmp\", stretch = \"{mode}\" }},"
        )));
        assert!(
            output.contains(&format!(
                "!define MUI_HEADERIMAGE_BITMAP_STRETCH \"{mode}\""
            )),
            "{output}"
        );
    }
}

/// Two of the six have no `UN` spelling: MUI2 reads them once for the whole
/// script, so the uninstaller's copy would govern both halves.
#[test]
fn the_script_wide_header_switches_are_the_installers() {
    let output = build(&program(
        "headerImage = { file = \"h.bmp\", right = true, transparentText = true },",
    ));
    assert!(output.contains("!define MUI_HEADERIMAGE_RIGHT"), "{output}");
    assert!(
        output.contains("!define MUI_HEADER_TRANSPARENT_TEXT"),
        "{output}"
    );

    let raised = errors(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         uninstaller { headerImage = { file = \"h.bmp\", right = true } }\n",
    );
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].0, Code::UnknownField);
    assert!(raised[0].1.contains("right"), "{raised:?}");
}

/// Two pages read one define, so it is the block's and neither page's.
#[test]
fn the_wizard_image_is_one_field_for_two_pages_and_two_halves() {
    let output = build(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         installer { wizardImage = { file = \"wizard.bmp\", stretch = \"FitControl\" } }\n\
         uninstaller { wizardImage = \"wizard-un.bmp\" }\n",
    );
    for expected in [
        "!define MUI_WELCOMEFINISHPAGE_BITMAP \"wizard.bmp\"",
        "!define MUI_WELCOMEFINISHPAGE_BITMAP_STRETCH \"FitControl\"",
        "!define MUI_UNWELCOMEFINISHPAGE_BITMAP \"wizard-un.bmp\"",
    ] {
        assert!(output.contains(expected), "missing {expected}:\n{output}");
    }

    // There is no enable to hold a table up, so a table with no file is empty.
    let raised = errors(&program("wizardImage = { stretch = \"FitControl\" },"));
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].0, Code::MissingAttribute);
}

/// The description box's three settings, split by MUI2's own scoping. The two
/// texts are `MUI_DEFAULT`ed inside `MUI_PAGEDECLARATION_COMPONENTS` and
/// `MUI_UNSET` after, so they belong to the page and two components pages may
/// differ. `smallDescriptions` is a `ChangeUI` inside the `!ifndef`-guarded
/// interface macro, so it belongs to the block and is read once.
#[test]
fn the_description_box_is_two_page_settings_and_one_block_switch() {
    let output = build(&program(
        "smallDescriptions = true,\n\
         page.components { descriptionTitle = \"Component\", \
         descriptionText = \"Hover one.\" },\n\
         page.components {},",
    ));
    assert!(
        output.contains("!define MUI_COMPONENTSPAGE_SMALLDESC\n"),
        "{output}"
    );
    assert!(
        output.contains(
            "!define MUI_COMPONENTSPAGE_TEXT_DESCRIPTION_TITLE \"Component\"\n\
             !define MUI_COMPONENTSPAGE_TEXT_DESCRIPTION_INFO \"Hover one.\"\n\
             !insertmacro MUI_PAGE_COMPONENTS\n"
        ),
        "{output}"
    );
    // MUI2 unsets both itself, so the second page needs no `!undef` from us and
    // gets the default text rather than the first page's.
    assert!(
        !output.contains("!undef MUI_COMPONENTSPAGE_TEXT_DESCRIPTION"),
        "{output}"
    );
}

/// `smallDescriptions` picks one dialog resource for the whole script, so the
/// two halves cannot disagree — and the message says where to write it instead
/// rather than only that this is the wrong place.
#[test]
fn the_narrow_description_box_is_the_installers_to_choose() {
    let raised = errors(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         uninstaller { smallDescriptions = true, page.components {} }\n",
    );
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].0, Code::UnknownField);
    assert!(
        raised[0].1.contains("not an `uninstaller` field"),
        "{raised:?}"
    );
}

/// The three MUI2 hooks are **entries**, beside `onInit`, and not fields: a
/// block's positional entries are its declarations of code and its named fields
/// are its settings, and a hook is code.
///
/// Each is a define holding the name of a generated function, because the NSIS
/// callback that calls it is MUI2's — `MUI2.nsh` writes `.onGUIInit` and
/// `.onUserAbort` itself, so the define is the only door in.
#[test]
fn the_mui_hooks_are_entries_beside_on_init() {
    let output = build(&program(
        "onInit(function() detailPrint(\"init\") end),\n\
         onGUIInit(function() detailPrint(\"gui\") end),\n\
         onUserAbort(function() detailPrint(\"bye\") end),\n\
         page.instFiles {},",
    ));
    assert!(
        output.contains("!define MUI_CUSTOMFUNCTION_GUIINIT \"mui.onGUIInit\"\n"),
        "{output}"
    );
    assert!(
        output.contains("!define MUI_CUSTOMFUNCTION_ABORT \"mui.onUserAbort\"\n"),
        "{output}"
    );
    assert!(
        output.contains("Function mui.onGUIInit\n  DetailPrint \"gui\"\n"),
        "{output}"
    );
    // `.onInit` is nobody else's, so it is still written directly rather than
    // through a define.
    assert!(output.contains("Function .onInit\n"), "{output}");
}

/// The uninstaller's are three other defines and an `un.` on the function,
/// exactly as `MUI_CUSTOMFUNCTION_UN*` and `un.onGUIInit` spell it.
#[test]
fn each_half_hooks_its_own_callbacks() {
    let output = build(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         installer { onGUIInit(function() end), page.instFiles {},\n\
         section(\"Core\", function() writeUninstaller(INSTDIR .. \"/un.exe\") end) }\n\
         uninstaller { onGUIInit(function() end), onUserAbort(function() end),\n\
         page.instFiles {} }\n",
    );
    assert!(
        output.contains("!define MUI_CUSTOMFUNCTION_UNGUIINIT \"un.mui.onGUIInit\"\n"),
        "{output}"
    );
    assert!(
        output.contains("!define MUI_CUSTOMFUNCTION_UNABORT \"un.mui.onUserAbort\"\n"),
        "{output}"
    );
    assert!(output.contains("Function un.mui.onGUIInit\n"), "{output}");
}

/// `MUI_INSERT` writes the `un.` halves of the two callbacks behind
/// `!ifdef MUI_UNINSTALLER`, which only `MUI_UNPAGE_INIT` sets — so an
/// uninstaller with no page gets a define and a function that nothing calls.
/// Refused, rather than emitted for nobody.
#[test]
fn an_uninstaller_hook_needs_an_uninstaller_page() {
    let raised = errors(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         installer { page.instFiles {},\n\
         section(\"Core\", function() writeUninstaller(INSTDIR .. \"/un.exe\") end) }\n\
         uninstaller { onGUIInit(function() end),\n\
         section(\"Remove\", function() end) }\n",
    );
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].0, Code::MissingAttribute);
    assert!(
        raised[0].1.contains("needs an uninstaller page"),
        "{raised:?}"
    );
}

// -- the start menu page ---------------------------------------------------
//
// The one page bound to a `local`, because `MUI_PAGE_STARTMENU` takes an id and
// a variable and both of them are the compiler's to mint. Everything below is a
// consequence of that: the id is the local, the variable is derived from it, and
// the two `MUI_STARTMENU_*` macros that name the id are reached through the
// handle rather than spelled.

/// A helper for the shape every start menu test needs: the declaration above
/// the block, and the block that lists it.
fn menu(page: &str, installer: &str) -> String {
    format!(
        "attributes {{ outFile = \"a.exe\", name = \"a\" }}\n\
         local menu = page.startMenu {{ {page} }}\n\
         installer {{ menu, page.instFiles {{}}, {installer} }}\n"
    )
}

/// The macro takes two arguments and a script writes neither. The id is the
/// local — already unique, since resolution refuses a second declaration under
/// one name — and the `Var` is minted from it and declared before the page that
/// names it.
#[test]
fn the_page_is_named_by_its_local_and_stores_into_a_var_of_its_own() {
    let output = build(&menu("defaultFolder = \"App\"", ""));
    let declaration = output.find("Var __GENERATED_sm_menu").expect("the Var");
    let insert = output
        .find("!insertmacro MUI_PAGE_STARTMENU menu $__GENERATED_sm_menu")
        .expect("the page");
    assert!(declaration < insert, "{output}");
    assert!(
        output.contains("!define MUI_STARTMENUPAGE_DEFAULTFOLDER \"App\"\n"),
        "{output}"
    );
}

/// Written inline there is no id to give the macro and nothing to address the
/// folder through, so the page would ask a question whose answer is
/// unreachable.
#[test]
fn a_start_menu_page_written_inline_has_no_name_to_be_addressed_by() {
    let raised = errors(&program("page.startMenu { defaultFolder = \"App\" },"));
    assert!(
        raised
            .iter()
            .any(|(code, message)| *code == Code::MissingAttribute
                && message.contains("bound to a local")),
        "{raised:?}"
    );
}

/// There is no `MUI_UNPAGE_STARTMENU`, which is MUI2's fact and not a policy
/// here — so the uninstaller half is refused with the name it does not define.
#[test]
fn there_is_no_uninstaller_start_menu_page() {
    let raised = errors(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         local menu = page.startMenu {}\n\
         uninstaller { menu, page.instFiles {} }\n",
    );
    assert!(
        raised
            .iter()
            .any(|(code, message)| *code == Code::UnknownField
                && message.contains("no uninstaller `startMenu` page")),
        "{raised:?}"
    );
}

/// One field holding three. `StartMenu.nsh` reads all three inside one
/// `!ifdef … & … & …`, so any subset is a define nothing reads.
#[test]
fn the_registry_is_one_field_holding_three() {
    let output = build(&menu(
        "registry = { root = \"HKCU\", key = \"Software\\\\App\", value = \"Folder\" }",
        "",
    ));
    for define in [
        "!define MUI_STARTMENUPAGE_REGISTRY_ROOT \"HKCU\"\n",
        "!define MUI_STARTMENUPAGE_REGISTRY_KEY \"Software\\App\"\n",
        "!define MUI_STARTMENUPAGE_REGISTRY_VALUENAME \"Folder\"\n",
    ] {
        assert!(output.contains(define), "{define}\n{output}");
    }

    let raised = errors(&menu("registry = { root = \"HKCU\" }", ""));
    assert!(
        raised
            .iter()
            .any(|(code, message)| *code == Code::MissingAttribute
                && message.contains("`registry` has no `key`")),
        "{raised:?}"
    );
}

/// Three states and two defines, because MUI2 expands the wording only inside
/// the `!ifndef …_NODISABLE` branch: a script that took the box away and worded
/// it too would write a define nothing reads.
#[test]
fn the_checkbox_is_one_field_that_words_it_or_takes_it_away() {
    let worded = build(&menu("checkbox = \"No shortcuts\"", ""));
    assert!(
        worded.contains("!define MUI_STARTMENUPAGE_TEXT_CHECKBOX \"No shortcuts\"\n"),
        "{worded}"
    );
    assert!(!worded.contains("NODISABLE"), "{worded}");

    let gone = build(&menu("checkbox = false", ""));
    assert!(
        gone.contains("!define MUI_STARTMENUPAGE_NODISABLE\n"),
        "{gone}"
    );
    assert!(!gone.contains("TEXT_CHECKBOX"), "{gone}");

    // MUI2's own box, with MUI2's own words: neither define.
    let stock = build(&menu("checkbox = true", ""));
    assert!(!stock.contains("NODISABLE"), "{stock}");
    assert!(!stock.contains("TEXT_CHECKBOX"), "{stock}");
}

/// `MUI_STARTMENUPAGE_BGCOLOR` and `…_TEXTCOLOR` are refused rather than
/// scheduled, and the reason is a typo in MUI2 3.12: `StartMenu.nsh:141` paints
/// `$mui.StartMenuMenu.FolderList`, a variable nothing declares. The line is
/// reached only when the background is defined, so the page cannot be coloured
/// and assemble under `-WX` at the same time.
#[test]
fn the_start_menu_page_has_no_colours_because_mui2_misspells_the_control() {
    let raised = errors(&menu("colors = { background = \"FFFFFF\" }", ""));
    assert!(
        raised
            .iter()
            .any(|(code, message)| *code == Code::UnknownField
                && message.contains("`colors` is not a `startMenu` page field")),
        "{raised:?}"
    );
}

/// The two macros are one construct because they are useless apart, and the
/// region is lowered **inline**: the closure is written inside a section body
/// and reads that body's locals, which a generated `Function` would put out of
/// scope.
#[test]
fn the_write_region_wraps_the_sections_own_instructions() {
    let output = build(&menu(
        "defaultFolder = \"App\"",
        "section(\"Core\", function()\n\
         menu.write(function() createDirectory(SMPROGRAMS .. \"/\" .. menu.folder) end)\n\
         end),",
    ));
    let begin = output
        .find("!insertmacro MUI_STARTMENU_WRITE_BEGIN menu")
        .expect("the begin");
    let body = output.find("CreateDirectory").expect("the body");
    let end = output
        .find("!insertmacro MUI_STARTMENU_WRITE_END")
        .expect("the end");
    assert!(begin < body && body < end, "{output}");
}

/// One spelling, two lowerings, and the half decides. The installer reads the
/// `Var` its page filled in; the uninstaller never had a page, so it reads the
/// registry through the macro MUI2 wrote for exactly this.
#[test]
fn the_folder_is_a_variable_in_one_half_and_a_registry_read_in_the_other() {
    let output = build(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         local menu = page.startMenu { defaultFolder = \"App\" }\n\
         installer { menu, page.instFiles {},\n\
         section(\"Core\", function()\n\
         writeUninstaller(INSTDIR .. \"/un.exe\")\n\
         menu.write(function() createDirectory(menu.folder) end)\n\
         end) }\n\
         uninstaller { page.instFiles {},\n\
         section(\"Remove\", function() rmDir(menu.folder) end) }\n",
    );
    assert!(
        output.contains("StrCpy $0 $__GENERATED_sm_menu\n"),
        "{output}"
    );
    assert!(
        output.contains("!insertmacro MUI_STARTMENU_GETFOLDER menu $0\n"),
        "{output}"
    );
}

/// A `func` is called by both halves and the two lowerings are not the same
/// code, so there is no answer to pick — and picking wrong is silent, since an
/// empty `Var` copies without complaint.
#[test]
fn a_folder_read_in_a_func_has_no_half_to_decide_it() {
    let raised = errors(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         local menu = page.startMenu {}\n\
         func(\"where\", function() return menu.folder end)\n\
         installer { menu, page.instFiles {} }\n",
    );
    assert!(
        raised
            .iter()
            .any(|(code, message)| *code == Code::NotYetImplemented
                && message.contains("start menu folder read in a `func`")),
        "{raised:?}"
    );
}

/// `…_WRITE_END` writes the chosen folder back to the registry, which is the
/// installer's half of the bargain: the uninstaller has no page to have chosen
/// one. What it wants is the folder, and that is `menu.folder`.
#[test]
fn the_write_region_is_the_installers() {
    let raised = errors(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         local menu = page.startMenu {}\n\
         installer { menu, page.instFiles {},\n\
         section(\"Core\", function() writeUninstaller(INSTDIR .. \"/un.exe\") end) }\n\
         uninstaller { page.instFiles {},\n\
         section(\"Remove\", function() menu.write(function() end) end) }\n",
    );
    assert!(
        raised
            .iter()
            .any(|(code, message)| *code == Code::BadFieldValue
                && message.contains("`write` is the installer's")),
        "{raised:?}"
    );
}

/// Claim rule 1 over the third kind of declaration: a page no block lists is a
/// page order the user wrote nowhere.
#[test]
fn a_start_menu_page_no_block_lists_is_refused() {
    let raised = errors(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         local menu = page.startMenu {}\n\
         installer { page.instFiles {} }\n",
    );
    assert!(
        raised
            .iter()
            .any(|(code, message)| *code == Code::MissingAttribute
                && message.contains("`menu` is a `start menu page` no block lists")),
        "{raised:?}"
    );
}

/// `subCaption` is the one page setting that becomes an NSIS line rather than a
/// MUI2 define, and each page carries its own index into `SubCaption`'s five.
///
/// The indices are the whole of the feature: a wrong one is a caption on the
/// wrong page, which nothing downstream can catch — `makensis` accepts 0 to 4
/// and MUI2 never reads them back.
#[test]
fn a_page_writes_its_own_subcaption_index() {
    let output = build(&program(
        "page.license { file = \"tests/fixtures/assets/license.txt\", subCaption = \"Terms\" },\n\
         page.components { subCaption = \"Pick\" },\n\
         page.directory { subCaption = \"Where\" },\n\
         page.instFiles { subCaption = \"Working\" },",
    ));

    let lines: Vec<&str> = output
        .lines()
        .filter(|line| line.starts_with("SubCaption"))
        .collect();
    assert_eq!(
        lines,
        [
            "SubCaption 0 \"Terms\"",
            "SubCaption 1 \"Pick\"",
            "SubCaption 2 \"Where\"",
            "SubCaption 3 \"Working\"",
        ],
        "in:\n{output}"
    );
}

/// The uninstaller numbers three pages of its own, and neither the command nor
/// the index is the installer's. `instFiles` is 3 in one half and 1 in the
/// other, which is why the field carries a pair rather than a number.
#[test]
fn the_uninstaller_half_has_its_own_numbering() {
    let output = build(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         installer { page.instFiles { subCaption = \"Working\" }, }\n\
         uninstaller {\n\
         page.confirm { subCaption = \"Really?\" },\n\
         page.instFiles { subCaption = \"Removing\" },\n\
         }\n",
    );

    assert!(output.contains("SubCaption 3 \"Working\""), "{output}");
    assert!(
        output.contains("UninstallSubCaption 0 \"Really?\""),
        "{output}"
    );
    assert!(
        output.contains("UninstallSubCaption 1 \"Removing\""),
        "{output}"
    );
}

/// Three of the five installer pages have no uninstaller number, and that is
/// NSIS's arithmetic rather than a decision here: `UninstallSubCaption` counts
/// to 2. A page that has the field in one half and not the other is the first
/// of its kind, so the message says which half and why.
#[test]
fn a_subcaption_with_no_uninstaller_number_is_refused() {
    let raised = errors(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         installer { page.instFiles {} }\n\
         uninstaller {\n\
         page.directory { subCaption = \"Where\" },\n\
         page.instFiles {},\n\
         }\n",
    );
    assert!(
        raised
            .iter()
            .any(|(code, message)| *code == Code::UnknownField
                && message.contains("`subCaption` has no uninstaller half")),
        "{raised:?}"
    );
}

/// Index 4 and its uninstaller twin are MUI2's: `MUI_PAGE_INSTFILES` writes
/// `SubCaption 4 " "` to blank the *Completed* caption. Nothing refuses them,
/// because there is no field to refuse — 3 and 4 are one page in two states and
/// this language names the page.
#[test]
fn the_completed_subcaption_is_mui2s_and_unwritable() {
    let output = build(&program("page.instFiles { subCaption = \"Working\" },"));
    assert!(!output.contains("SubCaption 4"), "{output}");
    assert!(output.contains("SubCaption 3 \"Working\""), "{output}");
}
