//! `languages { … }` — the locale tables and the dialog that picks between them
//! (§15.26).
//!
//! ```lua
//! languages {
//!   ask = { title = "Installer Language", info = "Please select a language." },
//!   locales = {
//!     English = { greeting = "Hello" },
//!     German  = { greeting = "Hallo" },
//!   },
//! }
//! ```
//!
//! Two fields and not one open map. An earlier shape put the locales at the top
//! level of the block with `ask` reserved among them, which reads as a Lua
//! **mixed table** — a map keyed by data with one key that is not data — and
//! `pairs()` over it would have to know the exception. A record whose fields are
//! `ask` and `locales` has no exception in it.
//!
//! **Locale-first inside `locales`**, so a translator owns one contiguous block
//! and a file split (§15.28) cuts along the same line. The output is the other
//! way round — one `LangString` name at a time, every locale — because that is
//! what NSIS reads, and rearranging it is the compiler's job rather than the
//! translator's.
//!
//! ## What this block is lowered before
//!
//! It runs in a pass of its own, before any body, for the reason §15.6 gives:
//! `lang.greeting` is a read the body lowerer has to resolve, and the block that
//! declares it may be written underneath the `installer {}` that uses it. Same
//! argument as [`super::Lowerer::claim_pass`], same answer.
//!
//! ## Include order, which is the point
//!
//! Four MUI2 macros here have to be inserted in four different places, and
//! getting one wrong is a `!warning` at best and a dialog that never appears at
//! worst. None of the four is written:
//!
//! | macro | where it must go | why |
//! | --- | --- | --- |
//! | `MUI_LANGUAGE` | after every page macro | it `!warning`s otherwise |
//! | `MUI_RESERVEFILE_LANGDLL` | after the language lines | the plugin has to be extractable before `.onInit` runs |
//! | `MUI_LANGDLL_DISPLAY` | first line of `.onInit` | it reads `MUI_LANGDLL_LANGUAGES`, which the language lines accumulate |
//! | `MUI_UNGETLANGUAGE` | first line of `un.onInit` | the uninstaller has no page to ask on |
//!
//! `MUI_LANGDLL_SAVELANGUAGE` is not in the table and is not exposed either:
//! MUI2's own `instfiles` page inserts it, at `Pages/InstallFiles.nsh:145`. It is
//! its state, not a setting (§15.23).

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::{Expr, Program, Stmt, TableField};
use crate::diag::{Code, Diagnostic, Span};
use crate::ir;
use crate::locale;
use crate::resolve::ConstValue;

use super::{Half, Lowerer, list};

/// One locale's table, in the order the source listed the locales.
struct Locale {
    name: String,
    span: Span,
    /// `LangString` name → text, sorted so the output is stable whatever order
    /// a translator wrote the entries in (§12: a Lua table has no order).
    strings: BTreeMap<String, (String, Span)>,
}

impl Lowerer<'_, '_> {
    /// The pass. Finds the block, rejects a second one, lowers the first.
    pub(super) fn languages_pass(&mut self, program: &Program) {
        let mut blocks = program.block.iter().filter_map(|stmt| {
            let Stmt::Call(call) = stmt else { return None };
            let name = call.callee_name()?;
            (name == "languages").then(|| (call, call.span()))
        });

        let Some((call, span)) = blocks.next() else {
            return;
        };
        for (_, second) in blocks {
            let first = span.start_line;
            self.diags.push(
                Diagnostic::error(
                    Code::DuplicateBlock,
                    second,
                    "`languages {}` appears more than once",
                )
                .note(format!("the first one is at line {first}"))
                .note("it is script-global, so there is exactly one (§15.10)"),
            );
        }

        let Some(fields) = self.block_fields(call) else {
            return;
        };
        self.languages(fields, span);
    }

    fn languages(&mut self, fields: &[TableField], span: Span) {
        let mut locales: Option<&Expr> = None;
        let mut ask: Option<&Expr> = None;

        for field in fields {
            let TableField::Named { name, value } = field else {
                self.bad_value(
                    field.span(),
                    "languages",
                    "named fields",
                    "the two are `locales = { … }` and `ask = { … }`",
                );
                continue;
            };
            match name.text.as_str() {
                "locales" => locales = Some(value),
                "ask" => ask = Some(value),
                other => {
                    // The mistake the old shape invited, named: a locale
                    // written where a field goes.
                    let note = if locale::is_locale(other) {
                        format!("`{other}` is a locale — it goes inside `locales = {{ … }}`")
                    } else {
                        "the two fields are `locales` and `ask`".to_string()
                    };
                    let span = name.span;
                    self.diags.push(
                        Diagnostic::error(
                            Code::UnknownField,
                            span,
                            format!("`{other}` is not a field of `languages {{}}`"),
                        )
                        .note(note),
                    );
                }
            }
        }

        let Some(locales) = locales else {
            self.diags.push(
                Diagnostic::error(
                    Code::MissingAttribute,
                    span,
                    "`languages {}` wants `locales`",
                )
                .note(
                    "write `locales = { English = { … } }`; a block with no locale in it \
                       would emit no language at all",
                ),
            );
            return;
        };

        let locales = self.locales(locales);
        if locales.is_empty() {
            return;
        }

        // Every name every locale mentions, so a name missing from one of them
        // is a diagnostic rather than an empty string on somebody's screen.
        self.completeness(&locales);

        // MUI2 is what `MUI_LANGUAGE` lives in, so a `languages {}` asks for it
        // whether or not the program has pages.
        self.mui = true;

        for locale in &locales {
            self.module.languages.push(ir::Instruction::new(
                "!insertmacro",
                vec![
                    ir::Arg::raw("MUI_LANGUAGE"),
                    ir::Arg::str(locale.name.clone()),
                ],
            ));
        }

        if let Some(ask) = ask {
            self.ask(ask);
        }

        // One name at a time, every locale — transposed from the source, which
        // is grouped the way a translator wants it.
        let mut names: BTreeSet<&str> = BTreeSet::new();
        for locale in &locales {
            names.extend(locale.strings.keys().map(String::as_str));
        }
        for name in &names {
            for locale in &locales {
                let Some((text, _)) = locale.strings.get(*name) else {
                    continue;
                };
                self.module.languages.push(ir::Instruction::new(
                    "LangString",
                    vec![
                        ir::Arg::raw(*name),
                        ir::Arg::raw(format!("${{{}}}", locale::define(&locale.name))),
                        ir::Arg::str(text.clone()),
                    ],
                ));
            }
        }
        self.lang_strings = names.iter().map(|name| (*name).to_string()).collect();
    }

    /// `locales = { English = { … }, German = { … } }`.
    fn locales(&mut self, value: &Expr) -> Vec<Locale> {
        let Expr::Table { fields, .. } = value else {
            self.bad_value(
                value.span(),
                "locales",
                "a table keyed by language",
                "`locales = { English = { greeting = \"Hello\" } }`",
            );
            return Vec::new();
        };

        let mut out: Vec<Locale> = Vec::new();
        for field in fields {
            let TableField::Named { name, value } = field else {
                self.bad_value(
                    field.span(),
                    "locales",
                    "a table keyed by language",
                    "every entry is `<Language> = { … }`",
                );
                continue;
            };

            if !locale::is_locale(&name.text) {
                self.unknown_locale(&name.text, name.span);
                continue;
            }
            if let Some(previous) = out.iter().find(|other| other.name == name.text) {
                let line = previous.span.start_line;
                let span = name.span;
                let text = name.text.clone();
                self.diags.push(
                    Diagnostic::error(
                        Code::DuplicateBlock,
                        span,
                        format!("`{text}` is listed twice"),
                    )
                    .note(format!("the first one is at line {line}")),
                );
                continue;
            }

            let strings = self.strings(value, &name.text);
            out.push(Locale {
                name: name.text.clone(),
                span: name.span,
                strings,
            });
        }
        out
    }

    /// One locale's `{ greeting = "Hello", … }`.
    fn strings(&mut self, value: &Expr, locale: &str) -> BTreeMap<String, (String, Span)> {
        let mut out = BTreeMap::new();
        let Expr::Table { fields, .. } = value else {
            self.bad_value(
                value.span(),
                locale,
                "a table of strings",
                "`English = { greeting = \"Hello\" }`",
            );
            return out;
        };

        for field in fields {
            let TableField::Named { name, value } = field else {
                self.bad_value(
                    field.span(),
                    locale,
                    "a table of strings",
                    "every entry is `<name> = \"…\"`, and the name is what `lang.<name>` reads",
                );
                continue;
            };
            let Some(ConstValue::Str(text)) = self.constant(value) else {
                self.bad_value(
                    value.span(),
                    &name.text,
                    "a string constant",
                    "a `LangString` is chosen by the preprocessor, so its text cannot be \
                     computed at run time (§7-1)",
                );
                continue;
            };
            out.insert(name.text.clone(), (text, name.span));
        }
        out
    }

    /// Every name in every locale, or a diagnostic naming both ends.
    ///
    /// NSIS does not check this. A `LangString` with no entry for the running
    /// language expands to nothing — an empty label, on one machine, in one
    /// country, which is the failure hardest to find and cheapest to prevent.
    fn completeness(&mut self, locales: &[Locale]) {
        let mut names: BTreeMap<&str, &Locale> = BTreeMap::new();
        for locale in locales {
            for name in locale.strings.keys() {
                names.entry(name).or_insert(locale);
            }
        }

        for (name, first) in names {
            for locale in locales {
                if locale.strings.contains_key(name) {
                    continue;
                }
                let line = first.strings[name].1.start_line;
                let (from, span) = (first.name.clone(), locale.span);
                let missing = locale.name.clone();
                self.diags.push(
                    Diagnostic::error(
                        Code::MissingAttribute,
                        span,
                        format!("`{missing}` has no `{name}`"),
                    )
                    .note(format!("`{from}` declares it, at line {line}"))
                    .note(
                        "NSIS expands a language string with no entry for the running language \
                         to nothing at all, so the gap would be an empty label rather than an \
                         error",
                    ),
                );
            }
        }
    }

    /// `ask = { … }` — the language dialog, and the three macros it drags with
    /// it.
    fn ask(&mut self, value: &Expr) {
        let Expr::Table { fields, .. } = value else {
            self.bad_value(
                value.span(),
                "ask",
                "a table of settings",
                "`ask = {}` on its own is the dialog with MUI2's own wording",
            );
            return;
        };

        let mut registry: [Option<String>; 3] = [None, None, None];

        for field in fields {
            let TableField::Named { name, value } = field else {
                self.bad_value(
                    field.span(),
                    "ask",
                    "named fields",
                    &format!(
                        "the fields are {}",
                        list(&["title", "info", "allLanguages", "alwaysShow", "remember"])
                    ),
                );
                continue;
            };
            match name.text.as_str() {
                "title" => self.ask_string(value, "title", "MUI_LANGDLL_WINDOWTITLE"),
                "info" => self.ask_string(value, "info", "MUI_LANGDLL_INFO"),
                "allLanguages" => self.ask_flag(value, "allLanguages", "MUI_LANGDLL_ALLLANGUAGES"),
                "alwaysShow" => self.ask_flag(value, "alwaysShow", "MUI_LANGDLL_ALWAYSSHOW"),
                "remember" => registry = self.remember(value),
                other => {
                    let span = name.span;
                    self.diags.push(
                        Diagnostic::error(
                            Code::UnknownField,
                            span,
                            format!("`{other}` is not one of `ask`'s settings"),
                        )
                        .note(format!(
                            "the settings are {}",
                            list(&["title", "info", "allLanguages", "alwaysShow", "remember"])
                        )),
                    );
                }
            }
        }

        // All three or none — [`Self::remember`] has already said so, and
        // returns nothing at all rather than a partial set.
        if let [Some(root), Some(key), Some(name)] = &registry {
            for (define, value) in [
                ("MUI_LANGDLL_REGISTRY_ROOT", root),
                ("MUI_LANGDLL_REGISTRY_KEY", key),
                ("MUI_LANGDLL_REGISTRY_VALUENAME", name),
            ] {
                self.module.mui_defines.push(ir::Define {
                    name: define.to_string(),
                    value: Some(ir::Arg::str(value.clone())),
                });
            }
        }

        // The plugin, reserved where MUI2's own examples put it: after the
        // language lines, so it is the first thing in the data block and
        // `.onInit` can extract it before anything else has been written.
        self.module.languages.push(ir::Instruction::new(
            "!insertmacro",
            vec![ir::Arg::raw("MUI_RESERVEFILE_LANGDLL")],
        ));

        // And the two insertion points. Queued rather than emitted, because the
        // function they belong in may not have been written at all — see
        // [`Lowerer::finish`], which invents whichever one is missing.
        self.init_prelude[Half::Installer.index()].push(ir::Instruction::new(
            "!insertmacro",
            vec![ir::Arg::raw("MUI_LANGDLL_DISPLAY")],
        ));
        self.init_prelude[Half::Uninstaller.index()].push(ir::Instruction::new(
            "!insertmacro",
            vec![ir::Arg::raw("MUI_UNGETLANGUAGE")],
        ));
    }

    fn ask_string(&mut self, value: &Expr, field: &str, define: &str) {
        let Some(ConstValue::Str(text)) = self.constant(value) else {
            self.bad_value(
                value.span(),
                field,
                "a string constant",
                "the dialog opens before any language is chosen, so its own wording cannot be a \
                 language string",
            );
            return;
        };
        self.module.mui_defines.push(ir::Define {
            name: define.to_string(),
            value: Some(ir::Arg::str(text)),
        });
    }

    fn ask_flag(&mut self, value: &Expr, field: &str, define: &str) {
        match self.constant(value) {
            Some(ConstValue::Bool(true)) => self.module.mui_defines.push(ir::Define {
                name: define.to_string(),
                value: None,
            }),
            // `false` is the default and writes nothing: MUI2 reads these with
            // `!ifdef`, so a define holding `false` would be true.
            Some(ConstValue::Bool(false)) => {}
            _ => self.bad_value(
                value.span(),
                field,
                "a `bool`",
                "MUI2 reads it with `!ifdef`, so there is no value to give it",
            ),
        }
    }

    /// `remember = { root = …, key = …, value = … }` — all three or none.
    ///
    /// MUI2 guards the variable it stores the answer in with a single
    /// `!ifdef ROOT & KEY & VALUENAME`, so two out of three is not a partial
    /// setting: it is the whole feature, silently off. Same shape and same
    /// argument as the start menu page's `registry` (§15.23).
    fn remember(&mut self, value: &Expr) -> [Option<String>; 3] {
        let mut out: [Option<String>; 3] = [None, None, None];
        let Expr::Table { fields, span } = value else {
            self.bad_value(
                value.span(),
                "remember",
                "a table of three",
                "`remember = { root = \"HKCU\", key = \"Software\\\\App\", value = \"Language\" }`",
            );
            return out;
        };

        for field in fields {
            let TableField::Named { name, value } = field else {
                continue;
            };
            let slot = match name.text.as_str() {
                "root" => 0,
                "key" => 1,
                "value" => 2,
                other => {
                    let span = name.span;
                    self.diags.push(
                        Diagnostic::error(
                            Code::UnknownField,
                            span,
                            format!("`{other}` is not part of `remember`"),
                        )
                        .note(format!("the three are {}", list(&["root", "key", "value"]))),
                    );
                    continue;
                }
            };
            match self.constant(value) {
                Some(ConstValue::Str(text)) => out[slot] = Some(text),
                _ => self.bad_value(
                    value.span(),
                    &name.text,
                    "a string constant",
                    "the registry location is read by the preprocessor, not at run time",
                ),
            }
        }

        if out.iter().any(Option::is_some) && out.iter().any(Option::is_none) {
            let missing: Vec<&str> = ["root", "key", "value"]
                .iter()
                .zip(&out)
                .filter(|(_, value)| value.is_none())
                .map(|(name, _)| *name)
                .collect();
            let span = *span;
            self.diags.push(
                Diagnostic::error(
                    Code::MissingAttribute,
                    span,
                    format!("`remember` is missing {}", list(&missing)),
                )
                .note(
                    "MUI2 guards the stored answer with one `!ifdef` over all three, so a \
                     partial `remember` is the whole feature switched off without saying so",
                ),
            );
            return [None, None, None];
        }
        out
    }

    fn unknown_locale(&mut self, name: &str, span: Span) {
        let mut diagnostic = Diagnostic::error(
            Code::UnknownField,
            span,
            format!("`{name}` is not a language NSIS ships"),
        );
        if let Some(nearest) = locale::nearest(name) {
            diagnostic = diagnostic.note(format!("did you mean `{nearest}`?"));
        }
        self.diags.push(diagnostic.note(
            "the names are the `.nlf` files in `Contrib/Language files`, and they are endonym-free: \
             `German`, not `Deutsch`",
        ));
    }
}
