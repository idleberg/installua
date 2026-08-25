//! Top-level `raw`, and the anchor that says where it goes.
//!
//! The interesting half is not that the text comes out — a `raw` block has
//! always come out — but *where*. `head` is above every line the compiler
//! writes, `Unicode` included, and that is the one position in the file whose
//! contents the compiler has not read. So tier 3 carries more weight here than
//! anywhere else: text at `head` can break `makensis` in ways no comparison of
//! strings would notice, and a `head` block that assembles is the only evidence
//! that the slot is where the plan says it is.

use std::process::Command;

use installua::diag::{Code, Diagnostics};

/// Enough of a program to have a spine to be positioned against.
fn program(top: &str) -> String {
    format!(
        "{top}\n\
         attributes {{ name = \"A\", outFile = \"a.exe\" }}\n\
         installer {{\n\
           page.instFiles {{}},\n\
           section(\"Core\", function() detailPrint(\"x\") end),\n\
         }}\n"
    )
}

fn build(source: &str) -> String {
    let mut diags = Diagnostics::new();
    let output = installua::build(source, &mut diags);
    assert!(diags.is_empty(), "{}", diags.render("<test>"));
    output.expect("it compiles")
}

fn errors(source: &str) -> Diagnostics {
    let mut diags = Diagnostics::new();
    let _ = installua::build(source, &mut diags);
    assert!(!diags.is_empty(), "no diagnostic");
    diags
}

fn raised(diags: &Diagnostics, code: Code) -> bool {
    diags.iter().any(|diag| diag.code == code)
}

/// The whole promise of `head`, and the reason it is not simply "first of the
/// user's lines": it precedes `Unicode`, which is otherwise the first line of
/// every program this compiler emits.
#[test]
fn head_is_above_unicode() {
    let output = build(&program("raw.head [[ !system 'echo hi' ]]"));
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "!system 'echo hi'");
    assert_eq!(lines[2], "Unicode true");
}

/// And `tail` is the last line, past the functions.
#[test]
fn tail_is_below_everything() {
    let output = build(&program(
        "raw.tail [[ !packhdr \"tmp.dat\" '\"upx.exe\" \"tmp.dat\"' ]]",
    ));
    assert_eq!(
        output.lines().last(),
        Some("!packhdr \"tmp.dat\" '\"upx.exe\" \"tmp.dat\"'")
    );
}

/// A program with no anchor is byte-for-byte the program it was before the
/// anchors existed — `Unicode` still leads, with no blank line ahead of it.
#[test]
fn no_anchor_costs_nothing() {
    let output = build(&program(""));
    assert_eq!(output.lines().next(), Some("Unicode true"));
}

/// The text is not read, which is what `raw` promises everywhere. Leading
/// whitespace goes so the block sits where the emitter's indentation puts
/// everything else, and blank lines go with it; nothing else is touched.
#[test]
fn the_block_is_emitted_line_by_line_and_unread() {
    let output = build(&program(
        "raw.head [[\n  !tempfile REV\n\n  !system 'git rev-parse HEAD > \"${REV}\"'\n]]",
    ));
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "!tempfile REV");
    assert_eq!(lines[1], "!system 'git rev-parse HEAD > \"${REV}\"'");
}

/// The `$` doubling a string literal gets is exactly what a `raw` block must
/// not get, and the anchored form goes through a different lift path than the
/// body form — so it is asserted rather than assumed.
#[test]
fn a_dollar_survives_an_anchored_block() {
    let output = build(&program("raw.head [[ !echo \"$EXEDIR\" ]]"));
    assert_eq!(output.lines().next(), Some("!echo \"$EXEDIR\""));
}

/// Two blocks at one anchor are one run, in the order they were written.
#[test]
fn blocks_at_one_anchor_keep_their_order() {
    let output = build(&program(
        "raw.head [[ !echo \"first\" ]]\nraw.head [[ !echo \"second\" ]]",
    ));
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "!echo \"first\"");
    assert_eq!(lines[1], "!echo \"second\"");
}

/// A `makensis` complaint about anchored text is reported against the Lua that
/// wrote it — the same contract a body's `raw` has. Asserted through the line
/// map rather than through `makensis`, which is what makes it a tier-1 test.
#[test]
fn an_anchored_line_maps_back_to_its_source() {
    use installua::map::Origin;

    let source = program("raw.head [[ !echo \"hi\" ]]");
    let mut diags = Diagnostics::new();
    let options = installua::Options::default();
    let (_, map) = installua::build_mapped(&source, &options, &mut diags).expect("it compiles");
    assert!(diags.is_empty(), "{}", diags.render("<test>"));

    assert!(
        matches!(map.origin(1), Some(Origin::Raw(_))),
        "the first line is not the user's: {:?}",
        map.origin(1)
    );
}

/// What the two forms are for, stated as a test: a body has a "here" and needs
/// no anchor, the top level has none and cannot be given a default.
mod form {
    use super::*;

    #[test]
    fn a_top_level_block_without_an_anchor_names_both() {
        let diags = errors(&program("raw [[ !echo \"hi\" ]]"));
        assert!(raised(&diags, Code::RawAnchor), "{}", diags.render("<t>"));
        let text = diags.render("<t>");
        assert!(text.contains("raw.head"), "{text}");
        assert!(text.contains("raw.tail"), "{text}");
    }

    /// And an anchor that is not one says which are, rather than reporting
    /// `raw.middle` as a name nobody defined.
    #[test]
    fn an_anchor_that_is_not_one_names_the_two_that_are() {
        let diags = errors(&program("raw.middle [[ !echo \"hi\" ]]"));
        assert!(raised(&diags, Code::RawAnchor), "{}", diags.render("<t>"));
        assert!(diags.render("<t>").contains("`head` and `tail`"));
    }

    /// The other direction: in a body the anchor answers a question nobody
    /// asked, and `raw [[ … ]]` already lands where it is written.
    #[test]
    fn an_anchor_in_a_body_is_an_error() {
        let source = "\
attributes { name = \"A\", outFile = \"a.exe\" }
installer {
  page.instFiles {},
  section(\"Core\", function() raw.head [[ !echo \"hi\" ]] end),
}
";
        let diags = errors(source);
        assert!(raised(&diags, Code::RawAnchor), "{}", diags.render("<t>"));
    }

    /// A computed string would be text the compiler assembled and did not read,
    /// which is neither half of what `raw` is.
    #[test]
    fn the_block_has_to_be_a_literal() {
        let diags = errors(&program("local X <const> = \"!echo x\"\nraw.head(X)"));
        assert!(raised(&diags, Code::WrongArity), "{}", diags.render("<t>"));
    }

    /// `$PLUGINSDIR` is the invariant `lower::plugins_dir` holds, at the one
    /// position that pass cannot reach: the directory is made by an
    /// `InitPluginsDir` inserted above the statement that names it, and an
    /// anchor is outside every body. Diagnosed rather than scanned, because
    /// there is nowhere here for the fix to go.
    #[test]
    fn the_plugins_directory_cannot_be_named_at_an_anchor() {
        let diags = errors(&program("raw.head [[ !echo \"$PLUGINSDIR\" ]]"));
        assert!(raised(&diags, Code::RawAnchor), "{}", diags.render("<t>"));
        assert!(diags.render("<t>").contains("InitPluginsDir"));
    }
}

/// Tier 3. `!system` and `!tempfile` above a `Unicode` line assemble clean —
/// that is the fact slot 0 rests on, and it is checked rather than remembered.
#[test]
fn the_anchors_assemble_under_wx() {
    let Some(makensis) = makensis() else {
        eprintln!("skipping: `makensis` is not installed");
        return;
    };

    let directory = std::env::temp_dir().join(format!("installua-anchors-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("temp directory");
    let script = directory.join("anchors.nsi");
    std::fs::write(
        &script,
        build(&program(
            "raw.head [[\n\
               !tempfile STAMP\n\
               !system 'echo built > \"${STAMP}\"'\n\
             ]]\n\
             raw.tail [[ !finalize 'echo done' ]]",
        )),
    )
    .expect("write the script");

    let output = Command::new(makensis)
        .arg("-WX")
        .arg(&script)
        .current_dir(&directory)
        .output()
        .expect("run makensis");

    let status = output.status.success();
    let log = String::from_utf8_lossy(&output.stdout).into_owned();
    let _ = std::fs::remove_dir_all(&directory);

    assert!(status, "makensis -WX rejected the anchored script:\n{log}");
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
