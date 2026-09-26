//! `installLib(…)` and `uninstallLib(…)` — a DLL, type library or COM server,
//! through the stock `Library.nsh`.
//!
//! ```lua
//! installLib("bin/shared.dll", SYSDIR .. "/shared.dll", { type = "REGDLL", reboot = true })
//! uninstallLib(SYSDIR .. "/shared.dll", { type = "REGDLL", shared = true, remove = true })
//! ```
//!
//! A lowering rather than a declaration, for the reason `declarations.rs`
//! gives: the version check, the reboot fallback, registration and the shared
//! DLL count are behaviour, and the header already gets them right. So each
//! call is one `!insertmacro`, and the table is its keyword arguments spelt as
//! fields: `reboot` and `protected` are the two halves of `REBOOT_PROTECTED`,
//! and the `LIBRARY_*` switches the header reads are `!define`d around the one
//! call that asked for them.
//!
//! Both macros save and restore every register they touch, so the line is an
//! ordinary instruction and not an opaque site: there is nothing to save.

use crate::ast::{Expr, TableField};
use crate::diag::{Code, Diagnostic, Span};
use crate::ir;
use crate::resolve::ConstValue;
use crate::types::Ty;

use super::{BodyLowerer, list};

const TYPES: &[&str] = &["DLL", "REGDLL", "TLB", "REGDLLTLB", "REGEXE"];

/// The `bool` fields that are a `!define` the header reads, and which macro
/// reads each.
const SWITCHES: &[(&str, &str, bool)] = &[
    ("x64", "LIBRARY_X64", true),
    ("shellExtension", "LIBRARY_SHELL_EXTENSION", true),
    ("com", "LIBRARY_COM", true),
    ("ignoreVersion", "LIBRARY_IGNORE_VERSION", false),
    ("equalVersion", "LIBRARY_INSTALL_EQUAL_VERSION", false),
];

impl BodyLowerer<'_, '_> {
    pub(super) fn library(
        &mut self,
        name: &str,
        args: &[Expr],
        dest: Option<&crate::regs::Slot>,
        span: Span,
    ) -> Option<Ty> {
        let install = name == "installLib";
        if dest.is_some() {
            self.diags.push(Diagnostic::error(
                Code::TypeMismatch,
                span,
                format!("`{name}` produces no value"),
            ));
            return None;
        }
        let positional = if install { 2 } else { 1 };
        let (files, fields) = match args {
            [files @ .., Expr::Table { fields, .. }] if files.len() == positional => {
                (files, fields.as_slice())
            }
            files if files.len() == positional => (files, &[][..]),
            _ => {
                self.diags.push(
                    Diagnostic::error(
                        Code::WrongArity,
                        span,
                        format!("`{name}` takes {positional} argument(s) and an options table"),
                    )
                    .note(if install {
                        "write `installLib(\"bin/x.dll\", SYSDIR .. \"/x.dll\", { … })`"
                    } else {
                        "write `uninstallLib(SYSDIR .. \"/x.dll\", { … })`"
                    }),
                );
                return None;
            }
        };

        let (mut kind, mut shared, mut temp) = ("DLL".to_string(), None, None);
        let (mut remove, mut reboot, mut protected) = (false, false, false);
        let mut switches = Vec::new();
        for field in fields {
            let TableField::Named { name: key, value } = field else {
                self.todo(span, &format!("a positional entry in `{name}`'s options"));
                return None;
            };
            let flag = |this: &mut Self| match this.constant(value) {
                Some(ConstValue::Bool(on)) => Some(on),
                _ => {
                    this.bad_value(
                        value.span(),
                        &key.text,
                        "`true` or `false`",
                        "it is a switch",
                    );
                    None
                }
            };
            match key.text.as_str() {
                "type" => match self.constant(value) {
                    Some(ConstValue::Str(text)) if TYPES.contains(&text.as_str()) => kind = text,
                    _ => {
                        self.bad_value(
                            value.span(),
                            "type",
                            "a library type",
                            &format!("the types are {}", list(TYPES)),
                        );
                        return None;
                    }
                },
                // `InstallLib` counts the DLL as shared when this string is
                // empty, which is a first install; `UnInstallLib` just asks.
                "shared" if install => shared = Some(self.value(value)?.arg),
                "shared" => shared = flag(self)?.then_some(ir::Arg::raw("SHARED")),
                "tempDir" if install => temp = Some(self.value(value)?.arg.into_path()),
                "remove" if !install => remove = flag(self)?,
                "reboot" => reboot = flag(self)?,
                "protected" => protected = flag(self)?,
                other => match SWITCHES
                    .iter()
                    .find(|(field, _, both)| *field == other && (install || *both))
                {
                    Some((_, define, _)) => {
                        if flag(self)? {
                            switches.push(*define);
                        }
                    }
                    None => {
                        self.diags.push(Diagnostic::error(
                            Code::UnknownField,
                            key.span,
                            format!("`{other}` is not an option of `{name}`"),
                        ));
                        return None;
                    }
                },
            }
        }

        let mode = format!(
            "{}REBOOT_{}PROTECTED",
            if reboot { "" } else { "NO" },
            if protected { "" } else { "NOT" }
        );
        let arguments = if install {
            let local = match self.constant(&files[0]) {
                Some(ConstValue::Str(path)) => ir::Arg::path(path),
                _ => {
                    self.bad_value(
                        files[0].span(),
                        "installLib",
                        "a path known at build time",
                        "`makensis` reads the file's version while it builds",
                    );
                    return None;
                }
            };
            let destination = self.value(&files[1])?.arg.into_path();
            let Some(temp) = temp.or_else(|| parent(&destination)) else {
                self.bad_value(
                    files[1].span(),
                    "installLib",
                    "a destination ending in a file name, or a `tempDir`",
                    "a file in use is copied to `tempDir` and moved on reboot, so it must be on \
                     the destination's drive",
                );
                return None;
            };
            // `InstallLib` compares the text of `shared` bare, so a literal
            // `""` would be no argument at all; a register always reads.
            // ponytail: a value allocated to `$R4`/`$R5` is overwritten before
            // the macro reads it, which needs fifteen live values to happen.
            let shared = match shared {
                None => ir::Arg::raw("NOTSHARED"),
                Some(arg) if is_register(&arg) => arg,
                Some(arg) => {
                    let slot = self.claim_temp(span);
                    self.emit(ir::Instruction::new(
                        "StrCpy",
                        vec![ir::Arg::dest(slot.clone()), arg],
                    ));
                    ir::Arg::slot(slot)
                }
            };
            vec![
                ir::Arg::raw("InstallLib"),
                ir::Arg::raw(kind),
                shared,
                ir::Arg::raw(mode),
                local,
                destination,
                temp,
            ]
        } else {
            let mode = if remove {
                mode
            } else if reboot || protected {
                self.bad_value(
                    span,
                    "reboot",
                    "`remove = true`",
                    "`reboot` and `protected` say how the file is removed",
                );
                return None;
            } else {
                "NOREMOVE".to_string()
            };
            let file = self.value(&files[0])?.arg.into_path();
            vec![
                ir::Arg::raw("UnInstallLib"),
                ir::Arg::raw(kind),
                shared.unwrap_or(ir::Arg::raw("NOTSHARED")),
                ir::Arg::raw(mode),
                file,
            ]
        };

        self.requires.headers.insert("Library".to_string());
        for define in &switches {
            self.emit(ir::Instruction::new("!define", vec![ir::Arg::raw(*define)]));
        }
        self.emit(ir::Instruction::new("!insertmacro", arguments));
        for define in &switches {
            self.emit(ir::Instruction::new("!undef", vec![ir::Arg::raw(*define)]));
        }
        None
    }
}

fn is_register(arg: &ir::Arg) -> bool {
    matches!(arg, ir::Arg::Data { pieces, .. }
        if matches!(pieces.as_slice(), [ir::Piece::Slot(_) | ir::Piece::Var(_)]))
}

/// `$SYSDIR\shared.dll` to `$SYSDIR`: the default `tempDir`, which the header
/// wants on the destination's drive.
fn parent(path: &ir::Arg) -> Option<ir::Arg> {
    let ir::Arg::Data { pieces, .. } = path else {
        return None;
    };
    let (ir::Piece::Text(last), rest) = pieces.split_last()? else {
        return None;
    };
    let cut = last.rfind(['/', '\\'])?;
    let mut pieces = rest.to_vec();
    if cut > 0 {
        pieces.push(ir::Piece::Text(last[..cut].to_string()));
    }
    (!pieces.is_empty()).then_some(ir::Arg::Data { pieces, path: true })
}
