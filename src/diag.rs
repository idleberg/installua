//! Diagnostics as data: span, code, severity, notes.
//!
//! Collected, never thrown, and never routed through a global sink — the
//! collector is threaded as `&mut` so that a caller can compile two sources
//! concurrently in one process. Every entry point in this crate takes one;
//! nothing in this crate owns one.

use std::fmt;

/// A half-open source range, in both the units a human reads and the units a
/// tool slices with.
///
/// `full-moon` reports `line`/`character` 1-based, which is also what
/// `makensis` and every editor protocol want, so no conversion happens here.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Span {
    /// Which source this span is measured in — an index into [`Files`], where
    /// `0` is always the file the build was started from.
    ///
    /// It lives here rather than on [`Diagnostic`] because `include` merges
    /// every file's declarations into one tree before resolution: past the
    /// frontend there is no "current file" for a raiser to be stamped with, and
    /// most diagnostics are raised past the frontend.
    pub file: u32,
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub start_byte: usize,
    pub end_byte: usize,
}

impl Span {
    /// A span covering both ends, for a diagnostic that spans two nodes.
    ///
    /// Two spans from different files never legitimately meet — every joining
    /// caller joins two nodes of one expression — so the left one's file wins
    /// rather than the join carrying a third answer.
    pub fn join(self, other: Span) -> Span {
        let (start, end) = if self.start_byte <= other.start_byte {
            (self, other)
        } else {
            (other, self)
        };
        Span {
            file: self.file,
            start_line: start.start_line,
            start_column: start.start_column,
            end_line: end.end_line,
            end_column: end.end_column,
            start_byte: start.start_byte,
            end_byte: end.end_byte,
        }
    }

    /// The same span, attributed to `file`. What the frontend stamps once per
    /// source, since `full-moon` measures every file from its own byte zero.
    pub fn in_file(self, file: u32) -> Span {
        Span { file, ..self }
    }
}

/// The sources a set of spans is measured in: index `0` is the file the build
/// started from, and the rest are what `include` pulled in, in load order.
///
/// Ordinary owned data, threaded like everything else here — the table is not
/// a registry and there is no global one.
#[derive(Clone, Debug, Default)]
pub struct Files {
    names: Vec<String>,
}

impl Files {
    /// Adds a source and returns its index. The root is added first and gets
    /// `0`, which is also [`Span::default`]'s file, so a span nobody stamped
    /// still points at the file the user named.
    pub fn add(&mut self, name: impl Into<String>) -> u32 {
        self.names.push(name.into());
        (self.names.len() - 1) as u32
    }

    /// The index this source already has, if it has one. What the loader uses
    /// to include a file once however many times it is named.
    pub fn find(&self, name: &str) -> Option<u32> {
        self.names.iter().position(|n| n == name).map(|i| i as u32)
    }

    pub fn name(&self, file: u32) -> Option<&str> {
        self.names.get(file as usize).map(String::as_str)
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.start_line, self.start_column)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Error => f.write_str("error"),
            Severity::Warning => f.write_str("warning"),
        }
    }
}

/// Every diagnostic this compiler can raise.
///
/// The registry is the point: `Code::ALL` is walked by a test that asserts each
/// one is actually reachable. A code with no test that produces it is a code
/// nobody has checked the wording of.
///
/// Codes are slugs rather than numbers deliberately — a number has to be
/// allocated, never reused and kept in a table, and buys nothing over a name
/// that is greppable in both the source and a user's terminal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Code {
    // -- frontend: parse
    /// `full-moon` could not parse the source as Lua 5.4.
    ParseError,

    // -- frontend: literals
    /// A float literal. NSIS has no float arithmetic at all.
    FloatLiteral,
    /// An escape sequence that is invalid in Lua 5.4. `full-moon` accepts these
    /// and real Lua does not, so the frontend checks them itself.
    InvalidEscape,
    /// A `$` followed by an identifier character inside a literal — the
    /// canonical NSIS muscle-memory trap, since literals are data.
    DollarInLiteral,
    /// A literal at or over the vanilla `NSIS_MAX_STRLEN` floor, which
    /// `makensis` truncates at runtime with no diagnostic at all.
    OverlongLiteral,

    // -- frontend: operators
    /// `/` is float division, which does not exist here.
    FloatDivision,
    /// `^` is exponentiation in Lua and *xor* in NSIS; remapping it silently
    /// would be the exact class of bug this compiler exists to prevent.
    Exponentiation,
    /// `#` on a string counts bytes in Lua and UTF-16 code units in NSIS.
    LengthOperator,

    // -- frontend: statements and expressions
    /// `goto` or a `::label::`. The compiler owns labels.
    Goto,
    /// `nil`. There is no such value.
    NilValue,
    /// `...`. There is no vararg calling convention.
    Varargs,
    /// `repeat … until`. `while` is the loop that survives.
    RepeatLoop,
    /// `function f() … end` as a statement. Declarations go through `func`.
    FunctionStatement,
    /// A function expression somewhere other than a call argument — closures
    /// are declaration bodies, never values.
    ClosureValue,
    /// `local x <close>`: there is no runtime to close over.
    CloseAttribute,
    /// A `local` attribute that is neither `<const>` nor `<close>`.
    UnknownAttribute,
    /// `t[k]`. There are no runtime tables to index.
    IndexExpression,
    /// A `for … in` over something outside the iterator whitelist.
    UnsupportedIterator,
    /// `require`. Loading is compile-time, and spelled `import`/`include`.
    RuntimeRequire,

    // -- resolution and types
    /// A name that resolves to nothing. Resolution is order-free, so this
    /// really does mean *nowhere in the file*.
    UndefinedName,
    /// A non-`bool` in a condition, or as an operand of `and`/`or`/`not`.
    /// By-type truthiness is not merely inference-dependent, it disagrees with
    /// Lua on `0` and `""` — the two values a reader is most likely to test.
    NotBool,
    /// `local x = a or b` on strings: the default-value idiom. Under Lua's
    /// semantics it is dead code, and under the intended NSIS semantics the
    /// source lies, so there is no reading that works.
    OrAsValue,
    /// A comparison whose operands are different types, or whose types are not
    /// known. Defaulting to `StrCmp` is how a compiler becomes a text expander
    /// with a type system bolted on.
    TypeMismatch,
    /// A variable assigned two different types. A register is one slot, so this
    /// is NSIS-shaped rather than arbitrary, and it reports the disagreement
    /// rather than privileging whichever line came first.
    TypeConflict,
    /// A call with the wrong number of arguments, or a binding that wants more
    /// values than the callee returns.
    WrongArity,
    /// A `func` returning a different number of values on two paths. `Call` has
    /// no arity at all — the callee pushes and the caller pops — so a
    /// disagreement is a stack that unbalances at runtime with no diagnostic
    /// from NSIS.
    ReturnArity,

    // -- lowering
    /// `break` outside a loop.
    BreakOutsideLoop,
    /// `continue()` outside a loop. It is a call that jumps, so unlike `break`
    /// it parses anywhere.
    ContinueOutsideLoop,
    /// More values live at once than NSIS has registers. Unlike Phase 2's
    /// placeholder, this is the real limit: values whose live ranges do not
    /// overlap already share a register.
    RegisterExhaustion,
    /// A `messageBox` answer whose comparison is already decided — either
    /// because the dialog has one button, so every comparison under it is, or
    /// because the literal it is compared against is not one of the answers
    /// that dialog can give. A **warning** in both cases: the program is
    /// well-formed and NSIS builds it without a word, since by the time it sees
    /// the comparison both sides are ordinary strings.
    ConstantAnswer,
    /// Unbounded recursion. A **warning**, because it is legal and sometimes
    /// intended — and one worth having, since the failure was measured as a
    /// silent process death at roughly 1300 frames.
    DeepRecursion,
    /// Well-formed, whitelisted, and outside what this version emits. This is
    /// the honest edge of the vertical slice, not a parse failure.
    NotYetImplemented,
    /// A field name that no block accepts.
    UnknownField,
    /// A field whose value has the wrong shape for its name.
    BadFieldValue,
    /// A block that may appear once appearing twice.
    DuplicateBlock,
    /// A required attribute that no block supplied.
    MissingAttribute,
    /// A setting NSIS would accept and then ignore, because a sibling field
    /// decides whether it is read at all: `compressorDictSize` is LZMA's, and
    /// beside any other compressor it is a line that does nothing. `makensis`
    /// warns rather than refusing, which is a diagnostic naming the NSIS command
    /// and arriving only under `-WX`.
    IgnoredSetting,
    /// A `string.format` whose format string is not one `IntFmt` can perform.
    /// `IntFmt` is `wsprintf` with exactly one argument, and `wsprintf` knows
    /// `c d i s u x X` and nothing else — so `%o` is not an octal conversion
    /// but a literal `o`, and `%s` reads an integer as a pointer. `makensis`
    /// objects to neither, which is what makes this a code rather than a
    /// warning nobody would see.
    FormatString,

    // -- loading
    /// An `include` whose file is missing, unreadable, or has no directory to
    /// resolve against.
    IncludeNotFound,
    /// A file that includes itself, however long the way round. Naming the
    /// whole loop is the point: the file the loader noticed it at is rarely the
    /// one with the mistake in it.
    IncludeCycle,
    /// An `include` written somewhere it cannot mean anything: inside a body,
    /// or with an argument that is not a string literal. The path has to be
    /// readable without running anything, which is the staging rule and not a
    /// parser limitation.
    IncludeForm,

    // -- build parameters
    /// A `param(…)` written somewhere it cannot mean anything: inside a body,
    /// composed into a larger expression, or with a name that is not a string
    /// literal. A parameter is a *declaration* — it is what `-D` is checked
    /// against — so it has to be readable without folding anything first.
    ParamForm,
    /// A `-D` naming a parameter the program does not declare. An error rather
    /// than a shrug, because silently ignoring it is precisely the `!ifndef`
    /// failure mode parameters exist to retire: the build succeeds, the value
    /// is the default, and nothing says so.
    UnknownParam,
    /// A parameter declared without a default — `param("NAME")` — and no `-D`
    /// giving it one. The declaration is the script saying the build cannot be
    /// done without this value, which is `!ifndef NAME` / `!error` written as a
    /// declaration instead of as a guard, and checked the same way for every
    /// build rather than wherever the guard was pasted.
    MissingParam,

    // -- anchored `raw`
    /// A top-level `raw` whose anchor is missing, is not one of the anchors, or
    /// is written where an anchor cannot mean anything — and text at an anchor
    /// that could only be true inside a body.
    ///
    /// All four are one question, because an anchor is a *position outside every
    /// body*. Inside a body `raw` means "here" and needs no anchor; at the top
    /// level there is no "here", since the emitter's slots are fixed and
    /// statement order is not emission order. Defaulting the anchor is the one
    /// answer that is not available: text that lands in the wrong slot assembles
    /// clean and ships something else, which is the `!ifndef` silent miss again.
    RawAnchor,

    // -- build-time `if`
    /// A top-level `if` whose condition does not fold. Out there the branch is
    /// the compiler's to take — it happens before anything is bucketed, which is
    /// what keeps the emitter's fixed spine out of it — so a condition that is
    /// only known at install time has nothing to be decided by. The same rule a
    /// `<const>` lives under, one level up.
    ConstIf,

    /// An NSIS instruction that has a Lua spelling instead: `StrCmp` is `==`,
    /// `IntOp` is `+`, `StrCpy` is assignment. Not an unknown name — the
    /// compiler knows exactly what it is, and says what to write.
    NsisRetired,

    /// A call written somewhere NSIS accepts it and then ignores it. `SetSilent`
    /// outside `.onInit` assembles clean under `-WX` and does nothing at run
    /// time, so this is the only place the difference is ever visible. See
    /// [`crate::table::Place`].
    WrongPlace,
}

impl Code {
    /// The registry. Walked by a test that asserts each code is reachable.
    pub const ALL: &'static [Code] = &[
        Code::ParseError,
        Code::FloatLiteral,
        Code::InvalidEscape,
        Code::DollarInLiteral,
        Code::OverlongLiteral,
        Code::FloatDivision,
        Code::Exponentiation,
        Code::LengthOperator,
        Code::Goto,
        Code::NilValue,
        Code::Varargs,
        Code::RepeatLoop,
        Code::FunctionStatement,
        Code::ClosureValue,
        Code::CloseAttribute,
        Code::UnknownAttribute,
        Code::IndexExpression,
        Code::UnsupportedIterator,
        Code::RuntimeRequire,
        Code::UndefinedName,
        Code::NotBool,
        Code::OrAsValue,
        Code::TypeMismatch,
        Code::TypeConflict,
        Code::WrongArity,
        Code::ReturnArity,
        Code::BreakOutsideLoop,
        Code::ContinueOutsideLoop,
        Code::RegisterExhaustion,
        Code::DeepRecursion,
        Code::ConstantAnswer,
        Code::NotYetImplemented,
        Code::UnknownField,
        Code::BadFieldValue,
        Code::DuplicateBlock,
        Code::MissingAttribute,
        Code::IgnoredSetting,
        Code::FormatString,
        Code::IncludeNotFound,
        Code::IncludeCycle,
        Code::IncludeForm,
        Code::ParamForm,
        Code::UnknownParam,
        Code::MissingParam,
        Code::RawAnchor,
        Code::ConstIf,
        Code::NsisRetired,
        Code::WrongPlace,
    ];

    pub fn slug(self) -> &'static str {
        match self {
            Code::ParseError => "parse-error",
            Code::FloatLiteral => "float-literal",
            Code::InvalidEscape => "invalid-escape",
            Code::DollarInLiteral => "dollar-in-literal",
            Code::OverlongLiteral => "overlong-literal",
            Code::FloatDivision => "float-division",
            Code::Exponentiation => "exponentiation",
            Code::LengthOperator => "length-operator",
            Code::Goto => "goto",
            Code::NilValue => "nil-value",
            Code::Varargs => "varargs",
            Code::RepeatLoop => "repeat-loop",
            Code::FunctionStatement => "function-statement",
            Code::ClosureValue => "closure-value",
            Code::CloseAttribute => "close-attribute",
            Code::UnknownAttribute => "unknown-attribute",
            Code::IndexExpression => "index-expression",
            Code::UnsupportedIterator => "unsupported-iterator",
            Code::RuntimeRequire => "runtime-require",
            Code::UndefinedName => "undefined-name",
            Code::NotBool => "not-a-bool",
            Code::OrAsValue => "or-as-value",
            Code::TypeMismatch => "type-mismatch",
            Code::TypeConflict => "type-conflict",
            Code::WrongArity => "wrong-arity",
            Code::ReturnArity => "return-arity",
            Code::BreakOutsideLoop => "break-outside-loop",
            Code::ContinueOutsideLoop => "continue-outside-loop",
            Code::RegisterExhaustion => "register-exhaustion",
            Code::DeepRecursion => "deep-recursion",
            Code::ConstantAnswer => "constant-answer",
            Code::NotYetImplemented => "not-yet-implemented",
            Code::UnknownField => "unknown-field",
            Code::BadFieldValue => "bad-field-value",
            Code::DuplicateBlock => "duplicate-block",
            Code::MissingAttribute => "missing-attribute",
            Code::IgnoredSetting => "ignored-setting",
            Code::FormatString => "format-string",
            Code::IncludeNotFound => "include-not-found",
            Code::IncludeCycle => "include-cycle",
            Code::IncludeForm => "include-form",
            Code::ParamForm => "param-form",
            Code::UnknownParam => "unknown-param",
            Code::MissingParam => "missing-param",
            Code::RawAnchor => "raw-anchor",
            Code::ConstIf => "const-if",
            Code::NsisRetired => "nsis-retired",
            Code::WrongPlace => "wrong-place",
        }
    }
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.slug())
    }
}

/// One line under a diagnostic.
///
/// The optional span is what makes a note that points *somewhere else* readable
/// in a multi-file program. A note saying "the first one is at line 3" is a
/// half-answer once `include` is in play: the header names one file and the note
/// names a line, and a reader joins the two into a position that may not exist.
/// So the note carries the other place as a span and [`Diagnostics::render`]
/// spells it — the one place that knows how file `0` is spelled.
#[derive(Clone, Debug)]
pub struct Note {
    /// Written to read as a sentence that a location finishes: `note_at` glues
    /// the position on the end with a space and nothing else, so the text ends
    /// with the preposition — "the first one is at".
    pub text: String,
    pub span: Option<Span>,
}

#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: Code,
    pub span: Span,
    pub message: String,
    /// Notes are a list because a rejection that names its replacement and a
    /// rejection that explains itself are two different notes.
    pub notes: Vec<Note>,
}

impl Diagnostic {
    pub fn error(code: Code, span: Span, message: impl Into<String>) -> Self {
        Diagnostic {
            severity: Severity::Error,
            code,
            span,
            message: message.into(),
            notes: Vec::new(),
        }
    }

    pub fn warning(code: Code, span: Span, message: impl Into<String>) -> Self {
        Diagnostic {
            severity: Severity::Warning,
            code,
            span,
            message: message.into(),
            notes: Vec::new(),
        }
    }

    pub fn note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(Note {
            text: note.into(),
            span: None,
        });
        self
    }

    /// A note that points at a second place in the program — the other
    /// declaration, the other assignment, the other `return`.
    ///
    /// The text ends where the location begins, so write it ending in the
    /// preposition: `note_at("the first one is at", previous.span)` renders as
    /// `the first one is at a.lua:3:1`.
    pub fn note_at(mut self, note: impl Into<String>, span: Span) -> Self {
        self.notes.push(Note {
            text: note.into(),
            span: Some(span),
        });
        self
    }
}

/// The collector. Ordinary owned data: no `static mut`, no thread local, and no
/// `getCurrent()` in any spelling.
#[derive(Clone, Debug, Default)]
pub struct Diagnostics {
    items: Vec<Diagnostic>,
    /// The sources the spans are measured in, filled by the loader as it
    /// follows `include`. It sits beside the items because a span is
    /// not readable without it: `4:12` is a position only once something says
    /// *in which file*.
    files: Files,
}

impl Diagnostics {
    pub fn new() -> Self {
        Diagnostics::default()
    }

    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.items.push(diagnostic);
    }

    pub fn error(&mut self, code: Code, span: Span, message: impl Into<String>) {
        self.push(Diagnostic::error(code, span, message));
    }

    pub fn has_errors(&self) -> bool {
        self.items.iter().any(|d| d.severity == Severity::Error)
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Diagnostic> {
        self.items.iter()
    }

    /// True when `code` was raised at least once. The shape most tests want.
    pub fn contains(&self, code: Code) -> bool {
        self.items.iter().any(|d| d.code == code)
    }

    /// The source table these spans are measured in.
    pub fn files(&self) -> &Files {
        &self.files
    }

    /// For the loader, which is the only thing that fills it.
    pub fn files_mut(&mut self) -> &mut Files {
        &mut self.files
    }

    /// Every diagnostic, in the order raised — not just the first.
    ///
    /// `path` names file `0`: the caller knows how it wants the root spelled —
    /// `installua build` uses the path as typed — and every other file is named
    /// by the loader, relative to the same root.
    pub fn render(&self, path: &str) -> String {
        let mut out = String::new();
        for d in &self.items {
            out.push_str(&format!(
                "{}:{}: {}[{}]: {}\n",
                self.spell(d.span.file, path),
                d.span,
                d.severity,
                d.code,
                d.message
            ));
            for note in &d.notes {
                match note.span {
                    // The file is named only when it is not the one the header
                    // already named. A note that repeats the current file on
                    // every line is noise in the common case — one file — and
                    // the case this exists for is the other one.
                    Some(span) if span.file != d.span.file => out.push_str(&format!(
                        "  note: {} {}:{span}\n",
                        note.text,
                        self.spell(span.file, path)
                    )),
                    Some(span) => out.push_str(&format!("  note: {} {span}\n", note.text)),
                    None => out.push_str(&format!("  note: {}\n", note.text)),
                }
            }
        }
        out
    }

    /// How a file index is written: `path` for the root, because the caller
    /// knows how it wants that one spelled, and the loader's name for the rest.
    fn spell<'a>(&'a self, file: u32, path: &'a str) -> &'a str {
        if file == 0 {
            path
        } else {
            self.files.name(file).unwrap_or(path)
        }
    }
}
