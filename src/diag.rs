//! Diagnostics as data (PLAN Phase 1, §9-4): span, code, severity, notes.
//!
//! Collected, never thrown, and never routed through a global sink — the
//! collector is threaded as `&mut` so that a caller can compile two sources
//! concurrently in one process (§9-2). Every entry point in this crate takes
//! one; nothing in this crate owns one.

use std::fmt;

/// A half-open source range, in both the units a human reads and the units a
/// tool slices with.
///
/// `full-moon` reports `line`/`character` 1-based, which is also what
/// `makensis` and every editor protocol want, so no conversion happens here.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Span {
    /// Which source this span is measured in — an index into [`Files`], where
    /// `0` is always the file the build was started from (§15.28).
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
/// started from, and the rest are what `include` pulled in, in load order
/// (§15.28).
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

    pub fn len(&self) -> usize {
        self.names.len()
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.names.iter().map(String::as_str)
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
/// one is actually reachable (PLAN §2). A code with no test that produces it is
/// a code nobody has checked the wording of.
///
/// Codes are slugs rather than numbers deliberately — a number has to be
/// allocated, never reused and kept in a table, and buys nothing over a name
/// that is greppable in both the source and a user's terminal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Code {
    // -- frontend: parse
    /// `full-moon` could not parse the source as Lua 5.4.
    ParseError,

    // -- frontend: literals (§5)
    /// A float literal. NSIS has no float arithmetic at all (§6).
    FloatLiteral,
    /// An escape sequence that is invalid in Lua 5.4. `full-moon` accepts these
    /// and real Lua does not, so the frontend checks them itself (§13).
    InvalidEscape,
    /// A `$` followed by an identifier character inside a literal — the
    /// canonical NSIS muscle-memory trap, since literals are data (§15.1).
    DollarInLiteral,
    /// A literal at or over the vanilla `NSIS_MAX_STRLEN` floor, which
    /// `makensis` truncates at runtime with no diagnostic at all (§15.31).
    OverlongLiteral,

    // -- frontend: operators (§6)
    /// `/` is float division, which does not exist here.
    FloatDivision,
    /// `^` is exponentiation in Lua and *xor* in NSIS; remapping it silently
    /// would be the exact class of bug this compiler exists to prevent.
    Exponentiation,
    /// `#` on a string counts bytes in Lua and UTF-16 code units in NSIS
    /// (§15.29).
    LengthOperator,

    // -- frontend: statements and expressions (§3, §7)
    /// `goto` or a `::label::`. §8 owns labels.
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
    /// are declaration bodies, never values (§3).
    ClosureValue,
    /// `local x <close>`: there is no runtime to close over.
    CloseAttribute,
    /// A `local` attribute that is neither `<const>` nor `<close>`.
    UnknownAttribute,
    /// `t[k]`. There are no runtime tables to index.
    IndexExpression,
    /// A `for … in` over something outside the iterator whitelist (§7).
    UnsupportedIterator,
    /// `require`. Loading is compile-time, and spelled `import`/`include`.
    RuntimeRequire,

    // -- resolution and types (§15.6, §15.14, §15.20, §15.24)
    /// A name that resolves to nothing. Resolution is order-free, so this
    /// really does mean *nowhere in the file* (§15.6).
    UndefinedName,
    /// A non-`bool` in a condition, or as an operand of `and`/`or`/`not`.
    /// By-type truthiness is not merely inference-dependent, it disagrees with
    /// Lua on `0` and `""` — the two values a reader is most likely to test
    /// (§15.20).
    NotBool,
    /// `local x = a or b` on strings: the default-value idiom. Under Lua's
    /// semantics it is dead code, and under the intended NSIS semantics the
    /// source lies, so there is no reading that works (§15.20).
    OrAsValue,
    /// A comparison whose operands are different types, or whose types are not
    /// known. Defaulting to `StrCmp` is how a compiler becomes a text expander
    /// with a type system bolted on (§15.14).
    TypeMismatch,
    /// A variable assigned two different types. A register is one slot, so this
    /// is NSIS-shaped rather than arbitrary, and it reports the disagreement
    /// rather than privileging whichever line came first (§15.24).
    TypeConflict,
    /// A call with the wrong number of arguments, or a binding that wants more
    /// values than the callee returns.
    WrongArity,
    /// A `func` returning a different number of values on two paths. `Call` has
    /// no arity at all — the callee pushes and the caller pops — so a
    /// disagreement is a stack that unbalances at runtime with no diagnostic
    /// from NSIS (§3).
    ReturnArity,

    // -- lowering
    /// `break` outside a loop.
    BreakOutsideLoop,
    /// `continue()` outside a loop. It is a call that jumps (§8), so unlike
    /// `break` it parses anywhere.
    ContinueOutsideLoop,
    /// More values live at once than NSIS has registers. Unlike Phase 2's
    /// placeholder, this is the real limit: values whose live ranges do not
    /// overlap already share a register (§9-3).
    RegisterExhaustion,
    /// A value bound from a dialog that has one button. A **warning**: the
    /// program is well-formed, and the comparison underneath it is simply
    /// already decided (§15.18).
    ConstantAnswer,
    /// Unbounded recursion. A **warning**, because it is legal and sometimes
    /// intended — and one worth having, since §3 measured the failure as a
    /// silent process death at roughly 1300 frames (§15.11).
    DeepRecursion,
    /// Well-formed, whitelisted, and outside what this version emits. This is
    /// the honest edge of the vertical slice (PLAN §0), not a parse failure.
    NotYetImplemented,
    /// A field name that no block accepts.
    UnknownField,
    /// A field whose value has the wrong shape for its name.
    BadFieldValue,
    /// A block that may appear once appearing twice.
    DuplicateBlock,
    /// A required attribute that no block supplied.
    MissingAttribute,

    // -- loading (§15.28)
    /// An `include` whose file is missing, unreadable, or has no directory to
    /// resolve against.
    IncludeNotFound,
    /// A file that includes itself, however long the way round. Naming the
    /// whole loop is the point: the file the loader noticed it at is rarely the
    /// one with the mistake in it.
    IncludeCycle,
    /// An `include` written somewhere it cannot mean anything: inside a body,
    /// or with an argument that is not a string literal. The path has to be
    /// readable without running anything, which is §2's staging rule and not a
    /// parser limitation.
    IncludeForm,

    /// An NSIS instruction that has a Lua spelling instead: `StrCmp` is `==`,
    /// `IntOp` is `+`, `StrCpy` is assignment. Not an unknown name — the
    /// compiler knows exactly what it is, and says what to write (§5).
    NsisRetired,
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
        Code::IncludeNotFound,
        Code::IncludeCycle,
        Code::IncludeForm,
        Code::NsisRetired,
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
            Code::IncludeNotFound => "include-not-found",
            Code::IncludeCycle => "include-cycle",
            Code::IncludeForm => "include-form",
            Code::NsisRetired => "nsis-retired",
        }
    }
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.slug())
    }
}

#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: Code,
    pub span: Span,
    pub message: String,
    /// Notes are a list because a rejection that names its replacement and a
    /// rejection that explains itself are two different notes (PLAN §2).
    pub notes: Vec<String>,
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
        self.notes.push(note.into());
        self
    }
}

/// The collector. Ordinary owned data: no `static mut`, no thread local, and no
/// `getCurrent()` in any spelling (§9-2).
#[derive(Clone, Debug, Default)]
pub struct Diagnostics {
    items: Vec<Diagnostic>,
    /// The sources the spans are measured in, filled by the loader as it
    /// follows `include` (§15.28). It sits beside the items because a span is
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

    /// Every diagnostic, in the order raised — not just the first (§9-4).
    ///
    /// `path` names file `0`: the caller knows how it wants the root spelled —
    /// `installua build` uses the path as typed — and every other file is named
    /// by the loader, relative to the same root.
    pub fn render(&self, path: &str) -> String {
        let mut out = String::new();
        for d in &self.items {
            let file = if d.span.file == 0 {
                path
            } else {
                self.files.name(d.span.file).unwrap_or(path)
            };
            out.push_str(&format!(
                "{file}:{}: {}[{}]: {}\n",
                d.span, d.severity, d.code, d.message
            ));
            for note in &d.notes {
                out.push_str(&format!("  note: {note}\n"));
            }
        }
        out
    }
}
