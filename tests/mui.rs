//! The MUI inventory's join, asserted in both directions (§14).
//!
//! The instruction table's census test is the model: a skeleton with no overlay
//! row and an overlay row with no skeleton are both failures, because either one
//! means the two halves have drifted and the coverage number is measuring
//! something that no longer exists.

use installua::mui::{self, Class};

#[test]
fn every_snapshot_name_has_an_inventory_row() {
    let missing: Vec<&str> = mui::inventory()
        .iter()
        .filter(|entry| matches!(entry.class, Class::Todo(reason) if reason.starts_with("no inventory row")))
        .map(|entry| entry.name())
        .collect();
    assert!(
        missing.is_empty(),
        "snapshot names with no row in `mui::rows`: {missing:#?}"
    );
}

#[test]
fn every_inventory_row_names_a_snapshot_name() {
    let stale: Vec<&str> = mui::rows::ROWS
        .iter()
        .filter(|row| mui::by_name(row.name).is_none())
        .map(|row| row.name)
        .collect();
    assert!(
        stale.is_empty(),
        "rows naming something the snapshot does not: {stale:#?}"
    );
}

#[test]
fn no_name_is_classified_twice() {
    let mut seen: Vec<&str> = mui::rows::ROWS.iter().map(|row| row.name).collect();
    let before = seen.len();
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(before, seen.len(), "a name is in `mui::rows` twice");
}

/// Every name Installua offers is one MUI2 documents. The `doc` tag comes from
/// MUI2's own `Readme.html`, so an `Exposed` row without it would be Installua
/// publishing a define MUI2 keeps to itself — which is exactly how a script
/// breaks on the next MUI2 release.
#[test]
fn nothing_undocumented_is_exposed() {
    // One exception, and it is MUI2's own asymmetry: `Directory.nsh` reads
    // `MUI_DIRECTORYPAGE_TEXTCOLOR` on the line after `…_BGCOLOR` and spends
    // both on one `SetCtlColors`, but the Readme documents only the background.
    // Offering the background alone would let a script recolour the box and
    // never reach the text in it — which is black on black, and the whole
    // reason `colors` is one field holding two.
    const UNDOCUMENTED_BUT_REAL: &[&str] = &["MUI_DIRECTORYPAGE_TEXTCOLOR"];

    let undocumented: Vec<&str> = mui::inventory()
        .iter()
        .filter(|entry| matches!(entry.class, Class::Exposed(_)))
        .filter(|entry| !entry.snapshot.has("doc"))
        .map(|entry| entry.name())
        .filter(|name| !UNDOCUMENTED_BUT_REAL.contains(name))
        .collect();
    assert!(
        undocumented.is_empty(),
        "exposed, but MUI2's Readme does not mention it: {undocumented:#?}"
    );
}

/// Nothing from `Deprecated.nsh` is in the inventory at all — not as `todo`, not
/// as `rejected`. Every macro in that file is a `!error`, so a row for one would
/// be a backlog entry for something that cannot be built.
#[test]
fn no_deprecated_macro_is_in_the_inventory() {
    let deprecated: Vec<&str> = mui::inventory()
        .iter()
        .map(|entry| entry.name())
        .filter(|name| {
            name.starts_with("MUI_INSTALLOPTIONS_") || *name == "MUI_LEGACY_MAP_NOSTRETCH"
        })
        .collect();
    assert!(
        deprecated.is_empty(),
        "deprecated MUI2 macros in the inventory: {deprecated:#?}"
    );
}

/// A `todo` reason is a sentence, not a shrug. Same rule the instruction
/// table's `Todo` carries, and the same reason: Phase 6 found four ways a
/// one-word reason outlives its blocker.
#[test]
fn every_todo_carries_a_reason() {
    for entry in mui::inventory() {
        if let Class::Todo(reason) = entry.class {
            assert!(reason.len() > 20, "`{}` says only {reason:?}", entry.name());
        }
    }
}

/// The census is the burndown, so it is a golden of its own inside
/// `tests/golden/coverage.txt` — this only asserts the arithmetic.
#[test]
fn the_buckets_add_up() {
    let total: usize = mui::census().iter().map(|(_, count)| count).sum();
    assert_eq!(total, mui::inventory().len());
}

/// The two directions of the *other* join: the defines the compiler writes and
/// the names the inventory calls `Exposed`.
///
/// A define written but not exposed is a burndown that under-reports; a name
/// exposed but never written is one that over-reports. Both are the census
/// measuring something other than the compiler, which is the one failure a
/// coverage number cannot survive.
#[test]
fn the_defines_the_compiler_writes_are_exactly_the_exposed_ones() {
    let mut written = installua::lower::mui_defines();
    let mut exposed: Vec<&str> = mui::inventory()
        .iter()
        .filter(|entry| matches!(entry.class, Class::Exposed(_)))
        // The page macros, `MUI_LANGUAGE` and `MUI_HEADER_TEXT` are
        // `!insertmacro` lines rather than defines, so none of them is in the
        // list the lowerer builds. The snapshot's own `kind` column is what says
        // which is which — the alternative was a list of prefixes here, and a
        // prefix would have had to be right about `MUI_PAGE_HEADER_TEXT`, which
        // is a define whose name begins like a macro's.
        //
        // `both` counts as a setting, because that is what `both` means: MUI2
        // spells `MUI_ABORTWARNING` as a define the script writes *and* a macro
        // of its own by the same name, and the define is the half exposed here.
        .filter(|entry| entry.snapshot.kind == "setting" || entry.snapshot.kind == "both")
        .map(|entry| entry.name())
        .collect();

    written.sort_unstable();
    exposed.sort_unstable();
    assert_eq!(
        written, exposed,
        "the defines the lowerer writes and the inventory's `Exposed` rows disagree"
    );
}

/// The snapshot against a local NSIS, the way `tests/census.rs` checks
/// `-CMDHELP` against a local `makensis`.
///
/// Keyed on `NSISDIR` and skipped when it is unset, because MUI2 is a directory
/// of headers rather than a program that can be asked where it is: there is no
/// `makensis` flag that prints its own `$NSISDIR`, so the one thing this test
/// cannot do is find it for you.
#[test]
fn the_snapshot_matches_the_local_mui2() {
    let Ok(nsis) = std::env::var("NSISDIR") else {
        eprintln!("skipping: NSISDIR is unset");
        return;
    };
    let local = match mui::scan::scan(std::path::Path::new(&nsis)) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("skipping: {error}");
            return;
        }
    };
    let snapshot = std::fs::read_to_string("tables/mui-3.12.txt").expect("the snapshot");
    assert_eq!(
        local, snapshot,
        "the local Modern UI 2 and tables/mui-3.12.txt disagree: regenerate the \
         snapshot and classify whatever is new"
    );
}
