//! `include`: source layout, and nothing else.
//!
//! Two words exist because they belong to two stages. `import "FileFunc"` emits
//! an `!include` into the output and is therefore *in* the artifact; `include
//! "strings/de.lua"` merges another file's declarations into this one and
//! leaves no trace at all. Overloading one name across that boundary is the
//! staging conflation this language forbids — a reader must always be able to
//! tell which stage a line belongs to.
//!
//! What this pass is not: a module system in the separately-compiled sense.
//! There is no search path and no unit that compiles on its own — the latter is
//! out for good, because clobber sets are whole-program. A file is spliced into
//! the top-level block of the file that named it, and everything downstream
//! sees one tree. That is what makes the merge free: resolution is already
//! order-free, so a `func` in one file and its caller in another need no
//! ordering rule between them either.
//!
//! What it does take from Lua is the *scoping*. A top-level `local` belongs to
//! its file, and a file shares one as a Lua module would, `return { name = name }`
//! read through `local m = include "…"` as `m.name`. This pass records both
//! halves in [`Modules`]; [`scope`] enforces the rule and rewrites `m.name` to
//! the bare name, so the tree downstream is unchanged.
//!
//! Three things a path may not do, all for the same reason — it has to be
//! readable without running anything:
//!
//!   * be an expression (`include prefix .. ".lua"`)
//!   * sit inside a body, where only install time could decide it
//!   * name a file that names it back, however long the way round
//!
//! **Two files that each declare `func("yolo", …)` collide, and that is the
//! intended behaviour.** `resolve` reports it on the Lua line, naming the other
//! file — a compile failure where NSIS would have given a build failure, which
//! is the whole point of the merge being free.
//!
//! **Automatic namespacing is the rejected fix**, and stays rejected: the
//! names are *NSIS-visible*. `Function yolo` is a real symbol, called from
//! `raw`, read in `makensis` output and diffed in hand-written goldens, so
//! renaming it behind the author's back breaks the one promise `include` makes.
//! A `func` is named by a string, so Lua has no scope to give it anyway — which
//! is also why the returned table carries only `local`s. The collision is
//! caught precisely, and a silent rename would replace a good diagnostic with
//! none.
//!
//! The returned table never exists at run time, which is how this squares with
//! a language that has no table values: it is a build-time namespace, the same
//! kind of binding `local fileFunc = import "FileFunc"` makes.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::ast::{Block, Expr, Name, Program, Stmt, TableField};
use crate::diag::{Code, Diagnostic, Diagnostics, Span};
use crate::frontend::scope;
use crate::{Options, frontend};

/// Where an included file's text comes from.
///
/// An enum rather than a boxed closure because [`Options`] is `Clone + Debug`
/// and a closure is neither — and because there are exactly two answers: the
/// build tool reads the disk, and everything else (tests, an editor, an
/// embedder) already has the sources in hand. The requirement that this crate
/// compile an in-memory string with no file system involved does not stop being
/// true because a program grew a second file.
#[derive(Clone, Debug, Default)]
pub enum Loader {
    #[default]
    Disk,
    /// Keyed by path relative to `Options::base`, slash-separated — the same
    /// keys the disk loader would join onto `base`.
    Memory(BTreeMap<String, String>),
}

impl Loader {
    fn read(&self, base: Option<&Path>, key: &str) -> Result<String, String> {
        match self {
            Loader::Memory(sources) => sources
                .get(key)
                .cloned()
                .ok_or_else(|| "no such file".to_string()),
            Loader::Disk => {
                let base = base.ok_or_else(|| {
                    "this source did not come from a file, so there is no directory to resolve \
                     against"
                        .to_string()
                })?;
                std::fs::read_to_string(base.join(key)).map_err(|error| error.to_string())
            }
        }
    }
}

/// The name file `0` gets when the root came from nowhere — an in-memory
/// string. Nothing can `include` it, since no path spells it, so no cycle can
/// run through it either.
const UNNAMED: &str = "<source>";

/// Parses `source`, follows every `include` in it, and returns the one tree
/// they make together. `diags` comes back holding the file table those spans
/// are measured in.
pub fn load(source: &str, options: &Options, diags: &mut Diagnostics) -> Option<Program> {
    let root = options
        .root
        .as_ref()
        .map(|path| normalize(&path.to_string_lossy()))
        .unwrap_or_else(|| UNNAMED.to_string());
    diags.files_mut().add(root.clone());

    let mut program = frontend::check_file(source, 0, diags)?;
    let mut expander = Expander {
        options,
        diags,
        stack: vec![root],
        modules: Modules::default(),
    };
    expander.expand(&mut program.block, 0);
    let modules = expander.modules;
    scope::check(&mut program.block, &modules, diags);
    Some(program)
}

/// What the splice leaves behind for [`scope`]: which file each
/// `local m = include "…"` names, and what each file's `return { … }` exports.
///
/// Recorded here because this is the last pass that can see either — the
/// `local` and the `return` are both gone from the tree it hands on.
#[derive(Default)]
pub(crate) struct Modules {
    pub bindings: Vec<Binding>,
    /// Keyed by file id. Each entry is `key = name`: `m.key` reads `name`.
    pub exports: BTreeMap<u32, Vec<(Name, Name)>>,
}

/// `local m = include "…"`: `name` is `m`, bound in the file its span names,
/// and `target` is the file it reads through.
pub(crate) struct Binding {
    pub name: Name,
    pub target: u32,
}

/// The walk itself, distinct from the [`Loader`] a caller configures.
struct Expander<'a, 'b> {
    options: &'a Options,
    diags: &'b mut Diagnostics,
    /// The chain of files currently being expanded, which is what makes a cycle
    /// nameable rather than merely detectable.
    stack: Vec<String>,
    modules: Modules,
}

impl Expander<'_, '_> {
    /// Replaces every top-level `include` in `block` with what it names.
    ///
    /// A malformed one is dropped rather than kept: it was reported here, and
    /// leaving it in the tree would earn it a second, worse diagnostic from
    /// lowering — where `include` is not a declaration and never will be.
    fn expand(&mut self, block: &mut Block, file: u32) {
        let dir = directory(&self.name(file));
        let mut out = Vec::with_capacity(block.len());
        for stmt in std::mem::take(block) {
            if let Stmt::Return { values, span } = &stmt {
                self.exports(values, *span, file);
                continue;
            }
            match self.include(&stmt) {
                Seen::Other => out.push(stmt),
                Seen::Malformed => {}
                Seen::Path(path, span, binding) => {
                    let (target, mut included) = self.file(&path, &dir, span);
                    out.append(&mut included);
                    if let (Some(name), Some(target)) = (binding, target) {
                        self.modules.bindings.push(Binding { name, target });
                    }
                }
            }
        }
        *block = out;
    }

    /// A top-level `return`, which is how a file says what an including file
    /// may read — Lua's own module shape, `return { name = name }`.
    ///
    /// Only names, and only as a table: the table never exists at run time. It
    /// is read here and gone, the same kind of binding `import` makes, so a
    /// value in it would have nowhere to live.
    fn exports(&mut self, values: &[Expr], span: Span, file: u32) {
        let fields = match values {
            [Expr::Table { fields, .. }] => fields,
            _ => {
                self.error(
                    Code::IncludeForm,
                    span,
                    "a file returns a table of its own names",
                    &["write `return { name = name, … }`; an including file reads them as `m.name`"],
                );
                return;
            }
        };
        let mut exports = Vec::new();
        for field in fields {
            match field {
                TableField::Named {
                    name,
                    value: Expr::Name(value),
                } => exports.push((name.clone(), value.clone())),
                _ => self.error(
                    Code::IncludeForm,
                    field.span(),
                    "each entry of a returned table is `key = name`",
                    &[
                        "the table is read at build time, so it can only hand on names this file \
                       declares",
                    ],
                ),
            }
        }
        self.modules.exports.insert(file, exports);
    }

    /// The path an `include` statement names, when the statement is one.
    ///
    /// Everything that is an `include` and not well-formed is reported here
    /// rather than dropped: `include(prefix .. ".lua")` is a mistake with an
    /// answer, and letting it fall through to resolution would produce
    /// "`include` is not a function", which is true and unhelpful.
    fn include(&mut self, stmt: &Stmt) -> Seen {
        let (call, binding) = match stmt {
            Stmt::Call(call) => (call, None),
            Stmt::Local {
                names,
                is_const,
                values,
                span,
            } if values
                .iter()
                .any(|value| value.callee_name() == Some("include")) =>
            {
                let ([name], [call], false) = (names.as_slice(), values.as_slice(), is_const)
                else {
                    self.error(
                        Code::IncludeForm,
                        *span,
                        "`include` binds one name",
                        &["write `local m = include \"other.lua\"`, then read its names as `m.name`"],
                    );
                    return Seen::Malformed;
                };
                (call, Some(name.clone()))
            }
            _ => return Seen::Other,
        };
        if call.callee_name() != Some("include") {
            return Seen::Other;
        }
        let Expr::Call { args, span, .. } = call else {
            return Seen::Other;
        };
        match args.as_slice() {
            [Expr::Str(literal)] => Seen::Path(literal.value.clone(), literal.span, binding),
            [] => {
                self.error(
                    Code::IncludeForm,
                    *span,
                    "`include` needs a path",
                    &["write `include \"other.lua\"`"],
                );
                Seen::Malformed
            }
            [other] => {
                self.error(
                    Code::IncludeForm,
                    other.span(),
                    "an included path must be a string literal",
                    &[
                        "nothing runs before this pass, so there is no stage that could evaluate \
                         an expression here",
                        "the path is relative to this file, and written whole: \
                         `include \"strings/de.lua\"`",
                    ],
                );
                Seen::Malformed
            }
            [_, second, ..] => {
                self.error(
                    Code::IncludeForm,
                    second.span(),
                    "`include` takes one path",
                    &["write one `include` per file"],
                );
                Seen::Malformed
            }
        }
    }

    /// Loads one file and returns its id and top-level statements, already
    /// expanded. The id comes back for a file already loaded too, with no
    /// statements, since a second `local m = include` still binds `m`.
    fn file(&mut self, path: &str, dir: &str, span: Span) -> (Option<u32>, Block) {
        let key = normalize(&join(dir, path));
        let block = self.load(path, &key, span).unwrap_or_default();
        (self.diags.files().find(&key), block)
    }

    fn load(&mut self, path: &str, key: &str, span: Span) -> Option<Block> {
        let key = key.to_string();

        // A file included twice is included once — the same set semantics
        // `import` has, and for the same reason: a declaration merged twice is
        // a duplicate declaration, and the second `include` did not ask for
        // one.
        if self.diags.files().find(&key).is_some() {
            if self.stack.iter().any(|entry| entry == &key) {
                let mut chain = self.stack.clone();
                chain.push(key.clone());
                self.error(
                    Code::IncludeCycle,
                    span,
                    format!("`{key}` includes itself"),
                    &[
                        &format!("the loop is {}", chain.join(" → ")),
                        "declarations are order-free, so a file never needs to include the one \
                         that includes it",
                    ],
                );
            }
            return None;
        }

        let text = match self.options.loader.read(self.options.base.as_deref(), &key) {
            Ok(text) => text,
            Err(reason) => {
                let mut notes = vec![format!("cannot read `{key}`: {reason}")];
                if !key.ends_with(".lua") {
                    notes.push(
                        "a path is written whole, including the extension: \
                         `include \"strings/de.lua\"`"
                            .to_string(),
                    );
                }
                notes.push("paths resolve against the file that names them".to_string());
                let notes: Vec<&str> = notes.iter().map(String::as_str).collect();
                self.error(
                    Code::IncludeNotFound,
                    span,
                    format!("cannot include `{path}`"),
                    &notes,
                );
                return None;
            }
        };

        let file = self.diags.files_mut().add(key.clone());
        let program = frontend::check_file(&text, file, self.diags)?;
        let mut block = program.block;
        self.stack.push(key);
        self.expand(&mut block, file);
        self.stack.pop();
        Some(block)
    }

    fn name(&self, file: u32) -> String {
        self.diags.files().name(file).unwrap_or(UNNAMED).to_string()
    }

    fn error(&mut self, code: Code, span: Span, message: impl Into<String>, notes: &[&str]) {
        let mut diagnostic = Diagnostic::error(code, span, message);
        for note in notes {
            diagnostic = diagnostic.note(*note);
        }
        self.diags.push(diagnostic);
    }
}

/// What one top-level statement turned out to be.
enum Seen {
    /// Not an `include` at all, and therefore not this pass's business.
    Other,
    /// An `include` that was reported here. Dropped, not kept.
    Malformed,
    /// The path, and the name a `local m = include` binds it to.
    Path(String, Span, Option<Name>),
}

/// The directory part of a key, `""` for a file at the root.
fn directory(key: &str) -> String {
    match key.rsplit_once('/') {
        Some((dir, _)) => dir.to_string(),
        None => String::new(),
    }
}

fn join(dir: &str, path: &str) -> String {
    if dir.is_empty() {
        path.to_string()
    } else {
        format!("{dir}/{path}")
    }
}

/// Lexical normalisation: `/` separators, no `.`, and `..` cancelled against
/// what precedes it.
///
/// Lexical rather than canonical on purpose. `std::fs::canonicalize` needs the
/// file to exist, which is exactly the case this table has to name when it does
/// not — and it resolves symlinks, so two spellings of one path would key
/// differently depending on how the project was checked out.
pub(crate) fn normalize(path: &str) -> String {
    let path = path.replace('\\', "/");
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." if matches!(parts.last(), Some(&last) if last != "..") => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
}

/// The root's key, for a caller that has a path and wants the two halves
/// [`Options`] wants: the directory to resolve against, and the file itself.
pub fn split(input: &Path) -> (PathBuf, PathBuf) {
    let base = input.parent().unwrap_or(Path::new(".")).to_path_buf();
    let root = input
        .file_name()
        .map(PathBuf::from)
        .unwrap_or_else(|| input.to_path_buf());
    (base, root)
}
