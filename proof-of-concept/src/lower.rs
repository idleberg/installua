//! Phase 3: AST -> IR. Overlay-driven, so the ergonomic decisions live in
//! `overlay.rs`; register assignment and control-flow shape live here.

use crate::ast::{
    BinOp, CmpOp, Condition, Expr, If, Local, MacroCall, MessageBox, PluginCall, Program, Section,
    SectionItem, Stmt, Ty,
};
use crate::diag::{Diagnostic, Diagnostics, Span};
use crate::ir::{self, Arg, Item};
use crate::overlay;

/// `$0`-`$9` then `$R0`-`$R9`. Locals are handed out from the front, expression
/// temporaries from the back, so the two never collide without a liveness pass
/// — which is what a real allocator replaces this with.
const REGISTERS: [&str; 20] = [
    "$0", "$1", "$2", "$3", "$4", "$5", "$6", "$7", "$8", "$9", "$R0", "$R1", "$R2", "$R3", "$R4",
    "$R5", "$R6", "$R7", "$R8", "$R9",
];

struct Value {
    arg: Arg,
    ty: Ty,
}

pub fn lower(program: &Program, diags: &mut Diagnostics) -> ir::Module {
    let mut attributes = Vec::new();

    if let Some(installer) = &program.installer {
        // Emit in overlay order, not source order, so output is stable.
        for spec in overlay::INSTALLER_FIELDS {
            let Some(field) = installer.fields.iter().find(|f| f.name == spec.luis) else {
                continue;
            };
            let mut args = vec![Arg::str(field.value.clone())];
            // Derived argument: `Name` takes an ampersand-doubled twin, emitted
            // only when the string actually contains `&`.
            if spec.luis == "name" && field.value.contains('&') {
                args.push(Arg::str(field.value.replace('&', "&&")));
            }
            attributes.push(ir::Instruction::new(spec.nsis, args));
        }
    }

    // The preprocessor is textual and sequential, so these keep source order
    // and each `pre.*` command sits immediately above the `!define` that names
    // its output.
    let mut directives = Vec::new();
    for define in &program.defines {
        if let Some(pre) = &define.pre {
            let mut args: Vec<Arg> = pre.args.iter().map(Arg::str).collect();
            args.push(Arg::raw(pre.prefix.clone()));
            directives.push(ir::Instruction::new(pre.pre.nsis, args));
        }
        directives.push(ir::Instruction::new(
            "!define",
            vec![
                Arg::raw(define.name.clone()),
                Arg::str(define.value.clone()),
            ],
        ));
    }

    let inits = program
        .inits
        .iter()
        .map(|init| ir::Instruction::new(format!("${{{init}}}"), Vec::new()))
        .collect();

    let mut lowering = Lowering {
        diags,
        labels: 0,
        locals: Vec::new(),
        next_local: 0,
        next_temp: REGISTERS.len(),
    };

    // Registers are per-body: NSIS `Goto` cannot cross a Function/Section
    // boundary, and neither does anything else here.
    let functions = program
        .functions
        .iter()
        .map(|function| ir::Function {
            name: function.name.clone(),
            body: lowering.body(&function.body),
        })
        .collect();

    let sections = program
        .items
        .iter()
        .map(|item| match item {
            SectionItem::Section(section) => ir::SectionItem::Section(lowering.section(section)),
            SectionItem::Group(group) => ir::SectionItem::Group(ir::SectionGroup {
                name: group.name.clone(),
                expanded: group.expanded,
                sections: group.sections.iter().map(|s| lowering.section(s)).collect(),
            }),
        })
        .collect();

    ir::Module {
        includes: program.includes.iter().map(|h| h.include).collect(),
        directives,
        inits,
        attributes,
        functions,
        sections,
    }
}

struct Lowering<'a> {
    diags: &'a mut Diagnostics,
    labels: usize,
    locals: Vec<(String, &'static str, Ty)>,
    next_local: usize,
    next_temp: usize,
}

impl Lowering<'_> {
    fn section(&mut self, section: &Section) -> ir::Section {
        ir::Section {
            name: section.name.clone(),
            optional: section.optional,
            body: self.body(&section.body),
        }
    }

    fn body(&mut self, stmts: &[Stmt]) -> Vec<Item> {
        self.locals.clear();
        self.next_local = 0;
        let mut items = Vec::new();
        self.stmts(stmts, &mut items);
        items
    }

    /// One number per construct, so a branch's labels read as a set.
    fn next_label_id(&mut self) -> usize {
        self.labels += 1;
        self.labels - 1
    }

    fn alloc_local(&mut self, span: Span) -> Option<&'static str> {
        if self.next_local >= self.next_temp {
            self.out_of_registers(span);
            return None;
        }
        let register = REGISTERS[self.next_local];
        self.next_local += 1;
        Some(register)
    }

    fn alloc_temp(&mut self, span: Span) -> Option<&'static str> {
        if self.next_temp <= self.next_local {
            self.out_of_registers(span);
            return None;
        }
        self.next_temp -= 1;
        Some(REGISTERS[self.next_temp])
    }

    fn out_of_registers(&mut self, span: Span) {
        self.diags.push(
            Diagnostic::error("E007", span, "out of registers")
                .with_note("this PoC assigns registers without a liveness pass; only 20 exist"),
        );
    }

    fn lookup_local(&self, name: &str) -> Option<(&'static str, Ty)> {
        self.locals
            .iter()
            .rev()
            .find(|(local, _, _)| local == name)
            .map(|(_, register, ty)| (*register, *ty))
    }

    fn stmts(&mut self, stmts: &[Stmt], out: &mut Vec<Item>) {
        let scope = self.locals.len();
        for stmt in stmts {
            // Temporaries never outlive a statement, so the back end of the
            // register file is recycled here rather than tracked.
            self.next_temp = REGISTERS.len();
            self.stmt(stmt, out);
        }
        self.locals.truncate(scope);
    }

    fn stmt(&mut self, stmt: &Stmt, out: &mut Vec<Item>) {
        match stmt {
            Stmt::Local(local) => self.local(local, out),
            Stmt::Call(call) => {
                let Some(instruction) = overlay::lookup(&call.name) else {
                    return;
                };
                let mut args = Vec::new();
                for argument in &call.args {
                    let Some(value) = self.expr(argument, out) else {
                        return;
                    };
                    args.push(value.arg);
                }
                out.push(Item::Instruction(ir::Instruction::new(
                    instruction.nsis,
                    args,
                )));
            }
            Stmt::If(if_stmt) => self.if_stmt(if_stmt, out),
            Stmt::CallFunction { name, .. } => out.push(Item::Instruction(ir::Instruction::new(
                "Call",
                vec![Arg::raw(name.clone())],
            ))),
            Stmt::Macro(call) => {
                // No destination: a macro used as a statement has no output.
                self.macro_call(call, None, out);
            }
            Stmt::Plugin(call) => self.plugin_call(call, out),
            Stmt::MessageBox(message_box) => self.message_box(message_box, out),
        }
    }

    /// `${MacroName} <inputs…> [$dest]` — where the output goes is a property
    /// of the macro, not a convention.
    fn macro_call(
        &mut self,
        call: &MacroCall,
        dest: Option<&'static str>,
        out: &mut Vec<Item>,
    ) -> Option<()> {
        let mut args = Vec::new();
        if call.mac.output_first
            && let Some(dest) = dest
        {
            args.push(Arg::raw(dest));
        }
        for argument in &call.args {
            args.push(self.expr(argument, out)?.arg);
        }
        for trailing in call.mac.trailing {
            args.push(Arg::str(*trailing));
        }
        if !call.mac.output_first
            && let Some(dest) = dest
        {
            args.push(Arg::raw(dest));
        }
        out.push(Item::Instruction(ir::Instruction::new(
            format!("${{{}}}", call.mac.nsis),
            args,
        )));
        Some(())
    }

    fn plugin_call(&mut self, call: &PluginCall, out: &mut Vec<Item>) {
        let mut args = Vec::new();
        for argument in &call.args {
            let Some(value) = self.expr(argument, out) else {
                return;
            };
            args.push(value.arg);
        }
        out.push(Item::Instruction(ir::Instruction::new(
            format!("{}::{}", call.plugin, call.method),
            args,
        )));
    }

    /// The jump-table half of `MessageBox`: one instruction, then a block per
    /// handled button. `IDxxx label` is the same fused-branch trick as `if` —
    /// no answer is ever materialized into a register.
    fn message_box(&mut self, message_box: &MessageBox, out: &mut Vec<Item>) {
        let Some(text) = self.template(&message_box.text, out) else {
            return;
        };

        let mut flags = message_box.buttons.nsis.to_string();
        if let Some(icon) = message_box.icon {
            flags.push('|');
            flags.push_str(icon.nsis);
        }

        let mut args = vec![Arg::raw(flags), Arg::str(text)];
        if let Some(default) = message_box.default {
            args.push(Arg::raw("/SD"));
            args.push(Arg::raw(default.id));
        }

        let id = self.next_label_id();
        let end = format!("mb_end_{id}");
        for handler in &message_box.handlers {
            args.push(Arg::raw(handler.button.id));
            args.push(Arg::raw(format!("mb_{}_{id}", handler.button.label)));
        }
        out.push(Item::Instruction(ir::Instruction::new("MessageBox", args)));

        if message_box.handlers.is_empty() {
            return;
        }

        // A button with no handler falls out of the `MessageBox` line into
        // whatever follows, which would be the first handler's block. The
        // guard jump is emitted only when some button is actually unhandled.
        if message_box.handlers.len() < message_box.buttons.buttons.len() {
            out.push(Item::Instruction(ir::Instruction::new(
                "Goto",
                vec![Arg::raw(end.clone())],
            )));
        }

        for (index, handler) in message_box.handlers.iter().enumerate() {
            out.push(Item::Label(format!("mb_{}_{id}", handler.button.label)));
            self.stmts(&handler.body, out);
            // The last block falls into the end label instead of jumping to it.
            if index + 1 < message_box.handlers.len() {
                out.push(Item::Instruction(ir::Instruction::new(
                    "Goto",
                    vec![Arg::raw(end.clone())],
                )));
            }
        }

        out.push(Item::Label(end));
    }

    fn local(&mut self, local: &Local, out: &mut Vec<Item>) {
        // The register is claimed before the initializer is evaluated, so
        // arithmetic can land in it directly instead of via a temporary.
        let Some(register) = self.alloc_local(local.span) else {
            return;
        };

        let ty = match &local.value {
            // Fold first: `96 * 1024` never reaches `IntOp`.
            Expr::Binary { op, lhs, rhs, .. } if fold(&local.value).is_none() => {
                if self.binary(*op, lhs, rhs, register, out).is_none() {
                    return;
                }
                Ty::Int
            }
            // Same destination-passing trick: the macro writes the local's
            // register directly instead of a temporary plus a `StrCpy`.
            Expr::Macro(call) => {
                if self.macro_call(call, Some(register), out).is_none() {
                    return;
                }
                call.mac.output.unwrap_or(Ty::Int)
            }
            value => {
                let Some(value) = self.expr(value, out) else {
                    return;
                };
                out.push(Item::Instruction(ir::Instruction::new(
                    "StrCpy",
                    vec![Arg::raw(register), value.arg],
                )));
                value.ty
            }
        };

        self.locals.push((local.name.clone(), register, ty));
    }

    fn if_stmt(&mut self, if_stmt: &If, out: &mut Vec<Item>) {
        let id = self.next_label_id();
        let end = format!("endif_{id}");
        let otherwise = if if_stmt.else_body.is_empty() {
            end.clone()
        } else {
            format!("else_{id}")
        };

        if self.condition(&if_stmt.cond, &otherwise, out).is_none() {
            return;
        }

        self.stmts(&if_stmt.then_body, out);

        if !if_stmt.else_body.is_empty() {
            out.push(Item::Instruction(ir::Instruction::new(
                "Goto",
                vec![Arg::raw(end.clone())],
            )));
            out.push(Item::Label(otherwise));
            self.stmts(&if_stmt.else_body, out);
        }

        out.push(Item::Label(end));
    }

    /// Conditions are fused into the branch targets of a single `IntCmp`; no
    /// boolean is ever materialized into a register.
    fn condition(&mut self, cond: &Condition, otherwise: &str, out: &mut Vec<Item>) -> Option<()> {
        let lhs = self.int_operand(&cond.lhs, out)?;
        let rhs = self.int_operand(&cond.rhs, out)?;

        // `0` means "do not jump". Each arm is the target taken when the
        // comparison makes the condition false.
        let (equal, less, greater) = match cond.op {
            CmpOp::Eq => ("0", otherwise, otherwise),
            CmpOp::Ne => (otherwise, "0", "0"),
            CmpOp::Lt => (otherwise, "0", otherwise),
            CmpOp::Le => ("0", "0", otherwise),
            CmpOp::Gt => (otherwise, otherwise, "0"),
            CmpOp::Ge => ("0", otherwise, "0"),
        };

        out.push(Item::Instruction(ir::Instruction::new(
            "IntCmp",
            vec![lhs, rhs, Arg::raw(equal), Arg::raw(less), Arg::raw(greater)],
        )));
        Some(())
    }

    fn binary(
        &mut self,
        op: BinOp,
        lhs: &Expr,
        rhs: &Expr,
        dest: &'static str,
        out: &mut Vec<Item>,
    ) -> Option<()> {
        let lhs = self.int_operand(lhs, out)?;
        let rhs = self.int_operand(rhs, out)?;
        out.push(Item::Instruction(ir::Instruction::new(
            "IntOp",
            vec![Arg::raw(dest), lhs, Arg::raw(op.nsis()), rhs],
        )));
        Some(())
    }

    fn int_operand(&mut self, expr: &Expr, out: &mut Vec<Item>) -> Option<Arg> {
        let value = self.expr(expr, out)?;
        if value.ty != Ty::Int {
            self.diags.push(
                Diagnostic::error(
                    "E005",
                    expr.span(),
                    format!("expected an integer, found a {}", value.ty.name()),
                )
               .with_note("arithmetic and comparisons are integer-only in this PoC"),
            );
            return None;
        }
        Some(value.arg)
    }

    /// A string operand as NSIS text: literals inline, registers as `$0`,
    /// constants as `${NAME}`. This is what makes `..` cost zero instructions —
    /// the concatenation happens in the emitted literal.
    fn template(&mut self, expr: &Expr, out: &mut Vec<Item>) -> Option<String> {
        match expr {
            Expr::Str(value, _) => Some(value.clone()),
            Expr::Number(value, _) => Some(value.to_string()),
            Expr::Const { text, .. } => Some(text.clone()),
            Expr::Concat(parts, _) => {
                let mut text = String::new();
                for part in parts {
                    text.push_str(&self.template(part, out)?);
                }
                Some(text)
            }
            // Anything that needs computing is computed into a register first,
            // and the register's name is what lands in the template.
            other => match self.expr(other, out)? {
                Value {
                    arg: Arg::Raw(text),
                    ..
                } => Some(text),
                Value {
                    arg: Arg::Str(text),
                    ..
                } => Some(text),
            },
        }
    }

    fn expr(&mut self, expr: &Expr, out: &mut Vec<Item>) -> Option<Value> {
        if let Expr::Binary { .. } = expr
            && let Some(value) = fold(expr)
        {
            return Some(Value {
                arg: Arg::raw(value.to_string()),
                ty: Ty::Int,
            });
        }

        match expr {
            Expr::Number(value, _) => Some(Value {
                arg: Arg::raw(value.to_string()),
                ty: Ty::Int,
            }),
            Expr::Str(value, _) => Some(Value {
                arg: Arg::str(value.clone()),
                ty: Ty::Str,
            }),
            // A constant is text, not storage: an `int` one can sit in an
            // `IntOp` operand, a `string` one has to be quoted.
            Expr::Const { text, ty, .. } => Some(Value {
                arg: match ty {
                    Ty::Int => Arg::raw(text.clone()),
                    Ty::Str => Arg::str(text.clone()),
                },
                ty: *ty,
            }),
            Expr::Local(name, span) => match self.lookup_local(name) {
                Some((register, ty)) => Some(Value {
                    arg: Arg::raw(register),
                    ty,
                }),
                None => {
                    self.diags.push(Diagnostic::error(
                        "E003",
                        *span,
                        format!("unknown variable `{name}`"),
                    ));
                    None
                }
            },
            Expr::Binary { op, lhs, rhs, span } => {
                let dest = self.alloc_temp(*span)?;
                self.binary(*op, lhs, rhs, dest, out)?;
                Some(Value {
                    arg: Arg::raw(dest),
                    ty: Ty::Int,
                })
            }
            Expr::Concat(_, _) => {
                let text = self.template(expr, out)?;
                Some(Value {
                    arg: Arg::str(text),
                    ty: Ty::Str,
                })
            }
            Expr::Macro(call) => {
                let dest = self.alloc_temp(call.span)?;
                self.macro_call(call, Some(dest), out)?;
                Some(Value {
                    arg: Arg::raw(dest),
                    ty: call.mac.output.unwrap_or(Ty::Int),
                })
            }
        }
    }
}

/// Constant folding. Only literals fold: a `<const>` deliberately stays
/// `${NAME}` in the output, so the `!define` remains the thing a reader edits.
fn fold(expr: &Expr) -> Option<i64> {
    match expr {
        Expr::Number(value, _) => Some(*value),
        Expr::Binary { op, lhs, rhs, .. } => op.fold(fold(lhs)?, fold(rhs)?),
        _ => None,
    }
}
