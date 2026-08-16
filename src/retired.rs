//! The NSIS-facing migration table, as a diagnostic rather than a page (§5).
//!
//! Some NSIS instructions are deliberately not ported, because Lua already has
//! the better spelling: `StrCmp` is `==`, `IntOp` is `+`, `StrCpy` is
//! assignment. Without this table they resolve to nothing and get the generic
//! unknown-name error, which tells an NSIS user that the compiler has never
//! heard of the single instruction they use most.
//!
//! ```text
//! error[nsis-retired]: `StrCmp` is not a function here
//!   note: write `==`, which is case-sensitive (§15.9)
//! ```
//!
//! **Keyed on the Installua spelling, matched case-insensitively.** The rows
//! are `strCmp`, `intOp`, `strCpy` — the camelCase shape the Lua equivalent
//! *would* have had, not NSIS's `StrCmp` — because that is the convention
//! [`crate::builtins::nearest`] already uses for the names that do exist, so
//! one casing rule covers both tables. The case-insensitive match still catches
//! the capitalisation the user actually typed, and the diagnostic quotes their
//! spelling rather than the table's.
//!
//! **There is no second table.** A row *is* a `LoweringTarget` or `Rejected`
//! entry in [`crate::table`], read through the class's own text: the census
//! already requires every command to carry a reason, and this is that reason
//! reaching the person who needs it. Nothing here feeds the stubs or the selene
//! std — a retired name must not autocomplete, or it is being taught rather
//! than retired.

use crate::table::{self, Class};

/// What a retired name maps to.
pub struct Retired {
    /// The camelCase spelling a user would have typed.
    pub installua: String,
    /// The NSIS name, for the diagnostic's second line.
    pub nsis: &'static str,
    /// What to write instead.
    pub instead: &'static str,
}

/// The camelCase of an NSIS name is occasionally a Lua keyword — `Goto` is
/// `goto` — and those are gone before lowering ever sees a name: the frontend
/// rejects the *keyword* with its own code and its own note (§8 owns labels).
/// A row here would be unreachable, and an unreachable row is a diagnostic
/// nobody can test.
const KEYWORDS: &[&str] = &["goto", "return", "function", "while", "repeat", "not"];

/// Looks up a name a user typed. Case-insensitive, so `StrCmp`, `strcmp` and
/// `strCmp` all land here.
pub fn lookup(name: &str) -> Option<Retired> {
    all()
        .into_iter()
        .find(|row| row.installua.eq_ignore_ascii_case(name))
}

/// `StrCmp` ⇒ `strCmp`. Only the first letter moves: `IntPtrOp` is `intPtrOp`
/// and not `intptrOp`, because the camelCase shape a Lua equivalent would have
/// had is the NSIS name with a lowercase initial, which is exactly the
/// convention the exposed names already follow (`detailPrint`, `writeReg`).
fn camel(nsis: &str) -> String {
    let mut chars = nsis.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Every retired name, for the registry test and for nothing else.
pub fn all() -> Vec<Retired> {
    table::table()
        .iter()
        .filter_map(|entry| {
            let instead = match entry.class {
                Class::LoweringTarget(instead) | Class::Rejected(instead) => instead,
                _ => return None,
            };
            let installua = camel(entry.nsis);
            if KEYWORDS.contains(&installua.as_str()) {
                return None;
            }
            Some(Retired {
                installua,
                nsis: entry.nsis,
                instead,
            })
        })
        .collect()
}
