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

/// `0` in a branch slot: fall through to the next line.
const FALLTHROUGH: &str = "0";

pub fn lay_out(body: &Body) -> Vec<ir::Item> {
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
            out.push(ir::Item::Label(block.label.clone()));
        }
        out.extend(
            block
                .instructions
                .iter()
                .cloned()
                .map(ir::Item::Instruction),
        );
        out.extend(items);
    }
    out
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
/// `IfFileExists "…" 0 _luagen_endif_0` the shape: the arm a reader expects to
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
) -> (Vec<ir::Item>, Vec<BlockId>) {
    let block: &BasicBlock = body.block(id);
    match &block.terminator {
        Terminator::Jump(target) if next == Some(*target) => (Vec::new(), Vec::new()),
        Terminator::Jump(target) => (
            vec![instruction("Goto", vec![label(body, *target)])],
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

                Test::Predicate { name, args } => {
                    let arms = vec![arm(*then_block), arm(*else_block)];
                    let mut all = args.clone();
                    all.extend(arms);
                    instruction(name.clone(), all)
                }
            };

            (vec![line], targets)
        }

        // A `Return` at the very end of a body is what falling off the end
        // already does, so emitting one would be a line nobody asked for.
        Terminator::Return if last => (Vec::new(), Vec::new()),
        Terminator::Return => (vec![instruction("Return", Vec::new())], Vec::new()),

        Terminator::Unreachable => {
            debug_assert!(
                block.instructions.is_empty(),
                "a reachable block was never terminated: {} carries {} instruction(s)",
                block.label,
                block.instructions.len()
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
