//! The stock headers: the third surface, counted like the other two.
//!
//! ```text
//! tables/headers-3.12.txt  ─split─▶  header paths  ┐
//!                                                  ├─join─▶  (path, Class)
//!                                            ROWS  ┘
//! ```
//!
//! Commands and MUI2 names were counted from the start, and headers were picked
//! by hand on evidence — which is how `MultiUser.nsh` and `Memento.nsh` went
//! unmentioned for as long as they did. This is that pick replaced by a list
//! that fails the build when an NSIS release adds a file nobody has read.
//!
//! The snapshot is paths only. A header's macros would be facts nothing here
//! reads, and the judgement is per header, not per macro: `Library.nsh` is one
//! lowering or none, whatever it defines.

use std::path::Path;

/// The checked-in snapshot, for [`crate::locale`]'s reason.
const SNAPSHOT: &str = include_str!("../tables/headers-3.12.txt");

/// What Installua does with one header.
///
/// Five buckets, one more than MUI2's: `replaced` is a header whose job the
/// language does its own way, which is neither a gap nor a refusal, and
/// counting LogicLib as `rejected` would say something that is not so.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// A script reaches it. The text is the spelling.
    Exposed(&'static str),
    /// The compiler writes its `!include` for a construct. The text is which.
    Lowering(&'static str),
    /// The language has its own spelling for what it does; never included.
    Replaced(&'static str),
    /// Deliberately unavailable, with the reason.
    Rejected(&'static str),
    /// Not yet, with a one-line reason.
    Todo(&'static str),
}

impl Class {
    pub fn bucket(&self) -> &'static str {
        match self {
            Class::Exposed(_) => "exposed",
            Class::Lowering(_) => "lowering",
            Class::Replaced(_) => "replaced",
            Class::Rejected(_) => "rejected",
            Class::Todo(_) => "todo",
        }
    }

    pub const BUCKETS: &'static [&'static str] =
        &["exposed", "lowering", "replaced", "rejected", "todo"];
}

const WIN: Class = Class::Rejected(
    "Windows constants; write the number, as `WinMessages` covers the family `sendMessage` needs",
);

/// One row per snapshot path, in its order.
pub const ROWS: &[(&str, Class)] = &[
    (
        "Colors",
        Class::Rejected("HTML colour names as defines; a colour is its hex string, `\"FFFFFF\"`"),
    ),
    ("FileFunc", Class::Exposed("`import \"FileFunc\"`")),
    (
        "InstallOptions",
        Class::Rejected("superseded by nsDialogs, which `page.custom` is"),
    ),
    ("Integration", Class::Exposed("`import \"Integration\"`")),
    (
        "LangFile",
        Class::Rejected("the macros a language file is written in; MUI2 includes it itself"),
    ),
    (
        "Library",
        Class::Lowering("`installLib` and `uninstallLib`"),
    ),
    (
        "LogicLib",
        Class::Replaced("`if`, `while` and `for`, which lower to jumps without it"),
    ),
    ("MUI", Class::Rejected("Modern UI 1, superseded by MUI2")),
    (
        "MUI2",
        Class::Lowering("the `page.*` entries and `installer {}`'s interface fields"),
    ),
    (
        "Memento",
        Class::Lowering("`memento {}` and a section's `remember`"),
    ),
    (
        "MultiUser",
        Class::Lowering("`multiUser {}` and `page.installMode`"),
    ),
    (
        "Sections",
        Class::Replaced("a section's fields, `selected` and the rest, as the bit arithmetic"),
    ),
    (
        "StrFunc",
        Class::Lowering("`string.lower`, `string.upper` and `string.find`"),
    ),
    ("TextFunc", Class::Exposed("`import \"TextFunc\"`")),
    (
        "UpgradeDLL",
        Class::Rejected("an NSIS 2.0 compatibility shim; `Library.nsh` is its successor"),
    ),
    (
        "Util",
        Class::Rejected("`CallArtificialFunction` plumbing other headers use, nothing to call"),
    ),
    (
        "VB6RunTime",
        Class::Rejected("the VB6 runtime, which every Windows NSIS 3 targets already ships"),
    ),
    (
        "VPatchLib",
        Class::Rejected("the NSIS 2.0.5 wrapper of the `VPatch` plugin, which is declared"),
    ),
    ("Win/COM", WIN),
    ("Win/Propkey", WIN),
    ("Win/RestartManager", WIN),
    ("Win/WinDef", WIN),
    ("Win/WinError", WIN),
    ("Win/WinNT", WIN),
    ("Win/WinUser", WIN),
    ("WinCore", WIN),
    (
        "WinMessages",
        Class::Exposed("its names as numbers, `sendMessage(h, WM_SETTEXT, 0, \"…\")`"),
    ),
    (
        "WinVer",
        Class::Replaced("`getWinVer`, a real instruction since NSIS 3"),
    ),
    ("WordFunc", Class::Exposed("`import \"WordFunc\"`")),
    (
        "nsDialogs",
        Class::Replaced("`page.custom` and its controls, which call the plugin directly"),
    ),
    (
        "x64",
        Class::Lowering("`runningX64()`, `wow64()` and `nativeMachine(\"ARM64\")`"),
    ),
];

/// Every snapshot path: `MUI2`, `Win/COM`.
pub fn snapshot() -> Vec<&'static str> {
    SNAPSHOT
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
}

/// The join, total for [`crate::mui::inventory`]'s reason: a header with no row
/// is `todo`, and `tests/headers.rs` is where that is a failure.
pub fn inventory() -> Vec<(&'static str, Class)> {
    snapshot()
        .into_iter()
        .map(|path| {
            let class = ROWS.iter().find(|(name, _)| *name == path).map_or(
                Class::Todo("no row: added by a newer NSIS"),
                |(_, class)| *class,
            );
            (path, class)
        })
        .collect()
}

/// The headers block of `installua coverage`.
pub fn coverage() -> String {
    let inventory = inventory();
    let mut out = format!(
        "\ninstallua header coverage -- NSISDIR/Include, {} headers\n\n",
        inventory.len()
    );
    for bucket in Class::BUCKETS {
        let count = inventory
            .iter()
            .filter(|(_, class)| class.bucket() == *bucket)
            .count();
        out.push_str(&format!("  {bucket:<16}{count:>4}\n"));
    }
    let todo: Vec<_> = inventory
        .iter()
        .filter_map(|(path, class)| match class {
            Class::Todo(reason) => Some((path, reason)),
            _ => None,
        })
        .collect();
    if !todo.is_empty() {
        out.push_str("\nheader todo:\n");
        for (path, reason) in todo {
            out.push_str(&format!("  {path:<24}{reason}\n"));
        }
    }
    out
}

/// The snapshot, regenerated: `tests/headers.rs` under `UPDATE_SNAPSHOTS`.
/// `Include/` and `Include/Win/`, which is all the tree there is.
pub fn scan(root: &Path) -> Result<String, String> {
    let include = root.join("Include");
    let mut paths = Vec::new();
    for (directory, prefix) in [(include.clone(), ""), (include.join("Win"), "Win/")] {
        let entries = std::fs::read_dir(&directory)
            .map_err(|error| format!("{}: {error}", directory.display()))?;
        for entry in entries {
            let path = entry
                .map_err(|error| format!("{}: {error}", directory.display()))?
                .path();
            if path.extension().is_some_and(|extension| extension == "nsh")
                && let Some(stem) = path.file_stem().and_then(|stem| stem.to_str())
            {
                paths.push(format!("{prefix}{stem}"));
            }
        }
    }
    paths.sort_unstable();

    let mut out = String::from(HEADER);
    for path in paths {
        out.push_str(&path);
        out.push('\n');
    }
    Ok(out)
}

const HEADER: &str = "\
# The headers NSIS ships, as its `Include` directory reads it.
#
# One path per line, without `.nsh`, sorted. Each has a row in
# `src/headers.rs`. Regenerate with:
#
#     NSISDIR=\"...\" UPDATE_SNAPSHOTS=1 cargo test --test headers

";
