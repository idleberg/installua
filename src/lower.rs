//! The thin spine: AST → IR, for `attributes {}`, one `section`, and
//! `detailPrint`.
//!
//! This is deliberately the narrowest end-to-end path that produces something
//! `makensis` will assemble (PLAN Phase 1). It exists so that `makensis -WX` is
//! a live gate from week one instead of from Phase 4 — the longest blind
//! stretch in the plan is the one between a specification and its first
//! assembled output, and this closes it.
//!
//! What it is *not* is a first draft of Phase 2. There is no name resolution,
//! no type lattice and no CFG here, and adding them by accretion is exactly the
//! shape §9-2 warns about. Everything outside the spine is
//! [`Code::NotYetImplemented`] — the honest edge of the vertical slice, and
//! distinct from [`Code::UnknownField`], which means *no version will ever
//! accept this*.

use crate::ast::*;
use crate::diag::{Code, Diagnostic, Diagnostics, Span};
use crate::ir;

/// The frozen v1 attribute surface (Phase 0). A name in here is scheduled; a
/// name outside it is a typo. Telling the user which of the two they hit is the
/// difference between a five-second fix and a search through the NSIS docs.
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

pub fn lower(program: &Program, diags: &mut Diagnostics) -> ir::Module {
    let mut lowerer = Lowerer {
        diags,
        module: ir::Module::new(),
        attributes_span: None,
        installer_span: None,
    };
    lowerer.program(program);
    lowerer.finish()
}

struct Lowerer<'a> {
    diags: &'a mut Diagnostics,
    module: ir::Module,
    attributes_span: Option<Span>,
    installer_span: Option<Span>,
}

impl Lowerer<'_> {
    fn program(&mut self, program: &Program) {
        for stmt in &program.block {
            self.top_level(stmt);
        }
    }

    fn finish(self) -> ir::Module {
        self.module
    }

    fn top_level(&mut self, stmt: &Stmt) {
        let Stmt::Call(call) = stmt else {
            self.todo(stmt.span(), "this declaration");
            return;
        };
        let (name, args) = match self.block_call(call) {
            Some(parts) => parts,
            None => return,
        };

        match name {
            "attributes" => {
                if let Some(previous) = self.attributes_span {
                    self.diags.push(
                        Diagnostic::error(
                            Code::DuplicateBlock,
                            call.span(),
                            "`attributes {}` appears more than once",
                        )
                        .note(format!("the first one is at line {}", previous.start_line))
                        .note("it is script-global, so there is exactly one (§2)"),
                    );
                    return;
                }
                self.attributes_span = Some(call.span());
                self.attributes(args, call.span());
            }
            "installer" => {
                if let Some(previous) = self.installer_span {
                    self.diags.push(
                        Diagnostic::error(
                            Code::DuplicateBlock,
                            call.span(),
                            "`installer {}` appears more than once",
                        )
                        .note(format!("the first one is at line {}", previous.start_line)),
                    );
                    return;
                }
                self.installer_span = Some(call.span());
                self.installer(args, call.span());
            }
            other if V1_BLOCKS.contains(&other) => {
                self.todo(call.span(), &format!("`{other}`"));
            }
            other => {
                self.diags.push(
                    Diagnostic::error(
                        Code::UnknownField,
                        call.span(),
                        format!("`{other}` is not a declaration"),
                    )
                    .note(format!("the declarations are {}", list(V1_BLOCKS))),
                );
            }
        }
    }

    /// Both `attributes { … }` and `attributes({ … })` are the same call in
    /// Lua, so both arrive here as one table argument.
    fn block_call<'e>(&mut self, call: &'e Expr) -> Option<(&'e str, &'e [TableField])> {
        let Expr::Call { callee, args, .. } = call else {
            self.todo(call.span(), "this statement");
            return None;
        };
        let Expr::Name(name) = callee.as_ref() else {
            self.todo(call.span(), "this call");
            return None;
        };
        match args.as_slice() {
            [Expr::Table { fields, .. }] => Some((name.text.as_str(), fields.as_slice())),
            _ => {
                self.todo(call.span(), &format!("`{}` in this form", name.text));
                None
            }
        }
    }

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
                "unicode" => match value {
                    Expr::Bool { value, .. } => self.module.unicode = *value,
                    other => self.bad_value(
                        other.span(),
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

    fn installer(&mut self, fields: &[TableField], span: Span) {
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

        let _ = span;
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

        // The spine is `section(name, body)`. The three-argument form carries
        // `{ optional = true }`, which is Phase 5's overlay work.
        let [name, Expr::Function { block, .. }] = args.as_slice() else {
            self.todo(value.span(), "this `section` form");
            return None;
        };
        let name = self.constant_string(name, "section")?;

        Some(ir::Section {
            name,
            optional: false,
            body: self.body(block),
        })
    }

    fn body(&mut self, block: &Block) -> Vec<ir::Item> {
        let mut items = Vec::new();
        for stmt in block {
            let Stmt::Call(call) = stmt else {
                self.todo(stmt.span(), "this statement");
                continue;
            };
            let Expr::Call { callee, args, .. } = call else {
                self.todo(call.span(), "this statement");
                continue;
            };
            let Expr::Name(callee) = callee.as_ref() else {
                self.todo(call.span(), "this call");
                continue;
            };
            if callee.text != "detailPrint" {
                self.todo(call.span(), &format!("`{}`", callee.text));
                continue;
            }
            let [message] = args.as_slice() else {
                self.bad_value(
                    call.span(),
                    "detailPrint",
                    "exactly one argument",
                    "join the parts with `..`: concatenation is a string template and \
                     usually costs no instruction at all (§5)",
                );
                continue;
            };
            let Some(message) = self.constant_string(message, "detailPrint") else {
                continue;
            };
            items.push(ir::Item::Instruction(ir::Instruction::new(
                "DetailPrint",
                vec![ir::Arg::str(message)],
            )));
        }
        items
    }

    /// Constant folding over the spine's expression set: a literal, an integer,
    /// or a `..` chain of them. Anything with a runtime value is Phase 2.
    fn constant_string(&mut self, expr: &Expr, what: &str) -> Option<String> {
        match expr {
            Expr::Str(literal) => Some(literal.value.clone()),
            Expr::Number { value, .. } => Some(value.to_string()),
            Expr::Binary {
                op: BinOp::Concat,
                lhs,
                rhs,
                ..
            } => {
                let lhs = self.constant_string(lhs, what);
                let rhs = self.constant_string(rhs, what);
                Some(format!("{}{}", lhs?, rhs?))
            }
            other => {
                self.todo(other.span(), &format!("a non-constant value for `{what}`"));
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
        self.diags.push(
            Diagnostic::error(
                Code::NotYetImplemented,
                span,
                format!("{what} is not in this version's exposed set"),
            )
            .note(
                "it is scheduled rather than missing: `installua coverage` counts it \
                 (PLAN §0)",
            ),
        );
    }
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
