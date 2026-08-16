//! The layout pass: CFG in, a flat list of instructions and labels out.
//!
//! This is the *only* place in the compiler that knows what a label is (§8).
//! Every statement lowerer creates blocks and points terminators at them; the
//! decision of which of those blocks needs a name, and what that name is, is
//! made here and nowhere else.
//!
//! Three of §8's four promised consequences are this file:
//!
//!   * a label is emitted only for a block something jumps to explicitly, so
//!     the output has far fewer labels than a per-construct scheme emits;
//!   * a `Goto` to the next line is never emitted;
//!   * dead blocks disappear.
//!
//! And one thing is deliberately *not* here: relative jumps. `IntCmp a b +2` is
//! correct until a later pass inserts an instruction and then silently wrong,
//! so a fallthrough is always spelled `0` and a target is always a label (§8).

use std::collections::HashSet;

use crate::cfg::{Arm, BasicBlock, BlockId, Body, Terminator, Test};
use crate::ir;
use crate::map::Origin;

/// `0` in a branch slot: fall through to the next line.
const FALLTHROUGH: &str = "0";

/// One laid-out line: what it is, and where it came from (§15.22).
pub type Line = (ir::Item, Origin);

/// The origin of an instruction, from the span the lowerer stamped on it. An
/// instruction with no span is one the compiler emitted on its own behalf —
/// which is precisely the set whose failure under `makensis` is a compiler bug
/// rather than a user's mistake.
fn origin(instruction: &ir::Instruction, what: &'static str) -> Origin {
    match instruction.span {
        Some(span) => Origin::User(span),
        None => Origin::Emitted(what),
    }
}

pub fn lay_out(body: &Body) -> Vec<Line> {
    let order = order(body);
    let mut rendered = Vec::with_capacity(order.len());
    let mut referenced: HashSet<BlockId> = HashSet::new();

    for (index, id) in order.iter().enumerate() {
        let next = order.get(index + 1).copied();
        let last = index + 1 == order.len();
        let (items, targets) = terminator(body, *id, next, last);
        referenced.extend(targets.iter().copied());
        rendered.push(items);
    }

    let mut out = Vec::new();
    for (id, items) in order.iter().zip(rendered) {
        let block = body.block(*id);
        debug_assert!(
            !block.label.starts_with('.'),
            "a leading `.` makes a label global in NSIS, and every label here is body-local (§8)"
        );
        if referenced.contains(id) {
            // A label is the compiler's own line by construction: §8 gives the
            // user no spelling for one at all.
            out.push((
                ir::Item::Label(block.label.clone()),
                Origin::Emitted("label"),
            ));
        }
        for step in &block.steps {
            expand(body, step, &mut out);
        }
        out.extend(items);
    }
    out
}

/// A step, as the lines it becomes.
///
/// The whole calling convention is these fourteen lines, and it is here rather
/// than in the lowerer for the same reason labels are: one place decides, and
/// everywhere else points at it. Program 4 pins every choice in it (§11).
fn expand(body: &Body, step: &ir::Step, out: &mut Vec<Line>) {
    match step {
        // `StrCpy $2 $2` is what a copy out of a scratch register becomes once
        // the allocator has given both ends the same colour — §15.4's fixup
        // computes into a scratch precisely so that `x = x // y` is safe, and
        // when `x` was dead the two coalesce. Dropping it here rather than
        // avoiding it in the lowerer keeps the rule where the register numbers
        // are (§9-3).
        ir::Step::Instruction(instruction) if self_copy(instruction) => {}
        ir::Step::Instruction(instruction) => out.push((
            ir::Item::Instruction(instruction.clone()),
            origin(instruction, "instruction"),
        )),

        // Saves go on **before** the arguments, so the callee's results sit on
        // top when it returns and the restores fall out underneath them — no
        // `Exch` anywhere in the convention.
        ir::Step::Saves(site) => {
            for save in &body.calls[*site].saves {
                out.push((
                    instruction("Push", vec![ir::Arg::slot(save.clone())]),
                    Origin::Emitted("caller-save"),
                ));
            }
        }

        ir::Step::Call(site) => {
            let site = &body.calls[*site];
            match &site.kind {
                // An opaque callee already carries its arguments: a plugin
                // takes them inline, and a `raw` block is whatever was written.
                // A `raw` block is the user's text and nothing checked it; a
                // plugin line is the compiler's, built from a declaration. The
                // two fail differently and §15.22 reports them differently.
                ir::CallKind::Opaque { lines, raw } => {
                    for line in lines {
                        let origin = match (raw, line.span) {
                            (true, _) => Origin::Raw(site.span),
                            (false, _) => origin(line, "plugin call"),
                        };
                        out.push((ir::Item::Instruction(line.clone()), origin));
                    }
                }
                ir::CallKind::Function => {
                    // Reverse source order, so the callee's first `Pop` is its
                    // first parameter.
                    for arg in site.args.iter().rev() {
                        out.push((
                            instruction("Push", vec![arg.clone()]),
                            Origin::User(site.span),
                        ));
                    }
                    out.push((
                        instruction("Call", vec![ir::Arg::raw(site.callee.clone())]),
                        Origin::User(site.span),
                    ));
                }
            }
            // The callee pushed its returns in reverse too, so these come off in
            // source order.
            for result in &site.results {
                out.push((
                    instruction("Pop", vec![ir::Arg::dest(result.clone())]),
                    Origin::User(site.span),
                ));
            }
            for save in site.saves.iter().rev() {
                out.push((
                    instruction("Pop", vec![ir::Arg::dest(save.clone())]),
                    Origin::Emitted("caller-save"),
                ));
            }
        }
    }
}

/// `StrCpy $2 $2`: a copy of a register into itself, and nothing else. The
/// three-and four-argument forms take a length and an offset, so they are
/// substrings rather than copies and are left alone.
fn self_copy(instruction: &ir::Instruction) -> bool {
    match instruction.args.as_slice() {
        [ir::Arg::Dest(dest), source] => {
            matches!(source, ir::Arg::Data { pieces, .. } if pieces.as_slice() == [ir::Piece::Slot(dest.clone())])
                && instruction.name == "StrCpy"
        }
        _ => false,
    }
}

/// Reverse postorder from the entry.
///
/// Not a preorder walk, which is the tempting version and the wrong one: a
/// preorder lays the whole of an `if`'s then-arm *and everything after the
/// join* before it reaches the else-arm, which strands the else-arm past the
/// end of the body. Reverse postorder places a block after the blocks that
/// reach it, so a join lands after both arms and a loop's exit lands after its
/// body.
///
/// Successors are walked in reverse — the else-arm first — precisely so that
/// reversing puts the *then*-arm first. That is what makes
/// `IfFileExists "…" 0 __GENERATED_endif_0` the shape: the arm a reader expects to
/// run next is the one that costs no jump.
///
/// Blocks never reached are simply not in the result, which is how dead code
/// disappears without anybody writing a pass for it (§8).
fn order(body: &Body) -> Vec<BlockId> {
    let mut post = Vec::new();
    let mut seen = HashSet::from([Body::ENTRY]);
    // `(block, how many successors have been dealt with)`, iterative so a
    // pathological body cannot blow the stack.
    let mut stack = vec![(Body::ENTRY, 0usize)];

    while let Some((id, index)) = stack.pop() {
        let successors = body.block(id).terminator.successors();
        if index == successors.len() {
            post.push(id);
            continue;
        }
        stack.push((id, index + 1));
        let successor = successors[successors.len() - 1 - index];
        if seen.insert(successor) {
            stack.push((successor, 0));
        }
    }

    post.reverse();
    post
}

/// The instructions a block's terminator becomes, plus the blocks it names.
///
/// `next` is the block laid out immediately after this one — the only thing a
/// fallthrough can mean.
fn terminator(
    body: &Body,
    id: BlockId,
    next: Option<BlockId>,
    last: bool,
) -> (Vec<Line>, Vec<BlockId>) {
    let block: &BasicBlock = body.block(id);
    // A terminator is the tail of whatever statement built the block, so the
    // block's span is the best attribution there is — and a `Goto` is nobody's
    // line but the compiler's (§8).
    let here = Origin::User(block.span);
    match &block.terminator {
        Terminator::Jump(target) if next == Some(*target) => (Vec::new(), Vec::new()),
        Terminator::Jump(target) => (
            vec![(
                instruction("Goto", vec![label(body, *target)]),
                Origin::Emitted("goto"),
            )],
            vec![*target],
        ),

        Terminator::Branch {
            test,
            then_block,
            else_block,
        } => {
            let mut targets = Vec::new();
            let mut arm = |target: BlockId| {
                if next == Some(target) {
                    ir::Arg::raw(FALLTHROUGH)
                } else {
                    targets.push(target);
                    label(body, target)
                }
            };

            let line = match test {
                Test::Str {
                    lhs,
                    rhs,
                    case_sensitive,
                    negate,
                } => {
                    let (equal, not_equal) = if *negate {
                        (*else_block, *then_block)
                    } else {
                        (*then_block, *else_block)
                    };
                    // `==` is case-sensitive here, which is the reverse of the
                    // NSIS habit `StrCmp` teaches (§15.9).
                    let name = if *case_sensitive { "StrCmpS" } else { "StrCmp" };
                    let arms = vec![arm(equal), arm(not_equal)];
                    let mut args = vec![lhs.clone(), rhs.clone()];
                    args.extend(arms);
                    instruction(name, args)
                }

                Test::Int {
                    op,
                    lhs,
                    rhs,
                    family,
                } => {
                    let arms = op
                        .arms()
                        .map(|slot| match slot {
                            Arm::Then => *then_block,
                            Arm::Else => *else_block,
                        })
                        .map(&mut arm);
                    let mut args = vec![lhs.clone(), rhs.clone()];
                    args.extend(arms);
                    instruction(family.instruction(), args)
                }

                Test::Predicate {
                    name,
                    args,
                    keywords,
                } if keywords.is_empty() => {
                    let arms = vec![arm(*then_block), arm(*else_block)];
                    let mut all = args.clone();
                    all.extend(arms);
                    instruction(name.clone(), all)
                }

                // A keyed jump table: `MessageBox … IDYES lbl`. An arm that
                // falls through is left out altogether rather than spelled `0`,
                // because `MessageBox` has no fall-through slot to put a `0` in
                // — its arms are optional pairs (§15.18).
                Test::Predicate {
                    name,
                    args,
                    keywords,
                } => {
                    let mut all = args.clone();
                    for (keyword, target) in keywords.iter().zip([*then_block, *else_block]) {
                        if next == Some(target) {
                            continue;
                        }
                        targets.push(target);
                        all.push(ir::Arg::raw(keyword.clone()));
                        all.push(label(body, target));
                    }
                    instruction(name.clone(), all)
                }
            };

            (vec![(line, here)], targets)
        }

        // A `Return` at the very end of a body is what falling off the end
        // already does, so emitting one would be a line nobody asked for.
        Terminator::Return if last => (Vec::new(), Vec::new()),
        Terminator::Return => (
            vec![(instruction("Return", Vec::new()), Origin::Emitted("return"))],
            Vec::new(),
        ),

        Terminator::Unreachable => {
            debug_assert!(
                block.steps.is_empty(),
                "a reachable block was never terminated: {} carries {} step(s)",
                block.label,
                block.steps.len()
            );
            (Vec::new(), Vec::new())
        }
    }
}

fn label(body: &Body, target: BlockId) -> ir::Arg {
    ir::Arg::raw(body.block(target).label.clone())
}

fn instruction(name: impl Into<String>, args: Vec<ir::Arg>) -> ir::Item {
    ir::Item::Instruction(ir::Instruction::new(name, args))
}
