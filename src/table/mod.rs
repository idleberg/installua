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
pub mod doc;
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
    /// The members are worth *offering* but not worth enforcing, because
    /// `-CMDHELP` ended the list with a placeholder: `…|Win10|{GUID}` accepts
    /// any GUID beside the seven names, and a closed check would reject them.
    pub open: bool,
}

/// A flag, as `-CMDHELP` states it. Not a parameter, because its position in
/// NSIS syntax is arbitrary — leading for `GetDLLVersion`, medial for
/// `SetCtlColors`, trailing for `SendMessage` — and there is nothing to gain
/// from making a user learn that (§15.23). The Installua surface takes an
/// unordered options table; `after` records where the emitter puts it back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Opt {
    pub nsis: &'static str,
    /// `/TIMEOUT=X` rather than `/BRANDING`.
    pub value: bool,
    /// The number of parameters this flag follows; 0 is leading.
    pub after: usize,
}

/// What the Installua surface does with one flag. The overlay's judgement, one
/// entry per snapshot flag, the way [`Ann`](overlay::Ann) is one entry per
/// snapshot parameter.
///
/// [`Offer::Unoffered`] carries its reason for the same argument
/// [`Class::Todo`] does: a flag that says "not yet" without saying why is
/// indistinguishable from one nobody has read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Offer {
    /// A named field of the trailing options table, holding a `bool`. The
    /// compiler writes the `/FLAG`, so `delete(p, { rebootOk = true })` is the
    /// whole surface and the spelling never leaves the table.
    Named(&'static str),
    /// Written on every call, because NSIS requires it and the caller has
    /// nothing to decide: `WriteRegMultiStr` without `/REGEDIT5` is an error
    /// rather than a different instruction.
    Always,
    /// A named field holding a *list*, written as one `/FLAG value` pair per
    /// element: `file(p, { exclude = { "*.tmp", "*.log" } })` becomes `File /x
    /// "*.tmp" /x "*.log" p`. The repetition is the overlay's to know — the
    /// snapshot records that `/x` takes a value ([`Opt::value`]) and not that
    /// the syntax line lets it repeat.
    List {
        name: &'static str,
        /// The element's kind, applied to each one: `Path` for `/x`, because a
        /// filespec is a path and §5 turns `/` into `\`.
        kind: Kind,
    },
    /// A named field of a *hand-shaped* row's own table, written by the
    /// lowering that shapes it rather than by [`Instruction::flags`].
    /// `MessageBox`'s `/SD` is the one: the answer it names has to be legal for
    /// the button set beside it, and checking one field against another is a
    /// thing the generic options table cannot do (§15.18).
    Handled(&'static str),
    /// Not offered, with the reason — which as of batch 9 is always the join's
    /// own: a row that says nothing about a flag leaves it here. Every flag on
    /// an `Exposed` row is reachable, and the census is what keeps it that way.
    Unoffered(&'static str),
}

/// One flag, joined: what NSIS prints, and what the surface does with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Flag {
    pub opt: Opt,
    pub offer: Offer,
}

impl Flag {
    /// The options-table name, when the flag has one. `Handled` has one too:
    /// the name is legal inside its row's table, and only *who writes it*
    /// differs.
    pub fn name(&self) -> Option<&'static str> {
        match self.offer {
            Offer::Named(name) | Offer::List { name, .. } | Offer::Handled(name) => Some(name),
            Offer::Always | Offer::Unoffered(_) => None,
        }
    }
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
    /// A position that is not one, because the help text has a typo in it.
    ///
    /// `-CMDHELP` prints `CreateShortcut … [icon index [showmode …`, and `icon
    /// index` is a *single* argument missing its underscore. NSIS's own parser
    /// settles it — `script.cpp` reads token 5 once, with `gettoken_int`, and
    /// its error message calls the thing "icon index" as well — and `tokens.cpp`
    /// agrees on the count: `{TOK_CREATESHORTCUT, …, 2, 7, …}` is nine tokens,
    /// one of which is the `/NoWorkingDir` flag `eattoken` removes. Eight
    /// arguments.
    ///
    /// The snapshot records what `makensis` *prints*, which is the whole point
    /// of it, so the correction cannot live there. It is judgement and it lives
    /// in the overlay: the second half is a parameter in `generated.rs` and
    /// nowhere else — never offered, never emitted, never counted. Without it
    /// `createShortcut` emits nine arguments, which assembles and puts the
    /// description in the keyboard shortcut.
    Fused,
    /// A position the compiler resolves from a **name**, so the caller writes
    /// neither an argument nor a value here: `SectionSetText ${SEC_core} "…"`
    /// takes its index from the handle the field was written on, and
    /// `SetCurInstType 1` takes its position from the block's `installTypes`
    /// list (§13).
    ///
    /// Between [`Kind::Label`] and [`Kind::Fused`] and neither of them. A label
    /// is a jump target the compiler *invents*; a fused position is not a
    /// position at all. This one is emitted, is counted by NSIS, and holds a
    /// number that exists in exactly one place — which is the whole reason it
    /// cannot be an argument: an index the surface could write is a second
    /// numbering to keep in step with the first.
    ///
    /// A row with one is reached through the name rather than called, which is
    /// what [`crate::table::doc`] prints and what keeps `handle.text` out of the
    /// generated function stubs.
    Bound,
}

/// What an *optional* position is called.
///
/// Whether it is reached by that name or positionally is not a judgement — it
/// follows from `req: false` and the position's place in the snapshot, via
/// [`Instruction::tail_optional`]. The name is written either way, because it
/// is what the stub and the error message call the position, and it has to be
/// written by hand because `-CMDHELP` calls them `showmode` and
/// `hex_string_like_12848412AB` (§15.23).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Field {
    pub name: &'static str,
    /// The token the emitter writes when the field is `true`, for a position
    /// whose only legal value is one NSIS spells itself: `execShell`'s
    /// `invokeIdList` becomes `/INVOKEIDLIST`. A ninth instance of *emitted,
    /// never written*.
    ///
    /// A toggle is also the one optional that never needs a [`Param::fill`]:
    /// NSIS tells it from a following argument by its leading `/` rather than
    /// by counting, so leaving it out shifts nothing.
    pub toggle: Option<&'static str>,
}

/// What one block field holds, which is the whole of what its lowering needs to
/// know. A field is one row and no code: [`crate::lower`] switches on this
/// rather than on the field's name, so a new setting is an overlay line.
///
/// The closed sets are *not* here. `-CMDHELP` prints `DirVerify auto|leave` and
/// the snapshot records those two words, so [`Setting::Enum`] names the shape and
/// the generated half names the members — which is the join doing the job it
/// exists for (§15.23).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
    /// `Name "${APP}"`: one string. `path` applies §5's `/`-to-`\`, which is
    /// right for a file the build machine reads and wrong for a caption.
    Str { path: bool },
    /// A Lua `bool`, emitted as the pair of words NSIS spells it with
    /// (§15.16). `CRCCheck` also accepts `force`, and offering it would take a
    /// third state this field does not have.
    Bool { on: &'static str, off: &'static str },
    /// One of the bare keywords the snapshot lists. NSIS *ignores* a keyword it
    /// does not know rather than objecting, so the closed set is checked here
    /// or nowhere (§13).
    Enum,
    /// A whole number, emitted bare.
    Int,
    /// `InstallDirRegKey HKLM "Software\App" "Path"`: one line built out of a
    /// Lua table, one key per position, emitted in the order the parts are
    /// written here.
    ///
    /// A *table* and not a list because the keys are the only thing that tells
    /// three strings apart, and because a language whose tables have no order
    /// (§12) cannot be asked to supply one by counting.
    Table(&'static [Part]),
    /// `PEAddResource f t n` written once per resource: the field holds a Lua
    /// **array**, and the whole line is emitted once per element, in the order
    /// the elements were written — the one order a Lua table does have (§12).
    ///
    /// Not the repetition a *position* has. `Rep::Many` puts many values on one
    /// line and is the snapshot's to say; that a **line** repeats is said only
    /// in NSIS's prose, so it is a variant here rather than a bit read off the
    /// skeleton.
    Each(&'static Setting),
    /// Shaped by the block's own lowering instead: `unicode` sets a field of
    /// the module rather than emitting a line, and `versionInfo` is a nested
    /// table. The analogue of [`Offer::Handled`] one level up, and it carries
    /// the Lua type for the same reason — the stub generator has to describe a
    /// field it does not lower.
    Handled(&'static str),
}

/// One position of a [`Setting::Table`], and the Lua key it takes.
///
/// The key is the only thing written by hand, for the reason [`overlay::opt`]
/// names an optional position: `-CMDHELP` calls these `root_key` and `addbits`,
/// which are NSIS's names for them and not this language's. Everything else —
/// whether the position is required, which keywords it accepts — is the
/// snapshot's, because one `Part` stands against one [`Param`] in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Part {
    pub field: &'static str,
    pub holds: Setting,
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
    /// A field of one of the four blocks (§15.10, §15.26), and what it holds.
    Attribute(Setting),
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
            Class::Attribute(_) => "attribute",
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
    /// What the emitter writes here when the caller omitted this position **and
    /// a later one still has to be emitted**. See [`overlay::Ann::fill`].
    pub fill: Option<&'static str>,
    /// The name this position takes in the trailing options table, when it is
    /// optional. A required position is positional and has none.
    pub field: Option<Field>,
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

    /// Whether a value outside [`Self::members`] is still legal (§15.23).
    pub fn open(&self) -> bool {
        self.shape.open
    }

    pub fn repeats(&self) -> bool {
        self.shape.rep == Rep::Many
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
    /// Every flag `-CMDHELP` prints for this command, in its order, each with
    /// what the surface does with it.
    pub options: Vec<Flag>,
    /// Mutually exclusive option sets: `File`'s `/oname=` branch against its
    /// repeated-filespec branch. The error names both spellings (§15.23).
    pub conflicts: &'static [&'static [&'static str]],
    pub note: Note,
    /// A `bool`-valued call rather than a statement (§15.20). See
    /// [`overlay::Row::predicate`].
    pub predicate: bool,
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

    /// The positions a *user* writes: the inputs, minus the ones the compiler
    /// fills and the ones that are not really positions. A `Kind::Label` is an
    /// argument to NSIS and not to Installua, so `fileExists(p)` takes one
    /// argument where `IfFileExists` takes three (§15.20); a `Kind::Fused` is
    /// not an argument to either, and a `Kind::Bound` is the compiler's too —
    /// the index behind `handle.text` comes from the handle.
    pub fn surface(&self) -> impl Iterator<Item = &Param> {
        self.inputs().filter(|param| {
            param.kind != Kind::Label && param.kind != Kind::Fused && param.kind != Kind::Bound
        })
    }

    /// Reached through a name the compiler resolves rather than called: a field
    /// of a section handle, or `currentInstType` (§13). One [`Kind::Bound`]
    /// position is what says so, and it is why these rows have an Installua
    /// spelling that is not a function name.
    pub fn bound(&self) -> bool {
        self.params.iter().any(|param| param.kind == Kind::Bound)
    }

    /// The positions a caller writes as *arguments*, including the trailing
    /// optional when the row has exactly one.
    ///
    /// The line is ambiguity rather than optionality. `Abort [message]` has one
    /// optional position and it is last, so an extra argument can only mean
    /// that position and `abort("stopped")` decides nothing by counting.
    /// `CreateShortcut`'s six optionals do: whether the fourth argument is the
    /// icon file or the icon index depends on whether the third was given. So
    /// those are named instead (§15.23), and `ExecShell`'s *leading* optional
    /// has no positional spelling at all.
    pub fn positional(&self) -> impl Iterator<Item = &Param> {
        let tail = self.tail_optional().is_some();
        self.surface().filter(move |param| param.required() || tail)
    }

    /// The single trailing optional, when that is the row's whole optional
    /// half. `None` when there is nothing optional, when there is more than
    /// one, or when an optional is followed by a required position — all three
    /// being the cases a count cannot resolve.
    pub fn tail_optional(&self) -> Option<&Param> {
        let surface: Vec<&Param> = self.surface().collect();
        let (last, rest) = surface.split_last()?;
        if last.required() || rest.iter().any(|param| !param.required()) {
            return None;
        }
        Some(last)
    }

    /// The named half: one entry per optional surface position, in emit order,
    /// and empty for a row whose optional is unambiguously positional.
    pub fn fields(&self) -> impl Iterator<Item = (Field, &Param)> {
        let named = self.tail_optional().is_none();
        self.surface()
            .filter(move |param| named && !param.required())
            .filter_map(|param| param.field.map(|field| (field, param)))
    }

    /// The field of that name, if the row has one.
    pub fn field(&self, name: &str) -> Option<(Field, &Param)> {
        self.fields().find(|(field, _)| field.name == name)
    }

    /// The flags a caller can name, in snapshot order.
    ///
    /// The other half of the options table, and the half that is not a
    /// position at all: `Delete [/REBOOTOK] filespec` has one argument and one
    /// flag, and `delete(p, { rebootOk = true })` writes both without the
    /// caller ever learning that the flag goes first (§15.23).
    pub fn flags(&self) -> impl Iterator<Item = (&'static str, &Flag)> {
        self.options
            .iter()
            .filter_map(|flag| flag.name().map(|name| (name, flag)))
    }

    /// The flag of that name, if the row has one.
    pub fn flag(&self, name: &str) -> Option<&Flag> {
        self.flags().find(|(each, _)| *each == name).map(|(_, f)| f)
    }

    /// Whether a call may end in `{ … }`. Both halves of the options table are
    /// optional and either one alone is enough to make the table meaningful, so
    /// this is what the lowerer, the stub and the doc all ask.
    pub fn takes_options(&self) -> bool {
        self.fields().next().is_some() || self.flags().next().is_some()
    }

    /// Every name legal inside the options table, positions first and flags
    /// after, which is the order they are declared in.
    pub fn option_names(&self) -> Vec<&'static str> {
        let fields = self.fields().map(|(field, _)| field.name);
        fields.chain(self.flags().map(|(name, _)| name)).collect()
    }

    /// What the call evaluates to: the first output's type, because that is
    /// what an output *is* (§15.23). A predicate's branch supplies its own
    /// `bool` and it has no output register to read.
    pub fn returns(&self) -> Option<Ty> {
        if self.predicate {
            return Some(Ty::Bool);
        }
        self.outputs().next().map(|param| param.ty)
    }

    /// The legal *argument* count. A range only where a count still decides
    /// something: one trailing optional (`abort` takes none or one) and real
    /// repetition (`File a b c`, where `usize::MAX` is the honest upper bound).
    ///
    /// Everywhere else it is a point, because the optional positions are named
    /// and counting arguments no longer says which one a caller meant.
    pub fn arity(&self) -> std::ops::RangeInclusive<usize> {
        let positional: Vec<&Param> = self.positional().collect();
        let least = positional.iter().filter(|param| param.required()).count();
        let most = match positional.last() {
            Some(param) if param.shape.rep == Rep::Many => usize::MAX,
            _ => positional.len(),
        };
        least..=most
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
                        fill: annotation.and_then(|annotation| annotation.fill),
                        field: annotation.and_then(|annotation| annotation.field),
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
                options: skeleton
                    .options
                    .iter()
                    .enumerate()
                    .map(|(index, opt)| Flag {
                        opt: *opt,
                        // Same rule as the parameters above: no row, or a row
                        // that says nothing about this flag, leaves the flag in
                        // the table and out of the surface. The census is where
                        // an `Exposed` row with an unjudged flag is a failure.
                        offer: row
                            .and_then(|row| row.options.get(index))
                            .copied()
                            .unwrap_or(Offer::Unoffered("no overlay judgement")),
                    })
                    .collect(),
                conflicts: row.map(|row| row.conflicts).unwrap_or(&[]),
                note: skeleton.note,
                predicate: row.is_some_and(|row| row.predicate),
            }
        })
        .collect()
}
