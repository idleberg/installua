//! File scope: a top-level `local` belongs to the file that declares it.
//!
//! `include` splices every file into one tree, and before this pass that made
//! each file's `local`s visible in every other — which is not Lua, where a
//! chunk's `local` is its own, and not what `lua-language-server` sees either.
//! So this pass puts Lua's rule back on top of the splice: a name declared at
//! the top of one file and read in another is an error, and the way to share it
//! is Lua's way, `return { name = name }` in the one and
//! `local m = include "…"` plus `m.name` in the other.
//!
//! The splice stays, and so does its one namespace. `m.name` is rewritten here
//! to the bare name it exports, so `resolve` and `lower` go on seeing the tree
//! they always saw and neither learns what a file is. The price is that two
//! files still cannot each declare a top-level `local` of the same name — that
//! is `duplicate-block`, a program Lua would accept and this refuses. Refusing
//! a valid program with a diagnostic is the lesser fault; accepting one Lua
//! would reject was the bug. Lifting it means keying resolution by file, and
//! a `!define` name per file.
//!
//! Only top-level `local`s are file-scoped. A `func` is named by a string and a
//! bare assignment is a global, so both are program-wide in Lua as well.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::ast::{Block, Expr, Name, Stmt, TableField};
use crate::diag::{Code, Diagnostic, Diagnostics, Span};
use crate::frontend::include::Modules;

pub(crate) fn check(block: &mut Block, modules: &Modules, diags: &mut Diagnostics) {
    let mut top = HashMap::new();
    declared(block, &mut top);

    let aliases: HashMap<(u32, &str), u32> = modules
        .bindings
        .iter()
        .map(|binding| {
            (
                (binding.name.span.file, binding.name.text.as_str()),
                binding.target,
            )
        })
        .collect();

    let mut exports: BTreeMap<u32, HashMap<&str, &Name>> = BTreeMap::new();
    for (file, entries) in &modules.exports {
        let table = exports.entry(*file).or_default();
        for (key, value) in entries {
            if !declares(&top, &value.text, *file) {
                diags.push(
                    Diagnostic::error(
                        Code::NotInScope,
                        value.span,
                        format!("`{}` is not a top-level `local` of this file", value.text),
                    )
                    .note("a file exports the names it declares, not ones it was handed"),
                );
                continue;
            }
            table.insert(key.text.as_str(), value);
        }
    }

    let mut walk = Walk {
        top: &top,
        aliases: &aliases,
        exports: &exports,
        scopes: Vec::new(),
        diags,
    };
    for stmt in block {
        walk.stmt(stmt);
    }
}

/// Every top-level `local`, with the files that declare it. The branches of a
/// top-level `if` count: `resolve` treats their contents as top level, and
/// that is its own item, not this one.
fn declared(block: &Block, top: &mut HashMap<String, Vec<Span>>) {
    for stmt in block {
        match stmt {
            Stmt::Local { names, .. } => {
                for name in names {
                    top.entry(name.text.clone()).or_default().push(name.span);
                }
            }
            Stmt::If {
                then_block,
                else_block,
                ..
            } => {
                declared(then_block, top);
                if let Some(block) = else_block {
                    declared(block, top);
                }
            }
            _ => {}
        }
    }
}

fn declares(top: &HashMap<String, Vec<Span>>, name: &str, file: u32) -> bool {
    top.get(name)
        .is_some_and(|spans| spans.iter().any(|span| span.file == file))
}

struct Walk<'a> {
    top: &'a HashMap<String, Vec<Span>>,
    aliases: &'a HashMap<(u32, &'a str), u32>,
    exports: &'a BTreeMap<u32, HashMap<&'a str, &'a Name>>,
    /// Body scopes only. The top level is order-free, so it is `top` and not a
    /// scope that fills as the walk goes.
    scopes: Vec<HashSet<String>>,
    diags: &'a mut Diagnostics,
}

impl Walk<'_> {
    fn block(&mut self, block: &mut Block, bound: HashSet<String>) {
        self.scopes.push(bound);
        for stmt in block {
            self.stmt(stmt);
        }
        self.scopes.pop();
    }

    fn stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::Local { names, values, .. } => {
                for value in values {
                    self.expr(value);
                }
                if let Some(scope) = self.scopes.last_mut() {
                    scope.extend(names.iter().map(|name| name.text.clone()));
                }
            }
            Stmt::Assign {
                targets, values, ..
            } => {
                for expr in targets.iter_mut().chain(values) {
                    self.expr(expr);
                }
            }
            Stmt::Call(expr) => self.expr(expr),
            Stmt::If {
                cond,
                then_block,
                else_block,
                ..
            } => {
                self.expr(cond);
                self.block(then_block, HashSet::new());
                if let Some(block) = else_block {
                    self.block(block, HashSet::new());
                }
            }
            Stmt::While { cond, block, .. } => {
                self.expr(cond);
                self.block(block, HashSet::new());
            }
            Stmt::NumericFor {
                name,
                start,
                end,
                step,
                block,
                ..
            } => {
                self.expr(start);
                self.expr(end);
                if let Some(step) = step {
                    self.expr(step);
                }
                self.block(block, HashSet::from([name.text.clone()]));
            }
            Stmt::GenericFor {
                names,
                iterator,
                block,
                ..
            } => {
                self.expr(iterator);
                self.block(block, names.iter().map(|n| n.text.clone()).collect());
            }
            Stmt::Do { block, .. } => self.block(block, HashSet::new()),
            Stmt::Return { values, .. } => {
                for value in values {
                    self.expr(value);
                }
            }
            Stmt::Break { .. } => {}
        }
    }

    fn expr(&mut self, expr: &mut Expr) {
        if let Expr::Field { base, name, span } = expr
            && let Expr::Name(module) = base.as_ref()
            && let Some(target) = self.alias(module)
        {
            let span = *span;
            match self
                .exports
                .get(&target)
                .and_then(|table| table.get(name.text.as_str()))
            {
                Some(exported) => {
                    *expr = Expr::Name(Name {
                        text: exported.text.clone(),
                        span,
                    });
                }
                None => self.diags.push(
                    Diagnostic::error(
                        Code::NotInScope,
                        name.span,
                        format!("`{}` exports no `{}`", module.text, name.text),
                    )
                    .note(format!(
                        "add `{0} = {0}` to the `return {{ … }}` at the end of the included file",
                        name.text
                    )),
                ),
            }
            return;
        }

        match expr {
            Expr::Name(name) => self.name(name),
            Expr::Field { base, .. } => self.expr(base),
            Expr::Call { callee, args, .. } => {
                self.expr(callee);
                for arg in args {
                    self.expr(arg);
                }
            }
            Expr::MethodCall { receiver, args, .. } => {
                self.expr(receiver);
                for arg in args {
                    self.expr(arg);
                }
            }
            Expr::Function { params, block, .. } => {
                let bound = params.iter().map(|p| p.text.clone()).collect();
                self.block(block, bound);
            }
            Expr::Table { fields, .. } => {
                for field in fields {
                    let (TableField::Named { value, .. } | TableField::Positional { value }) =
                        field;
                    self.expr(value);
                }
            }
            Expr::Binary { lhs, rhs, .. } => {
                self.expr(lhs);
                self.expr(rhs);
            }
            Expr::Unary { operand, .. } => self.expr(operand),
            Expr::Number { .. } | Expr::Str(_) | Expr::Bool { .. } => {}
        }
    }

    fn bound(&self, name: &str) -> bool {
        self.scopes.iter().any(|scope| scope.contains(name))
    }

    /// The file `name` reads through, when it is a `local m = include` of the
    /// file it is read in and nothing in a body shadows it.
    fn alias(&self, name: &Name) -> Option<u32> {
        if self.bound(&name.text) {
            return None;
        }
        self.aliases
            .get(&(name.span.file, name.text.as_str()))
            .copied()
    }

    fn name(&mut self, name: &Name) {
        if self.alias(name).is_some() {
            self.diags.push(
                Diagnostic::error(
                    Code::NotInScope,
                    name.span,
                    format!("`{}` names a file, not a value", name.text),
                )
                .note(format!(
                    "read one of the names it exports: `{}.name`",
                    name.text
                )),
            );
            return;
        }
        if self.bound(&name.text) {
            return;
        }
        let Some(spans) = self.top.get(&name.text) else {
            return;
        };
        if spans.iter().any(|span| span.file == name.span.file) {
            return;
        }
        self.diags.push(
            Diagnostic::error(
                Code::NotInScope,
                name.span,
                format!("`{}` is a `local` of another file", name.text),
            )
            .note_at("it is declared at", spans[0])
            .note("a top-level `local` belongs to its file, as in Lua")
            .note(format!(
                "export it there with `return {{ {0} = {0} }}`, bind that file here with \
                 `local m = include \"…\"`, and read it as `m.{0}`",
                name.text
            )),
        );
    }
}
