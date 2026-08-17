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
use crate::builtins;
use crate::cfg::{self, BlockId, CmpOp, Terminator, Test};
use crate::diag::{Code, Diagnostic, Span};
use crate::ir;
use crate::regs::Slot;
use crate::resolve::ConstValue;
use crate::table;
use crate::types::{Sign, Ty};

use super::{Binding, BodyLowerer};

/// What a caller wrote for one instruction, sorted into the two things an NSIS
/// line is made of.
///
/// They are two because a flag is not a position: `Delete [/REBOOTOK] filespec`
/// has one argument whatever the caller writes, and `/REBOOTOK` goes in front
/// of it. Keeping them apart until [`place`] is what lets the surface be one
/// unordered options table (§15.23).
pub(super) struct Written {
    /// One entry per surface position, in table order: empty where nobody
    /// filled it, and several for the one repeated tail a row may have.
    inputs: Vec<Vec<ir::Arg>>,
    /// One entry per flag `-CMDHELP` prints for the row, in its order: whether
    /// this call writes it.
    flags: Vec<bool>,
}

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
        // A name and a concatenation are looked at structurally *before*
        // folding, because a top-level `<const>` is a `!define` and the output
        // should say `${APP}` rather than the fourth copy of its value (§7-1).
        // Everything else — a literal, arithmetic over constants — folds, since
        // there is no name left to preserve.
        if !matches!(
            expr,
            Expr::Name(_)
                | Expr::Binary {
                    op: BinOp::Concat,
                    ..
                }
        ) && let Some(folded) = self.constant(expr)
        {
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
                            arg: constant.arg(),
                            ty: constant.ty,
                        });
                    }
                    if let Some(constant) = self.resolved.consts.get(&name.text) {
                        return Some(Typed {
                            ty: constant.value.ty(),
                            arg: ir::Arg::constant(&name.text, constant.value.text()),
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
            Expr::MethodCall {
                receiver,
                method,
                args,
                span,
            } => return self.method(receiver, method, args, dest, *span),
            other => {
                self.todo(other.span(), "this call");
                return None;
            }
        };

        // Hand-written lowerings first (§15.21's "kind 2"): a command whose
        // shape is not one row of the table, because the instruction it becomes
        // depends on a type, or because it is a branch wearing an expression's
        // syntax.
        match name.as_str() {
            "writeReg" => return self.write_reg(args, dest, span),
            "messageBox" => return self.message_box(args, dest, span),
            "raw" => return self.raw(args, dest, span),
            "string.sub" | "string.find" | "string.lower" | "string.upper" | "string.format" => {
                return self.string_adapter(&name, args, dest, span);
            }
            _ => {}
        }

        if let Some((base, method)) = name.split_once('.')
            && self.resolved.namespaces.contains_key(base)
        {
            let dests: Vec<Slot> = dest.cloned().into_iter().collect();
            let (base, method) = (base.to_string(), method.to_string());
            let types = self.namespaced(&base, &method, args, &dests, span)?;
            return types.first().copied();
        }

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

        let name = builtin.installua.unwrap_or(builtin.nsis);
        let lowered = self.surface_args(builtin, name, args, 0, span)?;

        match builtin.predicate {
            true => match dest {
                Some(dest) => self.materialise(call, dest, span),
                None => {
                    self.diags.push(
                        Diagnostic::error(
                            Code::NotYetImplemented,
                            span,
                            format!("`{name}` answers a question that nothing reads"),
                        )
                        .note(format!(
                            "write `local answer = {name}(…)`, or use it directly in an `if`"
                        ))
                        .note(
                            "it is never optimised away — `IfErrors` clears the flag it reads, \
                             so the call is the side effect (§15.20)",
                        ),
                    );
                    None
                }
            },

            false => match (builtin.returns(), dest) {
                // One name for a row that may write several registers: Lua
                // adjusts the call to one value, so the rest still have to be
                // written and [`Self::destinations`] finds them somewhere.
                (Some(ty), dest) => {
                    let bound: &[Slot] = match dest {
                        Some(dest) => std::slice::from_ref(dest),
                        None => &[],
                    };
                    let dests = self.destinations(builtin, bound, span);
                    let all = place(builtin, lowered, dests);
                    self.emit(ir::Instruction::new(builtin.nsis, all));
                    Some(ty)
                }
                (None, Some(_)) => {
                    self.diags.push(
                        Diagnostic::error(
                            Code::TypeMismatch,
                            span,
                            format!("`{name}` produces no value"),
                        )
                        .note(format!("`{}` writes to no register (§15.23)", builtin.nsis)),
                    );
                    None
                }
                (None, None) => {
                    let all = place(builtin, lowered, Vec::new());
                    self.emit(ir::Instruction::new(builtin.nsis, all));
                    None
                }
            },
        }
    }

    /// Everything a caller wrote for one instruction, checked and placed
    /// against the surface positions it means.
    ///
    /// **A position is named when counting cannot say which one it is** — see
    /// [`table::Instruction::positional`]. Counting used to decide it
    /// everywhere, and that is what this replaces. It failed outright on a
    /// *leading* optional: `ExecShell [flags] verb file …` bound
    /// `execShell("open", url)`'s `"open"` to `flags`, putting every annotation
    /// one position left of what the author meant. It was already bad wherever
    /// there were several, since setting `createShortcut`'s description meant
    /// writing all nine arguments. It was fine for `abort [message]`, which is
    /// why that one is still `abort("stopped")`.
    ///
    /// The same table holds the row's **flags**, which are named the same way
    /// and are not positions at all: `rmDir(dir, { recursive = true })` writes
    /// `RMDir /r "$INSTDIR"`, and the caller never learns that `/r` goes first.
    ///
    /// The result is one entry per surface position, in table order, holding
    /// what belongs there: empty for a position nobody filled, and several for
    /// the one repeated tail a row may have (`File a b c`). `skip` is how a
    /// method drops its receiver, which is a syntactic prefix here and an
    /// ordinary first parameter in NSIS.
    fn surface_args(
        &mut self,
        builtin: &table::Instruction,
        name: &str,
        args: &[Expr],
        skip: usize,
        span: Span,
    ) -> Option<Written> {
        // The user-facing positions: the inputs, minus the ones the compiler
        // fills. `fileExists(p)` takes one argument where `IfFileExists` takes
        // three, and that difference is §15.20's whole point.
        let surface: Vec<&table::Param> = builtin.surface().skip(skip).collect();
        let named = builtin.takes_options();

        // The options table is the last argument when there is one to be. A row
        // with no optional position has no table, so `f({…})` there stays what
        // it always was: a table where a value was wanted.
        let (given, entries) = match args.split_last() {
            Some((Expr::Table { fields, .. }, rest)) if named => (rest, Some(fields)),
            _ => (args, None),
        };

        // The positions an argument can land in: the required ones, plus the
        // trailing optional when the row has exactly one and nothing about the
        // count is in doubt.
        let tail = builtin.tail_optional().is_some();
        let counted: Vec<(usize, &table::Param)> = surface
            .iter()
            .enumerate()
            .filter(|(_, param)| param.required() || tail)
            .map(|(index, param)| (index, *param))
            .collect();
        let least = counted.iter().filter(|(_, p)| p.required()).count();
        let most = match counted.last() {
            Some((_, param)) if param.shape.rep == table::Rep::Many => usize::MAX,
            _ => counted.len(),
        };
        let arity = least..=most;
        if !arity.contains(&given.len()) {
            let mut diagnostic = Diagnostic::error(
                Code::WrongArity,
                span,
                format!(
                    "`{name}` takes {}, and {} were given",
                    arguments(&arity),
                    given.len()
                ),
            )
            .note(format!("it becomes `{}` (§6)", builtin.nsis));
            if named {
                diagnostic = diagnostic.note(format!(
                    "its optional positions are named rather than counted: {}",
                    options(builtin)
                ));
            }
            self.diags.push(diagnostic);
            return None;
        }

        let mut placed: Vec<Vec<ir::Arg>> = surface.iter().map(|_| Vec::new()).collect();
        for (index, argument) in given.iter().enumerate() {
            // A repeated trailing position — `File a b c` — takes every
            // argument past the last one, with the last one's own type, because
            // that is what the repetition means.
            let (position, param) = *counted.get(index).or_else(|| counted.last())?;
            let lowered = self.coerce(param, name, argument)?;
            placed[position].push(lowered);
        }

        // A flag is written when the call names it and `true`, and on every call
        // when NSIS requires it (`WriteRegMultiStr /REGEDIT5`).
        let mut flags: Vec<bool> = builtin
            .options
            .iter()
            .map(|flag| flag.offer == table::Offer::Always)
            .collect();
        let mut set: Vec<bool> = vec![false; flags.len()];

        for field in entries.into_iter().flatten() {
            let TableField::Named { name: key, value } = field else {
                self.todo(span, "a positional entry in an options table");
                return None;
            };
            let position = surface.iter().position(|param| {
                param
                    .field
                    .is_some_and(|field| field.name == key.text && !param.required())
            });
            let Some(position) = position else {
                // Not a position, so it is a flag or it is nothing. Both halves
                // of the table are reached by name and the caller has no reason
                // to know which half a name is in — which is the point.
                let flag = builtin
                    .options
                    .iter()
                    .position(|flag| flag.name() == Some(key.text.as_str()));
                let Some(flag) = flag else {
                    self.diags.push(
                        Diagnostic::error(
                            Code::UnknownField,
                            key.span,
                            format!("`{name}` has no option `{}`", key.text),
                        )
                        .note(format!("its options are {}", options(builtin))),
                    );
                    return None;
                };
                if set[flag] {
                    self.diags.push(Diagnostic::error(
                        Code::UnknownField,
                        key.span,
                        format!("`{}` is set twice", key.text),
                    ));
                    return None;
                }
                set[flag] = true;
                let nsis = builtin.options[flag].opt.nsis;
                match value {
                    Expr::Bool { value: on, .. } => flags[flag] = *on,
                    other => {
                        self.diags.push(
                            Diagnostic::error(
                                Code::BadFieldValue,
                                other.span(),
                                format!("`{}` is on or off", key.text),
                            )
                            .note(format!(
                                "write `{} = true`, and the compiler writes `{nsis}`",
                                key.text
                            )),
                        );
                        return None;
                    }
                }
                continue;
            };
            if !placed[position].is_empty() {
                self.diags.push(Diagnostic::error(
                    Code::UnknownField,
                    key.span,
                    format!("`{}` is set twice", key.text),
                ));
                return None;
            }
            let param = surface[position];
            let lowered = match param.field.and_then(|field| field.toggle) {
                // A position whose only legal value is a token NSIS spells
                // itself. The field is a `bool` and the compiler writes the
                // token, so there is nothing for the user to look up.
                Some(nsis) => match value {
                    Expr::Bool { value: true, .. } => ir::Arg::raw(nsis),
                    Expr::Bool { value: false, .. } => continue,
                    other => {
                        self.diags.push(
                            Diagnostic::error(
                                Code::BadFieldValue,
                                other.span(),
                                format!("`{}` is on or off", key.text),
                            )
                            .note(format!(
                                "write `{} = true`, and the compiler writes `{nsis}`",
                                key.text
                            )),
                        );
                        return None;
                    }
                },
                None => self.coerce(param, name, value)?,
            };
            placed[position].push(lowered);
        }

        Some(Written {
            inputs: placed,
            flags,
        })
    }

    /// One argument, checked against the position it lands in and converted for
    /// it.
    fn coerce(&mut self, param: &table::Param, name: &str, argument: &Expr) -> Option<ir::Arg> {
        let value = self.value(argument)?;
        // Assignable, not equal. The lattice already says which types
        // subsume which — `a.join(b) == b` is exactly "a fits where b is
        // wanted" — and equality got this wrong in one direction that
        // matters: a literal `0` is `nonneg`, so every `Ty::int()` position
        // rejected every integer literal (§15.14).
        if param.ty != Ty::Unknown && value.ty != Ty::Unknown && value.ty.join(param.ty) != param.ty
        {
            let mut diagnostic = Diagnostic::error(
                Code::TypeMismatch,
                argument.span(),
                format!("`{name}` wants a {}, and this is a {}", param.ty, value.ty),
            )
            .note("types come from the instruction table, never from an annotation (§15.14)");
            // `int` is what a user calls both signs (§15.14), so the
            // message above reads "wants a int, and this is a int" when the
            // sign is the whole disagreement. Say what it will not say.
            if param.ty.is_int() && value.ty.is_int() {
                diagnostic = diagnostic.note(
                    "this position cannot be negative, and the value is not known to be \
                     non-negative",
                );
            }
            self.diags.push(diagnostic);
            return None;
        }
        // Pathness is decided at the parameter, so the expression lowerer
        // never has to know where its result is going (§15.23).
        Some(if param.kind == table::Kind::Path {
            value.arg.into_path()
        } else {
            value.arg
        })
    }

    /// The registers the instruction writes, given the ones a caller asked to
    /// bind. The two are not the same list: NSIS writes every *required* output
    /// whether or not Lua reads it — `GetFileTime` has no one-register spelling
    /// — so an unbound required output takes a temporary rather than a guess at
    /// which register is spare. A trailing *optional* output nobody reads is
    /// not written at all, because a register nobody wanted would still enter
    /// the clobber set and cost a caller a save.
    fn destinations(
        &mut self,
        builtin: &table::Instruction,
        bound: &[Slot],
        span: Span,
    ) -> Vec<ir::Arg> {
        let mut dests = Vec::new();
        for (index, output) in builtin.outputs().enumerate() {
            match bound.get(index) {
                Some(slot) => dests.push(ir::Arg::dest(slot.clone())),
                None if output.required() => dests.push(ir::Arg::dest(self.claim_temp(span))),
                None => break,
            }
        }
        dests
    }

    /// `local high, low = getFileTime(p)`. One call, several outputs.
    ///
    /// The plural is the table's, not the language's: an instruction's output
    /// *count* is its Lua arity (§15.23), so nothing here decides anything a
    /// row has not already said.
    fn builtin_multi(
        &mut self,
        builtin: &'static table::Instruction,
        args: &[Expr],
        dests: &[Slot],
        span: Span,
    ) -> Option<Vec<Ty>> {
        let name = builtin.installua.unwrap_or(builtin.nsis);
        let outputs: Vec<Ty> = match builtin.predicate {
            // A predicate's `bool` comes from its branch rather than from a
            // register, so there is no second value to bind.
            true => Vec::new(),
            false => builtin.outputs().map(|param| param.ty).collect(),
        };
        if dests.len() > outputs.len() {
            self.diags.push(
                Diagnostic::error(
                    Code::WrongArity,
                    span,
                    format!(
                        "`{name}` writes {} register(s), and {} are being bound",
                        outputs.len(),
                        dests.len()
                    ),
                )
                .note(format!(
                    "`{}` is what it becomes, and its outputs are its values (§15.23)",
                    builtin.nsis
                )),
            );
            return None;
        }

        let lowered = self.surface_args(builtin, name, args, 0, span)?;
        let placed = self.destinations(builtin, dests, span);
        let all = place(builtin, lowered, placed);
        self.emit(ir::Instruction::new(builtin.nsis, all));
        Some(outputs[..dests.len()].to_vec())
    }

    /// `f:close()` — a method on a handle.
    ///
    /// The receiver is a syntactic prefix here and an ordinary argument in
    /// NSIS, which is the whole reason [`Expr::MethodCall`] is a variant rather
    /// than a `Field` and a `Call`: `f:write(s)` is `FileWrite $f "s"`, with
    /// the receiver first.
    fn method(
        &mut self,
        receiver: &Expr,
        method: &Name,
        args: &[Expr],
        dest: Option<&Slot>,
        span: Span,
    ) -> Option<Ty> {
        let handle = self.value(receiver)?;
        if handle.ty != Ty::Handle && handle.ty != Ty::Unknown {
            self.diags.push(
                Diagnostic::error(
                    Code::TypeMismatch,
                    receiver.span(),
                    format!("a {} has no methods", handle.ty),
                )
                .note("methods exist on a handle, which comes from `fileOpen`"),
            );
            return None;
        }
        // The table names these `f:close`, `f:read`, `f:write` — it has since
        // Phase 5 — and this used to hold a second copy as a hardcoded match.
        // Two vocabularies for one set is what the join exists to remove, and
        // the copy could not describe a method with an *output* at all, which is
        // what `f:readByte` and `f:seek` are.
        let Some(builtin) = builtins::lookup(&format!("f:{}", method.text)) else {
            self.diags.push(
                Diagnostic::error(
                    Code::UndefinedName,
                    method.span,
                    format!("a handle has no `{}`", method.text),
                )
                .note(format!("the methods are {}", methods())),
            );
            return None;
        };

        // The receiver is the first parameter in NSIS and a syntactic prefix
        // here, so the method's own surface is the instruction's minus it.
        let mut lowered = self.surface_args(builtin, &method.text, args, 1, span)?;
        lowered.inputs.insert(0, vec![handle.arg]);

        match (builtin.returns(), dest) {
            (Some(ty), dest) => {
                let bound: &[Slot] = match dest {
                    Some(dest) => std::slice::from_ref(dest),
                    None => &[],
                };
                let dests = self.destinations(builtin, bound, span);
                let all = place(builtin, lowered, dests);
                self.emit(ir::Instruction::new(builtin.nsis, all));
                Some(ty)
            }
            (None, Some(_)) => {
                self.diags.push(
                    Diagnostic::error(
                        Code::TypeMismatch,
                        span,
                        format!("`{}` produces no value", method.text),
                    )
                    .note("it acts on the file rather than answering a question"),
                );
                None
            }
            (None, None) => {
                let all = place(builtin, lowered, Vec::new());
                self.emit(ir::Instruction::new(builtin.nsis, all));
                None
            }
        }
    }

    // -- namespaces --------------------------------------------------------

    /// `fileFunc.getSize(dir, "")` — a call into a namespace a `local` bound
    /// (§15.27).
    ///
    /// The header half is a macro expansion, and its calling convention is not
    /// this compiler's choice: `!insertmacro` cannot return anything, so a
    /// macro takes its inputs first and writes its outputs into **trailing
    /// register arguments**. That is the whole difference from an instruction,
    /// whose one output comes first.
    pub(super) fn namespaced(
        &mut self,
        base: &str,
        method: &str,
        args: &[Expr],
        dests: &[Slot],
        span: Span,
    ) -> Option<Vec<Ty>> {
        let namespace = self.resolved.namespaces.get(base)?.clone();
        let header = match &namespace {
            crate::resolve::Namespace::Header(header) => header.clone(),
            crate::resolve::Namespace::Plugin(plugin) => {
                let plugin = plugin.clone();
                return self.plugin_call(&plugin, method, args, dests, span);
            }
        };

        let Some(entry) = crate::headers::lookup(&header, method) else {
            let mut diagnostic = Diagnostic::error(
                Code::UndefinedName,
                span,
                format!("`{header}` declares no `{method}`"),
            );
            diagnostic = if crate::headers::known(&header) {
                diagnostic.note(format!(
                    "it declares {}",
                    crate::headers::methods(&header)
                        .iter()
                        .map(|name| format!("`{name}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            } else {
                diagnostic.note(format!(
                    "nothing is declared for `{header}` — a macro's parameter list says nothing \
                     about directions or counts, so the declaration is written rather than \
                     discovered (§15.27)"
                ))
            };
            self.diags.push(diagnostic);
            return None;
        };

        if args.len() != entry.params.len() {
            self.diags.push(
                Diagnostic::error(
                    Code::WrongArity,
                    span,
                    format!(
                        "`{base}.{method}` takes {} argument(s), and {} were given",
                        entry.params.len(),
                        args.len()
                    ),
                )
                .note(format!("it becomes `${{{}}}` (§15.27)", entry.nsis)),
            );
            return None;
        }

        let mut lowered = Vec::with_capacity(entry.params.len() + entry.outputs.len());
        for (argument, param) in args.iter().zip(entry.params) {
            let value = self.value(argument)?;
            if param.ty != Ty::Unknown && value.ty != param.ty && value.ty != Ty::Unknown {
                self.diags.push(
                    Diagnostic::error(
                        Code::TypeMismatch,
                        argument.span(),
                        format!(
                            "`{base}.{method}` wants a {}, and this is a {}",
                            param.ty, value.ty
                        ),
                    )
                    .note("types come from the declaration, never from an annotation (§15.14)"),
                );
                return None;
            }
            lowered.push(if param.path {
                value.arg.into_path()
            } else {
                value.arg
            });
        }

        if dests.len() > entry.outputs.len() {
            self.diags.push(
                Diagnostic::error(
                    Code::WrongArity,
                    span,
                    format!(
                        "`{base}.{method}` writes {} value(s), and {} are being bound",
                        entry.outputs.len(),
                        dests.len()
                    ),
                )
                .note("there is no `nil` to pad with (§3)"),
            );
            return None;
        }

        // Every output gets a register whether or not anybody wanted one: the
        // macro writes all of them, so a slot that is not bound is still
        // written and still has to be a definition liveness can see.
        for index in 0..entry.outputs.len() {
            let slot = match dests.get(index) {
                Some(slot) => slot.clone(),
                None => self.body.vreg(span),
            };
            lowered.push(ir::Arg::dest(slot));
        }

        self.emit(ir::Instruction::new(format!("${{{}}}", entry.nsis), lowered).atomic());
        Some(entry.outputs.to_vec())
    }

    /// `raw [[ … ]]` — the third opaque callee, and the escape hatch.
    ///
    /// The text is emitted verbatim, one line per line, with the leading
    /// whitespace dropped so the block sits where the emitter's indentation
    /// puts everything else. Nothing is hoisted, nothing is checked and no
    /// local survives it: it clobbers every register, which is what makes the
    /// hatch safe to have rather than a hole (§13, §15.11).
    fn raw(&mut self, args: &[Expr], dest: Option<&Slot>, span: Span) -> Option<Ty> {
        if dest.is_some() {
            self.diags.push(
                Diagnostic::error(Code::TypeMismatch, span, "`raw` produces no value").note(
                    "it is text handed to `makensis`, so there is nothing here that knows what it \
                     left in a register — write to a global instead (§13)",
                ),
            );
            return None;
        }
        let [Expr::Str(text)] = args else {
            self.diags.push(
                Diagnostic::error(Code::WrongArity, span, "`raw` takes one literal block").note(
                    "write `raw [[ … ]]`; a computed string would be text this compiler assembled \
                     and did not read",
                ),
            );
            return None;
        };

        let lines: Vec<ir::Instruction> = text
            .value
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(|line| ir::Instruction::new(line, Vec::new()))
            .collect();
        if lines.is_empty() {
            return None;
        }

        let site = self.body.opaque_site(lines, true, span);
        let current = self.current;
        self.body.push_step(current, ir::Step::Saves(site));
        self.body.push_step(current, ir::Step::Call(site));
        None
    }

    /// `nsExec.execToStack(cmd)` — the first of §15.11's three opaque callees.
    ///
    /// A plugin takes its arguments **inline** and leaves its outputs on the
    /// stack, so the shape is one line plus a `Pop` each. What makes it a *call
    /// site* rather than an instruction is the other half: nothing here has
    /// read the DLL, so it clobbers every register, and anything live across it
    /// has to be saved exactly as it would be around a `func`.
    fn plugin_call(
        &mut self,
        plugin: &str,
        method: &str,
        args: &[Expr],
        dests: &[Slot],
        span: Span,
    ) -> Option<Vec<Ty>> {
        let Some(entry) = crate::headers::plugin(plugin, method) else {
            self.diags.push(
                Diagnostic::error(
                    Code::UndefinedName,
                    span,
                    format!("`{plugin}` declares no `{method}`"),
                )
                .note(match crate::headers::plugin_methods(plugin).as_slice() {
                    [] => format!(
                        "nothing is declared for `{plugin}` — a DLL cannot be asked how many \
                         values it pushes, so the count is written down rather than discovered \
                         (§11)"
                    ),
                    methods => format!(
                        "it declares {}",
                        methods
                            .iter()
                            .map(|name| format!("`{name}`"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                }),
            );
            return None;
        };

        if args.len() != entry.params.len() {
            self.diags.push(
                Diagnostic::error(
                    Code::WrongArity,
                    span,
                    format!(
                        "`{plugin}.{method}` takes {} argument(s), and {} were given",
                        entry.params.len(),
                        args.len()
                    ),
                )
                .note(format!("it becomes `{}`", entry.nsis)),
            );
            return None;
        }

        // `System::Call`'s output count is in its signature: every `.s` pushes
        // one value. That is as far as the signature is read — narrowing the
        // clobber set from the rest of it is a later optimisation (PLAN §3).
        let outputs: Vec<Ty> = if entry.nsis == "System::Call" {
            let signature = self.constant(&args[0]).map(|value| value.text());
            let Some(signature) = signature else {
                self.diags.push(
                    Diagnostic::error(
                        Code::BadFieldValue,
                        span,
                        "`System.call` needs a build-time signature",
                    )
                    .note(
                        "the number of values it leaves on the stack is the number of `.s` in the \
                         signature, and a runtime string cannot be counted (§15.11)",
                    ),
                );
                return None;
            };
            vec![Ty::Str; signature.matches(".s").count()]
        } else {
            entry.outputs.to_vec()
        };

        if dests.len() > outputs.len() {
            self.diags.push(
                Diagnostic::error(
                    Code::WrongArity,
                    span,
                    format!(
                        "`{plugin}.{method}` pushes {} value(s), and {} are being bound",
                        outputs.len(),
                        dests.len()
                    ),
                )
                .note("the count comes from the declaration, since nothing can ask the DLL (§11)"),
            );
            return None;
        }

        // The site is reserved and the saves marked *before* the arguments are
        // lowered, for the same reason a `func` call does it: a save has to
        // precede instructions this lowerer has not emitted yet (§15.11).
        let site = self.body.opaque_site(Vec::new(), false, span);
        let current = self.current;
        self.body.push_step(current, ir::Step::Saves(site));

        let mut lowered = Vec::with_capacity(args.len());
        for (argument, param) in args.iter().zip(entry.params) {
            let value = self.value(argument)?;
            lowered.push(if param.path {
                value.arg.into_path()
            } else {
                value.arg
            });
        }

        let results = (0..outputs.len())
            .map(|index| match dests.get(index) {
                Some(slot) => slot.clone(),
                // A dropped output still comes off the stack: the plugin pushed
                // it either way, and leaving it there unbalances everything
                // after it with no diagnostic from NSIS (§3).
                None => self.body.vreg(span),
            })
            .collect();

        self.body.calls[site].kind = ir::CallKind::Opaque {
            lines: vec![ir::Instruction::new(entry.nsis, lowered).at(span)],
            raw: false,
        };
        self.body.calls[site].results = results;
        let current = self.current;
        self.body.push_step(current, ir::Step::Call(site));
        Some(outputs)
    }

    // -- hand-written lowerings -------------------------------------------

    /// `writeReg(root, key, name, value)`.
    ///
    /// One surface name, two instructions: NSIS spells the value's type in the
    /// instruction rather than in the argument, and a `WriteRegStr` holding
    /// digits is not the same registry entry as a `WriteRegDWORD` holding the
    /// same digits — nothing downstream can recover the difference, which is
    /// why the lattice picks it here (§15.14).
    fn write_reg(&mut self, args: &[Expr], dest: Option<&Slot>, span: Span) -> Option<Ty> {
        if dest.is_some() {
            self.diags.push(
                Diagnostic::error(Code::TypeMismatch, span, "`writeReg` produces no value")
                    .note("read it back with `readRegStr` if that is what is meant"),
            );
            return None;
        }
        let [root, key, name, value] = args else {
            self.diags.push(
                Diagnostic::error(
                    Code::WrongArity,
                    span,
                    format!(
                        "`writeReg` takes 4 argument(s), and {} were given",
                        args.len()
                    ),
                )
                .note("write `writeReg(HKLM, key, name, value)`"),
            );
            return None;
        };

        let root = self.value(root)?;
        let key = self.value(key)?;
        let name = self.value(name)?;
        let value = self.value(value)?;

        let nsis = if value.ty.is_int() {
            "WriteRegDWORD"
        } else {
            "WriteRegStr"
        };
        self.emit(ir::Instruction::new(
            nsis,
            vec![root.arg, key.arg.into_path(), name.arg, value.arg],
        ));
        None
    }

    /// The `string.*` adapters (§15.21's "kind 2").
    ///
    /// These are hand-written lowerings rather than table rows, and the reason
    /// is arithmetic: Lua indexes from 1 and NSIS from 0, so every index
    /// crosses a boundary that no declaration can describe. `string.sub(v, 1,
    /// n)` is `StrCpy dest src <n> 0` — a length and an offset where Lua wrote
    /// two positions.
    fn string_adapter(
        &mut self,
        name: &str,
        args: &[Expr],
        dest: Option<&Slot>,
        span: Span,
    ) -> Option<Ty> {
        // An adapter always writes somewhere: `string.upper(x)` as a statement
        // computes a value nobody reads, which is legal and useless, and the
        // register costs nothing once liveness has run.
        let owned;
        let dest = match dest {
            Some(dest) => dest,
            None => {
                owned = self.claim_temp(span);
                &owned
            }
        };

        match name {
            "string.lower" | "string.upper" => {
                let [subject] = args else {
                    return self.wrong_arity(name, 1, args.len(), span);
                };
                let subject = self.value(subject)?;
                // `${StrCase}` is `StrFunc`, and `StrFunc` refuses to work
                // unless the function was declared with `${Using:StrFunc}`
                // first — a missing line aborts the build rather than warning
                // (§15.21).
                self.requires.str_func("StrCase");
                let mode = if name == "string.lower" { "L" } else { "U" };
                self.emit(
                    ir::Instruction::new(
                        "${StrCase}",
                        vec![ir::Arg::dest(dest.clone()), subject.arg, ir::Arg::str(mode)],
                    )
                    .atomic(),
                );
                Some(Ty::Str)
            }

            "string.find" => {
                let [subject, needle] = args else {
                    return self.wrong_arity(name, 2, args.len(), span);
                };
                let subject = self.value(subject)?;
                let needle = self.value(needle)?;
                self.requires.str_func("StrLoc");
                // `${StrLoc}` counts from 0 and Lua counts from 1, so the `+ 1`
                // is the conversion rather than an optimisation nobody did.
                // `""` when the needle is absent, where Lua answers `nil` —
                // there is no `nil`, and the difference is one row of the
                // migration table.
                self.emit(
                    ir::Instruction::new(
                        "${StrLoc}",
                        vec![
                            ir::Arg::dest(dest.clone()),
                            subject.arg,
                            needle.arg,
                            ir::Arg::str(">"),
                        ],
                    )
                    .atomic(),
                );
                self.emit(ir::Instruction::new(
                    "IntOp",
                    vec![
                        ir::Arg::dest(dest.clone()),
                        ir::Arg::slot(dest.clone()),
                        ir::Arg::raw("+"),
                        ir::Arg::int(1),
                    ],
                ));
                Some(Ty::nonneg())
            }

            "string.format" => {
                let [format, value] = args else {
                    return self.wrong_arity(name, 2, args.len(), span);
                };
                let format = self.value(format)?;
                let value = self.value(value)?;
                self.require_int(&value, span)?;
                // `IntFmt` is the whole of `string.format` that NSIS has: one
                // integer, one specifier. `%s` and several arguments are
                // `Class::Todo` rather than a lowering nobody can write.
                self.emit(ir::Instruction::new(
                    "IntFmt",
                    vec![ir::Arg::dest(dest.clone()), format.arg, value.arg],
                ));
                Some(Ty::Str)
            }

            "string.sub" => self.string_sub(args, dest, span),

            _ => unreachable!("dispatched on the same list"),
        }
    }

    /// `string.sub(s, i)` and `string.sub(s, i, j)`.
    ///
    /// `StrCpy dest src <maxlen> <start>` takes a **length and an offset**
    /// where Lua takes two 1-based positions, so `start = i - 1` and `maxlen =
    /// j - i + 1`. Both fold when the indices are constants, which is the case
    /// every program in the five actually writes.
    fn string_sub(&mut self, args: &[Expr], dest: &Slot, span: Span) -> Option<Ty> {
        let (subject, from, to) = match args {
            [subject, from] => (subject, from, None),
            [subject, from, to] => (subject, from, Some(to)),
            _ => return self.wrong_arity("string.sub", 3, args.len(), span),
        };

        for index in [Some(from), to] {
            let Some(index) = index else { continue };
            if let Some(ConstValue::Int(value)) = self.constant(index)
                && value < 0
            {
                self.todo(index.span(), "a negative `string.sub` index");
                return None;
            }
        }

        let subject = self.value(subject)?;
        let from_value = self.value(from)?;
        self.require_int(&from_value, span)?;

        // `i - 1`, folded when `i` is a constant, which is the common case and
        // the one that keeps the output a single line.
        let start = match self.constant(from) {
            Some(ConstValue::Int(value)) => ir::Arg::int(value - 1),
            _ => {
                let slot = self.claim_temp(span);
                self.emit(ir::Instruction::new(
                    "IntOp",
                    vec![
                        ir::Arg::dest(slot.clone()),
                        from_value.arg.clone(),
                        ir::Arg::raw("-"),
                        ir::Arg::int(1),
                    ],
                ));
                ir::Arg::slot(slot)
            }
        };

        // An absent `j` is "to the end", which `StrCpy` spells as an empty
        // length rather than as a number.
        let length = match to {
            None => ir::Arg::str(""),
            Some(to) => {
                let to_value = self.value(to)?;
                self.require_int(&to_value, span)?;
                match (self.constant(from), self.constant(to)) {
                    (Some(ConstValue::Int(from)), Some(ConstValue::Int(to))) => {
                        ir::Arg::int(to - from + 1)
                    }
                    // `maxlen` is `j - (i - 1)`, and `i = 1` — the overwhelming
                    // case, since Lua strings start there — makes the
                    // subtraction `- 0`. Emitting it would be a line whose only
                    // effect is to be read by somebody wondering what it does
                    // (§9-6).
                    (Some(ConstValue::Int(1)), _) => to_value.arg,
                    _ => {
                        let slot = self.claim_temp(span);
                        self.emit(ir::Instruction::new(
                            "IntOp",
                            vec![
                                ir::Arg::dest(slot.clone()),
                                to_value.arg,
                                ir::Arg::raw("-"),
                                start.clone(),
                            ],
                        ));
                        ir::Arg::slot(slot)
                    }
                }
            }
        };

        self.emit(ir::Instruction::new(
            "StrCpy",
            vec![ir::Arg::dest(dest.clone()), subject.arg, length, start],
        ));
        Some(Ty::Str)
    }

    fn wrong_arity(&mut self, name: &str, wanted: usize, got: usize, span: Span) -> Option<Ty> {
        self.diags.push(
            Diagnostic::error(
                Code::WrongArity,
                span,
                format!("`{name}` takes {wanted} argument(s), and {got} were given"),
            )
            .note("the adapter is hand-written, so the count is what NSIS can express (§15.21)"),
        );
        None
    }

    /// `messageBox` (§15.18): an expression whose value is a branch.
    ///
    /// The answer is an ordinary string — `"YES"`, `"NO"` — and the jump table
    /// is recovered from the comparison. A one-button dialog has no table at
    /// all, which is why the short form costs exactly one line.
    fn message_box(&mut self, args: &[Expr], dest: Option<&Slot>, span: Span) -> Option<Ty> {
        let (text, buttons, icon) = self.message_box_fields(args, span)?;
        let Some(set) = BUTTONS.iter().find(|set| set.installua == buttons) else {
            self.diags.push(
                Diagnostic::error(
                    Code::BadFieldValue,
                    span,
                    format!("`{buttons}` is not a button set"),
                )
                .note(format!(
                    "the sets are {}",
                    BUTTONS
                        .iter()
                        .map(|set| format!("`{}`", set.installua))
                        .collect::<Vec<_>>()
                        .join(", ")
                )),
            );
            return None;
        };

        let mut flags = format!("MB_{}", set.installua);
        if let Some(icon) = icon {
            let known = ["EXCLAMATION", "INFORMATION", "QUESTION", "STOP"];
            if !known.contains(&icon.as_str()) {
                self.diags.push(
                    Diagnostic::error(
                        Code::BadFieldValue,
                        span,
                        format!("`{icon}` is not an icon"),
                    )
                    .note(format!(
                        "the icons are {}",
                        known
                            .iter()
                            .map(|name| format!("`{name}`"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )),
                );
                return None;
            }
            flags.push_str(&format!("|MB_ICON{icon}"));
        }

        // One button, or an answer nobody reads: one line, no table, and the
        // fusion machinery is never entered.
        if set.answers.len() == 1 || dest.is_none() {
            self.emit(ir::Instruction::new(
                "MessageBox",
                vec![ir::Arg::raw(flags), text],
            ));
            if let Some(dest) = dest {
                self.diags.push(
                    Diagnostic::warning(
                        Code::ConstantAnswer,
                        span,
                        format!("this dialog can only answer `{}`", set.answers[0]),
                    )
                    .note(
                        "a one-button dialog has one outcome, so the comparison below it is \
                           already decided (§15.18)",
                    ),
                );
                self.emit(ir::Instruction::new(
                    "StrCpy",
                    vec![ir::Arg::dest(dest.clone()), ir::Arg::str(set.answers[0])],
                ));
            }
            return Some(Ty::Str);
        }

        if set.answers.len() > 2 {
            self.todo(span, &format!("the `{}` button set", set.installua));
            return None;
        }
        let dest = dest?;

        let n = self.body.construct();
        let first = self.fresh(format!("mb_{n}_{}", set.answers[0].to_lowercase()));
        let second = self.fresh(format!("mb_{n}_{}", set.answers[1].to_lowercase()));
        let end = self.fresh(format!("mb_{n}_end"));

        let current = self.current;
        self.body.terminate(
            current,
            Terminator::Branch {
                test: Test::Predicate {
                    name: "MessageBox".to_string(),
                    args: vec![ir::Arg::raw(flags), text],
                    keywords: set
                        .answers
                        .iter()
                        .map(|answer| format!("ID{answer}"))
                        .collect(),
                },
                then_block: first,
                else_block: second,
            },
        );

        self.current = first;
        self.emit(ir::Instruction::new(
            "StrCpy",
            vec![ir::Arg::dest(dest.clone()), ir::Arg::str(set.answers[0])],
        ));
        self.terminate(Terminator::Jump(end), second);
        self.emit(ir::Instruction::new(
            "StrCpy",
            vec![ir::Arg::dest(dest.clone()), ir::Arg::str(set.answers[1])],
        ));
        self.terminate(Terminator::Jump(end), end);

        Some(Ty::Str)
    }

    /// `messageBox("done")` and `messageBox { text = …, buttons = … }` — the
    /// positional-or-table pair §15.23 establishes, with `text` as the first
    /// positional parameter and `buttons` defaulting to `OK`.
    fn message_box_fields(
        &mut self,
        args: &[Expr],
        span: Span,
    ) -> Option<(ir::Arg, String, Option<String>)> {
        let [argument] = args else {
            self.diags.push(
                Diagnostic::error(
                    Code::WrongArity,
                    span,
                    format!(
                        "`messageBox` takes 1 argument, and {} were given",
                        args.len()
                    ),
                )
                .note("write `messageBox(\"text\")` or `messageBox { text = …, buttons = … }`"),
            );
            return None;
        };

        let Expr::Table { fields, .. } = argument else {
            let text = self.value(argument)?;
            return Some((text.arg, "OK".to_string(), None));
        };

        let (mut text, mut buttons, mut icon) = (None, "OK".to_string(), None);
        for field in fields {
            let TableField::Named { name, value } = field else {
                self.todo(span, "a positional entry in `messageBox`");
                continue;
            };
            match name.text.as_str() {
                "text" => text = Some(self.value(value)?),
                "buttons" => buttons = self.constant(value)?.text(),
                "icon" => icon = Some(self.constant(value)?.text()),
                other => {
                    self.diags.push(
                        Diagnostic::error(
                            Code::UnknownField,
                            name.span,
                            format!("`{other}` is not a `messageBox` field"),
                        )
                        .note("the fields are `text`, `buttons` and `icon`"),
                    );
                    return None;
                }
            }
        }

        let Some(text) = text else {
            self.diags.push(
                Diagnostic::error(Code::BadFieldValue, span, "`messageBox` needs a `text`")
                    .note("it is the message, and there is no default for it"),
            );
            return None;
        };
        Some((text.arg, buttons, icon))
    }

    /// `local a, b = f(x)`. Several names, one call.
    pub(super) fn call_multi(&mut self, call: &Expr, dests: &[Slot]) -> Option<Vec<Ty>> {
        let Expr::Call { callee, args, span } = call else {
            self.todo(call.span(), "this call");
            return None;
        };
        let name = callee_path(callee)?;
        if let Some((base, method)) = name.split_once('.')
            && self.resolved.namespaces.contains_key(base)
        {
            let (base, method) = (base.to_string(), method.to_string());
            return self.namespaced(&base, &method, args, dests, *span);
        }
        if !self.resolved.functions.contains_key(&name) {
            // An instruction's outputs are its values, and `GetFileTime` has
            // two of them. This is the only path that can bind both: the
            // single-value one drops everything past the first (§15.23).
            if let Some(builtin) = builtins::lookup(&name) {
                return self.builtin_multi(builtin, args, dests, *span);
            }
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
                    .filter(|builtin| builtin.predicate);
                match predicate {
                    Some(builtin) if builtin.arity().contains(&args.len()) => {
                        let params: Vec<&table::Param> = builtin.surface().collect();
                        let mut lowered = Vec::with_capacity(args.len());
                        for (argument, param) in args.iter().zip(params) {
                            let Some(value) = self.value(argument) else {
                                return;
                            };
                            lowered.push(if param.kind == table::Kind::Path {
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
                                    keywords: Vec::new(),
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
        // `string.lower(a) == "beta"` is a bare `StrCmp` — no `${StrCase}`, no
        // temporary, no `StrFunc` dependency — because case-insensitivity is
        // what the *instruction* already does. Case conversion for its value
        // still costs a macro; only the comparison collapses (§15.9).
        let folded = case_folded(lhs);
        let folded_rhs = case_folded(rhs);
        let case_sensitive = folded.is_none() && folded_rhs.is_none();
        let (lhs, rhs) = (folded.unwrap_or(lhs), folded_rhs.unwrap_or(rhs));

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
                    case_sensitive,
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

/// The subject of a `string.lower`/`string.upper` call, when that is what this
/// expression is. `Some` means the comparison it sits in can drop the case
/// conversion entirely and compare case-insensitively instead (§15.9).
fn case_folded(expr: &Expr) -> Option<&Expr> {
    let Expr::Call { callee, args, .. } = expr else {
        return None;
    };
    let name = callee_path(callee)?;
    if name != "string.lower" && name != "string.upper" {
        return None;
    }
    match args.as_slice() {
        [subject] => Some(subject),
        _ => None,
    }
}

/// A `messageBox` button set, and the answers it can give. The answer names
/// are the NSIS return keywords without their `ID` — `IDYES` is the jump-table
/// token and `"YES"` is the value the user compares against (§15.18).
struct ButtonSet {
    installua: &'static str,
    answers: &'static [&'static str],
}

const BUTTONS: &[ButtonSet] = &[
    ButtonSet {
        installua: "OK",
        answers: &["OK"],
    },
    ButtonSet {
        installua: "OKCANCEL",
        answers: &["OK", "CANCEL"],
    },
    ButtonSet {
        installua: "YESNO",
        answers: &["YES", "NO"],
    },
    ButtonSet {
        installua: "RETRYCANCEL",
        answers: &["RETRY", "CANCEL"],
    },
    ButtonSet {
        installua: "ABORTRETRYIGNORE",
        answers: &["ABORT", "RETRY", "IGNORE"],
    },
    ButtonSet {
        installua: "YESNOCANCEL",
        answers: &["YES", "NO", "CANCEL"],
    },
];

/// The handle's methods, for the error that says a name is not one of them.
/// Read from the table rather than listed, so a new `f:` row appears here the
/// day it is written.
fn methods() -> String {
    let mut names: Vec<&str> = table::table()
        .iter()
        .filter(|entry| entry.class == table::Class::Exposed)
        .filter_map(|entry| entry.installua)
        .filter_map(|name| name.strip_prefix("f:"))
        .collect();
    names.sort_unstable();
    names
        .iter()
        .map(|name| format!("`{name}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The emitted argument list, in **NSIS** order.
///
/// The surface order and the NSIS order are not the same list, and treating
/// them as one is how `FileReadByte $0 $1` gets emitted for
/// `local b = fileReadByte(f)` — two registers, so it assembles, and at run time
/// it reads from the destination and writes over the handle. The output is
/// second in `-CMDHELP` and the emitter used to write it first, unconditionally.
///
/// So the table says where everything goes and this walks it:
///
/// - a **flag** is written before the parameter it precedes — `Opt::after` is a
///   place in this line, not an argument index, which is why
///   `getFullPathName(p, { short = true })` puts `/SHORT` in front of the
///   *output* register;
/// - an **output** takes the next destination, or is skipped when there is none
///   (only reachable for an optional output — a required one was given a
///   temporary before the call);
/// - a [`Kind::Label`] input is the compiler's and is never emitted here;
/// - any other input takes the next lowered argument, and a [`Rep::Many`] tail
///   takes all of them;
/// - an input the caller omitted becomes *pending* rather than absent, and is
///   written only if some later position turns out to be emitted.
///
/// That last rule is the whole reason `fill` exists. `FileSeek handle offset
/// [mode] [$(user_var: new position)]` cannot be written `FileSeek $1 0 $0` —
/// NSIS reads `$0` as the mode and rejects the line — so reaching the output
/// means writing a mode the author never named. Pending-until-needed keeps
/// `fileSeek(f, 0)` at `FileSeek $1 0` and makes `local p = fileSeek(f, 0)`
/// into `FileSeek $1 0 SET $0`, from one table field and no special case.
fn place(builtin: &table::Instruction, written: Written, dests: Vec<ir::Arg>) -> Vec<ir::Arg> {
    let mut emitted = Vec::with_capacity(builtin.params.len());
    let mut pending: Vec<ir::Arg> = Vec::new();
    let mut inputs = written.inputs.into_iter();
    let mut dests = dests.into_iter();

    // The flags this call writes, by the number of parameters each one follows.
    let flags = |before: usize, emitted: &mut Vec<ir::Arg>, pending: &mut Vec<ir::Arg>| {
        for (flag, _) in builtin
            .options
            .iter()
            .zip(&written.flags)
            .filter(|(flag, on)| **on && flag.opt.after == before)
        {
            // A flag is an emitted token like any other, so anything pending in
            // front of it is no longer optional.
            emitted.append(pending);
            emitted.push(ir::Arg::raw(flag.opt.nsis));
        }
    };

    for (index, param) in builtin.params.iter().enumerate() {
        flags(index, &mut emitted, &mut pending);
        let argument = match param.dir() {
            table::Dir::Out => dests.next(),
            // Neither of these is a surface position, so neither consumes one:
            // a label is §8's and a fused half is not an argument at all.
            table::Dir::In if param.kind == table::Kind::Label => continue,
            table::Dir::In if param.kind == table::Kind::Fused => continue,
            table::Dir::In if param.shape.rep == table::Rep::Many => {
                // The repeated tail is the last position by construction, so
                // draining is safe and the count is the caller's.
                let group = inputs.next().unwrap_or_default();
                if !group.is_empty() {
                    emitted.append(&mut pending);
                    emitted.extend(group);
                }
                continue;
            }
            table::Dir::In => inputs.next().and_then(|group| group.into_iter().next()),
        };
        match argument {
            Some(argument) => {
                emitted.append(&mut pending);
                emitted.push(argument);
            }
            // Not "absent" yet: whether this position is written depends on
            // whether anything after it is. A position with no `fill` that turns
            // out to be needed is a table bug, and the census refuses it.
            None => pending.extend(param.fill.map(ir::Arg::raw)),
        }
    }
    // A trailing flag — `SendMessage … /TIMEOUT=n` — follows every parameter.
    flags(builtin.params.len(), &mut emitted, &mut pending);
    emitted
}

/// A row's option names, for the error that has to list them. Naming the legal
/// ones is the whole gain over counting: `` `execShell` has no option
/// `showmode` `` says what to write, where "takes 2 to 5 arguments" does not.
///
/// Positions and flags are one list, because a caller writing `{ … }` has no
/// reason to know which half a name comes from.
fn options(builtin: &table::Instruction) -> String {
    builtin
        .option_names()
        .iter()
        .map(|name| format!("`{name}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// An arity, said the way a reader counts. A repeated tail — `File a b c` — is
/// the one row shape left where the count is the caller's.
fn arguments(arity: &std::ops::RangeInclusive<usize>) -> String {
    match (*arity.start(), *arity.end()) {
        (least, usize::MAX) => format!("{least} or more argument(s)"),
        (least, most) if least == most => format!("{least} argument(s)"),
        (least, most) => format!("{least} to {most} argument(s)"),
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
