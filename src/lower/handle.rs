//! Section handles: `core.selected`, `docs.text = ""`.
//!
//! A handle is the §13 binding made runtime-visible. `local core = section { … }`
//! binds no value — there is nothing at run time for `core` to *be* — and the
//! block that lists it turns it into a `!define`, so `core.selected` is
//! `SectionGetFlags ${SEC_core}` and the bit test after it. The number NSIS
//! actually wants never appears in the source, which is the whole point: a
//! section can be moved, renamed or given a sibling and nothing renumbers.
//!
//! Two things are decided here rather than in [`super::expr`]. The first is
//! claim rule 4 — a handle read from the half that did not list it names a
//! section that is not in that executable, and NSIS would not notice, because
//! one `.nsi` holds both halves and the define exists either way. The second is
//! the read-modify-write: `Sections.nsh` spells `SelectSection` as a macro that
//! clobbers `$0`, and this compiler has a register allocator, so it emits the
//! three instructions itself (ruling 6).

use crate::ast::{Expr, Name, TableField};
use crate::diag::{Code, Diagnostic, Span};
use crate::ir;
use crate::regs::Slot;
use crate::resolve::{ConstValue, DeferredKind};
use crate::types::{Int, Sign, Ty, Width};

use super::{BodyLowerer, HANDLE_FIELDS, HandleField, Where, handle_field, index_name, list};

/// A handle, resolved: the define it is addressed through and which of the two
/// things it is.
pub(super) struct Handle {
    index: String,
    kind: DeferredKind,
}

impl BodyLowerer<'_, '_> {
    /// `core` as a handle, when that is what the name is.
    ///
    /// `None` without a diagnostic when the name is not a declaration at all:
    /// the caller is looking at `a.b` and has its own thing to say about a `b`
    /// on something that is not a handle.
    pub(super) fn handle(&mut self, base: &str, span: Span) -> Option<Handle> {
        let kind = self.resolved.deferred.get(base)?.kind;
        // Unclaimed. Claim rule 1 has already said so at the declaration, and
        // saying it again at every use would bury it.
        let claim = self.claims.get(base)?;

        // Claim rule 4. The define is in the file — one `.nsi` holds both
        // halves — so `${SEC_core}` in `un.onInit` compiles, addresses whatever
        // index the installer's `Core` got, and means nothing in an executable
        // that does not contain it.
        if let Some(half) = self.half
            && claim.half != half
        {
            self.diags.push(
                Diagnostic::error(
                    Code::UnknownField,
                    span,
                    format!("`{base}` is a `{}` of the `{}`", kind.word(), claim.half),
                )
                .note(format!(
                    "this code runs in the `{half}`, which has no `{base}`"
                ))
                .note(
                    "the two halves are two executables: an index from one is a number the \
                     other's sections do not share (§13)",
                ),
            );
            return None;
        }

        Some(Handle {
            index: index_name(base, claim.half),
            kind,
        })
    }

    /// The field, checked against what the handle is. `expanded` is a heading's
    /// and `size` is a section's, and the handle knows which it holds.
    fn field(&mut self, handle: &Handle, field: &Name) -> Option<HandleField> {
        let Some(what) = handle_field(&field.text) else {
            self.diags.push(
                Diagnostic::error(
                    Code::UnknownField,
                    field.span,
                    format!("`{}` is not a field of a section", field.text),
                )
                .note(format!("the fields are {}", list(HANDLE_FIELDS))),
            );
            return None;
        };
        let on = match what {
            HandleField::Flag { on, .. } => on,
            // A group has no size and is in no install type: what it holds is
            // sections, and each of those answers for itself.
            HandleField::Size | HandleField::InstallTypes => Where::Sections,
            HandleField::Text => Where::Both,
        };
        if !on.accepts(handle.kind) {
            let (is, isnt) = match handle.kind {
                DeferredKind::Section => ("a `section`", "a `group`"),
                DeferredKind::Group => ("a `group`", "a `section`"),
            };
            self.diags.push(
                Diagnostic::error(
                    Code::UnknownField,
                    field.span,
                    format!("`{}` is {isnt}'s field, and this is {is}", field.text),
                )
                .note(match what {
                    HandleField::Flag { .. } => {
                        "`expanded` opens a heading in the components tree, and a section is not \
                         one"
                    }
                    _ => {
                        "a group holds sections and nothing else: the size and the install types \
                         are each section's"
                    }
                }),
            );
            return None;
        }
        Some(what)
    }

    /// `core.selected` — a read, which is one `Section*Get` and, for a flag, the
    /// bit test after it.
    pub(super) fn handle_read(&mut self, base: &Expr, field: &Name, dest: &Slot) -> Option<Ty> {
        let handle = self.handle(base.name()?, base.span())?;
        let index = ir::Arg::raw(format!("${{{}}}", handle.index));
        match self.field(&handle, field)? {
            HandleField::Flag { shift, .. } => {
                let flags = self.claim_temp(field.span);
                self.emit(ir::Instruction::new(
                    "SectionGetFlags",
                    vec![index, ir::Arg::dest(flags.clone())],
                ));
                // Down to bit 0 and masked. A `bool` in this compiler is `0` or
                // `1` and nothing else (§15.20), so `SF_RO` being 16 is not a
                // truth value that happens to work — it is the wrong number.
                let source = if shift == 0 {
                    ir::Arg::slot(flags)
                } else {
                    self.emit(ir::Instruction::new(
                        "IntOp",
                        vec![
                            ir::Arg::dest(dest.clone()),
                            ir::Arg::slot(flags),
                            ir::Arg::raw(">>>"),
                            ir::Arg::int(i64::from(shift)),
                        ],
                    ));
                    ir::Arg::slot(dest.clone())
                };
                self.emit(ir::Instruction::new(
                    "IntOp",
                    vec![
                        ir::Arg::dest(dest.clone()),
                        source,
                        ir::Arg::raw("&"),
                        ir::Arg::int(1),
                    ],
                ));
                Some(Ty::Bool)
            }
            HandleField::Text => {
                self.emit(ir::Instruction::new(
                    "SectionGetText",
                    vec![index, ir::Arg::dest(dest.clone())],
                ));
                Some(Ty::Str)
            }
            HandleField::Size => {
                self.emit(ir::Instruction::new(
                    "SectionGetSize",
                    vec![index, ir::Arg::dest(dest.clone())],
                ));
                // Kilobytes, and a section cannot give space back.
                Some(Ty::Int(Int {
                    width: Width::W32,
                    sign: Sign::NonNeg,
                }))
            }
            // `SectionGetInstTypes` yields the bit field NSIS stores, and the
            // field's type is `string[]`. There is no list value in this
            // language to decode it into, so the read waits for one rather than
            // handing back a number the surface never promised.
            HandleField::InstallTypes => {
                self.todo(field.span, "reading `installTypes`");
                None
            }
        }
    }

    /// `docs.text = ""` — a write, which is one `Section*Set` and, for a flag,
    /// the read-modify-write around it.
    pub(super) fn handle_write(&mut self, base: &Expr, field: &Name, value: &Expr) {
        let span = base.span();
        let Some(handle) = base.name().and_then(|base| self.handle(base, span)) else {
            return;
        };
        let Some(what) = self.field(&handle, field) else {
            return;
        };
        let index = || ir::Arg::raw(format!("${{{}}}", handle.index));
        match what {
            HandleField::Flag { bit, shift, .. } => {
                self.write_flag(&index(), bit, shift, value, field)
            }
            HandleField::Text => {
                let Some(text) = self.value(value) else {
                    return;
                };
                if text.ty != Ty::Str && text.ty != Ty::Unknown {
                    self.diags.push(
                        Diagnostic::error(
                            Code::TypeConflict,
                            value.span(),
                            format!("`text` is a `string`, and this is a {}", text.ty),
                        )
                        .note(
                            "it is the row the components tree draws; blank it to draw no row                              at all",
                        ),
                    );
                    return;
                }
                self.emit(ir::Instruction::new(
                    "SectionSetText",
                    vec![index(), text.arg],
                ));
            }
            HandleField::Size => {
                let Some(size) = self.value(value) else {
                    return;
                };
                if self.require_int(&size, value.span()).is_none() {
                    return;
                }
                self.emit(ir::Instruction::new(
                    "SectionSetSize",
                    vec![index(), size.arg],
                ));
            }
            HandleField::InstallTypes => {
                let Some(mask) = self.inst_type_mask(value) else {
                    return;
                };
                self.emit(ir::Instruction::new(
                    "SectionSetInstTypes",
                    vec![index(), ir::Arg::int(mask)],
                ));
            }
        }
    }

    /// One bit of the flags word, set to `value`.
    ///
    /// A literal is the case worth spending code on, because it is the one every
    /// script writes: `docs.selected = false` is three instructions, and the
    /// general form — a bit cleared and then or-ed back in from a register — is
    /// six. Both are what `Sections.nsh`'s `SelectSection` does by hand, minus
    /// its clobber of `$0`.
    fn write_flag(&mut self, index: &ir::Arg, bit: u32, shift: u32, value: &Expr, field: &Name) {
        let bit = i64::from(bit);
        let flags = self.claim_temp(field.span);
        let mut masked = false;
        let literal = match self.constant(value) {
            Some(ConstValue::Bool(flag)) => Some(flag),
            _ => None,
        };

        let lowered = match literal {
            Some(_) => None,
            None => {
                let Some(typed) = self.value(value) else {
                    return;
                };
                if typed.ty != Ty::Bool && typed.ty != Ty::Unknown {
                    self.diags.push(
                        Diagnostic::error(
                            Code::TypeConflict,
                            value.span(),
                            format!("`{}` is a `bool`, and this is a {}", field.text, typed.ty),
                        )
                        .note("it is one bit of the section's flags, so there is no third value"),
                    );
                    return;
                }
                // A `bool` is already `0` or `1` (§15.20), so the mask below
                // is skipped: the invariant is the reason the type exists.
                masked = typed.ty == Ty::Bool;
                Some(typed.arg)
            }
        };

        self.emit(ir::Instruction::new(
            "SectionGetFlags",
            vec![index.clone(), ir::Arg::dest(flags.clone())],
        ));

        match literal {
            Some(true) => self.emit(ir::Instruction::new(
                "IntOp",
                vec![
                    ir::Arg::dest(flags.clone()),
                    ir::Arg::slot(flags.clone()),
                    ir::Arg::raw("|"),
                    ir::Arg::int(bit),
                ],
            )),
            Some(false) => self.clear(&flags, bit, field.span),
            None => {
                self.clear(&flags, bit, field.span);
                // The value back where it came from: `0`/`1` masked, shifted up
                // to the bit's position, or-ed in.
                let carry = self.claim_temp(field.span);
                let mut source = lowered.expect("a non-literal was lowered");
                if !masked {
                    self.emit(ir::Instruction::new(
                        "IntOp",
                        vec![
                            ir::Arg::dest(carry.clone()),
                            source,
                            ir::Arg::raw("&"),
                            ir::Arg::int(1),
                        ],
                    ));
                    source = ir::Arg::slot(carry.clone());
                }
                if shift > 0 {
                    self.emit(ir::Instruction::new(
                        "IntOp",
                        vec![
                            ir::Arg::dest(carry.clone()),
                            source,
                            ir::Arg::raw("<<"),
                            ir::Arg::int(i64::from(shift)),
                        ],
                    ));
                    source = ir::Arg::slot(carry.clone());
                }
                self.emit(ir::Instruction::new(
                    "IntOp",
                    vec![
                        ir::Arg::dest(flags.clone()),
                        ir::Arg::slot(flags.clone()),
                        ir::Arg::raw("|"),
                        source,
                    ],
                ));
            }
        }

        self.emit(ir::Instruction::new(
            "SectionSetFlags",
            vec![index.clone(), ir::Arg::slot(flags)],
        ));
    }

    /// `flags &= ~bit`. NSIS's `~` is unary and takes a whole line of its own,
    /// which is why clearing a bit costs two instructions and setting one costs
    /// one.
    fn clear(&mut self, flags: &Slot, bit: i64, span: Span) {
        let mask = self.claim_temp(span);
        self.emit(ir::Instruction::new(
            "IntOp",
            vec![
                ir::Arg::dest(mask.clone()),
                ir::Arg::int(bit),
                ir::Arg::raw("~"),
            ],
        ));
        self.emit(ir::Instruction::new(
            "IntOp",
            vec![
                ir::Arg::dest(flags.clone()),
                ir::Arg::slot(flags.clone()),
                ir::Arg::raw("&"),
                ir::Arg::slot(mask),
            ],
        ));
    }

    /// `{ "Full", "Minimal" }` as the bit field `SectionSetInstTypes` reads.
    ///
    /// Compile-time, and it has to be: the names are the block's declaration and
    /// the positions exist in exactly one place, so a runtime list would be a
    /// second numbering to keep in step with the first (§13).
    fn inst_type_mask(&mut self, value: &Expr) -> Option<i64> {
        let Expr::Table { fields, .. } = value else {
            self.diags.push(
                Diagnostic::error(
                    Code::BadFieldValue,
                    value.span(),
                    "`installTypes` wants a list of names",
                )
                .note(
                    "write `installTypes = { \"Full\" }`, naming types the block declares; the \
                     positions are the compiler's (§13)",
                ),
            );
            return None;
        };

        let mut mask = 0i64;
        for field in fields {
            let TableField::Positional { value } = field else {
                self.todo(value.span(), "a named entry in `installTypes`");
                continue;
            };
            if let Some(position) = self.inst_type_position(value, "installTypes") {
                mask |= 1 << position;
            }
        }
        Some(mask)
    }
}
