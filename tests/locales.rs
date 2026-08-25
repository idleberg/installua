//! The NLF snapshot against a local NSIS.
//!
//! `tests/mui.rs` is the model, and until the scraper moved in here there was
//! no test at all on this side: `tables/locales-3.12.txt` was written by a
//! command someone had to remember to run, so a release that added a language
//! would have gone unnoticed until a user wrote its name and was told it does
//! not exist.

use std::path::Path;

mod common;

/// The snapshot against `Contrib/Language files` — and, with `UPDATE_SNAPSHOTS`
/// set, the thing that writes it.
///
/// Keyed on `NSISDIR` and skipped when it is unset, for the reason
/// `tests/mui.rs` gives: the languages are a directory beside `makensis` and
/// not something `makensis` will name.
///
/// ```text
/// NSISDIR=… UPDATE_SNAPSHOTS=1 cargo test --test locales
/// ```
#[test]
fn the_snapshot_matches_the_local_nsis() {
    let Ok(nsis) = std::env::var("NSISDIR") else {
        eprintln!("skipping: NSISDIR is unset");
        return;
    };
    let local = match installua::locale::scan(Path::new(&nsis)) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("skipping: {error}");
            return;
        }
    };
    common::check_or_update(
        "tables/locales-3.12.txt",
        &local,
        "check that each new name's `LANG_*` define is its name uppercased",
    );
}

/// Every name the snapshot carries is one the compiler will accept.
///
/// The scraper and the reader are separate halves — one writes the file, the
/// other `include_str!`s it — so a formatting change in the header that swallowed
/// a name would leave both halves passing their own tests and the language list
/// quietly short.
#[test]
fn every_snapshot_name_is_a_known_locale() {
    let all = installua::locale::all();
    assert!(
        all.len() > 60,
        "the snapshot parsed to {} names, which is too few to be the NLF list",
        all.len()
    );
    assert!(
        all.contains(&"PortugueseBR"),
        "`PortugueseBR` is the worked example and must survive the split"
    );
    assert!(
        all.iter().all(|name| !name.starts_with('#')),
        "a comment line reached the name list"
    );
    let mut sorted = all.clone();
    sorted.sort();
    assert_eq!(all, sorted, "the snapshot is documented as sorted");
}
