//! Liveness-based register allocation (§9-3), and the half of §15.11 that is
//! local to a body.
//!
//! nsL hands a register back with `setInUse(false)`, called by whoever
//! remembers to. Its issue #5 — a user's variable handed out as a temporary —
//! is not a bug in that call, it is the consequence of tracking ownership by
//! convention at all. Here nothing is handed back: lowering produces virtual
//! slots, this pass computes where each one is *live*, and two slots share a
//! register exactly when their live ranges do not overlap. A slot cannot be
//! stolen while it is live, because "live" is computed rather than remembered.
//!
//! The same analysis answers §15.11's question. A caller-save is `live ∩
//! clobbered` at the call, and the left half of that intersection is the
//! live-out set this pass already has to compute. That is the second of the
//! three consumers §15.11 promised one traversal would serve.
//!
//! Colouring is greedy over an interference graph, in slot order, taking the
//! lowest register that fits. Not optimal — optimal is NP-hard and twenty
//! registers is not where the wins are — but **deterministic**, which §14's
//! diffed goldens require.

use std::collections::{BTreeMap, BTreeSet};

use crate::cfg::{BasicBlock, Body};
use crate::diag::{Code, Diagnostic, Diagnostics};
use crate::ir;
use crate::regs::{COUNT, Slot};

/// What allocating one body learned.
pub struct Allocation {
    /// The registers this body writes: its **direct** clobber set, before
    /// anything its callees contribute (§15.11 step 2).
    pub clobbers: BTreeSet<u8>,
    /// Registers live across each call site, indexed as [`Body::calls`] is.
    /// The left half of `live ∩ clobbered`.
    pub live_across: Vec<BTreeSet<u8>>,
}

/// Colours every virtual slot in `body` and rewrites the body in place.
pub fn allocate(body: &mut Body, diags: &mut Diagnostics) -> Allocation {
    let live_out = dataflow(body);

    let mut interference: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); body.vregs.len()];
    let mut across: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); body.calls.len()];

    for (index, block) in body.blocks.iter().enumerate() {
        let mut live = live_out[index].clone();

        // The terminator runs last, so it is walked first.
        for slot in block.terminator_uses() {
            live.insert(slot);
        }

        for step in block.steps.iter().rev() {
            let (uses, defs) = step_slots(body, step);

            if let ir::Step::Call(site) = step {
                // Live *across* is what survives the call, which is the live-out
                // set minus the values the call itself produces: a returned
                // value did not exist on the way in and must not be saved.
                across[*site] = live.difference(&defs).copied().collect();
            }

            for def in &defs {
                for other in live.iter().chain(defs.iter()) {
                    if other != def {
                        interference[*def].insert(*other);
                        interference[*other].insert(*def);
                    }
                }
                live.remove(def);
            }
            live.extend(uses);
        }
    }

    let colours = colour(body, &interference, diags);

    let recolour = |index: u32| colours[index as usize];
    for block in &mut body.blocks {
        for step in &mut block.steps {
            if let ir::Step::Instruction(instruction) = step {
                instruction.recolour(&recolour);
            }
        }
        block.terminator.recolour(&recolour);
    }
    for site in &mut body.calls {
        for arg in &mut site.args {
            arg.recolour(&recolour);
        }
        for result in &mut site.results {
            if let Slot::Virtual(index) = *result {
                *result = Slot::Reg(recolour(index));
            }
        }
    }

    Allocation {
        clobbers: colours.iter().copied().collect(),
        live_across: across
            .into_iter()
            .map(|set| set.into_iter().map(|slot| colours[slot]).collect())
            .collect(),
    }
}

/// Fills in each call site's save list: `live ∩ clobbered`, ascending register
/// number (§15.11).
///
/// Ascending is not a correctness property — any order works as long as the
/// restores mirror it — but a save set that reordered between runs would churn
/// every golden file §14 diffs, so it is pinned.
pub fn insert_saves(
    body: &mut Body,
    live_across: &[BTreeSet<u8>],
    clobbers: &BTreeMap<String, BTreeSet<u8>>,
) {
    for (index, site) in body.calls.iter_mut().enumerate() {
        let Some(clobbered) = clobbers.get(&site.callee) else {
            continue;
        };
        site.saves = live_across[index]
            .intersection(clobbered)
            .map(|register| Slot::Reg(*register))
            .collect();
    }
}

/// Backward liveness to a fixpoint: `live_in = use ∪ (live_out − def)`.
fn dataflow(body: &Body) -> Vec<BTreeSet<usize>> {
    let blocks = body.blocks.len();
    let mut upward: Vec<BTreeSet<usize>> = Vec::with_capacity(blocks);
    let mut killed: Vec<BTreeSet<usize>> = Vec::with_capacity(blocks);

    for block in &body.blocks {
        let mut uses = BTreeSet::new();
        let mut defs = BTreeSet::new();
        for step in &block.steps {
            let (stepped_uses, stepped_defs) = step_slots(body, step);
            for slot in stepped_uses {
                // Upward-exposed only: a use after a def in the same block is
                // answered inside it.
                if !defs.contains(&slot) {
                    uses.insert(slot);
                }
            }
            defs.extend(stepped_defs);
        }
        for slot in block.terminator_uses() {
            if !defs.contains(&slot) {
                uses.insert(slot);
            }
        }
        upward.push(uses);
        killed.push(defs);
    }

    let mut live_in: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); blocks];
    let mut live_out: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); blocks];
    let mut changed = true;
    while changed {
        changed = false;
        for index in (0..blocks).rev() {
            let mut out = BTreeSet::new();
            for successor in body.blocks[index].terminator.successors() {
                out.extend(live_in[successor.0].iter().copied());
            }
            let mut into: BTreeSet<usize> = out.difference(&killed[index]).copied().collect();
            into.extend(upward[index].iter().copied());
            if out != live_out[index] || into != live_in[index] {
                live_out[index] = out;
                live_in[index] = into;
                changed = true;
            }
        }
    }
    live_out
}

/// A step's `(uses, defs)`, as virtual slot numbers. Globals are dropped here
/// and nowhere else: a `Var` is not allocated, not coloured and — the part that
/// is correctness rather than thrift — never saved around a call (§15.11).
fn step_slots(body: &Body, step: &ir::Step) -> (BTreeSet<usize>, BTreeSet<usize>) {
    let (uses, defs) = match step {
        ir::Step::Instruction(instruction) => (instruction.uses(), instruction.defs()),
        // The pushes read registers that are live by construction, and the pops
        // write back what they read, so a save is neither a use nor a def.
        ir::Step::Saves(_) => (Vec::new(), Vec::new()),
        ir::Step::Call(site) => {
            let site = &body.calls[*site];
            let mut uses = Vec::new();
            for arg in &site.args {
                arg.uses(&mut uses);
            }
            (uses, site.results.clone())
        }
    };
    (virtuals(&uses), virtuals(&defs))
}

fn virtuals(slots: &[Slot]) -> BTreeSet<usize> {
    slots
        .iter()
        .filter_map(|slot| slot.virtual_index().map(|index| index as usize))
        .collect()
}

impl BasicBlock {
    fn terminator_uses(&self) -> BTreeSet<usize> {
        match &self.terminator {
            crate::cfg::Terminator::Branch { test, .. } => virtuals(&test.uses()),
            _ => BTreeSet::new(),
        }
    }
}

/// Greedy colouring in slot order, lowest register first.
fn colour(body: &Body, interference: &[BTreeSet<usize>], diags: &mut Diagnostics) -> Vec<u8> {
    let mut colours = vec![0u8; interference.len()];
    let mut assigned: Vec<Option<u8>> = vec![None; interference.len()];
    let mut reported = false;

    for slot in 0..interference.len() {
        let taken: BTreeSet<u8> = interference[slot]
            .iter()
            .filter_map(|other| assigned[*other])
            .collect();
        match (0..COUNT).find(|register| !taken.contains(register)) {
            Some(register) => {
                assigned[slot] = Some(register);
                colours[slot] = register;
            }
            None if reported => {}
            None => {
                reported = true;
                diags.push(
                    Diagnostic::error(
                        Code::RegisterExhaustion,
                        body.vregs[slot],
                        "this body keeps more values alive at once than NSIS has registers",
                    )
                    .note(format!(
                        "there are {COUNT} — `$0`–`$9` and `$R0`–`$R9` — and all of them are \
                         holding something here"
                    ))
                    .note(
                        "values whose live ranges do not overlap already share a register, so \
                         this is the real limit rather than an allocator that leaks (§9-3)",
                    )
                    .note("split the body into `func`s: a call's arguments travel on the stack"),
                );
            }
        }
    }
    colours
}
