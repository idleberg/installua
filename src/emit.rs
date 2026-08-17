//! IR → `.nsi` text, and the sidecar that says where each line came from.
//!
//! The emitter decides layout and quoting and nothing else. In particular it
//! does not decide what a label is — [`crate::layout`] has already turned each
//! body's CFG into a flat list by the time anything here runs.
//!
//! Every line is written through [`Out::line`], which takes its origin
//! alongside its text. That is the whole of §15.22's collection half: a line
//! and its provenance are produced together, so there is no second pass that
//! could disagree with the first about how many lines there are.

use crate::ir;
use crate::layout;
use crate::map::{LineMap, Origin};

const INDENT: &str = "  ";

/// The output and its map, built together and never separately.
#[derive(Default)]
struct Out {
    text: String,
    map: LineMap,
}

impl Out {
    fn line(&mut self, text: impl AsRef<str>, origin: Origin) {
        self.text.push_str(text.as_ref());
        self.text.push('\n');
        self.map.push(origin);
    }

    /// A blank separator. It is a line too — `makensis` counts it — so it goes
    /// through the same door as everything else.
    fn blank(&mut self) {
        if !self.text.is_empty() {
            self.line("", Origin::Emitted("blank"));
        }
    }

    /// A run of lines the compiler emitted on its own behalf, preceded by a
    /// blank when there is anything in it.
    fn section(&mut self, what: &'static str, lines: impl Iterator<Item = String>) {
        let mut first = true;
        for text in lines {
            if first {
                self.blank();
                first = false;
            }
            self.line(text, Origin::Emitted(what));
        }
    }
}

pub fn emit(module: &ir::Module) -> String {
    emit_mapped(module).0
}

/// The `.nsi` and its line map (§15.22).
pub fn emit_mapped(module: &ir::Module) -> (String, LineMap) {
    let mut out = Out::default();

    // 1. `Unicode` leads. A later `raw` then overrides it, rather than being
    //    silently overridden by a `Unicode` the compiler emitted afterwards
    //    (§15.16) — last one wins in NSIS, with no diagnostic either way.
    out.line(
        format!("Unicode {}", boolean(module.unicode)),
        Origin::Emitted("Unicode"),
    );

    // 2. `!define`s, in source order.
    out.section("!define", module.defines.iter().map(define_line));

    // 3. `!include`s.
    out.section(
        "!include",
        module
            .includes
            .iter()
            .map(|header| format!("!include {}", argument(&ir::Arg::str(header.clone())))),
    );

    // 4. Header init lines.
    out.section("StrFunc init", module.inits.iter().map(line));

    // 5. Attributes, in overlay order.
    out.section("attribute", module.attributes.iter().map(line));

    // 6. MUI defines, page macros, `MUI_LANGUAGE`.
    out.section("MUI define", module.mui_defines.iter().map(define_line));
    out.section("page", module.pages.iter().map(line));
    out.section("page", module.unpages.iter().map(line));
    out.section("language", module.languages.iter().map(line));

    // 7. `Var`s. A `Var` used before it is declared is a hard error in NSIS,
    //    unlike a `Function`, which is why these are collected rather than
    //    emitted where they were written (§12).
    out.section("Var", module.vars.iter().map(|name| format!("Var {name}")));

    // 8. Functions. NSIS hoists calls, so these could go anywhere; before the
    //    sections is a readability choice, and that is a sufficient one (§9-6).
    for function in &module.functions {
        out.blank();
        out.line(
            format!("Function {}", function.name),
            Origin::Emitted("Function"),
        );
        body(&mut out, &function.body, 1);
        out.line("FunctionEnd", Origin::Emitted("FunctionEnd"));
    }

    // 9. Install types, in the order they were declared. That order is their
    //    identity — a `SectionIn` names one by position — so like the pages and
    //    unlike the attributes these are never reordered.
    out.section(
        "installTypes",
        module
            .inst_types
            .iter()
            .map(|name| format!("InstType {}", argument(&ir::Arg::str(name.clone()))))
            .chain(
                module
                    .uninst_types
                    .iter()
                    .map(|name| format!("InstType un.{}", argument(&ir::Arg::str(name.clone())))),
            ),
    );

    // 10. Sections, in source order — section order is install order, and it is
    //    user-visible, so unlike attributes these are never reordered.
    for item in &module.sections {
        out.blank();
        match item {
            ir::SectionItem::Section(section) => section_block(&mut out, section, 0),
            ir::SectionItem::Group(group) => {
                let flag = if group.expanded { " /e" } else { "" };
                let name = argument(&ir::Arg::str(group.name.clone()));
                out.line(
                    format!("SectionGroup{flag} {name}"),
                    Origin::Emitted("SectionGroup"),
                );
                for (index, section) in group.sections.iter().enumerate() {
                    if index > 0 {
                        out.blank();
                    }
                    section_block(&mut out, section, 1);
                }
                out.line("SectionGroupEnd", Origin::Emitted("SectionGroupEnd"));
            }
        }
    }

    (out.text, out.map)
}

fn section_block(out: &mut Out, section: &ir::Section, depth: usize) {
    let indent = INDENT.repeat(depth);
    let flag = if section.optional { " /o" } else { "" };
    let name = argument(&ir::Arg::str(section.name.clone()));
    out.line(
        format!("{indent}Section{flag} {name}"),
        Origin::Emitted("Section"),
    );
    // `SectionIn` and `AddSize` are declarations that NSIS spells as
    // instructions: they read as the first two lines of the body and are not
    // executed, which is why they are options on the surface and are emitted
    // here rather than lowered into the CFG.
    if !section.inst_types.is_empty() || section.required {
        let mut words: Vec<String> = section.inst_types.iter().map(usize::to_string).collect();
        if section.required {
            words.push("RO".to_string());
        }
        out.line(
            format!("{indent}{INDENT}SectionIn {}", words.join(" ")),
            Origin::Emitted("SectionIn"),
        );
    }
    if let Some(size) = section.size {
        out.line(
            format!("{indent}{INDENT}AddSize {size}"),
            Origin::Emitted("AddSize"),
        );
    }
    body(out, &section.body, depth + 1);
    out.line(format!("{indent}SectionEnd"), Origin::Emitted("SectionEnd"));
}

fn body(out: &mut Out, body: &crate::cfg::Body, depth: usize) {
    for (item, origin) in layout::lay_out(body) {
        match item {
            ir::Item::Instruction(instruction) => {
                out.line(
                    format!("{}{}", INDENT.repeat(depth), line(&instruction)),
                    origin,
                );
            }
            // Labels sit one level out from the code they head, as they do in
            // hand-written NSIS — the output is the only debugger anyone has
            // (§9-6).
            ir::Item::Label(label) => {
                out.line(
                    format!("{}{label}:", INDENT.repeat(depth.saturating_sub(1))),
                    origin,
                );
            }
        }
    }
}

fn define_line(define: &ir::Define) -> String {
    format!("!define {} {}", define.name, argument(&define.value))
}

fn line(instruction: &ir::Instruction) -> String {
    let mut out = instruction.name.clone();
    for arg in &instruction.args {
        out.push(' ');
        out.push_str(&argument(arg));
    }
    out
}

/// NSIS quoting for **data** (§12, §15.1).
///
/// A literal is data and never a template, so every `$` in a text piece is
/// doubled — which is what keeps an unknown `${NOPE}` from reaching the output
/// and shipping as warning 6000 plus a silently wrong installer. The *only*
/// unescaped `$` in the output comes from a [`ir::Piece::Var`], which the
/// compiler put there.
fn argument(arg: &ir::Arg) -> String {
    match arg {
        ir::Arg::Raw(value) => value.clone(),
        // A destination is syntax: a register name and nothing else.
        ir::Arg::Dest(slot) => slot.nsis(),
        // Three shapes need no quotes at all, and leaving them off is the
        // difference between output a human reads and output they flinch at
        // (§9-6). All are provably safe: NSIS parses `$0` and `"$0"`
        // identically, and an integer has nothing in it to quote.
        ir::Arg::Data { pieces, path }
            if matches!(
                pieces.as_slice(),
                [ir::Piece::Var(_)] | [ir::Piece::Slot(_)]
            ) || arg.as_text().is_some_and(|text| is_integer(&text)) =>
        {
            let mut out = String::new();
            for piece in pieces {
                push_piece(&mut out, piece, *path);
            }
            out
        }

        ir::Arg::Data { pieces, path } => {
            let mut out = String::from("\"");
            for piece in pieces {
                push_piece(&mut out, piece, *path);
            }
            out.push('"');
            out
        }
    }
}

/// One piece, into whatever the caller is building. `path` normalises `/` in
/// **text** only: what a register holds is runtime data, and a `${…}` is
/// substituted after this compiler has stopped looking (§5).
fn push_piece(out: &mut String, piece: &ir::Piece, path: bool) {
    match piece {
        ir::Piece::Text(text) => {
            let text = if path {
                text.replace('/', "\\")
            } else {
                text.clone()
            };
            escape_into(out, &text);
        }
        ir::Piece::Var(var) => out.push_str(var),
        ir::Piece::Slot(slot) => out.push_str(&slot.nsis()),
        ir::Piece::Const { name, .. } => out.push_str(&format!("${{{name}}}")),
    }
}

fn is_integer(text: &str) -> bool {
    let digits = text.strip_prefix('-').unwrap_or(text);
    !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
}

fn escape_into(out: &mut String, value: &str) {
    for c in value.chars() {
        match c {
            '$' => out.push_str("$$"),
            '"' => out.push_str("$\\\""),
            '\n' => out.push_str("$\\n"),
            '\r' => out.push_str("$\\r"),
            '\t' => out.push_str("$\\t"),
            other => out.push(other),
        }
    }
}

fn boolean(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}
