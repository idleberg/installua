//! The call graph — built once, read three times.
//!
//! The three consumers are what makes a whole-program traversal worth its cost:
//! clobber sets for caller-saves, uninstaller reachability and return-type
//! inference. Two of them are here; the third is not, because `uninstaller {}`
//! has no lowering yet, and building the consumer before the block it consumes
//! would be inventing the answer to a question nobody has asked.
//!
//! **Recursion needs no special case.** It is an SCC whose fixpoint saturates
//! in one extra round, which is what the condensation buys: process strongly
//! connected components in reverse topological order, and a cycle is just a
//! component with more than one member — or with one member and an edge to
//! itself.
//!
//! The one thing this file *cannot* answer is how deep a recursion goes at
//! runtime, and Phase 0 measured what happens when it goes too deep: at roughly
//! 1300 frames under wine the process dies **silently** — no dialog, no log
//! line, no error level. A warning naming the iterative form is the honest
//! response to a failure mode that ships without a symptom.

use std::collections::{BTreeMap, BTreeSet};

use crate::diag::{Code, Diagnostic, Diagnostics, Span};
use crate::ir;

pub struct CallGraph {
    /// Function names, in module order. The index is the node.
    pub names: Vec<String>,
    /// Caller → callees. Only `func`s are nodes: a section is a root that
    /// nothing calls.
    pub edges: Vec<BTreeSet<usize>>,
    /// Strongly connected components, **callees first**. Tarjan emits them in
    /// exactly this order, which is why no separate topological sort is needed.
    pub sccs: Vec<Vec<usize>>,
    /// Where each function was declared, for the diagnostics this file raises.
    /// Public so a test can build a graph by hand: the clobber fixpoint was
    /// exercised once across all five programs, and a synthetic graph is how a
    /// cycle gets tested without a program around it.
    pub spans: Vec<Span>,
}

pub fn build(module: &ir::Module) -> CallGraph {
    let names: Vec<String> = module
        .functions
        .iter()
        .map(|function| function.name.clone())
        .collect();
    let index: BTreeMap<&str, usize> = names
        .iter()
        .enumerate()
        .map(|(node, name)| (name.as_str(), node))
        .collect();

    let mut edges = vec![BTreeSet::new(); names.len()];
    let mut spans = Vec::with_capacity(names.len());
    for (node, function) in module.functions.iter().enumerate() {
        spans.push(function.body.blocks[0].span);
        for site in &function.body.calls {
            if let Some(callee) = index.get(site.callee.as_str()) {
                edges[node].insert(*callee);
            }
        }
    }

    let sccs = tarjan(&edges);
    CallGraph {
        names,
        edges,
        sccs,
        spans,
    }
}

impl CallGraph {
    /// The clobber-set fixpoint (step 3).
    ///
    /// `direct` is what each body writes itself, from the register allocator.
    /// Within an SCC every member ends up with the union of the whole
    /// component, which is why a recursive cycle is not a special case: it is a
    /// component whose members all clobber whatever any of them clobbers.
    pub fn clobbers(
        &self,
        direct: &BTreeMap<String, BTreeSet<u8>>,
    ) -> BTreeMap<String, BTreeSet<u8>> {
        let mut total: Vec<BTreeSet<u8>> = vec![BTreeSet::new(); self.names.len()];

        for component in &self.sccs {
            // Callees outside the component are already final — that is what
            // reverse topological order means — so one pass over the component
            // saturates it, and the union is then shared by every member.
            let mut union = BTreeSet::new();
            for node in component {
                if let Some(own) = direct.get(&self.names[*node]) {
                    union.extend(own.iter().copied());
                }
                for callee in &self.edges[*node] {
                    union.extend(total[*callee].iter().copied());
                }
            }
            for node in component {
                total[*node] = union.clone();
            }
        }

        self.names.iter().cloned().zip(total).collect()
    }

    /// The depth-cliff lint.
    pub fn lint_recursion(&self, diags: &mut Diagnostics) {
        for component in &self.sccs {
            let recursive = component.len() > 1
                || component
                    .first()
                    .is_some_and(|node| self.edges[*node].contains(node));
            if !recursive {
                continue;
            }

            let cycle = component
                .iter()
                .map(|node| format!("`{}`", self.names[*node]))
                .collect::<Vec<_>>()
                .join(" → ");
            let node = component[0];
            diags.push(
                Diagnostic::warning(
                    Code::DeepRecursion,
                    self.spans[node],
                    format!("{cycle} is recursive, and NSIS gives no warning when that runs out"),
                )
                .note(
                    "the exehead recurses natively on `Call`, so the ceiling is the installer \
                     thread's stack: measured at roughly 1300 frames, and it differs per machine \
                    ",
                )
                .note(
                    "past it the process dies silently — no dialog, no log line, no error level \
                     — so a bug here ships looking like a cancelled install",
                )
                .note("rewrite it as a `while` loop if the depth is not statically bounded"),
            );
        }
    }
}

/// Tarjan's algorithm, iterative so a deep graph cannot blow the compiler's own
/// stack. Components come out in reverse topological order.
fn tarjan(edges: &[BTreeSet<usize>]) -> Vec<Vec<usize>> {
    #[derive(Clone, Copy)]
    struct Node {
        index: Option<usize>,
        low: usize,
        on_stack: bool,
    }

    let count = edges.len();
    let mut nodes = vec![
        Node {
            index: None,
            low: 0,
            on_stack: false,
        };
        count
    ];
    let mut next = 0usize;
    let mut stack: Vec<usize> = Vec::new();
    let mut sccs = Vec::new();

    for root in 0..count {
        if nodes[root].index.is_some() {
            continue;
        }
        // `(node, how many successors have been dealt with)`.
        let mut work: Vec<(usize, usize)> = vec![(root, 0)];
        nodes[root] = Node {
            index: Some(next),
            low: next,
            on_stack: true,
        };
        next += 1;
        stack.push(root);

        while let Some((node, step)) = work.pop() {
            let successors: Vec<usize> = edges[node].iter().copied().collect();
            if step < successors.len() {
                work.push((node, step + 1));
                let successor = successors[step];
                match nodes[successor].index {
                    None => {
                        nodes[successor] = Node {
                            index: Some(next),
                            low: next,
                            on_stack: true,
                        };
                        next += 1;
                        stack.push(successor);
                        work.push((successor, 0));
                    }
                    Some(index) if nodes[successor].on_stack => {
                        nodes[node].low = nodes[node].low.min(index);
                    }
                    Some(_) => {}
                }
                continue;
            }

            if nodes[node].low == nodes[node].index.unwrap_or_default() {
                let mut component = Vec::new();
                while let Some(member) = stack.pop() {
                    nodes[member].on_stack = false;
                    component.push(member);
                    if member == node {
                        break;
                    }
                }
                component.sort_unstable();
                sccs.push(component);
            }
            if let Some((parent, _)) = work.last().copied() {
                nodes[parent].low = nodes[parent].low.min(nodes[node].low);
            }
        }
    }

    sccs
}
