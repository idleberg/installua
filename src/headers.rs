//! Declared foreign macros: what `import "FileFunc"` brings into scope (§15.27).
//!
//! **This is a seed, not the mechanism.** §15.27 rules that a header's macros
//! need a *declaration* rather than a discovery pass — nothing can read
//! `FileFunc.nsh` and recover that `${GetSize}` writes three registers and
//! takes two arguments, because an `!insertmacro` parameter list carries no
//! directions — and that declarations live in `.installua/headers/`. Reading
//! that directory is Phase 5's; this is the subset the five programs reach,
//! written in the shape those files will parse into (§15.17).
//!
//! Every macro here shares one calling convention, and it is not a choice this
//! compiler made: a header macro takes its inputs first and its **outputs as
//! trailing register arguments**, because `!insertmacro` has no way to return a
//! value. That is why an output is a `Param` position rather than a `returns`
//! field the way an instruction's is (§15.23).

use crate::types::Ty;

/// One macro argument. Deliberately *not* [`crate::table::Param`]: an
/// instruction's parameter is a row of `-CMDHELP` with a direction, optionality
/// and enum members, and a macro's is a positional slot in an `!insertmacro`
/// with none of those. Sharing the struct would mean carrying four fields that
/// can never be anything but their defaults.
#[derive(Clone, Copy, Debug)]
pub struct Param {
    pub ty: Ty,
    /// A path position: `/` is normalised to `\` (§5).
    pub path: bool,
}

pub struct Macro {
    /// The header it comes from, without the `.nsh`.
    pub header: &'static str,
    /// The method name as written after the namespace: `fileFunc.getSize`.
    pub installua: &'static str,
    /// The macro name, without the `${}`.
    pub nsis: &'static str,
    pub params: &'static [Param],
    /// One per trailing output register, in the order the macro writes them.
    pub outputs: &'static [Ty],
}

const fn data(ty: Ty) -> Param {
    Param { ty, path: false }
}

const fn path() -> Param {
    Param {
        ty: Ty::Str,
        path: true,
    }
}

pub const MACROS: &[Macro] = &[
    Macro {
        header: "FileFunc",
        installua: "getSize",
        nsis: "GetSize",
        params: &[path(), data(Ty::Str)],
        // Size, files, directories. A byte count cannot be negative, and that
        // is what makes `size // 1024` a bare `IntOp` with no sign fixup
        // (§15.4) — the one place program 4's README says the lattice pays.
        outputs: &[Ty::nonneg(), Ty::nonneg(), Ty::nonneg()],
    },
    Macro {
        header: "FileFunc",
        installua: "driveSpace",
        nsis: "DriveSpace",
        params: &[path(), data(Ty::Str)],
        outputs: &[Ty::nonneg()],
    },
    Macro {
        header: "WordFunc",
        installua: "versionCompare",
        nsis: "VersionCompare",
        params: &[data(Ty::Str), data(Ty::Str)],
        // `"0"`, `"1"` or `"2"` — a string, because that is what the macro
        // leaves in the register and comparing it as an int would be a guess
        // the lattice has no evidence for.
        outputs: &[Ty::Str],
    },
];

/// One declared plugin method (§11).
///
/// A plugin's **output count** is the thing a declaration exists for. NSIS
/// gives no way to ask a DLL how many values it pushes, and getting it wrong
/// unbalances the stack with no diagnostic from anybody — which is why `local
/// rc, out = nsExec.execToStack(…)` is legal at all: the two come from here.
pub struct PluginMethod {
    pub plugin: &'static str,
    pub installua: &'static str,
    /// The full `Plugin::Method` spelling.
    pub nsis: &'static str,
    pub params: &'static [Param],
    /// One per value the plugin leaves on the stack, in `Pop` order.
    pub outputs: &'static [Ty],
}

pub const PLUGINS: &[PluginMethod] = &[
    PluginMethod {
        plugin: "nsExec",
        installua: "execToStack",
        nsis: "nsExec::ExecToStack",
        params: &[data(Ty::Str)],
        // The exit code first, then the captured output — `Pop` order, which is
        // the order the declaration has to state and the source cannot see.
        outputs: &[Ty::Str, Ty::Str],
    },
    PluginMethod {
        plugin: "UserInfo",
        installua: "getAccountType",
        nsis: "UserInfo::GetAccountType",
        params: &[],
        outputs: &[Ty::Str],
    },
    PluginMethod {
        plugin: "System",
        installua: "call",
        nsis: "System::Call",
        params: &[data(Ty::Str)],
        // Counted from the signature instead: every `.s` in it pushes one
        // value. Parsing the rest of a `System::Call` signature — which would
        // narrow the clobber set from "everything" — is deferred (PLAN §3).
        outputs: &[],
    },
];

pub fn plugin(plugin: &str, method: &str) -> Option<&'static PluginMethod> {
    PLUGINS
        .iter()
        .find(|entry| entry.plugin == plugin && entry.installua == method)
}

pub fn plugin_methods(plugin: &str) -> Vec<&'static str> {
    PLUGINS
        .iter()
        .filter(|entry| entry.plugin == plugin)
        .map(|entry| entry.installua)
        .collect()
}

/// A macro by header and method name. The header is half of the key because two
/// headers may spell the same method differently, and §13's namespace boundary
/// is what makes that a fact rather than a hazard.
pub fn lookup(header: &str, method: &str) -> Option<&'static Macro> {
    MACROS
        .iter()
        .find(|entry| entry.header == header && entry.installua == method)
}

/// Every method a header declares, for the diagnostic that names them.
pub fn methods(header: &str) -> Vec<&'static str> {
    MACROS
        .iter()
        .filter(|entry| entry.header == header)
        .map(|entry| entry.installua)
        .collect()
}

/// Whether anything is declared for `header` at all. A header nobody has
/// declared is not an error — `import` still emits the `!include`, and `raw`
/// can reach whatever is in it — but calling a method on it cannot work.
pub fn known(header: &str) -> bool {
    MACROS.iter().any(|entry| entry.header == header)
}
