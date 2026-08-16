//! AST → IR: declarations, then one CFG per body.
//!
//! Two halves, and the split is the phase's shape. This file walks
//! *declarations* — the blocks, their fields, and the bodies hanging off them —
//! and [`expr`] walks *expressions*, including the condition fusion that is
//! Phase 2's exit criterion.
//!
//! Nothing here invents a label or picks a register number for a temporary. A
//! statement creates blocks and points terminators at them ([`crate::cfg`]),
//! and what a label is stays entirely inside [`crate::layout`].
//!
//! Anything outside the exposed set is [`Code::NotYetImplemented`] — the honest
//! edge of the vertical slice (PLAN §0), and deliberately a different code from
//! [`Code::UnknownField`], which means *no version will ever accept this*. The
//! first is a five-second wait and the second is a five-second fix, and
//! collapsing them is how a `todo` count stops predicting anything.

mod expr;

use std::collections::BTreeMap;

use crate::ast::*;
use crate::cfg::{self, BlockId, Body, Terminator};
use crate::diag::{Code, Diagnostic, Diagnostics, Span};
use crate::ir;
use crate::regs::{Registers, Slot};
use crate::resolve::{ConstValue, Resolved};
use crate::types::Ty;

/// The frozen v1 attribute surface (Phase 0). A name in here is scheduled; a
/// name outside it is a typo.
const V1_ATTRIBUTES: &[&str] = &[
    "name",
    "outFile",
    "unicode",
    "compressor",
    "requestExecutionLevel",
    "installDir",
    "icon",
    "license",
    "pages",
    "caption",
    "text",
    "manifest",
    "versionInfo",
    "crcCheck",
    "dateSave",
];

/// The frozen v1 block surface, for the same reason.
const V1_BLOCKS: &[&str] = &[
    "attributes",
    "installer",
    "uninstaller",
    "languages",
    "func",
    "import",
    "plugin",
    "include",
];

pub fn lower(program: &Program, resolved: &Resolved<'_>, diags: &mut Diagnostics) -> ir::Module {
    let mut lowerer = Lowerer {
        diags,
        resolved,
        module: ir::Module::new(),
        globals: BTreeMap::new(),
        attributes_span: None,
        installer_span: None,
    };
    lowerer.program(program);
    lowerer.finish()
}

/// A global's agreed type, and every site that agreed on it. §15.24 checks
/// across all assignments rather than pinning to the declaring one, because
/// §15.6 makes the language order-free and "the declaring assignment" is
/// therefore arbitrary.
type GlobalTypes = BTreeMap<String, (Ty, Vec<Span>)>;

struct Lowerer<'a, 'p> {
    diags: &'a mut Diagnostics,
    resolved: &'a Resolved<'p>,
    module: ir::Module,
    globals: GlobalTypes,
    attributes_span: Option<Span>,
    installer_span: Option<Span>,
}

impl Lowerer<'_, '_> {
    fn program(&mut self, program: &Program) {
        for stmt in &program.block {
            self.top_level(stmt);
        }
        // Globals in first-seen order, emitted before the first body that
        // touches them (§12) — which the field order in `ir::Module` already
        // guarantees.
        self.module.vars = self
            .resolved
            .globals
            .iter()
            .map(|global| global.name.clone())
            .collect();
    }

    fn finish(self) -> ir::Module {
        self.module
    }

    fn top_level(&mut self, stmt: &Stmt) {
        // A top-level `<const>` is a build-time value that folds at every use,
        // and `resolve` has already dealt with it. Anything else `local` was
        // rejected there too.
        if matches!(stmt, Stmt::Local { .. }) {
            return;
        }

        let Stmt::Call(call) = stmt else {
            self.todo(stmt.span(), "this declaration");
            return;
        };
        let Some((name, span)) = call.callee_name().map(|name| (name, call.span())) else {
            self.todo(call.span(), "this call");
            return;
        };

        match name {
            "attributes" => {
                let Some(fields) = self.block_fields(call) else {
                    return;
                };
                if self.duplicate(&mut Field::Attributes, span) {
                    return;
                }
                self.attributes(fields, span);
            }
            "installer" => {
                let Some(fields) = self.block_fields(call) else {
                    return;
                };
                if self.duplicate(&mut Field::Installer, span) {
                    return;
                }
                self.installer(fields);
            }
            "func" => self.function(name, call),
            other if V1_BLOCKS.contains(&other) => {
                self.todo(span, &format!("`{other}`"));
            }
            other => {
                self.diags.push(
                    Diagnostic::error(
                        Code::UnknownField,
                        span,
                        format!("`{other}` is not a declaration"),
                    )
                    .note(format!("the declarations are {}", list(V1_BLOCKS))),
                );
            }
        }
    }

    /// Both `attributes { … }` and `attributes({ … })` are the same call in Lua,
    /// so both arrive here as one table argument.
    fn block_fields<'e>(&mut self, call: &'e Expr) -> Option<&'e [TableField]> {
        let Expr::Call { args, .. } = call else {
            return None;
        };
        match args.as_slice() {
            [Expr::Table { fields, .. }] => Some(fields.as_slice()),
            _ => {
                self.todo(call.span(), "this block in this form");
                None
            }
        }
    }

    fn duplicate(&mut self, which: &mut Field, span: Span) -> bool {
        let (slot, name) = match which {
            Field::Attributes => (&mut self.attributes_span, "attributes"),
            Field::Installer => (&mut self.installer_span, "installer"),
        };
        if let Some(previous) = *slot {
            let previous = previous.start_line;
            self.diags.push(
                Diagnostic::error(
                    Code::DuplicateBlock,
                    span,
                    format!("`{name} {{}}` appears more than once"),
                )
                .note(format!("the first one is at line {previous}"))
                .note("it is script-global, so there is exactly one (§2)"),
            );
            return true;
        }
        *slot = Some(span);
        false
    }

    // -- attributes -------------------------------------------------------

    fn attributes(&mut self, fields: &[TableField], span: Span) {
        for field in fields {
            let TableField::Named { name, value } = field else {
                self.todo(span, "a positional entry in `attributes {}`");
                continue;
            };

            match name.text.as_str() {
                "name" => {
                    if let Some(text) = self.constant_string(value, "name") {
                        self.module
                            .attributes
                            .push(ir::Instruction::new("Name", vec![ir::Arg::str(text)]));
                    }
                }
                "outFile" => {
                    if let Some(text) = self.constant_string(value, "outFile") {
                        self.module
                            .attributes
                            .push(ir::Instruction::new("OutFile", vec![ir::Arg::path(text)]));
                    }
                }
                "installDir" => {
                    if let Some(text) = self.constant_string(value, "installDir") {
                        self.module.attributes.push(ir::Instruction::new(
                            "InstallDir",
                            vec![ir::Arg::path(text)],
                        ));
                    }
                }
                "unicode" => match self.constant(value) {
                    Some(ConstValue::Bool(value)) => self.module.unicode = value,
                    _ => self.bad_value(
                        value.span(),
                        "unicode",
                        "a `bool`",
                        "write `unicode = true`; NSIS's charset otherwise depends on how the \
                         local `makensis` was built, which is why it is always emitted (§15.16)",
                    ),
                },
                other if V1_ATTRIBUTES.contains(&other) => {
                    self.todo(name.span, &format!("the `{other}` attribute"));
                }
                other => {
                    self.diags.push(
                        Diagnostic::error(
                            Code::UnknownField,
                            name.span,
                            format!("`{other}` is not an attribute"),
                        )
                        .note(format!("the attributes are {}", list(V1_ATTRIBUTES))),
                    );
                }
            }
        }
    }

    // -- bodies -----------------------------------------------------------

    fn installer(&mut self, fields: &[TableField]) {
        for field in fields {
            match field {
                TableField::Positional { value } => {
                    if let Some(section) = self.section(value) {
                        self.module.sections.push(ir::SectionItem::Section(section));
                    }
                }
                TableField::Named { name, .. } => {
                    self.todo(name.span, &format!("the `{}` field", name.text));
                }
            }
        }
    }

    fn section(&mut self, value: &Expr) -> Option<ir::Section> {
        let Expr::Call { callee, args, .. } = value else {
            self.todo(value.span(), "this entry");
            return None;
        };
        let Expr::Name(callee) = callee.as_ref() else {
            self.todo(value.span(), "this entry");
            return None;
        };
        if callee.text != "section" {
            self.todo(value.span(), &format!("`{}`", callee.text));
            return None;
        }

        // `section(name, body)`. The three-argument form carries
        // `{ optional = true }`, which is Phase 5's overlay work.
        let [name, Expr::Function { block, span, .. }] = args.as_slice() else {
            self.todo(value.span(), "this `section` form");
            return None;
        };
        let name = self.constant_string(name, "section")?;

        Some(ir::Section {
            name,
            optional: false,
            body: self.body(block, &[], *span),
        })
    }

    fn function(&mut self, keyword: &str, call: &Expr) {
        let Expr::Call { args, .. } = call else {
            return;
        };
        // `resolve` has already validated the shape and reported anything else,
        // so a mismatch here means it was reported once already.
        let [
            Expr::Str(name),
            Expr::Function {
                params,
                block,
                span,
            },
        ] = args.as_slice()
        else {
            return;
        };
        let _ = keyword;

        let body = self.body(block, params, *span);
        self.module.functions.push(ir::Function {
            name: name.value.clone(),
            body,
        });
    }

    /// One body, one CFG, one register file. Everything about a body is local
    /// to it — the label counter resets (§15.25) and NSIS `Goto` cannot cross
    /// the boundary anyway (§8).
    fn body(&mut self, block: &Block, params: &[Name], span: Span) -> Body {
        let mut lowerer = BodyLowerer {
            diags: self.diags,
            resolved: self.resolved,
            globals: &mut self.globals,
            body: Body::new(span),
            regs: Registers::new(),
            scopes: vec![Vec::new()],
            loops: Vec::new(),
            current: Body::ENTRY,
        };
        lowerer.parameters(params);
        lowerer.block(block);
        lowerer.finish()
    }

    // -- constants --------------------------------------------------------

    fn constant(&self, expr: &Expr) -> Option<ConstValue> {
        let consts = &self.resolved.consts;
        crate::resolve::fold(expr, &|name| consts.get(name).map(|c| c.value.clone()))
    }

    /// A field whose value has to be known at compile time. An attribute is a
    /// `!define`-shaped thing, not an instruction, so there is no register for a
    /// runtime value to arrive in.
    fn constant_string(&mut self, expr: &Expr, what: &str) -> Option<String> {
        match self.constant(expr) {
            Some(value) => Some(value.text()),
            None => {
                self.diags.push(
                    Diagnostic::error(
                        Code::BadFieldValue,
                        expr.span(),
                        format!("`{what}` wants a compile-time value"),
                    )
                    .note(
                        "an attribute is written into the script's header, before any \
                         instruction has run (§12)",
                    ),
                );
                None
            }
        }
    }

    fn bad_value(&mut self, span: Span, what: &str, wanted: &str, note: &str) {
        self.diags.push(
            Diagnostic::error(
                Code::BadFieldValue,
                span,
                format!("`{what}` wants {wanted}"),
            )
            .note(note),
        );
    }

    fn todo(&mut self, span: Span, what: &str) {
        todo_at(self.diags, span, what);
    }
}

enum Field {
    Attributes,
    Installer,
}

// -- the body lowerer ------------------------------------------------------

/// What a name in a body means. `Const` is not a register at all — a `<const>`
/// is build-time (§7-1), so a use folds rather than reads.
#[derive(Clone, Debug)]
enum Binding {
    Local { slot: Slot, ty: Ty },
    Const(ConstValue),
}

/// Where `break` and `continue()` go. Both are ordinary terminators, which is
/// the whole reason no statement lowerer does label bookkeeping (§8).
struct LoopTargets {
    break_to: BlockId,
    continue_to: BlockId,
}

struct BodyLowerer<'a, 'p> {
    diags: &'a mut Diagnostics,
    resolved: &'a Resolved<'p>,
    globals: &'a mut GlobalTypes,
    body: Body,
    regs: Registers,
    scopes: Vec<Vec<(String, Binding)>>,
    loops: Vec<LoopTargets>,
    current: BlockId,
}

impl BodyLowerer<'_, '_> {
    fn finish(mut self) -> Body {
        // Whatever block execution ends in returns; the layout pass drops the
        // instruction when it is the last line anyway.
        if matches!(
            self.body.block(self.current).terminator,
            Terminator::Unreachable
        ) {
            self.body.terminate(self.current, Terminator::Return);
        }
        self.body.temps = self.regs.high_water();
        self.body
    }

    fn parameters(&mut self, params: &[Name]) {
        for param in params {
            // A parameter arrives in a register like any other local. The stack
            // ABI that puts it there is Phase 3's (§15.11).
            let Some(slot) = self.claim_local(param.span) else {
                continue;
            };
            self.bind(&param.text, Binding::Local { slot, ty: Ty::Str });
        }
    }

    // -- scopes -----------------------------------------------------------

    fn bind(&mut self, name: &str, binding: Binding) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.push((name.to_string(), binding));
        }
    }

    fn lookup(&self, name: &str) -> Option<&Binding> {
        self.scopes
            .iter()
            .rev()
            .flat_map(|scope| scope.iter().rev())
            .find(|(bound, _)| bound == name)
            .map(|(_, binding)| binding)
    }

    fn claim_local(&mut self, span: Span) -> Option<Slot> {
        match self.regs.local() {
            Some(slot) => Some(slot),
            None => {
                self.diags.push(
                    Diagnostic::error(
                        Code::RegisterExhaustion,
                        span,
                        "this body needs more registers than the allocator can hand out",
                    )
                    .note(
                        "twenty is not the real limit — the allocator is: it hands locals out \
                         from `$0` upward and never reclaims one (§12)",
                    )
                    .note("split the body into `func`s, or reuse a local"),
                );
                None
            }
        }
    }

    fn claim_temp(&mut self, span: Span) -> Option<Slot> {
        match self.regs.temp() {
            Some(slot) => Some(slot),
            None => {
                self.diags.push(
                    Diagnostic::error(
                        Code::RegisterExhaustion,
                        span,
                        "this expression needs more temporaries than the allocator can hand out",
                    )
                    .note(
                        "temporaries come from `$R9` downward and locals from `$0` upward, so \
                         the two ends met (§12)",
                    ),
                );
                None
            }
        }
    }

    // -- blocks and statements --------------------------------------------

    fn block(&mut self, block: &Block) {
        self.scopes.push(Vec::new());
        for stmt in block {
            self.stmt(stmt);
            self.regs.end_statement();
        }
        self.scopes.pop();
    }

    fn emit(&mut self, instruction: ir::Instruction) {
        let current = self.current;
        self.body.push(current, instruction);
    }

    /// Ends the current block with `terminator` and continues in a fresh one.
    /// The fresh block is often unreachable — code after a `break` — and the
    /// layout pass drops it without anybody here having to notice.
    fn terminate(&mut self, terminator: Terminator, next: BlockId) {
        let current = self.current;
        self.body.terminate(current, terminator);
        self.current = next;
    }

    fn stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Local {
                names,
                is_const,
                values,
                span,
            } => self.local(names, *is_const, values, *span),

            Stmt::Assign {
                targets,
                values,
                span,
            } => self.assign(targets, values, *span),

            Stmt::Call(call) => self.call_statement(call),

            Stmt::If {
                cond,
                then_block,
                else_block,
                span,
            } => self.if_stmt(cond, then_block, else_block.as_ref(), *span),

            Stmt::While { cond, block, span } => self.while_stmt(cond, block, *span),

            Stmt::NumericFor {
                name,
                start,
                end,
                step,
                block,
                span,
            } => self.numeric_for(name, start, end, step.as_ref(), block, *span),

            Stmt::Do { block, .. } => self.block(block),

            Stmt::Break { span } => match self.loops.last() {
                Some(targets) => {
                    let target = targets.break_to;
                    let next = self.fresh("after_break");
                    self.terminate(Terminator::Jump(target), next);
                }
                None => self.diags.push(
                    Diagnostic::error(Code::BreakOutsideLoop, *span, "`break` is not in a loop")
                        .note("it leaves the innermost `while` or `for`, and there is none here"),
                ),
            },

            Stmt::Return { values, span } => {
                if !values.is_empty() {
                    self.todo(*span, "returning a value");
                    return;
                }
                let next = self.fresh("after_return");
                self.terminate(Terminator::Return, next);
            }

            Stmt::GenericFor { span, .. } => self.todo(*span, "`for … in`"),
        }
    }

    fn local(&mut self, names: &[Name], is_const: bool, values: &[Expr], span: Span) {
        if names.len() != values.len() {
            // `local a, b = f()` needs the multiple-return ABI, which is
            // Phase 3's (§15.11).
            self.todo(
                span,
                "a `local` with a different number of names and values",
            );
            return;
        }

        for (name, value) in names.iter().zip(values) {
            if is_const {
                match self.constant(value) {
                    Some(folded) => self.bind(&name.text, Binding::Const(folded)),
                    None => self.diags.push(
                        Diagnostic::error(
                            Code::BadFieldValue,
                            value.span(),
                            format!("`{}` is not a build-time constant", name.text),
                        )
                        .note(
                            "a `<const>` folds at compile time and never reaches a register \
                             (§7-1)",
                        ),
                    ),
                }
                continue;
            }

            // The register is claimed *before* the initialiser is walked, so
            // `local sum = 1 + 1` is one `IntOp` into the local rather than an
            // `IntOp` into a temporary and a `StrCpy` after it (§12).
            let Some(slot) = self.claim_local(name.span) else {
                continue;
            };
            let Some(ty) = self.value_into(value, &slot) else {
                continue;
            };
            self.bind(&name.text, Binding::Local { slot, ty });
        }
    }

    fn assign(&mut self, targets: &[Expr], values: &[Expr], span: Span) {
        if targets.len() != values.len() {
            self.todo(
                span,
                "an assignment with a different number of targets and values",
            );
            return;
        }

        for (target, value) in targets.iter().zip(values) {
            let Expr::Name(name) = target else {
                self.todo(target.span(), "this assignment target");
                continue;
            };

            let (slot, declared) = match self.lookup(&name.text).cloned() {
                Some(Binding::Local { slot, ty }) => (slot, Some(ty)),
                Some(Binding::Const(_)) => {
                    self.diags.push(
                        Diagnostic::error(
                            Code::TypeConflict,
                            name.span,
                            format!("`{}` is `<const>`", name.text),
                        )
                        .note("a build-time constant has no register to assign to (§7-1)"),
                    );
                    continue;
                }
                None if self.resolved.global(&name.text) => (
                    Slot::Global(name.text.clone()),
                    self.globals.get(&name.text).map(|(ty, _)| *ty),
                ),
                None => {
                    self.undefined(name);
                    continue;
                }
            };

            let Some(ty) = self.value_into(value, &slot) else {
                continue;
            };

            match (slot, declared) {
                (Slot::Global(name), Some(previous)) if previous != ty => {
                    let sites = self
                        .globals
                        .get(&name)
                        .map(|(_, spans)| spans.clone())
                        .unwrap_or_default();
                    let mut diagnostic = Diagnostic::error(
                        Code::TypeConflict,
                        target.span(),
                        format!("`{name}` is assigned a {ty} here and a {previous} elsewhere"),
                    )
                    .note(
                        "a `Var` is one slot, so a global has one type for its lifetime (§15.24)",
                    );
                    for site in sites {
                        diagnostic =
                            diagnostic.note(format!("assigned at line {}", site.start_line));
                    }
                    self.diags.push(diagnostic);
                }
                (Slot::Global(name), _) => {
                    let entry = self.globals.entry(name).or_insert((ty, Vec::new()));
                    entry.0 = ty;
                    entry.1.push(target.span());
                }
                (Slot::Reg(_), Some(previous)) if previous != ty => {
                    self.diags.push(
                        Diagnostic::error(
                            Code::TypeConflict,
                            target.span(),
                            format!("`{}` holds a {previous} and is assigned a {ty}", name.text),
                        )
                        .note("a register is one slot; declare a second `local` instead"),
                    );
                }
                _ => {}
            }
        }
    }

    /// A call used for its effect. `continue()` is one of these syntactically
    /// and a terminator semantically — a slight wart, and better than the
    /// alternatives, since Lua has no `continue` and `goto` is what 5.4 added to
    /// spell the idiom (§8).
    fn call_statement(&mut self, call: &Expr) {
        if call.callee_name() == Some("continue")
            && let Expr::Call { args, span, .. } = call
        {
            if !args.is_empty() {
                self.diags.push(
                    Diagnostic::error(Code::WrongArity, *span, "`continue()` takes no arguments")
                        .note("it is a jump wearing a call's syntax (§8)"),
                );
                return;
            }
            match self.loops.last() {
                Some(targets) => {
                    let target = targets.continue_to;
                    let next = self.fresh("after_continue");
                    self.terminate(Terminator::Jump(target), next);
                }
                None => self.diags.push(
                    Diagnostic::error(
                        Code::ContinueOutsideLoop,
                        *span,
                        "`continue()` is not in a loop",
                    )
                    .note("it skips to the next iteration of the innermost `while` or `for`"),
                ),
            }
            return;
        }

        self.call(call, None);
    }

    fn if_stmt(&mut self, cond: &Expr, then_block: &Block, else_block: Option<&Block>, span: Span) {
        // A `<const>` condition folds away entirely, which is what makes
        // `!if`/`!ifdef` need no surface spelling at all (§7-2, §15.6).
        if let Some(ConstValue::Bool(taken)) = self.constant(cond) {
            match (taken, else_block) {
                (true, _) => self.block(then_block),
                (false, Some(block)) => self.block(block),
                (false, None) => {}
            }
            return;
        }

        let n = self.body.construct();
        let then_id = self.fresh(format!("then_{n}"));
        let else_id = else_block.map(|_| self.fresh(format!("else_{n}")));
        let end = self.fresh(format!("endif_{n}"));

        self.branch(cond, then_id, else_id.unwrap_or(end), span);

        self.current = then_id;
        self.block(then_block);
        self.terminate(Terminator::Jump(end), end);

        if let (Some(else_id), Some(else_block)) = (else_id, else_block) {
            self.current = else_id;
            self.block(else_block);
            self.terminate(Terminator::Jump(end), end);
        }

        self.current = end;
    }

    fn while_stmt(&mut self, cond: &Expr, block: &Block, span: Span) {
        let n = self.body.construct();
        let top = self.fresh(format!("while_{n}_top"));
        let inside = self.fresh(format!("while_{n}_body"));
        let end = self.fresh(format!("while_{n}_end"));

        self.terminate(Terminator::Jump(top), top);
        self.branch(cond, inside, end, span);

        self.current = inside;
        self.loops.push(LoopTargets {
            break_to: end,
            continue_to: top,
        });
        self.block(block);
        self.loops.pop();
        self.terminate(Terminator::Jump(top), end);

        self.current = end;
    }

    /// `for i = a, b, step`. The step has to be a build-time constant, because
    /// its *sign* picks the comparison and a runtime step would need both
    /// comparisons and a branch to choose between them.
    fn numeric_for(
        &mut self,
        name: &Name,
        start: &Expr,
        end_expr: &Expr,
        step: Option<&Expr>,
        block: &Block,
        span: Span,
    ) {
        let step_value = match step {
            None => 1,
            Some(expr) => match self.constant(expr) {
                Some(ConstValue::Int(value)) if value != 0 => value,
                Some(ConstValue::Int(_)) => {
                    self.diags.push(
                        Diagnostic::error(Code::BadFieldValue, expr.span(), "a `for` step is zero")
                            .note("the loop would never finish"),
                    );
                    return;
                }
                _ => {
                    self.todo(
                        expr.span(),
                        "a `for` step that is not a build-time constant",
                    );
                    return;
                }
            },
        };

        let Some(slot) = self.claim_local(name.span) else {
            return;
        };
        let Some(start_ty) = self.value_into(start, &slot) else {
            return;
        };
        if !start_ty.is_int() {
            self.diags.push(
                Diagnostic::error(
                    Code::TypeMismatch,
                    start.span(),
                    format!("a `for` bound is a {start_ty}, not an int"),
                )
                .note("`for i = 1, 10` counts, and there is nothing else to count over"),
            );
            return;
        }

        // Lua evaluates the bound **once**, before the first iteration, so a
        // temporary will not do: it does not survive the loop. A constant is
        // used where it stands and anything else spills to a local, which the
        // placeholder allocator never reclaims and Phase 3's liveness will.
        let (limit, limit_ty) = match self.constant(end_expr) {
            Some(ConstValue::Int(value)) => (ir::Arg::int(value), ConstValue::Int(value).ty()),
            _ => {
                let Some(bound) = self.claim_local(end_expr.span()) else {
                    return;
                };
                let Some(ty) = self.value_into(end_expr, &bound) else {
                    return;
                };
                (ir::Arg::var(bound.nsis()), ty)
            }
        };
        if !limit_ty.is_int() {
            self.diags.push(
                Diagnostic::error(
                    Code::TypeMismatch,
                    end_expr.span(),
                    format!("a `for` bound is a {limit_ty}, not an int"),
                )
                .note("`for i = 1, 10` counts, and there is nothing else to count over"),
            );
            return;
        }

        let n = self.body.construct();
        let top = self.fresh(format!("for_{n}_top"));
        let inside = self.fresh(format!("for_{n}_body"));
        let step_block = self.fresh(format!("for_{n}_step"));
        let end = self.fresh(format!("for_{n}_end"));

        self.terminate(Terminator::Jump(top), top);

        self.scopes.push(vec![(
            name.text.clone(),
            Binding::Local {
                slot: slot.clone(),
                ty: start_ty,
            },
        )]);

        let counter = ir::Arg::var(slot.nsis());
        let op = if step_value > 0 {
            cfg::CmpOp::Le
        } else {
            cfg::CmpOp::Ge
        };
        let family = cfg::IntFamily::of(start_ty.join(limit_ty));
        self.terminate(
            Terminator::Branch {
                test: cfg::Test::Int {
                    op,
                    lhs: counter.clone(),
                    rhs: limit,
                    family,
                },
                then_block: inside,
                else_block: end,
            },
            inside,
        );

        self.loops.push(LoopTargets {
            break_to: end,
            continue_to: step_block,
        });
        self.block(block);
        self.loops.pop();
        self.terminate(Terminator::Jump(step_block), step_block);

        self.emit(ir::Instruction::new(
            "IntOp",
            vec![
                ir::Arg::raw(slot.nsis()),
                counter,
                ir::Arg::raw("+"),
                ir::Arg::int(step_value),
            ],
        ));
        self.terminate(Terminator::Jump(top), end);

        self.scopes.pop();
        self.current = end;
        let _ = span;
    }

    fn fresh(&mut self, hint: impl std::fmt::Display) -> BlockId {
        let span = self.body.block(self.current).span;
        self.body
            .new_block(format!("{}{hint}", cfg::LABEL_PREFIX), span)
    }

    pub(super) fn constant(&self, expr: &Expr) -> Option<ConstValue> {
        let scopes = &self.scopes;
        let resolved = self.resolved;
        crate::resolve::fold(expr, &|name| {
            let local = scopes
                .iter()
                .rev()
                .flat_map(|scope| scope.iter().rev())
                .find(|(bound, _)| bound == name);
            match local {
                Some((_, Binding::Const(value))) => Some(value.clone()),
                // A `local` shadows a top-level `<const>` without folding to it.
                Some((_, Binding::Local { .. })) => None,
                None => resolved.consts.get(name).map(|c| c.value.clone()),
            }
        })
    }

    fn undefined(&mut self, name: &Name) {
        let mut diagnostic = Diagnostic::error(
            Code::UndefinedName,
            name.span,
            format!("`{}` is not defined", name.text),
        );
        diagnostic = match crate::builtins::nearest(&name.text) {
            // camelCase in, NSIS casing out — so `detailprint` is a spelling
            // mistake with an obvious fix rather than an unknown name (§6).
            Some(suggestion) => diagnostic.note(format!("did you mean `{suggestion}`?")),
            None => diagnostic.note(
                "resolution is order-free, so this means nowhere in the file — not merely \
                 not yet (§15.6)",
            ),
        };
        self.diags.push(diagnostic);
    }

    fn todo(&mut self, span: Span, what: &str) {
        todo_at(self.diags, span, what);
    }
}

fn todo_at(diags: &mut Diagnostics, span: Span, what: &str) {
    diags.push(
        Diagnostic::error(
            Code::NotYetImplemented,
            span,
            format!("{what} is not in this version's exposed set"),
        )
        .note("it is scheduled rather than missing: `installua coverage` counts it (PLAN §0)"),
    );
}

/// Required attributes, checked once at the end rather than at the block, so
/// the message can name what is missing rather than what is present.
pub fn check_required(module: &ir::Module, diags: &mut Diagnostics) {
    let has_out_file = module
        .attributes
        .iter()
        .any(|instruction| instruction.name == "OutFile");
    if !has_out_file {
        diags.push(
            Diagnostic::error(
                Code::MissingAttribute,
                Span::default(),
                "`outFile` is required",
            )
            .note("`makensis` has no default for it, and fails without one"),
        );
    }
}

fn list(names: &[&str]) -> String {
    names
        .iter()
        .map(|name| format!("`{name}`"))
        .collect::<Vec<_>>()
        .join(", ")
}
