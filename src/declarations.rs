//! Declared foreign macros and plugin methods: what `import "FileFunc"` and
//! `plugin "nsExec"` bring into scope.
//!
//! A header's macros and a plugin's methods need a *declaration* rather than a
//! discovery pass. Nothing can read `FileFunc.nsh` and recover that `${GetSize}`
//! writes three registers and takes two arguments, because an `!insertmacro`
//! parameter list carries no directions; and nothing can ask a DLL how many
//! values it pushes, because NSIS offers no way to ask. Both are written down.
//!
//! Two sources, one table and **one format**. [`Declarations::builtin`] parses
//! the files in [`SHIPPED`] and [`Declarations::load`] parses whatever
//! `.installua/declarations/*.toml` holds, through the same parser — so what ships
//! here is not a privileged kind of declaration, it is the same five lines a
//! project writes, and adding a plugin is a `.toml` file rather than a code
//! change. A project may redeclare one of ours and wins when it does, because a
//! wrong count shipped here must not be a wall.
//!
//! That is what makes a *third-party* plugin ordinary rather than a special
//! case: the compiler checks its arity, the editor stubs type its calls, and
//! neither half is hand-maintained.
//!
//! Every macro shares one calling convention, and it is not a choice this
//! compiler made: a header macro takes its inputs first and its **outputs as
//! trailing register arguments**, because `!insertmacro` has no way to return a
//! value. That is why an output is a `Param` position rather than a `returns`
//! field the way an instruction's is. A plugin's outputs are on the stack
//! instead, in `Pop` order — which is why the two are separate lists here and
//! not one shape with a flag.
//!
//! The stack is also why only a *plugin* can have a variable arity. A macro's
//! outputs are registers written on every path; a plugin's are a depth, and a
//! great many plugins vary it — see [`PluginMethod::tagged`].

use std::path::Path;

use crate::types::Ty;

/// One macro or method argument. Deliberately *not* [`crate::table::Param`]: an
/// instruction's parameter is a row of `-CMDHELP` with a direction, optionality
/// and enum members, and a macro's is a positional slot in an `!insertmacro`
/// with none of those. Sharing the struct would mean carrying four fields that
/// can never be anything but their defaults.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Param {
    pub ty: Ty,
    /// A path position: `/` is normalised to `\`.
    pub path: bool,
    /// A function position: the macro takes an address here, and NSIS calls
    /// back into it.
    ///
    /// **This says only that a function goes here.** It does not say how the
    /// callback receives its arguments, because that is not a signature: NSIS
    /// hands them over in named registers — `$R9` down to `$R6` for `Locate`,
    /// `$9` down to `$6` for `TextCompare` — and the answer comes back as a
    /// pushed sentinel, with `LineFind` writing `$R9` on the way out as well.
    /// A register map is behaviour, and behaviour does not go in a declaration
    /// file: one written down wrongly compiles, assembles, and hands the caller
    /// a directory where it asked for a file name, with no diagnostic possible
    /// from here or from NSIS.
    ///
    /// So the map lives in [`crate::lower::callback`], keyed by the `nsis`
    /// name, and a declaration naming a macro that table does not know is an
    /// error that says so. That costs third-party callback macros — there is no
    /// way to declare one — and buys the guarantee that no register is ever
    /// spelled outside the compiler.
    pub callback: bool,
}

/// How a flag carries its value.
///
/// Three spellings because the corpus has three, and no plugin lets you pick:
/// `NSISdl::download /TIMEOUT=5000` glues, `nsisunz::Unzip /text "Extracting"`
/// does not, and `AccessControl::GrantOnFile /noinherit` has nothing to carry.
/// Which one a flag uses is the plugin's, so it is written down beside it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Carry {
    /// The flag word alone. The call site writes `true` or `false`, and `false`
    /// writes nothing — the flag *is* the value.
    Bare,
    /// Glued with `=`, as `/TIMEOUT=5000`.
    Joined,
    /// A token of its own, after the flag: `/text "…"`.
    Separate,
}

/// One leading flag a plugin method accepts.
///
/// **Flags are leading and unordered, and that is why they are a table rather
/// than parameters.** Every flag on every plugin declared here comes ahead of
/// the fixed arguments — `AccessControl::GrantOnFile /noinherit "$INSTDIR" …` —
/// and no plugin gives two of them a meaningful order. So the call site names
/// them and this list places them: what the user writes last is emitted first,
/// in the order declared here, and a call that names none is character for
/// character the call it was before flags existed.
///
/// A `Vec<Flag>` rather than a map for the reason [`PluginMethod::tagged`] is a
/// list: nothing name-keyed reaches the lowerer, and a declaration's order is
/// the only order there is to emit in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Flag {
    /// The options-table key: `{ noinherit = true }`.
    pub name: String,
    /// The flag as NSIS spells it, `/` and all. Written rather than derived
    /// from [`name`](Self::name) for the reason `nsis` is on every block:
    /// `/TIMEOUT` is upper, `/checknoshortcuts` is lower, and a compiler that
    /// guessed would emit a flag the plugin silently takes for a positional
    /// argument.
    pub nsis: String,
    /// The value's type, and [`Ty::Bool`] for a [`Carry::Bare`] flag.
    pub ty: Ty,
    /// A path value: `/` is normalised to `\`, as in a [`Param`].
    pub path: bool,
    pub carry: Carry,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Macro {
    /// Whether a project declared this, as opposed to it shipping here.
    ///
    /// Provenance rather than decoration: replacing a builtin is the point, and
    /// replacing a declaration the same project already made is a mistake, so
    /// the two are told apart by where an entry came from and not by what it is
    /// called.
    pub project: bool,
    /// The header it comes from, without the `.nsh`.
    pub header: String,
    /// The method name as written after the namespace: `fileFunc.getSize`.
    pub installua: String,
    /// The macro name, without the `${}`.
    pub nsis: String,
    pub params: Vec<Param>,
    /// One per trailing output register, in the order the macro writes them.
    pub outputs: Vec<Ty>,
}

/// One declared plugin method.
///
/// A plugin's **output count** is the thing a declaration exists for. NSIS
/// gives no way to ask a DLL how many values it pushes, and getting it wrong
/// unbalances the stack with no diagnostic from anybody — which is why `local
/// rc, out = nsExec.execToStack(…)` is legal at all: the two come from here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginMethod {
    /// Whether a project declared this. See [`Macro::project`].
    pub project: bool,
    pub plugin: String,
    pub installua: String,
    /// The full `Plugin::Method` spelling.
    pub nsis: String,
    pub params: Vec<Param>,
    /// One per value the plugin leaves on the stack **on every path**, in `Pop`
    /// order.
    pub outputs: Vec<Ty>,
    /// The leading flags this method accepts, in the order they are emitted.
    /// Empty for most methods; see [`Flag`].
    pub flags: Vec<Flag>,
    /// First-popped values that mean [`more`](Self::more) follows.
    ///
    /// A great many plugins push a *variable* number of values, and the first
    /// one says how many: `AccessControl::GrantOnFile` pushes `"ok"` alone or
    /// `"error"` and a message, and `StartMenu::Select` pushes `"success"`
    /// **and** a folder or one of `"cancel"` and an error alone. Without a
    /// spelling for that, a declaration has to pick a path and be wrong on the
    /// other — which is not a wrong *type*, it is an unbalanced stack, and
    /// NSIS diagnoses neither.
    ///
    /// A **list of literals** rather than a flag, because the polarity is the
    /// plugin's to choose and the two above chose opposite ones: hardcoding
    /// `"error"` would describe AccessControl and misdescribe StartMenu by
    /// exactly one value. And a list of literals rather than a predicate,
    /// because the test has to be one the compiler can emit — a `StrCmpS`
    /// against a constant — not a parse of whatever the tag happens to say.
    ///
    /// Empty when the arity is fixed, which is the common case and the one
    /// every field above was written for.
    pub tagged: Vec<String>,
    /// The values that follow when the first popped one is in
    /// [`tagged`](Self::tagged). Empty exactly when `tagged` is.
    pub more: Vec<Ty>,
    /// Where the DLL lives, when it is not in `NSISDIR/Plugins`. Relative to
    /// the project root; [`crate::lower::addplugindir`] absolutises it.
    ///
    /// **The one field that is about a file rather than a signature**, and it
    /// has to be: a plugin outside `NSISDIR` is unreachable otherwise, and both
    /// halves of what the compiler emits fail on it — `Plugin not found` at the
    /// call site, `no files found` at the `ReserveFile /plugin`. It sits on the
    /// declaration rather than in `installua.toml` so that adding a vendored
    /// plugin stays one file, and per block rather than project-wide so that
    /// only a plugin the program actually calls costs an `!addplugindir` line.
    pub dir: Option<String>,
}

/// Everything declared for one compilation: the builtins, plus whatever the
/// project added.
///
/// Owned rather than `&'static`, because a project's declarations are read at
/// run time and this table is per-compilation — two sources compiled
/// concurrently in one process may declare different plugins, and a static
/// table could not tell them apart. It travels in [`crate::Options`] for the
/// same reason `include`'s loader does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Declarations {
    macros: Vec<Macro>,
    plugins: Vec<PluginMethod>,
}

/// Something wrong with a declaration file, located well enough to fix.
///
/// Not a [`crate::diag::Diagnostic`]: those carry a [`crate::diag::Span`] into
/// the Lua sources, and a malformed `.toml` is in none of them. The CLI prints
/// these before it compiles anything, because a program checked against
/// half-loaded declarations is checked against the wrong language.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    pub file: String,
    /// 1-based, and `0` when the problem is the file rather than a line in it.
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.line == 0 {
            write!(f, "{}: {}", self.file, self.message)
        } else {
            write!(f, "{}:{}: {}", self.file, self.line, self.message)
        }
    }
}

/// The declarations that ship with the compiler, as the files they are.
///
/// The *same* format a project writes, parsed by the same parser: one shape for
/// one idea, rather than a `const` table here and a file format there that drift
/// the first time either grows a field. Three things follow from it — a plugin
/// added here is five lines of TOML and no Rust, the parser is exercised by this
/// crate's own data on every run, and the files can be copied into a project's
/// `.installua/declarations/` verbatim by anyone who needs to correct one.
///
/// The cost is parsing a few kilobytes per [`Declarations::builtin`], and that a
/// malformed file here would be a run-time surprise rather than a compile error
/// — which `tests/declarations.rs` turns back into a build failure by asserting that
/// this parses clean.
///
/// # What earns a slot here
///
/// **Ship a declaration when the fact is undiscoverable and the caller is
/// common.** Both halves are load-bearing, and each rules out a different kind
/// of entry.
///
/// *Undiscoverable* is the sentence this module opens with: a plugin's output
/// count is a fact nothing can recover — not the compiler, not the reader, not
/// NSIS — and getting it wrong unbalances the stack with no diagnostic from
/// anybody. That is why every plugin whose arity is the whole story belongs
/// here even if few projects call it. `${GetParent}`'s
/// one-string-in-one-register-out, by contrast, is rediscoverable from
/// `FileFunc.nsh` in a minute; it earns its slot on the second half instead,
/// because withholding something every `.onInit` needs is merely rude.
///
/// *Common* is what keeps this from becoming a mirror of `NSISDIR/Include`.
/// A macro nobody reaches for is a project's own `.installua/declarations/` file,
/// and the format is identical precisely so that costs a project nothing.
///
/// The rule cuts one way that is easy to miss: a header whose value is
/// **behaviour rather than arity** does not become a `.toml` however common it
/// is. `Library.nsh` is the case — `SetOverwrite`, reference counting, the
/// reboot flag and shared-DLL bookkeeping are not a signature, and a
/// declaration that described only its parameter list would be a correct file
/// documenting the wrong thing. Those are a compiler-owned lowering or nothing.
///
/// `FileFunc.getSize` predates the rule and does not quite meet it — its own
/// comment says it is here because a byte count proved the `uint` lattice.
/// It stays because it is now documented and called; the rule is what the
/// *next* entry is measured against.
const SHIPPED: &[(&str, &str)] = &[
    (
        "AccessControl.toml",
        include_str!("declarations/AccessControl.toml"),
    ),
    (
        "AdvSplash.toml",
        include_str!("declarations/AdvSplash.toml"),
    ),
    ("Banner.toml", include_str!("declarations/Banner.toml")),
    ("Dialer.toml", include_str!("declarations/Dialer.toml")),
    ("EnVar.toml", include_str!("declarations/EnVar.toml")),
    ("FileFunc.toml", include_str!("declarations/FileFunc.toml")),
    ("NSISdl.toml", include_str!("declarations/NSISdl.toml")),
    ("Nsis7z.toml", include_str!("declarations/Nsis7z.toml")),
    ("SimpleSC.toml", include_str!("declarations/SimpleSC.toml")),
    ("Splash.toml", include_str!("declarations/Splash.toml")),
    (
        "StartMenu.toml",
        include_str!("declarations/StartMenu.toml"),
    ),
    ("System.toml", include_str!("declarations/System.toml")),
    ("TextFunc.toml", include_str!("declarations/TextFunc.toml")),
    ("TypeLib.toml", include_str!("declarations/TypeLib.toml")),
    ("UserInfo.toml", include_str!("declarations/UserInfo.toml")),
    ("VPatch.toml", include_str!("declarations/VPatch.toml")),
    ("WordFunc.toml", include_str!("declarations/WordFunc.toml")),
    ("nsExec.toml", include_str!("declarations/nsExec.toml")),
    (
        "nsProcess.toml",
        include_str!("declarations/nsProcess.toml"),
    ),
    (
        "nsisFirewall.toml",
        include_str!("declarations/nsisFirewall.toml"),
    ),
];

/// Where a project's own declarations live, relative to its root.
pub const DIRECTORY: &str = ".installua/declarations";

impl Default for Declarations {
    fn default() -> Declarations {
        Declarations::builtin()
    }
}

impl Declarations {
    /// The declarations that ship with the compiler, from [`SHIPPED`].
    ///
    /// Problems are dropped rather than returned, because there is nothing a
    /// caller could do about a file compiled into the binary: the check that
    /// matters happens in `tests/declarations.rs`, where a malformed shipped file
    /// fails the build instead of the installer.
    pub fn builtin() -> Declarations {
        Declarations::shipped().0
    }

    /// The same, with whatever the shipped files got wrong — the half the test
    /// reads, and the reason [`builtin`](Declarations::builtin) can throw the
    /// problems away.
    pub fn shipped() -> (Declarations, Vec<Problem>) {
        let mut declarations = Declarations {
            macros: Vec::new(),
            plugins: Vec::new(),
        };
        let mut problems = Vec::new();
        for (name, text) in SHIPPED {
            declarations.parse(name, text, &mut problems);
        }

        // Parsed as a project's files are — which is what catches two shipped
        // files declaring one method, since the second reports — and then
        // marked as ours. Provenance is what decides whether a *project*
        // redeclaring this is a correction or a mistake, and everything up to
        // here was read out of a file that could equally have been a project's.
        for entry in &mut declarations.macros {
            entry.project = false;
        }
        for entry in &mut declarations.plugins {
            entry.project = false;
        }
        (declarations, problems)
    }

    /// The builtins plus every `*.toml` in `dir`, in file-name order.
    ///
    /// A missing directory is not a problem: a project that declares nothing is
    /// the common case, and an error there would make `.installua/declarations/`
    /// mandatory ceremony. A directory that cannot be *read* is a problem,
    /// because that is a permission or a typo rather than an absence.
    pub fn load(dir: &Path) -> (Declarations, Vec<Problem>) {
        let mut declarations = Declarations::builtin();
        let mut problems = Vec::new();

        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return (declarations, problems);
            }
            Err(error) => {
                problems.push(Problem {
                    file: dir.display().to_string(),
                    line: 0,
                    message: format!("cannot read: {error}"),
                });
                return (declarations, problems);
            }
        };

        // Sorted, because two files may declare the same method and "the later
        // one wins" has to mean the same thing on every machine.
        let mut paths: Vec<_> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "toml"))
            .collect();
        paths.sort();

        for path in paths {
            let name = path.display().to_string();
            match std::fs::read_to_string(&path) {
                Ok(text) => declarations.parse(&name, &text, &mut problems),
                Err(error) => problems.push(Problem {
                    file: name,
                    line: 0,
                    message: format!("cannot read: {error}"),
                }),
            }
        }

        (declarations, problems)
    }

    /// Adds everything one declaration file declares. Public so that a caller
    /// with the text already in hand — an editor, a test — never has to touch
    /// the file system to get the answer the CLI gets.
    pub fn parse(&mut self, file: &str, text: &str, problems: &mut Vec<Problem>) {
        for record in parse::records(file, text, problems) {
            self.insert(record, problems);
        }
    }

    fn insert(&mut self, record: parse::Record, problems: &mut Vec<Problem>) {
        let parse::Record {
            plugin,
            name,
            method,
            nsis,
            params,
            outputs,
            flags,
            tagged,
            more,
            dir,
            file,
            line,
        } = record;

        // A project file replacing a *builtin* is the point; two project files
        // disagreeing is a mistake nobody makes on purpose, so the second says
        // so and still wins — refusing would leave the compiler holding
        // whichever file happened to sort first.
        if plugin {
            if self
                .plugin(&name, &method)
                .is_some_and(|existing| existing.project)
            {
                problems.push(Problem {
                    file,
                    line,
                    message: format!("`{name}.{method}` is already declared; this one replaces it"),
                });
            }
            self.plugins
                .retain(|entry| entry.plugin != name || entry.installua != method);
            self.plugins.push(PluginMethod {
                project: true,
                plugin: name,
                installua: method,
                nsis,
                params,
                outputs,
                flags,
                tagged,
                more,
                dir,
            });
        } else {
            if self
                .lookup(&name, &method)
                .is_some_and(|existing| existing.project)
            {
                problems.push(Problem {
                    file,
                    line,
                    message: format!("`{name}.{method}` is already declared; this one replaces it"),
                });
            }
            self.macros
                .retain(|entry| entry.header != name || entry.installua != method);
            self.macros.push(Macro {
                project: true,
                header: name,
                installua: method,
                nsis,
                params,
                outputs,
            });
        }
    }

    /// A macro by header and method name. The header is half of the key because
    /// two headers may spell the same method differently, and NSIS's namespace
    /// boundary is what makes that a fact rather than a hazard.
    pub fn lookup(&self, header: &str, method: &str) -> Option<&Macro> {
        self.macros
            .iter()
            .find(|entry| entry.header == header && entry.installua == method)
    }

    /// Every method a header declares, for the diagnostic that names them.
    pub fn methods(&self, header: &str) -> Vec<&str> {
        self.macros
            .iter()
            .filter(|entry| entry.header == header)
            .map(|entry| entry.installua.as_str())
            .collect()
    }

    /// Whether anything is declared for `header` at all. A header nobody has
    /// declared is not an error — `import` still emits the `!include`, and `raw`
    /// can reach whatever is in it — but calling a method on it cannot work.
    pub fn known(&self, header: &str) -> bool {
        self.macros.iter().any(|entry| entry.header == header)
    }

    pub fn plugin(&self, plugin: &str, method: &str) -> Option<&PluginMethod> {
        self.plugins
            .iter()
            .find(|entry| entry.plugin == plugin && entry.installua == method)
    }

    pub fn plugin_methods(&self, plugin: &str) -> Vec<&str> {
        self.plugins
            .iter()
            .filter(|entry| entry.plugin == plugin)
            .map(|entry| entry.installua.as_str())
            .collect()
    }

    /// Every declared macro, for the stub generator. Sorted, because a stub is
    /// a generated file read in diffs and `read_dir` order is not stable across
    /// machines.
    pub fn all_macros(&self) -> Vec<&Macro> {
        let mut all: Vec<&Macro> = self.macros.iter().collect();
        all.sort_by(|a, b| (&a.header, &a.installua).cmp(&(&b.header, &b.installua)));
        all
    }

    pub fn all_plugins(&self) -> Vec<&PluginMethod> {
        let mut all: Vec<&PluginMethod> = self.plugins.iter().collect();
        all.sort_by(|a, b| (&a.plugin, &a.installua).cmp(&(&b.plugin, &b.installua)));
        all
    }

    /// The header names anything is declared for, in sorted order.
    pub fn header_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.macros.iter().map(|e| e.header.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        names
    }

    /// Every directory declared for `plugin`, in sorted order.
    ///
    /// A list rather than an `Option`, because [`PluginMethod::dir`] is per
    /// block and a plugin is as many blocks as it has methods. Two blocks
    /// naming two directories is not an error to report here — `!addplugindir`
    /// is a search path and NSIS is happy with several — so both are emitted
    /// and the DLL is found in whichever one holds it.
    pub fn plugin_dirs(&self, plugin: &str) -> Vec<&str> {
        let mut dirs: Vec<&str> = self
            .plugins
            .iter()
            .filter(|entry| entry.plugin == plugin)
            .filter_map(|entry| entry.dir.as_deref())
            .collect();
        dirs.sort_unstable();
        dirs.dedup();
        dirs
    }

    /// The plugin names anything is declared for, in sorted order.
    pub fn plugin_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.plugins.iter().map(|e| e.plugin.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        names
    }
}

/// The declaration file format.
///
/// A deliberately small subset of TOML — array-of-tables headers and flat
/// `key = value` lines, where a value is a string, an array of strings, or an
/// array of inline tables — parsed here rather than by a dependency. The
/// messages can name the field the user got wrong, and a compiler that reads
/// six keys does not need a deserialiser to do it.
///
/// `flags` is what bought the last two shapes, and it is worth saying what it
/// cost. Before it, a value was one line and held no structure, and the module
/// could claim a full parser would buy syntax the format does not use. A flag
/// is four fields — [`Flag::name`], `nsis`, a type and a
/// [`Carry`](super::Carry) — so it is a record however it is written, and the
/// alternative spelling was a `[[plugin.flag]]` sub-block per flag, which needs
/// no new syntax at all. That lost on the files rather than on the parser:
/// `AccessControl` has 22 methods taking two flags each, and 44 four-line
/// blocks would have doubled the file and stopped `[[plugin]]` from meaning
/// "here is a method". Two shapes here buy one line per flag there, and the
/// files stay valid TOML either way.
mod parse {
    use super::{Carry, Flag, Param, Problem};
    use crate::types::{Int, Sign, Ty, Width};

    /// One `[[plugin]]` or `[[header]]` block, checked.
    pub struct Record {
        pub plugin: bool,
        pub name: String,
        pub method: String,
        pub nsis: String,
        pub params: Vec<Param>,
        pub outputs: Vec<Ty>,
        pub flags: Vec<Flag>,
        pub tagged: Vec<String>,
        pub more: Vec<Ty>,
        pub dir: Option<String>,
        pub file: String,
        /// The line the block opened on, so "already declared" points at the
        /// block rather than at whichever field happened to be last.
        pub line: usize,
    }

    /// A block being filled in, before its required fields are checked.
    #[derive(Default)]
    struct Pending {
        plugin: bool,
        line: usize,
        name: Option<String>,
        method: Option<String>,
        nsis: Option<String>,
        params: Option<Vec<Param>>,
        outputs: Option<Vec<Ty>>,
        flags: Option<Vec<Flag>>,
        tagged: Option<Vec<String>>,
        more: Option<Vec<Ty>>,
        dir: Option<String>,
    }

    pub fn records(file: &str, text: &str, problems: &mut Vec<Problem>) -> Vec<Record> {
        let mut records = Vec::new();
        let mut pending: Option<Pending> = None;

        let mut complain = |line: usize, message: String| {
            problems.push(Problem {
                file: file.to_string(),
                line,
                message,
            });
        };

        for (line, joined) in logical(text) {
            let content = joined.as_str();
            if content.is_empty() {
                continue;
            }

            if let Some(kind) = table_header(content) {
                if let Some(done) = pending.take() {
                    finish(file, done, &mut records, &mut complain);
                }
                match kind {
                    "plugin" | "header" => {
                        pending = Some(Pending {
                            plugin: kind == "plugin",
                            line,
                            ..Pending::default()
                        });
                    }
                    other => complain(
                        line,
                        format!(
                            "`[[{other}]]` is not a declaration; write `[[plugin]]` or \
                             `[[header]]`"
                        ),
                    ),
                }
                continue;
            }

            let Some((key, value)) = content.split_once('=') else {
                complain(
                    line,
                    format!("`{content}` is neither `[[plugin]]`, `[[header]]` nor `key = value`"),
                );
                continue;
            };
            let key = key.trim();
            let value = value.trim();

            let Some(block) = pending.as_mut() else {
                complain(
                    line,
                    format!("`{key}` comes before any `[[plugin]]` or `[[header]]` line"),
                );
                continue;
            };

            match key {
                "name" | "method" | "nsis" => match string(value) {
                    Some(text) => {
                        let slot = match key {
                            "name" => &mut block.name,
                            "method" => &mut block.method,
                            _ => &mut block.nsis,
                        };
                        *slot = Some(text);
                    }
                    None => complain(line, format!("`{key}` wants a quoted string")),
                },
                "params" => match array(value) {
                    Some(words) => block.params = Some(types(&words, line, false, &mut complain)),
                    None => complain(
                        line,
                        "`params` wants an array of quoted type names on one line".to_string(),
                    ),
                },
                "outputs" => match array(value) {
                    Some(words) => {
                        block.outputs = Some(
                            types(&words, line, true, &mut complain)
                                .into_iter()
                                .map(|param| param.ty)
                                .collect(),
                        );
                    }
                    None => complain(
                        line,
                        "`outputs` wants an array of quoted type names on one line".to_string(),
                    ),
                },
                // `tagged` and `more` describe a *stack* whose depth the first
                // popped value decides, and a header macro pushes nothing: its
                // outputs are trailing registers `!insertmacro` writes on every
                // path, so there is no first value to test and no arity to
                // vary.
                "tagged" | "more" if !block.plugin => complain(
                    line,
                    format!(
                        "`{key}` is a plugin field: a macro writes its outputs into registers, \
                         so its arity cannot depend on what it returned"
                    ),
                ),
                // Literals, not types: this is the text the emitted `StrCmpS`
                // compares against, and it is whatever the plugin's source
                // spells — `"error"`, `"success"`, `"cancel"`.
                "tagged" => match array(value) {
                    Some(words) => block.tagged = Some(words),
                    None => complain(
                        line,
                        "`tagged` wants an array of quoted first-values on one line".to_string(),
                    ),
                },
                "more" => match array(value) {
                    Some(words) => {
                        block.more = Some(
                            types(&words, line, true, &mut complain)
                                .into_iter()
                                .map(|param| param.ty)
                                .collect(),
                        );
                    }
                    None => complain(
                        line,
                        "`more` wants an array of quoted type names on one line".to_string(),
                    ),
                },
                // `flags` is a plugin's alone, and for a reason about NSIS
                // rather than about tidiness: a flag is a token on a plugin's
                // *call line*, which `!insertmacro` has no equivalent of. A
                // `/M=*.txt` reaching a macro is one of its positional
                // arguments and nothing else — `${GetSize} "$dir" "/M=*.txt"`
                // is exactly that — so a macro's flags are already its
                // `params`, and a second spelling for them would emit an
                // argument in a position the macro does not have.
                "flags" if !block.plugin => complain(
                    line,
                    "`flags` is a plugin field: a macro takes `!insertmacro` arguments by \
                     position, so an option string is one of its `params`"
                        .to_string(),
                ),
                "flags" => match tables(value) {
                    Some(entries) => {
                        block.flags = Some(flags(&entries, line, &mut complain));
                    }
                    None => complain(
                        line,
                        "`flags` wants an array of `{ name = \"…\", nsis = \"/…\" }` tables"
                            .to_string(),
                    ),
                },
                // `dir` is a plugin's alone. A header is `!include`d and NSIS
                // searches for one along `!addincludedir`, which is a different
                // directive with a different position — accepting the key here
                // would promise a lookup nothing performs.
                "dir" if !block.plugin => complain(
                    line,
                    "`dir` is a plugin field: a header is found along the include path, not \
                     the plugin path"
                        .to_string(),
                ),
                "dir" => match string(value) {
                    Some(text) => block.dir = Some(text),
                    None => complain(line, "`dir` wants a quoted string".to_string()),
                },
                other => complain(
                    line,
                    format!(
                        "`{other}` is not a declaration field; the fields are `name`, `method`, \
                         `nsis`, `params`, `outputs` and — on a `[[plugin]]` — `flags`, \
                         `tagged`, `more` and `dir`"
                    ),
                ),
            }
        }

        if let Some(done) = pending.take() {
            finish(file, done, &mut records, &mut complain);
        }
        records
    }

    /// The file's lines, with a value that wraps joined onto the line it opened
    /// on.
    ///
    /// A `flags` list is one entry per line and does not fit on one, so the
    /// line-per-field rule the rest of the format keeps had to bend for it. It
    /// bends by *depth* rather than by a continuation marker: a line that
    /// leaves a `[` or a `{` open is unfinished, which is the same thing TOML
    /// means by it, so nothing new is written in the file to say so.
    ///
    /// The joined line keeps the number it *opened* on, because that is where
    /// the reader has to go to fix it — a message pointing at the closing `]`
    /// of a nine-line list names the wrong end of the mistake.
    ///
    /// Comments come off each physical line first, so a `#` on a wrapped line
    /// is a comment rather than the rest of the value.
    fn logical(text: &str) -> Vec<(usize, String)> {
        let mut lines: Vec<(usize, String)> = Vec::new();
        let mut open = 0;
        for (index, raw) in text.lines().enumerate() {
            let content = strip(raw);
            match lines.last_mut() {
                Some((_, started)) if open > 0 => {
                    started.push(' ');
                    started.push_str(content);
                }
                _ if content.is_empty() => continue,
                _ => lines.push((index + 1, content.to_string())),
            }
            // A negative depth is a stray `]`, and it is not reported here: the
            // value parsers say what the field wanted, which names the field.
            // Clamping keeps one typo from swallowing the rest of the file.
            open = (open + depth(content)).max(0);
        }
        lines
    }

    /// How many brackets a line leaves open. Quoted text is skipped, because a
    /// plugin's NSIS name is written in a string and `System::Call`'s signature
    /// has braces in it.
    fn depth(content: &str) -> i32 {
        let mut quoted = false;
        let mut depth = 0;
        for byte in content.bytes() {
            match byte {
                b'"' => quoted = !quoted,
                b'[' | b'{' if !quoted => depth += 1,
                b']' | b'}' if !quoted => depth -= 1,
                _ => {}
            }
        }
        depth
    }

    const VOCABULARY: &str = "the types are `string`, `path`, `int`, `uint`, `int64`, `intptr`, \
                              `bool`, `handle`, `any` and `callback`";

    /// A `flags` list, checked. Like [`types`], an unreadable entry is dropped
    /// rather than guessed at: a flag the compiler invented would emit a token
    /// the plugin reads as a positional argument.
    fn flags(
        entries: &[Vec<(String, String)>],
        line: usize,
        complain: &mut impl FnMut(usize, String),
    ) -> Vec<Flag> {
        let mut flags = Vec::new();
        for entry in entries {
            let field = |wanted: &str| {
                entry
                    .iter()
                    .find(|(key, _)| key == wanted)
                    .and_then(|(_, value)| string(value))
            };
            if let Some((key, _)) = entry
                .iter()
                .find(|(key, _)| !matches!(key.as_str(), "name" | "nsis" | "ty" | "value"))
            {
                complain(
                    line,
                    format!(
                        "`{key}` is not a flag field; a flag is `name`, `nsis` and — when it \
                         carries a value — `ty` and `value`"
                    ),
                );
                continue;
            }
            let (Some(name), Some(nsis)) = (field("name"), field("nsis")) else {
                complain(
                    line,
                    "a flag needs `name`, the options-table key, and `nsis`, the flag as the \
                     plugin spells it"
                        .to_string(),
                );
                continue;
            };
            if !nsis.starts_with('/') {
                complain(
                    line,
                    format!(
                        "`{nsis}` is not a flag: NSIS reads a token without a leading `/` as an \
                         argument, and the plugin would take it for one"
                    ),
                );
                continue;
            }

            // `ty` and `value` are one field in two halves, as `tagged` and
            // `more` are. A `ty` with no `value` types something that is never
            // written — a bare flag is its own value — and a `value` with no
            // `ty` says where to put a value nothing checks, which is the hole
            // `fits` was unified to close.
            let (ty, carry) = match (field("ty"), field("value")) {
                (None, None) => (
                    Param {
                        ty: Ty::Bool,
                        path: false,
                        callback: false,
                    },
                    Carry::Bare,
                ),
                (Some(_), None) | (None, Some(_)) => {
                    complain(
                        line,
                        format!(
                            "`{name}` gives one of `ty` and `value`, and they are one field in \
                             two halves — the type of what it carries, and whether `{nsis}` \
                             glues it on with `=` or writes it as the next token"
                        ),
                    );
                    continue;
                }
                (Some(word), Some(carry)) => {
                    let Some(param) = ty(&word) else {
                        complain(line, format!("`{word}` is not a type; {VOCABULARY}"));
                        continue;
                    };
                    // A flag is a token on a call line and a callback is an
                    // address the compiler writes a function for. There is no
                    // spelling in which one is the other.
                    if param.callback {
                        complain(
                            line,
                            format!(
                                "`{name}` cannot carry a `callback`: a flag is a token, and \
                                     a callback is a function this compiler emits"
                            ),
                        );
                        continue;
                    }
                    let carry = match carry.as_str() {
                        "joined" => Carry::Joined,
                        "separate" => Carry::Separate,
                        other => {
                            complain(
                                line,
                                format!(
                                    "`{other}` is not a `value` spelling; a flag's value is \
                                     `joined` — `{nsis}=x` — or `separate` — `{nsis} x`"
                                ),
                            );
                            continue;
                        }
                    };
                    (param, carry)
                }
            };
            flags.push(Flag {
                name,
                nsis,
                ty: ty.ty,
                path: ty.path,
                carry,
            });
        }
        flags
    }

    /// A type list, checked. An unreadable word is dropped rather than
    /// substituted: a `params` list one short is a wrong arity, and the CLI
    /// stops on the problem either way.
    fn types(
        words: &[String],
        line: usize,
        output: bool,
        complain: &mut impl FnMut(usize, String),
    ) -> Vec<Param> {
        let mut params = Vec::new();
        for word in words {
            let Some(param) = ty(word) else {
                complain(line, format!("`{word}` is not a type; {VOCABULARY}"));
                continue;
            };
            if output && param.path {
                complain(
                    line,
                    "`path` is an input spelling: it normalises `/` on the way in, and an \
                     output is whatever the callee already wrote"
                        .to_string(),
                );
                continue;
            }
            // A macro that takes a callback pushes nothing back through the
            // stack — everything the caller learns arrives as an argument to
            // the callback, which is the whole shape of these six.
            if output && param.callback {
                complain(
                    line,
                    "`callback` is an input spelling: a macro that takes a function address \
                     writes no output register"
                        .to_string(),
                );
                continue;
            }
            params.push(param);
        }
        params
    }

    /// Turns a filled-in block into a record, or says which half is missing.
    ///
    /// `nsis` is required rather than derived. `${StrCase}` puts its destination
    /// first and `${GetSize}` puts its destinations last, and the Installua name
    /// is not the NSIS one in any mechanical way — a compiler that guessed would
    /// emit NSIS that looks right and is not.
    fn finish(
        file: &str,
        block: Pending,
        records: &mut Vec<Record>,
        complain: &mut impl FnMut(usize, String),
    ) {
        let what = if block.plugin { "plugin" } else { "header" };
        let (Some(name), Some(method), Some(nsis)) = (block.name, block.method, block.nsis) else {
            complain(
                block.line,
                format!(
                    "this `[[{what}]]` is missing one of `name`, `method` or `nsis`, and all \
                     three are required"
                ),
            );
            return;
        };
        let outputs = block.outputs.unwrap_or_default();
        let tagged = block.tagged.unwrap_or_default();
        let more = block.more.unwrap_or_default();

        // Three checks, and each rules out a declaration that would *compile*
        // into an unbalanced stack rather than into a diagnostic.
        //
        // Half a pair is the common typo and the worst outcome: `tagged` alone
        // names a condition with nothing to do, and `more` alone names values
        // with no condition — a plugin whose extra value is popped always, or
        // never. Neither is a shape any plugin has.
        //
        // And `tagged` tests the **first popped value**, so a declaration
        // without one is testing nothing. That is not a degenerate case to
        // allow: a plugin that pushes a conditional value and nothing else
        // pushes *nothing* on the other path, which is a boolean the stack
        // cannot carry.
        let (tagged, more) = match (tagged.is_empty(), more.is_empty()) {
            (true, true) => (tagged, more),
            (false, true) | (true, false) => {
                complain(
                    block.line,
                    "`tagged` and `more` are one field in two halves — the first says when the \
                     extra values follow and the second says what they are, and neither means \
                     anything alone"
                        .to_string(),
                );
                (Vec::new(), Vec::new())
            }
            (false, false) if outputs.is_empty() => {
                complain(
                    block.line,
                    "`tagged` tests the first value the plugin pushes, so `outputs` has to \
                     declare one: a value pushed on only some paths cannot be the value that \
                     says which path it was"
                        .to_string(),
                );
                (Vec::new(), Vec::new())
            }
            (false, false) => (tagged, more),
        };

        records.push(Record {
            plugin: block.plugin,
            name,
            method,
            nsis,
            params: block.params.unwrap_or_default(),
            outputs,
            flags: block.flags.unwrap_or_default(),
            tagged,
            more,
            dir: block.dir,
            file: file.to_string(),
            line: block.line,
        });
    }

    /// The line with its comment removed. A `#` inside a quoted string is text,
    /// which matters because a plugin's NSIS name is written in one.
    fn strip(line: &str) -> &str {
        let mut quoted = false;
        for (index, byte) in line.bytes().enumerate() {
            match byte {
                b'"' => quoted = !quoted,
                b'#' if !quoted => return line[..index].trim(),
                _ => {}
            }
        }
        line.trim()
    }

    /// `[[plugin]]` ⇒ `Some("plugin")`. A single-bracket `[table]` is not this
    /// format and falls through to the "neither" message, which names both
    /// spellings that are.
    fn table_header(content: &str) -> Option<&str> {
        content
            .strip_prefix("[[")
            .and_then(|rest| rest.strip_suffix("]]"))
            .map(str::trim)
    }

    fn string(value: &str) -> Option<String> {
        value
            .strip_prefix('"')
            .and_then(|rest| rest.strip_suffix('"'))
            .filter(|text| !text.contains('"'))
            .map(str::to_string)
    }

    fn array(value: &str) -> Option<Vec<String>> {
        items(value, '[', ']')?
            .iter()
            .map(|item| string(item))
            .collect()
    }

    /// An array of inline tables, each a list of `key = value` pairs in the
    /// order written. A list rather than a map because the only consumer is
    /// [`flags`], which reports an unknown key by name and wants to say which
    /// one — and four pairs are not worth a hash.
    fn tables(value: &str) -> Option<Vec<Vec<(String, String)>>> {
        items(value, '[', ']')?
            .iter()
            .map(|item| {
                items(item, '{', '}')?
                    .iter()
                    .map(|pair| {
                        let (key, value) = pair.split_once('=')?;
                        Some((key.trim().to_string(), value.trim().to_string()))
                    })
                    .collect::<Option<Vec<_>>>()
            })
            .collect()
    }

    /// The top-level items of a bracketed list, trimmed.
    ///
    /// Separators are commas at depth zero and outside quotes, so a `{ … }`
    /// holding commas is one item and so is a string holding one. The naive
    /// `split(',')` this replaces could not have either, which was invisible
    /// while every value was a one-word type name.
    ///
    /// A trailing comma is allowed and produces no empty item, because a list
    /// written one entry per line grows by copying the line above it.
    fn items(value: &str, open: char, close: char) -> Option<Vec<&str>> {
        let inside = value.trim().strip_prefix(open)?.strip_suffix(close)?.trim();
        if inside.is_empty() {
            return Some(Vec::new());
        }
        let mut items = Vec::new();
        let mut start = 0;
        let mut quoted = false;
        let mut depth = 0;
        for (index, byte) in inside.bytes().enumerate() {
            match byte {
                b'"' => quoted = !quoted,
                b'[' | b'{' if !quoted => depth += 1,
                b']' | b'}' if !quoted => depth -= 1,
                b',' if !quoted && depth == 0 => {
                    items.push(inside[start..index].trim());
                    start = index + 1;
                }
                _ => {}
            }
        }
        let last = inside[start..].trim();
        if !last.is_empty() {
            items.push(last);
        }
        Some(items)
    }

    /// The type vocabulary. `path` is `string` plus the normalisation, and is
    /// spelled as a type because that is where a declaration puts it: the file
    /// says what a position *is*, and `/` becoming `\` follows from that.
    fn ty(word: &str) -> Option<Param> {
        if word == "callback" {
            return Some(Param {
                ty: Ty::Unknown,
                path: false,
                callback: true,
            });
        }
        let (ty, path) = match word {
            "string" => (Ty::Str, false),
            "path" => (Ty::Str, true),
            "int" => (Ty::int(), false),
            "uint" => (Ty::nonneg(), false),
            "int64" => (
                Ty::Int(Int {
                    width: Width::W64,
                    sign: Sign::Unknown,
                }),
                false,
            ),
            "intptr" => (
                Ty::Int(Int {
                    width: Width::Ptr,
                    sign: Sign::Unknown,
                }),
                false,
            ),
            "bool" => (Ty::Bool, false),
            "handle" => (Ty::Handle, false),
            "any" => (Ty::Unknown, false),
            _ => return None,
        };
        Some(Param {
            ty,
            path,
            callback: false,
        })
    }
}
