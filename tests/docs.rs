//! The docs' `import "X"` and `plugin "X"` spellings resolve against what
//! actually ships declared.
//!
//! `docs/reference-map.md` and `docs/mui-reference.md` both claim exhaustive
//! coverage and nothing enforced the claim, so a spelling could rot in place:
//! `docs/nsis-shaped-not-nsis.md` offered `local winver = import "WinVer"` long
//! after `WinVer.nsh` had been superseded by the `getWinVer` instruction, and
//! no `WinVer.toml` ever existed. A reader copying that line got the compiler's
//! "no method declared" diagnostic and no hint that the document was wrong.
//!
//! This is the narrow gate for that class of rot, and only that class: a name
//! shown to a reader as `import "X"` or `plugin "X"` must be one
//! [`Declarations::builtin`] knows, and a method called on the local that
//! binding produces must be one it declares. It deliberately does **not** check
//! the other direction — that every declared method appears in the docs —
//! because that is the larger claim, and it is only worth asserting once there
//! is a body of declarations big enough for the absence to mean something.
//!
//! The rule this leaves for whoever writes the next example: a spelling in the
//! docs is a promise. Show a name that ships, or write the `.toml` in the same
//! change.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use installua::headers::Declarations;

/// Which of the two namespaces a spelling reaches into.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Namespace {
    /// `import "X"` — a header's macros.
    Header,
    /// `plugin "X"` — a DLL's methods.
    Plugin,
}

impl Namespace {
    fn keyword(self) -> &'static str {
        match self {
            Namespace::Header => "import",
            Namespace::Plugin => "plugin",
        }
    }
}

/// One `import "X"` / `plugin "X"` in a document, with the local it binds when
/// it is written as `local name = …`.
#[derive(Debug)]
struct Spelling {
    namespace: Namespace,
    /// The quoted name: what `Declarations` is asked about.
    target: String,
    /// The `local` on the left, when there is one. This is what turns a name
    /// check into a method check.
    binding: Option<String>,
    line: usize,
}

fn docs() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("docs")
}

/// Every `docs/*.md`, read at test time rather than `include_str!`-ed, so a new
/// document is covered by existing here rather than by somebody remembering to
/// add a line.
fn documents() -> Vec<(String, String)> {
    let mut found: Vec<(String, String)> = fs::read_dir(docs())
        .expect("docs/ exists")
        .map(|entry| entry.expect("readable").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
        .map(|path| {
            let name = path
                .file_name()
                .expect("a file")
                .to_string_lossy()
                .into_owned();
            (name, fs::read_to_string(&path).expect("readable"))
        })
        .collect();
    // `read_dir` order is the filesystem's; a failing assertion should name the
    // same document every run.
    found.sort_by(|a, b| a.0.cmp(&b.0));
    assert!(!found.is_empty(), "docs/ has no markdown in it");
    found
}

fn is_ident(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// The identifier ending at `end`, if the text there is one.
fn ident_before(text: &str, end: usize) -> Option<&str> {
    let bytes = text.as_bytes();
    let start = (0..end)
        .rev()
        .find(|&i| !is_ident(bytes[i]))
        .map_or(0, |i| i + 1);
    (start < end).then(|| &text[start..end])
}

/// The `local name =` immediately left of a keyword, if that is what is there.
///
/// Prose and table cells are scanned the same way as fenced code, because the
/// row that named `WinVer` was a table cell: a spelling misleads a reader
/// wherever it is printed, and a scanner that only read fences would have
/// missed the one defect this test exists for.
fn binding_before(line: &str, keyword_at: usize) -> Option<String> {
    let head = line[..keyword_at].trim_end();
    let head = head.strip_suffix('=')?.trim_end();
    let name = ident_before(head, head.len())?;
    let rest = head[..head.len() - name.len()].trim_end();
    rest.ends_with("local").then(|| name.to_owned())
}

/// Every `import "X"` and `plugin "X"` in one document.
///
/// `import(header)` in a **Usage** line takes a variable rather than a literal
/// and matches nothing here, which is right: it names no header, so it promises
/// nothing about one.
fn spellings(text: &str) -> Vec<Spelling> {
    let mut found = Vec::new();
    for (index, line) in text.lines().enumerate() {
        for namespace in [Namespace::Header, Namespace::Plugin] {
            let keyword = namespace.keyword();
            let mut from = 0;
            while let Some(offset) = line[from..].find(keyword) {
                let at = from + offset;
                from = at + keyword.len();
                // `!include` and `reimport` are not this keyword, and neither
                // is the `name = "…"` field of a declaration file.
                if at > 0 && is_ident(line.as_bytes()[at - 1]) {
                    continue;
                }
                let after = line[from..]
                    .trim_start()
                    .trim_start_matches('(')
                    .trim_start();
                let Some(quoted) = after.strip_prefix('"') else {
                    continue;
                };
                let Some(end) = quoted.find('"') else {
                    continue;
                };
                let target = &quoted[..end];
                // `import "…"` in a comment about the format is a placeholder,
                // not a name: it promises a reader nothing to copy.
                if target.is_empty() || !target.bytes().all(is_ident) {
                    continue;
                }
                found.push(Spelling {
                    namespace,
                    target: target.to_owned(),
                    binding: binding_before(line, at),
                    line: index + 1,
                });
            }
        }
    }
    found
}

/// Every `binding.method(` in one document, as `(method, line)` per binding.
fn calls<'a>(text: &'a str, binding: &str) -> Vec<(&'a str, usize)> {
    let needle = format!("{binding}.");
    let mut found = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let mut from = 0;
        while let Some(offset) = line[from..].find(&needle) {
            let at = from + offset;
            from = at + needle.len();
            if at > 0 && is_ident(line.as_bytes()[at - 1]) {
                continue;
            }
            let tail = &line[from..];
            let end = tail
                .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                .unwrap_or(tail.len());
            if end > 0 {
                found.push((&tail[..end], index + 1));
            }
        }
    }
    found
}

#[test]
fn every_documented_header_and_plugin_ships_declared() {
    let declarations = Declarations::builtin();
    let mut missing: Vec<String> = Vec::new();

    for (name, text) in documents() {
        for spelling in spellings(&text) {
            let known = match spelling.namespace {
                Namespace::Header => declarations.known(&spelling.target),
                Namespace::Plugin => !declarations.plugin_methods(&spelling.target).is_empty(),
            };
            if !known {
                missing.push(format!(
                    "docs/{name}:{}: {} \"{}\" is not declared",
                    spelling.line,
                    spelling.namespace.keyword(),
                    spelling.target
                ));
            }
        }
    }

    assert_eq!(
        missing,
        Vec::<String>::new(),
        "these spellings would hit the \"no method declared\" diagnostic if a \
         reader copied them: ship the declaration, or stop showing the name"
    );
}

#[test]
fn every_method_called_on_a_documented_binding_is_declared() {
    let declarations = Declarations::builtin();
    let mut undeclared: Vec<String> = Vec::new();

    for (name, text) in documents() {
        // A document may bind the same local twice; the last one wins for a
        // reader reading downwards, and in practice they agree.
        let bindings: BTreeMap<String, (Namespace, String)> = spellings(&text)
            .into_iter()
            .filter_map(|spelling| {
                let binding = spelling.binding?;
                Some((binding, (spelling.namespace, spelling.target)))
            })
            .collect();

        for (binding, (namespace, target)) in bindings {
            for (method, line) in calls(&text, &binding) {
                let declared = match namespace {
                    Namespace::Header => declarations.lookup(&target, method).is_some(),
                    Namespace::Plugin => declarations.plugin(&target, method).is_some(),
                };
                if !declared {
                    undeclared.push(format!(
                        "docs/{name}:{line}: {target}.{method} is not declared"
                    ));
                }
            }
        }
    }

    assert_eq!(
        undeclared,
        Vec::<String>::new(),
        "these calls name a declared header or plugin but a method it does not \
         have — the shape `winver.getMajor()` had"
    );
}

#[test]
fn the_scanner_sees_the_defect_it_was_written_for() {
    // Without this, the two tests above pass just as well when `spellings`
    // silently finds nothing — which is the failure mode a scanner has.
    let rotted = "| `!include \"WinVer.nsh\"` | `local winver = import \"WinVer\"`, \
                  then `winver.getMajor()` |";
    let found = spellings(rotted);

    assert_eq!(found.len(), 1);
    assert_eq!(found[0].namespace, Namespace::Header);
    assert_eq!(found[0].target, "WinVer");
    assert_eq!(found[0].binding.as_deref(), Some("winver"));
    assert!(!Declarations::builtin().known("WinVer"));
    assert_eq!(calls(rotted, "winver"), vec![("getMajor", 1)]);

    // The plugin half, and the parenthesised spelling.
    let plugin = "local nsExec = plugin(\"nsExec\")";
    let found = spellings(plugin);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].namespace, Namespace::Plugin);
    assert_eq!(found[0].target, "nsExec");
    assert_eq!(found[0].binding.as_deref(), Some("nsExec"));

    // A declaration file's own `name = "…"` field names no spelling.
    assert!(spellings("name = \"TextFunc\"   # what `import \"…\"` is given").is_empty());
}
