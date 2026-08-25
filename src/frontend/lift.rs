//! The whitelist pass: `full-moon`'s Lua CST in, Installua's AST out.
//!
//! One responsibility, and it is worth being strict about what it is *not*.
//! This pass decides **which Lua forms exist in this language**. It does not
//! resolve a name, know an instruction from a declaration, or have an opinion
//! about types — all of which are Phase 2. So `detailPrint(nope)` passes here
//! and `goto done` does not, which is the split that keeps either pass small.
//!
//! Two rewrites happen on the way through, both cheaper before names mean
//! anything:
//!
//!   * `elseif` desugars into a nested `if` in the else branch, so every `If`
//!     downstream has exactly one condition and one layout rule.
//!   * string escapes are decoded, so no later pass re-reads a literal.
//!
//! Everything rejected is rejected *with its replacement named* — a diagnostic
//! that says only "not supported" is a diagnostic that sends the user back to
//! the NSIS docs.

use full_moon::ast as lua;
use full_moon::node::Node;
use full_moon::tokenizer::{TokenReference, TokenType};

use crate::ast::*;
use crate::diag::{Code, Diagnostic, Diagnostics, Span};
use crate::frontend::strings::{self, LiteralKind};

/// The closed set of `for … in` iterators. Syntax is enough to check it, so it
/// is checked here rather than waiting for resolution: `pairs` is the one a Lua
/// programmer reaches for and the one that can never work.
const ITERATORS: &[&str] = &["glob", "lines", "range"];

pub fn lift(ast: &lua::Ast, file: u32, diags: &mut Diagnostics) -> Program {
    let mut lifter = Lifter {
        diags,
        file,
        depth: 0,
    };
    Program {
        block: lifter.block(ast.nodes()),
    }
}

struct Lifter<'a> {
    diags: &'a mut Diagnostics,
    /// Which source this is, stamped onto every span the pass produces —
    /// `full-moon` measures each file from its own byte zero, so the answer is
    /// known here and nowhere downstream.
    file: u32,
    /// How many blocks deep the walk is. One thing depends on it: `include` is
    /// a top-level statement, and nesting is the difference between merging a
    /// file and asking the compiler to do so conditionally, which no stage
    /// could.
    depth: usize,
}

/// Where an expression sits, which decides one thing only: whether a function
/// expression is a declaration body or a value.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Position {
    /// A direct call argument: `section("Core", function() … end)`.
    Argument,
    /// Anywhere else.
    Value,
}

impl Lifter<'_> {
    /// Every span this pass produces comes through here, which is what makes
    /// one field enough to attribute a whole file.
    fn span(&self, node: &impl Node) -> Span {
        span_of(node).in_file(self.file)
    }

    fn name(&self, token: &TokenReference) -> Name {
        Name {
            text: token.token().to_string(),
            span: self.span(token),
        }
    }

    fn block(&mut self, block: &lua::Block) -> Block {
        self.depth += 1;
        let mut out = Vec::new();
        for stmt in block.stmts() {
            if let Some(stmt) = self.stmt(stmt) {
                out.push(stmt);
            }
        }
        if let Some(last) = block.last_stmt()
            && let Some(stmt) = self.last_stmt(last)
        {
            out.push(stmt);
        }
        self.depth -= 1;
        out
    }

    fn stmt(&mut self, stmt: &lua::Stmt) -> Option<Stmt> {
        let span = self.span(stmt);
        match stmt {
            lua::Stmt::Assignment(assignment) => {
                let targets = assignment
                    .variables()
                    .iter()
                    .filter_map(|var| self.var(var))
                    .collect();
                let values = self.expr_list(assignment.expressions(), Position::Value);
                Some(Stmt::Assign {
                    targets,
                    values,
                    span,
                })
            }

            lua::Stmt::LocalAssignment(local) => Some(self.local(local, span)),

            lua::Stmt::FunctionCall(call) => {
                let call = Stmt::Call(self.function_call(call, span)?);
                // An `include` merges one file's declarations into another's,
                // which is something the whole program either does or does not.
                // Inside a body there is no stage that could decide, because
                // the deciding would be install-time and the merging is
                // compile-time. Inside a top-level `if` both halves are
                // compile-time and it is still no: merging happens here, in the
                // frontend, and the branch is taken two passes later — there is
                // no order in which one could inform the other.
                if self.depth > 1
                    && matches!(&call, Stmt::Call(expr) if expr.callee_name() == Some("include"))
                {
                    self.diags.push(
                        Diagnostic::error(
                            Code::IncludeForm,
                            span,
                            "`include` is a top-level statement",
                        )
                        .note("it merges another file's declarations into this one, so it cannot depend on anything decided at install time")
                        .note("a top-level `if` is no exception: files are merged in the frontend, and the branch is taken later, in resolution")
                       .note("move it to the top of the file; declarations are order-free"),
                    );
                    return None;
                }
                Some(call)
            }

            lua::Stmt::Do(block) => Some(Stmt::Do {
                block: self.block(block.block()),
                span,
            }),

            lua::Stmt::If(node) => Some(self.if_stmt(node, span)),

            lua::Stmt::While(node) => Some(Stmt::While {
                cond: self.expr(node.condition(), Position::Value)?,
                block: self.block(node.block()),
                span,
            }),

            lua::Stmt::NumericFor(node) => Some(Stmt::NumericFor {
                name: self.name(node.index_variable()),
                start: self.expr(node.start(), Position::Value)?,
                end: self.expr(node.end(), Position::Value)?,
                step: node.step().and_then(|e| self.expr(e, Position::Value)),
                block: self.block(node.block()),
                span,
            }),

            lua::Stmt::GenericFor(node) => self.generic_for(node, span),

            lua::Stmt::FunctionDeclaration(_) | lua::Stmt::LocalFunction(_) => {
                self.reject(
                    Code::FunctionStatement,
                    span,
                    "a `function` statement is not a declaration here",
                    &["declare it with `func(\"name\", function(…) … end)`"],
                );
                None
            }

            lua::Stmt::Repeat(_) => {
                self.reject(
                    Code::RepeatLoop,
                    span,
                    "`repeat … until` is not supported",
                    &["use `while` — it rotates to a bottom test and costs the same"],
                );
                None
            }

            lua::Stmt::Goto(_) | lua::Stmt::Label(_) => {
                self.reject(
                    Code::Goto,
                    span,
                    "`goto` is not supported",
                    &[
                        "use `continue()` to skip to the next iteration, or `break` to leave the loop",
                        "labels are owned by the compiler's layout pass",
                    ],
                );
                None
            }

            other => {
                self.reject(
                    Code::NotYetImplemented,
                    span,
                    format!("this statement is not part of Installua: `{other}`"),
                    &[],
                );
                None
            }
        }
    }

    fn last_stmt(&mut self, last: &lua::LastStmt) -> Option<Stmt> {
        let span = self.span(last);
        match last {
            lua::LastStmt::Break(_) => Some(Stmt::Break { span }),
            lua::LastStmt::Return(node) => Some(Stmt::Return {
                values: self.expr_list(node.returns(), Position::Value),
                span,
            }),
            other => {
                self.reject(
                    Code::NotYetImplemented,
                    span,
                    format!("this statement is not part of Installua: `{other}`"),
                    &[],
                );
                None
            }
        }
    }

    /// `local x = …`, and `local X <const> = …`, which is a different language
    /// feature wearing the same syntax: build-time, `!define`d, never a
    /// register.
    fn local(&mut self, local: &lua::LocalAssignment, span: Span) -> Stmt {
        let mut is_const = false;
        for attribute in local.attributes().flatten() {
            let text = attribute.name().token().to_string();
            match text.as_str() {
                "const" => is_const = true,
                "close" => self.reject(
                    Code::CloseAttribute,
                    self.span(attribute),
                    "`<close>` is not supported",
                    &["there is no runtime to close over; close a handle explicitly"],
                ),
                other => self.reject(
                    Code::UnknownAttribute,
                    self.span(attribute),
                    format!("`<{other}>` is not a Lua attribute"),
                    &["Lua 5.4 has `<const>` and `<close>`; only `<const>` exists here"],
                ),
            }
        }

        Stmt::Local {
            names: local.names().iter().map(|token| self.name(token)).collect(),
            is_const,
            values: self.expr_list(local.expressions(), Position::Value),
            span,
        }
    }

    /// The `elseif` desugaring. An `if/elseif/elseif/else` chain becomes a
    /// right-nested tree, built from the tail so the `else` is threaded through
    /// exactly once.
    fn if_stmt(&mut self, node: &lua::If, span: Span) -> Stmt {
        let mut else_block = node.else_block().map(|block| self.block(block));

        if let Some(else_ifs) = node.else_if() {
            for else_if in else_ifs.iter().rev() {
                let arm_span = self.span(else_if);
                let Some(cond) = self.expr(else_if.condition(), Position::Value) else {
                    continue;
                };
                let then_block = self.block(else_if.block());
                else_block = Some(vec![Stmt::If {
                    cond,
                    then_block,
                    else_block,
                    span: arm_span,
                }]);
            }
        }

        let then_block = self.block(node.block());
        match self.expr(node.condition(), Position::Value) {
            Some(cond) => Stmt::If {
                cond,
                then_block,
                else_block,
                span,
            },
            // The condition was already diagnosed; keep a well-formed node so
            // the rest of the body is still checked.
            None => Stmt::Do {
                block: then_block,
                span,
            },
        }
    }

    fn generic_for(&mut self, node: &lua::GenericFor, span: Span) -> Option<Stmt> {
        let expressions: Vec<&lua::Expression> = node.expressions().iter().collect();
        let [iterator] = expressions.as_slice() else {
            self.reject(
                Code::UnsupportedIterator,
                span,
                "a `for … in` loop takes exactly one iterator",
                &[&format!("the iterators are {}", iterator_list())],
            );
            return None;
        };

        let iterator = self.expr(iterator, Position::Value)?;
        match iterator.callee_name() {
            Some(name) if ITERATORS.contains(&name) => {}
            Some(name) => {
                let note = match name {
                    "pairs" | "ipairs" => "there are no runtime tables to iterate; \
                         `glob` walks the build machine and `lines` walks a file"
                        .to_string(),
                    _ => format!("the iterators are {}", iterator_list()),
                };
                self.reject(
                    Code::UnsupportedIterator,
                    iterator.span(),
                    format!("`{name}` is not an iterator"),
                    &[&note],
                );
                return None;
            }
            None => {
                self.reject(
                    Code::UnsupportedIterator,
                    iterator.span(),
                    "a `for … in` loop iterates a call, not a value",
                    &[&format!("the iterators are {}", iterator_list())],
                );
                return None;
            }
        }

        Some(Stmt::GenericFor {
            names: node.names().iter().map(|token| self.name(token)).collect(),
            iterator,
            block: self.block(node.block()),
            span,
        })
    }

    // -- expressions ------------------------------------------------------

    fn expr_list(
        &mut self,
        list: &lua::punctuated::Punctuated<lua::Expression>,
        position: Position,
    ) -> Vec<Expr> {
        list.iter().filter_map(|e| self.expr(e, position)).collect()
    }

    fn expr(&mut self, expr: &lua::Expression, position: Position) -> Option<Expr> {
        let span = self.span(expr);
        match expr {
            // Parentheses carry no meaning once precedence is in the tree.
            lua::Expression::Parentheses { expression, .. } => self.expr(expression, position),

            lua::Expression::Number(token) => self.number(token, span),
            lua::Expression::String(token) => Some(Expr::Str(self.string(token, false, span)?)),
            lua::Expression::Symbol(token) => self.symbol(token, span),
            lua::Expression::Var(var) => self.var(var),
            lua::Expression::FunctionCall(call) => self.function_call(call, span),

            lua::Expression::TableConstructor(table) => Some(self.table(table, span)),

            lua::Expression::Function(function) => {
                if position == Position::Value {
                    self.reject(
                        Code::ClosureValue,
                        span,
                        "a function is not a value here",
                        &[
                            "a function expression is a declaration body: it goes directly \
                             into `func(…)`, `section(…)` or `onInit(…)`",
                        ],
                    );
                    return None;
                }
                let body = function.body();
                let mut params = Vec::new();
                for parameter in body.parameters() {
                    match parameter {
                        lua::Parameter::Name(token) => params.push(self.name(token)),
                        other => self.reject(
                            Code::Varargs,
                            self.span(other),
                            "`...` is not a parameter",
                            &["there is no vararg calling convention; the stack is the ABI"],
                        ),
                    }
                }
                Some(Expr::Function {
                    params,
                    block: self.block(body.block()),
                    span,
                })
            }

            lua::Expression::UnaryOperator { unop, expression } => {
                let op = match unop {
                    lua::UnOp::Minus(_) => UnOp::Neg,
                    lua::UnOp::Not(_) => UnOp::Not,
                    lua::UnOp::Tilde(_) => UnOp::BitNot,
                    lua::UnOp::Hash(_) => {
                        self.reject(
                            Code::LengthOperator,
                            span,
                            "`#` is not supported on a string",
                            &[
                                "use `string.len(s)`: `StrLen` counts UTF-16 code units and \
                                 Lua's `#` counts UTF-8 bytes, and they agree only on ASCII",
                            ],
                        );
                        return None;
                    }
                    other => {
                        self.reject(
                            Code::NotYetImplemented,
                            span,
                            format!("`{other}` is not an Installua operator"),
                            &[],
                        );
                        return None;
                    }
                };
                Some(Expr::Unary {
                    op,
                    operand: Box::new(self.expr(expression, Position::Value)?),
                    span,
                })
            }

            lua::Expression::BinaryOperator { lhs, binop, rhs } => {
                let op = self.binop(binop, span)?;
                // Both sides are lifted even when the operator was rejected, so
                // a mistake inside one does not hide a mistake inside the other.
                let lhs = self.expr(lhs, Position::Value);
                let rhs = self.expr(rhs, Position::Value);
                Some(Expr::Binary {
                    op,
                    lhs: Box::new(lhs?),
                    rhs: Box::new(rhs?),
                    span,
                })
            }

            other => {
                self.reject(
                    Code::NotYetImplemented,
                    span,
                    format!("this expression is not part of Installua: `{other}`"),
                    &[],
                );
                None
            }
        }
    }

    fn binop(&mut self, binop: &lua::BinOp, span: Span) -> Option<BinOp> {
        let op = match binop {
            lua::BinOp::Plus(_) => BinOp::Add,
            lua::BinOp::Minus(_) => BinOp::Sub,
            lua::BinOp::Star(_) => BinOp::Mul,
            lua::BinOp::DoubleSlash(_) => BinOp::FloorDiv,
            lua::BinOp::Percent(_) => BinOp::Mod,
            lua::BinOp::TwoDots(_) => BinOp::Concat,
            lua::BinOp::TwoEqual(_) => BinOp::Eq,
            lua::BinOp::TildeEqual(_) => BinOp::Ne,
            lua::BinOp::LessThan(_) => BinOp::Lt,
            lua::BinOp::LessThanEqual(_) => BinOp::Le,
            lua::BinOp::GreaterThan(_) => BinOp::Gt,
            lua::BinOp::GreaterThanEqual(_) => BinOp::Ge,
            lua::BinOp::And(_) => BinOp::And,
            lua::BinOp::Or(_) => BinOp::Or,
            lua::BinOp::Ampersand(_) => BinOp::BitAnd,
            lua::BinOp::Pipe(_) => BinOp::BitOr,
            lua::BinOp::Tilde(_) => BinOp::BitXor,
            lua::BinOp::DoubleLessThan(_) => BinOp::Shl,
            lua::BinOp::DoubleGreaterThan(_) => BinOp::Shr,

            lua::BinOp::Slash(_) => {
                self.reject(
                    Code::FloatDivision,
                    span,
                    "`/` is float division, and there are no floats",
                    &["use `//`, which is integer division and means what Lua says it means"],
                );
                return None;
            }
            lua::BinOp::Caret(_) => {
                self.reject(
                    Code::Exponentiation,
                    span,
                    "`^` is exponentiation, which NSIS cannot do",
                    &[
                        "NSIS's `^` is bitwise xor, so mapping this across would be silently \
                         wrong; write `~` for xor, or a multiplication for a small power",
                    ],
                );
                return None;
            }
            other => {
                self.reject(
                    Code::NotYetImplemented,
                    span,
                    format!("`{other}` is not an Installua operator"),
                    &[],
                );
                return None;
            }
        };
        Some(op)
    }

    /// A `Var` is either a bare name or a prefix with suffixes — `a.b`, `f()`,
    /// `a.b:c()`. The suffix chain is folded left, which is the shape the AST
    /// wants anyway.
    fn var(&mut self, var: &lua::Var) -> Option<Expr> {
        match var {
            lua::Var::Name(token) => Some(Expr::Name(self.name(token))),
            lua::Var::Expression(expression) => {
                let base = self.prefix(expression.prefix())?;
                self.suffixes(base, expression.suffixes())
            }
            other => {
                self.reject(
                    Code::NotYetImplemented,
                    self.span(other),
                    format!("this expression is not part of Installua: `{other}`"),
                    &[],
                );
                None
            }
        }
    }

    fn function_call(&mut self, call: &lua::FunctionCall, span: Span) -> Option<Expr> {
        let base = self.prefix(call.prefix())?;
        let call = self.suffixes(base, call.suffixes())?;

        if call.callee_name() == Some("require") {
            self.reject(
                Code::RuntimeRequire,
                span,
                "`require` does not exist",
                &[
                    "there is no load at install time; `import \"WinVer\"` pulls in an NSIS \
                     header and `include` splits a project across files",
                ],
            );
            return None;
        }

        Some(call)
    }

    fn prefix(&mut self, prefix: &lua::Prefix) -> Option<Expr> {
        match prefix {
            lua::Prefix::Name(token) => Some(Expr::Name(self.name(token))),
            lua::Prefix::Expression(expression) => self.expr(expression, Position::Value),
            other => {
                self.reject(
                    Code::NotYetImplemented,
                    self.span(other),
                    format!("this expression is not part of Installua: `{other}`"),
                    &[],
                );
                None
            }
        }
    }

    fn suffixes<'s>(
        &mut self,
        base: Expr,
        suffixes: impl Iterator<Item = &'s lua::Suffix>,
    ) -> Option<Expr> {
        let mut acc = base;
        for suffix in suffixes {
            let span = acc.span().join(self.span(suffix));
            acc = match suffix {
                lua::Suffix::Index(lua::Index::Dot { name, .. }) => Expr::Field {
                    base: Box::new(acc),
                    name: self.name(name),
                    span,
                },
                lua::Suffix::Index(lua::Index::Brackets { expression, .. }) => {
                    self.reject(
                        Code::IndexExpression,
                        self.span(expression),
                        "`[…]` indexing is not supported",
                        &[
                            "there are no runtime tables; a compile-time table's fields are \
                             reached with `.`",
                        ],
                    );
                    return None;
                }
                lua::Suffix::Call(lua::Call::AnonymousCall(args)) => {
                    // `raw [[ … ]]` and `raw.head [[ … ]]` alike: the anchored
                    // form is a different position, not a different kind of
                    // text, so the head of the callee is what decides whether
                    // the block is read. Missing this is not a cosmetic bug —
                    // it is the line that keeps `$` from being escaped inside a
                    // block nobody is supposed to have touched.
                    let verbatim = match &acc {
                        Expr::Name(name) => name.text == "raw",
                        Expr::Field { base, .. } => base.name() == Some("raw"),
                        _ => false,
                    };
                    Expr::Call {
                        args: self.args(args, verbatim),
                        callee: Box::new(acc),
                        span,
                    }
                }
                lua::Suffix::Call(lua::Call::MethodCall(method)) => Expr::MethodCall {
                    receiver: Box::new(acc),
                    method: self.name(method.name()),
                    args: self.args(method.args(), false),
                    span,
                },
                other => {
                    self.reject(
                        Code::NotYetImplemented,
                        self.span(other),
                        format!("this expression is not part of Installua: `{other}`"),
                        &[],
                    );
                    return None;
                }
            };
        }
        Some(acc)
    }

    /// `verbatim` is true for the arguments of `raw`, whose string is NSIS
    /// source rather than data — so `$INSTDIR` in there is correct and warning
    /// about it would be noise.
    fn args(&mut self, args: &lua::FunctionArgs, verbatim: bool) -> Vec<Expr> {
        match args {
            lua::FunctionArgs::Parentheses { arguments, .. } => arguments
                .iter()
                .filter_map(|argument| match (verbatim, argument) {
                    (true, lua::Expression::String(token)) => {
                        let span = self.span(argument);
                        self.string(token, true, span).map(Expr::Str)
                    }
                    _ => self.expr(argument, Position::Argument),
                })
                .collect(),
            lua::FunctionArgs::String(token) => {
                let span = self.span(token);
                self.string(token, verbatim, span)
                    .map(Expr::Str)
                    .into_iter()
                    .collect()
            }
            lua::FunctionArgs::TableConstructor(table) => {
                let span = self.span(table);
                vec![self.table(table, span)]
            }
            other => {
                self.reject(
                    Code::NotYetImplemented,
                    self.span(other),
                    "this call form is not part of Installua",
                    &[],
                );
                Vec::new()
            }
        }
    }

    fn table(&mut self, table: &lua::TableConstructor, span: Span) -> Expr {
        let mut fields = Vec::new();
        for field in table.fields() {
            match field {
                lua::Field::NameKey { key, value, .. } => {
                    if let Some(value) = self.expr(value, Position::Argument) {
                        fields.push(TableField::Named {
                            name: self.name(key),
                            value,
                        });
                    }
                }
                lua::Field::NoKey(value) => {
                    if let Some(value) = self.expr(value, Position::Argument) {
                        fields.push(TableField::Positional { value });
                    }
                }
                lua::Field::ExpressionKey { key, .. } => {
                    self.reject(
                        Code::IndexExpression,
                        self.span(key),
                        "`[…] =` is not a table key here",
                        &["a compile-time table's keys are names, as the fields they become are"],
                    );
                }
                other => {
                    self.reject(
                        Code::NotYetImplemented,
                        self.span(other),
                        "this table field is not part of Installua",
                        &[],
                    );
                }
            }
        }
        Expr::Table { fields, span }
    }

    // -- tokens -----------------------------------------------------------

    fn number(&mut self, token: &TokenReference, span: Span) -> Option<Expr> {
        let TokenType::Number { text } = token.token().token_type() else {
            return None;
        };
        let text = text.to_string();

        match parse_integer(&text) {
            Some(value) => Some(Expr::Number { value, span }),
            None => {
                self.reject(
                    Code::FloatLiteral,
                    span,
                    format!("`{text}` is not an integer"),
                    &[
                        "NSIS has no float arithmetic at all, so there is nothing to lower a \
                         float to",
                    ],
                );
                None
            }
        }
    }

    fn string(&mut self, token: &TokenReference, verbatim: bool, span: Span) -> Option<StrLit> {
        let TokenType::StringLiteral {
            literal,
            quote_type,
            ..
        } = token.token().token_type()
        else {
            return None;
        };
        let long = *quote_type == full_moon::tokenizer::StringLiteralQuoteType::Brackets;
        let kind = LiteralKind { long, verbatim };
        Some(StrLit {
            value: strings::decode(literal, kind, span, self.diags),
            long,
            span,
        })
    }

    fn symbol(&mut self, token: &TokenReference, span: Span) -> Option<Expr> {
        match token.token().to_string().as_str() {
            "true" => Some(Expr::Bool { value: true, span }),
            "false" => Some(Expr::Bool { value: false, span }),
            "nil" => {
                self.reject(
                    Code::NilValue,
                    span,
                    "`nil` does not exist",
                    &[
                        "every value has a type and a representation; an absent one is spelled \
                         with the empty string or a `bool`",
                    ],
                );
                None
            }
            "..." => {
                self.reject(
                    Code::Varargs,
                    span,
                    "`...` does not exist",
                    &["there is no vararg calling convention; the stack is the ABI"],
                );
                None
            }
            other => {
                self.reject(
                    Code::NotYetImplemented,
                    span,
                    format!("`{other}` is not part of Installua"),
                    &[],
                );
                None
            }
        }
    }

    fn reject(&mut self, code: Code, span: Span, message: impl Into<String>, notes: &[&str]) {
        let mut diagnostic = Diagnostic::error(code, span, message);
        for note in notes {
            diagnostic = diagnostic.note(*note);
        }
        self.diags.push(diagnostic);
    }
}

fn iterator_list() -> String {
    ITERATORS
        .iter()
        .map(|name| format!("`{name}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Lua integer literals, decimal and hexadecimal. Anything with a fraction or
/// an exponent is not one, which is what makes the float rejection a *literal*
/// check rather than a type check.
fn parse_integer(text: &str) -> Option<i64> {
    let trimmed = text.trim();
    if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        if hex.contains('.') || hex.contains('p') || hex.contains('P') {
            return None;
        }
        // NSIS integers are 32-bit and wrap; a hex literal is written for its
        // bit pattern, so `0xFFFFFFFF` is read as one rather than rejected.
        return u64::from_str_radix(hex, 16).ok().map(|v| v as i64);
    }
    if trimmed.contains('.') || trimmed.contains('e') || trimmed.contains('E') {
        return None;
    }
    trimmed.parse::<i64>().ok()
}

pub fn span_of(node: &impl Node) -> Span {
    let (start, end) = (node.start_position(), node.end_position());
    match (start, end) {
        (Some(start), Some(end)) => Span {
            file: 0,
            start_line: start.line(),
            start_column: start.character(),
            end_line: end.line(),
            end_column: end.character(),
            start_byte: start.bytes(),
            end_byte: end.bytes(),
        },
        _ => Span::default(),
    }
}
