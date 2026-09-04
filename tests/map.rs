//! The line map, one test per origin kind.
//!
//! The requirement is exactly this: *"all of this is a tested surface — the
//! mapping is asserted on, not eyeballed, with one test per origin kind."*
//! There are three kinds, so there are three mapping tests, plus the two
//! `makensis` message syntaxes that the translation has to survive.

use std::path::Path;

use installua::assemble::{self, Message};
use installua::diag::{Diagnostics, Files};
use installua::map::{LineMap, Origin};

/// A program with all three origins in it: an ordinary statement, a `raw`
/// block, and the `!include`/label/`Var` lines nobody wrote.
const PROGRAM: &str = r#"
attributes { outFile = "a.exe" }

seen = ""

installer {
	page.instFiles {},

	section("Core", function()
		detailPrint("ordinary")
		raw [[
			DetailPrint "verbatim"
		]]
		if fileExists("x") then
			detailPrint("branch")
		end
	end),
}
"#;

fn build() -> (String, LineMap) {
    let mut diags = Diagnostics::new();
    let built = installua::build_mapped(PROGRAM, &installua::Options::default(), &mut diags);
    assert!(
        !diags.has_errors(),
        "the program should compile:\n{}",
        diags.render("<test>")
    );
    built.expect("compiles")
}

/// The 1-based output line whose text contains `needle`, and the whole of it.
fn line_of(text: &str, needle: &str) -> usize {
    text.lines()
        .position(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("no line contains {needle:?} in:\n{text}"))
        + 1
}

#[test]
fn every_emitted_line_has_an_origin() {
    let (text, map) = build();
    // Not "roughly as many": a map that drifts by one is worse than no map,
    // because every message it rewrites points at the wrong line.
    assert_eq!(map.len(), text.lines().count());
}

#[test]
fn an_ordinary_line_maps_to_the_statement_that_wrote_it() {
    let (text, map) = build();
    let line = line_of(&text, r#"DetailPrint "ordinary""#);
    let Some(Origin::User(span)) = map.origin(line) else {
        panic!("expected a user origin, got {:?}", map.origin(line));
    };
    // `detailPrint("ordinary")` is on line 10 of `PROGRAM`, counting the
    // leading newline the raw string starts with.
    assert_eq!(span.start_line, 10);
}

#[test]
fn a_raw_line_maps_to_its_block_and_says_it_is_unchecked() {
    let (text, map) = build();
    let line = line_of(&text, r#"DetailPrint "verbatim""#);
    let Some(Origin::Raw(span)) = map.origin(line) else {
        panic!("expected a raw origin, got {:?}", map.origin(line));
    };
    assert_eq!(span.start_line, 11);

    let message = Message {
        line: Some(line),
        text: "Error in script \"a.nsi\" on line 12 -- aborting creation process".to_string(),
        cause: None,
    };
    let rendered = assemble::translate(
        &message,
        &map,
        "install.lua",
        &Files::default(),
        Path::new("a.nsi"),
    );
    assert_eq!(
        rendered,
        "install.lua:11:3: error[makensis]: Error in script \"a.nsi\" on line 12 -- aborting \
         creation process\n  note: this line is inside a `raw` block, which nothing in this \
         compiler checked"
    );
}

#[test]
fn a_generated_line_is_reported_as_a_compiler_bug() {
    let (text, map) = build();
    let line = line_of(&text, "!include");
    assert_eq!(map.origin(line), Some(&Origin::Emitted("!include")));

    let message = Message {
        line: Some(line),
        text: "Error in script \"a.nsi\" on line 6 -- aborting creation process".to_string(),
        cause: None,
    };
    let rendered = assemble::translate(
        &message,
        &map,
        "install.lua",
        &Files::default(),
        Path::new("out/a.nsi"),
    );
    assert!(
        rendered.starts_with(
            "error: makensis rejected a line Installua generated (!include, out/a.nsi:"
        ),
        "a generated line has to say whose bug it is, got:\n{rendered}"
    );
    assert!(rendered.contains("this is a compiler bug"), "{rendered}");
    assert!(rendered.contains("was kept at out/a.nsi"), "{rendered}");
}

/// The three lines nobody wrote, each landing on the label named for it.
#[test]
fn the_compilers_own_lines_are_labelled_as_its_own() {
    let (text, map) = build();
    for (needle, what) in [
        ("Unicode true", "Unicode"),
        ("Var seen", "Var"),
        ("!insertmacro MUI_PAGE_INSTFILES", "page"),
        // The label itself, not the branch that names it: a branch line is
        // the `if` the user wrote, and only the label is nobody's.
        ("endif_0:", "label"),
    ] {
        let line = line_of(&text, needle);
        assert_eq!(
            map.origin(line),
            Some(&Origin::Emitted(what)),
            "{needle} should be an emitted {what}"
        );
    }
}

/// Every syntax verified against NSIS 3.12, and the line-less class.
#[test]
fn every_makensis_syntax_is_recognised() {
    let log = concat!(
        "Processing script file: \"a.nsi\"\n",
        "Error in script \"a.nsi\" on line 4 -- aborting creation process\n",
        "warning 6000: unknown variable/constant \"NOPE\" detected (a.nsi:3)\n",
        "Error: could not resolve label \"nowhere\" in unnamed install section (0)\n",
        "Total size: 1 byte\n",
    );
    let parsed = assemble::parse(log);
    assert_eq!(
        parsed.iter().map(|m| m.line).collect::<Vec<_>>(),
        vec![Some(4), Some(3), None]
    );
    // The banner is not a cause: the cursor above it is the first thing in the
    // log, which is what a failure on line 1 looks like.
    assert_eq!(parsed[0].cause, None);
}

/// The two shapes that were hit while porting, verbatim from `makensis -WX`
/// 3.12.
///
/// Both are the user's own mistake and both used to arrive as *"aborting
/// creation process"* — the complaint sits on the line before the cursor, in
/// no syntax at all, and was dropped. The `Error in script` line is where
/// `makensis` stopped, never what it objected to.
#[test]
fn a_cursor_carries_the_line_before_it_as_its_cause() {
    for (log, cause) in [
        (
            concat!(
                "Processing script file: \"i.nsi\" (UTF8)\n",
                "Error while loading icon from \"nope.ico\": can't open file\n",
                "Error in script \"i.nsi\" on line 2 -- aborting creation process\n",
            ),
            "Error while loading icon from \"nope.ico\": can't open file",
        ),
        (
            concat!(
                "Processing script file: \"l.nsi\" (UTF8)\n",
                "!define: \"X\" already defined!\n",
                "Error in script \"l.nsi\" on line 2 -- aborting creation process\n",
            ),
            "!define: \"X\" already defined!",
        ),
    ] {
        let parsed = assemble::parse(log);
        // One diagnostic, not two: the cause is folded in rather than reported
        // as a second unpositioned failure.
        assert_eq!(parsed.len(), 1, "{log}");
        assert_eq!(parsed[0].cause.as_deref(), Some(cause));
    }
}

/// A generated line is only a compiler bug when it is what `makensis`
/// objected to.
///
/// This is the one that sent the user to file an issue over their own missing
/// file. The cursor lands on a line nobody wrote, so the old report blamed the
/// compiler — with the cause dropped, there was nothing in it to say
/// otherwise.
#[test]
fn a_cause_on_a_generated_line_is_not_a_compiler_bug() {
    let (text, map) = build();
    let line = line_of(&text, "!include");
    let message = Message {
        line: Some(line),
        text: "Error in script \"a.nsi\" on line 6 -- aborting creation process".to_string(),
        cause: Some("Error while loading icon from \"nope.ico\": can't open file".to_string()),
    };
    let rendered = assemble::translate(
        &message,
        &map,
        "install.lua",
        &Files::default(),
        Path::new("out/a.nsi"),
    );
    assert!(
        rendered.starts_with(
            "error[makensis]: Error while loading icon from \"nope.ico\": can't open file"
        ),
        "the cause is the message, got:\n{rendered}"
    );
    assert!(
        !rendered.contains("compiler bug"),
        "the user's missing file is not one, got:\n{rendered}"
    );
    assert!(
        rendered.contains("stopped at !include, a line Installua generated (out/a.nsi:"),
        "the generated line is still named, as where and not as what: {rendered}"
    );
}

/// The same for a line the user wrote: the useless cursor text is replaced,
/// and the position is still theirs.
#[test]
fn a_cause_replaces_the_cursor_text_on_a_user_line() {
    let (text, map) = build();
    let line = line_of(&text, r#"DetailPrint "ordinary""#);
    let message = Message {
        line: Some(line),
        text: "Error in script \"a.nsi\" on line 12 -- aborting creation process".to_string(),
        cause: Some("Invalid command: \"Foobar\"".to_string()),
    };
    let rendered = assemble::translate(
        &message,
        &map,
        "install.lua",
        &Files::default(),
        Path::new("a.nsi"),
    );
    assert_eq!(
        rendered,
        "install.lua:10:3: error[makensis]: Invalid command: \"Foobar\""
    );
}

/// A message with no line is reported rather than attributed. Link-time errors
/// name a section and not a position, and inventing one is worse than saying
/// there is none.
#[test]
fn a_message_with_no_line_keeps_the_script() {
    let (_, map) = build();
    let message = Message {
        line: None,
        text: "Error: could not resolve label \"nowhere\" in unnamed install section (0)"
            .to_string(),
        cause: None,
    };
    let rendered = assemble::translate(
        &message,
        &map,
        "install.lua",
        &Files::default(),
        Path::new("a.nsi"),
    );
    assert_eq!(
        rendered,
        "error[makensis]: Error: could not resolve label \"nowhere\" in unnamed install section \
         (0)\n  note: this message names no line; the generated script was kept at a.nsi"
    );
}
