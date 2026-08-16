//! A two-ended register file — **a placeholder**, and it says so (§12).
//!
//! Locals are handed out from `$0` upward and expression temporaries from `$R9`
//! downward, with the temporary end reset at every statement. Ten lines, and the
//! one property worth having falls out of the shape rather than out of care: it
//! is structurally incapable of handing a user's variable out as a temporary,
//! which is nsL's issue #5.
//!
//! It also leaks — nothing is ever reclaimed — which is exactly why Phase 3
//! replaces it with liveness-based allocation (§9-3). The exhaustion diagnostic
//! says that rather than pretending twenty registers is the real limit.

/// `$0`–`$9` then `$R0`–`$R9`.
pub const COUNT: u8 = 20;

/// Where a value lives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Slot {
    /// A numbered register.
    Reg(u8),
    /// A `Var`: a global, which is one slot for the program's lifetime (§15.24).
    Global(String),
}

impl Slot {
    /// The NSIS spelling, `$` included.
    pub fn nsis(&self) -> String {
        match self {
            Slot::Reg(index) if *index < 10 => format!("${index}"),
            Slot::Reg(index) => format!("$R{}", index - 10),
            Slot::Global(name) => format!("${name}"),
        }
    }
}

/// The two ends are signed so that running off either one is a comparison
/// rather than a wrapping subtraction that silently starts working again.
#[derive(Clone, Debug)]
pub struct Registers {
    next_local: i16,
    next_temp: i16,
    /// The most temporaries alive at once, over the whole body. Phase 2's exit
    /// criterion is a claim about this number.
    high_water: usize,
}

impl Default for Registers {
    fn default() -> Self {
        Registers::new()
    }
}

impl Registers {
    pub fn new() -> Registers {
        Registers {
            next_local: 0,
            next_temp: i16::from(COUNT) - 1,
            high_water: 0,
        }
    }

    /// A register for a `local`, claimed *before* its initialiser is walked so
    /// that `local sum = 1 + 1` is one `IntOp` into the local rather than an
    /// `IntOp` into a temporary and a `StrCpy` after it (§12).
    pub fn local(&mut self) -> Option<Slot> {
        if self.next_local > self.next_temp {
            return None;
        }
        let index = self.next_local;
        self.next_local += 1;
        Some(Slot::Reg(index as u8))
    }

    /// A register for an intermediate value. Every one of these is a line of
    /// output a fused condition would not have needed.
    pub fn temp(&mut self) -> Option<Slot> {
        if self.next_temp < self.next_local {
            return None;
        }
        let index = self.next_temp;
        self.next_temp -= 1;
        let alive = usize::from(COUNT) - index as usize;
        self.high_water = self.high_water.max(alive);
        Some(Slot::Reg(index as u8))
    }

    /// Called at each statement boundary. Temporaries do not outlive the
    /// statement that produced them, and locals are never touched.
    pub fn end_statement(&mut self) {
        self.next_temp = i16::from(COUNT) - 1;
    }

    pub fn high_water(&self) -> usize {
        self.high_water
    }
}
