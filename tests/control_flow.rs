//! Phase 2 end to end (§14 tiers 2 and 3).
//!
//! [`tests/cfg.rs`](cfg.rs) asserts the properties; this asserts the text, by
//! exact equality against a golden and then by handing that golden to real
//! `makensis`. Both are needed and neither substitutes for the other: a diff
//! against a hand-written expectation only proves the compiler agrees with
//! whoever wrote the expectation, and an assembler that accepts the output says
//! nothing about whether the output is the one that was meant.

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
    output.expect("compiles")
}

#[test]
fn control_flow_matches_its_golden() {
    let source = std::fs::read_to_string(golden().join("control-flow.lua")).expect("the source");
    let expected = std::fs::read_to_string(golden().join("control-flow.nsi")).expect("the golden");
    assert_eq!(build(&source), expected);
}

/// Tier 3, with an empty warning allowlist. A `$`-sigil mistake, a mis-ordered
/// `!define` and an unknown `${FOO}` are all warning 6000 plus a silently wrong
/// installer, so a test that checks only the exit code passes on precisely the
/// bugs this compiler exists to prevent (§14).
#[test]
fn control_flow_assembles_under_wx() {
    let Some(makensis) = makensis() else {
        eprintln!("skipping: `makensis` is not installed");
        return;
    };

    let source = std::fs::read_to_string(golden().join("control-flow.lua")).expect("the source");
    let directory =
        std::env::temp_dir().join(format!("installua-control-flow-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("temp directory");
    let script = directory.join("control-flow.nsi");
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

    assert!(status, "makensis -WX rejected the output:\n{log}");
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
