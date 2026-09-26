//! `remember` — the components page remembers what was ticked, through the
//! stock `Memento.nsh`.
//!
//! ```lua
//! local docs = section { "Docs", remember = true, body = function() … end }
//! ```
//!
//! Lowered through the header, like `multiUser {}`: a section with `remember`
//! is written as `MementoSectionEx`/`MementoSectionEnd` instead of
//! `Section`/`SectionEnd`, and the `MEMENTO_REGISTRY_*` defines say where the
//! boxes are kept — `SHCTX` beside `multiUser {}` and `HKLM` otherwise, under
//! `Software\<name>\Components`. `memento { root = …, key = … }` overrides
//! either; it is optional, since one registry key per script is all the header
//! has. What the compiler adds is the bookkeeping the readme lists as steps:
//!
//! - `MementoSectionDone` after the last section;
//! - `MementoSectionRestore` in the init prelude, after `multiUser {}`'s, since
//!   an `SHCTX` root is only decided once that has run. First rather than last,
//!   so an `x.selected = false` in `onInit` is not undone by it; an author who
//!   wants it later — after writing a saved state, as `Examples/Memento.nsi`
//!   does — writes `memento.restore()` there, and the prelude's goes;
//! - `MementoSectionSave` first in `.onInstSuccess`: the author's, when the
//!   block has an `onInstSuccess`, or else one of its own.
//!
//! The id is the registry value's name, and the header says it must not change
//! between versions. `true` takes the section's `local`, which only changes
//! when someone renames it; a minted index or the display name would change
//! without anyone deciding to. So a rename pins the old id with
//! `remember = "old"`, as renaming the product pins the old `key`.

use crate::ast::{Expr, Stmt, TableField};
use crate::diag::{Code, Diagnostic, Span};
use crate::ir;
use crate::resolve::ConstValue;

use super::{BodyLowerer, Half, Lowerer};

impl Lowerer<'_, '_> {
    /// Its own pass, for `multiUser {}`'s reason, and after it: the restore
    /// line follows the mode line in `.onInit`. The sections are searched for a
    /// `remember` before any is lowered, since one is what writes the block
    /// when the author wrote none.
    pub(super) fn memento_pass(&mut self) {
        let mut blocks = self.resolved.block.iter().filter_map(|stmt| {
            let Stmt::Call(call) = stmt else { return None };
            (call.callee_name()? == "memento").then(|| (call, call.span()))
        });
        let block = blocks.next();
        for (_, second) in blocks {
            let first = block.map_or(second, |(_, span)| span);
            self.diags.push(
                Diagnostic::error(
                    Code::DuplicateBlock,
                    second,
                    "`memento {}` appears more than once",
                )
                .note_at("the first one is at", first)
                .note("it is script-global, so there is exactly one"),
            );
        }
        let remember = self.resolved.block.iter().find_map(|stmt| match stmt {
            Stmt::Local { values, .. } => values.iter().find_map(first_remember),
            Stmt::Call(call) => first_remember(call),
            _ => None,
        });

        let (fields, span): (&[TableField], Span) = match (block, remember) {
            (None, None) => return,
            (Some((call, span)), remember) => {
                if remember.is_none() {
                    self.diags.push(
                        Diagnostic::error(
                            Code::MissingAttribute,
                            span,
                            "`memento {}` with no section that `remember`s",
                        )
                        .note("write `remember = true` on each section whose box should be kept"),
                    );
                    return;
                }
                let Some(fields) = self.block_fields(call) else {
                    return;
                };
                (fields, span)
            }
            (None, Some(span)) => (&[], span),
        };
        self.memento = Some(span);

        let mut root = None;
        let mut key = None;
        for field in fields {
            let TableField::Named { name, value } = field else {
                continue;
            };
            match name.text.as_str() {
                "root" => {
                    // The header puts it straight after `ReadRegDWord`, so a
                    // word that is not a root is a makensis error far from here.
                    // Not a constant at all is `keyword`'s to report.
                    let Some(word) = self.keyword(value, "root") else {
                        return;
                    };
                    match crate::builtins::constant_named(&word) {
                        Some(constant) if constant.is_root() => root = Some(ir::Arg::raw(word)),
                        _ => {
                            self.bad_value(
                                value.span(),
                                "root",
                                "a registry root",
                                "write `root = HKLM`, or `SHCTX` beside `multiUser {}`",
                            );
                            return;
                        }
                    }
                }
                "key" => match self.constant_arg(value, "key") {
                    Some(arg) => key = Some(arg.into_path()),
                    None => return,
                },
                other => {
                    let span = name.span;
                    self.diags.push(
                        Diagnostic::error(
                            Code::UnknownField,
                            span,
                            format!("`{other}` is not a field of `memento {{}}`"),
                        )
                        .note("the two are `root` and `key`"),
                    );
                }
            }
        }

        // `SHCTX` beside `multiUser {}`, so the boxes follow the install mode
        // the way the files do.
        let root = root.unwrap_or_else(|| {
            ir::Arg::raw(if self.multi_user.is_some() {
                "SHCTX"
            } else {
                "HKLM"
            })
        });
        let Some(key) = key.or_else(|| {
            self.product_name()
                .map(|name| ir::Arg::str(format!("Software\\{name}\\Components")))
        }) else {
            self.diags.push(
                Diagnostic::error(
                    Code::MissingAttribute,
                    span,
                    "the remembered boxes have no registry key",
                )
                .note("the default is `Software\\<name>\\Components`, from `attributes { name }`")
                .note("or write `memento { key = \"Software/App\" }`"),
            );
            return;
        };

        for (name, value) in [
            ("MEMENTO_REGISTRY_ROOT", root),
            ("MEMENTO_REGISTRY_KEY", key),
        ] {
            self.module.defines.push(ir::Define {
                name: name.to_string(),
                value: Some(value),
            });
        }
        self.requires.headers.insert("Memento".to_string());
        self.init_prelude[Half::Installer.index()].push(ir::Instruction::new(
            "!insertmacro",
            vec![ir::Arg::raw("MementoSectionRestore")],
        ));
    }

    /// `attributes { name }`, when it is a build-time string.
    pub(super) fn product_name(&self) -> Option<String> {
        let consts = &self.resolved.consts;
        self.resolved.block.iter().find_map(|stmt| match stmt {
            Stmt::Call(call @ Expr::Call { args, .. })
                if call.callee_name() == Some("attributes") =>
            {
                let [Expr::Table { fields, .. }] = args.as_slice() else {
                    return None;
                };
                fields.iter().find_map(|field| match field {
                    TableField::Named { name, value } if name.text == "name" => {
                        match crate::resolve::fold(value, &|name| {
                            consts.get(name).map(|c| c.value.clone())
                        }) {
                            Some(ConstValue::Str(name)) => Some(name),
                            _ => None,
                        }
                    }
                    _ => None,
                })
            }
            _ => None,
        })
    }

    /// A section's `remember`: the id, when the section may have one. `true`
    /// takes the section's `local`, which is what `index` was minted from.
    pub(super) fn memento_id(
        &mut self,
        value: &Expr,
        half: Half,
        index: Option<&str>,
    ) -> Option<String> {
        let span = value.span();
        if half == Half::Uninstaller {
            self.diags.push(
                Diagnostic::error(
                    Code::BadFieldValue,
                    span,
                    "an uninstaller section cannot `remember`",
                )
                .note("`Memento.nsh` saves in `.onInstSuccess`, which is the installer's"),
            );
            return None;
        }
        let local = index.and_then(|index| index.strip_prefix("SEC_"));
        let id = match self.constant(value) {
            Some(ConstValue::Bool(true)) if let Some(local) = local => local.to_string(),
            Some(ConstValue::Bool(true)) => {
                self.diags.push(
                    Diagnostic::error(
                        Code::BadFieldValue,
                        span,
                        "`remember = true` takes its id from the section's `local`",
                    )
                    .note("this one has none, so write `remember = \"id\"`"),
                );
                return None;
            }
            // Backticks quote it inside the header's macros, so the id is kept
            // to what needs no quoting anywhere.
            Some(ConstValue::Str(id))
                if !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') =>
            {
                id
            }
            _ => {
                self.bad_value(
                    span,
                    "remember",
                    "`true`, or an id of letters, digits and `_`",
                    "it names the registry value the box is stored in, so it must not change \
                     between versions",
                );
                return None;
            }
        };
        if let Some(&first) = self.remembered.get(&id) {
            self.diags.push(
                Diagnostic::error(
                    Code::BadFieldValue,
                    span,
                    format!("`{id}` is remembered by two sections"),
                )
                .note_at("the first is at", first)
                .note("one id is one registry value, so the two would share one box"),
            );
            return None;
        }
        self.remembered.insert(id.clone(), span);
        Some(id)
    }
}

/// `memento.restore()`, the one call spelled on the block's name.
pub(super) fn restores(call: &Expr) -> bool {
    matches!(call.callee_field(), Some(("memento", name)) if name.text == "restore")
}

/// The prelude's restore line, which a written `memento.restore()` replaces.
pub(super) fn is_restore(line: &ir::Instruction) -> bool {
    matches!(line.args.as_slice(), [ir::Arg::Raw(name)] if name == "MementoSectionRestore")
}

impl BodyLowerer<'_, '_> {
    /// Top level of the installer's `onInit` only: that is the one place the
    /// prelude's line is dropped for it, so anywhere else would restore twice
    /// or not at all.
    pub(super) fn memento_restore(&mut self, call: &Expr) {
        let Expr::Call { args, span, .. } = call else {
            return;
        };
        if !self.requires.headers.contains("Memento") {
            self.diags.push(
                Diagnostic::error(
                    Code::MissingAttribute,
                    *span,
                    "`memento.restore()` needs a remembered section",
                )
                .note("write `remember = true` on a section whose box should be kept"),
            );
            return;
        }
        if !args.is_empty() {
            self.diags.push(
                Diagnostic::error(
                    Code::WrongArity,
                    *span,
                    "`memento.restore()` takes no arguments",
                )
                .note("the block says where the boxes are stored"),
            );
            return;
        }
        // The body's own scope, then the block's: any deeper is inside an `if`
        // or a loop.
        if self.half != Some(Half::Installer)
            || self.place != crate::table::Place::OnInit
            || self.scopes.len() != 2
        {
            self.diags.push(
                Diagnostic::error(
                    Code::WrongPlace,
                    *span,
                    "`memento.restore()` goes in the installer's `onInit`, not inside an `if` or a loop",
                )
                .note("without it, the restore runs first in `onInit`"),
            );
            return;
        }
        self.emit(ir::Instruction::new(
            "!insertmacro",
            vec![ir::Arg::raw("MementoSectionRestore")],
        ));
    }
}

/// Where the first section's `remember` is, looked for through blocks, groups
/// and tables but not into bodies, which hold no sections.
fn first_remember(expr: &Expr) -> Option<Span> {
    match expr {
        Expr::Call { args, .. } => {
            let section = expr.callee_name() == Some("section");
            args.iter().find_map(|arg| match arg {
                Expr::Table { fields, .. } if section => {
                    fields.iter().find_map(|field| match field {
                        TableField::Named { name, value } if name.text == "remember" => {
                            Some(value.span())
                        }
                        _ => None,
                    })
                }
                _ => first_remember(arg),
            })
        }
        Expr::Table { fields, .. } => fields.iter().find_map(|field| match field {
            TableField::Named { value, .. } | TableField::Positional { value } => {
                first_remember(value)
            }
        }),
        _ => None,
    }
}
