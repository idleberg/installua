//! `radioButtons { a, b, c }` — one of these sections ticked at a time, through
//! the stock `Sections.nsh`.
//!
//! ```lua
//! installer {
//!   a, b, c,
//!   radioButtons { a, b, c },
//! }
//! ```
//!
//! Lowered through the header, like `memento {}`: a `Var` holds the ticked
//! section's index, set to the first listed in the init prelude, and
//! `StartRadioButtons`/`RadioButton`/`EndRadioButtons` go first in the half's
//! `.onSelChange` — the author's, or one of the compiler's own. An entry of its
//! block and not a field of a section, because it is about several sections
//! and no one of them owns it.
//!
//! Its own pass, after the claims: which half a section is in is what the
//! pass checks, and the lines it adds have to be in place before either
//! callback lowers, wherever the block lists them.

use crate::ast::{Expr, Stmt, TableField};
use crate::cfg;
use crate::diag::{Code, Diagnostic};
use crate::ir;

use super::{DeferredKind, Half, Lowerer, index_name};

impl Lowerer<'_, '_> {
    pub(super) fn radio_pass(&mut self) {
        for stmt in self.resolved.block.clone() {
            let Stmt::Call(call) = stmt else {
                continue;
            };
            let half = match call.callee_name() {
                Some("installer") => Half::Installer,
                Some("uninstaller") => Half::Uninstaller,
                _ => continue,
            };
            let Expr::Call { args, .. } = call else {
                continue;
            };
            let [Expr::Table { fields, .. }] = args.as_slice() else {
                continue;
            };
            for field in fields {
                if let TableField::Positional { value } = field
                    && value.callee_name() == Some("radioButtons")
                {
                    self.radio_buttons(value, half);
                }
            }
        }
    }

    fn radio_buttons(&mut self, value: &Expr, half: Half) {
        let Some(fields) = self.block_fields(value) else {
            return;
        };
        let mut indices = Vec::new();
        let mut first = None;
        for field in fields {
            let TableField::Positional {
                value: Expr::Name(name),
            } = field
            else {
                self.diags.push(
                    Diagnostic::error(
                        Code::BadFieldValue,
                        field.span(),
                        "`radioButtons {}` lists sections by their `local`",
                    )
                    .note("write `radioButtons { a, b, c }`"),
                );
                return;
            };
            let section = self
                .resolved
                .deferred
                .get(&name.text)
                .is_some_and(|deferred| deferred.kind == DeferredKind::Section);
            match self.claims.get(&name.text) {
                Some(claim) if section && claim.half == half => {
                    first.get_or_insert(&name.text);
                    indices.push(index_name(&name.text, half));
                }
                // Unclaimed: claim rule 1 says so at the declaration.
                None if section => return,
                _ => {
                    self.diags.push(
                        Diagnostic::error(
                            Code::BadFieldValue,
                            name.span,
                            format!("`{}` is not a section of the `{half}`", name.text),
                        )
                        .note("`radioButtons {}` lists sections this block lists"),
                    );
                    return;
                }
            }
        }
        if indices.len() < 2 {
            self.diags.push(
                Diagnostic::error(
                    Code::BadFieldValue,
                    value.span(),
                    "`radioButtons {}` needs two sections or more",
                )
                .note("with one, there is nothing to choose between"),
            );
            return;
        }

        let first = first.map_or("", String::as_str);
        let var = match half {
            Half::Installer => format!("${}radio_{first}", cfg::LABEL_PREFIX),
            Half::Uninstaller => format!("${}unradio_{first}", cfg::LABEL_PREFIX),
        };
        let index = |name: &str| ir::Arg::raw(format!("${{{name}}}"));
        self.requires.headers.insert("Sections".to_string());
        self.radio_vars.push(var[1..].to_string());
        self.init_prelude[half.index()].push(ir::Instruction::new(
            "StrCpy",
            vec![ir::Arg::var(var.clone()), index(&indices[0])],
        ));
        let lines = &mut self.sel_prelude[half.index()];
        lines.push(ir::Instruction::new(
            "!insertmacro",
            vec![ir::Arg::raw("StartRadioButtons"), ir::Arg::var(var)],
        ));
        for name in &indices {
            lines.push(ir::Instruction::new(
                "!insertmacro",
                vec![ir::Arg::raw("RadioButton"), index(name)],
            ));
        }
        lines.push(ir::Instruction::new(
            "!insertmacro",
            vec![ir::Arg::raw("EndRadioButtons")],
        ));
    }
}
