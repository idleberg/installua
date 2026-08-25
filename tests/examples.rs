//! The five programs, end to end — Phase 4's exit criterion, as a test.
//!
//! *"All five programs assemble under `-WX`, empty warning allowlist."* A phase
//! is not done until its criterion is mechanically checkable and checked, so it
//! is checked here rather than asserted in a document.
//!
//! Two tiers, and neither substitutes for the other. The generated `.nsi`
//! beside each program is diffed by **exact equality**, so a change in what the
//! compiler emits shows up as a diff a human reads; then the same text is
//! handed to real `makensis` with warnings promoted to errors, because a golden
//! only proves the compiler agrees with whoever last regenerated it.
//!
//! The `expected.nsi` in each directory is the **hand-written oracle** and is
//! deliberately not what this diffs against: the two differ where the allocator
//! made a choice by hand. Diffing against the oracle would either freeze a
//! hand-written register numbering the allocator has no reason to reproduce, or
//! quietly rewrite the oracle every time the compiler changed its mind — and
//! the oracle is worth more as a fixed point to argue with.

use std::path::{Path, PathBuf};
use std::process::Command;

use installua::diag::{Code, Diagnostics};

/// Each program, with the diagnostics it is *expected* to raise — exactly, in
/// both directions. Program 4 recurses, and the depth-cliff warning firing on
/// it is the point rather than a nuisance.
const PROGRAMS: &[(&str, &[Code])] = &[
    ("01-mui-uninstaller", &[]),
    ("02-plugins", &[]),
    ("03-file-iteration", &[]),
    ("04-multiple-returns", &[Code::DeepRecursion]),
    ("05-strings-and-ints", &[]),
];

fn examples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples")
}

/// One program's `.nsi`, compiled against its own directory — `glob` resolves
/// relative to the source rather than to wherever the test runs.
fn build(name: &str, expected: &[Code]) -> String {
    let directory = examples().join(name);
    let source = std::fs::read_to_string(directory.join("install.lua"))
        .unwrap_or_else(|error| panic!("{name}/install.lua: {error}"));

    let options = installua::Options::for_file(&directory.join("install.lua"));
    let mut diags = Diagnostics::new();
    let output = installua::build_with(&source, &options, &mut diags);

    let raised: Vec<Code> = diags.iter().map(|d| d.code).collect();
    assert_eq!(
        raised,
        expected.to_vec(),
        "unexpected diagnostics for {name}:\n{}",
        diags.render(&format!("{name}/install.lua"))
    );
    output.unwrap_or_else(|| panic!("{name} should compile"))
}

#[test]
fn the_five_programs_match_their_generated_output() {
    for (name, expected) in PROGRAMS {
        let golden = examples().join(name).join("generated.nsi");
        let want = std::fs::read_to_string(&golden)
            .unwrap_or_else(|error| panic!("{}: {error}", golden.display()));
        assert_eq!(build(name, expected), want, "{name}");
    }
}

/// Tier 3, with an empty warning allowlist: a `$`-sigil mistake, a mis-ordered
/// `!define` and an unknown `${FOO}` are all warning 6000 plus a silently wrong
/// installer, so a test that checked only the exit code would pass on precisely
/// the bugs this compiler exists to prevent.
#[test]
fn the_five_programs_assemble_under_wx() {
    let Some(makensis) = makensis() else {
        eprintln!("skipping: `makensis` is not installed");
        return;
    };

    for (name, expected) in PROGRAMS {
        let directory = examples().join(name);
        let text = build(name, expected);
        // `File` and `Icon` paths are relative to the script, so the script
        // goes where the assets are and `makensis` runs there.
        let script = directory.join("assembling.nsi");
        std::fs::write(&script, &text).expect("write the script");

        let output = Command::new(&makensis)
            .arg("-WX")
            .arg(&script)
            .current_dir(&directory)
            .output()
            .expect("run makensis");

        let _ = std::fs::remove_file(&script);
        for installer in installers(&directory) {
            let _ = std::fs::remove_file(installer);
        }

        assert!(
            output.status.success(),
            "makensis -WX rejected {name}:\n{}",
            String::from_utf8_lossy(&output.stdout)
        );
    }
}

/// The installers a run left behind, so the test cleans up after itself. Found
/// by extension rather than by name: the `OutFile` in the script is a `${APP}`
/// the preprocessor expanded, and re-expanding it here would be a second
/// implementation of something that already ran.
fn installers(directory: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "exe"))
        .collect()
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
