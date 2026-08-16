//! Installua: a Lua-shaped language that compiles to NSIS.
//!
//! **Everything here is re-entrant.** There is no `static mut`, no thread-local
//! diagnostic sink and no `getCurrent()` in any spelling — §9-2 is an
//! architectural requirement rather than a preference, because nsL's statics
//! are exactly why it can never be an LSP backend. Two sources can be compiled
//! concurrently in one process, against in-memory strings, with no file system
//! involved: the CLI is one caller of this API and never a privileged one.
//!
//! The pipeline (§9-1):
//!
//! ```text
//! source ─frontend─▶ AST ─resolve─▶ symbols ─lower─▶ CFG ─alloc─▶ registers ─layout─▶ IR ─emit─▶ .nsi
//! ```
//!
//! `resolve` runs to completion before any body is lowered, which is what makes
//! the language order-free (§15.6) — and it can be, because Installua compiles
//! rather than executing Lua at build time, so there is no evaluation order for
//! a declaration to have to precede.
//!
//! Two of those arrows are **whole-program** rather than per-body, and both are
//! fixpoints. `lower` runs repeatedly against a signature table until types stop
//! changing, because a parameter's type comes from the call sites and a return
//! type comes from the body (§15.14). `alloc` then colours every body before
//! [`callgraph`] propagates clobber sets over the SCC condensation, because
//! caller-saves are `live ∩ clobbered` and neither half exists earlier (§15.11).
//! The cost is that Installua has no separately-compilable unit — a door closed
//! deliberately, since an installer is one program with one output.

pub mod alloc;
pub mod ast;
pub mod builtins;
pub mod callgraph;
pub mod cfg;
pub mod diag;
pub mod emit;
pub mod frontend;
pub mod ir;
pub mod layout;
pub mod lower;
pub mod regs;
pub mod resolve;
pub mod types;

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

/// Compiles as far as the IR, stopping before layout and emission.
///
/// This exists because §14 asks for assertions at pass boundaries rather than
/// only end to end: Phase 2's exit criterion is a claim about the CFG — that a
/// fused condition allocates no temporaries — and reading it out of emitted
/// text would be inferring a property from the absence of a line.
pub fn compile(source: &str, diags: &mut Diagnostics) -> Option<ir::Module> {
    let program = check(source, diags)?;
    if diags.has_errors() {
        return None;
    }

    let resolved = resolve::resolve(&program, diags);
    if diags.has_errors() {
        return None;
    }

    let module = lower::lower(&program, &resolved, diags);
    lower::check_required(&module, diags);
    if diags.has_errors() {
        return None;
    }
    Some(module)
}

/// Compiles `source` to `.nsi` text. `None` when any error was raised.
pub fn build(source: &str, diags: &mut Diagnostics) -> Option<String> {
    compile(source, diags).map(|module| emit::emit(&module))
}
