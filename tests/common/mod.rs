//! The snapshot scrapers' one direction-switch, shared by their drift tests.
//!
//! Not a test target: `cargo` compiles `tests/*.rs`, so a `tests/common/`
//! module is a library the test binaries include rather than a suite of its
//! own.

/// Assert the checked-in snapshot matches, or rewrite it — `UPDATE_SNAPSHOTS`
/// picks.
///
/// Unset, which is what CI runs, this is the drift check it always was: the
/// file is read and compared against what the scraper found in a local NSIS.
/// Set, the scraper *is* the generator and the file is rewritten.
///
/// One function rather than a test beside a `generate` subcommand, because the
/// two halves then cannot disagree about what the file should hold: the bytes
/// asserted and the bytes written come from the same call. The command they
/// replace could be forgotten after a failure, or run against a different NSIS
/// than the one the test read.
pub fn check_or_update(path: &str, generated: &str, hint: &str) {
    if std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
        std::fs::write(path, generated)
            .unwrap_or_else(|error| panic!("cannot write {path}: {error}"));
        eprintln!("wrote {path}");
        return;
    }

    let snapshot = std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("cannot read {path}: {error}"));
    assert_eq!(
        generated, snapshot,
        "the local NSIS and {path} disagree: rerun with \
         `UPDATE_SNAPSHOTS=1 cargo test` to refresh it, then {hint}"
    );
}
