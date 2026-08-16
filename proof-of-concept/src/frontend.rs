//! Phase 1-2: parse full Lua with full-moon, then run a whitelist pass (§1).
//! Anything outside the supported subset is rejected with a real diagnostic
//! rather than half-supported.
//!
//! Two walks over the top level, not one: names are resolved before any body is
//! lowered, so `import`, `plugin` and `function` bindings work regardless of the
//! order they appear in. `<const>` is the exception — the preprocessor is
//! sequential, so constants see only the constants above them (§12).

use std::collections::{BTreeMap, BTreeSet};

use full_moon::ast as lua;
use full_moon::node::Node;
use full_moon::tokenizer::{TokenReference, TokenType};

use crate::ast::{
    BinOp, Call, CmpOp, Condition, Define, Expr, Field, Function, Handler, If, Installer, Local,
    MacroCall, MessageBox, PluginCall, PreCall, Program, Section, SectionGroup, SectionItem, Stmt,
    Ty,
};
use crate::diag::{Diagnostic, Diagnostics, Severity, Span};
use crate::overlay;

pub fn parse(source: &str, diags: &mut Diagnostics) -> Option<Program> {
    let ast = match full_moon::parse(source) {
        Ok(ast) => ast,
        Err(errors) => {
            for error in errors {
                let span = match &error {
                    full_moon::Error::AstError(e) => position_span(e.range().0),
                    full_moon::Error::TokenizerError(e) => position_span(e.position()),
                };
                diags.push(Diagnostic::error("E001", span, error.error_message()));
            }
            return None;
        }
    };

    let mut frontend = Frontend {
        diags,
        functions: BTreeSet::new(),
        imports: BTreeMap::new(),
        plugins: BTreeMap::new(),
        consts: BTreeMap::new(),
        defines: Vec::new(),
        used_headers: BTreeSet::new(),
        inits: Vec::new(),
    };

    let stmts: Vec<&lua::Stmt> = ast.nodes().stmts().collect();
    frontend.resolve(&stmts);
    Some(frontend.program(&stmts))
}

/// What a name in call position turned out to mean.
enum Target<'a> {
    Plain {
        name: String,
        args: &'a lua::FunctionArgs,
    },
    Method {
        object: String,
        method: String,
        args: &'a lua::FunctionArgs,
    },
}

struct Frontend<'a> {
    diags: &'a mut Diagnostics,
    functions: BTreeSet<String>,
    imports: BTreeMap<String, Import>,
    plugins: BTreeMap<String, String>,
    consts: BTreeMap<String, ConstDef>,
    defines: Vec<Define>,
    used_headers: BTreeSet<&'static str>,
    inits: Vec<&'static str>,
}

/// A `<const>` is build-time, so what it holds is NSIS text — `${NAME}` — not a
/// register and not a copy of its initializer (§7-1).
struct ConstDef {
    text: String,
    ty: Ty,
}

impl Frontend<'_> {
    /// Pass 1: collect every top-level binding, and diagnose the bindings
    /// themselves. Bodies are not walked here.
    fn resolve(&mut self, stmts: &[&lua::Stmt]) {
        for stmt in stmts {
            match stmt {
                lua::Stmt::FunctionDeclaration(declaration) => {
                    let Some(name) = self.function_name(declaration) else {
                        continue;
                    };
                    if !self.functions.insert(name.clone()) {
                        self.diags.push(Diagnostic::error(
                            "E006",
                            node_span(*stmt),
                            format!("duplicate function `{name}`"),
                        ));
                    }
                }
                lua::Stmt::LocalAssignment(local) if is_const(local) => self.constant(local),
                lua::Stmt::LocalAssignment(local) => self.binding(local),
                _ => {}
            }
        }
    }

    /// `local x = import "…"` and `local x = plugin "…"`. Both are compile-time
    /// bindings, not registers — nothing is emitted for them.
    fn binding(&mut self, local: &lua::LocalAssignment) {
        let span = node_span(local);
        let Some(Binding {
            name,
            kind,
            argument,
        }) = binding_shape(local)
        else {
            return;
        };

        let Some(argument) = argument else {
            self.diags.push(Diagnostic::error(
                "E005",
                span,
                format!("`{kind}` takes a constant string"),
            ));
            return;
        };

        match kind {
            "import" => {
                let Some(header) = overlay::header(&argument) else {
                    let known: Vec<&str> = overlay::HEADERS.iter().map(|h| h.luis).collect();
                    self.diags.push(
                        Diagnostic::error("E003", span, format!("unknown header `{argument}`"))
                            .with_note(format!("known headers: {}", known.join(", "))),
                    );
                    return;
                };
                self.imports.insert(name, Import { header, span });
            }
            _ => {
                self.plugins.insert(name, argument);
            }
        }
    }

    /// `local NAME <const> = …` is `!define` (§7-1): the value is build-time,
    /// the name is `${NAME}` everywhere it is used.
    fn constant(&mut self, local: &lua::LocalAssignment) {
        let span = node_span(local);

        if let Some(attribute) = local.attributes().flatten().next()
            && attribute.name().token().to_string() != "const"
        {
            self.diags.push(Diagnostic::error(
                "E002",
                span,
                "only the `<const>` attribute is supported",
            ));
            return;
        }

        let names: Vec<&TokenReference> = local.names().iter().collect();
        let expressions: Vec<&lua::Expression> = local.expressions().iter().collect();
        let ([name], [value]) = (names.as_slice(), expressions.as_slice()) else {
            self.diags.push(Diagnostic::error(
                "E004",
                span,
                "a `<const>` binds exactly one name to one value",
            ));
            return;
        };

        let name = name.token().to_string();
        if self.consts.contains_key(&name) {
            self.diags.push(Diagnostic::error(
                "E006",
                span,
                format!("duplicate constant `{name}`"),
            ));
            return;
        }

        // `pre.*` is the one initializer that is not an expression: it runs a
        // command on the build machine and defines symbols from its output.
        if let Some(define) = self.pre_call(&name, value, span) {
            self.consts.insert(
                name,
                ConstDef {
                    text: define.0,
                    ty: define.1,
                },
            );
            self.defines.push(define.2);
            return;
        }

        let Some((value, ty)) = self.const_value(value) else {
            return;
        };

        self.defines.push(Define {
            name: name.clone(),
            value: value.clone(),
            pre: None,
            span,
        });
        self.consts.insert(
            name.clone(),
            ConstDef {
                text: format!("${{{name}}}"),
                ty,
            },
        );
    }

    /// `local V <const> = pre.getDllVersion("app.dll")` — the command defines
    /// `V_1`…`V_4` on the build machine, and `V` names their composition (§7-3).
    fn pre_call(
        &mut self,
        name: &str,
        expression: &lua::Expression,
        span: Span,
    ) -> Option<(String, Ty, Define)> {
        let lua::Expression::FunctionCall(call) = expression else {
            return None;
        };
        let lua::Prefix::Name(prefix) = call.prefix() else {
            return None;
        };
        if prefix.token().to_string() != "pre" {
            return None;
        }

        let suffixes: Vec<&lua::Suffix> = call.suffixes().collect();
        let [
            lua::Suffix::Index(lua::Index::Dot { name: method, .. }),
            lua::Suffix::Call(lua::Call::AnonymousCall(args)),
        ] = suffixes.as_slice()
        else {
            self.diags.push(Diagnostic::error(
                "E002",
                span,
                "`pre` is a namespace, as in `pre.getDllVersion(…)`",
            ));
            return None;
        };

        let method = method.token().to_string();
        let Some(pre) = overlay::pre(&method) else {
            let known: Vec<&str> = overlay::PRE.iter().map(|p| p.luis).collect();
            self.diags.push(
                Diagnostic::error(
                    "E003",
                    span,
                    format!("unknown build-time command `pre.{method}`"),
                )
                .with_note(format!("known: {}", known.join(", "))),
            );
            return None;
        };

        let lua::FunctionArgs::Parentheses { arguments, .. } = args else {
            self.diags.push(Diagnostic::error(
                "E004",
                span,
                format!("`pre.{method}` must be called with parentheses"),
            ));
            return None;
        };

        let mut values = Vec::new();
        for argument in arguments {
            let (text, _) = self.const_value(argument)?;
            values.push(text);
        }
        if values.len() != pre.inputs {
            self.diags.push(Diagnostic::error(
                "E004",
                span,
                format!(
                    "`pre.{method}` expects {} argument(s), got {}",
                    pre.inputs,
                    values.len()
                ),
            ));
            return None;
        }

        let prefix = format!("{name}_");
        let value = pre
            .outputs
            .iter()
            .map(|output| format!("${{{prefix}{output}}}"))
            .collect::<Vec<String>>()
            .join(pre.separator);

        Some((
            format!("${{{name}}}"),
            pre.ty,
            Define {
                name: name.to_string(),
                value,
                pre: Some(PreCall {
                    pre,
                    args: values,
                    prefix,
                }),
                span,
            },
        ))
    }

    /// Build-time evaluation (§7-2): the result is NSIS text plus a type, so a
    /// constant that refers to another one composes `${A} ${B}` rather than
    /// copying either value.
    fn const_value(&mut self, expression: &lua::Expression) -> Option<(String, Ty)> {
        let span = node_span(expression);
        match expression {
            lua::Expression::Number(_) => {
                let value = self.integer(expression)?;
                Some((value.to_string(), Ty::Int))
            }
            lua::Expression::String(token) => {
                string_value(token, self.diags).map(|value| (value, Ty::Str))
            }
            lua::Expression::Parentheses { expression, .. } => self.const_value(expression),
            lua::Expression::Var(lua::Var::Name(name)) => {
                let name = name.token().to_string();
                match self.consts.get(&name) {
                    Some(def) => Some((def.text.clone(), def.ty)),
                    None => {
                        self.diags.push(
                            Diagnostic::error(
                                "E003",
                                span,
                                format!("`{name}` is not a build-time constant"),
                            )
                            .with_note(
                                "a `<const>` can only be built from literals and earlier `<const>`s",
                            ),
                        );
                        None
                    }
                }
            }
            lua::Expression::BinaryOperator { lhs, binop, rhs } => match binop {
                lua::BinOp::TwoDots(_) => {
                    let (lhs, _) = self.const_value(lhs)?;
                    let (rhs, _) = self.const_value(rhs)?;
                    Some((format!("{lhs}{rhs}"), Ty::Str))
                }
                _ => {
                    // Arithmetic folds, and only folds: `!define /math` exists,
                    // but a compiler that constant-folds never needs it (§7-2).
                    let op = self.binary_op(binop, span)?;
                    let lhs = self.integer(lhs)?;
                    let rhs = self.integer(rhs)?;
                    let Some(value) = op.fold(lhs, rhs) else {
                        self.diags.push(Diagnostic::error(
                            "E005",
                            span,
                            "this constant expression does not evaluate",
                        ));
                        return None;
                    };
                    Some((value.to_string(), Ty::Int))
                }
            },
            _ => {
                self.diags.push(
                    Diagnostic::error("E005", span, "not a build-time constant").with_note(
                        "`<const>` values are folded at build time, so they cannot read registers",
                    ),
                );
                None
            }
        }
    }

    /// A number literal, as an integer. Used where nothing else will do.
    fn integer(&mut self, expression: &lua::Expression) -> Option<i64> {
        let span = node_span(expression);
        match expression {
            lua::Expression::Parentheses { expression, .. } => self.integer(expression),
            lua::Expression::Number(token) => match token.token().token_type() {
                // §3/§10-7: floats are rejected outright rather than truncated.
                TokenType::Number { text } => match text.parse::<i64>() {
                    Ok(value) => Some(value),
                    Err(_) => {
                        self.diags.push(
                            Diagnostic::error("E005", span, format!("`{text}` is not an integer"))
                                .with_note(
                                    "NSIS has no floating point; only integers are supported",
                                ),
                        );
                        None
                    }
                },
                _ => None,
            },
            _ => {
                self.diags.push(Diagnostic::error(
                    "E005",
                    span,
                    "expected an integer literal",
                ));
                None
            }
        }
    }

    /// Pass 2: lower everything, now that every name is known.
    fn program(&mut self, stmts: &[&lua::Stmt]) -> Program {
        let mut program = Program {
            installer: None,
            includes: Vec::new(),
            inits: Vec::new(),
            defines: Vec::new(),
            functions: Vec::new(),
            items: Vec::new(),
        };

        for stmt in stmts {
            match stmt {
                lua::Stmt::FunctionCall(call) => self.top_level_call(call, &mut program),
                lua::Stmt::FunctionDeclaration(declaration) => {
                    if let Some(function) = self.function(declaration) {
                        program.functions.push(function);
                    }
                }
                // Already handled by `resolve`; anything else is a mistake.
                lua::Stmt::LocalAssignment(local)
                    if binding_shape(local).is_some() || is_const(local) => {}
                lua::Stmt::LocalAssignment(local) => self.diags.push(
                    Diagnostic::error(
                        "E002",
                        node_span(local),
                        "a top-level `local` must bind an `import`, a `plugin` or a `<const>`",
                    )
                    .with_note("there is no build-time value to hold otherwise (§7)"),
                ),
                other => self.diags.push(
                    Diagnostic::error(
                        "E002",
                        node_span(other),
                        "unsupported construct at the top level",
                    )
                    .with_note(
                        "supported: `installer`, `section`, `sectionGroup`, `function`, `import`, `plugin` and `<const>`",
                    ),
                ),
            }
        }

        if program.installer.is_none() {
            self.diags.push(Diagnostic::error(
                "E006",
                Span::new(1, 1),
                "missing `installer { ... }` block",
            ));
        }

        // §7-4: emit each `!include` once, only if something used it, in
        // overlay order so the output stays stable.
        program.includes = overlay::HEADERS
            .iter()
            .filter(|header| self.used_headers.contains(header.luis))
            .collect();
        program.inits = std::mem::take(&mut self.inits);
        program.defines = std::mem::take(&mut self.defines);

        for (name, import) in &self.imports {
            if !self.used_headers.contains(import.header.luis) {
                self.diags.push(Diagnostic {
                    severity: Severity::Warning,
                    code: "W001",
                    span: import.span,
                    message: format!("unused import `{name}`"),
                    note: Some(format!(
                        "no `!include \"{}\"` was emitted",
                        import.header.include
                    )),
                });
            }
        }

        program
    }

    fn function_name(&mut self, declaration: &lua::FunctionDeclaration) -> Option<String> {
        let span = node_span(declaration);
        let name = declaration.name();
        let names: Vec<&TokenReference> = name.names().iter().collect();

        if name.method_name().is_some() || names.len() != 1 {
            self.diags.push(
                Diagnostic::error("E002", span, "only plain function names are supported")
                    .with_note("there are no tables to hang a method off (§3)"),
            );
            return None;
        }

        Some(names[0].token().to_string())
    }

    fn function(&mut self, declaration: &lua::FunctionDeclaration) -> Option<Function> {
        let span = node_span(declaration);
        let name = self.function_name(declaration)?;

        let body = declaration.body();
        if body.parameters().iter().next().is_some() {
            self.diags.push(
                Diagnostic::error("E004", span, "functions take no parameters yet").with_note(
                    "parameters need a calling convention, which is still open (§3, §10-3)",
                ),
            );
        }

        Some(Function {
            name,
            span,
            body: self.block(body.block()),
        })
    }

    fn top_level_call(&mut self, call: &lua::FunctionCall, program: &mut Program) {
        let span = node_span(call);
        let Some(Target::Plain { name, args }) = self.call_target(call) else {
            return;
        };

        match name.as_str() {
            "installer" => {
                if program.installer.is_some() {
                    self.diags.push(Diagnostic::error(
                        "E006",
                        span,
                        "duplicate `installer` block",
                    ));
                    return;
                }
                program.installer = self.installer(args, span);
            }
            "section" => {
                if let Some(section) = self.section(args, span) {
                    program.items.push(SectionItem::Section(section));
                }
            }
            "sectionGroup" => {
                if let Some(group) = self.section_group(args, span) {
                    program.items.push(SectionItem::Group(group));
                }
            }
            other => self.diags.push(Diagnostic::error(
                "E003",
                span,
                format!("unknown top-level call `{other}`"),
            )),
        }
    }

    fn installer(&mut self, args: &lua::FunctionArgs, span: Span) -> Option<Installer> {
        let lua::FunctionArgs::TableConstructor(table) = args else {
            self.diags.push(Diagnostic::error(
                "E004",
                span,
                "`installer` takes a single table, as in `installer { name = \"…\" }`",
            ));
            return None;
        };

        let mut fields = Vec::new();
        for field in table.fields() {
            let field_span = node_span(field);
            let lua::Field::NameKey { key, value, .. } = field else {
                self.diags.push(Diagnostic::error(
                    "E002",
                    field_span,
                    "only `name = value` fields are supported in `installer`",
                ));
                continue;
            };

            let key_name = key.token().to_string();
            if overlay::installer_field(&key_name).is_none() {
                let known: Vec<&str> = overlay::INSTALLER_FIELDS.iter().map(|f| f.luis).collect();
                self.diags.push(
                    Diagnostic::error(
                        "E003",
                        field_span,
                        format!("unknown `installer` field `{key_name}`"),
                    )
                    .with_note(format!("supported fields: {}", known.join(", "))),
                );
                continue;
            }

            // Attributes are build-time by nature, so a field is exactly what a
            // `<const>` initializer is: folded text.
            let Some((value, ty)) = self.const_value(value) else {
                continue;
            };
            if ty != Ty::Str {
                self.diags.push(
                    Diagnostic::error(
                        "E005",
                        field_span,
                        format!("`{key_name}` is a string, found a {}", ty.name()),
                    )
                    .with_note("installer attributes are text, even when they look numeric"),
                );
                continue;
            }
            fields.push(Field {
                name: key_name,
                value,
                span: field_span,
            });
        }

        Some(Installer { span, fields })
    }

    /// `section("name", body)` and `section("name", { optional = true }, body)`.
    fn section(&mut self, args: &lua::FunctionArgs, span: Span) -> Option<Section> {
        let arguments = self.declaration_args("section", args, span)?;
        let (name, options, body) = self.name_options_body("section", &arguments, span)?;

        let optional = self.option(&options, "optional", overlay::SECTION_OPTIONS)?;

        Some(Section {
            name,
            optional,
            span,
            body: self.block(body.block()),
        })
    }

    /// A section group is a section that contains sections, so its body is
    /// checked structurally rather than lowered (§13).
    fn section_group(&mut self, args: &lua::FunctionArgs, span: Span) -> Option<SectionGroup> {
        let arguments = self.declaration_args("sectionGroup", args, span)?;
        let (name, options, body) = self.name_options_body("sectionGroup", &arguments, span)?;

        let expanded = self.option(&options, "expanded", overlay::SECTION_GROUP_OPTIONS)?;

        let mut sections = Vec::new();
        for stmt in body.block().stmts() {
            let stmt_span = node_span(stmt);
            let lua::Stmt::FunctionCall(call) = stmt else {
                self.diags.push(Diagnostic::error(
                    "E002",
                    stmt_span,
                    "a `sectionGroup` body holds only `section` declarations",
                ));
                continue;
            };
            match self.call_target(call) {
                Some(Target::Plain { name, args }) if name == "section" => {
                    if let Some(section) = self.section(args, stmt_span) {
                        sections.push(section);
                    }
                }
                Some(_) => self.diags.push(Diagnostic::error(
                    "E002",
                    stmt_span,
                    "a `sectionGroup` body holds only `section` declarations",
                )),
                None => {}
            }
        }

        Some(SectionGroup {
            name,
            expanded,
            sections,
            span,
        })
    }

    fn declaration_args<'a>(
        &mut self,
        kind: &str,
        args: &'a lua::FunctionArgs,
        span: Span,
    ) -> Option<Vec<&'a lua::Expression>> {
        let lua::FunctionArgs::Parentheses { arguments, .. } = args else {
            self.diags.push(Diagnostic::error(
                "E004",
                span,
                format!("`{kind}` takes a name and a body, as in `{kind}(\"…\", function() end)`"),
            ));
            return None;
        };
        Some(arguments.iter().collect())
    }

    /// The shared shape of `section` and `sectionGroup`: a constant name, an
    /// optional table of flags, and an anonymous function.
    fn name_options_body<'a>(
        &mut self,
        kind: &str,
        arguments: &[&'a lua::Expression],
        span: Span,
    ) -> Option<(String, Vec<(String, bool, Span)>, &'a lua::FunctionBody)> {
        let (name_expr, options_expr, body_expr) = match arguments {
            [name, body] => (*name, None, *body),
            [name, options, body] => (*name, Some(*options), *body),
            _ => {
                self.diags.push(Diagnostic::error(
                    "E004",
                    span,
                    format!("`{kind}` expects 2 or 3 arguments, got {}", arguments.len()),
                ));
                return None;
            }
        };

        let name = const_string_in(name_expr, self.diags)?;

        let mut options = Vec::new();
        if let Some(options_expr) = options_expr {
            let lua::Expression::TableConstructor(table) = options_expr else {
                self.diags.push(Diagnostic::error(
                    "E004",
                    node_span(options_expr),
                    format!("the second argument to `{kind}` is a table of options"),
                ));
                return None;
            };
            for field in table.fields() {
                let field_span = node_span(field);
                let lua::Field::NameKey { key, value, .. } = field else {
                    self.diags.push(Diagnostic::error(
                        "E002",
                        field_span,
                        "only `name = value` fields are supported here",
                    ));
                    continue;
                };
                let Some(value) = self.boolean(value) else {
                    continue;
                };
                options.push((key.token().to_string(), value, field_span));
            }
        }

        let lua::Expression::Function(function) = body_expr else {
            self.diags.push(Diagnostic::error(
                "E002",
                node_span(body_expr),
                format!("a `{kind}` body must be an anonymous function"),
            ));
            return None;
        };

        let body = function.body();
        if body.parameters().iter().next().is_some() {
            self.diags.push(Diagnostic::error(
                "E004",
                node_span(body_expr),
                format!("a `{kind}` body takes no parameters"),
            ));
        }

        Some((name, options, body))
    }

    /// Reads one flag out of an options table, and rejects the rest by name so
    /// a typo is a diagnostic rather than a silently dropped flag.
    fn option(
        &mut self,
        options: &[(String, bool, Span)],
        wanted: &str,
        known: &[&str],
    ) -> Option<bool> {
        let mut value = false;
        for (name, flag, span) in options {
            if name == wanted {
                value = *flag;
            } else {
                self.diags.push(
                    Diagnostic::error("E003", *span, format!("unknown option `{name}`"))
                        .with_note(format!("supported options: {}", known.join(", "))),
                );
            }
        }
        Some(value)
    }

    fn boolean(&mut self, expression: &lua::Expression) -> Option<bool> {
        let span = node_span(expression);
        match expression {
            lua::Expression::Symbol(symbol) => match symbol.token().to_string().as_str() {
                "true" => Some(true),
                "false" => Some(false),
                _ => {
                    self.diags.push(Diagnostic::error(
                        "E005",
                        span,
                        "expected `true` or `false`",
                    ));
                    None
                }
            },
            _ => {
                self.diags.push(Diagnostic::error(
                    "E005",
                    span,
                    "expected `true` or `false`",
                ));
                None
            }
        }
    }

    fn block(&mut self, block: &lua::Block) -> Vec<Stmt> {
        let mut stmts = Vec::new();
        for stmt in block.stmts() {
            match stmt {
                lua::Stmt::FunctionCall(call) => {
                    if let Some(stmt) = self.call_stmt(call) {
                        stmts.push(stmt);
                    }
                }
                lua::Stmt::LocalAssignment(local) => {
                    if let Some(local) = self.local(local) {
                        stmts.push(Stmt::Local(local));
                    }
                }
                lua::Stmt::If(if_stmt) => {
                    if let Some(if_stmt) = self.if_stmt(if_stmt) {
                        stmts.push(Stmt::If(if_stmt));
                    }
                }
                other => self.diags.push(
                    Diagnostic::error("E002", node_span(other), "unsupported statement")
                        .with_note("supported: calls, `local`, and `if`"),
                ),
            }
        }

        if let Some(last) = block.last_stmt() {
            self.diags.push(Diagnostic::error(
                "E002",
                node_span(last),
                "`return`, `break` and `goto` are not supported",
            ));
        }

        stmts
    }

    fn local(&mut self, local: &lua::LocalAssignment) -> Option<Local> {
        let span = node_span(local);

        if binding_shape(local).is_some() {
            self.diags.push(Diagnostic::error(
                "E002",
                span,
                "`import` and `plugin` are only allowed at the top level",
            ));
            return None;
        }

        if is_const(local) {
            self.diags.push(
                Diagnostic::error("E002", span, "`<const>` is only allowed at the top level")
                    .with_note("it becomes a `!define`, and the preprocessor has no scopes (§7-1)"),
            );
            return None;
        }

        let names: Vec<&TokenReference> = local.names().iter().collect();
        let expressions: Vec<&lua::Expression> = local.expressions().iter().collect();

        let ([name], [value]) = (names.as_slice(), expressions.as_slice()) else {
            self.diags.push(
                Diagnostic::error("E004", span, "`local` binds exactly one name to one value")
                    .with_note("multiple assignment needs a calling convention (§3)"),
            );
            return None;
        };

        if local.attributes().flatten().next().is_some() {
            self.diags.push(Diagnostic::error(
                "E002",
                span,
                "`<close>` is not supported",
            ));
            return None;
        }

        Some(Local {
            name: name.token().to_string(),
            value: self.expr(value)?,
            span,
        })
    }

    /// `elseif` is desugared into a nested `if` in the else branch, so lowering
    /// and the IR only ever see the two-armed form.
    fn if_stmt(&mut self, if_stmt: &lua::If) -> Option<If> {
        let span = node_span(if_stmt);

        let mut else_body = match if_stmt.else_block() {
            Some(block) => self.block(block),
            None => Vec::new(),
        };

        if let Some(else_ifs) = if_stmt.else_if() {
            for else_if in else_ifs.iter().rev() {
                let span = node_span(else_if);
                else_body = vec![Stmt::If(If {
                    cond: self.condition(else_if.condition())?,
                    then_body: self.block(else_if.block()),
                    else_body,
                    span,
                })];
            }
        }

        Some(If {
            cond: self.condition(if_stmt.condition())?,
            then_body: self.block(if_stmt.block()),
            else_body,
            span,
        })
    }

    fn condition(&mut self, expression: &lua::Expression) -> Option<Condition> {
        let span = node_span(expression);

        if let lua::Expression::Parentheses { expression, .. } = expression {
            return self.condition(expression);
        }

        let lua::Expression::BinaryOperator { lhs, binop, rhs } = expression else {
            return self.unsupported_condition(span);
        };

        let op = match binop {
            lua::BinOp::TwoEqual(_) => CmpOp::Eq,
            lua::BinOp::TildeEqual(_) => CmpOp::Ne,
            lua::BinOp::LessThan(_) => CmpOp::Lt,
            lua::BinOp::LessThanEqual(_) => CmpOp::Le,
            lua::BinOp::GreaterThan(_) => CmpOp::Gt,
            lua::BinOp::GreaterThanEqual(_) => CmpOp::Ge,
            lua::BinOp::And(_) | lua::BinOp::Or(_) => {
                self.diags.push(
                    Diagnostic::error("E002", span, "`and` and `or` are not supported yet")
                        .with_note("short-circuiting needs the block graph in §8; use nested `if`"),
                );
                return None;
            }
            _ => return self.unsupported_condition(span),
        };

        Some(Condition {
            op,
            lhs: self.expr(lhs)?,
            rhs: self.expr(rhs)?,
            span,
        })
    }

    fn unsupported_condition<T>(&mut self, span: Span) -> Option<T> {
        self.diags.push(
            Diagnostic::error("E002", span, "an `if` condition must be a comparison").with_note(
                "there are no booleans to test: a condition lowers into the branch targets of `IntCmp` (§8)",
            ),
        );
        None
    }

    fn expr(&mut self, expression: &lua::Expression) -> Option<Expr> {
        let span = node_span(expression);

        match expression {
            lua::Expression::Number(_) => self.integer(expression).map(|v| Expr::Number(v, span)),
            lua::Expression::String(token) => {
                string_value(token, self.diags).map(|s| Expr::Str(s, span))
            }
            lua::Expression::Var(lua::Var::Name(name)) => {
                let name = name.token().to_string();
                // A `<const>` shadows nothing: it is build-time text, and the
                // register file never hears about it (§7-1).
                match self.consts.get(&name) {
                    Some(def) => Some(Expr::Const {
                        text: def.text.clone(),
                        ty: def.ty,
                        span,
                    }),
                    None => Some(Expr::Local(name, span)),
                }
            }
            lua::Expression::Parentheses { expression, .. } => self.expr(expression),
            lua::Expression::FunctionCall(call) => self.call_expr(call, span),
            lua::Expression::BinaryOperator { lhs, binop, rhs } => {
                if matches!(binop, lua::BinOp::TwoDots(_)) {
                    let mut parts = Vec::new();
                    self.concat_parts(lhs, &mut parts);
                    self.concat_parts(rhs, &mut parts);
                    return Some(Expr::Concat(parts, span));
                }
                let op = self.binary_op(binop, span)?;
                Some(Expr::Binary {
                    op,
                    lhs: Box::new(self.expr(lhs)?),
                    rhs: Box::new(self.expr(rhs)?),
                    span,
                })
            }
            _ => {
                self.diags
                    .push(Diagnostic::error("E005", span, "unsupported expression"));
                None
            }
        }
    }

    /// `..` is left-associative, so `a .. b .. c` arrives as a tree; flatten it,
    /// because the lowering wants one template rather than two.
    fn concat_parts(&mut self, expression: &lua::Expression, parts: &mut Vec<Expr>) {
        if let lua::Expression::BinaryOperator { lhs, binop, rhs } = expression
            && matches!(binop, lua::BinOp::TwoDots(_))
        {
            self.concat_parts(lhs, parts);
            self.concat_parts(rhs, parts);
            return;
        }
        if let Some(expr) = self.expr(expression) {
            parts.push(expr);
        }
    }

    fn binary_op(&mut self, binop: &lua::BinOp, span: Span) -> Option<BinOp> {
        match binop {
            lua::BinOp::Plus(_) => Some(BinOp::Add),
            lua::BinOp::Minus(_) => Some(BinOp::Sub),
            lua::BinOp::Star(_) => Some(BinOp::Mul),
            // §4: Lua's `//` is NSIS's `/`, and Lua's `%` is NSIS's `%` with the
            // opposite sign rule on negatives. Both are accepted; the rounding
            // difference is a documentation problem, not a silent remap.
            lua::BinOp::DoubleSlash(_) => Some(BinOp::Div),
            lua::BinOp::Percent(_) => Some(BinOp::Mod),
            // §4/§10-7: `/` is float division in Lua but integer division in
            // NSIS, and `^` is exponentiation in Lua but XOR in NSIS. Both mean
            // something different than they say, so neither is accepted.
            lua::BinOp::Slash(_) | lua::BinOp::Caret(_) => {
                self.diags.push(
                    Diagnostic::error(
                        "E002",
                        span,
                        format!("`{}` is not supported", binop.token().token()),
                    )
                    .with_note(match binop {
                        lua::BinOp::Slash(_) => "NSIS has no float division; use `//`",
                        _ => "NSIS's `^` is xor, not exponentiation, and there is no exponent operator",
                    }),
                );
                None
            }
            _ => {
                self.diags.push(Diagnostic::error(
                    "E002",
                    span,
                    format!("unsupported operator `{}`", binop.token().token()),
                ));
                None
            }
        }
    }

    /// A call used for its value: only a macro with an output variable
    /// qualifies. Everything else has nothing to return (§3, §10-3).
    fn call_expr(&mut self, call: &lua::FunctionCall, span: Span) -> Option<Expr> {
        let target = self.call_target(call)?;
        let Target::Method {
            object,
            method,
            args,
        } = target
        else {
            self.diags.push(
                Diagnostic::error("E005", span, "this call does not produce a value")
                    .with_note("functions and plugins have no return values yet (§3, §10-3)"),
            );
            return None;
        };

        let Some((header, mac)) = self.resolve_macro(&object, &method, span)? else {
            self.diags.push(
                Diagnostic::error("E005", span, "this call does not produce a value")
                    .with_note("only macros from an `import`ed header have an output"),
            );
            return None;
        };

        let call = self.macro_call(header, mac, &object, &method, args, span)?;
        if call.mac.output.is_none() {
            self.diags.push(Diagnostic::error(
                "E005",
                span,
                format!("`{object}.{method}` has no output variable"),
            ));
            return None;
        }

        Some(Expr::Macro(Box::new(call)))
    }

    /// A call used as a statement: an instruction, a user function, a header
    /// macro or a plugin method, in that resolution order.
    fn call_stmt(&mut self, call: &lua::FunctionCall) -> Option<Stmt> {
        let span = node_span(call);

        match self.call_target(call)? {
            Target::Plain { name, args } => {
                if name == "messageBox" {
                    return self.message_box(args, span).map(Stmt::MessageBox);
                }
                if let Some(instruction) = overlay::lookup(&name) {
                    return self
                        .instruction(instruction, &name, args, span)
                        .map(Stmt::Call);
                }
                if self.functions.contains(&name) {
                    if !matches!(args, lua::FunctionArgs::Parentheses { arguments, .. }
                        if arguments.iter().next().is_none())
                    {
                        self.diags.push(Diagnostic::error(
                            "E004",
                            span,
                            format!("`{name}` takes no arguments"),
                        ));
                        return None;
                    }
                    return Some(Stmt::CallFunction { name, span });
                }

                let mut diagnostic =
                    Diagnostic::error("E003", span, format!("unknown instruction `{name}`"));
                if let Some(similar) = overlay::lookup_ignore_case(&name) {
                    diagnostic = diagnostic.with_note(format!("did you mean `{}`?", similar.luis));
                }
                self.diags.push(diagnostic);
                None
            }
            Target::Method {
                object,
                method,
                args,
            } => {
                if let Some(pair) = self.resolve_macro(&object, &method, span)? {
                    return self
                        .macro_call(pair.0, pair.1, &object, &method, args, span)
                        .map(Stmt::Macro);
                }
                if let Some(plugin) = self.plugins.get(&object).cloned() {
                    return self
                        .plugin_call(plugin, &method, args, span)
                        .map(Stmt::Plugin);
                }
                self.diags.push(Diagnostic::error(
                    "E003",
                    span,
                    format!("`{object}` is not an `import` or a `plugin`"),
                ));
                None
            }
        }
    }

    /// `object.method` is a macro when `object` is an import, and also when it
    /// is an adapted stdlib namespace — `string.upper` is `${StrCase}` wearing
    /// a Lua name (§5).
    ///
    /// The outer `Option` is "a diagnostic was raised"; the inner one is "this
    /// object is not a macro namespace at all".
    fn resolve_macro(
        &mut self,
        object: &str,
        method: &str,
        span: Span,
    ) -> Option<Option<(&'static overlay::Header, &'static overlay::Macro)>> {
        if let Some(header) = self.imports.get(object).map(|i| i.header) {
            let Some(mac) = header.macro_named(method) else {
                let known: Vec<&str> = header.macros.iter().map(|m| m.luis).collect();
                self.diags.push(
                    Diagnostic::error(
                        "E003",
                        span,
                        format!("`{}` has no macro `{method}`", header.luis),
                    )
                    .with_note(format!("known macros: {}", known.join(", "))),
                );
                return None;
            };
            return Some(Some((header, mac)));
        }

        if overlay::is_stdlib_object(object) {
            let Some(pair) = overlay::stdlib(object, method) else {
                let known: Vec<&str> = overlay::STDLIB
                    .iter()
                    .filter(|f| f.object == object)
                    .map(|f| f.name)
                    .collect();
                self.diags.push(
                    Diagnostic::error(
                        "E003",
                        span,
                        format!("`{object}.{method}` has no NSIS lowering"),
                    )
                    .with_note(format!("adapted from `{object}`: {}", known.join(", "))),
                );
                return None;
            };
            return Some(Some(pair));
        }

        Some(None)
    }

    fn instruction(
        &mut self,
        instruction: &'static overlay::Instruction,
        name: &str,
        args: &lua::FunctionArgs,
        span: Span,
    ) -> Option<Call> {
        let arguments = self.arguments(args, name, span)?;
        if arguments.len() != instruction.arity {
            self.diags.push(Diagnostic::error(
                "E004",
                span,
                format!(
                    "`{name}` expects {} argument(s), got {}",
                    instruction.arity,
                    arguments.len()
                ),
            ));
            return None;
        }

        Some(Call {
            name: name.to_string(),
            args: arguments,
            span,
        })
    }

    fn macro_call(
        &mut self,
        header: &'static overlay::Header,
        mac: &'static overlay::Macro,
        object: &str,
        method: &str,
        args: &lua::FunctionArgs,
        span: Span,
    ) -> Option<MacroCall> {
        let arguments = self.arguments(args, &format!("{object}.{method}"), span)?;
        if arguments.len() != mac.inputs {
            self.diags.push(Diagnostic::error(
                "E004",
                span,
                format!(
                    "`{object}.{method}` expects {} argument(s), got {}",
                    mac.inputs,
                    arguments.len()
                ),
            ));
            return None;
        }

        self.used_headers.insert(header.luis);
        if let Some(init) = mac.init
            && !self.inits.contains(&init)
        {
            self.inits.push(init);
        }

        Some(MacroCall {
            header,
            mac,
            args: arguments,
            span,
        })
    }

    fn plugin_call(
        &mut self,
        plugin: String,
        method: &str,
        args: &lua::FunctionArgs,
        span: Span,
    ) -> Option<PluginCall> {
        let arguments = self.arguments(args, method, span)?;
        Some(PluginCall {
            plugin,
            // Plugin exports are PascalCase by universal convention, so the
            // camelCase rule (§6) derives the spelling instead of tabulating it.
            method: pascal_case(method),
            args: arguments,
            span,
        })
    }

    /// `messageBox { … }` (§13). Buttons and icon are two independent flag
    /// groups sharing one argument; `onYes`/`onNo`/… are the jump table, and
    /// they are checked against the button set rather than accepted blindly.
    fn message_box(&mut self, args: &lua::FunctionArgs, span: Span) -> Option<MessageBox> {
        let lua::FunctionArgs::TableConstructor(table) = args else {
            self.diags.push(Diagnostic::error(
                "E004",
                span,
                "`messageBox` takes a single table, as in `messageBox { text = \"…\" }`",
            ));
            return None;
        };

        let mut text = None;
        let mut buttons_name: Option<(String, Span)> = None;
        let mut icon_name: Option<(String, Span)> = None;
        let mut default_name: Option<(String, Span)> = None;
        let mut handler_bodies: Vec<(String, &lua::FunctionBody, Span)> = Vec::new();

        for field in table.fields() {
            let field_span = node_span(field);
            let lua::Field::NameKey { key, value, .. } = field else {
                self.diags.push(Diagnostic::error(
                    "E002",
                    field_span,
                    "only `name = value` fields are supported in `messageBox`",
                ));
                continue;
            };

            let key_name = key.token().to_string();
            match key_name.as_str() {
                "text" => text = self.expr(value),
                "buttons" => {
                    if let Some(value) = const_string_in(value, self.diags) {
                        buttons_name = Some((value, field_span));
                    }
                }
                "icon" => {
                    if let Some(value) = const_string_in(value, self.diags) {
                        icon_name = Some((value, field_span));
                    }
                }
                "default" => {
                    if let Some(value) = const_string_in(value, self.diags) {
                        default_name = Some((value, field_span));
                    }
                }
                _ if key_name.starts_with("on") => {
                    let lua::Expression::Function(function) = value else {
                        self.diags.push(Diagnostic::error(
                            "E002",
                            field_span,
                            format!("`{key_name}` must be an anonymous function"),
                        ));
                        continue;
                    };
                    handler_bodies.push((key_name, function.body(), field_span));
                }
                _ => {
                    let known: Vec<&str> = overlay::MESSAGE_BOX_FIELDS.to_vec();
                    self.diags.push(
                        Diagnostic::error(
                            "E003",
                            field_span,
                            format!("unknown `messageBox` field `{key_name}`"),
                        )
                        .with_note(format!(
                            "supported fields: {}, plus one handler per button",
                            known.join(", ")
                        )),
                    );
                }
            }
        }

        let Some(text) = text else {
            self.diags.push(Diagnostic::error(
                "E004",
                span,
                "`messageBox` needs a `text` field",
            ));
            return None;
        };

        let buttons = match &buttons_name {
            Some((name, name_span)) => {
                let Some(set) = overlay::button_set(name) else {
                    let known: Vec<&str> = overlay::BUTTON_SETS.iter().map(|s| s.luis).collect();
                    self.diags.push(
                        Diagnostic::error(
                            "E003",
                            *name_span,
                            format!("unknown button set `{name}`"),
                        )
                        .with_note(format!("known sets: {}", known.join(", "))),
                    );
                    return None;
                };
                set
            }
            // NSIS's own default, spelled out rather than left implicit.
            None => overlay::button_set("OK").expect("OK is in the overlay"),
        };

        let icon = match &icon_name {
            Some((name, name_span)) => {
                let Some(icon) = overlay::icon(name) else {
                    let known: Vec<&str> = overlay::ICONS.iter().map(|i| i.luis).collect();
                    self.diags.push(
                        Diagnostic::error("E003", *name_span, format!("unknown icon `{name}`"))
                            .with_note(format!("known icons: {}", known.join(", "))),
                    );
                    return None;
                };
                Some(icon)
            }
            None => None,
        };

        let default = match &default_name {
            Some((name, name_span)) => {
                let Some(button) = buttons.button_by_id(name) else {
                    let known: Vec<&str> = buttons.buttons.iter().map(|b| &b.id[2..]).collect();
                    self.diags.push(
                        Diagnostic::error(
                            "E003",
                            *name_span,
                            format!("`{name}` is not a button of `{}`", buttons.luis),
                        )
                        .with_note(format!("`default` must be one of: {}", known.join(", "))),
                    );
                    return None;
                };
                Some(button)
            }
            None => None,
        };

        // Handlers are emitted in button order, not source order, so the jump
        // table and the generated labels stay deterministic.
        let mut handlers = Vec::new();
        for (name, body, field_span) in &handler_bodies {
            let Some(button) = buttons.button(name) else {
                let known: Vec<&str> = buttons.buttons.iter().map(|b| b.handler).collect();
                self.diags.push(
                    Diagnostic::error(
                        "E003",
                        *field_span,
                        format!("`{name}` is not a button of `{}`", buttons.luis),
                    )
                    .with_note(format!(
                        "`{}` has: {}",
                        buttons.luis,
                        known.join(", ")
                    )),
                );
                continue;
            };
            handlers.push(Handler {
                button,
                body: self.block(body.block()),
                span: *field_span,
            });
        }
        handlers.sort_by_key(|handler| {
            buttons
                .buttons
                .iter()
                .position(|b| b.handler == handler.button.handler)
                .unwrap_or(usize::MAX)
        });

        Some(MessageBox {
            buttons,
            icon,
            text,
            default,
            handlers,
            span,
        })
    }

    fn arguments(&mut self, args: &lua::FunctionArgs, name: &str, span: Span) -> Option<Vec<Expr>> {
        let lua::FunctionArgs::Parentheses { arguments, .. } = args else {
            self.diags.push(Diagnostic::error(
                "E004",
                span,
                format!("`{name}` must be called with parentheses"),
            ));
            return None;
        };

        let mut values = Vec::new();
        for argument in arguments {
            values.push(self.expr(argument)?);
        }
        Some(values)
    }

    /// Accepts `foo(...)` and `obj.method(...)`. Method calls with `:`, chained
    /// calls and bracket indexing are out of the subset.
    fn call_target<'a>(&mut self, call: &'a lua::FunctionCall) -> Option<Target<'a>> {
        let span = node_span(call);

        let lua::Prefix::Name(name) = call.prefix() else {
            self.diags.push(Diagnostic::error(
                "E002",
                span,
                "only calls of a plain name are supported",
            ));
            return None;
        };
        let name = name.token().to_string();

        let suffixes: Vec<&lua::Suffix> = call.suffixes().collect();
        match suffixes.as_slice() {
            [lua::Suffix::Call(lua::Call::AnonymousCall(args))] => {
                Some(Target::Plain { name, args })
            }
            [
                lua::Suffix::Index(lua::Index::Dot { name: method, .. }),
                lua::Suffix::Call(lua::Call::AnonymousCall(args)),
            ] => Some(Target::Method {
                object: name,
                method: method.token().to_string(),
                args,
            }),
            _ => {
                self.diags.push(
                    Diagnostic::error("E002", span, "unsupported call form")
                        .with_note("supported: `name(...)` and `object.method(...)`"),
                );
                None
            }
        }
    }
}

struct Import {
    header: &'static overlay::Header,
    span: Span,
}

struct Binding {
    name: String,
    kind: &'static str,
    /// `None` when the argument was not a constant string; `binding` diagnoses.
    argument: Option<String>,
}

fn is_const(local: &lua::LocalAssignment) -> bool {
    local.attributes().flatten().next().is_some()
}

/// Recognizes `local x = import "…"` / `local x = plugin "…"` without
/// diagnosing anything, so both passes can ask the same question. Both the
/// `f "x"` sugar and `f("x")` are accepted, as they are the same call in Lua.
fn binding_shape(local: &lua::LocalAssignment) -> Option<Binding> {
    let names: Vec<&TokenReference> = local.names().iter().collect();
    let expressions: Vec<&lua::Expression> = local.expressions().iter().collect();
    let ([name], [lua::Expression::FunctionCall(call)]) =
        (names.as_slice(), expressions.as_slice())
    else {
        return None;
    };

    let lua::Prefix::Name(callee) = call.prefix() else {
        return None;
    };
    let kind = match callee.token().to_string().as_str() {
        "import" => "import",
        "plugin" => "plugin",
        _ => return None,
    };

    let suffixes: Vec<&lua::Suffix> = call.suffixes().collect();
    let [lua::Suffix::Call(lua::Call::AnonymousCall(args))] = suffixes.as_slice() else {
        return None;
    };

    let argument = match args {
        lua::FunctionArgs::String(token) => string_literal(token),
        lua::FunctionArgs::Parentheses { arguments, .. } => match arguments.iter().next() {
            Some(lua::Expression::String(token)) => string_literal(token),
            _ => None,
        },
        _ => None,
    };

    Some(Binding {
        name: name.token().to_string(),
        kind,
        argument,
    })
}

fn string_literal(token: &TokenReference) -> Option<String> {
    match token.token().token_type() {
        TokenType::StringLiteral { literal, .. } => Some(literal.to_string()),
        _ => None,
    }
}

fn pascal_case(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Where only a literal will do: section names, enum members, flag values.
fn const_string_in(expression: &lua::Expression, diags: &mut Diagnostics) -> Option<String> {
    let span = node_span(expression);
    let lua::Expression::String(token) = expression else {
        diags.push(Diagnostic::error(
            "E005",
            span,
            "expected a constant string",
        ));
        return None;
    };
    string_value(token, diags)
}

fn string_value(token: &TokenReference, diags: &mut Diagnostics) -> Option<String> {
    string_literal(token).or_else(|| {
        diags.push(Diagnostic::error(
            "E005",
            node_span(token),
            "expected a constant string",
        ));
        None
    })
}

fn node_span(node: &impl Node) -> Span {
    node.start_position().map(position_span).unwrap_or_default()
}

fn position_span(position: full_moon::tokenizer::Position) -> Span {
    Span::new(position.line(), position.character())
}
