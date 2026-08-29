//! The six macros NSIS ships that call back into the script, and the register
//! protocol each one uses.
//!
//! # Why this is code and not a declaration
//!
//! Every other macro is describable by a `.toml`: how many arguments in, how
//! many registers out, which end they go on. These six are not. NSIS calls the
//! script's own function once per line or per file, and hands the values over
//! in **registers it names** — `$R9` down to `$R6` for `FileFunc`'s pair,
//! `$9` down to `$6` for `TextFunc`'s four — with the answer coming back as a
//! string pushed on the stack. `LineFind` goes further and reads `$R9` again
//! afterwards, so its first argument is an in/out parameter.
//!
//! That is behaviour, not arity, and [`crate::declarations`]' rule is that behaviour
//! stays out of a declaration file. The reason is not tidiness: a register map
//! written down wrongly emits NSIS that assembles and runs, handing the caller
//! a directory where it asked for a file name. Nothing downstream can catch it,
//! because nothing downstream knows what `$R8` was supposed to hold. Here the
//! map is one table, read by one lowering, pinned by one golden.
//!
//! The cost is that a **third-party** callback macro cannot be declared at all:
//! `params = [… , "callback"]` on a `nsis` name this table does not know is an
//! error. That is deliberate. The alternative was letting anyone write
//! `["$R9", "$R8", "$R7", "$R6"]` into a `.toml`, which is the one shape of
//! mistake this compiler exists to make impossible.
//!
//! # How the values get in and out
//!
//! The prologue **pushes every input register before popping any of them**:
//!
//! ```text
//! Push $R6 / Push $R7 / Push $R8 / Push $R9
//! Pop <slot> / Pop <slot> / Pop <slot> / Pop <slot>
//! ```
//!
//! Eight instructions where four `StrCpy`s would look like enough, and the four
//! would be wrong. The slots are virtual until [`crate::alloc`] colours them,
//! and it may well colour the first one `$R8` — which would clobber the second
//! input before the copy that reads it. Reading all four onto the stack first
//! is correct under *every* colouring, which is what makes it the version that
//! does not depend on a pass this file cannot see.
//!
//! The epilogue is the mirror: `LineFind`'s rewritten line goes back into `$R9`
//! last, and the sentinel is pushed after it.

/// Where one of a callback's arguments arrives.
///
/// A register number in [`crate::regs`]' numbering: `$0`–`$9` are 0–9 and
/// `$R0`–`$R9` are 10–19.
pub type Register = u8;

/// `$R9` … `$R6`, the four `FileFunc` uses.
const R9: Register = 19;
const R8: Register = 18;
const R7: Register = 17;
const R6: Register = 16;

/// `$9` … `$6`, which `TextFunc`'s comparers use instead. Nothing chose this
/// difference; the two headers were written by different people.
const N9: Register = 9;
const N8: Register = 8;
const N7: Register = 7;
const N6: Register = 6;

/// What one argument is, in the only two flavours these six deal in.
///
/// Spelled as its own enum rather than as a [`crate::types::Ty`] because the
/// table below is a `const`, and `Ty`'s integer forms are built by functions.
/// The mapping is one `match` at the binding site and stays there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Text: a path, a name, a line, a drive letter.
    Text,
    /// A count or a line number. Unsigned, which is what lets a body write
    /// `number // 2` with no sign fixup — and true of every one of them, since
    /// NSIS counts lines and bytes from zero upward.
    Count,
}

/// What a callback body may do with the value it was handed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// Read the arguments, answer "keep going" or "stop". A `for … in` loop:
    /// falling off the end continues and `break` stops.
    Iterate,
    /// Read the arguments and answer with a **line to write**, which goes back
    /// through `$R9`. Not expressible as a loop body, so it stays a function
    /// with a return value.
    Rewrite,
}

/// One macro's protocol.
pub struct Protocol {
    /// The `nsis` name in the declaration — `Locate`, `LineFind` — which is
    /// what this table is keyed by. Two declarations may reach the same
    /// protocol: `TextCompare` and `TextCompareS` differ only in case folding,
    /// which the callback never sees.
    pub nsis: &'static str,
    /// Which registers the arguments arrive in, in the order a program binds
    /// them. Binding fewer names than this is fine and common — `for path in
    /// locate(…)` wants one of four — and binding more is an error.
    pub inputs: &'static [Register],
    /// What each input is called, for the diagnostic that has to say which
    /// position a program overran.
    pub names: &'static [&'static str],
    /// What each input *is*, positionally alongside `names`. A callback's
    /// parameters cannot take their types from a call site the way every other
    /// parameter in this language does — NSIS is the caller — so this list is
    /// the only place they come from.
    pub types: &'static [Kind],
    /// The string that ends the walk, pushed in place of the empty one.
    pub stop: &'static str,
    /// The string that skips this line's output, where there is an output to
    /// skip. `LineFind` alone.
    pub skip: Option<&'static str>,
    pub shape: Shape,
}

/// Every callback macro the compiler knows, and the whole of what it knows.
///
/// Each `inputs` list was read out of the macro body in `NSISDIR/Include`, at
/// the `Call` and the `Pop` that brackets it — not from a wiki page, and not
/// from the argument names in the `…Call` wrapper, which are the *outer*
/// macro's and say nothing about what the callback sees.
pub const PROTOCOLS: &[Protocol] = &[
    // `${Locate}` sets `$R9` to `'$R8\$R7'` immediately before calling, so the
    // full path and its two halves are all live at once. `$R6` is the size in
    // whatever unit `/S=` asked for, and is empty for a directory.
    Protocol {
        nsis: "Locate",
        inputs: &[R9, R8, R7, R6],
        names: &["path", "directory", "name", "size"],
        types: &[Kind::Text, Kind::Text, Kind::Text, Kind::Count],
        stop: "StopLocate",
        skip: None,
        shape: Shape::Iterate,
    },
    // Two only: the drive string and the type word `GetDriveType` produced —
    // `FDD`, `HDD`, `NET`, `CDROM` or `RAM`.
    Protocol {
        nsis: "GetDrives",
        inputs: &[N9, N8],
        names: &["drive", "kind"],
        types: &[Kind::Text, Kind::Text],
        stop: "StopGetDrives",
        skip: None,
        shape: Shape::Iterate,
    },
    // The one that writes back. `$R9` arrives holding the line and is read
    // again after the call to decide what lands in the output file, which is
    // why this shape is a function and not a loop.
    Protocol {
        nsis: "LineFind",
        inputs: &[R9, R8, R7, R6],
        names: &["line", "number", "fromEnd", "range"],
        types: &[Kind::Text, Kind::Count, Kind::Count, Kind::Text],
        stop: "StopLineFind",
        skip: Some("SkipWrite"),
        shape: Shape::Rewrite,
    },
    // Backwards through a file. `$8` counts down the lines left and `$7` is
    // the line's number from the start, so a body that wants "the tenth from
    // the end" reads the first and "line 90" reads the second.
    Protocol {
        nsis: "FileReadFromEnd",
        inputs: &[N9, N8, N7],
        names: &["line", "remaining", "number"],
        types: &[Kind::Text, Kind::Count, Kind::Count],
        stop: "StopFileReadFromEnd",
        skip: None,
        shape: Shape::Iterate,
    },
    // `$7` is the *other* file's line and `$6` is its number, or `0` when the
    // line matched nothing — which is what makes this a diff rather than a
    // walk.
    Protocol {
        nsis: "TextCompare",
        inputs: &[N9, N8, N7, N6],
        names: &["line", "number", "other", "match"],
        types: &[Kind::Text, Kind::Count, Kind::Text, Kind::Count],
        stop: "StopTextCompare",
        skip: None,
        shape: Shape::Iterate,
    },
    Protocol {
        nsis: "TextCompareS",
        inputs: &[N9, N8, N7, N6],
        names: &["line", "number", "other", "match"],
        types: &[Kind::Text, Kind::Count, Kind::Text, Kind::Count],
        stop: "StopTextCompare",
        skip: None,
        shape: Shape::Iterate,
    },
];

/// The protocol for a declared macro, or `None` for a `nsis` name this table
/// does not know — which is what makes a third-party `callback` declaration an
/// error rather than a guess.
pub fn protocol(nsis: &str) -> Option<&'static Protocol> {
    PROTOCOLS.iter().find(|entry| entry.nsis == nsis)
}

/// The names this table knows, for the diagnostic that has to list them.
pub fn known() -> String {
    let mut names: Vec<&str> = PROTOCOLS.iter().map(|entry| entry.nsis).collect();
    names.sort_unstable();
    names.join("`, `")
}
