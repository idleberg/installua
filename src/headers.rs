//! Declared foreign macros and plugin methods: what `import "FileFunc"` and
//! `plugin "nsExec"` bring into scope.
//!
//! A header's macros and a plugin's methods need a *declaration* rather than a
//! discovery pass. Nothing can read `FileFunc.nsh` and recover that `${GetSize}`
//! writes three registers and takes two arguments, because an `!insertmacro`
//! parameter list carries no directions; and nothing can ask a DLL how many
//! values it pushes, because NSIS offers no way to ask. Both are written down.
//!
//! Two sources, one table. [`Declarations::builtin`] seeds the handful this
//! repository's own examples reach, and [`Declarations::load`] adds whatever
//! `.installua/headers/*.toml` declares — which is how a *third-party* plugin
//! or header becomes ordinary rather than a special case: the compiler checks
//! its arity, the editor stubs type its calls, and neither half is
//! hand-maintained. A project file may redeclare a builtin and wins when it
//! does, because a wrong count shipped here must not be a wall.
//!
//! Every macro shares one calling convention, and it is not a choice this
//! compiler made: a header macro takes its inputs first and its **outputs as
//! trailing register arguments**, because `!insertmacro` has no way to return a
//! value. That is why an output is a `Param` position rather than a `returns`
//! field the way an instruction's is. A plugin's outputs are on the stack
//! instead, in `Pop` order — which is why the two are separate lists here and
//! not one shape with a flag.

use std::path::Path;

use crate::types::{Int, Sign, Ty, Width};

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
    /// One per value the plugin leaves on the stack, in `Pop` order.
    pub outputs: Vec<Ty>,
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

const fn data(ty: Ty) -> Param {
    Param { ty, path: false }
}

const fn path() -> Param {
    Param {
        ty: Ty::Str,
        path: true,
    }
}

/// One row of a builtin table: the namespace, the method, the NSIS spelling,
/// the parameters and the outputs.
///
/// A tuple rather than [`Macro`] or [`PluginMethod`] themselves, because a
/// `const` cannot hold a `String` and those own theirs — the conversion happens
/// once per compilation, in [`Declarations::builtin`].
type Builtin = (
    &'static str,
    &'static str,
    &'static str,
    &'static [Param],
    &'static [Ty],
);

/// `(header, method, macro, params, outputs)`.
const BUILTIN_MACROS: &[Builtin] = &[
    (
        "FileFunc",
        "getSize",
        "GetSize",
        &[path(), data(Ty::Str)],
        // Size, files, directories. A byte count cannot be negative, and that
        // is what makes `size // 1024` a bare `IntOp` with no sign fixup — the
        // one place program 4's README says the lattice pays.
        &[Ty::nonneg(), Ty::nonneg(), Ty::nonneg()],
    ),
    (
        "FileFunc",
        "driveSpace",
        "DriveSpace",
        &[path(), data(Ty::Str)],
        &[Ty::nonneg()],
    ),
    (
        "WordFunc",
        "versionCompare",
        "VersionCompare",
        &[data(Ty::Str), data(Ty::Str)],
        // `"0"`, `"1"` or `"2"` — a string, because that is what the macro
        // leaves in the register and comparing it as an int would be a guess
        // the lattice has no evidence for.
        &[Ty::Str],
    ),
];

/// `(plugin, method, `Plugin::Method`, params, outputs)`.
const BUILTIN_PLUGINS: &[Builtin] = &[
    (
        "nsExec",
        "execToStack",
        "nsExec::ExecToStack",
        &[data(Ty::Str)],
        // The exit code first, then the captured output — `Pop` order, which is
        // the order the declaration has to state and the source cannot see.
        &[Ty::Str, Ty::Str],
    ),
    (
        "UserInfo",
        "getAccountType",
        "UserInfo::GetAccountType",
        &[],
        &[Ty::Str],
    ),
    (
        "System",
        "call",
        "System::Call",
        &[data(Ty::Str)],
        // Counted from the signature instead: every `.s` in it pushes one
        // value. Parsing the rest of a `System::Call` signature — which would
        // narrow the clobber set from "everything" — is deferred.
        &[],
    ),
];

/// Where a project's own declarations live, relative to its root.
pub const DIRECTORY: &str = ".installua/headers";

impl Default for Declarations {
    fn default() -> Declarations {
        Declarations::builtin()
    }
}

impl Declarations {
    /// The declarations that ship with the compiler.
    pub fn builtin() -> Declarations {
        Declarations {
            macros: BUILTIN_MACROS
                .iter()
                .map(|(header, installua, nsis, params, outputs)| Macro {
                    project: false,
                    header: (*header).to_string(),
                    installua: (*installua).to_string(),
                    nsis: (*nsis).to_string(),
                    params: params.to_vec(),
                    outputs: outputs.to_vec(),
                })
                .collect(),
            plugins: BUILTIN_PLUGINS
                .iter()
                .map(|(plugin, installua, nsis, params, outputs)| PluginMethod {
                    project: false,
                    plugin: (*plugin).to_string(),
                    installua: (*installua).to_string(),
                    nsis: (*nsis).to_string(),
                    params: params.to_vec(),
                    outputs: outputs.to_vec(),
                })
                .collect(),
        }
    }

    /// The builtins plus every `*.toml` in `dir`, in file-name order.
    ///
    /// A missing directory is not a problem: a project that declares nothing is
    /// the common case, and an error there would make `.installua/headers/`
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
/// `key = value` lines, where a value is a string or a one-line array of
/// strings — parsed here rather than by a dependency. The whole grammar fits on
/// a screen, the messages can name the field the user got wrong, and a compiler
/// that reads five keys does not need a deserialiser to do it. What a full
/// parser would buy is syntax this format does not use.
mod parse {
    use super::{Param, Problem};
    use crate::types::{Int, Sign, Ty, Width};

    /// One `[[plugin]]` or `[[header]]` block, checked.
    pub struct Record {
        pub plugin: bool,
        pub name: String,
        pub method: String,
        pub nsis: String,
        pub params: Vec<Param>,
        pub outputs: Vec<Ty>,
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

        for (index, raw) in text.lines().enumerate() {
            let line = index + 1;
            let content = strip(raw);
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
                other => complain(
                    line,
                    format!(
                        "`{other}` is not a declaration field; the fields are `name`, `method`, \
                         `nsis`, `params` and `outputs`"
                    ),
                ),
            }
        }

        if let Some(done) = pending.take() {
            finish(file, done, &mut records, &mut complain);
        }
        records
    }

    const VOCABULARY: &str = "the types are `string`, `path`, `int`, `uint`, `int64`, `intptr`, \
                              `bool`, `handle` and `any`";

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
        records.push(Record {
            plugin: block.plugin,
            name,
            method,
            nsis,
            params: block.params.unwrap_or_default(),
            outputs: block.outputs.unwrap_or_default(),
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
        let inside = value.strip_prefix('[')?.strip_suffix(']')?.trim();
        if inside.is_empty() {
            return Some(Vec::new());
        }
        inside
            .split(',')
            .map(|item| string(item.trim()))
            .collect::<Option<Vec<String>>>()
    }

    /// The type vocabulary. `path` is `string` plus the normalisation, and is
    /// spelled as a type because that is where a declaration puts it: the file
    /// says what a position *is*, and `/` becoming `\` follows from that.
    fn ty(word: &str) -> Option<Param> {
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
        Some(Param { ty, path })
    }
}

/// The type vocabulary, spelled back — the same words a declaration file
/// writes. Used by the docs test that checks the format's vocabulary and the
/// parser's have not drifted apart.
pub fn spelling(ty: Ty) -> &'static str {
    match ty {
        Ty::Str => "string",
        Ty::Bool => "bool",
        Ty::Handle => "handle",
        Ty::Unknown => "any",
        Ty::Int(Int {
            width: Width::W32,
            sign: Sign::NonNeg,
        }) => "uint",
        Ty::Int(Int {
            width: Width::W32, ..
        }) => "int",
        Ty::Int(Int {
            width: Width::W64, ..
        }) => "int64",
        Ty::Int(Int {
            width: Width::Ptr, ..
        }) => "intptr",
    }
}
