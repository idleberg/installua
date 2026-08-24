//! `InitPluginsDir`, which the compiler writes and no program spells (§11).
//!
//! `$PLUGINSDIR` is not a fact about the machine the way `$WINDIR` is. It names
//! a temporary directory that does not exist until something creates it, and
//! until then it expands to **nothing** — so `setOutPath(PLUGINSDIR)` in a
//! program that forgot the line is `SetOutPath ""`, which `makensis -WX`
//! assembles without a word and which writes the files somewhere else on
//! somebody else's machine.
//!
//! So the rule is a fact about bodies rather than about statements: a body that
//! names the directory opens with the line. What follows asserts the *placement*
//! and, at least as much, the bodies that do **not** get one — a pass that
//! cannot be wrong in the quiet direction is not paying for itself.

use std::path::Path;
use std::process::Command;

use installua::diag::Diagnostics;

/// Compiles, and refuses anything the compiler had an error about. Warnings are
/// allowed through: one test below is *about* a warning.
fn build(source: &str) -> String {
    let mut diags = Diagnostics::new();
    let output = installua::build(source, &mut diags);
    assert!(!diags.has_errors(), "{}", diags.render("<test>"));
    output.expect("compiles")
}

/// A program whose installer is `body`, plus the one page every program needs.
fn program(body: &str) -> String {
    format!(
        "attributes {{ outFile = \"a.exe\", name = \"a\" }}\n\
         installer {{ page.instFiles {{}}, {body} }}\n"
    )
}

/// The lines of one `Section` or `Function`, without its opening and closing
/// line — which is the unit the whole pass is about.
fn body<'a>(output: &'a str, opens: &str) -> Vec<&'a str> {
    let start = output
        .lines()
        .position(|line| line.trim() == opens)
        .unwrap_or_else(|| panic!("`{opens}` is not in:\n{output}"));
    output
        .lines()
        .skip(start + 1)
        .take_while(|line| !matches!(line.trim(), "SectionEnd" | "FunctionEnd"))
        .map(str::trim)
        .collect()
}

// -- the line --------------------------------------------------------------

/// First, and before the statement that needed it: every later mention is
/// covered by the same line, and a body has exactly one place that is ahead of
/// all of them.
#[test]
fn a_body_that_names_the_directory_opens_with_the_line() {
    let output = build(&program(
        "section(\"Core\", function()\n\
           setOutPath(PLUGINSDIR)\n\
           detailPrint(PLUGINSDIR .. \"/splash\")\n\
         end),",
    ));
    assert_eq!(
        body(&output, "Section \"Core\""),
        vec![
            "InitPluginsDir",
            "SetOutPath $PLUGINSDIR",
            "DetailPrint \"$PLUGINSDIR/splash\"",
        ],
        "{output}"
    );
}

/// The half that keeps the pass honest. A program that never mentions the
/// directory pays nothing — no line, and no temporary directory created on a
/// user's machine for a feature it does not use.
#[test]
fn a_body_that_does_not_name_it_gets_nothing() {
    let output = build(&program(
        "section(\"Core\", function() detailPrint(\"plain\") end),",
    ));
    assert!(!output.contains("InitPluginsDir"), "{output}");
}

/// Per body and not per program: two sections that both name it get one line
/// each, because NSIS defines the second as a no-op and a shared one would need
/// an argument about which body runs first.
#[test]
fn each_body_that_names_it_gets_its_own() {
    let output = build(&program(
        "section(\"One\", function() setOutPath(PLUGINSDIR) end),\n\
         section(\"Two\", function() setOutPath(PLUGINSDIR) end),",
    ));
    assert_eq!(output.matches("InitPluginsDir").count(), 2, "{output}");
}

/// The line goes where the *mention* is. A caller that only calls a function
/// naming the directory gets nothing: the callee opens with its own, so the
/// directory exists by the time the mention is reached, and a second line in the
/// caller would be one the program never needed.
#[test]
fn the_line_follows_the_mention_and_not_the_call() {
    let output = build(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         func(\"splash\", function() setOutPath(PLUGINSDIR) end)\n\
         installer { page.instFiles {}, section(\"Core\", function() splash() end), }\n",
    );
    assert_eq!(body(&output, "Function splash")[0], "InitPluginsDir");
    assert!(
        !body(&output, "Section \"Core\"").contains(&"InitPluginsDir"),
        "{output}"
    );
}

/// And the other direction: a caller that hands the directory *to* a function
/// has read it itself, so the caller is where the line belongs — the callee only
/// ever sees a register.
#[test]
fn a_caller_that_passes_it_has_read_it() {
    let output = build(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         func(\"show\", function(where) detailPrint(where) end)\n\
         installer { page.instFiles {}, section(\"Core\", function() show(PLUGINSDIR) end), }\n",
    );
    assert_eq!(body(&output, "Section \"Core\"")[0], "InitPluginsDir");
    assert!(
        !body(&output, "Function show").contains(&"InitPluginsDir"),
        "{output}"
    );
}

// -- what counts as naming it ----------------------------------------------

/// `raw` is the one place a `$PLUGINSDIR` arrives without having passed through
/// the constants table, and it is exactly the text nothing else in this compiler
/// checked. An escape hatch that skipped this pass would be an escape hatch into
/// the failure the pass exists to prevent.
#[test]
fn a_raw_block_is_read_too() {
    let output = build(&program(
        "section(\"Core\", function()\n\
           raw [[\n\
             File /oname=$PLUGINSDIR\\x.ini \"assets\\license.txt\"\n\
           ]]\n\
         end),",
    ));
    assert_eq!(
        body(&output, "Section \"Core\"")[0],
        "InitPluginsDir",
        "{output}"
    );
}

/// A `$` in a string literal is five dollars, not a variable (§15.1) — the
/// emitter doubles it, so the line reads the directory no more than any other
/// sentence does. The compiler already warns about the habit; what is asserted
/// here is that the warning is not also a plugins directory.
#[test]
fn a_literal_dollar_is_not_a_read() {
    let output = build(&program(
        "section(\"Core\", function() detailPrint(\"$PLUGINSDIR\") end),",
    ));
    assert!(output.contains("DetailPrint \"$$PLUGINSDIR\""), "{output}");
    assert!(!output.contains("InitPluginsDir"), "{output}");
}

// -- tier 3 ----------------------------------------------------------------

/// The line is legal where the pass puts it — including ahead of the `Pop`s a
/// function opens with, which is the one placement question the compiler cannot
/// answer on its own. NSIS itself inserts an implicit init at a plugin call,
/// where arguments are already on the stack, so the instruction has to leave the
/// stack alone; this is that read of the documentation, checked.
#[test]
fn a_program_that_uses_the_directory_assembles_under_wx() {
    let Some(makensis) = makensis() else {
        eprintln!("skipping: `makensis` is not installed");
        return;
    };

    let built = build(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         func(\"splash\", function(name)\n\
           setOutPath(PLUGINSDIR)\n\
           file(\"assets/license.txt\")\n\
           detailPrint(PLUGINSDIR .. \"/\" .. name)\n\
         end)\n\
         installer { page.instFiles {},\n\
           section(\"Core\", function() splash(\"license.txt\") end), }\n",
    );

    // `File` resolves relative to the script, so the script goes where the
    // fixtures are — see tests/fixtures/README.md.
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures");
    let script = fixtures.join("plugins-dir.nsi");
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
        "makensis -WX rejected a generated InitPluginsDir:\n{}{}",
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
