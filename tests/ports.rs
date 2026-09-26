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
//! nothing. Control flow is dropped too, and registers are renamed in order
//! of first use, since both are the compiler's. Control flow is the goldens'
//! job; this checks effects.
//!
//! The trace is in source order, not run order, so a port keeps the
//! original's order of effects where a loop would allow either.

use std::path::{Path, PathBuf};
use std::process::Command;

use installua::diag::Diagnostics;

const PORTS: &[&str] = &["example1", "example2", "primes", "silent"];

/// Trace lines that differ by design rather than by mistake.
const DROPPED: &[&str] = &[
    "!",
    "Processing default plugins",
    " + ",
    // The default, which the original says by saying nothing.
    "Unicode: true",
    "NSIS Modern User Interface",
    // Pages.
    "Page: ",
    "UninstPage: ",
    "DirText: ",
    // Control flow.
    "StrCpy ",
    "StrCmp",
    "IntOp: ",
    "IntCmp",
    "IfSilent",
    "Goto: ",
    "Call ",
    "Return",
    "Function",
    // Written around the one `File` it is for, and put back after it.
    "AllowSkipFiles: ",
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
/// `un.` prefix taken off: `Section "Uninstall"` is the same section. Functions
/// are moved after the sections, where Installua writes them. The
/// attributes above the first section are sorted and deduplicated, because
/// their order does nothing and Installua writes `Unicode` first. A message
/// box's `(on IDNO goto label)` is cut, because the label is the compiler's.
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
    let mut in_function = false;
    let (functions, rest): (Vec<&str>, Vec<&str>) = log
        .lines()
        .skip_while(|line| !line.starts_with("Processing script file"))
        .skip(1)
        .take_while(|line| !line.starts_with("Processed "))
        .partition(|line| {
            in_function |= line.starts_with("Function: ");
            let inside = in_function;
            in_function &= !line.starts_with("FunctionEnd");
            inside
        });
    let mut lines: Vec<String> = rest
        .into_iter()
        .chain(functions)
        .filter(|line| !line.trim().is_empty() && !DROPPED.iter().any(|p| line.starts_with(p)))
        .map(|line| {
            let line = line.replacen("Section: \"un.", "Section: \"", 1);
            let line = line.split(" (on ").next().unwrap_or_default();
            line.to_string() + "\n"
        })
        .collect();
    let attributes = lines
        .iter()
        .position(|line| line.starts_with("Section: "))
        .unwrap_or(lines.len());
    lines[..attributes].sort();
    let mut lines = lines
        .concat()
        .lines()
        .map(str::to_string)
        .collect::<Vec<_>>();
    lines.dedup();
    registers(&lines.join("\n"))
}

/// `$0`–`$9` and `$R0`–`$R9` renamed `r0`, `r1`, … in order of first use.
fn registers(trace: &str) -> String {
    let mut seen: Vec<&str> = Vec::new();
    let mut out = String::new();
    let mut rest = trace;
    while let Some(at) = rest.find('$') {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        let len = if tail[1..].starts_with('R') { 3 } else { 2 };
        let name = tail
            .get(..len)
            .filter(|n| n.as_bytes()[len - 1].is_ascii_digit());
        match name {
            Some(name) => {
                let index = seen.iter().position(|s| *s == name).unwrap_or_else(|| {
                    seen.push(name);
                    seen.len() - 1
                });
                out.push_str(&format!("r{index}"));
                rest = &tail[len..];
            }
            None => {
                out.push('$');
                rest = &tail[1..];
            }
        }
    }
    out + rest
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
