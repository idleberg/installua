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
pub mod assemble;
pub mod ast;
pub mod builtins;
pub mod callgraph;
pub mod cfg;
pub mod diag;
pub mod emit;
pub mod frontend;
pub mod headers;
pub mod ir;
pub mod layout;
pub mod locale;
pub mod lower;
pub mod map;
pub mod mui;
pub mod regs;
pub mod resolve;
pub mod retired;
pub mod stubs;
pub mod table;
pub mod types;

use std::path::PathBuf;

use crate::ast::Program;
use crate::diag::Diagnostics;

/// What the compiler needs from the world outside the source text.
///
/// Every entry is optional because §9-2 requires that all of this work against
/// an in-memory string with no file system involved. A source that never
/// reaches the build machine (no `glob`, no `include`) compiles either way; one
/// that does gets an honest diagnostic rather than a guess at the current
/// directory.
#[derive(Clone, Debug, Default)]
pub struct Options {
    /// The directory relative paths resolve against.
    pub base: Option<PathBuf>,
    /// The root source's own path, relative to `base` (§15.28). Without it a
    /// file that `include`s the root back cannot be recognised as the cycle it
    /// is, since the root would have no name for the loop to close on.
    pub root: Option<PathBuf>,
    /// Where `include` reads from. [`Loader::Disk`] by default.
    pub loader: frontend::include::Loader,
}

impl Options {
    /// The options `installua build <file>` uses: both halves of the path, so
    /// that relative paths mean the same thing wherever the build is run from.
    pub fn for_file(input: &std::path::Path) -> Options {
        let (base, root) = frontend::include::split(input);
        Options {
            base: Some(base),
            root: Some(root),
            ..Options::default()
        }
    }
}

/// Parses and checks `source` without lowering it — what an editor would call
/// on every keystroke.
///
/// **Not** what `installua check` runs: half the language's diagnostics come
/// from the lowering, so the command runs [`compile_with`] and discards the
/// module. What this is for is the caller that has to answer between
/// keystrokes and can afford to be told the rest a moment later.
///
/// Returns the checked tree even when diagnostics were raised, as long as the
/// source parsed: a caller that wants the errors reads `diags`, and one that
/// wants a tree for completion still gets one.
pub fn check(source: &str, diags: &mut Diagnostics) -> Option<Program> {
    frontend::check(source, diags)
}

/// The same, following `include` — one tree out of however many files named
/// each other (§15.28). `diags` comes back holding the table those spans are
/// measured in.
pub fn check_with(source: &str, options: &Options, diags: &mut Diagnostics) -> Option<Program> {
    frontend::include::load(source, options, diags)
}

/// Compiles as far as the IR, stopping before layout and emission.
///
/// This exists because §14 asks for assertions at pass boundaries rather than
/// only end to end: Phase 2's exit criterion is a claim about the CFG — that a
/// fused condition allocates no temporaries — and reading it out of emitted
/// text would be inferring a property from the absence of a line.
pub fn compile(source: &str, diags: &mut Diagnostics) -> Option<ir::Module> {
    compile_with(source, &Options::default(), diags)
}

/// The same, against a directory: what `installua build <file>` calls, with the
/// input's own directory as the base.
pub fn compile_with(
    source: &str,
    options: &Options,
    diags: &mut Diagnostics,
) -> Option<ir::Module> {
    let program = check_with(source, options, diags)?;
    if diags.has_errors() {
        return None;
    }

    let resolved = resolve::resolve(&program, diags);
    if diags.has_errors() {
        return None;
    }

    let module = lower::lower(&program, &resolved, options, diags);
    lower::check_required(&module, diags);
    if diags.has_errors() {
        return None;
    }
    Some(module)
}

/// Compiles `source` to `.nsi` text. `None` when any error was raised.
pub fn build(source: &str, diags: &mut Diagnostics) -> Option<String> {
    build_with(source, &Options::default(), diags)
}

pub fn build_with(source: &str, options: &Options, diags: &mut Diagnostics) -> Option<String> {
    build_mapped(source, options, diags).map(|(text, _)| text)
}

/// The `.nsi` and its line map (§15.22). What `installua build` calls, because
/// it has to translate whatever `makensis` says about the result.
pub fn build_mapped(
    source: &str,
    options: &Options,
    diags: &mut Diagnostics,
) -> Option<(String, map::LineMap)> {
    compile_with(source, options, diags).map(|module| emit::emit_mapped(&module))
}
