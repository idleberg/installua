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
mod sig;

use std::collections::{BTreeMap, BTreeSet};

use crate::alloc;
use crate::ast::*;
use crate::callgraph;
use crate::cfg::{self, BlockId, Body, Terminator};
use crate::diag::{Code, Diagnostic, Diagnostics, Span};
use crate::ir;
use crate::regs::Slot;
use crate::resolve::{ConstValue, Resolved};
use crate::types::Ty;

pub use sig::{Inferred, Signature};

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

/// How many rounds the signature fixpoint gets. Three is enough for the deepest
/// chain in the five programs — a caller learns a parameter, the callee learns
/// its return, the caller reads it — and the cap exists because a lattice with
/// `Unknown` at the top is not strictly monotone: a disagreement can flip a slot
/// back. A program that has not settled by then compiles against the last round,
/// which is sound: an unsettled type is `Unknown`, and `Unknown` is refused at
/// every point where guessing would matter (§15.14).
const MAX_ROUNDS: usize = 8;

pub fn lower(program: &Program, resolved: &Resolved<'_>, diags: &mut Diagnostics) -> ir::Module {
    // 1. Types, to a fixpoint. Rounds before the last are lowered against a
    //    scratch collector: their diagnostics are about a type table that was
    //    still incomplete, so reporting them would be reporting the compiler's
    //    intermediate state to the user (§9-4).
    let mut inferred = Inferred::seed(resolved);
    for _ in 0..MAX_ROUNDS {
        let mut scratch = Diagnostics::new();
        let round = lower_once(program, resolved, &mut scratch, &inferred).1;
        if round == inferred {
            break;
        }
        inferred = round;
    }

    let (mut module, _) = lower_once(program, resolved, diags, &inferred);

    // 2. Registers. Every body is allocated before any call site is filled in,
    //    because a clobber set is a fact about *physical* registers and there
    //    are none until colouring has run (§9-3).
    let mut across = Vec::new();
    let mut direct: BTreeMap<String, BTreeSet<u8>> = BTreeMap::new();
    let functions = module.functions.len();
    for (index, (name, body)) in module.bodies_mut().into_iter().enumerate() {
        let allocation = alloc::allocate(body, diags);
        across.push(allocation.live_across);
        // `bodies_mut` yields the functions first, and only a function can be
        // called: a section is a root.
        if index < functions {
            direct.insert(name, allocation.clobbers);
        }
    }

    // 3. The call graph, built once and read twice here (§15.11).
    let graph = callgraph::build(&module);
    let clobbers = graph.clobbers(&direct);
    graph.lint_recursion(diags);

    // 4. `live ∩ clobbered`, at last.
    for (index, (_, body)) in module.bodies_mut().into_iter().enumerate() {
        alloc::insert_saves(body, &across[index], &clobbers);
    }

    module
}

fn lower_once(
    program: &Program,
    resolved: &Resolved<'_>,
    diags: &mut Diagnostics,
    known: &Inferred,
) -> (ir::Module, Inferred) {
    let mut lowerer = Lowerer {
        diags,
        resolved,
        module: ir::Module::new(),
        globals: known
            .globals
            .iter()
            .map(|(name, ty)| (name.clone(), (*ty, Vec::new())))
            .collect(),
        known,
        learned: Inferred::seed(resolved),
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
    /// The previous round's type table: read, never written.
    known: &'a Inferred,
    /// This round's: written, never read.
    learned: Inferred,
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

    fn finish(mut self) -> (ir::Module, Inferred) {
        self.learned.globals = self
            .globals
            .iter()
            .map(|(name, (ty, _))| (name.clone(), *ty))
            .collect();
        (self.module, self.learned)
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
            body: self.body(block, &[], *span, None),
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

        let body = self.body(block, params, *span, Some(&name.value));
        self.module.functions.push(ir::Function {
            name: name.value.clone(),
            body,
        });
    }

    /// One body, one CFG, one register file. Everything about a body is local
    /// to it — the label counter resets (§15.25) and NSIS `Goto` cannot cross
    /// the boundary anyway (§8).
    fn body(&mut self, block: &Block, params: &[Name], span: Span, owner: Option<&str>) -> Body {
        let signature = owner
            .and_then(|name| self.known.signature(name))
            .cloned()
            .unwrap_or_default();

        let mut lowerer = BodyLowerer {
            diags: self.diags,
            resolved: self.resolved,
            known: self.known,
            learned: &mut self.learned,
            globals: &mut self.globals,
            body: Body::new(span),
            scopes: vec![Vec::new()],
            loops: Vec::new(),
            returns: Vec::new(),
            current: Body::ENTRY,
        };
        lowerer.parameters(params, &signature);
        lowerer.block(block);
        let (body, returns) = lowerer.finish();
        self.returns(owner, &returns);
        body
    }

    /// Every `return` in one body has to agree on how many values it leaves,
    /// because `Call` has no arity: the callee pushes and the caller pops, and a
    /// disagreement is a stack that unbalances at runtime with NSIS reporting
    /// nothing at all (§3).
    fn returns(&mut self, owner: Option<&str>, returns: &[(Vec<Ty>, Span)]) {
        let Some((first, first_span)) = returns.first() else {
            return;
        };
        for (types, span) in returns {
            if types.len() != first.len() {
                self.diags.push(
                    Diagnostic::error(
                        Code::ReturnArity,
                        *span,
                        format!(
                            "this returns {} value(s), and another path returns {}",
                            types.len(),
                            first.len()
                        ),
                    )
                    .note(format!(
                        "the other one is at line {}",
                        first_span.start_line
                    ))
                    .note(
                        "`Call` has no arity — the callee pushes and the caller pops — so the \
                         two would unbalance the stack with no diagnostic from NSIS (§3)",
                    )
                    .note(
                        "a path that falls off the end of the body returns nothing, which counts",
                    ),
                );
                return;
            }
        }

        match owner {
            Some(name) => {
                for (types, _) in returns {
                    self.learned.learn_return(name, types);
                }
            }
            None if !first.is_empty() => {
                self.diags.push(
                    Diagnostic::error(
                        Code::ReturnArity,
                        *first_span,
                        "a `section` cannot return a value",
                    )
                    .note("nothing calls a section, so there is nobody to pop what it pushed")
                    .note("a bare `return` to leave early is fine"),
                );
            }
            None => {}
        }
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
    known: &'a Inferred,
    learned: &'a mut Inferred,
    globals: &'a mut GlobalTypes,
    body: Body,
    scopes: Vec<Vec<(String, Binding)>>,
    loops: Vec<LoopTargets>,
    /// What each `return` in this body leaves on the stack.
    returns: Vec<(Vec<Ty>, Span)>,
    current: BlockId,
}

impl BodyLowerer<'_, '_> {
    fn finish(mut self) -> (Body, Vec<(Vec<Ty>, Span)>) {
        // Whatever block execution ends in returns; the layout pass drops the
        // instruction when it is the last line anyway.
        if matches!(
            self.body.block(self.current).terminator,
            Terminator::Unreachable
        ) {
            // Falling off the end returns nothing, and that counts against the
            // arity check — but only when it can happen. The block after an
            // unconditional `return` is not a second return path, and reporting
            // it as one would reject the ordinary shape where every path
            // returns explicitly.
            if self.body.reachable(self.current) {
                let span = self.body.block(self.current).span;
                self.returns.push((Vec::new(), span));
            }
            self.body.terminate(self.current, Terminator::Return);
        }
        (self.body, self.returns)
    }

    /// The callee's half of the calling convention: arguments come off the
    /// stack in source order, because the caller pushed them in reverse.
    fn parameters(&mut self, params: &[Name], signature: &Signature) {
        for (index, param) in params.iter().enumerate() {
            let slot = self.body.vreg(param.span);
            self.emit(ir::Instruction::new(
                "Pop",
                vec![ir::Arg::dest(slot.clone())],
            ));
            self.bind(
                &param.text,
                Binding::Local {
                    slot,
                    ty: signature.param(index),
                },
            );
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

    /// A slot for a named value. There is nothing to fail here any more:
    /// twenty registers is a fact about how many values are live at once, and
    /// [`crate::alloc`] is the only pass that can know that (§9-3).
    fn claim_local(&mut self, span: Span) -> Slot {
        self.body.vreg(span)
    }

    /// A slot for an intermediate value. Counted, because Phase 2's exit
    /// criterion is a claim about this number: a fused condition materialises
    /// nothing, so it never moves.
    fn claim_temp(&mut self, span: Span) -> Slot {
        self.body.temps += 1;
        self.body.vreg(span)
    }

    // -- blocks and statements --------------------------------------------

    fn block(&mut self, block: &Block) {
        self.scopes.push(Vec::new());
        for stmt in block {
            self.stmt(stmt);
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

            Stmt::Return { values, span } => self.return_stmt(values, *span),

            Stmt::GenericFor { span, .. } => self.todo(*span, "`for … in`"),
        }
    }

    /// `return`, which is the only place a value leaves a body.
    ///
    /// Values are pushed in **reverse source order**, so the caller's first
    /// `Pop` is the first return value — the mirror of the parameter rule, and
    /// the reason neither side needs an `Exch` (§11, program 4).
    fn return_stmt(&mut self, values: &[Expr], span: Span) {
        let mut lowered = Vec::with_capacity(values.len());
        for value in values {
            let Some(typed) = self.value(value) else {
                return;
            };
            lowered.push(typed);
        }

        self.returns
            .push((lowered.iter().map(|typed| typed.ty).collect(), span));

        for typed in lowered.into_iter().rev() {
            self.emit(ir::Instruction::new("Push", vec![typed.arg]));
        }

        let next = self.fresh("after_return");
        self.terminate(Terminator::Return, next);
    }

    fn local(&mut self, names: &[Name], is_const: bool, values: &[Expr], span: Span) {
        // `local a, b = f()`: one call, several names. Plural outputs are
        // invisible at the call site (§11), so the *declaration's* arity is what
        // decides, and it comes from the signature table rather than from here.
        if names.len() > 1
            && values.len() == 1
            && !is_const
            && matches!(&values[0], Expr::Call { .. })
        {
            let slots: Vec<Slot> = names
                .iter()
                .map(|name| self.claim_local(name.span))
                .collect();
            let Some(types) = self.call_multi(&values[0], &slots) else {
                return;
            };
            for ((name, slot), ty) in names.iter().zip(slots).zip(types) {
                self.bind(&name.text, Binding::Local { slot, ty });
            }
            return;
        }

        if names.len() != values.len() {
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

            // The slot is claimed *before* the initialiser is walked, so
            // `local sum = 1 + 1` is one `IntOp` into the local rather than an
            // `IntOp` into a temporary and a `StrCpy` after it (§12).
            let slot = self.claim_local(name.span);
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
                (_, Some(previous)) if previous != ty => {
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

        let slot = self.claim_local(name.span);
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
                let bound = self.claim_local(end_expr.span());
                let Some(ty) = self.value_into(end_expr, &bound) else {
                    return;
                };
                (ir::Arg::slot(bound), ty)
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

        let counter = ir::Arg::slot(slot.clone());
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
                ir::Arg::dest(slot),
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
