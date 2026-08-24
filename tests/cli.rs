//! What the commands promise, checked by running them.
//!
//! Everything else in this suite calls the library, which is the right level
//! for a question about the language. These are questions about the *shell*:
//! which pass a command runs and what it exits with are decisions made in
//! `src/main.rs` and visible nowhere else, and `installua check` exiting 0 on a
//! program `installua build` rejects is exactly the kind of thing a library
//! test cannot see.

use std::path::{Path, PathBuf};
use std::process::Command;

/// A program with one error, raised by the **lowering** and by nothing before
/// it: `manifest.gdiScaling` is a real setting written under a name that is not
/// one, so the frontend parses it and the resolver has nothing to say about it.
const LOWERING_ERROR: &str = "\
attributes { name = \"A\", outFile = \"a.exe\", manifestGdiScaling = true }
installer { page.instFiles {}, section(\"Core\", function() detailPrint(\"x\") end) }
";

/// The same program written right.
const CLEAN: &str = "\
attributes { name = \"A\", outFile = \"a.exe\", manifest = { gdiScaling = true } }
installer { page.instFiles {}, section(\"Core\", function() detailPrint(\"x\") end) }
";

/// `installua <args>`, against a source written to its own file.
///
/// The file goes beside the fixtures for the reason the tier-3 scripts do: a
/// path in a program is resolved against the file that names it, so a program
/// under test has to live somewhere a relative path would work from.
fn run(name: &str, source: &str, command: &str) -> (bool, String) {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures");
    let script = fixtures.join(name);
    std::fs::write(&script, source).expect("write the program");

    let output = Command::new(PathBuf::from(env!("CARGO_BIN_EXE_installua")))
        .arg(command)
        .arg(&script)
        .current_dir(&fixtures)
        .output()
        .expect("run installua");

    let _ = std::fs::remove_file(&script);
    // `emit` writes beside its input, so its output is named after the file
    // this just wrote. `check` writes nothing, which is what
    // [`check_writes_no_nsi`] is for.
    let _ = std::fs::remove_file(script.with_extension("nsi"));

    // Both streams, because which one a diagnostic goes to is not what this
    // asks about.
    (
        output.status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
    )
}

/// The whole of the bug: `check` ran the frontend and the resolver and stopped,
/// so every unknown field, every retired instruction and every page and control
/// error passed it — while `build` rejected the same file. A check that exits 0
/// on a program that does not compile is worse than no check, because it is the
/// one a CI job is wired to.
#[test]
fn check_fails_on_an_error_only_the_lowering_finds() {
    let (passed, output) = run("check-lowering.lua", LOWERING_ERROR, "check");
    assert!(!passed, "`check` accepted a program that does not compile");
    assert!(
        output.contains("unknown-field"),
        "`check` failed without saying why:\n{output}"
    );
}

/// And it is the same answer `build` gives, which is what "everything `build`
/// would say" claims. Asserted against the two runs rather than against a
/// remembered string: the promise is that they agree, not that either says
/// something in particular.
#[test]
fn check_says_what_emit_says() {
    let (_, checked) = run("check-agrees.lua", LOWERING_ERROR, "check");
    let (_, emitted) = run("emit-agrees.lua", LOWERING_ERROR, "emit");

    let strip = |text: String, name: &str| text.replace(name, "<program>");
    assert_eq!(
        strip(checked, "check-agrees.lua"),
        strip(emitted, "emit-agrees.lua")
    );
}

/// The other half, and it is not a formality: a `check` that always failed
/// would pass the test above.
#[test]
fn check_passes_a_program_that_compiles() {
    let (passed, output) = run("check-clean.lua", CLEAN, "check");
    assert!(passed, "`check` rejected a program that compiles:\n{output}");
    assert!(output.is_empty(), "`check` said something:\n{output}");
}

/// And it writes nothing, which is the difference between it and `emit`.
#[test]
fn check_writes_no_nsi() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures");
    let script = fixtures.join("check-writes.lua");
    std::fs::write(&script, CLEAN).expect("write the program");

    let output = Command::new(PathBuf::from(env!("CARGO_BIN_EXE_installua")))
        .arg("check")
        .arg(&script)
        .current_dir(&fixtures)
        .output()
        .expect("run installua");

    let written = fixtures.join("check-writes.nsi");
    let there = written.exists();
    let _ = std::fs::remove_file(&script);
    let _ = std::fs::remove_file(&written);

    assert!(output.status.success(), "`check` failed on a clean program");
    assert!(!there, "`check` wrote `check-writes.nsi`");
}
