//! The frontend: source text in, a checked Installua AST out.
//!
//! Four steps, in the order PLAN Phase 1 names them:
//!
//!   1. parse with `full-moon` (real Lua 5.4, no dialect of our own)
//!   2. the whitelist pass — which Lua forms exist in this language
//!   3. escape-sequence validation, because `full-moon` accepts
//!      `"C:\Program Files"` and real Lua does not (§13)
//!   4. `elseif` desugaring and float-literal rejection
//!
//! Steps 2–4 are one walk. Splitting them into three would mean three ways to
//! spell "this node's span", and the whole point of the pass is that there is
//! one place that decides what the language is.

pub mod include;
pub mod lift;
pub mod strings;

use crate::ast::Program;
use crate::diag::{Code, Diagnostic, Diagnostics, Span};

/// Parses and checks `source` as the root of a one-file program.
///
/// Returns `None` only when parsing failed outright — a source that parses
/// always produces a tree, however many diagnostics it also produced, so a
/// caller can keep checking (§9-4). `include` is *not* followed here: loading
/// needs a file system, and [`include::load`] is where that is decided.
pub fn check(source: &str, diags: &mut Diagnostics) -> Option<Program> {
    check_file(source, 0, diags)
}

/// The same, for a source that is one file of several: every span it produces
/// is stamped with `file` (§15.28).
pub fn check_file(source: &str, file: u32, diags: &mut Diagnostics) -> Option<Program> {
    let ast = match full_moon::parse(source) {
        Ok(ast) => ast,
        Err(errors) => {
            for error in errors {
                let span = match &error {
                    full_moon::Error::AstError(error) => position_span(error.range().0, file),
                    full_moon::Error::TokenizerError(error) => {
                        position_span(error.position(), file)
                    }
                };
                diags.push(Diagnostic::error(
                    Code::ParseError,
                    span,
                    error.error_message().to_string(),
                ));
            }
            return None;
        }
    };

    Some(lift::lift(&ast, file, diags))
}

fn position_span(position: full_moon::tokenizer::Position, file: u32) -> Span {
    Span {
        file,
        start_line: position.line(),
        start_column: position.character(),
        end_line: position.line(),
        end_column: position.character(),
        start_byte: position.bytes(),
        end_byte: position.bytes(),
    }
}
