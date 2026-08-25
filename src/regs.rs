//! Where a value lives, before and after allocation.
//!
//! Phase 2 handed registers out from both ends of the file and never reclaimed
//! one. That was a placeholder and said so. Here a body is lowered against an
//! **unbounded** supply of virtual slots, and [`crate::alloc`] colours them into
//! the twenty NSIS registers afterwards, from liveness.
//!
//! The ordering matters more than it looks. An allocator that runs *during*
//! lowering has to know a value's last use before it has seen it, so it either
//! guesses or tracks ownership by convention — and tracking ownership by
//! convention is precisely nsL's issue #5, where a user's variable is handed out
//! as a temporary. A virtual slot cannot be handed out twice, because it is
//! never handed back.

/// `$0`–`$9` then `$R0`–`$R9`.
pub const COUNT: u8 = 20;

/// Where a value lives.
///
/// [`Slot::Virtual`] is what lowering produces and what nothing else may see:
/// [`crate::alloc`] rewrites every one of them into a [`Slot::Reg`] before the
/// layout pass runs.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Slot {
    /// An unallocated value, numbered per body.
    Virtual(u32),
    /// A numbered register: `$0`–`$9`, `$R0`–`$R9`.
    Reg(u8),
    /// A `Var`: a global, which is one slot for the program's lifetime and is
    /// therefore never allocated, never coloured and — the part that is a
    /// correctness rule rather than an optimisation — never saved around a
    /// call.
    Global(String),
}

impl Slot {
    /// The NSIS spelling, `$` included.
    pub fn nsis(&self) -> String {
        match self {
            Slot::Reg(index) if *index < 10 => format!("${index}"),
            Slot::Reg(index) => format!("$R{}", index - 10),
            Slot::Global(name) => format!("${name}"),
            // Reaching the emitter with one of these is a compiler bug, not a
            // user error, so it is loud in debug and legible in release rather
            // than silently emitting something NSIS would accept.
            Slot::Virtual(index) => {
                debug_assert!(
                    false,
                    "virtual slot %{index} reached the output unallocated"
                );
                format!("$%{index}")
            }
        }
    }

    /// Allocated values only. A global is user-visible state, so it takes part
    /// in neither colouring nor caller-saving.
    pub fn virtual_index(&self) -> Option<u32> {
        match self {
            Slot::Virtual(index) => Some(*index),
            _ => None,
        }
    }

    pub fn register(&self) -> Option<u8> {
        match self {
            Slot::Reg(index) => Some(*index),
            _ => None,
        }
    }
}
