//! Luis proof of concept: compile a Lua-shaped source file to NSIS.
//!
//! Four phases (§9-1): parse -> check -> lower -> emit. The subset covered is
//! deliberately tiny — `installer { name, outFile }` plus `section` bodies made
//! of constant-argument instruction calls.

pub mod ast;
pub mod diag;
pub mod emit;
pub mod frontend;
pub mod ir;
pub mod lower;
pub mod overlay;

use diag::Diagnostics;

/// Compiles Luis source to NSIS. Returns the output when no error was raised;
/// diagnostics are collected either way.
pub fn compile(source: &str, diags: &mut Diagnostics) -> Option<String> {
    let program = frontend::parse(source, diags)?;
    if diags.has_errors() {
        return None;
    }

    let module = lower::lower(&program, diags);
    if diags.has_errors() {
        return None;
    }

    Some(emit::emit(&module))
}
