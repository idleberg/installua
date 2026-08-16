//! The instruction table: one struct, two sources, joined at first use (§15.23).
//!
//! ```text
//! tables/cmdhelp-3.12.txt  ─generate─▶  generated::SKELETONS  ┐
//!                                                             ├─join─▶  Instruction
//!                                        overlay::ROWS  ──────┘
//! ```
//!
//! **Two files, never two runtime tables.** The generated half knows arity,
//! optionality, flags, enum members and which positions are variables; the
//! hand-written half knows the Installua name, the census class, the types and
//! which positions are paths. A consumer that had to consult both could see
//! half an entry — a command with a name and no shape, or a shape with no
//! name — and there are three consumers (the emitter, the stub generator and
//! the census itself), so the join happens once, here, and everything
//! downstream reads [`Instruction`].
//!
//! The join is *total in both directions* and [`tests/census.rs`] asserts it:
//! every skeleton finds its row, every row finds its skeleton, and every row's
//! annotations are as long as its skeleton's parameter list. That is what makes
//! coverage a computed number rather than a document somebody remembers to
//! update (§14).

pub mod cmdhelp;
pub mod generated;
pub mod overlay;

use std::sync::OnceLock;

use crate::types::Ty;

/// Which side of the call a parameter is on. The number of [`Dir::Out`]
/// parameters **is** the number of Lua return values (§15.23), which is why
/// multiple returns need no special case anywhere else.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    In,
    Out,
}

/// A repeated *argument* — `File a b c` — as opposed to a repeated member
/// inside one argument, which is [`Kind::Flags`]. Only eleven non-preprocessor
/// commands use either, and they are two different things, so they are two
/// fields (§15.23).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rep {
    One,
    Many,
}

/// What the syntax line turned out to be, when it was not a parameter list.
///
/// Each variant exists because a row in 3.12 forced it, and each one is a claim
/// the census checks: prose rows must be `Rejected`, directive rows must be
/// `Directive`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Note {
    /// A parameter list, parsed in full.
    Plain,
    /// `SubSection deprecated - use SectionGroup`. NSIS prints English where
    /// the syntax would go, so there is no parameter model to read.
    Prose,
    /// A `!` command. §2 rules the preprocessor out of the surface, so these
    /// carry no parameter model by decision rather than by failure.
    Directive,
    /// The line has top-level alternation (`File`, `InstType`, `Exch`): two
    /// spellings of one command, which §15.23 makes *mutual exclusion between
    /// options* rather than a second parameter list. What is recorded is the
    /// first alternative; the overlay owns the `conflicts` sets.
    Alternation,
}

/// The generated half of one parameter: everything `-CMDHELP` states.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shape {
    pub name: &'static str,
    pub dir: Dir,
    /// A `$(user_var: …)` position: an NSIS *variable* is demanded, not a
    /// value, so the argument may not be an arbitrary expression spilled to a
    /// scratch register (§15.23).
    pub var: bool,
    /// `false` ⇒ bracketed in `-CMDHELP`.
    pub req: bool,
    pub rep: Rep,
    pub members: &'static [&'static str],
}

/// A flag. Not a parameter, because its position in NSIS syntax is arbitrary —
/// leading for `GetDLLVersion`, medial for `SetCtlColors`, trailing for
/// `SendMessage` — and there is nothing to gain from making a user learn that
/// (§15.23). The Installua surface takes an unordered options table; `after`
/// records where the emitter puts it back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Opt {
    pub nsis: &'static str,
    /// `/TIMEOUT=X` rather than `/BRANDING`.
    pub value: bool,
    /// The number of parameters this flag follows; 0 is leading.
    pub after: usize,
}

/// §15.2's fourth column and §15.23's `kind`. `Enum` is not written by hand —
/// it is [`Shape::members`] being non-empty — because the members come from
/// `-CMDHELP` and transcribing them is how a table drifts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Value,
    /// `/` becomes `\` here, and only here (§5).
    Path,
    /// The members are the enum's, case-folded on the way in.
    Enum,
    /// A table joined with `|`: `SetFileAttributes`, `FileOpen`'s mode.
    Flags,
    /// A position the **compiler** fills with a label, and the surface does not
    /// have at all: `IfFileExists f <then> <else>`.
    ///
    /// §15.23's four kinds have no way to say this, and §15.20 needs it said:
    /// a predicate is an ordinary `bool`-valued call, so `fileExists(p)` takes
    /// one argument and the other two positions are §8's business. Without the
    /// kind, the generated stub completes `fileExists(path, then, else)` and
    /// teaches the shape this language exists to remove.
    Label,
}

/// §14's census bucket. There is exactly one enum, because the buckets and the
/// overlay's `class` field are the same thing named twice (§15.23).
///
/// `Rejected` and `Todo` carry their text rather than a flag: an entry that says
/// "no" without saying why is indistinguishable from one nobody has looked at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// An Installua callable, with an emission example.
    Exposed,
    /// A field of one of the four blocks (§15.10, §15.26).
    Attribute,
    /// Reachable only as the output of `if`/`messageBox`, never callable. The
    /// emitter needs the shape; the stub generator must not offer it, or
    /// `intCmp(a, b, "yes", "no", "maybe")` reappears in completion and §8 is
    /// bypassed on day one (§13).
    ///
    /// The text is what the user writes instead. §15.23 spells this variant
    /// without a payload; it carries one here for the same reason `Rejected`
    /// does — a bucket that says "not callable" without saying what *is* leaves
    /// the reader exactly where the generic unknown-name error left them (§2) —
    /// and [`crate::retired`] reads it rather than keeping a second copy.
    LoweringTarget(&'static str),
    /// A preprocessor `!` command, out of the surface entirely (§2).
    Directive,
    /// The name is an Installua construct instead, and the text is that
    /// construct: `Function` is `func`, `Return` is `return`.
    Language(&'static str),
    /// Deliberately unavailable, with the diagnostic text that says so.
    Rejected(&'static str),
    /// Not yet done, with a one-line reason. The only honest backlog.
    Todo(&'static str),
}

impl Class {
    /// The bucket's name, which is also `installua coverage`'s row label and
    /// §14's vocabulary. One name per bucket, spelled once.
    pub fn bucket(&self) -> &'static str {
        match self {
            Class::Exposed => "exposed",
            Class::Attribute => "attribute",
            Class::LoweringTarget(_) => "lowering-target",
            Class::Directive => "directive",
            Class::Language(_) => "language",
            Class::Rejected(_) => "rejected",
            Class::Todo(_) => "todo",
        }
    }

    /// Every bucket, in the order `installua coverage` prints them: what is
    /// done, then what is deliberate, then what is left.
    pub const BUCKETS: &'static [&'static str] = &[
        "exposed",
        "attribute",
        "lowering-target",
        "language",
        "directive",
        "rejected",
        "todo",
    ];
}

/// One parameter, joined.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Param {
    pub shape: Shape,
    pub ty: Ty,
    pub kind: Kind,
}

impl Param {
    pub fn dir(&self) -> Dir {
        self.shape.dir
    }

    pub fn required(&self) -> bool {
        self.shape.req
    }

    /// The enum members, or empty when the position takes any value.
    pub fn members(&self) -> &'static [&'static str] {
        self.shape.members
    }
}

/// A command, both halves.
#[derive(Clone, Debug)]
pub struct Instruction {
    /// The Installua spelling, when there is one. `None` is not a gap: an
    /// `Attribute` is reached through a block field, a `Directive` is not
    /// reachable at all, and both are classified rather than missing.
    pub installua: Option<&'static str>,
    pub nsis: &'static str,
    pub class: Class,
    pub params: Vec<Param>,
    pub options: &'static [Opt],
    /// Mutually exclusive option sets: `File`'s `/oname=` branch against its
    /// repeated-filespec branch. The error names both spellings (§15.23).
    pub conflicts: &'static [&'static [&'static str]],
    pub note: Note,
}

impl Instruction {
    /// The outputs, in emit order. Their count is the Lua arity, and their
    /// *positions* are what `GetDLLVersion` and `FindFirst` disagree about.
    pub fn outputs(&self) -> impl Iterator<Item = &Param> {
        self.params.iter().filter(|param| param.dir() == Dir::Out)
    }

    pub fn inputs(&self) -> impl Iterator<Item = &Param> {
        self.params.iter().filter(|param| param.dir() == Dir::In)
    }
}

/// The joined table. Built once, on first use — never rebuilt, and never
/// mutated, so §9-2's re-entrancy holds: this is shared immutable data rather
/// than global state.
pub fn table() -> &'static [Instruction] {
    static TABLE: OnceLock<Vec<Instruction>> = OnceLock::new();
    TABLE.get_or_init(join)
}

/// Looks a command up by its NSIS name, case-insensitively — NSIS itself is
/// case-insensitive here and a user who typed `writeregstr` deserves the row
/// rather than "no such command".
pub fn by_nsis(nsis: &str) -> Option<&'static Instruction> {
    table()
        .iter()
        .find(|entry| entry.nsis.eq_ignore_ascii_case(nsis))
}

/// Looks a command up by its Installua name. Case-*sensitive*: `detailPrint` is
/// the name and `detailprint` is a typo `builtins::nearest` already answers.
pub fn by_installua(name: &str) -> Option<&'static Instruction> {
    table().iter().find(|entry| entry.installua == Some(name))
}

/// The bucket counts `installua coverage` prints, in `Class::BUCKETS` order.
pub fn census() -> Vec<(&'static str, usize)> {
    Class::BUCKETS
        .iter()
        .map(|bucket| {
            let count = table()
                .iter()
                .filter(|entry| entry.class.bucket() == *bucket)
                .count();
            (*bucket, count)
        })
        .collect()
}

/// What `installua coverage` prints, and a golden file (§14).
///
/// Counts first, because that is the number anybody quotes; then the `todo`
/// names with their reasons, because a count that drops by twelve does not say
/// which twelve and a diff of names does.
pub fn coverage() -> String {
    let mut out = format!(
        "installua coverage -- makensis {}, {} commands\n\n",
        generated::VERSION,
        table().len()
    );

    for (bucket, count) in census() {
        out.push_str(&format!("  {bucket:<16}{count:>4}\n"));
    }

    let todo: Vec<&Instruction> = table()
        .iter()
        .filter(|entry| matches!(entry.class, Class::Todo(_)))
        .collect();
    if !todo.is_empty() {
        out.push_str("\ntodo:\n");
        for entry in todo {
            let Class::Todo(reason) = entry.class else {
                continue;
            };
            out.push_str(&format!("  {:<24}{reason}\n", entry.nsis));
        }
    }

    out
}

/// The join itself.
///
/// It is deliberately total and deliberately loud in tests but quiet here: a
/// skeleton with no row keeps its shape and lands in `Todo`, because a compiler
/// that refuses to start because a *new NSIS version* added a command would be
/// unusable for the one thing that fixes it. The census test is where the
/// missing row is a failure.
fn join() -> Vec<Instruction> {
    generated::SKELETONS
        .iter()
        .map(|skeleton| {
            let row = overlay::lookup(skeleton.nsis);

            let params = skeleton
                .params
                .iter()
                .enumerate()
                .map(|(index, shape)| {
                    let annotation = row.and_then(|row| row.params.get(index)).copied();
                    let (ty, kind) = match annotation {
                        Some(annotation) => (annotation.ty, annotation.kind),
                        // No row, or a row shorter than the shape: the shape is
                        // still the truth about NSIS, so it survives with the
                        // weakest possible reading of it.
                        None => (Ty::Unknown, Kind::Value),
                    };
                    let kind = if !shape.members.is_empty() && kind == Kind::Value {
                        Kind::Enum
                    } else {
                        kind
                    };
                    Param {
                        shape: *shape,
                        ty,
                        kind,
                    }
                })
                .collect();

            Instruction {
                installua: row.and_then(|row| row.installua),
                nsis: skeleton.nsis,
                class: match row {
                    Some(row) => row.class,
                    None => Class::Todo("no overlay row: added by a newer NSIS"),
                },
                params,
                options: skeleton.options,
                conflicts: row.map(|row| row.conflicts).unwrap_or(&[]),
                note: skeleton.note,
            }
        })
        .collect()
}
