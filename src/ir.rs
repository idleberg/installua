//! The NSIS-side IR. Everything here is already in NSIS spelling; the emitter
//! decides layout and quoting and nothing else.
//!
//! `Module`'s field order **is** the emission order, as the five programs
//! forced it, so the one thing a reader has to keep straight is written down
//! once, in the type, rather than in the emitter's control flow:
//!
//!   0. `raw.head` text, above everything the compiler writes — the one slot
//!      whose contents the compiler has not read. It is above `Unicode` because
//!      that is the only place `!system` and `!tempfile` can run *before* the
//!      value they produce is needed, and it is safe there because nothing at
//!      this anchor may depend on compiler-generated state — see `Lowerer::anchored`
//!      in [`crate::lower`]
//!   1. `Unicode` — first of the compiler's own, so a later `raw` overrides it
//!      rather than being silently overridden
//!   2. `!define`s, in source order — the preprocessor is textual and strictly
//!      sequential, unlike everything below it
//!   3. `!include`s
//!   4. header init lines (`${Using:StrFunc}`)
//!   5. attributes, in overlay order
//!   6. `Var`s — before the pages and not merely before the bodies, because
//!      `page.directory { variable = … }` puts one in a `DirVar` and NSIS
//!      refuses a variable it has not seen declared
//!   7. MUI defines, then the pages, then the uninstaller's, then
//!      `MUI_LANGUAGE` — because MUI2 reads a `!define` at the point the next
//!      page macro is inserted and puts the language last. A *page-scoped*
//!      define is not in the first list at all: it belongs to the [`Page`] it
//!      configures, which is what keeps two Directory pages with different text
//!      expressible
//!   8. `InstType` lines, in declaration order — that order is their identity,
//!      because a `SectionIn` names one by position
//!   9. sections, in source order — a sequence, never reordered
//!  10. functions, last — because `Section "Core" SEC_core` is what `!define`s
//!      `${SEC_core}`, and the preprocessor is textual, so an `.onInit` that
//!      names a section has to come after the section. NSIS hoists calls, so
//!      nothing else about where the functions sit matters
//!  11. `raw.tail` text, below everything — the registrations
//!      (`!packhdr`, `!finalize`, `!uninstfinalize`) whose position does not
//!      matter, given a slot where it visibly does not
//!
//! Phase 1 filled 1, 5 and 9; Phase 2 adds 6 and 10. The rest exist empty,
//! because a widening is a smaller change than a reordering.

use crate::cfg;
use crate::diag::Span;
use crate::regs::Slot;

/// A whole `.nsi` file.
#[derive(Clone, Debug, Default)]
pub struct Module {
    /// `raw.head` text, above every line the compiler writes — including
    /// [`Module::unicode`], which is the whole point of having it: `!system` and
    /// `!tempfile` produce a value the rest of the script reads, so they have to
    /// run before the rest of the script exists.
    ///
    /// Unread by design, like every `raw`. What keeps that from selling the
    /// spine's guarantee back to the user is a rule about *content* rather than
    /// position: an anchor carries text whose meaning is position-independent,
    /// and text whose meaning depends on compiler-generated state is a
    /// declaration the compiler places — [`Module::plugin_dirs`] being the first
    /// concrete member of the second class.
    pub head: Vec<Instruction>,
    /// Always emitted, first of the compiler's own lines, defaults true.
    pub unicode: bool,
    /// `!addplugindir` lines, one per directory a called plugin was declared
    /// in. Directly under [`Module::unicode`] and above everything else,
    /// because an untagged `!addplugindir` binds to whichever target is current
    /// *when the directive is processed* — a line above `Unicode` binds to the
    /// default target and silently breaks every `unicode = false` build, and a
    /// line below a call site is too late for the lookup that call site does.
    /// See [`crate::lower::addplugindir`].
    pub plugin_dirs: Vec<Instruction>,
    pub defines: Vec<Define>,
    pub includes: Vec<String>,
    pub inits: Vec<Instruction>,
    pub attributes: Vec<Instruction>,
    /// Globals, declared by assignment, collected during lowering and emitted
    /// before anything that names one — which is the pages as well as the
    /// bodies, since `DirVar` takes a variable rather than a value.
    pub vars: Vec<String>,
    /// `!define MUI_ICON` and friends: the settings MUI2 reads **once**, on the
    /// first page of a type, behind an `!ifndef` interface guard. Separate from
    /// [`Module::defines`] because these are the compiler's, not the user's.
    ///
    /// The page-scoped ones — the settings MUI2 `!undef`s after the macro — are
    /// not here; they are [`Page::defines`], because a define read *at* the
    /// insertion point cannot live in a list that precedes every insertion
    /// point.
    pub mui_defines: Vec<Define>,
    /// The pages, in the order the user listed them: page order is
    /// user-visible, so unlike attributes these are never reordered.
    pub pages: Vec<Page>,
    /// The uninstaller's. A separate list because MUI2 requires every installer
    /// page before every uninstaller page, whatever order the two blocks were
    /// written in.
    pub unpages: Vec<Page>,
    /// `!insertmacro MUI_LANGUAGE` and the `LangString` lines under it, which
    /// have to come after every page — the macro `!warning`s otherwise, and a
    /// language string can only be filed against a language already loaded. One
    /// list rather than two because they are one run of output, in the one
    /// position both are legal in.
    pub languages: Vec<Instruction>,
    /// `LicenseLangString` lines, one per locale per license page whose `file`
    /// was written per locale. Under the languages for the same reason the
    /// `LangString`s are — the `${LANG_…}` a line names is defined by the
    /// `MUI_LANGUAGE` above it — but a list of their own, because they are
    /// gathered while the *pages* are lowered and the languages pass has long
    /// since finished by then.
    pub license_data: Vec<Instruction>,
    /// `ReserveFile /plugin X.dll`, one per plugin `.onInit` can reach — the
    /// compiler's half of `ReserveFile`, which no surface spelling reaches.
    /// After the languages because that is where MUI2 puts its own
    /// (`MUI_RESERVEFILE_LANGDLL`) and the head of the data block is what both
    /// are competing for; a `reserveFile(…)` the user wrote is an ordinary
    /// statement in a body and lands further down, which is the right order —
    /// nothing else can be needed earlier than `.onInit` needs these.
    pub reserved: Vec<Instruction>,
    /// `InstType` lines, in the order they were written — which is the whole of
    /// what an install type *is* to NSIS, since a section names one by its
    /// one-based position and by nothing else. The uninstaller's are the same
    /// list under an `un.` prefix, and NSIS numbers the two separately.
    pub inst_types: Vec<String>,
    pub uninst_types: Vec<String>,
    pub sections: Vec<SectionItem>,
    /// The components page's hover text, one block per half. After the sections
    /// because a `MUI_DESCRIPTION_TEXT` reads the `!define` a `Section` line
    /// makes, and the preprocessor is textual — the same rule that puts the
    /// functions last.
    pub descriptions: Vec<Descriptions>,
    pub functions: Vec<Function>,
    /// `raw.tail` text, below everything. The corpus's `tail` traffic —
    /// `!packhdr`, `!finalize`, `!uninstfinalize` — is registrations, which
    /// `makensis` acts on when the build ends rather than where the line sits.
    /// They get the slot where that is most obviously true, which also keeps
    /// them out of the one place position matters.
    pub tail: Vec<Instruction>,
}

impl Module {
    pub fn new() -> Self {
        Module {
            unicode: true,
            ..Module::default()
        }
    }

    /// Every body in the module, for a caller that wants to assert an IR
    /// property rather than read emitted text (tier 0).
    pub fn bodies(&self) -> impl Iterator<Item = (&str, &cfg::Body)> {
        let functions = self
            .functions
            .iter()
            .map(|f| (f.name.as_str(), &f.body))
            .collect::<Vec<_>>();
        let mut all = functions;
        for item in &self.sections {
            match item {
                SectionItem::Section(section) => all.push((section.name.as_str(), &section.body)),
                SectionItem::Group(group) => {
                    for section in &group.sections {
                        all.push((section.name.as_str(), &section.body));
                    }
                }
            }
        }
        all.into_iter()
    }

    /// The same, for the passes that rewrite a body in place: register
    /// allocation and caller-save insertion.
    pub fn bodies_mut(&mut self) -> Vec<(String, &mut cfg::Body)> {
        let mut all: Vec<(String, &mut cfg::Body)> = self
            .functions
            .iter_mut()
            .map(|f| (f.name.clone(), &mut f.body))
            .collect();
        for item in &mut self.sections {
            match item {
                SectionItem::Section(section) => {
                    all.push((section.name.clone(), &mut section.body))
                }
                SectionItem::Group(group) => {
                    for section in &mut group.sections {
                        all.push((section.name.clone(), &mut section.body));
                    }
                }
            }
        }
        all
    }
}

#[derive(Clone, Debug)]
pub struct Define {
    pub name: String,
    /// `None` for a define whose *existence* is the whole message:
    /// `!define MUI_DIRECTORYPAGE_VERIFYONLEAVE` is read by an `!ifdef` and
    /// never expanded, so giving it a value would be inventing one.
    pub value: Option<Arg>,
}

/// One MUI2 page: the `!insertmacro MUI_PAGE_*` line, the `!define`s that
/// configure it, and the `!undef`s that stop them leaking into the next one.
///
/// A page owns its defines rather than the module owning all of them, because
/// MUI2 reads a page-scoped setting **at** the insertion point and `!undef`s it
/// straight after. Two Directory pages with different text is the case that
/// decides it: under one shared list the second define would sit after the
/// first insertion and the first page would ship with the wrong words.
///
/// [`Self::undefines`] exists because MUI2's own cleanup has two holes in it,
/// not because ours is an alternative to it. `UninstallConfirm.nsh` clears its
/// two text settings and not `MUI_UNCONFIRMPAGE_VARIABLE`; `License.nsh` clears
/// `MUI_LICENSEPAGE_CHECKBOX_TEXT_ACCEPT`, which is a name nothing defines —
/// the radio button texts are spelled `…_RADIOBUTTONS_TEXT_ACCEPT`. So this
/// list is the difference between what a page sets and what MUI2 clears,
/// computed once in the lowering.
#[derive(Clone, Debug)]
pub struct Page {
    pub defines: Vec<Define>,
    pub insert: Instruction,
    pub undefines: Vec<String>,
}

/// One half's `.onMouseOverSection`, which MUI2 writes from three macros and
/// this compiler never lets a script spell.
///
/// A block rather than a per-section define, because that is the shape MUI2
/// gives it: `MUI_FUNCTION_DESCRIPTION_BEGIN` opens a `Function` and an
/// `${if}`, each text is an `${elseif}` on a section's index, and `…_END`
/// closes both. Nothing in it is optional and nothing in it is ordered by the
/// user, so the compiler owns all three lines.
#[derive(Clone, Debug)]
pub struct Descriptions {
    /// The uninstaller's, which is the `UN` in `MUI_UNFUNCTION_DESCRIPTION_*`
    /// and the `un.` on the function those macros write.
    pub un: bool,
    /// `(index define, text)`, in section order. Empty is legal and reachable:
    /// an `onMouseOverSection` with no described section still needs the block,
    /// because MUI2 calls the hook from inside it and from nowhere else.
    pub texts: Vec<(String, Arg)>,
}

#[derive(Clone, Debug)]
pub struct Function {
    pub name: String,
    pub body: cfg::Body,
}

#[derive(Clone, Debug)]
pub enum SectionItem {
    Section(Section),
    Group(SectionGroup),
}

#[derive(Clone, Debug)]
pub struct SectionGroup {
    pub name: String,
    /// `SectionGroup /e` — expanded in the components tree.
    pub expanded: bool,
    /// See [`Section::index_name`]; a group is addressable for the same reason
    /// and by the same third word.
    pub index_name: Option<String>,
    pub sections: Vec<Section>,
}

#[derive(Clone, Debug)]
pub struct Section {
    pub name: String,
    /// `Section /o` — unselected by default.
    pub optional: bool,
    /// The one-based positions of [`Module::inst_types`] this section belongs
    /// to, plus `RO` when [`Self::required`]: together they are the `SectionIn`
    /// line, and an empty list with no `RO` writes none at all.
    ///
    /// Positions rather than names because that is what NSIS reads, and the
    /// translation happens once, in the lowering, where the declaration list is
    /// in scope. Nothing downstream ever sees the name.
    pub inst_types: Vec<usize>,
    /// `SectionIn RO` — always installed, and greyed out in the components
    /// tree. Not the opposite of [`Self::optional`], which only says what the
    /// box starts as.
    pub required: bool,
    /// `AddSize` — extra kilobytes to charge this section beyond the files it
    /// installs, for the space estimate.
    pub size: Option<u32>,
    /// The third word of the `Section` line: a name NSIS `!define`s to this
    /// section's index, which is the only way a running program can name it.
    /// The index itself is NSIS's — it counts sections in emission order — so
    /// this is the install-type binding again, a compile-time name for a number
    /// the compiler does not own.
    ///
    /// `Some` exactly when the section was bound to a Lua local and listed by
    /// that name, which is the one way a program says it wants to address one.
    /// `None` for a section written inline in its block: no third word, and an
    /// unaddressed program pays nothing for the feature — the `!define` NSIS
    /// would make is one more name in a namespace shared with the author's.
    pub index_name: Option<String>,
    pub body: cfg::Body,
}

/// A laid-out body: what [`crate::layout`] produces from a CFG, and the only
/// thing the emitter ever sees. Labels are IR nodes rather than strings spliced
/// by the lowering code, and by the time one exists the layout pass has already
/// decided which blocks need one at all.
#[derive(Clone, Debug)]
pub enum Item {
    Instruction(Instruction),
    Label(String),
}

/// One entry in a basic block.
///
/// A call is not an instruction, because the instructions it becomes are not
/// all known when it is lowered: the caller-saves around it are `live ∩
/// clobbered`, and neither half exists until registers have been coloured and
/// the interprocedural fixpoint has run. So lowering records the two
/// *places* — where the saves go and where the call goes — and
/// [`crate::alloc`] fills in the registers between them.
///
/// The two are separate steps rather than one because the saves sit **before**
/// the argument evaluation, not merely before the argument pushes: that is what
/// keeps the callee's results on top of the stack when it returns, so the
/// restores fall out underneath them without a single `Exch`.
#[derive(Clone, Debug)]
pub enum Step {
    Instruction(Instruction),
    /// Push the caller-saves for call site `n`.
    Saves(usize),
    /// Push the arguments, `Call`, pop the results, pop the saves.
    Call(usize),
}

impl Step {
    pub fn instruction(name: impl Into<String>, args: Vec<Arg>) -> Step {
        Step::Instruction(Instruction::new(name, args))
    }
}

/// What a call site *is*, which decides both what it expands to and what it
/// clobbers.
#[derive(Clone, Debug)]
pub enum CallKind {
    /// `Call name`: the stack ABI, and a clobber set the fixpoint computed.
    Function,
    /// A plugin, `System::Call` or `raw` — the three opaque callees.
    ///
    /// Opaque means two things at once. It takes its arguments **inline**
    /// rather than on the stack, so the lines are already written by the time
    /// this exists; and it clobbers **every** register, because nothing here
    /// has read the plugin's DLL, `.r0` inside a `System::Call` signature
    /// writes a register no AST records, and a `raw` block is text.
    Opaque {
        lines: Vec<Instruction>,
        /// Whether the lines are a `raw` block. A plugin call is still the
        /// compiler's line — it built the argument — and a `raw` block is the
        /// user's text, which the line map reports differently because only one
        /// of the two was ever checked.
        raw: bool,
    },
}

/// A call site, in the order the stack sees it.
///
/// The convention is program 4's, and it is written here rather than discovered
/// from whichever code path was implemented first:
///
///   1. arguments are pushed in **reverse source order**, so the callee's first
///      `Pop` is its first parameter;
///   2. returns are pushed in **reverse source order**, so the caller's first
///      `Pop` is the first return value;
///   3. caller-saves are pushed **before** the arguments, ascending register
///      number, and restored in reverse.
#[derive(Clone, Debug)]
pub struct CallSite {
    /// The NSIS `Function` name. Empty for an opaque callee, which has no
    /// node in the call graph to be.
    pub callee: String,
    pub kind: CallKind,
    /// Argument values, in source order.
    pub args: Vec<Arg>,
    /// Where the returned values land, in source order. A dropped return still
    /// gets one: the callee pushed it either way, so it has to come off.
    pub results: Vec<Slot>,
    /// Set when the callee's arity depends on its first pushed value, and the
    /// trailing [`Tail::defaults`]`.len()` of [`results`](Self::results) come
    /// off only on some paths.
    pub tail: Option<Tail>,
    /// `live ∩ clobbered`, ascending. Empty until [`crate::alloc`] fills it,
    /// and empty afterwards too whenever the intersection really is empty —
    /// which is the common case, and the whole argument for caller-saves.
    pub saves: Vec<Slot>,
    pub span: Span,
}

/// A call site's **conditional** returns: the ones that are on the stack only
/// when the first popped value says so. See [`crate::declarations::PluginMethod::tagged`]
/// for why a plugin has these at all.
///
/// The slots themselves stay in [`CallSite::results`] rather than moving here,
/// and that is the whole trick. A conditional `Pop` reads like a branch, and a
/// branch in the middle of a call site would mean a CFG diamond — which would
/// have to exist before [`crate::alloc`] runs, would split the call's own
/// liveness, and would put control flow somewhere the lowerer has no block
/// boundary. Instead the tail is **pre-defaulted**: `layout` writes each tail
/// slot before testing the tag, so every slot is defined on every path, the
/// allocator sees a call that defines all its results exactly as before, and
/// the conditional part is a straight run of lines inside one block.
///
/// So this is a layout-local fact, and it is on the call site only because
/// layout is handed a [`CallSite`] and nothing else.
#[derive(Clone, Debug)]
pub struct Tail {
    /// First-popped values that mean the tail is there. Compared with
    /// `StrCmpS`, so the match is exact — a payload that differs from a tag
    /// only in case is a different value, and guessing otherwise would pop a
    /// caller-save.
    pub tags: Vec<String>,
    /// What each tail slot holds when the tag did *not* match, one per
    /// conditional trailing result. Written unconditionally, ahead of the test.
    pub defaults: Vec<String>,
}

/// `name` is a `String` and not `&'static str` because macro invocations
/// (`${WinVerGetMajor}`) and plugin calls (`System::Call`) are composed rather
/// than looked up whole.
#[derive(Clone, Debug)]
pub struct Instruction {
    pub name: String,
    pub args: Vec<Arg>,
    /// Whether the inputs are still live while the outputs are written.
    ///
    /// False for an NSIS instruction, whose destination is a whole write the
    /// allocator may put on top of a dying operand. True for a **macro**, which
    /// expands to a sequence nobody here has read: `${GetSize} $0 "" $0 $1 $2`
    /// happens to work and `${Foo} $0 $0` for the next macro may not, and the
    /// difference is invisible at this level. So a macro's inputs interfere
    /// with its outputs and the two never share a register.
    pub atomic: bool,
    /// The statement this line came from, for the line map. `None` for a line
    /// the compiler emitted on its own behalf, which is exactly the set whose
    /// failure is a compiler bug rather than a user's mistake.
    pub span: Option<Span>,
}

impl Instruction {
    pub fn new(name: impl Into<String>, args: Vec<Arg>) -> Self {
        Instruction {
            name: name.into(),
            args,
            atomic: false,
            span: None,
        }
    }

    /// The same instruction, attributed to the statement that produced it.
    pub fn at(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }

    /// The same instruction, with its inputs held live across its outputs.
    pub fn atomic(mut self) -> Self {
        self.atomic = true;
        self
    }

    /// The values this instruction reads.
    pub fn uses(&self) -> Vec<Slot> {
        let mut out = Vec::new();
        for arg in &self.args {
            arg.uses(&mut out);
        }
        out
    }

    /// The values it writes. A destination is an [`Arg::Dest`] wherever it
    /// appears in the argument list, so an instruction whose output is not
    /// first — and the real instruction set has several — needs no special case
    /// here.
    pub fn defs(&self) -> Vec<Slot> {
        self.args
            .iter()
            .filter_map(|arg| match arg {
                Arg::Dest(slot) => Some(slot.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn recolour(&mut self, colour: &dyn Fn(u32) -> u8) {
        for arg in &mut self.args {
            arg.recolour(colour);
        }
    }
}

fn recolour_slot(slot: &mut Slot, colour: &dyn Fn(u32) -> u8) {
    if let Slot::Virtual(index) = *slot {
        *slot = Slot::Reg(colour(index));
    }
}

/// One span of an argument's text.
///
/// The split exists because a `$` reaching the output means two opposite
/// things: in [`Piece::Text`] it is five dollars and gets doubled, and in
/// [`Piece::Var`] it is a register and must not be touched. Concatenation of
/// constants is therefore free — `"into ".. INSTDIR` is one quoted argument and
/// no instruction at all — which is the whole reason the string model is worth
/// having.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Piece {
    /// Literal text. Escaped on the way out.
    Text(String),
    /// An NSIS variable or define the *compiler* does not own: `$INSTDIR`,
    /// `${VERSION}`. Spliced in bare.
    Var(String),
    /// A read of an allocated value. Distinct from [`Piece::Var`] because this
    /// is the one the register allocator rewrites and liveness counts as a use
    /// — a `$INSTDIR` spliced into a template is neither.
    Slot(Slot),
    /// A top-level `<const>`, which is a `!define`. It carries its folded value
    /// as well as its name because the two are needed in different positions:
    /// `${APP}` is what a reader wants to see, and a **path** cannot contain
    /// one — `/` is normalised to `\` in text and there is no way to normalise
    /// inside a macro expansion — so [`Arg::into_path`] substitutes the value
    /// back at the parameter boundary.
    Const { name: String, value: String },
}

/// Whether the emitter quotes and escapes an argument — the data-versus-syntax
/// split. Registers *as destinations*, `IntOp` opcodes, flag lists and branch
/// targets are NSIS **syntax**; a section name and a `DetailPrint` message are
/// **data**, and only data is escaped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Arg {
    Data {
        pieces: Vec<Piece>,
        /// A path position: `/` in the *text* pieces is normalised to `\`,
        /// since NSIS does not accept a forward slash everywhere. A register's
        /// contents are runtime data and are left alone.
        path: bool,
    },
    /// Syntax. Emitted exactly as written, unquoted.
    Raw(String),
    /// A destination register: syntax, and — the reason it is a variant rather
    /// than a `Raw` holding `"$0"` — a **definition**, which is half of what
    /// liveness reads.
    Dest(Slot),
}

impl Arg {
    pub fn str(value: impl Into<String>) -> Self {
        Arg::data(vec![Piece::Text(value.into())])
    }

    pub fn path(value: impl Into<String>) -> Self {
        Arg::Data {
            pieces: vec![Piece::Text(value.into())],
            path: true,
        }
    }

    pub fn int(value: i64) -> Self {
        Arg::str(value.to_string())
    }

    /// A constant read the compiler does not own: `$INSTDIR`, `${VERSION}`.
    pub fn var(nsis: impl Into<String>) -> Self {
        Arg::data(vec![Piece::Var(nsis.into())])
    }

    /// A read of an allocated value.
    pub fn slot(slot: Slot) -> Self {
        Arg::data(vec![Piece::Slot(slot)])
    }

    /// A write to an allocated value.
    pub fn dest(slot: Slot) -> Self {
        Arg::Dest(slot)
    }

    pub fn raw(value: impl Into<String>) -> Self {
        Arg::Raw(value.into())
    }

    pub fn data(pieces: Vec<Piece>) -> Self {
        Arg::Data {
            pieces,
            path: false,
        }
    }

    pub fn data_path(pieces: Vec<Piece>) -> Self {
        Arg::Data { pieces, path: true }
    }

    /// A `<const>` reference: `${APP}`, with the folded value alongside.
    pub fn constant(name: impl Into<String>, value: impl Into<String>) -> Self {
        Arg::data(vec![Piece::Const {
            name: name.into(),
            value: value.into(),
        }])
    }

    /// The same argument in a path position. Called at the parameter boundary,
    /// so the overlay decides pathness and the expression lowerer does not have
    /// to know where its result is going.
    ///
    /// A `${APP}` becomes its value here **when the value contains a `/`**, and
    /// that is not a peephole: `/` is normalised to `\` in text pieces, and a
    /// macro expansion is opaque to that, so a define holding a separator would
    /// otherwise ship the one NSIS does not accept. A value with no `/` in it
    /// normalises to itself, so the reference survives and the output keeps
    /// saying `${APP}`.
    pub fn into_path(self) -> Self {
        match self {
            Arg::Data { pieces, .. } => Arg::Data {
                pieces: pieces
                    .into_iter()
                    .map(|piece| match piece {
                        Piece::Const { value, .. } if value.contains('/') => Piece::Text(value),
                        other => other,
                    })
                    .collect(),
                path: true,
            },
            raw => raw,
        }
    }

    /// Every value this argument *reads*. A [`Arg::Dest`] reads nothing: NSIS
    /// destinations are whole writes, so `StrCpy $0 "x"` kills `$0` rather than
    /// updating it, and treating it otherwise would keep dead values alive
    /// across half the program.
    pub fn uses(&self, out: &mut Vec<Slot>) {
        if let Arg::Data { pieces, .. } = self {
            for piece in pieces {
                if let Piece::Slot(slot) = piece {
                    out.push(slot.clone());
                }
            }
        }
    }

    /// Applies the allocator's colouring, in place.
    pub fn recolour(&mut self, colour: &dyn Fn(u32) -> u8) {
        match self {
            Arg::Dest(slot) => recolour_slot(slot, colour),
            Arg::Data { pieces, .. } => {
                for piece in pieces {
                    if let Piece::Slot(slot) = piece {
                        recolour_slot(slot, colour);
                    }
                }
            }
            Arg::Raw(_) => {}
        }
    }

    /// The whole argument as literal text, when nothing in it is a register. A
    /// `<const>` contributes its folded value, because that is what the
    /// preprocessor will substitute — which is how `IntOp $0 $1 / ${BLOCK}`
    /// gets to be an unquoted integer argument like any other.
    pub fn as_text(&self) -> Option<String> {
        match self {
            Arg::Data { pieces, .. } => pieces
                .iter()
                .map(|piece| match piece {
                    Piece::Text(text) => Some(text.as_str()),
                    Piece::Const { value, .. } => Some(value.as_str()),
                    Piece::Var(_) | Piece::Slot(_) => None,
                })
                .collect::<Option<Vec<_>>>()
                .map(|parts| parts.concat()),
            Arg::Raw(_) | Arg::Dest(_) => None,
        }
    }

    /// Concatenation, flattened and with adjacent text runs merged, so
    /// `"a" .. "b" .. INSTDIR` is two pieces rather than three.
    pub fn concat(self, other: Arg) -> Arg {
        let (mut pieces, path) = match self {
            Arg::Data { pieces, path } => (pieces, path),
            Arg::Raw(text) => (vec![Piece::Text(text)], false),
            Arg::Dest(slot) => (vec![Piece::Slot(slot)], false),
        };
        let (rhs, rhs_path) = match other {
            Arg::Data { pieces, path } => (pieces, path),
            Arg::Raw(text) => (vec![Piece::Text(text)], false),
            Arg::Dest(slot) => (vec![Piece::Slot(slot)], false),
        };
        for piece in rhs {
            match (pieces.last_mut(), piece) {
                (Some(Piece::Text(tail)), Piece::Text(text)) => tail.push_str(&text),
                (_, piece) => pieces.push(piece),
            }
        }
        Arg::Data {
            pieces,
            path: path || rhs_path,
        }
    }
}
