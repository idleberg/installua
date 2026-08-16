//! A control-flow graph of basic blocks with explicit terminators (§8).
//!
//! This is the structure that keeps labels out of the statement lowerers. nsL's
//! `ISSUES.md` lists "parse-time state corruption" as a whole *category*, and
//! the known instance was break/continue labels — the cause being labels
//! invented ad-hoc, mid-single-pass, by whoever needed one. Here nothing invents
//! a label: a lowerer creates blocks and points terminators at them, and
//! [`crate::layout`] has sole knowledge of what a label is.
//!
//! Four consequences fall out without being written as special cases:
//!
//!   * `break`, `continue()` and early `return` are terminators, so no statement
//!     lowerer does any label bookkeeping at all;
//!   * a label is emitted only for a block something actually jumps to;
//!   * a `Goto` to the next line is never emitted;
//!   * dead blocks disappear.
//!
//! CFGs are strictly per-body. NSIS `Goto` cannot cross a `Function`/`Section`
//! boundary and a leading `.` makes a label global, so per-body construction
//! makes that a non-issue — and [`crate::layout`] asserts it anyway.

use crate::diag::Span;
use crate::ir;
use crate::regs::Slot;
use crate::types::{Sign, Ty, Width};

/// Every generated label carries this prefix (§15.19). It defends against
/// header macros and `raw` — `WinVer.nsh` emits `_winver_sp_done` and
/// `StrFunc` emits labels as generic as `done`, `loop` and `ret` — rather than
/// against user code, which cannot name a label at all. It is deliberately not
/// the product name: a rename would otherwise rewrite every golden file for no
/// semantic change.
///
/// Uppercase and double-underscored so that a label is unmistakably not a
/// user's line in a diff, and sorts away from everything a header is likely to
/// emit. A leading `.` would make it global in NSIS, which is why the leading
/// character is an underscore and [`crate::layout`] asserts it stays one.
pub const LABEL_PREFIX: &str = "__GENERATED_";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockId(pub usize);

/// One body — a section, a function, a callback. The entry block is always
/// [`Body::ENTRY`].
#[derive(Clone, Debug)]
pub struct Body {
    pub blocks: Vec<BasicBlock>,
    /// Expression temporaries the lowerer had to materialise. Phase 2's exit
    /// criterion is a statement about this number: condition fusion never moves
    /// it, because a fused condition materialises nothing.
    pub temps: usize,
    /// Where each virtual slot was created, indexed by slot number. The
    /// allocator needs a span to point at when twenty registers are not enough,
    /// and this is the only place that knows one.
    pub vregs: Vec<Span>,
    /// Call sites, in creation order. [`ir::Step::Saves`] and
    /// [`ir::Step::Call`] index into this.
    pub calls: Vec<ir::CallSite>,
    /// The per-body construct counter (§15.25). It resets here rather than
    /// running monotonically over the program so that inserting an `if` early
    /// renumbers labels in that body only, which is what keeps §14's whole-file
    /// goldens diffable.
    next_construct: usize,
}

impl Body {
    pub const ENTRY: BlockId = BlockId(0);

    pub fn new(span: Span) -> Body {
        Body {
            blocks: vec![BasicBlock::new(format!("{LABEL_PREFIX}entry"), span)],
            temps: 0,
            vregs: Vec::new(),
            calls: Vec::new(),
            next_construct: 0,
        }
    }

    /// A fresh virtual slot. There is no supply to run out of here: exhaustion
    /// is a fact about *simultaneously live* values, which nothing knows until
    /// the body is complete, so the allocator raises it and lowering does not
    /// (§9-3).
    pub fn vreg(&mut self, span: Span) -> Slot {
        self.vregs.push(span);
        Slot::Virtual((self.vregs.len() - 1) as u32)
    }

    /// Reserves a call site, returning its index. The site is created *before*
    /// its arguments are lowered, because the saves are pushed before the
    /// argument evaluation and the marker has to go in first.
    pub fn call_site(&mut self, callee: impl Into<String>, span: Span) -> usize {
        self.calls.push(ir::CallSite {
            callee: callee.into(),
            args: Vec::new(),
            results: Vec::new(),
            saves: Vec::new(),
            span,
        });
        self.calls.len() - 1
    }

    /// A fresh construct number. One per `if`, `while` or `for`, so a
    /// construct's blocks share it: `_luagen_else_3` and `_luagen_endif_3` are
    /// visibly the same statement.
    pub fn construct(&mut self) -> usize {
        let n = self.next_construct;
        self.next_construct += 1;
        n
    }

    /// A new block, named deterministically at creation rather than at layout —
    /// determinism is what makes a golden `.nsi` diffable (§8).
    pub fn new_block(&mut self, label: impl Into<String>, span: Span) -> BlockId {
        self.blocks.push(BasicBlock::new(label.into(), span));
        BlockId(self.blocks.len() - 1)
    }

    /// Whether control can reach `id` from the entry. The layout pass answers
    /// the same question by simply not laying a block out; this exists because
    /// the arity check has to distinguish "falls off the end" from "the block
    /// after an unconditional `return`", and those differ only in reachability.
    pub fn reachable(&self, id: BlockId) -> bool {
        let mut seen = vec![false; self.blocks.len()];
        let mut stack = vec![Body::ENTRY];
        seen[Body::ENTRY.0] = true;
        while let Some(block) = stack.pop() {
            if block == id {
                return true;
            }
            for successor in self.blocks[block.0].terminator.successors() {
                if !seen[successor.0] {
                    seen[successor.0] = true;
                    stack.push(successor);
                }
            }
        }
        false
    }

    pub fn block(&self, id: BlockId) -> &BasicBlock {
        &self.blocks[id.0]
    }

    pub fn block_mut(&mut self, id: BlockId) -> &mut BasicBlock {
        &mut self.blocks[id.0]
    }

    pub fn push(&mut self, id: BlockId, instruction: ir::Instruction) {
        self.blocks[id.0]
            .steps
            .push(ir::Step::Instruction(instruction));
    }

    pub fn push_step(&mut self, id: BlockId, step: ir::Step) {
        self.blocks[id.0].steps.push(step);
    }

    /// Points `id` at its successors. A `Branch` whose arms are the same block
    /// is a `Jump`, which is the one normalisation the layout pass is allowed to
    /// assume rather than check.
    pub fn terminate(&mut self, id: BlockId, terminator: Terminator) {
        let terminator = match terminator {
            Terminator::Branch {
                then_block,
                else_block,
                ..
            } if then_block == else_block => Terminator::Jump(then_block),
            other => other,
        };
        self.blocks[id.0].terminator = terminator;
    }
}

#[derive(Clone, Debug)]
pub struct BasicBlock {
    pub label: String,
    pub steps: Vec<ir::Step>,
    pub terminator: Terminator,
    /// Where the block came from, for the diagnostics a later pass raises about
    /// it and for §15.22's line map.
    pub span: Span,
}

impl BasicBlock {
    fn new(label: String, span: Span) -> BasicBlock {
        BasicBlock {
            label,
            steps: Vec::new(),
            // A block that nobody terminates is a lowering bug, and saying so is
            // cheaper than defaulting to a fallthrough that silently works.
            terminator: Terminator::Unreachable,
            span,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Terminator {
    Jump(BlockId),
    Branch {
        test: Test,
        then_block: BlockId,
        else_block: BlockId,
    },
    Return,
    Unreachable,
}

impl Terminator {
    pub fn recolour(&mut self, colour: &dyn Fn(u32) -> u8) {
        if let Terminator::Branch { test, .. } = self {
            test.recolour(colour);
        }
    }

    pub fn successors(&self) -> Vec<BlockId> {
        match self {
            Terminator::Jump(target) => vec![*target],
            Terminator::Branch {
                then_block,
                else_block,
                ..
            } => vec![*then_block, *else_block],
            Terminator::Return | Terminator::Unreachable => Vec::new(),
        }
    }
}

/// A compare-and-jump. NSIS has no test-then-jump instruction at all, which is
/// why a `Test` is part of the terminator rather than a value some earlier
/// instruction produced.
#[derive(Clone, Debug)]
pub enum Test {
    /// `StrCmp` / `StrCmpS`. Two arms, so strings get equality and nothing else
    /// — which is why the lattice, not the operator, decides the instruction.
    Str {
        lhs: ir::Arg,
        rhs: ir::Arg,
        /// `==` is `StrCmpS`: case-sensitive is the *default* here and the
        /// reversal from NSIS habit that will bite hardest (§15.9).
        case_sensitive: bool,
        /// `~=` swaps the arms rather than inverting at the call site.
        negate: bool,
    },
    /// The `IntCmp` family. Three arms, so one instruction and no temporaries
    /// covers all six relational operators (§12).
    Int {
        op: CmpOp,
        lhs: ir::Arg,
        rhs: ir::Arg,
        family: IntFamily,
    },
    /// `IfFileExists`, `IfErrors`, `IfSilent` — a branching instruction with no
    /// right-hand side. §15.20 dissolves the second condition shape these used
    /// to need: `fileExists(p)` is an ordinary `bool`-valued call, and fusing it
    /// spends no register.
    Predicate { name: String, args: Vec<ir::Arg> },
}

impl Test {
    /// `if flag then`, where `flag : bool`. The runtime representation of a
    /// `bool` is the text `1` or `0`, so the test is an ordinary string compare
    /// — chosen over `IntCmp` because it has two arms rather than three and a
    /// bool has two values.
    /// Everything the test reads. A terminator defines nothing — NSIS has no
    /// compare that writes — so uses are the whole story.
    pub fn uses(&self) -> Vec<Slot> {
        let mut out = Vec::new();
        match self {
            Test::Str { lhs, rhs, .. } | Test::Int { lhs, rhs, .. } => {
                lhs.uses(&mut out);
                rhs.uses(&mut out);
            }
            Test::Predicate { args, .. } => {
                for arg in args {
                    arg.uses(&mut out);
                }
            }
        }
        out
    }

    pub fn recolour(&mut self, colour: &dyn Fn(u32) -> u8) {
        match self {
            Test::Str { lhs, rhs, .. } | Test::Int { lhs, rhs, .. } => {
                lhs.recolour(colour);
                rhs.recolour(colour);
            }
            Test::Predicate { args, .. } => {
                for arg in args {
                    arg.recolour(colour);
                }
            }
        }
    }

    pub fn boolean(value: ir::Arg) -> Test {
        Test::Str {
            lhs: value,
            rhs: ir::Arg::str(TRUE),
            case_sensitive: true,
            negate: false,
        }
    }
}

/// A `bool` in a register. Not `"true"`/`"false"`: NSIS's own flag-shaped
/// values are `1` and `0`, and an `IntOp` over one then needs no conversion.
pub const TRUE: &str = "1";
pub const FALSE: &str = "0";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

/// Which arm of a three-armed `IntCmp` goes where.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arm {
    Then,
    Else,
}

impl CmpOp {
    /// `IntCmp a b <eq> <lt> <gt>` covers every relational operator in one
    /// instruction — the payoff of §8's "no materialised booleans", and not
    /// obvious from the NSIS documentation, so it is written down as a table
    /// (§12).
    pub fn arms(self) -> [Arm; 3] {
        use Arm::{Else, Then};
        match self {
            CmpOp::Eq => [Then, Else, Else],
            CmpOp::Ne => [Else, Then, Then],
            CmpOp::Lt => [Else, Then, Else],
            CmpOp::Le => [Then, Then, Else],
            CmpOp::Gt => [Else, Else, Then],
            CmpOp::Ge => [Then, Else, Then],
        }
    }

    /// The operator with its operands swapped, for the one place that is
    /// cheaper than negating.
    pub fn flip(self) -> CmpOp {
        match self {
            CmpOp::Eq => CmpOp::Eq,
            CmpOp::Ne => CmpOp::Ne,
            CmpOp::Lt => CmpOp::Gt,
            CmpOp::Le => CmpOp::Ge,
            CmpOp::Gt => CmpOp::Lt,
            CmpOp::Ge => CmpOp::Le,
        }
    }
}

/// The `IntCmp` product: width picks the family, sign picks the member. Six
/// instructions, two axes, exactly as §15.14 factors the type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntFamily {
    Int,
    IntU,
    Int64,
    Int64U,
    IntPtr,
    IntPtrU,
}

impl IntFamily {
    /// Chosen from the joined type of both operands.
    pub fn of(ty: Ty) -> IntFamily {
        let Some(int) = ty.as_int() else {
            return IntFamily::Int;
        };
        // Unsigned is used only when *both* operands are known non-negative,
        // which the caller has already joined into `sign`. Guessing the other
        // way makes `-1 < 0` false.
        match (int.width, int.sign) {
            (Width::W32, Sign::Unknown) => IntFamily::Int,
            (Width::W32, Sign::NonNeg) => IntFamily::IntU,
            (Width::W64, Sign::Unknown) => IntFamily::Int64,
            (Width::W64, Sign::NonNeg) => IntFamily::Int64U,
            (Width::Ptr, Sign::Unknown) => IntFamily::IntPtr,
            (Width::Ptr, Sign::NonNeg) => IntFamily::IntPtrU,
        }
    }

    pub fn instruction(self) -> &'static str {
        match self {
            IntFamily::Int => "IntCmp",
            IntFamily::IntU => "IntCmpU",
            IntFamily::Int64 => "Int64Cmp",
            IntFamily::Int64U => "Int64CmpU",
            IntFamily::IntPtr => "IntPtrCmp",
            IntFamily::IntPtrU => "IntPtrCmpU",
        }
    }
}
