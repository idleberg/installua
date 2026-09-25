//! The header census's join, both ways, the way `tests/mui.rs` asserts MUI2's.

use installua::headers::{self, Class};

mod common;

#[test]
fn every_header_has_a_row_and_every_row_a_header() {
    let snapshot = headers::snapshot();
    let missing: Vec<_> = snapshot
        .iter()
        .filter(|path| !headers::ROWS.iter().any(|(name, _)| name == *path))
        .collect();
    let stale: Vec<_> = headers::ROWS
        .iter()
        .map(|(name, _)| name)
        .filter(|name| !snapshot.contains(name))
        .collect();
    assert!(missing.is_empty(), "headers with no row: {missing:?}");
    assert!(stale.is_empty(), "rows naming no header: {stale:?}");
    assert_eq!(snapshot.len(), headers::ROWS.len(), "a header has two rows");
}

#[test]
fn every_todo_carries_a_reason() {
    for (path, class) in headers::inventory() {
        if let Class::Todo(reason) = class {
            assert!(reason.len() > 20, "`{path}` says only {reason:?}");
        }
    }
}

/// ```text
/// NSISDIR=… UPDATE_SNAPSHOTS=1 cargo test --test headers
/// ```
#[test]
fn the_snapshot_matches_the_local_nsis() {
    let Ok(nsis) = std::env::var("NSISDIR") else {
        eprintln!("skipping: NSISDIR is unset");
        return;
    };
    let local = match headers::scan(std::path::Path::new(&nsis)) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("skipping: {error}");
            return;
        }
    };
    common::check_or_update(
        "tables/headers-3.12.txt",
        &local,
        "classify whatever is new in `headers::ROWS`",
    );
}
