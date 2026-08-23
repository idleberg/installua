//! Handles and their fields: `core.selected`, `docs.text = ""`, `serial.value`.
//!
//! A handle is the §13 binding made runtime-visible. `local core = section { … }`
//! binds no value — there is nothing at run time for `core` to *be* — and the
//! block that lists it turns it into a `!define`, so `core.selected` is
//! `SectionGetFlags ${SEC_core}` and the bit test after it. The number NSIS
//! actually wants never appears in the source, which is the whole point: a
//! section can be moved, renamed or given a sibling and nothing renumbers.
//!
//! Two things are decided here rather than in [`super::expr`]. The first is
//! claim rule 4 — a handle read from the half that did not list it names a
//! section that is not in that executable, and NSIS would not notice, because
//! one `.nsi` holds both halves and the define exists either way. The second is
//! the read-modify-write: `Sections.nsh` spells `SelectSection` as a macro that
//! clobbers `$0`, and this compiler has a register allocator, so it emits the
//! three instructions itself (ruling 6).
//!
//! A **control**'s field is the same surface over a different mechanism, and the
//! difference is worth naming: a section's seven fields are seven bits of one
//! word, so every write is a read-modify-write, and a control's seven are seven
//! separate instructions, so none of them is. What the two share is the four
//! claim rules — a control named from the half that did not draw it is rule 4 in
//! the same words with a `Var` in place of an index (§15.32).

use crate::ast::{Expr, Name, TableField};
use crate::diag::{Code, Diagnostic, Span};
use crate::ir;
use crate::regs::Slot;
use crate::resolve::{ConstValue, DeferredKind};
use crate::types::{Int, Sign, Ty, Width};

use super::control::{self, ControlField};
use super::fields::Fields;
use super::{
    Binding, BodyLowerer, HANDLE_FIELDS, Half, HandleField, Where, control_var, handle_field,
    index_name, list, start_menu_var,
};

/// A handle, resolved: the define it is addressed through and which of the two
/// things it is.
pub(super) struct Handle {
    index: String,
    kind: DeferredKind,
}

/// A window, resolved: where its `HWND` lives, and what the compiler knows about
/// how it was drawn.
pub(super) struct ControlHandle {
    slot: Slot,
    /// The kind, for a control this compiler created. `None` is a window it did
    /// not — `getDlgItem(HWNDPARENT, 2)` is MUI2's Cancel button — which has the
    /// fields every window has and not the two that depend on the class.
    control: Option<&'static control::Control>,
    /// What the program calls it, for the diagnostics that name it.
    base: String,
}

/// What `a.b` is addressing. The two halves of the field surface are told apart
/// here, once, so that neither has to ask what the other is.
pub(super) enum Addressed {
    Section(Handle),
    Control(ControlHandle),
    /// `local menu = page.startMenu { … }`, addressed from either half — the
    /// one handle claim rule 4 does not apply to, because MUI2 gives the
    /// uninstaller a way to reach the installer's answer.
    StartMenu(StartMenuHandle),
}

/// A start menu page, resolved: the id MUI2's macros take, and the `Var` the
/// page stored the folder into.
pub(super) struct StartMenuHandle {
    id: String,
    var: Slot,
}

/// A window message, in the hexadecimal every Windows reference writes it in.
/// `0x000C` is findable and `12` is not.
pub(super) fn message(number: u32) -> ir::Arg {
    ir::Arg::raw(format!("0x{number:04X}"))
}

/// `LoadAndSetImage`'s arguments with the control's own left out, since the two
/// callers — a `bitmap`'s declaration and a write to `image` — know where their
/// handle is and neither should know the constants.
pub(super) fn image_args(path: ir::Arg) -> Vec<ir::Arg> {
    vec![
        // The image is named by a string — a file beside the installer at run
        // time — rather than by an id compiled into the executable.
        ir::Arg::raw("/STRINGID"),
        ir::Arg::int(control::IMAGE_BITMAP.into()),
        ir::Arg::raw(format!("0x{:04X}", control::LR_LOADFROMFILE)),
        path,
    ]
}

impl BodyLowerer<'_, '_> {
    /// What the base of `a.b` addresses.
    ///
    /// Three answers and one of them is silence: a declaration this half listed,
    /// a window in a register, or a name with no field surface at all — and the
    /// last of those is a `todo` rather than an error, because `a.b` on
    /// something else is a shape this version lacks rather than a mistake.
    pub(super) fn addressed(&mut self, base: &Expr, span: Span) -> Option<Addressed> {
        let Some(name) = base.name() else {
            self.todo(span, "this expression");
            return None;
        };

        if let Some(kind) = self
            .resolved
            .deferred
            .get(name)
            .map(|deferred| deferred.kind)
        {
            // Unclaimed. Claim rule 1 has already said so at the declaration,
            // and saying it again at every use would bury it.
            let claim = self.claims.get(name)?;

            // The one declaration both halves may name. `MUI_STARTMENU_GETFOLDER`
            // exists precisely so the uninstaller can find the folder the
            // installer's page chose — MUI2 says so in `StartMenu.nsh` — so the
            // rule below would refuse the use the macro was written for.
            if kind == DeferredKind::StartMenu {
                return Some(Addressed::StartMenu(StartMenuHandle {
                    id: name.to_string(),
                    var: Slot::Global(start_menu_var(name)),
                }));
            }

            // Claim rule 4. The name is in the file — one `.nsi` holds both
            // halves — so `${SEC_core}` in `un.onInit` compiles, addresses
            // whatever index the installer's `Core` got, and means nothing in an
            // executable that does not contain it. A control's `Var` is worse
            // in the same way: it is declared, it is empty, and `SendMessage 0`
            // is a silent no-op.
            if let Some(half) = self.half
                && claim.half != half
            {
                self.diags.push(
                    Diagnostic::error(
                        Code::UnknownField,
                        span,
                        format!("`{name}` is a `{}` of the `{}`", kind.word(), claim.half),
                    )
                    .note(format!(
                        "this code runs in the `{half}`, which has no `{name}`"
                    ))
                    .note(match kind.is_control() {
                        true => {
                            "the two halves are two executables: the other's dialog is not drawn \
                             here, so its handle is a `Var` that never gets a window (§15.32)"
                        }
                        false => {
                            "the two halves are two executables: an index from one is a number \
                             the other's sections do not share (§13)"
                        }
                    }),
                );
                return None;
            }

            return Some(match kind {
                DeferredKind::Control(control) => Addressed::Control(ControlHandle {
                    slot: Slot::Global(control_var(name, claim.half)),
                    control: Some(control),
                    base: name.to_string(),
                }),
                kind => Addressed::Section(Handle {
                    index: index_name(name, claim.half),
                    kind,
                }),
            });
        }

        // A window this compiler did not draw, in a register: `getDlgItem`
        // reaches MUI2's own buttons, and `findWindow` reaches other programs'.
        // The lattice has one `handle` type covering files, registry roots and
        // windows alike (§15.14), so this accepts more than it should — a
        // `fileOpen` handle has an `enabled` here — and narrowing it is a fifth
        // type rather than a check.
        match self.lookup(name) {
            Some(Binding::Local { slot, ty }) if *ty == Ty::Handle || *ty == Ty::Unknown => {
                Some(Addressed::Control(ControlHandle {
                    slot: slot.clone(),
                    control: None,
                    base: name.to_string(),
                }))
            }
            _ => {
                self.todo(span, "this expression");
                None
            }
        }
    }

    /// The field, checked against what the handle is. `expanded` is a heading's
    /// and `size` is a section's, and the handle knows which it holds.
    fn field(&mut self, handle: &Handle, field: &Name) -> Option<HandleField> {
        let Some(what) = handle_field(&field.text) else {
            self.diags.push(
                Diagnostic::error(
                    Code::UnknownField,
                    field.span,
                    format!("`{}` is not a field of a section", field.text),
                )
                .note(format!("the fields are {}", list(HANDLE_FIELDS))),
            );
            return None;
        };
        let on = match what {
            HandleField::Flag { on, .. } => on,
            // A group has no size and is in no install type: what it holds is
            // sections, and each of those answers for itself.
            HandleField::Size | HandleField::InstallTypes => Where::Sections,
            HandleField::Text => Where::Both,
        };
        if !on.accepts(handle.kind) {
            let (is, isnt) = match handle.kind {
                DeferredKind::Group => ("a `group`", "a `section`"),
                // A section, or a control that never gets here: `Self::handle`
                // turns a control back at the door.
                _ => ("a `section`", "a `group`"),
            };
            self.diags.push(
                Diagnostic::error(
                    Code::UnknownField,
                    field.span,
                    format!("`{}` is {isnt}'s field, and this is {is}", field.text),
                )
                .note(match what {
                    HandleField::Flag { .. } => {
                        "`expanded` opens a heading in the components tree, and a section is not \
                         one"
                    }
                    _ => {
                        "a group holds sections and nothing else: the size and the install types \
                         are each section's"
                    }
                }),
            );
            return None;
        }
        Some(what)
    }

    /// `a.b` as a value, whichever kind of handle `a` turns out to be.
    pub(super) fn field_read(&mut self, base: &Expr, field: &Name, dest: &Slot) -> Option<Ty> {
        // A known `lang.greeting` never reaches here — `simple` folds it into
        // `$(greeting)` — so one that does is a name no locale declared. Said
        // here rather than there because `simple` is speculative and a
        // diagnostic raised speculatively is a diagnostic raised twice.
        if let Expr::Name(name) = base
            && name.text == "lang"
        {
            self.unknown_lang_string(field);
            return None;
        }

        match self.addressed(base, base.span())? {
            Addressed::Section(handle) => self.handle_read(&handle, field, dest),
            Addressed::Control(handle) => self.control_read(&handle, field, dest),
            Addressed::StartMenu(handle) => self.start_menu_read(&handle, field, dest),
        }
    }

    /// `a.b = c`, likewise.
    pub(super) fn field_write(&mut self, base: &Expr, field: &Name, value: &Expr) {
        match self.addressed(base, base.span()) {
            Some(Addressed::Section(handle)) => self.handle_write(&handle, field, value),
            Some(Addressed::Control(handle)) => self.control_write(&handle, field, value),
            // Read-only, and not for want of a `StrCpy`: the page reads the
            // variable when it opens and writes it when it leaves, so a value
            // put there by install-time code is either overwritten or too late.
            // `defaultFolder` is where a script says what it wants.
            Some(Addressed::StartMenu(_)) => self.diags.push(
                Diagnostic::error(
                    Code::BadFieldValue,
                    field.span,
                    format!("`{}` is not something to assign to", field.text),
                )
                .note(
                    "the start menu folder is the user's answer: write `defaultFolder` on the \
                     page to choose what it starts as",
                ),
            ),
            None => {}
        }
    }

    /// `lang.nosuch` — a language string nothing declares.
    ///
    /// Worth an error rather than a pass-through, because NSIS does not raise
    /// one: an undeclared `$(name)` is `warning 6000` at best and an empty
    /// label at worst, and empty is exactly what a missing translation looks
    /// like when it is working correctly.
    fn unknown_lang_string(&mut self, field: &Name) {
        let mut diagnostic = Diagnostic::error(
            Code::UnknownField,
            field.span,
            format!("`{}` is not a language string", field.text),
        );
        diagnostic = if self.lang_strings.is_empty() {
            diagnostic.note(
                "nothing declares one: `languages { locales = { English = { … } } }` is where                  they live (§15.26)",
            )
        } else {
            let names: Vec<&str> = self.lang_strings.iter().map(String::as_str).collect();
            diagnostic.note(format!("the strings are {}", list(&names)))
        };
        self.diags.push(diagnostic);
    }

    /// `menu.write(function() … end)` — the region MUI2 wraps the shortcut
    /// writing in.
    ///
    /// The two macros are one construct because they are useless apart.
    /// `MUI_STARTMENU_WRITE_BEGIN` skips the region when the user ticked *do not
    /// create shortcuts* and fills the variable from the registry when it is
    /// empty; `…_WRITE_END` writes the folder back. A script that opened one and
    /// not the other would leave an `${if}` hanging.
    ///
    /// Lowered **inline** rather than into a function of its own, which is the
    /// choice that matters: the closure is written inside a section body and
    /// reads that body's locals, and a generated `Function` would put them out
    /// of scope. Reverse postorder is what makes it safe — the blocks the body
    /// creates all reach the join, so they are laid out between the two lines
    /// (§8-2).
    pub(super) fn start_menu_write(
        &mut self,
        base: &str,
        method: &str,
        args: &[Expr],
        dest: Option<&Slot>,
        span: Span,
    ) {
        // Unclaimed: claim rule 1 has already said so at the declaration.
        if self.claims.get(base).is_none() {
            return;
        }
        if method != "write" {
            self.diags.push(
                Diagnostic::error(
                    Code::UnknownField,
                    span,
                    format!("`{method}` is not something a start menu page does"),
                )
                .note("the one is `write`, which wraps the code that creates the shortcuts"),
            );
            return;
        }
        if dest.is_some() {
            self.diags.push(
                Diagnostic::error(Code::TypeMismatch, span, "`write` produces no value")
                    .note("it is a region of install-time code, not a question about the folder"),
            );
            return;
        }
        // The uninstaller has no page to have chosen a folder and no registry
        // value to write back — `…_WRITE_END` would store the folder the
        // installer already stored. Deleting shortcuts needs the folder and not
        // the region, and `menu.folder` is that.
        if self.half == Some(Half::Uninstaller) {
            self.diags.push(
                Diagnostic::error(
                    Code::BadFieldValue,
                    span,
                    "`write` is the installer's, and this code runs in the uninstaller",
                )
                .note(
                    "it writes the chosen folder back to the registry, which is the installer's \
                     half of the bargain",
                )
                .note("read `menu.folder` here instead: it finds the folder the installer chose"),
            );
            return;
        }
        let [
            Expr::Function {
                params,
                block,
                span: at,
            },
        ] = args
        else {
            self.todo(span, "this `write` form");
            return;
        };
        if !params.is_empty() {
            self.diags.push(
                Diagnostic::error(
                    Code::WrongArity,
                    *at,
                    "the function `write` takes has no parameters",
                )
                .note("it is a region of the section it is written in, not a callback"),
            );
            return;
        }

        self.emit(ir::Instruction::new(
            "!insertmacro",
            vec![
                ir::Arg::raw("MUI_STARTMENU_WRITE_BEGIN"),
                ir::Arg::raw(base.to_string()),
            ],
        ));
        self.block(block);
        self.emit(ir::Instruction::new(
            "!insertmacro",
            vec![ir::Arg::raw("MUI_STARTMENU_WRITE_END")],
        ));
    }

    /// `menu.folder` — the folder the page chose, in whichever half is asking.
    ///
    /// Two lowerings for one spelling, and the half decides. In the installer
    /// the page stored it in the `Var` and the read is a copy. In the
    /// uninstaller there was no page: `MUI_STARTMENU_GETFOLDER` is MUI2's own
    /// answer to that, reading the registry the page wrote and falling back to
    /// `MUI_STARTMENUPAGE_DEFAULTFOLDER`. It lowers straight into the
    /// destination rather than into the `Var`, because in the installer that
    /// would overwrite the user's choice with the default.
    fn start_menu_read(
        &mut self,
        handle: &StartMenuHandle,
        field: &Name,
        dest: &Slot,
    ) -> Option<Ty> {
        if field.text != "folder" {
            self.diags.push(
                Diagnostic::error(
                    Code::UnknownField,
                    field.span,
                    format!("`{}` is not a field of a start menu page", field.text),
                )
                .note("the field is `folder`"),
            );
            return None;
        }
        match self.half {
            Some(Half::Installer) => self.emit(ir::Instruction::new(
                "StrCpy",
                vec![
                    ir::Arg::dest(dest.clone()),
                    ir::Arg::slot(handle.var.clone()),
                ],
            )),
            Some(Half::Uninstaller) => self.emit(ir::Instruction::new(
                "!insertmacro",
                vec![
                    ir::Arg::raw("MUI_STARTMENU_GETFOLDER"),
                    ir::Arg::raw(handle.id.clone()),
                    ir::Arg::dest(dest.clone()),
                ],
            )),
            // A `func` either half may call, and the two lowerings are not the
            // same code. Nothing here can pick one, and picking wrong is silent
            // — the uninstaller would copy an empty `Var`.
            None => {
                self.diags.push(
                    Diagnostic::error(
                        Code::NotYetImplemented,
                        field.span,
                        "a start menu folder read in a `func`",
                    )
                    .note(
                        "the installer reads the variable the page filled in and the uninstaller \
                         reads the registry, and a `func` is called by both (§15.3)",
                    )
                    .note("read `menu.folder` in the section or callback that needs it"),
                );
                return None;
            }
        }
        Some(Ty::Str)
    }

    /// `core.selected` — a read, which is one `Section*Get` and, for a flag, the
    /// bit test after it.
    fn handle_read(&mut self, handle: &Handle, field: &Name, dest: &Slot) -> Option<Ty> {
        let index = ir::Arg::raw(format!("${{{}}}", handle.index));
        match self.field(handle, field)? {
            HandleField::Flag { shift, .. } => {
                let flags = self.claim_temp(field.span);
                self.emit(ir::Instruction::new(
                    "SectionGetFlags",
                    vec![index, ir::Arg::dest(flags.clone())],
                ));
                // Down to bit 0 and masked. A `bool` in this compiler is `0` or
                // `1` and nothing else (§15.20), so `SF_RO` being 16 is not a
                // truth value that happens to work — it is the wrong number.
                let source = if shift == 0 {
                    ir::Arg::slot(flags)
                } else {
                    self.emit(ir::Instruction::new(
                        "IntOp",
                        vec![
                            ir::Arg::dest(dest.clone()),
                            ir::Arg::slot(flags),
                            ir::Arg::raw(">>>"),
                            ir::Arg::int(i64::from(shift)),
                        ],
                    ));
                    ir::Arg::slot(dest.clone())
                };
                self.emit(ir::Instruction::new(
                    "IntOp",
                    vec![
                        ir::Arg::dest(dest.clone()),
                        source,
                        ir::Arg::raw("&"),
                        ir::Arg::int(1),
                    ],
                ));
                Some(Ty::Bool)
            }
            HandleField::Text => {
                self.emit(ir::Instruction::new(
                    "SectionGetText",
                    vec![index, ir::Arg::dest(dest.clone())],
                ));
                Some(Ty::Str)
            }
            HandleField::Size => {
                self.emit(ir::Instruction::new(
                    "SectionGetSize",
                    vec![index, ir::Arg::dest(dest.clone())],
                ));
                // Kilobytes, and a section cannot give space back.
                Some(Ty::Int(Int {
                    width: Width::W32,
                    sign: Sign::NonNeg,
                }))
            }
            // `SectionGetInstTypes` yields the bit field NSIS stores, and the
            // field's type is `string[]`. There is no list value in this
            // language to decode it into, so the read waits for one rather than
            // handing back a number the surface never promised.
            HandleField::InstallTypes => {
                self.todo(field.span, "reading `installTypes`");
                None
            }
        }
    }

    /// `docs.text = ""` — a write, which is one `Section*Set` and, for a flag,
    /// the read-modify-write around it.
    fn handle_write(&mut self, handle: &Handle, field: &Name, value: &Expr) {
        let Some(what) = self.field(handle, field) else {
            return;
        };
        let index = || ir::Arg::raw(format!("${{{}}}", handle.index));
        match what {
            HandleField::Flag { bit, shift, .. } => {
                self.write_flag(&index(), bit, shift, value, field)
            }
            HandleField::Text => {
                let Some(text) = self.value(value) else {
                    return;
                };
                if text.ty != Ty::Str && text.ty != Ty::Unknown {
                    self.diags.push(
                        Diagnostic::error(
                            Code::TypeConflict,
                            value.span(),
                            format!("`text` is a `string`, and this is a {}", text.ty),
                        )
                        .note(
                            "it is the row the components tree draws; blank it to draw no row                              at all",
                        ),
                    );
                    return;
                }
                self.emit(ir::Instruction::new(
                    "SectionSetText",
                    vec![index(), text.arg],
                ));
            }
            HandleField::Size => {
                let Some(size) = self.value(value) else {
                    return;
                };
                if self.require_int(&size, value.span()).is_none() {
                    return;
                }
                self.emit(ir::Instruction::new(
                    "SectionSetSize",
                    vec![index(), size.arg],
                ));
            }
            HandleField::InstallTypes => {
                let Some(mask) = self.inst_type_mask(value) else {
                    return;
                };
                self.emit(ir::Instruction::new(
                    "SectionSetInstTypes",
                    vec![index(), ir::Arg::int(mask)],
                ));
            }
        }
    }

    /// One bit of the flags word, set to `value`.
    ///
    /// A literal is the case worth spending code on, because it is the one every
    /// script writes: `docs.selected = false` is three instructions, and the
    /// general form — a bit cleared and then or-ed back in from a register — is
    /// six. Both are what `Sections.nsh`'s `SelectSection` does by hand, minus
    /// its clobber of `$0`.
    fn write_flag(&mut self, index: &ir::Arg, bit: u32, shift: u32, value: &Expr, field: &Name) {
        let bit = i64::from(bit);
        let flags = self.claim_temp(field.span);
        let mut masked = false;
        let literal = match self.constant(value) {
            Some(ConstValue::Bool(flag)) => Some(flag),
            _ => None,
        };

        let lowered = match literal {
            Some(_) => None,
            None => {
                let Some(typed) = self.value(value) else {
                    return;
                };
                if typed.ty != Ty::Bool && typed.ty != Ty::Unknown {
                    self.diags.push(
                        Diagnostic::error(
                            Code::TypeConflict,
                            value.span(),
                            format!("`{}` is a `bool`, and this is a {}", field.text, typed.ty),
                        )
                        .note("it is one bit of the section's flags, so there is no third value"),
                    );
                    return;
                }
                // A `bool` is already `0` or `1` (§15.20), so the mask below
                // is skipped: the invariant is the reason the type exists.
                masked = typed.ty == Ty::Bool;
                Some(typed.arg)
            }
        };

        self.emit(ir::Instruction::new(
            "SectionGetFlags",
            vec![index.clone(), ir::Arg::dest(flags.clone())],
        ));

        match literal {
            Some(true) => self.emit(ir::Instruction::new(
                "IntOp",
                vec![
                    ir::Arg::dest(flags.clone()),
                    ir::Arg::slot(flags.clone()),
                    ir::Arg::raw("|"),
                    ir::Arg::int(bit),
                ],
            )),
            Some(false) => self.clear(&flags, bit, field.span),
            None => {
                self.clear(&flags, bit, field.span);
                // The value back where it came from: `0`/`1` masked, shifted up
                // to the bit's position, or-ed in.
                let carry = self.claim_temp(field.span);
                let mut source = lowered.expect("a non-literal was lowered");
                if !masked {
                    self.emit(ir::Instruction::new(
                        "IntOp",
                        vec![
                            ir::Arg::dest(carry.clone()),
                            source,
                            ir::Arg::raw("&"),
                            ir::Arg::int(1),
                        ],
                    ));
                    source = ir::Arg::slot(carry.clone());
                }
                if shift > 0 {
                    self.emit(ir::Instruction::new(
                        "IntOp",
                        vec![
                            ir::Arg::dest(carry.clone()),
                            source,
                            ir::Arg::raw("<<"),
                            ir::Arg::int(i64::from(shift)),
                        ],
                    ));
                    source = ir::Arg::slot(carry.clone());
                }
                self.emit(ir::Instruction::new(
                    "IntOp",
                    vec![
                        ir::Arg::dest(flags.clone()),
                        ir::Arg::slot(flags.clone()),
                        ir::Arg::raw("|"),
                        source,
                    ],
                ));
            }
        }

        self.emit(ir::Instruction::new(
            "SectionSetFlags",
            vec![index.clone(), ir::Arg::slot(flags)],
        ));
    }

    /// `flags &= ~bit`. NSIS's `~` is unary and takes a whole line of its own,
    /// which is why clearing a bit costs two instructions and setting one costs
    /// one.
    fn clear(&mut self, flags: &Slot, bit: i64, span: Span) {
        let mask = self.claim_temp(span);
        self.emit(ir::Instruction::new(
            "IntOp",
            vec![
                ir::Arg::dest(mask.clone()),
                ir::Arg::int(bit),
                ir::Arg::raw("~"),
            ],
        ));
        self.emit(ir::Instruction::new(
            "IntOp",
            vec![
                ir::Arg::dest(flags.clone()),
                ir::Arg::slot(flags.clone()),
                ir::Arg::raw("&"),
                ir::Arg::slot(mask),
            ],
        ));
    }

    // -- controls ---------------------------------------------------------

    /// The field, checked against what the window is.
    fn control_field(&mut self, handle: &ControlHandle, field: &Name) -> Option<ControlField> {
        let Some(what) = control::control_field(&field.text) else {
            self.diags.push(
                Diagnostic::error(
                    Code::UnknownField,
                    field.span,
                    format!("`{}` is not a field of a control", field.text),
                )
                .note(format!("the fields are {}", list(control::CONTROL_FIELDS))),
            );
            return None;
        };

        if !what.on(handle.control) {
            let diagnostic = Diagnostic::error(
                Code::UnknownField,
                field.span,
                format!("`{}` needs {}", field.text, what.needs()),
            );
            self.diags.push(match handle.control {
                Some(control) => diagnostic.note(format!(
                    "`{}` is a `{}`, and {}",
                    handle.base,
                    control.installua,
                    what.why()
                )),
                // `getDlgItem` hands back an `HWND` and nothing else. The class
                // behind it is readable from Windows and not from NSIS, and a
                // field that guessed would be a message sent to a window that
                // does not implement it.
                None => diagnostic.note(format!(
                    "`{}` is a window this program did not draw, so its kind is not known here: \
                     the fields that need one are on a control a `page.custom` lists (§15.32)",
                    handle.base
                )),
            });
            return None;
        }
        Some(what)
    }

    /// `serial.value` — a read, which is one instruction and no bit test.
    fn control_read(&mut self, handle: &ControlHandle, field: &Name, dest: &Slot) -> Option<Ty> {
        let what = self.control_field(handle, field)?;
        if !what.readable() {
            self.diags.push(
                Diagnostic::error(
                    Code::UnknownField,
                    field.span,
                    format!("`{}` can be written and not read", field.text),
                )
                .note(format!(
                    "`{}` sets it and NSIS has no instruction that asks, so a read here would \
                     have to be a value this compiler remembered rather than one the window \
                     reported (§15.32)",
                    what.setter()
                )),
            );
            return None;
        }

        match what {
            // The one field whose read is not the write's twin. `SendMessage`
            // in NSIS has no way to be handed a buffer, so `WM_GETTEXT` cannot
            // be spelled at all; `GetWindowText` through the `System` plugin is
            // what `nsDialogs.nsh` does, and `${NSIS_MAX_STRLEN}` is makensis'
            // own define, so this still includes nothing (ruling 5).
            ControlField::Value => {
                let signature = ir::Arg::str("user32::GetWindowText(p")
                    .concat(ir::Arg::slot(handle.slot.clone()))
                    .concat(ir::Arg::str(",t.s,i"))
                    .concat(ir::Arg::var("${NSIS_MAX_STRLEN}"))
                    .concat(ir::Arg::str(")"));
                self.generated_plugin_call(
                    "System::Call",
                    vec![signature],
                    vec![dest.clone()],
                    field.span,
                );
                Some(Ty::Str)
            }
            // `BM_GETCHECK` answers `BST_UNCHECKED` or `BST_CHECKED`, which are
            // 0 and 1. The third answer, `BST_INDETERMINATE`, needs `BS_3STATE`,
            // and no kind in this table carries it — so the `bool` is a fact
            // about the styles above rather than a hope (§15.20).
            _ => {
                self.emit(ir::Instruction::new(
                    "SendMessage",
                    vec![
                        ir::Arg::slot(handle.slot.clone()),
                        message(control::BM_GETCHECK),
                        ir::Arg::int(0),
                        ir::Arg::int(0),
                        ir::Arg::dest(dest.clone()),
                    ],
                ));
                Some(Ty::Bool)
            }
        }
    }

    /// `agree.checked = true` — a write, which is one instruction each.
    fn control_write(&mut self, handle: &ControlHandle, field: &Name, value: &Expr) {
        let Some(what) = self.control_field(handle, field) else {
            return;
        };
        let hwnd = || ir::Arg::slot(handle.slot.clone());

        match what {
            ControlField::Value => {
                let Some(text) = self.typed(value, Ty::Str, &field.text) else {
                    return;
                };
                self.emit(ir::Instruction::new(
                    "SendMessage",
                    vec![
                        hwnd(),
                        message(control::WM_SETTEXT),
                        ir::Arg::int(0),
                        // `STR:` is how `SendMessage` is told that an argument
                        // is a string and not a number, and it is the
                        // compiler's to write for the same reason the message
                        // number is (ruling 8).
                        ir::Arg::str("STR:").concat(text),
                    ],
                ));
            }
            ControlField::Checked => {
                let Some(state) = self.typed(value, Ty::Bool, &field.text) else {
                    return;
                };
                self.emit(ir::Instruction::new(
                    "SendMessage",
                    vec![
                        hwnd(),
                        message(control::BM_SETCHECK),
                        state,
                        ir::Arg::int(0),
                    ],
                ));
            }
            // `EnableWindow` takes the `bool` as it stands: its two states are
            // 0 and 1, which is what a `bool` in this language already is.
            ControlField::Enabled => {
                let Some(state) = self.typed(value, Ty::Bool, &field.text) else {
                    return;
                };
                self.emit(ir::Instruction::new("EnableWindow", vec![hwnd(), state]));
            }
            // `ShowWindow`'s are `SW_HIDE` and `SW_SHOW`, which are 0 and 5, so
            // a literal picks one and anything else is multiplied up. Five times
            // a value that is 0 or 1 is 0 or 5, and the `bool` invariant is what
            // makes that an identity rather than a trick (§15.20).
            ControlField::Visible => {
                let state = match self.constant(value) {
                    Some(ConstValue::Bool(true)) => Some(ir::Arg::int(control::SW_SHOW.into())),
                    Some(ConstValue::Bool(false)) => Some(ir::Arg::int(control::SW_HIDE.into())),
                    _ => None,
                };
                let state = match state {
                    Some(literal) => literal,
                    None => {
                        let Some(flag) = self.typed(value, Ty::Bool, &field.text) else {
                            return;
                        };
                        let scaled = self.claim_temp(field.span);
                        self.emit(ir::Instruction::new(
                            "IntOp",
                            vec![
                                ir::Arg::dest(scaled.clone()),
                                flag,
                                ir::Arg::raw("*"),
                                ir::Arg::int(control::SW_SHOW.into()),
                            ],
                        ));
                        ir::Arg::slot(scaled)
                    }
                };
                self.emit(ir::Instruction::new("ShowWindow", vec![hwnd(), state]));
            }
            ControlField::Colors => {
                let Some((text, back)) = self.colours(value, "colors") else {
                    return;
                };
                self.emit(ir::Instruction::new(
                    "SetCtlColors",
                    vec![hwnd(), text, back],
                ));
            }
            ControlField::Font => {
                let Some(font) = self.font(value, field.span) else {
                    return;
                };
                self.emit(ir::Instruction::new(
                    "SendMessage",
                    vec![
                        hwnd(),
                        message(control::WM_SETFONT),
                        ir::Arg::slot(font),
                        // Redraw. A font set before the dialog is shown would
                        // not need it and one set from a callback would, and
                        // the second case is the one that looks broken.
                        ir::Arg::int(1),
                    ],
                ));
            }
            ControlField::Image => {
                let Some(path) = self.typed(value, Ty::Str, &field.text) else {
                    return;
                };
                let mut args = image_args(path);
                args.insert(1, hwnd());
                self.emit(ir::Instruction::new("LoadAndSetImage", args));
            }
        }
    }

    /// A value the field's type demands, with the mismatch named after the
    /// field rather than after the instruction it lands in.
    fn typed(&mut self, value: &Expr, wanted: Ty, field: &str) -> Option<ir::Arg> {
        let typed = self.value(value)?;
        if typed.ty != wanted && typed.ty != Ty::Unknown {
            self.diags.push(Diagnostic::error(
                Code::TypeConflict,
                value.span(),
                format!("`{field}` is a `{wanted}`, and this is a {}", typed.ty),
            ));
            return None;
        }
        Some(typed.arg)
    }

    /// `font = { face = "Tahoma", size = 10, bold = true }`, as the handle
    /// `WM_SETFONT` wants.
    ///
    /// `CreateFont` makes a GDI object and nothing here frees it, which is what
    /// every NSIS script does: the font lives as long as the installer, and the
    /// installer is a process that exits.
    fn font(&mut self, value: &Expr, span: Span) -> Option<Slot> {
        let Expr::Table { fields, .. } = value else {
            self.bad_value(
                value.span(),
                "font",
                "a table",
                "`{ face = \"Tahoma\", size = 10 }` — the two Windows needs, and `bold` if it is \
                 wanted",
            );
            return None;
        };

        let mut face = None;
        let mut size = None;
        let mut bold = false;
        for field in fields {
            let TableField::Named { name, value } = field else {
                self.bad_value(
                    value.span(),
                    "font",
                    "a table of named settings",
                    "a typeface and a size are not an order anyone would remember",
                );
                continue;
            };
            match name.text.as_str() {
                "face" => face = self.typed(value, Ty::Str, "face"),
                "size" => match self.constant(value) {
                    Some(ConstValue::Int(points)) => size = Some(points),
                    _ => self.bad_value(
                        value.span(),
                        "size",
                        "a whole number of points",
                        "this is the size a user would recognise from a word processor, not a \
                         pixel height",
                    ),
                },
                "bold" => match self.constant(value) {
                    Some(ConstValue::Bool(flag)) => bold = flag,
                    _ => self.bad_value(
                        value.span(),
                        "bold",
                        "`true` or `false`",
                        "`CreateFont` takes a weight at build time, so this is not a value that \
                         can arrive while the installer runs",
                    ),
                },
                other => {
                    self.diags.push(
                        Diagnostic::error(
                            Code::UnknownField,
                            name.span,
                            format!("`{other}` is not a font setting"),
                        )
                        .note("the settings are `face`, `size` and `bold`"),
                    );
                }
            }
        }

        let (Some(face), Some(size)) = (face, size) else {
            self.diags.push(
                Diagnostic::error(
                    Code::MissingAttribute,
                    span,
                    "`font` wants both `face` and `size`",
                )
                .note(
                    "`CreateFont` names a typeface and Windows substitutes one it has if the name \
                     is not installed; there is no shape for \"the same face, bigger\"",
                ),
            );
            return None;
        };

        let handle = self.claim_temp(span);
        let weight = match bold {
            true => control::FW_BOLD,
            false => control::FW_NORMAL,
        };
        self.emit(ir::Instruction::new(
            "CreateFont",
            vec![
                ir::Arg::dest(handle.clone()),
                face,
                ir::Arg::int(size),
                ir::Arg::int(weight.into()),
            ],
        ));
        Some(handle)
    }

    /// `{ "Full", "Minimal" }` as the bit field `SectionSetInstTypes` reads.
    ///
    /// Compile-time, and it has to be: the names are the block's declaration and
    /// the positions exist in exactly one place, so a runtime list would be a
    /// second numbering to keep in step with the first (§13).
    fn inst_type_mask(&mut self, value: &Expr) -> Option<i64> {
        let Expr::Table { fields, .. } = value else {
            self.diags.push(
                Diagnostic::error(
                    Code::BadFieldValue,
                    value.span(),
                    "`installTypes` wants a list of names",
                )
                .note(
                    "write `installTypes = { \"Full\" }`, naming types the block declares; the \
                     positions are the compiler's (§13)",
                ),
            );
            return None;
        };

        let mut mask = 0i64;
        for field in fields {
            let TableField::Positional { value } = field else {
                self.todo(value.span(), "a named entry in `installTypes`");
                continue;
            };
            if let Some(position) = self.inst_type_position(value, "installTypes") {
                mask |= 1 << position;
            }
        }
        Some(mask)
    }
}
