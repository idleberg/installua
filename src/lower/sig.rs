//! What the compiler knows about a name before it has finished reading the
//! program: `func` signatures and global types.
//!
//! Both are **whole-program** facts, and the language is order-free, so neither
//! can be learned by one pass in declaration order. A parameter's type comes
//! from the call sites, which are usually below the declaration; a return type
//! comes from the body, which the call site is above. The two directions meet
//! in the middle, and the middle is a fixpoint.
//!
//! There are no annotations to fall back on — types are inferred and never
//! declared — so this is not an optimisation that could be skipped. Without it
//! `countdown(n)` cannot know `n` is an int, and `n - 1` has no lowering.
//!
//! This is also a door closed deliberately: a signature that depends on every
//! call site in the program means there is no separately-compilable unit. An
//! installer is one program with one output, so nothing wants one.

use std::collections::BTreeMap;

use crate::resolve::Resolved;
use crate::types::Ty;

/// One `func`'s inferred type. `None` means *not observed yet*, which is a
/// different thing from [`Ty::Unknown`] — that is what two observations
/// disagreeing produces.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Signature {
    /// One entry per declared parameter, joined over every call site.
    pub params: Vec<Option<Ty>>,
    /// One entry per returned value, joined over every `return`. `None` until a
    /// `return` has been seen at all.
    pub returns: Option<Vec<Ty>>,
}

impl Signature {
    /// How many values the callee leaves on the stack. `None` while no `return`
    /// has been seen, which in the first round is every function.
    pub fn arity(&self) -> Option<usize> {
        self.returns.as_ref().map(Vec::len)
    }

    pub fn param(&self, index: usize) -> Ty {
        self.params
            .get(index)
            .copied()
            .flatten()
            .unwrap_or(Ty::Unknown)
    }

    pub fn result(&self, index: usize) -> Ty {
        self.returns
            .as_ref()
            .and_then(|types| types.get(index))
            .copied()
            .unwrap_or(Ty::Unknown)
    }
}

/// The fixpoint's state: everything one round of lowering learns and the next
/// round reads.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Inferred {
    pub signatures: BTreeMap<String, Signature>,
    /// A global's type, fixed for the program's lifetime. Carried through the
    /// fixpoint so that a body lowered before the assignment that types a
    /// global still sees the right type — the one thing Phase 2 left
    /// order-dependent.
    pub globals: BTreeMap<String, Ty>,
}

impl Inferred {
    /// Every `func` present, with nothing known about any of it.
    pub fn seed(resolved: &Resolved<'_>) -> Inferred {
        Inferred {
            signatures: resolved
                .functions
                .iter()
                .map(|(name, function)| {
                    (
                        name.clone(),
                        Signature {
                            params: vec![None; function.params.len()],
                            returns: None,
                        },
                    )
                })
                .collect(),
            globals: BTreeMap::new(),
        }
    }

    pub fn signature(&self, name: &str) -> Option<&Signature> {
        self.signatures.get(name)
    }

    /// Records what a call site says about a parameter.
    pub fn learn_param(&mut self, callee: &str, index: usize, ty: Ty) {
        if let Some(signature) = self.signatures.get_mut(callee)
            && let Some(slot) = signature.params.get_mut(index)
        {
            *slot = Some(join(*slot, ty));
        }
    }

    /// Records what one `return` says. Arity disagreements are reported by the
    /// lowerer, which has the spans; here the first arity seen wins so that the
    /// fixpoint still terminates on a program that will be rejected anyway.
    pub fn learn_return(&mut self, function: &str, types: &[Ty]) {
        let Some(signature) = self.signatures.get_mut(function) else {
            return;
        };
        match &mut signature.returns {
            None => signature.returns = Some(types.to_vec()),
            Some(existing) if existing.len() == types.len() => {
                for (slot, ty) in existing.iter_mut().zip(types) {
                    *slot = slot.join(*ty);
                }
            }
            Some(_) => {}
        }
    }
}

/// The join, with "not observed yet" as the unit. [`Ty::join`] cannot serve
/// directly because its `Unknown` is the lattice's *top* — two disagreeing
/// observations — and an absent observation is its bottom.
fn join(current: Option<Ty>, ty: Ty) -> Ty {
    match current {
        None => ty,
        Some(previous) => previous.join(ty),
    }
}
