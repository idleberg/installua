//! A license page whose text is translated — `LicenseLangString`.
//!
//! NSIS keeps the license out of the language tables it keeps everything else
//! in: a `LangString` holds a string, a `LicenseLangString` holds a *file*, and
//! the page reads the second through the same `$(…)` as the first. So the
//! surface is not a new page and not a new field. It is a second shape for the
//! one field a license page already has:
//!
//! ```lua
//! page.license { file = { English = "en.txt", German = "de.txt" } }
//! ```
//!
//! Most of what follows asserts the **set**, not the lines. A locale declared in
//! `languages {}` with no license file expands to nothing, which is a blank
//! license page rather than an error — and NSIS says nothing about it, which is
//! the whole reason to check it here.

use std::path::Path;
use std::process::Command;

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

/// A program with `languages` above it and `pages` in the installer.
fn program(languages: &str, pages: &str) -> String {
    format!(
        "attributes {{ outFile = \"a.exe\", name = \"a\" }}\n\
         {languages}\n\
         installer {{ {pages} page.instFiles {{}}, section(\"Core\", function() end), }}\n"
    )
}

const TWO: &str = "languages { locales = {\n\
                     English = { greeting = \"Hello\" },\n\
                     German = { greeting = \"Hallo\" },\n\
                   } }";

const BOTH: &str = "page.license { file = { English = \"en.txt\", German = \"de.txt\" } },";

/// The line number of the first line containing `needle`.
fn at(output: &str, needle: &str) -> usize {
    output
        .lines()
        .position(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("`{needle}` is not in:\n{output}"))
}

// -- the lines -------------------------------------------------------------

/// One line per locale, and a page that reads them by name rather than by path.
#[test]
fn a_license_per_locale_is_one_line_per_locale() {
    let output = build(&program(TWO, BOTH));
    assert!(
        output.contains("LicenseLangString licenseData ${LANG_ENGLISH} \"en.txt\""),
        "{output}"
    );
    assert!(
        output.contains("LicenseLangString licenseData ${LANG_GERMAN} \"de.txt\""),
        "{output}"
    );
    assert!(
        output.contains("!insertmacro MUI_PAGE_LICENSE $(licenseData)"),
        "{output}"
    );
}

/// Source order, because the first locale is the language NSIS falls back to
/// and a reader comparing the two blocks should find them in the same order.
#[test]
fn the_lines_are_in_declaration_order() {
    let output = build(&program(TWO, BOTH));
    assert!(at(&output, "${LANG_ENGLISH} \"en.txt\"") < at(&output, "${LANG_GERMAN} \"de.txt\""));
}

/// Placement, which is the half a golden cannot state as a fact. Each line
/// names a `${LANG_…}` that the `MUI_LANGUAGE` above it defines; the page that
/// reads the name is further up still, and that is legal, because a language
/// string is resolved when the tables are written rather than where it is
/// mentioned.
#[test]
fn the_lines_follow_the_language_they_are_filed_under() {
    let output = build(&program(TWO, BOTH));
    assert!(at(&output, "MUI_PAGE_LICENSE") < at(&output, "MUI_LANGUAGE \"English\""));
    assert!(at(&output, "MUI_LANGUAGE \"German\"") < at(&output, "LicenseLangString"));
}

/// The name is the compiler's, so two license pages do not file their files
/// under one name and silently take the second one's.
#[test]
fn two_license_pages_get_two_names() {
    let output = build(&program(
        TWO,
        "page.license { file = { English = \"en.txt\", German = \"de.txt\" } },\n\
         page.license { file = { English = \"eula-en.txt\", German = \"eula-de.txt\" } },",
    ));
    assert!(
        output.contains("LicenseLangString licenseData2 ${LANG_ENGLISH} \"eula-en.txt\""),
        "{output}"
    );
    assert!(
        output.contains("!insertmacro MUI_PAGE_LICENSE $(licenseData2)"),
        "{output}"
    );
}

/// The shape that was there before this existed, unchanged: one path is still
/// the macro's argument and writes no language table at all.
#[test]
fn one_path_is_still_one_path() {
    let output = build(&program(TWO, "page.license { file = \"LICENSE.txt\" },"));
    assert!(
        output.contains("!insertmacro MUI_PAGE_LICENSE \"LICENSE.txt\""),
        "{output}"
    );
    assert!(!output.contains("LicenseLangString"), "{output}");
}

// -- the set ---------------------------------------------------------------

/// A declared locale with no license file. NSIS is silent about this and the
/// symptom is a blank page in one country, which is why it is an error here.
#[test]
fn a_locale_with_no_license_file_is_refused() {
    let raised = errors(&program(
        TWO,
        "page.license { file = { English = \"en.txt\" } },",
    ));
    assert!(
        raised
            .iter()
            .any(|(code, text)| *code == Code::MissingAttribute
                && text.contains("`German` has no license file")),
        "{raised:?}"
    );
}

/// And the other direction, which fails louder but no more usefully: NSIS
/// reports an undefined `${LANG_FRENCH}` against a line the user never wrote.
#[test]
fn a_license_file_for_an_undeclared_locale_is_refused() {
    let raised = errors(&program(
        TWO,
        "page.license { file = { English = \"en.txt\", German = \"de.txt\", French = \"fr.txt\" } },",
    ));
    assert!(
        raised.iter().any(|(code, text)| *code == Code::UnknownField
            && text.contains("`French` is not one of this program's languages")),
        "{raised:?}"
    );
}

/// A key that is no language at all gets the block's own diagnostic, spelling
/// included — the same one `languages { locales }` raises, because it is the
/// same mistake in a second place.
#[test]
fn a_key_that_is_not_a_language_is_refused() {
    let raised = errors(&program(
        TWO,
        "page.license { file = { English = \"en.txt\", German = \"de.txt\", Deutsch = \"de.txt\" } },",
    ));
    assert!(
        raised
            .iter()
            .any(|(_, text)| text.contains("`Deutsch` is not a language NSIS ships")),
        "{raised:?}"
    );
}

// -- tier 3 ----------------------------------------------------------------

/// Only `makensis` can answer the question this feature turns on: whether a
/// `MUI_PAGE_LICENSE` reading `$(licenseData)` may stand *above* the
/// `LicenseLangString` lines that define it. Nothing in the compiler knows
/// that, and the placement is not negotiable in the other direction — the
/// `${LANG_…}` each line names is defined by the `MUI_LANGUAGE` above it, and
/// every page macro has to precede every one of those.
///
/// Skips cleanly when there is no `makensis`, the same trade the tests make
/// everywhere else.
#[test]
fn a_translated_license_assembles_under_wx() {
    let Some(makensis) = makensis() else {
        eprintln!("skipping: `makensis` is not installed");
        return;
    };

    // One file under two locales: what is under test is the shape of the
    // output, and a second fixture would only be a second copy of the first.
    let built = build(&program(
        TWO,
        "page.license { file = {\n\
           English = \"assets/license.txt\",\n\
           German = \"assets/license.txt\",\n\
         } },",
    ));

    // `LicenseLangString` resolves relative to the script, so the script goes
    // where the fixtures are — see tests/fixtures/README.md.
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures");
    let script = fixtures.join("license.nsi");
    std::fs::write(&script, &built).expect("write the script");

    let output = Command::new(&makensis)
        .arg("-WX")
        .arg(&script)
        .current_dir(&fixtures)
        .output()
        .expect("run makensis");

    let _ = std::fs::remove_file(&script);
    let _ = std::fs::remove_file(fixtures.join("a.exe"));

    assert!(
        output.status.success(),
        "makensis -WX rejected a translated license:\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn makensis() -> Option<String> {
    let name = std::env::var("MAKENSIS").unwrap_or_else(|_| "makensis".to_string());
    Command::new(&name)
        .arg("-VERSION")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|_| name)
}

/// Without a `languages {}` there is no `${LANG_…}` to file anything under, so
/// the table is not a translation — it is a program that does not know what
/// languages it has.
#[test]
fn a_per_locale_license_needs_a_languages_block() {
    let raised = errors(&program(
        "",
        "page.license { file = { English = \"en.txt\" } },",
    ));
    assert!(
        raised
            .iter()
            .any(|(code, text)| *code == Code::MissingAttribute
                && text.contains("needs a `languages {}` block")),
        "{raised:?}"
    );
}
