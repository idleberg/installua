//! The MUI2 inventory: the second surface, counted the way the first one is.
//!
//! ```text
//! tables/mui-3.12.txt  ─parse─▶  snapshot rows  ┐
//!                                               ├─join─▶  Setting
//!                                   rows::ROWS  ┘
//! ```
//!
//! The instruction table's census answers *what of NSIS can be written*, and for
//! the whole of Phase 6 it was the only number there was — which left the `MUI_*`
//! surface invisible, because a MUI2 setting is a `!define` and not a command, so
//! no `-CMDHELP` line exists to put it in a bucket. `PLAN.md` carried "roughly
//! seventy" as an estimate nobody could check. This module is that estimate
//! replaced by a count.
//!
//! **No generated Rust.** The instruction table compiles its snapshot into
//! `table::generated` because a `-CMDHELP` line is a parameter grammar and
//! parsing one at startup would be real work. A row here is a name, a handful of
//! tags and a site, so the snapshot is [`include_str!`]'d and split — one file
//! instead of two, and the same guarantee, since a name with no row and a row
//! with no name are both test failures either way.

pub mod rows;
pub mod scan;

use std::collections::BTreeMap;
use std::sync::OnceLock;

/// The checked-in snapshot, so the census runs with no NSIS installed (§14).
const SNAPSHOT: &str = include_str!("../../tables/mui-3.12.txt");

/// What Installua does with one MUI2 name.
///
/// Four buckets and not seven: an instruction can be a directive, a language
/// construct or a lowering target, and a `!define` can be none of those. What is
/// the same is that the two "no" buckets carry their reason, for the reason
/// [`crate::table::Class`] gives — an entry that says no without saying why is
/// indistinguishable from one nobody has read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// Writable today. The text is the Installua spelling.
    Exposed(&'static str),
    /// MUI2's own state: a define it writes and reads itself, or a macro it
    /// inserts for you. Nothing for a user to write, and not a gap.
    Internal,
    /// Deliberately unavailable, with the reason.
    Rejected(&'static str),
    /// Not yet, with a one-line reason. The only honest backlog.
    Todo(&'static str),
}

impl Class {
    pub fn bucket(&self) -> &'static str {
        match self {
            Class::Exposed(_) => "exposed",
            Class::Internal => "internal",
            Class::Rejected(_) => "rejected",
            Class::Todo(_) => "todo",
        }
    }

    pub const BUCKETS: &'static [&'static str] = &["exposed", "internal", "rejected", "todo"];
}

/// The hand-written half: one per snapshot name.
#[derive(Clone, Copy, Debug)]
pub struct Row {
    pub name: &'static str,
    pub class: Class,
}

/// The snapshot half: what MUI2's text says about the name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub name: String,
    pub kind: String,
    pub tags: Vec<String>,
    pub site: String,
    pub within: String,
}

impl Snapshot {
    pub fn has(&self, tag: &str) -> bool {
        self.tags.iter().any(|each| each == tag)
    }

    /// Page-scoped in MUI2's own terms: cleared after the page it was read at,
    /// so a second page of the same type does not inherit it (batch 20).
    pub fn page_scoped(&self) -> bool {
        self.has("page") && !self.has("once")
    }
}

/// One name, joined.
#[derive(Clone, Debug)]
pub struct Setting {
    pub snapshot: Snapshot,
    pub class: Class,
}

impl Setting {
    pub fn name(&self) -> &str {
        &self.snapshot.name
    }
}

/// Every snapshot line, parsed. Comments and blanks are the header.
pub fn snapshot() -> &'static [Snapshot] {
    static PARSED: OnceLock<Vec<Snapshot>> = OnceLock::new();
    PARSED.get_or_init(|| SNAPSHOT.lines().filter_map(parse).collect())
}

fn parse(line: &str) -> Option<Snapshot> {
    if line.starts_with('#') || line.trim().is_empty() {
        return None;
    }
    // Columns are separated by *two or more* spaces, and a tag list is single
    // spaced — which is what lets the two be told apart without counting
    // characters. Counting them was the first attempt, and one MUI2 macro name
    // is fifty-two characters long, so the columns it overflows would have
    // shifted every field after it.
    let columns: Vec<&str> = line
        .split("  ")
        .filter(|part| !part.trim().is_empty())
        .collect();
    let [kind, name, rest @ ..] = columns.as_slice() else {
        return None;
    };
    // The site is the one column with a line number in it, so the tag list is
    // whatever came before it and the enclosing macro whatever came after.
    let at = rest
        .iter()
        .position(|part| part.contains(".nsh:"))
        .unwrap_or(rest.len());
    Some(Snapshot {
        name: name.trim().to_string(),
        kind: kind.trim().to_string(),
        tags: rest[..at]
            .iter()
            .flat_map(|part| part.split_whitespace())
            .map(str::to_string)
            .collect(),
        site: rest
            .get(at)
            .map(|part| part.trim().to_string())
            .unwrap_or_default(),
        within: rest
            .get(at + 1)
            .map(|part| part.trim().to_string())
            .unwrap_or_default(),
    })
}

/// The joined inventory. Same shape as [`crate::table::table`] and the same
/// promise: a name with no row keeps its snapshot half and lands in `Todo`,
/// because a compiler that refused to start because a *newer MUI2* added a
/// define would be unusable for the one thing that fixes it.
pub fn inventory() -> &'static [Setting] {
    static JOINED: OnceLock<Vec<Setting>> = OnceLock::new();
    JOINED.get_or_init(|| {
        let rows: BTreeMap<&str, Class> =
            rows::ROWS.iter().map(|row| (row.name, row.class)).collect();
        snapshot()
            .iter()
            .map(|snapshot| Setting {
                class: rows
                    .get(snapshot.name.as_str())
                    .copied()
                    .unwrap_or(Class::Todo("no inventory row: added by a newer MUI2")),
                snapshot: snapshot.clone(),
            })
            .collect()
    })
}

pub fn by_name(name: &str) -> Option<&'static Setting> {
    inventory().iter().find(|entry| entry.name() == name)
}

/// The bucket counts, in `Class::BUCKETS` order.
pub fn census() -> Vec<(&'static str, usize)> {
    Class::BUCKETS
        .iter()
        .map(|bucket| {
            let count = inventory()
                .iter()
                .filter(|entry| entry.class.bucket() == *bucket)
                .count();
            (*bucket, count)
        })
        .collect()
}

/// The MUI half of what `installua coverage` prints, and part of its golden.
///
/// `internal` is printed beside the rest rather than subtracted from the total,
/// because "MUI2 has 255 names and 72 of them are its own" is the fact, and a
/// denominator quietly adjusted is how a coverage number stops meaning anything.
pub fn coverage() -> String {
    let mut out = format!(
        "\ninstallua MUI coverage -- Modern UI 2, {} names\n\n",
        inventory().len()
    );

    for (bucket, count) in census() {
        out.push_str(&format!("  {bucket:<16}{count:>4}\n"));
    }

    let todo: Vec<&Setting> = inventory()
        .iter()
        .filter(|entry| matches!(entry.class, Class::Todo(_)))
        .collect();
    if !todo.is_empty() {
        out.push_str("\nmui todo:\n");
        for entry in todo {
            let Class::Todo(reason) = entry.class else {
                continue;
            };
            out.push_str(&format!("  {:<48}{reason}\n", entry.name()));
        }
    }

    out
}
