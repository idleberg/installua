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
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub start_byte: usize,
    pub end_byte: usize,
}

impl Span {
    /// A span covering both ends, for a diagnostic that spans two nodes.
    pub fn join(self, other: Span) -> Span {
        let (start, end) = if self.start_byte <= other.start_byte {
            (self, other)
        } else {
            (other, self)
        };
        Span {
            start_line: start.start_line,
            start_column: start.start_column,
            end_line: end.end_line,
            end_column: end.end_column,
            start_byte: start.start_byte,
            end_byte: end.end_byte,
        }
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
    /// A call with the wrong number of arguments.
    WrongArity,

    // -- lowering
    /// `break` outside a loop.
    BreakOutsideLoop,
    /// `continue()` outside a loop. It is a call that jumps (§8), so unlike
    /// `break` it parses anywhere.
    ContinueOutsideLoop,
    /// The placeholder register file ran out. Twenty is not the real limit —
    /// the allocator is (§12).
    RegisterExhaustion,
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
        Code::BreakOutsideLoop,
        Code::ContinueOutsideLoop,
        Code::RegisterExhaustion,
        Code::NotYetImplemented,
        Code::UnknownField,
        Code::BadFieldValue,
        Code::DuplicateBlock,
        Code::MissingAttribute,
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
            Code::BreakOutsideLoop => "break-outside-loop",
            Code::ContinueOutsideLoop => "continue-outside-loop",
            Code::RegisterExhaustion => "register-exhaustion",
            Code::NotYetImplemented => "not-yet-implemented",
            Code::UnknownField => "unknown-field",
            Code::BadFieldValue => "bad-field-value",
            Code::DuplicateBlock => "duplicate-block",
            Code::MissingAttribute => "missing-attribute",
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

    /// Every diagnostic, in the order raised — not just the first (§9-4).
    pub fn render(&self, path: &str) -> String {
        let mut out = String::new();
        for d in &self.items {
            out.push_str(&format!(
                "{path}:{}: {}[{}]: {}\n",
                d.span, d.severity, d.code, d.message
            ));
            for note in &d.notes {
                out.push_str(&format!("  note: {note}\n"));
            }
        }
        out
    }
}
