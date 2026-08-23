//! `languages { … }` and the dialog that picks between its locales (§15.26).
//!
//! Two things are under test and they pull in opposite directions. The first is
//! the transposition: the source is keyed by locale because a translator owns a
//! locale, and NSIS wants one `LangString` name at a time, so the block a person
//! writes and the lines NSIS reads are transposes of each other. The second is
//! **placement** — four MUI2 macros that each have exactly one legal position,
//! none of them written, and every one of them a `!warning` or a dialog that
//! never opens if it lands somewhere else. That is the include-order problem
//! this block exists to make invisible, so most of what follows asserts *where*
//! a line is rather than that it exists.

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

/// A whole program around `block`, with one page in each half so that the
/// placement assertions have something to be placed after.
fn program(block: &str, body: &str) -> String {
    format!(
        "attributes {{ outFile = \"a.exe\", name = \"a\" }}\n\
         {block}\n\
         installer {{ page.instFiles {{}}, section(\"Core\", function() {body} end), }}\n\
         uninstaller {{ page.instFiles {{}}, section(\"Uninstall\", function() end), }}\n"
    )
}

const TWO: &str = "languages { locales = {\n\
                     English = { greeting = \"Hello\" },\n\
                     German = { greeting = \"Hallo\" },\n\
                   } }";

/// The line number of the first line containing `needle`.
fn at(output: &str, needle: &str) -> usize {
    output
        .lines()
        .position(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("`{needle}` is not in:\n{output}"))
}

// -- the tables ------------------------------------------------------------

/// One `MUI_LANGUAGE` per locale, in the order the source listed them — which
/// is the one thing about this block that is *not* sorted, because NSIS takes
/// the first as the default.
#[test]
fn each_locale_is_one_language_line_in_source_order() {
    let output = build(&program(TWO, ""));
    assert!(at(&output, "MUI_LANGUAGE \"English\"") < at(&output, "MUI_LANGUAGE \"German\""));
}

/// The transposition: written by locale, emitted by name.
#[test]
fn the_strings_are_emitted_by_name_and_not_by_locale() {
    let output = build(&program(
        "languages { locales = {\n\
           English = { greeting = \"Hello\", bye = \"Goodbye\" },\n\
           German = { greeting = \"Hallo\", bye = \"Tschuess\" },\n\
         } }",
        "",
    ));
    assert!(
        at(&output, "LangString bye ${LANG_ENGLISH}")
            < at(&output, "LangString bye ${LANG_GERMAN}")
    );
    assert!(
        at(&output, "LangString bye ${LANG_GERMAN}")
            < at(&output, "LangString greeting ${LANG_ENGLISH}")
    );
}

/// `PortugueseBR` is `${LANG_PORTUGUESEBR}` and not `${LANG_PORTUGUESE_BR}`.
/// Uppercasing is the whole rule, and the awkward names are where a rule that
/// is nearly right stops being right.
#[test]
fn the_define_is_the_name_uppercased() {
    let output = build(&program(
        "languages { locales = { PortugueseBR = { a = \"x\" }, SimpChinese = { a = \"y\" } } }",
        "",
    ));
    assert!(
        output.contains("LangString a ${LANG_PORTUGUESEBR}"),
        "{output}"
    );
    assert!(
        output.contains("LangString a ${LANG_SIMPCHINESE}"),
        "{output}"
    );
}

/// `lang.greeting` is `$(greeting)` — no register, no instruction, and it
/// concatenates like any other piece.
#[test]
fn a_language_string_read_is_a_reference_and_not_a_copy() {
    let output = build(&program(TWO, "detailPrint(\"say: \" .. lang.greeting)"));
    assert!(
        output.contains(r#"DetailPrint "say: $(greeting)""#),
        "{output}"
    );
    assert!(!output.contains("StrCpy"), "{output}");
}

/// Either half may read one. §15.26's `un.` prefixing is a size optimisation
/// and not a boundary — verified against `makensis -WX` both ways — so nothing
/// here stops the uninstaller naming a string the installer also uses.
#[test]
fn both_halves_read_the_same_table() {
    let source = format!(
        "attributes {{ outFile = \"a.exe\", name = \"a\" }}\n\
         {TWO}\n\
         installer {{ page.instFiles {{}}, \
           section(\"Core\", function() detailPrint(lang.greeting) end), }}\n\
         uninstaller {{ page.instFiles {{}}, \
           section(\"Uninstall\", function() detailPrint(lang.greeting) end), }}\n"
    );
    let output = build(&source);
    assert_eq!(
        output.matches("DetailPrint $(greeting)").count(),
        2,
        "{output}"
    );
}

// -- placement, which is the point -----------------------------------------

/// `MUI_LANGUAGE` `!warning`s when it is inserted before the page macros, so
/// the language lines follow every page in both halves whatever order the
/// blocks were written in (§15.3).
#[test]
fn the_language_lines_come_after_every_page() {
    let source = format!(
        "attributes {{ outFile = \"a.exe\", name = \"a\" }}\n\
         uninstaller {{ page.instFiles {{}}, section(\"Uninstall\", function() end), }}\n\
         {TWO}\n\
         installer {{ page.instFiles {{}}, section(\"Core\", function() end), }}\n"
    );
    let output = build(&source);
    assert!(at(&output, "MUI_PAGE_INSTFILES") < at(&output, "MUI_LANGUAGE"));
    assert!(at(&output, "MUI_UNPAGE_INSTFILES") < at(&output, "MUI_LANGUAGE"));
}

/// The block may be written under the code that reads it. Same order-freeness
/// as every other declaration (§15.6), and it is a separate pass that buys it.
#[test]
fn the_block_may_be_written_below_its_readers() {
    let source = format!(
        "attributes {{ outFile = \"a.exe\", name = \"a\" }}\n\
         installer {{ page.instFiles {{}}, \
           section(\"Core\", function() detailPrint(lang.greeting) end), }}\n\
         {TWO}\n"
    );
    assert!(build(&source).contains("DetailPrint $(greeting)"));
}

/// Three macros nobody writes: the plugin reservation after the language lines,
/// the dialog first in `.onInit`, and the uninstaller's lookup first in
/// `un.onInit`. This is the ninth, tenth and eleventh instance of *emitted,
/// never written* (§15.7).
#[test]
fn asking_writes_three_macros_the_source_never_names() {
    let output = build(&program(
        "languages { ask = {}, locales = { English = { a = \"x\" } } }",
        "",
    ));
    assert!(at(&output, "MUI_LANGUAGE \"English\"") < at(&output, "MUI_RESERVEFILE_LANGDLL"));
    assert!(at(&output, "Function .onInit") < at(&output, "MUI_LANGDLL_DISPLAY"));
    assert!(at(&output, "Function un.onInit") < at(&output, "MUI_UNGETLANGUAGE"));
}

/// And the dialog runs before anything a user wrote, because until it has run
/// `$LANGUAGE` is whatever the machine's locale said.
#[test]
fn the_dialog_is_the_first_line_of_on_init() {
    let source = "attributes { outFile = \"a.exe\", name = \"a\" }\n\
                  languages { ask = {}, locales = { English = { a = \"x\" } } }\n\
                  installer { page.instFiles {}, section(\"Core\", function() end),\n\
                    onInit(function() detailPrint(lang.a) end), }\n";
    let output = build(source);
    assert!(
        at(&output, "MUI_LANGDLL_DISPLAY") < at(&output, "DetailPrint $(a)"),
        "{output}"
    );
}

/// No uninstaller, no `un.onInit`. A callback NSIS never calls is not a
/// harmless extra: it is a plugin reservation defended by nothing.
#[test]
fn there_is_no_uninstaller_hook_without_an_uninstaller() {
    let source = "attributes { outFile = \"a.exe\", name = \"a\" }\n\
                  languages { ask = {}, locales = { English = { a = \"x\" } } }\n\
                  installer { page.instFiles {}, section(\"Core\", function() end), }\n";
    let output = build(source);
    assert!(output.contains("MUI_LANGDLL_DISPLAY"), "{output}");
    assert!(!output.contains("un.onInit"), "{output}");
}

/// A program with no `languages {}` still gets a language line, because MUI2
/// `!warning`s without one.
#[test]
fn a_program_with_no_block_still_gets_one_language() {
    let output = build(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         installer { page.instFiles {}, section(\"Core\", function() end), }\n",
    );
    assert_eq!(
        output.matches("!insertmacro MUI_LANGUAGE").count(),
        1,
        "{output}"
    );
}

// -- the settings ----------------------------------------------------------

/// A `false` writes nothing. MUI2 reads these with `!ifdef`, so a define
/// holding the word `false` would be true.
#[test]
fn a_false_flag_is_the_absence_of_the_define() {
    let output = build(&program(
        "languages { ask = { alwaysShow = false }, locales = { English = { a = \"x\" } } }",
        "",
    ));
    assert!(!output.contains("MUI_LANGDLL_ALWAYSSHOW"), "{output}");
}

#[test]
fn remember_writes_all_three_registry_defines() {
    let output = build(&program(
        "languages { ask = { remember = { root = \"HKCU\", key = \"Software\\\\A\", \
           value = \"Language\" } }, locales = { English = { a = \"x\" } } }",
        "",
    ));
    for define in [
        "MUI_LANGDLL_REGISTRY_ROOT",
        "MUI_LANGDLL_REGISTRY_KEY",
        "MUI_LANGDLL_REGISTRY_VALUENAME",
    ] {
        assert!(output.contains(define), "{define} missing from:\n{output}");
    }
}

// -- what is refused -------------------------------------------------------

/// MUI2 guards the stored answer with one `!ifdef` over all three names, so two
/// out of three is the whole feature off without saying so — the same ruling
/// the start menu page's `registry` gets (§15.23).
#[test]
fn a_partial_remember_is_refused_rather_than_half_applied() {
    let errors = errors(&program(
        "languages { ask = { remember = { root = \"HKCU\" } }, \
           locales = { English = { a = \"x\" } } }",
        "",
    ));
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].0, Code::MissingAttribute);
    assert!(errors[0].1.contains("`key`"), "{errors:?}");
}

/// NSIS expands a language string with no entry for the running language to
/// nothing at all, so a gap is an empty label on one machine in one country.
/// The compiler is the only thing that can see it.
#[test]
fn a_string_missing_from_one_locale_is_an_error() {
    let errors = errors(&program(
        "languages { locales = { English = { a = \"x\", b = \"y\" }, German = { a = \"z\" } } }",
        "",
    ));
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].0, Code::MissingAttribute);
    assert!(errors[0].1.contains("`German` has no `b`"), "{errors:?}");
}

/// The key set is the 67 `.nlf` files NSIS ships, checked out of a snapshot so
/// that the answer does not depend on the build machine (§14).
#[test]
fn a_name_nsis_does_not_ship_is_refused_with_the_nearest_one() {
    let errors = errors(&program(
        "languages { locales = { Enlgish = { a = \"x\" } } }",
        "",
    ));
    assert_eq!(errors[0].0, Code::UnknownField);
    assert!(errors[0].1.contains("Enlgish"), "{errors:?}");
}

/// The mistake the rejected shape would have invited, caught by name: a locale
/// written where a field of the block goes.
#[test]
fn a_locale_at_the_top_of_the_block_says_where_it_belongs() {
    let errors = errors(&program("languages { English = { a = \"x\" } }", ""));
    assert_eq!(errors[0].0, Code::UnknownField);
    assert!(
        errors[0].1.contains("`English` is not a field"),
        "{errors:?}"
    );
}

/// `$(name)` for a name nothing declared is `warning 6000` at best and an empty
/// label at worst — indistinguishable from a translation that is simply blank.
#[test]
fn a_string_nothing_declares_is_refused() {
    let errors = errors(&program(TWO, "detailPrint(lang.nosuch)"));
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].0, Code::UnknownField);
    assert!(errors[0].1.contains("nosuch"), "{errors:?}");
}

/// One block, like every other script-global declaration (§15.10).
#[test]
fn a_second_block_names_the_first() {
    let source = format!("{TWO}\n{TWO}\n");
    let errors = errors(&format!(
        "attributes {{ outFile = \"a.exe\", name = \"a\" }}\n{source}"
    ));
    assert_eq!(errors[0].0, Code::DuplicateBlock);
}

/// A `LangString` is chosen by the preprocessor, so its text cannot be
/// computed: there is no run time yet when the table is built (§7-1).
#[test]
fn a_computed_string_is_refused() {
    let errors = errors(&program(
        "languages { locales = { English = { a = INSTDIR } } }",
        "",
    ));
    assert_eq!(errors[0].0, Code::BadFieldValue);
}
