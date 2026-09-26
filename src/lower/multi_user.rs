//! `multiUser { … }` — a per-machine or a per-user install, through the stock
//! `MultiUser.nsh`.
//!
//! ```lua
//! multiUser {
//!   executionLevel = "highest",
//!   commandLine = true,
//!   folder = "Example",
//! }
//! ```
//!
//! Lowered *through* the header, the way MUI2 is: every field is a
//! `MULTIUSER_*` define the header reads once, at its `!include`, so they go in
//! the `!define` slot above it. What the compiler adds is the part a script
//! gets wrong by hand:
//!
//! - `MULTIUSER_INIT` and `MULTIUSER_UNINIT` as lines of the init prelude,
//!   after `languages {}`'s dialog, so the header's message boxes are in the
//!   language that was picked;
//! - `MULTIUSER_NOUNINSTALL` when there is no `uninstaller {}`, since the
//!   header otherwise writes `un.` functions nothing calls;
//! - no second `RequestExecutionLevel`: the header writes one from
//!   `executionLevel`, and an `attributes {}` one below it would win silently.
//!
//! `MULTIUSER_MUI` is never written. All it does is `!include MUI2.nsh`, which
//! the compiler already does first; the strings `page.installMode` needs are
//! keyed on the page macro rather than on it.

use crate::ast::{Expr, Name, Stmt, TableField};
use crate::diag::{Code, Diagnostic, Span};
use crate::ir;
use crate::resolve::ConstValue;

use super::{BodyLowerer, Half, Lowerer, Requirements, list};

const FIELDS: &[&str] = &[
    "executionLevel",
    "commandLine",
    "defaultCurrentUser",
    "folder",
    "programFiles64",
    "folderRegistry",
    "modeRegistry",
];

/// What the rest of the lowering needs to know about the block.
pub(super) struct MultiUser {
    pub(super) span: Span,
    /// Whether the level lets the installer ask for all users at all. The
    /// install-mode page `!error`s without it: with `standard` there is no
    /// choice to offer.
    pub(super) all_users: bool,
}

impl Lowerer<'_, '_> {
    /// Its own pass, ahead of the bodies, for `languages {}`'s reason: an
    /// `.onInit` written above the block still has to start with its prelude.
    pub(super) fn multi_user_pass(&mut self) {
        let mut blocks = self.resolved.block.iter().filter_map(|stmt| {
            let Stmt::Call(call) = stmt else { return None };
            (call.callee_name()? == "multiUser").then(|| (call, call.span()))
        });

        let Some((call, span)) = blocks.next() else {
            return;
        };
        for (_, second) in blocks {
            self.diags.push(
                Diagnostic::error(
                    Code::DuplicateBlock,
                    second,
                    "`multiUser {}` appears more than once",
                )
                .note_at("the first one is at", span)
                .note("it is script-global, so there is exactly one"),
            );
        }

        let Some(fields) = self.block_fields(call) else {
            return;
        };
        self.multi_user(fields, span);
    }

    fn multi_user(&mut self, fields: &[TableField], span: Span) {
        let mut all_users = None;
        for field in fields {
            let TableField::Named { name, value } = field else {
                self.bad_value(
                    field.span(),
                    "multiUser",
                    "named fields",
                    "every setting is a `MULTIUSER_*` define, and a define has a name",
                );
                continue;
            };
            match name.text.as_str() {
                "executionLevel" => all_users = self.execution_level(value),
                "commandLine" => {
                    self.multi_user_flag(value, "commandLine", "MULTIUSER_INSTALLMODE_COMMANDLINE")
                }
                "defaultCurrentUser" => self.multi_user_flag(
                    value,
                    "defaultCurrentUser",
                    "MULTIUSER_INSTALLMODE_DEFAULT_CURRENTUSER",
                ),
                "programFiles64" => {
                    self.multi_user_flag(value, "programFiles64", "MULTIUSER_USE_PROGRAMFILES64")
                }
                "folder" => {
                    if let Some(arg) = self.constant_arg(value, "folder") {
                        let arg = Some(arg.into_path());
                        self.multi_user_define("MULTIUSER_INSTALLMODE_INSTDIR", arg);
                    }
                }
                "folderRegistry" => self.registry_pair(
                    value,
                    "folderRegistry",
                    "MULTIUSER_INSTALLMODE_INSTDIR_REGISTRY",
                ),
                "modeRegistry" => self.registry_pair(
                    value,
                    "modeRegistry",
                    "MULTIUSER_INSTALLMODE_DEFAULT_REGISTRY",
                ),
                other => {
                    let span = name.span;
                    self.diags.push(
                        Diagnostic::error(
                            Code::UnknownField,
                            span,
                            format!("`{other}` is not a field of `multiUser {{}}`"),
                        )
                        .note(format!("the fields are {}", list(FIELDS))),
                    );
                }
            }
        }

        let Some(all_users) = all_users else {
            // Unwritten, or written wrong and already reported. The header only
            // `!warning`s without one and then asks for no rights at all, which
            // is a silent per-user install.
            if !fields.iter().any(|field| {
                matches!(field, TableField::Named { name, .. } if name.text == "executionLevel")
            }) {
                self.diags.push(
                    Diagnostic::error(
                        Code::MissingAttribute,
                        span,
                        "`multiUser {}` wants `executionLevel`",
                    )
                    .note("write `executionLevel = \"highest\"` to offer both modes"),
                );
            }
            return;
        };

        self.requires.headers.insert("MultiUser".to_string());
        self.init_prelude[Half::Installer.index()].push(ir::Instruction::new(
            "!insertmacro",
            vec![ir::Arg::raw("MULTIUSER_INIT")],
        ));
        self.init_prelude[Half::Uninstaller.index()].push(ir::Instruction::new(
            "!insertmacro",
            vec![ir::Arg::raw("MULTIUSER_UNINIT")],
        ));
        self.multi_user = Some(MultiUser { span, all_users });
    }

    /// `"admin" | "power" | "highest" | "standard"`, and whether it can install
    /// for all users.
    fn execution_level(&mut self, value: &Expr) -> Option<bool> {
        let levels = ["admin", "power", "highest", "standard"];
        let word = match self.constant(value) {
            Some(ConstValue::Str(word)) if levels.contains(&word.as_str()) => word,
            _ => {
                self.bad_value(
                    value.span(),
                    "executionLevel",
                    &format!("one of {}", list(&levels)),
                    "the header writes `RequestExecutionLevel` from it and compares it as a word",
                );
                return None;
            }
        };
        // The header's spelling is capitalised: it compares `== Admin`.
        let mut nsis = word.clone();
        nsis[..1].make_ascii_uppercase();
        self.multi_user_define("MULTIUSER_EXECUTIONLEVEL", Some(ir::Arg::raw(nsis)));
        Some(word != "standard")
    }

    fn multi_user_flag(&mut self, value: &Expr, field: &str, define: &str) {
        match self.constant(value) {
            Some(ConstValue::Bool(true)) => self.multi_user_define(define, None),
            // The header asks `!ifdef`, so `false` is the absence of the define.
            Some(ConstValue::Bool(false)) => {}
            _ => self.bad_value(
                value.span(),
                field,
                "a `bool`",
                "the header reads it with `!ifdef`, so there is no value to give it",
            ),
        }
    }

    /// `{ key = …, value = … }` — both or neither. The header guards each pair
    /// with one `!ifdef KEY & VALUENAME`, so one alone is the whole feature
    /// switched off without a word. The root is the header's: `HKLM` for all
    /// users and `HKCU` for one.
    fn registry_pair(&mut self, value: &Expr, field: &str, stem: &str) {
        let Expr::Table { fields, span } = value else {
            self.bad_value(
                value.span(),
                field,
                "a table",
                &format!("`{field} = {{ key = \"Software\\\\App\", value = \"InstallDir\" }}`"),
            );
            return;
        };
        let mut pair = [None, None];
        for entry in fields {
            let TableField::Named { name, value } = entry else {
                continue;
            };
            let slot = match name.text.as_str() {
                "key" => 0,
                "value" => 1,
                other => {
                    let span = name.span;
                    self.diags.push(
                        Diagnostic::error(
                            Code::UnknownField,
                            span,
                            format!("`{other}` is not part of `{field}`"),
                        )
                        .note("the two are `key` and `value`"),
                    );
                    continue;
                }
            };
            // Written but not a constant is reported by `constant_arg`, and is
            // not also missing.
            // The key is a path like any other registry key, `/` and all.
            let arg = self.constant_arg(value, field);
            pair[slot] = Some(if slot == 0 {
                arg.map(ir::Arg::into_path)
            } else {
                arg
            });
        }
        let [Some(key), Some(name)] = pair else {
            let span = *span;
            self.diags.push(
                Diagnostic::error(
                    Code::MissingAttribute,
                    span,
                    format!("`{field}` wants both `key` and `value`"),
                )
                .note("the header reads the two under one `!ifdef`, so one alone does nothing"),
            );
            return;
        };
        if let (Some(key), Some(name)) = (key, name) {
            self.multi_user_define(&format!("{stem}_KEY"), Some(key));
            self.multi_user_define(&format!("{stem}_VALUENAME"), Some(name));
        }
    }

    /// In the user's `!define` slot rather than MUI2's: the header reads these
    /// at its `!include`, and that slot is the one above it.
    fn multi_user_define(&mut self, name: &str, value: Option<ir::Arg>) {
        self.module.defines.push(ir::Define {
            name: name.to_string(),
            value,
        });
    }
}

/// `multiUser.installMode` and `multiUser.privileges`: the header's two `Var`s,
/// which `MULTIUSER_INIT` and `MULTIUSER_UNINIT` fill in. Read-only, because
/// the header changes mode through its own functions, and a bare write would
/// leave `SHCTX` and `$INSTDIR` on the old one.
const VARS: &[(&str, &str)] = &[
    ("installMode", "$MultiUser.InstallMode"),
    ("privileges", "$MultiUser.Privileges"),
];

/// The variable `multiUser.x` reads, folded like a `lang.x`; anything else
/// falls through to [`BodyLowerer::multi_user_field`] for its diagnostic.
pub(super) fn var_ref(expr: &Expr, requires: &Requirements) -> Option<&'static str> {
    let Expr::Field { base, name, .. } = expr else {
        return None;
    };
    if !matches!(&**base, Expr::Name(base) if base.text == "multiUser")
        || !requires.headers.contains("MultiUser")
    {
        return None;
    }
    VARS.iter()
        .find(|(field, _)| *field == name.text)
        .map(|(_, var)| *var)
}

impl BodyLowerer<'_, '_> {
    /// A `multiUser.x` that [`var_ref`] did not fold, or any write to one.
    pub(super) fn multi_user_field(&mut self, field: &Name, write: bool) {
        let diagnostic = if !self.requires.headers.contains("MultiUser") {
            Diagnostic::error(
                Code::MissingAttribute,
                field.span,
                format!("`multiUser.{}` needs a `multiUser {{}}` block", field.text),
            )
            .note("the header that sets it is only included beside one")
        } else if !VARS.iter().any(|(name, _)| *name == field.text) {
            Diagnostic::error(
                Code::UnknownField,
                field.span,
                format!("`{}` is not a field of `multiUser`", field.text),
            )
            .note("the two are `installMode` and `privileges`")
        } else if write {
            Diagnostic::error(
                Code::BadFieldValue,
                field.span,
                format!("`multiUser.{}` is not something to assign to", field.text),
            )
            .note("the user picks the mode, on `page.installMode` or with `commandLine = true`")
        } else {
            return;
        };
        self.diags.push(diagnostic);
    }
}
