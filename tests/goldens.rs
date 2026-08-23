//! Every golden program, end to end (§14 tiers 2 and 3).
//!
//! [`tests/cfg.rs`](cfg.rs) and [`tests/registers.rs`](registers.rs) assert the
//! properties; this asserts the text, by exact equality against a golden and
//! then by handing that golden to real `makensis`. Both are needed and neither
//! substitutes for the other: a diff against a hand-written expectation only
//! proves the compiler agrees with whoever wrote the expectation, and an
//! assembler that accepts the output says nothing about whether the output is
//! the one that was meant.

use std::path::{Path, PathBuf};
use std::process::Command;

use installua::diag::{Code, Diagnostics};

/// Each golden, with the diagnostics it is *expected* to raise.
///
/// The list is exact in both directions — an unexpected code fails, and a
/// missing one fails too. `returns` is recursive, and §15.11's depth-cliff lint
/// firing on it is the point rather than a nuisance: it is the only tier-2 test
/// of a warning whose subject compiles perfectly well.
const GOLDENS: &[(&str, &[Code])] = &[
    ("components", &[]),
    ("control-flow", &[]),
    ("dialog", &[]),
    ("languages", &[]),
    ("pages", &[]),
    ("returns", &[Code::DeepRecursion]),
    ("sections", &[]),
];

/// Files tier 3 needs on disk beside the script, because NSIS reads them at
/// *assembly* time rather than at install time: `LicenseData` opens the licence
/// and `CheckBitmap` loads the bitmap while `makensis` is still running. The
/// contents do not matter to any assertion here, only that opening them works.
const ASSETS: &[(&str, &[u8])] = &[
    ("LICENSE.txt", b"Terms.\n"),
    ("check.bmp", include_bytes!("golden/assets/check.bmp")),
];

fn golden() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

fn build(source: &str, expected: &[Code]) -> String {
    let mut diags = Diagnostics::new();
    let output = installua::build(source, &mut diags);

    let raised: Vec<Code> = diags.iter().map(|d| d.code).collect();
    assert_eq!(
        raised,
        expected.to_vec(),
        "unexpected diagnostics:\n{}",
        diags.render("<test>")
    );
    output.expect("compiles")
}

#[test]
fn goldens_match_their_expected_output() {
    for (name, expected) in GOLDENS {
        let source = std::fs::read_to_string(golden().join(format!("{name}.lua")))
            .unwrap_or_else(|_| panic!("the source for {name}"));
        let want = std::fs::read_to_string(golden().join(format!("{name}.nsi")))
            .unwrap_or_else(|_| panic!("the golden for {name}"));
        assert_eq!(build(&source, expected), want, "{name}");
    }
}

/// Tier 3, with an empty warning allowlist. A `$`-sigil mistake, a mis-ordered
/// `!define` and an unknown `${FOO}` are all warning 6000 plus a silently wrong
/// installer, so a test that checks only the exit code passes on precisely the
/// bugs this compiler exists to prevent (§14).
#[test]
fn goldens_assemble_under_wx() {
    let Some(makensis) = makensis() else {
        eprintln!("skipping: `makensis` is not installed");
        return;
    };

    for (name, expected) in GOLDENS {
        let source = std::fs::read_to_string(golden().join(format!("{name}.lua")))
            .unwrap_or_else(|_| panic!("the source for {name}"));
        let directory =
            std::env::temp_dir().join(format!("installua-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("temp directory");
        let script = directory.join(format!("{name}.nsi"));
        std::fs::write(&script, build(&source, expected)).expect("write the script");
        for (asset, bytes) in ASSETS {
            std::fs::write(directory.join(asset), bytes).expect("write the asset");
        }

        let output = Command::new(&makensis)
            .arg("-WX")
            .arg(&script)
            .current_dir(&directory)
            .output()
            .expect("run makensis");

        let status = output.status.success();
        let log = String::from_utf8_lossy(&output.stdout).into_owned();
        let _ = std::fs::remove_dir_all(&directory);

        assert!(status, "makensis -WX rejected {name}:\n{log}");
    }
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
