//! The languages NSIS ships, and the one thing Installua does with a name.
//!
//! ```text
//! tables/locales-3.12.txt  ─split─▶  NLF names  ─uppercase─▶  ${LANG_*}
//! ```
//!
//! There is no overlay here and no census. A MUI2 name has semantics — it is a
//! setting or a macro, defaulted or not, read once per page or once per script —
//! so [`crate::mui`] joins a snapshot against a hand-written row for each one. A
//! locale has none: the name is a file in `Contrib/Language files`, and the
//! define `LoadLanguageFile` writes is that name uppercased. `PortugueseBR` is
//! `${LANG_PORTUGUESEBR}`, verified against 3.12 rather than assumed.
//!
//! The snapshot is checked in for the reason the other two are: the compiler
//! must reject `languages { locales = { Klingon = … } }` on a machine with no
//! NSIS installed, and a list read from disk at compile time would make the
//! same source compile differently on two machines.

use std::path::Path;

/// The checked-in snapshot.
const SNAPSHOT: &str = include_str!("../tables/locales-3.12.txt");

/// Every NLF name, sorted, comments dropped.
pub fn all() -> Vec<&'static str> {
    SNAPSHOT
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
}

/// Whether `name` is one NSIS ships.
pub fn is_locale(name: &str) -> bool {
    all().binary_search(&name).is_ok()
}

/// The `LANG_*` define for a name: `English` → `LANG_ENGLISH`.
///
/// Uppercasing is all of it, which is why this is a function and not a column.
pub fn define(name: &str) -> String {
    format!("LANG_{}", name.to_ascii_uppercase())
}

/// The nearest name to `typo`, when one is near enough to be worth naming.
///
/// A locale is misspelled by case (`german`), by endonym (`Deutsch`) or by
/// keystroke (`Enlgish`). The first and third are edit-distance work; the
/// second is not, and gets nothing rather than a wrong guess — an unknown
/// locale's diagnostic lists the whole set anyway, so a missing suggestion
/// costs a reader one glance rather than the answer.
pub fn nearest(typo: &str) -> Option<&'static str> {
    let lowered = typo.to_ascii_lowercase();
    let mut best: Option<(usize, &'static str)> = None;
    for name in all() {
        let distance = edit_distance(&lowered, &name.to_ascii_lowercase());
        // A third of the shorter name, so `German`/`Germany` matches and
        // `Danish`/`Spanish` does not.
        if distance > typo.len().min(name.len()) / 3 {
            continue;
        }
        if best.is_none_or(|(previous, _)| distance < previous) {
            best = Some((distance, name));
        }
    }
    best.map(|(_, name)| name)
}

/// Levenshtein, two rows, because the table is 67 names long.
fn edit_distance(left: &str, right: &str) -> usize {
    let right: Vec<char> = right.chars().collect();
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    let mut current = vec![0; right.len() + 1];

    for (row, left) in left.chars().enumerate() {
        current[0] = row + 1;
        for (column, right) in right.iter().enumerate() {
            let substitute = previous[column] + usize::from(left != *right);
            current[column + 1] = substitute
                .min(previous[column + 1] + 1)
                .min(current[column] + 1);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[right.len()]
}

/// The snapshot, regenerated: `tests/locales.rs` under `UPDATE_SNAPSHOTS`.
///
/// The scan is a directory listing, so unlike [`crate::mui::scan`] there is
/// nothing to parse and nothing that can be misread — the only failure is a
/// directory that is not an NSIS installation.
pub fn scan(root: &Path) -> Result<String, String> {
    let directory = root.join("Contrib").join("Language files");
    let entries = std::fs::read_dir(&directory)
        .map_err(|error| format!("{}: {error}", directory.display()))?;

    let mut names: Vec<String> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("{}: {error}", directory.display()))?;
        let path = entry.path();
        if path.extension().is_some_and(|extension| extension == "nlf")
            && let Some(stem) = path.file_stem().and_then(|stem| stem.to_str())
        {
            names.push(stem.to_string());
        }
    }
    if names.is_empty() {
        return Err(format!("{}: no .nlf files", directory.display()));
    }
    names.sort_unstable();

    let mut out = String::from(HEADER);
    for name in names {
        out.push_str(&name);
        out.push('\n');
    }
    Ok(out)
}

const HEADER: &str = "\
# The languages NSIS ships, as its `Contrib/Language files` directory reads it.
#
# One NLF name per line, sorted. The name is what `languages { locales = { … } }`
# is keyed by, and uppercasing it is the `LANG_*` define `LoadLanguageFile`
# writes — `PortugueseBR` is `${LANG_PORTUGUESEBR}`, verified against 3.12.
# Regenerate with:
#
#     NSISDIR=\"...\" UPDATE_SNAPSHOTS=1 cargo test --test locales

";
