//! Installua: a Lua-shaped language that compiles to NSIS.
//!
//! **Everything here is re-entrant.** There is no `static mut`, no thread-local
//! diagnostic sink and no `getCurrent()` in any spelling — §9-2 is an
//! architectural requirement rather than a preference, because nsL's statics
//! are exactly why it can never be an LSP backend. Two sources can be compiled
//! concurrently in one process, against in-memory strings, with no file system
//! involved: the CLI is one caller of this API and never a privileged one.
//!
//! The pipeline is four passes (§9-1):
//!
//! ```text
//! source ──frontend──▶ AST ──lower──▶ IR ──emit──▶ .nsi
//! ```

pub mod ast;
pub mod diag;
pub mod emit;
pub mod frontend;
pub mod ir;
pub mod lower;

use crate::ast::Program;
use crate::diag::Diagnostics;

/// Parses and checks `source` without lowering it — what `installua check`
/// runs, and what an editor would call on every keystroke.
///
/// Returns the checked tree even when diagnostics were raised, as long as the
/// source parsed: a caller that wants the errors reads `diags`, and one that
/// wants a tree for completion still gets one.
pub fn check(source: &str, diags: &mut Diagnostics) -> Option<Program> {
    frontend::check(source, diags)
}

/// Compiles `source` to `.nsi` text. `None` when any error was raised.
pub fn build(source: &str, diags: &mut Diagnostics) -> Option<String> {
    let program = check(source, diags)?;
    if diags.has_errors() {
        return None;
    }

    let module = lower::lower(&program, diags);
    lower::check_required(&module, diags);
    if diags.has_errors() {
        return None;
    }

    Some(emit::emit(&module))
}
