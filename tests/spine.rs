//! The thin spine, end to end (PLAN Phase 1's second exit criterion).
//!
//! Two tiers, both live from this phase on:
//!
//!   * tier 2 — the golden `.nsi` is compared by **exact equality**.
//!     `assert!(out.contains(…))` is banned: it passes on output carrying one
//!     spurious `StrCpy` too many, which is the failure mode of every compiler
//!     this one is trying not to be.
//!   * tier 3 — `makensis -WX` with an empty warning allowlist, skipped cleanly
//!     when `makensis` is not installed.

use std::path::{Path, PathBuf};
use std::process::Command;

use installua::diag::Diagnostics;

fn golden() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

fn build(source: &str) -> String {
    let mut diags = Diagnostics::new();
    let output = installua::build(source, &mut diags);
    assert!(diags.is_empty(), "{}", diags.render("<test>"));
    output.expect("the spine compiles")
}

#[test]
fn the_spine_matches_its_golden() {
    let source = std::fs::read_to_string(golden().join("spine.lua")).expect("spine.lua");
    let expected = std::fs::read_to_string(golden().join("spine.nsi")).expect("spine.nsi");
    assert_eq!(build(&source), expected);
}

/// `Unicode` leads, and it leads even when nothing asked for it — a later `raw`
/// then overrides it rather than being silently overridden, since NSIS takes
/// the last one with no diagnostic either way.
#[test]
fn unicode_is_always_first() {
    let output = build(r#"attributes { outFile = "a.exe" }"#);
    assert_eq!(output.lines().next(), Some("Unicode true"));

    let output = build(r#"attributes { outFile = "a.exe", unicode = false }"#);
    assert_eq!(output.lines().next(), Some("Unicode false"));
}

/// A string literal is data, never a template, so every `$` is doubled. `$5` is
/// the case that needs no warning: in raw NSIS it is register 5, and here it is
/// five dollars, correctly.
#[test]
fn literals_are_data() {
    let output = build(&program(r#"detailPrint("costs $5")"#));
    assert_eq!(detail_print(&output), r#"DetailPrint "costs $$5""#);
}

/// And `${NOPE}` is the case that does. The doubling is what keeps it from
/// shipping as warning 6000 plus a silently wrong installer — but a `$` in
/// front of an identifier is muscle memory rather than intent, so it is also
/// diagnosed.
#[test]
fn a_sigil_in_a_literal_is_diagnosed_and_escaped() {
    let mut diags = Diagnostics::new();
    let output = installua::build(&program(r#"detailPrint("into ${NOPE}")"#), &mut diags)
        .expect("a warning does not block the build");

    assert_eq!(detail_print(&output), r#"DetailPrint "into $${NOPE}""#);
    assert_eq!(diags.len(), 1);
    assert!(diags.contains(installua::diag::Code::DollarInLiteral));
}

/// One section, one statement — the shape every spine test but the golden wants.
fn program(statement: &str) -> String {
    format!(
        "attributes {{ outFile = \"a.exe\" }}\n\
         installer {{ section(\"Core\", function() {statement} end), }}\n"
    )
}

fn detail_print(output: &str) -> String {
    output
        .lines()
        .find(|line| line.trim_start().starts_with("DetailPrint"))
        .expect("a DetailPrint line")
        .trim()
        .to_string()
}

/// A path position takes `/` and emits `\`, because NSIS does not accept a
/// forward slash everywhere and Installua does not leave that to the user. A
/// non-path position is left alone.
#[test]
fn path_positions_are_normalised() {
    let output = build(
        r#"
attributes { outFile = "dist/a.exe" }
installer {
	section("Core", function()
		detailPrint("see docs/readme")
	end),
}
"#,
    );

    assert!(output.lines().any(|line| line == r#"OutFile "dist\a.exe""#));
    assert_eq!(
        detail_print(&output),
        r#"DetailPrint "see docs/readme""#,
        "a message is not a path"
    );
}

/// Tier 3, live from Phase 1. The allowlist is empty: a `$`-sigil mistake, a
/// mis-ordered `!define` and an unknown `${FOO}` are all warning 6000 plus a
/// silently wrong installer, so a test that checks only the exit code passes on
/// precisely the bugs this compiler exists to prevent.
#[test]
fn the_spine_assembles_under_wx() {
    let Some(makensis) = makensis() else {
        eprintln!("skipping: `makensis` is not installed");
        return;
    };

    let source = std::fs::read_to_string(golden().join("spine.lua")).expect("spine.lua");
    let directory = std::env::temp_dir().join(format!("installua-spine-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("temp directory");
    let script = directory.join("spine.nsi");
    std::fs::write(&script, build(&source)).expect("write the script");

    let output = Command::new(makensis)
        .arg("-WX")
        .arg(&script)
        .current_dir(&directory)
        .output()
        .expect("run makensis");

    let status = output.status.success();
    let log = String::from_utf8_lossy(&output.stdout).into_owned();
    let _ = std::fs::remove_dir_all(&directory);

    assert!(status, "makensis -WX rejected the spine:\n{log}");
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
