//! The type lattice.
//!
//! Four types, and `int` carries **attributes** rather than splitting into
//! siblings:
//!
//! ```text
//! int | string | bool | handle
//!
//! int { width: 32 | 64 | ptr,  sign: nonneg | unknown }
//! ```
//!
//! NSIS itself factors these as two axes — `IntCmp`, `IntCmpU`, `Int64Cmp`,
//! `Int64CmpU`, `IntPtrCmp`, `IntPtrCmpU` is a product, not six types — so
//! `width` picks the instruction family and `sign` picks the signed or unsigned
//! member of it. As types they would force every rule to enumerate the product.
//!
//! `sign` is the load-bearing one: it is an inferred interval rather than a
//! `uint` a user declares, and it is what lets the `//` fixup be elided rather
//! than emitted defensively at every division.

use std::fmt;

/// Which `IntCmp`/`IntOp` family a value belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Width {
    W32,
    /// No `Int64Op` exists, so 64-bit values compare and do not compute.
    W64,
    Ptr,
}

/// The three-point sign lattice. `⊥` is not represented, because a value with
/// no possible sign is a value that does not exist.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sign {
    NonNeg,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Int {
    pub width: Width,
    pub sign: Sign,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ty {
    Int(Int),
    Str,
    Bool,
    Handle,
    /// The lattice's top: two observations that disagree, or a call whose
    /// return type the fixpoint has not settled. Comparisons refuse it rather
    /// than guessing, which is the rule the lattice exists to state.
    Unknown,
}

impl Ty {
    /// A plain 32-bit integer of unknown sign: what a subtraction produces.
    pub const fn int() -> Ty {
        Ty::Int(Int {
            width: Width::W32,
            sign: Sign::Unknown,
        })
    }

    /// A 32-bit integer known not to be negative: a literal `7`, a `StrLen`, a
    /// `for` bound. This is the case that elides a fixup.
    pub const fn nonneg() -> Ty {
        Ty::Int(Int {
            width: Width::W32,
            sign: Sign::NonNeg,
        })
    }

    /// A literal's type. Its sign is known, and so is its width: NSIS's 32-bit
    /// `IntCmp` reads `0x100000000` as 0, so a value outside what 32 bits can
    /// spell, either way round, is compared with `Int64Cmp`.
    pub fn literal(value: i64) -> Ty {
        let width = if (i64::from(i32::MIN)..=i64::from(u32::MAX)).contains(&value) {
            Width::W32
        } else {
            Width::W64
        };
        let sign = if value >= 0 {
            Sign::NonNeg
        } else {
            Sign::Unknown
        };
        Ty::Int(Int { width, sign })
    }

    pub fn as_int(self) -> Option<Int> {
        match self {
            Ty::Int(int) => Some(int),
            _ => None,
        }
    }

    pub fn is_int(self) -> bool {
        matches!(self, Ty::Int(_))
    }

    /// A 64-bit int: one NSIS can compare, through `System::Int64Op`, and
    /// cannot compute with, since `IntOp` is 32-bit.
    pub fn is_int64(self) -> bool {
        self.as_int().is_some_and(|int| int.width == Width::W64)
    }

    /// The lattice join, used where two paths reach one slot.
    pub fn join(self, other: Ty) -> Ty {
        match (self, other) {
            (a, b) if a == b => a,
            (Ty::Int(a), Ty::Int(b)) => Ty::Int(Int {
                width: a.width.join(b.width),
                sign: a.sign.join(b.sign),
            }),
            _ => Ty::Unknown,
        }
    }

    /// The join, or `None` where it is a conflict: two known types that
    /// disagree, or a known one meeting `Unknown`. A slot already holding
    /// `Unknown` stays `Unknown` without a second report.
    pub fn try_join(self, other: Ty) -> Option<Ty> {
        match self.join(other) {
            Ty::Unknown if self != other => None,
            joined => Some(joined),
        }
    }
}

impl Width {
    fn join(self, other: Width) -> Width {
        match (self, other) {
            (a, b) if a == b => a,
            // A pointer joined with anything is still addressed as a pointer;
            // otherwise widen, since a 32-bit value fits in a 64-bit compare and
            // not the reverse.
            (Width::Ptr, _) | (_, Width::Ptr) => Width::Ptr,
            _ => Width::W64,
        }
    }
}

impl Sign {
    fn join(self, other: Sign) -> Sign {
        match (self, other) {
            (Sign::NonNeg, Sign::NonNeg) => Sign::NonNeg,
            _ => Sign::Unknown,
        }
    }
}

/// What a diagnostic calls the type. Deliberately not `Debug`: `Int(Int { width:
/// W32, .. })` is the compiler's spelling and `int` is the user's.
impl fmt::Display for Ty {
    /// `{:#}` puts the article in front — `an int`, `a string` — so a message
    /// never has to write `a {}` and get `a int`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if f.alternate() {
            let article = match self {
                Ty::Int(_) | Ty::Unknown => "an",
                _ => "a",
            };
            write!(f, "{article} ")?;
        }
        match self {
            Ty::Int(int) => match int.width {
                Width::W32 => f.write_str("int"),
                Width::W64 => f.write_str("int (64-bit)"),
                Width::Ptr => f.write_str("int (pointer-width)"),
            },
            Ty::Str => f.write_str("string"),
            Ty::Bool => f.write_str("bool"),
            Ty::Handle => f.write_str("handle"),
            Ty::Unknown => f.write_str("unknown"),
        }
    }
}
