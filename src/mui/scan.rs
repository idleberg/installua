//! The snapshot half of the MUI inventory: `Contrib/Modern UI 2`, read.
//!
//! The analogue of `makensis -CMDHELP` for the other surface (§14). MUI2 has no
//! command that prints its settings, so this reads the headers themselves —
//! which is not a weaker source but the *same* one the settings live in, and it
//! is why every tag below is a fact about MUI2's text rather than a judgement
//! about it. The judgement is in [`super::rows`], and the two are joined the way
//! the instruction table's two halves are (§15.23).
//!
//! `Deprecated.nsh` is not read. Every macro in it is a `!error` MUI2 raises on
//! purpose, so a row for one would be an Installua name for something MUI2
//! itself refuses to compile. The file's macro names are printed in the
//! snapshot's header, because a file skipped in silence is indistinguishable
//! from one nobody found.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// What a name is: a `!define` a user writes, or a `!macro` a user inserts.
/// Both are the MUI2 surface, and only one of them is a "setting".
///
/// `both` is not a hedge. `MUI_ABORTWARNING` is a define a user writes *and* a
/// macro MUI2 inserts when the define exists, and the two share one name — so a
/// column that had to pick one would have to be wrong about the other.
pub const KIND_SETTING: &str = "setting";
pub const KIND_MACRO: &str = "macro";
pub const KIND_BOTH: &str = "both";

/// The tags, spelled once. Each is a shape in MUI2's text and nothing more.
const TAG_DEFAULT: &str = "default";
const TAG_PAGE: &str = "page";
const TAG_ONCE: &str = "once";
const TAG_SET: &str = "set";
const TAG_UN: &str = "un";
/// The name appears in MUI2's own `Readme.html`. The single most useful fact
/// about a `MUI_*` name and the one the headers cannot state: MUI2 documents
/// what a user writes and says nothing about what it writes itself.
const TAG_DOC: &str = "doc";

/// The files, in the order MUI2 includes them. Fixed rather than walked so the
/// snapshot's order is the same on every machine, and so a file MUI2 gains is a
/// change to this list rather than a silent diff.
const FILES: &[&str] = &[
    "MUI2.nsh",
    "Interface.nsh",
    "Localization.nsh",
    "Pages.nsh",
    "Pages/Welcome.nsh",
    "Pages/License.nsh",
    "Pages/Components.nsh",
    "Pages/Directory.nsh",
    "Pages/StartMenu.nsh",
    "Pages/InstallFiles.nsh",
    "Pages/Finish.nsh",
    "Pages/UninstallConfirm.nsh",
];

/// The file that is deliberately not read, and what is in it.
const DEPRECATED: &str = "Deprecated.nsh";

/// Where the headers and the documentation live under `$NSISDIR`.
const HEADERS: &str = "Contrib/Modern UI 2";
const README: &str = "Docs/Modern UI 2/Readme.html";

#[derive(Debug, Default)]
struct Entry {
    macro_: bool,
    setting: bool,
    tags: BTreeSet<&'static str>,
    site: String,
    within: String,
}

/// Reads a `Contrib/Modern UI 2` directory and prints the snapshot.
///
/// Errors are returned rather than printed: the caller is the CLI and owns the
/// exit code.
pub fn scan(nsis: &Path) -> Result<String, String> {
    let mut entries: BTreeMap<String, Entry> = BTreeMap::new();
    let mut constructed: BTreeSet<String> = BTreeSet::new();
    let root = nsis.join(HEADERS);

    for file in FILES {
        let path = root.join(file);
        let text = std::fs::read_to_string(&path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        read(file, &text, &mut entries, &mut constructed);
    }

    let deprecated = deprecated_macros(&root)?;

    // The Readme is read for one bit per name and nothing else. It is MUI2's
    // own answer to "is this yours or mine", which no amount of reading the
    // headers can supply: a define MUI2 writes for itself looks exactly like
    // one it expects from you.
    for name in documented(&nsis.join(README))? {
        if let Some(entry) = entries.get_mut(&name) {
            entry.tags.insert(TAG_DOC);
        }
    }

    let mut out = String::new();
    out.push_str(&header(&deprecated, &constructed));
    for (name, entry) in &entries {
        let tags = entry.tags.iter().copied().collect::<Vec<&str>>().join(" ");
        let kind = match (entry.macro_, entry.setting) {
            (true, true) => KIND_BOTH,
            (true, false) => KIND_MACRO,
            _ => KIND_SETTING,
        };
        out.push_str(&format!(
            "{kind:<9}{name:<54}{tags:<30}{:<30}{}\n",
            entry.site, entry.within
        ));
    }
    Ok(out)
}

fn header(deprecated: &[String], constructed: &BTreeSet<String>) -> String {
    let mut out = String::from(
        "\
# Modern UI 2, as its own headers read it.
#
# Columns: what the name is, the name, what MUI2's text does with it, where it
# is first seen, and the `!macro` that line sits in. Regenerate with:
#
#     NSISDIR=\"...\" UPDATE_SNAPSHOTS=1 cargo test --test mui
#
# Tags, each a fact about the text and none of them a judgement:
#
#   default  MUI2 supplies one through `MUI_DEFAULT`, so leaving it out is legal
#   page     MUI2 `!undef`s or `MUI_UNSET`s it, so it is read once per page
#   once     it is read inside an `!ifndef`-guarded `*_INTERFACE` macro, so the
#            first page of its type wins and every later one is ignored
#   set      MUI2 writes it itself somewhere, so a row may be its state and not
#            a setting at all
#   un       MUI2 builds the name with its uninstaller prefix, so `MUI_UN…`
#            exists too and is the same setting for the other half
#   doc      MUI2's own Readme.html names it, which is the only place that says
#            whether a define is yours to write or MUI2's to keep\n#\n",
    );

    out.push_str("# Not read: Deprecated.nsh, whose every macro is a `!error` MUI2 raises on\n");
    out.push_str("# purpose. Naming one in Installua would be an Installua spelling for a\n");
    out.push_str("# thing MUI2 refuses to compile:\n#\n");
    for name in deprecated {
        out.push_str(&format!("#   {name}\n"));
    }

    if !constructed.is_empty() {
        out.push_str(
            "#\n# Names MUI2 builds out of a macro argument. Each is derived from a name\n\
             # already listed, so there is nothing here for a user to write:\n#\n",
        );
        for name in constructed {
            out.push_str(&format!("#   {name}\n"));
        }
    }

    out.push('\n');
    out
}

/// One file's worth. `within` is the enclosing `!macro`, which is what makes
/// `once` mechanical: MUI2 puts every block-scoped setting inside a macro whose
/// name ends in `_INTERFACE` (batch 20).
fn read(
    file: &str,
    text: &str,
    entries: &mut BTreeMap<String, Entry>,
    constructed: &mut BTreeSet<String>,
) {
    let mut within = String::new();

    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.starts_with(';') {
            continue;
        }
        let site = format!("{file}:{}", index + 1);

        if let Some(rest) = after(line, "!macro ") {
            let name = rest.split_whitespace().next().unwrap_or("").to_string();
            within = name.clone();
            if name.starts_with("MUI_") {
                entry(entries, &name, &site, &within).macro_ = true;
            }
            continue;
        }
        if line.starts_with("!macroend") {
            within.clear();
            continue;
        }

        let once = within.ends_with("_INTERFACE");
        let mut note = |name: &str, tag: Option<&'static str>| {
            let (name, un) = normalise(name);
            if !name.starts_with("MUI_") || name.len() <= 4 {
                return;
            }
            if name.contains("${") {
                constructed.insert(name);
                return;
            }
            let entry = entry(entries, &name, &site, &within);
            entry.setting = true;
            if let Some(tag) = tag {
                entry.tags.insert(tag);
            }
            if un {
                entry.tags.insert(TAG_UN);
            }
            if once {
                entry.tags.insert(TAG_ONCE);
            }
        };

        // `!insertmacro MUI_DEFAULT X …` and `MUI_SET`/`MUI_UNSET` are MUI2's
        // three verbs for a setting, and each one says something different
        // about the name that follows it.
        if let Some(rest) = after(line, "!insertmacro MUI_DEFAULT ") {
            note(word(rest), Some(TAG_DEFAULT));
            continue;
        }
        if let Some(rest) = after(line, "!insertmacro MUI_SET ") {
            note(word(rest), Some(TAG_SET));
            continue;
        }
        // The one name MUI2 builds out of an argument that a *user* writes:
        // `MUI_PAGE_FUNCTION_CUSTOM PRE` reads `MUI_PAGE_CUSTOMFUNCTION_PRE`.
        // Expanded from the insertion sites rather than from a list here, so
        // the three hooks are MUI2's count and not ours (§15.23).
        if let Some(rest) = after(line, "!insertmacro MUI_PAGE_FUNCTION_CUSTOM ") {
            let hook = format!("MUI_PAGE_CUSTOMFUNCTION_{}", word(rest));
            note(&hook, Some(TAG_PAGE));
            continue;
        }
        if let Some(rest) = after(line, "!insertmacro MUI_UNSET ") {
            note(word(rest), Some(TAG_PAGE));
            continue;
        }
        if let Some(rest) = after(line, "!undef ") {
            note(word(rest).trim_matches('"'), Some(TAG_PAGE));
            continue;
        }
        if let Some(rest) = after(line, "!define ") {
            let name = word(rest).trim_matches('"');
            // `!define /IfNDef` and friends put a switch first; the name is the
            // first token that is not one.
            let name = if name.starts_with('/') {
                word(rest[name.len()..].trim_start())
            } else {
                name
            };
            note(name.trim_matches('"'), Some(TAG_SET));
            continue;
        }

        // Everything else is a *read*: `!ifdef`, `!ifndef`, and any `${MUI_…}`
        // expansion. `!ifdef A & B` is one line asking about two names, which
        // is why the tail is split rather than taken whole.
        for keyword in ["!ifdef ", "!ifndef "] {
            if let Some(rest) = after(line, keyword) {
                for token in rest.split(|c: char| c.is_whitespace() || c == '&' || c == '|') {
                    if token.starts_with("MUI_") {
                        note(token, None);
                    }
                }
            }
        }
        for name in expansions(line) {
            note(&name, None);
        }
    }
}

fn entry<'e>(
    entries: &'e mut BTreeMap<String, Entry>,
    name: &str,
    site: &str,
    within: &str,
) -> &'e mut Entry {
    entries.entry(name.to_string()).or_insert_with(|| Entry {
        macro_: false,
        setting: false,
        tags: BTreeSet::new(),
        site: site.to_string(),
        within: within.to_string(),
    })
}

/// `${MUI_X}` occurrences, innermost first — `${MUI_A_${MUI_B}}` is two names
/// and the outer one is constructed.
fn expansions(line: &str) -> Vec<String> {
    let bytes = line.as_bytes();
    let mut out = Vec::new();
    let mut index = 0;
    while index + 1 < bytes.len() {
        if bytes[index] == b'$' && bytes[index + 1] == b'{' {
            let start = index + 2;
            let mut depth = 1;
            let mut end = start;
            while end < bytes.len() && depth > 0 {
                match bytes[end] {
                    b'{' => depth += 1,
                    b'}' => depth -= 1,
                    _ => {}
                }
                if depth > 0 {
                    end += 1;
                }
            }
            if depth == 0 {
                out.push(line[start..end].to_string());
            }
        }
        index += 1;
    }
    out
}

/// The uninstaller prefix, removed. MUI2 spells it three ways — the define, the
/// macro argument `_un`, and the argument `UN` — and all three build the same
/// pair of names out of one setting (§15.3).
fn normalise(name: &str) -> (String, bool) {
    let mut out = name.trim().to_string();
    let mut un = false;
    for prefix in ["${MUI_PAGE_UNINSTALLER_PREFIX}", "${_un}", "${UN}", "${un}"] {
        if out.contains(prefix) {
            out = out.replace(prefix, "");
            un = true;
        }
    }
    (out, un)
}

fn after<'l>(line: &'l str, prefix: &str) -> Option<&'l str> {
    let mut squeezed = line.split_whitespace();
    let head: Vec<&str> = prefix.split_whitespace().collect();
    for word in &head {
        if squeezed.next() != Some(*word) {
            return None;
        }
    }
    let offset = line.find(head.last()?)? + head.last()?.len();
    Some(line[offset..].trim_start())
}

fn word(rest: &str) -> &str {
    rest.split_whitespace().next().unwrap_or("")
}

/// The names in `Deprecated.nsh`, for the header. Read for its macro list and
/// nothing else — a missing file is not an error, because an NSIS that dropped
/// it has nothing to warn about.
/// Every `MUI_*` name MUI2's `Readme.html` mentions.
fn documented(path: &Path) -> Result<Vec<String>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let mut names = Vec::new();
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if text[index..].starts_with("MUI_") {
            let end = text[index..]
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .map_or(text.len(), |offset| index + offset);
            names.push(text[index..end].to_string());
            index = end;
            continue;
        }
        index += 1;
    }
    names.sort();
    names.dedup();
    Ok(names)
}

fn deprecated_macros(root: &Path) -> Result<Vec<String>, String> {
    let path = root.join(DEPRECATED);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(Vec::new());
    };
    let mut names: Vec<String> = text
        .lines()
        .filter_map(|line| after(line.trim(), "!macro "))
        .map(|rest| word(rest).to_string())
        .filter(|name| name.starts_with("MUI_"))
        .collect();
    names.sort();
    names.dedup();
    Ok(names)
}
