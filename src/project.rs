//! `installua.toml`: the marker that says where a project's search scope ends.
//!
//! The file holds no settings. What it does is bound a *walk*: a compile reads
//! every `.installua/declarations` from the source's directory up to the
//! marker that says `root = true`, so several installers in one checkout can
//! share one declaration of a vendored plugin instead of copying the same five
//! lines into each. Without it a declaration has to sit beside every `.lua`
//! file that needs it, which is the only problem this module solves.
//!
//! Absence is the common case and means today's behaviour exactly: no marker
//! anywhere leaves the source's own directory as the whole search scope.
//!
//! The walk stops at three things, and each rules out a different way for a
//! build to read a directory nobody meant it to:
//!
//! - a marker with `root = true`, which is the deliberate one;
//! - a directory holding `.git`, because a checkout is a scope whether or not
//!   anyone wrote a marker at its top;
//! - the filesystem root, without which a stray `installua.toml` in `$HOME`
//!   would join every build on the machine.
//!
//! Parsed here rather than by a dependency, for the reason
//! [`crate::declarations`] parses its own files: three keys and two types do
//! not need a deserialiser, and the messages can name the key that was wrong.

use std::path::{Path, PathBuf};

use crate::declarations::{DIRECTORY, Problem};

/// The marker's file name.
pub const MARKER: &str = "installua.toml";

/// One marker, read.
///
/// Only `root` is a field: it is the one key whose *value* changes what the
/// compiler does today. `entry` and `name` are accepted by the parser and
/// carried nowhere, because the commands that would read them —
/// `installua build` with no file, `--project <name>` — do not exist yet, and
/// a field nothing reads is a promise the CLI has not made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Marker {
    /// The directory the file sits in.
    pub dir: PathBuf,
    pub root: bool,
}

/// The markers above `dir`, **outermost first**, and whatever they got wrong.
///
/// Outermost first because that is the order the declarations have to be
/// loaded in: `.installua/declarations` files already resolve a clash by
/// "later wins", so a project overriding the workspace's declaration of a
/// plugin is the same rule as a project's second file overriding its first.
pub fn chain(dir: &Path) -> (Vec<Marker>, Vec<Problem>) {
    // Canonical, because the walk is `parent()` in a loop: a relative `dir`
    // runs out of components after two steps and would stop inside the
    // checkout it was supposed to climb. A path that cannot be canonicalised
    // is walked as it stands rather than refused — it is about to fail at the
    // read anyway, and with a better message than this could give.
    let start = dir.canonicalize();
    let mut current: &Path = start.as_deref().unwrap_or(dir);

    let mut markers = Vec::new();
    let mut problems = Vec::new();
    loop {
        if let Some((marker, mut found)) = read(current) {
            let stop = marker.root;
            markers.push(marker);
            problems.append(&mut found);
            if stop {
                break;
            }
        }
        // Checked after the marker is taken, so the marker at the top of a
        // checkout is read and only then stopped on.
        if current.join(".git").exists() {
            break;
        }
        match current.parent() {
            Some(parent) => current = parent,
            None => break,
        }
    }

    markers.reverse();
    (markers, problems)
}

/// Every directory a compile from `dir` reads declarations from, outermost
/// first.
///
/// `dir` itself is always last, marker or no marker. That is what makes the
/// cascade purely additive: a project with no `installua.toml` anywhere reads
/// exactly the one directory it read before this module existed, and one with
/// markers reads those as well — never instead.
pub fn declaration_dirs(dir: &Path) -> (Vec<PathBuf>, Vec<Problem>) {
    let (markers, problems) = chain(dir);
    let mut dirs: Vec<PathBuf> = markers
        .into_iter()
        .map(|marker| marker.dir.join(DIRECTORY))
        .collect();

    // The nearest marker is usually `dir`'s own, in which case this is the
    // same directory under a canonical name; the compare is against the
    // *canonical* form so the two do not both get loaded, which would report
    // every declaration in it as declared twice.
    let own = dir.join(DIRECTORY);
    let canonical = own.canonicalize().unwrap_or_else(|_| own.clone());
    if !dirs.contains(&canonical) {
        dirs.push(own);
    }
    (dirs, problems)
}

/// The marker in `dir`, if there is one.
///
/// A file that exists and cannot be read is a problem rather than an absence —
/// that is a permission or a broken link, and treating it as "no marker" would
/// silently narrow the search scope.
fn read(dir: &Path) -> Option<(Marker, Vec<Problem>)> {
    let path = dir.join(MARKER);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
        Err(error) => {
            return Some((
                Marker {
                    dir: dir.to_path_buf(),
                    root: false,
                },
                vec![Problem {
                    file: path.display().to_string(),
                    line: 0,
                    message: format!("cannot read: {error}"),
                }],
            ));
        }
    };
    let mut problems = Vec::new();
    let root = parse(&path.display().to_string(), &text, &mut problems);
    Some((
        Marker {
            dir: dir.to_path_buf(),
            root,
        },
        problems,
    ))
}

/// The marker file format: an optional `[project]` header and `key = value`
/// lines under it. Returns what `root` was set to.
///
/// An unknown key is a problem, and the CLI stops on it. A typo'd
/// `rooot = true` that silently does nothing is the `-DVERSOIN` failure mode
/// that [`crate::diag::Code::UnknownParam`] exists to reject, and here it would
/// compile the program against a different set of declarations than the author
/// meant.
fn parse(file: &str, text: &str, problems: &mut Vec<Problem>) -> bool {
    let mut root = false;
    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        let content = strip(raw);
        if content.is_empty() {
            continue;
        }

        let mut complain = |message: String| {
            problems.push(Problem {
                file: file.to_string(),
                line,
                message,
            });
        };

        if content.starts_with('[') {
            if content != "[project]" {
                complain(format!(
                    "`{content}` is not a marker table; the only one is `[project]`"
                ));
            }
            continue;
        }

        let Some((key, value)) = content.split_once('=') else {
            complain(format!(
                "`{content}` is neither `[project]` nor `key = value`"
            ));
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        match key {
            "root" => match value {
                "true" => root = true,
                "false" => {}
                other => complain(format!("`root` wants `true` or `false`, not `{other}`")),
            },
            // Accepted and carried nowhere; see [`Marker`]. Still type-checked,
            // so that a marker written today for a command that lands later is
            // wrong here rather than at the moment it starts mattering.
            "entry" | "name" => {
                if !(value.starts_with('"') && value.ends_with('"') && value.len() >= 2) {
                    complain(format!("`{key}` wants a quoted string"));
                }
            }
            other => complain(format!(
                "`{other}` is not a marker field; the fields are `root`, `entry` and `name`"
            )),
        }
    }
    root
}

/// The line with its comment removed, as [`crate::declarations`] does it: a
/// `#` inside a quoted string is text.
fn strip(line: &str) -> &str {
    let mut quoted = false;
    for (index, byte) in line.bytes().enumerate() {
        match byte {
            b'"' => quoted = !quoted,
            b'#' if !quoted => return line[..index].trim(),
            _ => {}
        }
    }
    line.trim()
}
