//! Ports of the scripts in `$NSISDIR/Examples`, checked against the original.
//!
//! The emitted text cannot equal the original's — registers, labels and
//! `LogicLib` all differ — so what is compared is what `makensis -V4` reports
//! doing: one line per instruction, arguments resolved, macros expanded. The
//! trace keeps the lines that do something and drops the ones where a port is
//! expected to differ, so an equal trace means the same files, keys and
//! shortcuts, in the same sections, in the same order.
//!
//! Pages are dropped: the originals use classic `Page` lines and Installua
//! only writes MUI2 pages, whose macros run with verbosity off and print
//! nothing. Control flow is the goldens' job; this checks effects.

use std::path::{Path, PathBuf};
use std::process::Command;

use installua::diag::Diagnostics;

const PORTS: &[&str] = &["example1", "example2"];

/// Trace lines that differ by design rather than by mistake.
const DROPPED: &[&str] = &[
    "!",
    "Page: ",
    "UninstPage: ",
    "Processing default plugins",
    "NSIS Modern User Interface",
];

fn port(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/ports/{name}.lua"));
    let source = std::fs::read_to_string(&path).expect("the port");
    let mut diags = Diagnostics::new();
    let output = installua::build_with(&source, &installua::Options::for_file(&path), &mut diags);
    assert!(diags.is_empty(), "{name}:\n{}", diags.render("<test>"));
    output.expect("compiles")
}

#[test]
fn ports_compile() {
    for name in PORTS {
        port(name);
    }
}

#[test]
fn ports_do_what_the_originals_do() {
    let Some(examples) = examples() else {
        eprintln!("skipping: `makensis` is not installed");
        return;
    };
    for name in PORTS {
        let directory =
            std::env::temp_dir().join(format!("installua-port-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("temp directory");
        // The original installs itself, so it is also the file the port's `file()` reads.
        std::fs::copy(
            examples.join(format!("{name}.nsi")),
            directory.join(format!("{name}.nsi")),
        )
        .expect("copy the original");
        std::fs::write(directory.join("port.nsi"), port(name)).expect("write the port");

        let original = trace(&directory, &format!("{name}.nsi"), &[]);
        let ported = trace(&directory, "port.nsi", &["-WX"]);
        let _ = std::fs::remove_dir_all(&directory);
        assert_eq!(ported, original, "{name}");
    }
}

/// The effect lines of one `makensis -V4` run, with an uninstaller section's
/// `un.` prefix taken off: `Section "Uninstall"` is the same section. The
/// attributes above the first section are sorted, because their order does
/// nothing and Installua writes `Unicode` first.
fn trace(directory: &Path, script: &str, flags: &[&str]) -> String {
    let output = Command::new("makensis")
        .args(flags)
        .arg("-V4")
        .arg(script)
        .current_dir(directory)
        .output()
        .expect("run makensis");
    let log = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        output.status.success(),
        "makensis rejected {script}:\n{log}"
    );
    let mut lines: Vec<String> = log
        .lines()
        .skip_while(|line| !line.starts_with("Processing script file"))
        .skip(1)
        .take_while(|line| !line.starts_with("Processed "))
        .filter(|line| !line.trim().is_empty() && !DROPPED.iter().any(|p| line.starts_with(p)))
        .map(|line| line.replacen("Section: \"un.", "Section: \"", 1) + "\n")
        .collect();
    let attributes = lines
        .iter()
        .position(|line| line.starts_with("Section: "))
        .unwrap_or(lines.len());
    lines[..attributes].sort();
    lines.concat()
}

/// `$NSISDIR/Examples` of the `makensis` in use.
fn examples() -> Option<PathBuf> {
    let output = Command::new("makensis").arg("-HDRINFO").output().ok()?;
    let info = String::from_utf8_lossy(&output.stdout).into_owned();
    let nsisdir = info
        .split(',')
        .find_map(|part| part.trim().strip_prefix("NSISDIR="))?;
    Some(Path::new(nsisdir.trim_matches('"')).join("Examples"))
}
