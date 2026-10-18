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

/// Paths under `$NSISDIR/Examples`; the port is the same path in `tests/ports/`,
/// lower-cased and with `-` for a space.
const PORTS: &[&str] = &[
    "bigtest",
    "example1",
    "example2",
    "languages",
    "Library",
    "LogicLib",
    "Modern UI/Basic",
    "Modern UI/HeaderBitmap",
    "Modern UI/MultiLanguage",
    "Modern UI/StartMenu",
    "Modern UI/WelcomeFinish",
    "Memento",
    "MultiUser",
    "nsDialogs/InstallOptions",
    "one-section",
    "primes",
    "silent",
    "StartMenu/Example",
    "unicode",
    "VersionInfo",
];

/// Lines one side writes inside a macro, where `-V4` prints nothing, and the
/// other side writes in the open, or that cannot match: taken out of both
/// sides, by prefix.
const HIDDEN: &[(&str, &str)] = &[
    // `${If} ${Cmd} `MessageBox …``.
    (
        "Memento",
        "MessageBox: 4: \"Would you like to load an example state?\"",
    ),
    // MUI2 writes it in `MUI_INTERFACE`, and refuses a second one.
    ("Memento", "XPStyle: on"),
    // MUI2 writes these from its own defines, inside its macros.
    ("bigtest", "CheckBitmap: "),
    ("bigtest", "InstallColors: "),
    ("bigtest", "LicenseData: "),
    ("bigtest", "LicenseText: "),
    ("bigtest", "UninstallIcon: "),
    ("bigtest", "UninstallText: "),
    ("bigtest", "XPStyle: on"),
    // `File /a`, which the port leaves out, warns off Windows.
    ("bigtest", "warning 5050: "),
    // Has no spelling.
    ("bigtest", "Nop"),
    // The original leaves `try` in force after its last `File`, where it does
    // nothing; the port puts back the default.
    ("bigtest", "SetOverwrite: try"),
    ("bigtest", "SetOverwrite: on"),
    // Inside LogicLib's macros: `${IfCmd}`, `${Cmd}`, `${SectionIsSelected}`.
    ("LogicLib", "MessageBox: 4: \"Please click Yes\""),
    ("LogicLib", "MessageBox: 260: \"Please click No\""),
    ("LogicLib", "SectionGetFlags: "),
    // `${__FILE__}`, which names the original.
    ("LogicLib", "DetailPrint: \"Source file "),
    // The `StrFunc` functions `string.lower` and `string.find` are made of.
    ("LogicLib", "Exch("),
    ("LogicLib", "File: \"System.dll\""),
    ("LogicLib", "Plugin command: "),
    ("LogicLib", "StrLen "),
    // MUI2 writes it in `MUI_INTERFACE`.
    ("StartMenu/Example", "XPStyle: on"),
    ("unicode", "XPStyle: on"),
    // The second line of `DirText`'s first argument, which MUI2 writes inside
    // its macro and the original in the open.
    ("StartMenu/Example", "For this it needs NSIS's path."),
    // English, loaded only for `${LANG_ENGLISH}`; with no language file at
    // all it is the one NSIS builds in.
    ("VersionInfo", "LoadLanguageFile: "),
    // MUI2 writes these from its own defines, inside its macros: the license
    // page, `MUI_LANGUAGE` and `MUI_LANGDLL_DISPLAY`.
    ("languages", "LicenseData: "),
    ("languages", "LoadLanguageFile: "),
    ("languages", "XPStyle: on"),
    ("languages", "File: \"LangDLL.dll\""),
    ("languages", "Plugin command: LangDialog "),
    ("languages", "Abort: "),
    // MUI2 writes it in `MUI_INTERFACE`.
    ("Library", "XPStyle: on"),
    ("nsDialogs/InstallOptions", "XPStyle: on"),
    // `${If} ${Cmd} `MessageBox …``.
    (
        "nsDialogs/InstallOptions",
        "MessageBox: 292: \"Test the right-to-left version?\"",
    ),
    // The compiler writes it in every body that names `PLUGINSDIR`, and the
    // original only where it unpacks `io.ini`.
    ("nsDialogs/InstallOptions", "InitPluginsDir"),
    // The original builds its application into a `!tempfile` with `!makensis`,
    // and the port installs a stand-in.
    ("MultiUser", "File: "),
];

/// Names NSIS reads in any case, which an original does not always write the
/// way the compiler does, and numbers the compiler writes in decimal.
const SPELLED: &[(&str, &str)] = &[
    ("ShCtx", "SHCTX"),
    ("=0xdeadbeef", "=3735928559"),
    // Octal, to NSIS.
    ("DwordTest_0123=0123", "DwordTest_0123=83"),
    ("$InstDir", "$INSTDIR"),
    ("$SMPrograms", "$SMPROGRAMS"),
    ("$PluginsDir", "$PLUGINSDIR"),
    ("FileWriteWord: 0xD83C", "FileWriteWord: 55356"),
    ("FileWriteWord: 0xDF0D", "FileWriteWord: 57101"),
    ("SetShellVarContext: All", "SetShellVarContext: all"),
    // The port sets the components page's text through MUI2, as a language
    // string of its own, and the compiler names the license's.
    (
        "LangString: \"^ComponentsText\"",
        "LangString: \"ComponentsText\"",
    ),
    (
        "LicenseLangString: \"myLicenseData\"",
        "LicenseLangString: \"licenseData\"",
    ),
    // The readme: "the order of the switches doesn't matter". The compiler
    // writes them in the declaration's order.
    (
        "Select /checknoshortcuts Don't create a start menu folder /autoadd /lastused $R0 ",
        "Select /autoadd /lastused $R0 /checknoshortcuts Don't create a start menu folder ",
    ),
];

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
    "IfErrors",
    "IfFileExists",
    "Goto: ",
    "Call ",
    "Return",
    "Function: ",
    // How a `func` takes its arguments.
    "Push: ",
    "Pop: ",
    // Written around the one `File` it is for, and put back after it.
    "AllowSkipFiles: ",
    // Globals, renamed below like registers: the compiler names its own.
    "Var: ",
];

fn port(name: &str) -> String {
    let slug = name.to_lowercase().replace(' ', "-");
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/ports/{slug}.lua"));
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
        let file = format!("{}.nsi", name.rsplit('/').next().unwrap_or(name));
        let root = std::env::temp_dir().join(format!(
            "installua-port-{}-{}",
            name.replace(['/', ' '], "-"),
            std::process::id()
        ));
        // One level down, for nsDialogs/InstallOptions.nsi's `..\InstallOptions\test.ini`.
        let directory = root.join("script");
        std::fs::create_dir_all(&directory).expect("temp directory");
        std::fs::create_dir_all(root.join("InstallOptions")).expect("temp directory");
        std::fs::copy(
            examples.join("InstallOptions/test.ini"),
            root.join("InstallOptions/test.ini"),
        )
        .expect("test.ini");
        // The original installs itself, so it is also the file the port's `file()` reads.
        // What MultiUser.nsi builds its pretend application from, what
        // bigtest.nsi installs, and the license per language in languages.nsi.
        for other in [
            "AppGen.nsi",
            "silent.nsi",
            "LogicLib.nsi",
            "bigtest.nsi",
            "WordFuncTest.nsi",
            "waplugin.nsi",
            "example1.nsi",
            "example2.nsi",
            "gfx.nsi",
            "makensis.nsi",
            "one-section.nsi",
            "primes.nsi",
        ] {
            std::fs::copy(examples.join(other), directory.join(other)).expect(other);
        }
        // LogicLib.nsi turns its own trace off with `!verbose 2`.
        let original = std::fs::read_to_string(examples.join(format!("{name}.nsi")))
            .expect("read the original");
        let traced: String = original
            .lines()
            .filter(|line| !line.starts_with("!verbose"))
            .map(|line| format!("{line}\n"))
            .collect();
        std::fs::write(directory.join(&file), traced).expect("write the original");
        std::fs::write(directory.join("port.nsi"), port(name)).expect("write the port");

        let hidden: Vec<&str> = HIDDEN
            .iter()
            .filter(|(port, _)| port == name)
            .map(|(_, line)| *line)
            .collect();
        let shown = |trace: String| {
            let lines = trace
                .lines()
                .filter(|line| !hidden.iter().any(|h| line.starts_with(h)));
            lines.collect::<Vec<_>>().join("\n")
        };
        let original = shown(trace(&directory, &file, &[]));
        let ported = shown(trace(&directory, "port.nsi", &["-WX"]));
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(ported, original, "{name}");
    }
}

/// The effect lines of one `makensis -V4` run, with an uninstaller section's
/// `un.` or `-` prefix taken off: `Section "Uninstall"` is the same section. Functions
/// are moved after the sections, where Installua writes them, and sorted,
/// because their order does nothing and a `raw.tail` puts a macro's last. The
/// attributes above the first section are sorted and deduplicated, because
/// their order does nothing and Installua writes `Unicode` first. A
/// `LangString` is a declaration wherever it is written, so it joins them. A message
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
    let vars: Vec<&str> = log
        .lines()
        .filter_map(|line| line.strip_prefix("Var: \""))
        .filter_map(|rest| rest.strip_suffix('"'))
        .collect();
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
    // By what they do, not their name: the compiler names its own.
    let mut functions: Vec<&[&str]> = functions
        .split_inclusive(|line| line.starts_with("FunctionEnd"))
        .collect();
    functions.sort_by_key(|lines| {
        let effects = lines
            .iter()
            .filter(|line| !DROPPED.iter().any(|p| line.starts_with(p)));
        effects.copied().collect::<Vec<_>>()
    });
    let mut lines: Vec<String> = rest
        .into_iter()
        .chain(functions.concat())
        .filter(|line| !line.trim().is_empty() && !DROPPED.iter().any(|p| line.starts_with(p)))
        .map(|line| {
            let line = line.replacen("Section: \"un.", "Section: \"", 1).replacen(
                "Section: \"-Uninstall\"",
                "Section: \"Uninstall\"",
                1,
            );
            let line = SPELLED
                .iter()
                .fold(line, |line, (from, to)| line.replace(from, to));
            // A section's define is named from its `local`, with the half.
            let line = line
                .replacen("->(SEC_", "->(", 1)
                .replacen("->(UNSEC_", "->(", 1);
            let line = line.split(" (on ").next().unwrap_or_default();
            line.to_string() + "\n"
        })
        .collect();
    lines.sort_by_key(|line| !line.starts_with("LangString: "));
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
    registers(&lines.join("\n"), &vars)
}

/// `$0`–`$9`, `$R0`–`$R9` and the globals `vars` declares renamed `r0`, `r1`,
/// … in order of first use, counted afresh in each section and function: which
/// register a body reuses for an unrelated value is the compiler's choice.
/// `FunctionEnd` is kept this far only to mark where a function stops.
fn registers(trace: &str, vars: &[&str]) -> String {
    let mut bodies = vec![String::new()];
    for line in trace.lines() {
        if line.starts_with("Section: ") {
            bodies.push(String::new());
        }
        if line != "FunctionEnd" {
            let body = bodies.last_mut().expect("one body");
            body.push_str(line);
            body.push('\n');
        }
        if line == "FunctionEnd" || line == "SectionEnd" {
            bodies.push(String::new());
        }
    }
    let renamed: String = bodies.iter().map(|body| first_use(body, vars)).collect();
    renamed.trim_end().to_string()
}

fn first_use(trace: &str, vars: &[&str]) -> String {
    let mut seen: Vec<&str> = Vec::new();
    let mut out = String::new();
    let mut rest = trace;
    while let Some(at) = rest.find('$') {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        let end = tail[1..]
            .find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '.')
            .map_or(tail.len(), |end| end + 1);
        let name = if vars.contains(&&tail[1..end]) {
            Some(&tail[..end])
        } else {
            let len = if tail[1..].starts_with('R') { 3 } else { 2 };
            tail.get(..len)
                .filter(|n| n.as_bytes()[len - 1].is_ascii_digit())
        };
        match name {
            Some(name) => {
                let index = seen.iter().position(|s| *s == name).unwrap_or_else(|| {
                    seen.push(name);
                    seen.len() - 1
                });
                out.push_str(&format!("r{index}"));
                rest = &tail[name.len()..];
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
