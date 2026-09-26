//! `memento { … }` — the components page remembers what was ticked, through the
//! stock `Memento.nsh`.
//!
//! ```lua
//! memento { root = HKLM, key = "Software/Example/Components" }
//!
//! installer {
//!   section { "Docs", remember = "docs", body = function() … end },
//! }
//! ```
//!
//! Lowered through the header, like `multiUser {}`: the two fields are the
//! `MEMENTO_REGISTRY_*` defines, and a section with `remember` is written as
//! `MementoSectionEx`/`MementoSectionEnd` instead of `Section`/`SectionEnd`.
//! What the compiler adds is the bookkeeping the readme lists as steps:
//!
//! - `MementoSectionDone` after the last section;
//! - `MementoSectionRestore` in the init prelude, after `multiUser {}`'s, since
//!   an `SHCTX` root is only decided once that has run;
//! - `MementoSectionSave` first in `.onInstSuccess`: the author's, when the
//!   block has an `onInstSuccess`, or else one of its own.
//!
//! `remember` takes the id rather than `true`: it is the registry value's name,
//! and the header says it must not change between versions. A minted index or
//! the section's display name would both change without anyone deciding to.

use crate::ast::{Expr, Stmt, TableField};
use crate::diag::{Code, Diagnostic};
use crate::ir;
use crate::resolve::ConstValue;

use super::{Half, Lowerer};

impl Lowerer<'_, '_> {
    /// Its own pass, for `multiUser {}`'s reason, and after it: the restore
    /// line follows the mode line in `.onInit`.
    pub(super) fn memento_pass(&mut self) {
        let mut blocks = self.resolved.block.iter().filter_map(|stmt| {
            let Stmt::Call(call) = stmt else { return None };
            (call.callee_name()? == "memento").then(|| (call, call.span()))
        });

        let Some((call, span)) = blocks.next() else {
            return;
        };
        for (_, second) in blocks {
            self.diags.push(
                Diagnostic::error(
                    Code::DuplicateBlock,
                    second,
                    "`memento {}` appears more than once",
                )
                .note_at("the first one is at", span)
                .note("it is script-global, so there is exactly one"),
            );
        }
        // Set before the fields are read, so a section's `remember` under a
        // broken block is not told the block is missing as well.
        self.memento = Some(span);

        let Some(fields) = self.block_fields(call) else {
            return;
        };
        let mut pair = [None, None];
        for field in fields {
            let TableField::Named { name, value } = field else {
                continue;
            };
            match name.text.as_str() {
                "root" => {
                    // The header puts it straight after `ReadRegDWord`, so a
                    // word that is not a root is a makensis error far from here.
                    // Not a constant at all is `keyword`'s to report.
                    let Some(root) = self.keyword(value, "root") else {
                        continue;
                    };
                    match crate::builtins::constant_named(&root) {
                        Some(constant) if constant.is_root() => pair[0] = Some(ir::Arg::raw(root)),
                        _ => self.bad_value(
                            value.span(),
                            "root",
                            "a registry root",
                            "write `root = HKLM`, or `SHCTX` beside `multiUser {}`",
                        ),
                    }
                }
                "key" => pair[1] = self.constant_arg(value, "key").map(ir::Arg::into_path),
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

        let [Some(root), Some(key)] = pair else {
            let written = |which: &str| {
                fields.iter().any(
                    |field| matches!(field, TableField::Named { name, .. } if name.text == which),
                )
            };
            if !written("root") || !written("key") {
                self.diags.push(
                    Diagnostic::error(
                        Code::MissingAttribute,
                        span,
                        "`memento {}` wants both `root` and `key`",
                    )
                    .note("they are where the ticked boxes are stored"),
                );
            }
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

    /// A section's `remember = "id"`: the id, when the section may have one.
    pub(super) fn memento_id(&mut self, value: &Expr, half: Half) -> Option<String> {
        let span = value.span();
        self.remember_written = true;
        let id = match self.constant(value) {
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
                    "an id of letters, digits and `_`",
                    "it names the registry value the box is stored in, so it must not change \
                     between versions",
                );
                return None;
            }
        };
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
        if self.memento.is_none() {
            self.diags.push(
                Diagnostic::error(
                    Code::MissingAttribute,
                    span,
                    "`remember` needs a `memento {}` block",
                )
                .note("write `memento { root = HKLM, key = \"Software/App\" }`"),
            );
            return None;
        }
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
