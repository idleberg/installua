//! `MAKENSIS.echo`, `MAKENSIS.system`, `MAKENSIS.getDllVersion` — the build
//! machine's side effects, spelled so a reader cannot mistake them for code
//! that runs on the user's.
//!
//! Each is its `!` command, written where the call stands, and `makensis` runs
//! it when it reads that line. What one answers is caught the only way a
//! preprocessor answer can be: as a `!define` the next line copies into a
//! register. So the answer is an **install-time value** holding a build-time
//! fact, never a `<const>` — a `<const>` folds before `makensis` has run
//! anything, and that ordering is what keeps resolution order-free.

use crate::ast::Expr;
use crate::diag::{Code, Diagnostic, Span};
use crate::ir;
use crate::regs::Slot;
use crate::types::Ty;

use super::BodyLowerer;

/// The define a caught answer passes through. One name for every call: each
/// `!system` and `!getdllversion` redefines it, and the line that reads it is
/// the next one.
const ANSWER: &str = "_MAKENSIS";

impl BodyLowerer<'_, '_> {
    pub(super) fn makensis_call(
        &mut self,
        method: &str,
        args: &[Expr],
        dests: &[Slot],
        span: Span,
    ) -> Option<Vec<Ty>> {
        let (outputs, directive) = match method {
            "echo" => (0, "!echo"),
            "system" => (1, "!system"),
            "getDllVersion" => (4, "!getdllversion"),
            _ => {
                self.diags.push(
                    Diagnostic::error(
                        Code::UndefinedName,
                        span,
                        format!("`MAKENSIS` has no `{method}`"),
                    )
                    .note("it has `echo`, `system` and `getDllVersion`"),
                );
                return None;
            }
        };
        let [argument] = args else {
            self.diags.push(Diagnostic::error(
                Code::WrongArity,
                span,
                format!(
                    "`MAKENSIS.{method}` takes 1 argument(s), and {} were given",
                    args.len()
                ),
            ));
            return None;
        };
        if dests.len() > outputs {
            self.diags.push(Diagnostic::error(
                Code::WrongArity,
                span,
                format!(
                    "`MAKENSIS.{method}` answers {outputs} value(s), and {} are being bound",
                    dests.len()
                ),
            ));
            return None;
        }

        let value = self.value(argument)?.arg;
        let Some(value) = directive_text(&value) else {
            self.diags.push(
                Diagnostic::error(
                    Code::TypeMismatch,
                    argument.span(),
                    format!(
                        "`MAKENSIS.{method}` runs on the build machine, and this is known only \
                         at install time"
                    ),
                )
                .note("a literal, a `<const>` or `NSISDIR` is known there; a register is not"),
            );
            return None;
        };

        let mut line = vec![ir::Arg::raw(value)];
        if !dests.is_empty() {
            line.push(ir::Arg::raw(ANSWER));
        }
        self.emit(ir::Instruction::new(directive, line));

        let read = |suffix: &str| ir::Arg::raw(format!("\"${{{ANSWER}{suffix}}}\""));
        match method {
            "system" => {
                if let Some(dest) = dests.first() {
                    // `makensis` hands back what the C library's `system()`
                    // does, which off Windows is a wait status: `exit 3` reads
                    // 768. The shift makes it the exit code everywhere.
                    self.emit(ir::Instruction::new(
                        "!ifndef",
                        vec![ir::Arg::raw("NSIS_WIN32_MAKENSIS")],
                    ));
                    self.emit(ir::Instruction::new(
                        "!define",
                        vec![
                            ir::Arg::raw("/redef /math"),
                            ir::Arg::raw(ANSWER),
                            ir::Arg::raw(format!("${{{ANSWER}}}")),
                            ir::Arg::raw(">> 8"),
                        ],
                    ));
                    self.emit(ir::Instruction::new("!endif", Vec::new()));
                    self.emit(ir::Instruction::new(
                        "StrCpy",
                        vec![ir::Arg::dest(dest.clone()), read("")],
                    ));
                }
                Some(vec![Ty::int()])
            }
            "getDllVersion" => {
                // A file with no version resource defines all four as `""`
                // rather than failing — plugins and APEs often ship without
                // one — and `IntOp` reads `""` as the zero it means. A missing
                // file still stops the build, which is why there is no
                // `/noerrors`: that flag is what would make a typo silent.
                for (index, dest) in dests.iter().enumerate() {
                    self.emit(ir::Instruction::new(
                        "IntOp",
                        vec![
                            ir::Arg::dest(dest.clone()),
                            read(&(index + 1).to_string()),
                            ir::Arg::raw("+"),
                            ir::Arg::int(0),
                        ],
                    ));
                }
                Some(vec![Ty::nonneg(); 4])
            }
            _ => Some(Vec::new()),
        }
    }
}

/// The argument as `makensis` reads it on a `!` line, or `None` when it holds
/// something only the installer knows: a register or a `$VAR`.
///
/// Written here rather than by the emitter because the preprocessor unescapes
/// `$\"` but not `$$`, so the emitter's doubling would hand a shell
/// `$$HOME`. And no path rewrite: `makensis` opens the file itself, and off
/// Windows a `\` is not a separator.
fn directive_text(arg: &ir::Arg) -> Option<String> {
    let ir::Arg::Data { pieces, .. } = arg else {
        return None;
    };
    let mut out = String::from("\"");
    for piece in pieces {
        match piece {
            ir::Piece::Text(text) => out.push_str(&text.replace('"', "$\\\"")),
            ir::Piece::Const { name, .. } => out.push_str(&format!("${{{name}}}")),
            ir::Piece::Var(var) if var.starts_with("${") => out.push_str(var),
            ir::Piece::Var(_) | ir::Piece::Slot(_) => return None,
        }
    }
    out.push('"');
    Some(out)
}
