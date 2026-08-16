//! Expressions, and the one technique that decides what the output reads like.
//!
//! NSIS has no test-then-jump instruction: every branch is compare-and-jump —
//! `StrCmp a b eq ne`, `IntCmp a b eq lt gt`, `IfFileExists f then else`. So a
//! condition is never *evaluated*; it is **fused** into the branch that consumes
//! it. [`BodyLowerer::branch`] recurses on `and`/`or` and handles `not` by
//! swapping its two destinations, and
//!
//! ```lua
//! if a and (b or not c) then
//! ```
//!
//! becomes three compare-and-jumps and **zero temporaries**. That is Phase 2's
//! exit criterion, and it is asserted as a property of the IR rather than
//! inferred from the absence of a `StrCpy` in the output text — because a
//! `contains` check passes on output carrying one spurious line, which is the
//! failure mode this is trying to avoid (§14).
//!
//! Materialising a boolean instead would burn registers there are only twenty
//! of, which is nsL's issue #5. The same recursion serves all three callers
//! §8 names: comparisons, predicates (§15.20) and — once it lands —
//! `messageBox` (§15.18).

use crate::ast::*;
use crate::builtins::{self, Kind};
use crate::cfg::{self, BlockId, CmpOp, Terminator, Test};
use crate::diag::{Code, Diagnostic, Span};
use crate::ir;
use crate::regs::Slot;
use crate::resolve::ConstValue;
use crate::types::{Sign, Ty};

use super::{Binding, BodyLowerer};

/// A value and what the lattice knows about it.
pub(super) struct Typed {
    pub arg: ir::Arg,
    pub ty: Ty,
}

impl BodyLowerer<'_, '_> {
    // -- values -----------------------------------------------------------

    /// An expression that needs no instruction: a folded constant, a variable
    /// read, or a concatenation of those.
    ///
    /// Concatenation being free is the string model paying for itself (§5).
    /// `"into " .. INSTDIR .. "/bin"` is one quoted argument and no `StrCpy` at
    /// all, because a literal is *data* and an NSIS variable spliced into it is
    /// the one `$` that survives unescaped (§15.1).
    pub(super) fn simple(&mut self, expr: &Expr) -> Option<Typed> {
        if let Some(folded) = self.constant(expr) {
            return Some(Typed {
                ty: folded.ty(),
                arg: ir::Arg::str(folded.text()),
            });
        }

        match expr {
            Expr::Name(name) => match self.lookup(&name.text) {
                Some(Binding::Local { slot, ty }) => Some(Typed {
                    arg: ir::Arg::slot(slot.clone()),
                    ty: *ty,
                }),
                Some(Binding::Const(value)) => Some(Typed {
                    ty: value.ty(),
                    arg: ir::Arg::str(value.text()),
                }),
                None => {
                    if let Some(constant) = builtins::constant_named(&name.text) {
                        return Some(Typed {
                            arg: ir::Arg::var(format!("${}", constant.nsis)),
                            ty: constant.ty,
                        });
                    }
                    if self.resolved.global(&name.text) {
                        let ty = self
                            .globals
                            .get(&name.text)
                            .map(|(ty, _)| *ty)
                            .unwrap_or(Ty::Unknown);
                        return Some(Typed {
                            arg: ir::Arg::slot(Slot::Global(name.text.clone())),
                            ty,
                        });
                    }
                    None
                }
            },

            Expr::Binary {
                op: BinOp::Concat,
                lhs,
                rhs,
                ..
            } => {
                let lhs = self.simple(lhs)?;
                let rhs = self.simple(rhs)?;
                Some(Typed {
                    arg: lhs.arg.concat(rhs.arg),
                    ty: Ty::Str,
                })
            }

            _ => None,
        }
    }

    /// Any expression, as a value. Falls back to a temporary, and every
    /// temporary is a line of output a fused condition would not have needed.
    pub(super) fn value(&mut self, expr: &Expr) -> Option<Typed> {
        if let Some(simple) = self.simple(expr) {
            return Some(simple);
        }

        // Concatenation never needs a destination, even when a side does: the
        // pieces of a template are assembled in the argument, so only the side
        // that computes something spends a register (§5).
        if let Expr::Binary {
            op: BinOp::Concat,
            lhs,
            rhs,
            ..
        } = expr
        {
            let lhs = self.value(lhs)?;
            let rhs = self.value(rhs)?;
            return Some(Typed {
                arg: lhs.arg.concat(rhs.arg),
                ty: Ty::Str,
            });
        }

        let slot = self.claim_temp(expr.span());
        let ty = self.value_into(expr, &slot)?;
        Some(Typed {
            arg: ir::Arg::slot(slot),
            ty,
        })
    }

    /// Lowering takes a **destination**, not just a return value (§12).
    ///
    /// `local sum = 1 + 1` is `IntOp $0 1 + 1`, not `IntOp $R9 1 + 1` followed
    /// by `StrCpy $0 $R9`. Written the other way round it emits a redundant
    /// `StrCpy` for every assignment in the program, and no peephole pass buys
    /// that back cheaply.
    pub(super) fn value_into(&mut self, expr: &Expr, dest: &Slot) -> Option<Ty> {
        if let Some(simple) = self.simple(expr) {
            self.emit(ir::Instruction::new(
                "StrCpy",
                vec![ir::Arg::dest(dest.clone()), simple.arg],
            ));
            return Some(simple.ty);
        }

        match expr {
            Expr::Call { .. } | Expr::MethodCall { .. } => self.call(expr, Some(dest)),

            Expr::Binary { op, lhs, rhs, span } => match op {
                BinOp::Concat => {
                    let value = self.value(expr)?;
                    self.emit(ir::Instruction::new(
                        "StrCpy",
                        vec![ir::Arg::dest(dest.clone()), value.arg],
                    ));
                    Some(value.ty)
                }
                BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                    self.materialise(expr, dest, *span)
                }
                BinOp::And | BinOp::Or => self.logical_value(expr, *op, lhs, rhs, dest, *span),
                arithmetic => self.int_op(*arithmetic, lhs, rhs, dest, *span),
            },

            Expr::Unary { op, operand, span } => match op {
                UnOp::Not => self.materialise(expr, dest, *span),
                UnOp::Neg => {
                    let operand = self.value(operand)?;
                    self.require_int(&operand, *span)?;
                    self.emit(ir::Instruction::new(
                        "IntOp",
                        vec![
                            ir::Arg::dest(dest.clone()),
                            ir::Arg::int(0),
                            ir::Arg::raw("-"),
                            operand.arg,
                        ],
                    ));
                    Some(Ty::int())
                }
                UnOp::BitNot => {
                    let operand = self.value(operand)?;
                    self.require_int(&operand, *span)?;
                    // `IntOp` takes one operand for `~`, unlike every other
                    // opcode it accepts.
                    self.emit(ir::Instruction::new(
                        "IntOp",
                        vec![ir::Arg::dest(dest.clone()), operand.arg, ir::Arg::raw("~")],
                    ));
                    Some(Ty::int())
                }
            },

            // A bare name that `simple` could not resolve is not a shape this
            // version lacks — it is a name that exists nowhere in the file,
            // which resolution being order-free is what makes worth saying.
            Expr::Name(name) => {
                self.undefined(name);
                None
            }

            other => {
                self.todo(other.span(), "this expression");
                None
            }
        }
    }

    /// `and`/`or` producing a *value* rather than a branch.
    ///
    /// Legal for `bool` and rejected for everything else, and the rejected case
    /// is the painful one: `local dir = customDir or PROGRAMFILES .. [[\App]]`
    /// is the idiom every Lua programmer reaches for. Under Lua's semantics it
    /// is dead code — `customDir` is a register, always holds a string, never
    /// `nil`, so `or` always takes the left branch. Under the meaning the user
    /// intends, `""` is falsy, which contradicts Lua on the value a reader is
    /// most likely to test. There is no third option (§15.20).
    fn logical_value(
        &mut self,
        whole: &Expr,
        op: BinOp,
        lhs: &Expr,
        rhs: &Expr,
        dest: &Slot,
        span: Span,
    ) -> Option<Ty> {
        for side in [lhs, rhs] {
            let ty = self.simple(side).map(|typed| typed.ty);
            if let Some(ty) = ty
                && ty != Ty::Bool
                && ty != Ty::Unknown
            {
                let spelling = if op == BinOp::Or { "or" } else { "and" };
                self.diags.push(
                    Diagnostic::error(
                        Code::OrAsValue,
                        side.span(),
                        format!("`{spelling}` needs a `bool`, and this is a {ty}"),
                    )
                    .note(
                        "in Lua only `nil` and `false` are falsy, so this would always take the \
                         left branch; write the comparison you mean, such as `x ~= \"\"` (§15.20)",
                    ),
                );
                return None;
            }
        }
        self.materialise(whole, dest, span)
    }

    /// A boolean that has to survive its fusion — `local exists = fileExists(p)`
    /// — which is what makes `bool` a runtime type rather than a compile-time
    /// one (§15.20).
    ///
    /// It falls out of the CFG rather than needing a shape of its own: branch
    /// to two blocks, each writing a literal, both joining.
    fn materialise(&mut self, expr: &Expr, dest: &Slot, span: Span) -> Option<Ty> {
        let n = self.body.construct();
        let yes = self.fresh(format!("true_{n}"));
        let no = self.fresh(format!("false_{n}"));
        let end = self.fresh(format!("bool_{n}"));

        self.branch(expr, yes, no, span);

        self.current = yes;
        self.emit(ir::Instruction::new(
            "StrCpy",
            vec![ir::Arg::dest(dest.clone()), ir::Arg::str(cfg::TRUE)],
        ));
        self.terminate(Terminator::Jump(end), no);

        self.emit(ir::Instruction::new(
            "StrCpy",
            vec![ir::Arg::dest(dest.clone()), ir::Arg::str(cfg::FALSE)],
        ));
        self.terminate(Terminator::Jump(end), end);

        Some(Ty::Bool)
    }

    // -- arithmetic -------------------------------------------------------

    fn int_op(&mut self, op: BinOp, lhs: &Expr, rhs: &Expr, dest: &Slot, span: Span) -> Option<Ty> {
        let lhs = self.value(lhs)?;
        let rhs = self.value(rhs)?;
        self.require_int(&lhs, span)?;
        self.require_int(&rhs, span)?;

        let sign = result_sign(op, &lhs.ty, &rhs.ty);
        let opcode = match op {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::FloorDiv => "/",
            BinOp::Mod => "%",
            BinOp::BitAnd => "&",
            BinOp::BitOr => "|",
            // Lua's `~` is NSIS's `^`: the spellings swap, which is precisely
            // why `^` is rejected at the operator rather than mapped (§6).
            BinOp::BitXor => "^",
            BinOp::Shl => "<<",
            // Lua's `>>` zero-fills, which is NSIS's `>>>`.
            BinOp::Shr => ">>>",
            other => {
                self.todo(span, &format!("the `{other:?}` operator"));
                return None;
            }
        };

        // NSIS truncates toward zero and Lua floors, so `//` and `%` disagree
        // with Lua whenever exactly one operand is negative (§15.4). When the
        // lattice says neither can be, the fixup is not merely elided — it is
        // provably unnecessary, which is what §15.14's sign axis buys.
        let needs_fixup = matches!(op, BinOp::FloorDiv | BinOp::Mod) && sign == Sign::Unknown;

        if !needs_fixup {
            self.emit(ir::Instruction::new(
                "IntOp",
                vec![
                    ir::Arg::dest(dest.clone()),
                    lhs.arg,
                    ir::Arg::raw(opcode),
                    rhs.arg,
                ],
            ));
            return Some(Ty::Int(crate::types::Int {
                width: crate::types::Width::W32,
                sign,
            }));
        }

        self.signed_division(op, lhs.arg, rhs.arg, dest, span)
    }

    /// The §15.4 fixup, as a CFG rather than as a formula.
    ///
    /// `a // b` and `a % b` are computed into a scratch register and copied out
    /// at the end rather than written straight to `dest`, because `dest` may be
    /// one of the operands — `x = x // y` is ordinary — and clobbering it before
    /// the sign test reads it is a bug that only shows up on negative input.
    fn signed_division(
        &mut self,
        op: BinOp,
        lhs: ir::Arg,
        rhs: ir::Arg,
        dest: &Slot,
        span: Span,
    ) -> Option<Ty> {
        let result = self.claim_temp(span);
        let scratch = self.claim_temp(span);
        let (result_arg, scratch_arg) = (
            ir::Arg::slot(result.clone()),
            ir::Arg::slot(scratch.clone()),
        );

        let opcode = if op == BinOp::FloorDiv { "/" } else { "%" };
        self.emit(ir::Instruction::new(
            "IntOp",
            vec![
                ir::Arg::dest(result.clone()),
                lhs.clone(),
                ir::Arg::raw(opcode),
                rhs.clone(),
            ],
        ));
        // The remainder decides both questions: whether there is anything to
        // fix, and — through its sign — whether the operands disagreed.
        self.emit(ir::Instruction::new(
            "IntOp",
            vec![
                ir::Arg::dest(scratch.clone()),
                lhs,
                ir::Arg::raw("%"),
                rhs.clone(),
            ],
        ));

        let n = self.body.construct();
        let signs = self.fresh(format!("div_{n}_signs"));
        let adjust = self.fresh(format!("div_{n}_adjust"));
        let done = self.fresh(format!("div_{n}_done"));

        self.terminate(
            Terminator::Branch {
                test: Test::Int {
                    op: CmpOp::Eq,
                    lhs: scratch_arg.clone(),
                    rhs: ir::Arg::int(0),
                    family: cfg::IntFamily::Int,
                },
                then_block: done,
                else_block: signs,
            },
            signs,
        );

        // Exclusive-or of the two operands has its sign bit set exactly when
        // they disagree, which is one instruction where a pair of comparisons
        // would be four.
        self.emit(ir::Instruction::new(
            "IntOp",
            vec![
                ir::Arg::dest(scratch.clone()),
                scratch_arg.clone(),
                ir::Arg::raw("^"),
                rhs.clone(),
            ],
        ));
        self.terminate(
            Terminator::Branch {
                test: Test::Int {
                    op: CmpOp::Lt,
                    lhs: scratch_arg,
                    rhs: ir::Arg::int(0),
                    family: cfg::IntFamily::Int,
                },
                then_block: adjust,
                else_block: done,
            },
            adjust,
        );

        // Flooring subtracts one from the quotient; Lua's remainder adds the
        // divisor back.
        let (operator, operand) = if op == BinOp::FloorDiv {
            ("-", ir::Arg::int(1))
        } else {
            ("+", rhs)
        };
        self.emit(ir::Instruction::new(
            "IntOp",
            vec![
                ir::Arg::dest(result.clone()),
                result_arg.clone(),
                ir::Arg::raw(operator),
                operand,
            ],
        ));
        self.terminate(Terminator::Jump(done), done);

        self.emit(ir::Instruction::new(
            "StrCpy",
            vec![ir::Arg::dest(dest.clone()), result_arg],
        ));
        Some(Ty::int())
    }

    fn require_int(&mut self, value: &Typed, span: Span) -> Option<()> {
        if value.ty.is_int() {
            return Some(());
        }
        self.diags.push(
            Diagnostic::error(
                Code::TypeMismatch,
                span,
                format!("arithmetic needs an int, and this is a {}", value.ty),
            )
            .note("there is no coercion: NSIS's `IntOp` reads whatever is there as a number (§6)"),
        );
        None
    }

    // -- calls ------------------------------------------------------------

    pub(super) fn call(&mut self, call: &Expr, dest: Option<&Slot>) -> Option<Ty> {
        let (name, args, span) = match call {
            Expr::Call { callee, args, span } => (callee_path(callee)?, args, *span),
            other => {
                self.todo(other.span(), "this call");
                return None;
            }
        };

        if self.resolved.functions.contains_key(&name) {
            let dests: Vec<Slot> = dest.cloned().into_iter().collect();
            // Lua adjusts a call in single-value position to one value, so a
            // `func` returning two used as `local x = f()` keeps the first and
            // drops the second — which still has to come off the stack.
            let types = self.call_function(&name, args, &dests, span)?;
            return types.first().copied();
        }

        let Some(builtin) = builtins::lookup(&name) else {
            match call {
                Expr::Call { callee, .. } => match callee.as_ref() {
                    Expr::Name(name) => self.undefined(name),
                    other => self.todo(other.span(), "this call"),
                },
                _ => unreachable!(),
            }
            return None;
        };

        if args.len() != builtin.params.len() {
            self.diags.push(
                Diagnostic::error(
                    Code::WrongArity,
                    span,
                    format!(
                        "`{}` takes {} argument(s), and {} were given",
                        builtin.installua,
                        builtin.params.len(),
                        args.len()
                    ),
                )
                .note(format!("it becomes `{}` (§6)", builtin.nsis)),
            );
            return None;
        }

        let mut lowered = Vec::with_capacity(args.len());
        for (argument, param) in args.iter().zip(builtin.params) {
            let value = self.value(argument)?;
            if param.ty != Ty::Unknown && value.ty != param.ty && value.ty != Ty::Unknown {
                self.diags.push(
                    Diagnostic::error(
                        Code::TypeMismatch,
                        argument.span(),
                        format!(
                            "`{}` wants a {}, and this is a {}",
                            builtin.installua, param.ty, value.ty
                        ),
                    )
                    .note(
                        "types come from the instruction table, never from an annotation (§15.14)",
                    ),
                );
                return None;
            }
            // Pathness is decided at the parameter, so the expression lowerer
            // never has to know where its result is going (§15.23).
            lowered.push(if param.path {
                value.arg.into_path()
            } else {
                value.arg
            });
        }

        match builtin.kind {
            Kind::Predicate => match dest {
                Some(dest) => self.materialise(call, dest, span),
                None => {
                    self.diags.push(
                        Diagnostic::error(
                            Code::NotYetImplemented,
                            span,
                            format!(
                                "`{}` answers a question that nothing reads",
                                builtin.installua
                            ),
                        )
                        .note(format!(
                            "write `local answer = {}(…)`, or use it directly in an `if`",
                            builtin.installua
                        ))
                        .note(
                            "it is never optimised away — `IfErrors` clears the flag it reads, \
                             so the call is the side effect (§15.20)",
                        ),
                    );
                    None
                }
            },

            Kind::Instruction => match (builtin.returns, dest) {
                (Some(ty), Some(dest)) => {
                    let mut all = vec![ir::Arg::dest(dest.clone())];
                    all.extend(lowered);
                    self.emit(ir::Instruction::new(builtin.nsis, all));
                    Some(ty)
                }
                (Some(ty), None) => {
                    // The instruction has an output register whether or not
                    // anybody wanted one, so it gets a temporary rather than a
                    // guess at which register is spare.
                    let slot = self.claim_temp(span);
                    let mut all = vec![ir::Arg::dest(slot)];
                    all.extend(lowered);
                    self.emit(ir::Instruction::new(builtin.nsis, all));
                    Some(ty)
                }
                (None, Some(_)) => {
                    self.diags.push(
                        Diagnostic::error(
                            Code::TypeMismatch,
                            span,
                            format!("`{}` produces no value", builtin.installua),
                        )
                        .note(format!("`{}` writes to no register (§15.23)", builtin.nsis)),
                    );
                    None
                }
                (None, None) => {
                    self.emit(ir::Instruction::new(builtin.nsis, lowered));
                    None
                }
            },
        }
    }

    /// `local a, b = f(x)`. Several names, one call.
    pub(super) fn call_multi(&mut self, call: &Expr, dests: &[Slot]) -> Option<Vec<Ty>> {
        let Expr::Call { callee, args, span } = call else {
            self.todo(call.span(), "this call");
            return None;
        };
        let name = callee_path(callee)?;
        if !self.resolved.functions.contains_key(&name) {
            // A builtin with several outputs is a header macro — `${GetSize}`
            // writes three — and those arrive with the overlay (§15.23).
            self.todo(*span, "binding several values from this call");
            return None;
        }
        self.call_function(&name, args, dests, *span)
    }

    /// A call to a `func`, which is the only thing in the language with a
    /// calling convention (§15.11, §11 program 4).
    ///
    /// The three steps here are all the lowerer knows: reserve the site, mark
    /// where the saves go, then evaluate arguments *after* that mark. What the
    /// saves are is not knowable until registers exist, and that is
    /// [`crate::alloc`]'s job.
    fn call_function(
        &mut self,
        name: &str,
        args: &[Expr],
        dests: &[Slot],
        span: Span,
    ) -> Option<Vec<Ty>> {
        let params = self.resolved.functions.get(name).map(|f| f.params.len())?;
        if args.len() != params {
            self.diags.push(
                Diagnostic::error(
                    Code::WrongArity,
                    span,
                    format!(
                        "`{name}` takes {params} argument(s), and {} were given",
                        args.len()
                    ),
                )
                .note("arguments travel on the stack, so a miscount is a stack that unbalances"),
            );
            return None;
        }

        let site = self.body.call_site(name, span);
        let current = self.current;
        self.body.push_step(current, ir::Step::Saves(site));

        let mut lowered = Vec::with_capacity(args.len());
        for (index, argument) in args.iter().enumerate() {
            let value = self.value(argument)?;
            // What a parameter's type is comes from here — there are no
            // annotations, so the call sites are the only evidence (§15.14).
            self.learned.learn_param(name, index, value.ty);
            lowered.push(value.arg);
        }

        let signature = self.known.signature(name).cloned().unwrap_or_default();
        // In the first round nothing has been observed returning anything, so
        // the binding's own count stands in. A real disagreement is reported
        // once the table has settled.
        let arity = signature.arity().unwrap_or(dests.len());
        if dests.len() > arity {
            self.diags.push(
                Diagnostic::error(
                    Code::WrongArity,
                    span,
                    format!(
                        "`{name}` returns {arity} value(s), and {} are being bound",
                        dests.len()
                    ),
                )
                .note("there is no `nil` to pad with (§3)"),
            );
            return None;
        }

        let mut results = Vec::with_capacity(arity);
        let mut types = Vec::with_capacity(arity);
        for index in 0..arity {
            results.push(match dests.get(index) {
                Some(slot) => slot.clone(),
                // A dropped return still needs a slot: the callee pushed it
                // either way, so it has to come off (§11, program 4).
                None => self.body.vreg(span),
            });
            types.push(signature.result(index));
        }

        self.body.calls[site].args = lowered;
        self.body.calls[site].results = results;
        // Argument lowering can have opened new blocks — a `bool` argument
        // materialises into two — so the call goes wherever lowering is *now*.
        let current = self.current;
        self.body.push_step(current, ir::Step::Call(site));
        Some(types)
    }

    // -- condition fusion -------------------------------------------------

    /// Terminates the current block with a branch to `then_b` or `else_b`.
    ///
    /// The caller decides where lowering continues, because only the caller
    /// knows: an `if` continues in its then-arm and a `while` in its body.
    pub(super) fn branch(&mut self, cond: &Expr, then_b: BlockId, else_b: BlockId, span: Span) {
        // A `<const>` condition never becomes a branch at all (§7-2).
        if let Some(ConstValue::Bool(taken)) = self.constant(cond) {
            let target = if taken { then_b } else { else_b };
            let current = self.current;
            self.body.terminate(current, Terminator::Jump(target));
            return;
        }

        match cond {
            Expr::Binary {
                op: op @ (BinOp::And | BinOp::Or),
                lhs,
                rhs,
                ..
            } => {
                let n = self.body.construct();
                let mid = self.fresh(format!(
                    "{}_{n}",
                    if *op == BinOp::And { "and" } else { "or" }
                ));
                if *op == BinOp::And {
                    self.branch(lhs, mid, else_b, span);
                } else {
                    self.branch(lhs, then_b, mid, span);
                }
                self.current = mid;
                self.branch(rhs, then_b, else_b, span);
            }

            // `not` costs nothing at all: it swaps the destinations.
            Expr::Unary {
                op: UnOp::Not,
                operand,
                ..
            } => self.branch(operand, else_b, then_b, span),

            Expr::Binary {
                op: op @ (BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge),
                lhs,
                rhs,
                span,
            } => self.compare(*op, lhs, rhs, then_b, else_b, *span),

            // A predicate fuses directly into its branching instruction and
            // spends no register — the case §15.20 dissolved a whole second
            // condition shape to reach.
            Expr::Call { callee, args, span } => {
                let predicate = callee_path(callee)
                    .and_then(|name| builtins::lookup(&name))
                    .filter(|builtin| builtin.kind == Kind::Predicate);
                match predicate {
                    Some(builtin) if args.len() == builtin.params.len() => {
                        let mut lowered = Vec::with_capacity(args.len());
                        for (argument, param) in args.iter().zip(builtin.params) {
                            let Some(value) = self.value(argument) else {
                                return;
                            };
                            lowered.push(if param.path {
                                value.arg.into_path()
                            } else {
                                value.arg
                            });
                        }
                        let current = self.current;
                        self.body.terminate(
                            current,
                            Terminator::Branch {
                                test: Test::Predicate {
                                    name: builtin.nsis.to_string(),
                                    args: lowered,
                                },
                                then_block: then_b,
                                else_block: else_b,
                            },
                        );
                    }
                    _ => self.branch_on_value(cond, then_b, else_b, *span),
                }
            }

            other => self.branch_on_value(other, then_b, else_b, span),
        }
    }

    /// The leaf case: something that has to hold a `bool`.
    ///
    /// By-type truthiness — `int` → `~= 0`, `string` → `~= ""` — is rejected
    /// rather than deferred, because it contradicts Lua on `0` and `""`, the
    /// two values a reader is most likely to test, and `lua-language-server`
    /// reports nothing either way (§15.20).
    fn branch_on_value(&mut self, cond: &Expr, then_b: BlockId, else_b: BlockId, span: Span) {
        let Some(value) = self.value(cond) else {
            return;
        };
        if value.ty != Ty::Bool {
            let replacement = match value.ty {
                Ty::Str => "compare it: `x ~= \"\"`",
                Ty::Unknown => "give it a type the compiler can see",
                _ => "compare it: `x ~= 0`",
            };
            self.diags.push(
                Diagnostic::error(
                    Code::NotBool,
                    span,
                    format!("a condition needs a `bool`, and this is a {}", value.ty),
                )
                .note(format!("{replacement} (§15.20)"))
                .note(
                    "in Lua every value but `nil` and `false` is truthy, so `if count then` \
                     would run on `0` — a by-type rule would disagree with Lua on exactly the \
                     value most likely to be tested",
                ),
            );
            return;
        }

        let current = self.current;
        self.body.terminate(
            current,
            Terminator::Branch {
                test: Test::boolean(value.arg),
                then_block: then_b,
                else_block: else_b,
            },
        );
    }

    /// A comparison. The type lattice, not the operator, decides which
    /// instruction this becomes — which is why §15.14 is not deferrable past
    /// the first `if` (§12).
    fn compare(
        &mut self,
        op: BinOp,
        lhs: &Expr,
        rhs: &Expr,
        then_b: BlockId,
        else_b: BlockId,
        span: Span,
    ) {
        let (Some(lhs), Some(rhs)) = (self.value(lhs), self.value(rhs)) else {
            return;
        };

        let cmp = match op {
            BinOp::Eq => CmpOp::Eq,
            BinOp::Ne => CmpOp::Ne,
            BinOp::Lt => CmpOp::Lt,
            BinOp::Le => CmpOp::Le,
            BinOp::Gt => CmpOp::Gt,
            BinOp::Ge => CmpOp::Ge,
            _ => unreachable!("only comparisons reach here"),
        };

        let test = match (lhs.ty, rhs.ty) {
            (a, b) if a.is_int() && b.is_int() => Test::Int {
                op: cmp,
                lhs: lhs.arg,
                rhs: rhs.arg,
                // The unsigned member is used only when *both* operands are
                // known non-negative, which the join computes. Guessing the
                // other way makes `-1 < 0` false.
                family: cfg::IntFamily::of(a.join(b)),
            },

            (a, b) if a == b && a != Ty::Unknown => {
                if !matches!(cmp, CmpOp::Eq | CmpOp::Ne) {
                    self.diags.push(
                        Diagnostic::error(
                            Code::TypeMismatch,
                            span,
                            format!("a {a} has no ordering"),
                        )
                        .note(
                            "`StrCmp` has an equal arm and a not-equal arm and nothing else, so \
                             there is no instruction to lower this to (§15.14)",
                        )
                        .note("compare `string.len` if length is what is meant"),
                    );
                    return;
                }
                Test::Str {
                    lhs: lhs.arg,
                    rhs: rhs.arg,
                    // `==` is `StrCmpS`. Case-sensitive being the default is the
                    // reversal from NSIS habit that will bite hardest, and
                    // `string.lower(a) == string.lower(b)` is the escape (§15.9).
                    case_sensitive: true,
                    negate: cmp == CmpOp::Ne,
                }
            }

            (a, b) => {
                self.diags.push(
                    Diagnostic::error(
                        Code::TypeMismatch,
                        span,
                        format!("this compares a {a} with a {b}"),
                    )
                    .note(
                        "defaulting to `StrCmp` would make `\"10\" < \"9\"` true and `10 < 9` \
                         false, and NSIS objects to neither (§15.14)",
                    ),
                );
                return;
            }
        };

        let current = self.current;
        self.body.terminate(
            current,
            Terminator::Branch {
                test,
                then_block: then_b,
                else_block: else_b,
            },
        );
    }
}

/// The dotted name a callee spells: `detailPrint`, `string.len`. Three
/// namespaces exist and NSIS enforces the boundary between them (§13), so the
/// path is the key rather than the last segment.
fn callee_path(callee: &Expr) -> Option<String> {
    match callee {
        Expr::Name(name) => Some(name.text.clone()),
        Expr::Field { base, name, .. } => match base.as_ref() {
            Expr::Name(base) => Some(format!("{}.{}", base.text, name.text)),
            _ => None,
        },
        _ => None,
    }
}

/// What the sign lattice knows about a result. Cheap propagation answers "can
/// this be negative" far more often than a declaration would, which is what
/// turns §15.4's fixup elision from occasional into routine (§15.14).
fn result_sign(op: BinOp, lhs: &Ty, rhs: &Ty) -> Sign {
    let both_nonneg = matches!(
        (lhs.as_int().map(|i| i.sign), rhs.as_int().map(|i| i.sign)),
        (Some(Sign::NonNeg), Some(Sign::NonNeg))
    );
    match op {
        // Subtraction is the one arithmetic operator that manufactures a
        // negative out of two non-negatives.
        BinOp::Sub => Sign::Unknown,
        _ if both_nonneg => Sign::NonNeg,
        _ => Sign::Unknown,
    }
}
