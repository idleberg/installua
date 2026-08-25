//! Install types at run time: `currentInstType`, `instTypes.getText(…)`.
//!
//! The same binding a second time. An install type's identity is its position
//! in the
//! block's `installTypes` list, and NSIS wants that number everywhere —
//! `SetCurInstType 1`, `InstTypeSetText 0 "…"`. Here the surface takes the
//! *name*, so inserting a type at the front of the list renumbers every use
//! silently and correctly, and a misspelling is an error rather than an
//! installer that offers the wrong thing.
//!
//! `currentInstType` is a name the compiler owns rather than a register: the
//! read is an instruction and the write is another one, so it is not in
//! [`crate::builtins::CONSTANTS`] — there is no `$CURINSTTYPE` to expand, and a
//! `Var` of the same name would be a second place the value lived.
//!
//! Reading it maps the position back to a name through a comparison chain built
//! at compile time, not through `InstTypeGetText`: the text is what
//! [`Self::inst_types_call`] lets a script *change*, and `currentInstType ==
//! "Full"` must keep working after it does.

use crate::ast::{Expr, Name};
use crate::cfg::{CmpOp, IntFamily, Terminator, Test};
use crate::diag::{Code, Diagnostic, Span};
use crate::ir;
use crate::regs::Slot;
use crate::resolve::ConstValue;
use crate::types::Ty;

use super::{BodyLowerer, list};

impl BodyLowerer<'_, '_> {
    /// `currentInstType` read — `GetCurInstType`, then the position turned back
    /// into the name the script wrote.
    ///
    /// Past the end of the list means the user chose the custom type, and there
    /// is no name for that: NSIS returns `${NSIS_MAX_INST_TYPES}` and the read
    /// yields `""`, which is this language's nothing.
    pub(super) fn owned_read(&mut self, name: &Name, dest: &Slot) -> Option<Ty> {
        if name.text != "currentInstType" {
            self.diags.push(
                Diagnostic::error(Code::TypeConflict, name.span, "`instTypes` is not a value")
                    .note(
                        "it is the compiler's table of the block's install types; write \
                     `instTypes.getText(\"Full\")` for one of their labels",
                    ),
            );
            return None;
        }

        let declared = self.inst_types.clone();
        if declared.is_empty() {
            self.diags.push(
                Diagnostic::error(
                    Code::BadFieldValue,
                    name.span,
                    "this block declares no install types",
                )
                .note(
                    "`currentInstType` is one of the names in `installTypes = { … }`, and with \
                     none declared it could only ever be `\"\"`",
                ),
            );
            return None;
        }

        let index = self.claim_temp(name.span);
        self.emit(ir::Instruction::new(
            "GetCurInstType",
            vec![ir::Arg::dest(index.clone())],
        ));

        let n = self.body.construct();
        let end = self.fresh(format!("insttype_{n}_end"));
        for (position, declared) in declared.iter().enumerate() {
            let hit = self.fresh(format!("insttype_{n}_{position}"));
            let miss = self.fresh(format!("insttype_{n}_{position}_next"));
            self.terminate(
                Terminator::Branch {
                    test: Test::Int {
                        op: CmpOp::Eq,
                        lhs: ir::Arg::slot(index.clone()),
                        rhs: ir::Arg::int(position as i64),
                        // A position is a position: it cannot be negative.
                        family: IntFamily::IntU,
                    },
                    then_block: hit,
                    else_block: miss,
                },
                hit,
            );
            self.emit(ir::Instruction::new(
                "StrCpy",
                vec![ir::Arg::dest(dest.clone()), ir::Arg::str(declared.clone())],
            ));
            self.terminate(Terminator::Jump(end), miss);
        }

        // The custom type, which every list has and no list declares.
        self.emit(ir::Instruction::new(
            "StrCpy",
            vec![ir::Arg::dest(dest.clone()), ir::Arg::str(String::new())],
        ));
        self.terminate(Terminator::Jump(end), end);
        Some(Ty::Str)
    }

    /// `currentInstType = "Minimal"` — `SetCurInstType`, and the position is the
    /// compiler's.
    pub(super) fn owned_write(&mut self, name: &Name, value: &Expr) {
        if name.text != "currentInstType" {
            self.diags.push(
                Diagnostic::error(
                    Code::TypeConflict,
                    name.span,
                    "`instTypes` cannot be assigned to",
                )
                .note(
                    "the list itself is the block's `installTypes` field, decided at compile \
                     time; `instTypes.setText(\"Full\", \"…\")` changes what one of them is \
                     called",
                ),
            );
            return;
        }
        let Some(position) = self.inst_type_position(value, "currentInstType") else {
            return;
        };
        self.emit(ir::Instruction::new(
            "SetCurInstType",
            vec![ir::Arg::int(position as i64)],
        ));
    }

    /// `instTypes.getText(name)` and `instTypes.setText(name, text)`.
    ///
    /// A compiler-owned table addressed by string rather than a handle, because
    /// an install type is a line in a block's field and there is nothing to
    /// bind a `local` to.
    pub(super) fn inst_types_call(
        &mut self,
        method: &str,
        args: &[Expr],
        dest: Option<&Slot>,
        span: Span,
    ) -> Option<Ty> {
        match (method, args) {
            ("instTypes.getText", [which]) => {
                let Some(dest) = dest else {
                    self.diags.push(
                        Diagnostic::error(
                            Code::TypeMismatch,
                            span,
                            "`instTypes.getText` answers a question that nothing reads",
                        )
                        .note("write `local label = instTypes.getText(\"Full\")`"),
                    );
                    return None;
                };
                let position = self.inst_type_position(which, "instTypes.getText")?;
                self.emit(ir::Instruction::new(
                    "InstTypeGetText",
                    vec![ir::Arg::int(position as i64), ir::Arg::dest(dest.clone())],
                ));
                Some(Ty::Str)
            }

            ("instTypes.setText", [which, text]) => {
                if dest.is_some() {
                    self.diags.push(
                        Diagnostic::error(
                            Code::TypeMismatch,
                            span,
                            "`instTypes.setText` produces no value",
                        )
                        .note("`InstTypeSetText` writes to no register"),
                    );
                    return None;
                }
                let position = self.inst_type_position(which, "instTypes.setText")?;
                let label = self.value(text)?;
                if label.ty != Ty::Str && label.ty != Ty::Unknown {
                    self.diags.push(
                        Diagnostic::error(
                            Code::TypeConflict,
                            text.span(),
                            format!(
                                "an install type's label is a `string`, and this is a {}",
                                label.ty
                            ),
                        )
                        .note("it is the row the components page draws in its drop-down"),
                    );
                    return None;
                }
                self.emit(ir::Instruction::new(
                    "InstTypeSetText",
                    vec![ir::Arg::int(position as i64), label.arg],
                ));
                None
            }

            (name, _) => {
                let wanted = if name == "instTypes.getText" { 1 } else { 2 };
                self.diags.push(
                    Diagnostic::error(
                        Code::WrongArity,
                        span,
                        format!(
                            "`{name}` takes {wanted} argument(s), and {} were given",
                            args.len()
                        ),
                    )
                    .note(
                        "write `instTypes.getText(\"Full\")` or `instTypes.setText(\"Full\", \
                         \"Everything\")`",
                    ),
                );
                None
            }
        }
    }

    /// A name from the block's `installTypes` list as the position NSIS wants.
    ///
    /// Compile-time, and it has to be: the names are the block's declaration
    /// and the positions exist in exactly one place, so resolving one at run
    /// time would be a second numbering to keep in step with the first.
    pub(super) fn inst_type_position(&mut self, value: &Expr, what: &str) -> Option<usize> {
        let Some(ConstValue::Str(name)) = self.constant(value) else {
            self.diags.push(
                Diagnostic::error(
                    Code::BadFieldValue,
                    value.span(),
                    format!("`{what}` wants a name the block declared"),
                )
                .note("the position is resolved at compile time, so the name has to be one too"),
            );
            return None;
        };
        let declared = self.inst_types.clone();
        let Some(position) = declared.iter().position(|known| *known == name) else {
            let note = if declared.is_empty() {
                "this block declares no install types; write `installTypes = { … }` beside its \
                 sections"
                    .to_string()
            } else {
                format!(
                    "the install types are {}",
                    list(&declared.iter().map(String::as_str).collect::<Vec<_>>())
                )
            };
            self.diags.push(
                Diagnostic::error(
                    Code::UnknownField,
                    value.span(),
                    format!("`{name}` is not an install type"),
                )
                .note(note),
            );
            return None;
        };
        Some(position)
    }
}
