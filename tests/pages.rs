//! The page surface (§15.7): where a MUI2 setting lives, and what it costs to
//! put it in the wrong place.
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

/// The uninstaller's half has no spelling of its own (§15.3): the block the
/// page is written in supplies both the `MUI_UNPAGE_` prefix and the `un.` on
/// the functions MUI2 will call.
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
