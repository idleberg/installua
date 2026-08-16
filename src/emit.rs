//! IR → `.nsi` text. Deterministic, because §14 diffs goldens.
//!
//! The emitter decides layout and quoting and nothing else. In particular it
//! does not decide what a label is — [`crate::layout`] has already turned each
//! body's CFG into a flat list by the time anything here runs.

use crate::ir;
use crate::layout;

const INDENT: &str = "  ";

pub fn emit(module: &ir::Module) -> String {
    let mut out = String::new();

    // 1. `Unicode` leads. A later `raw` then overrides it, rather than being
    //    silently overridden by a `Unicode` the compiler emitted afterwards
    //    (§15.16) — last one wins in NSIS, with no diagnostic either way.
    out.push_str(&format!("Unicode {}\n", boolean(module.unicode)));

    // 2. `!define`s, in source order.
    section_of(&mut out, module.defines.iter().map(define_line));

    // 3. `!include`s.
    section_of(
        &mut out,
        module
            .includes
            .iter()
            .map(|header| format!("!include {}", argument(&ir::Arg::str(header.clone())))),
    );

    // 4. Header init lines.
    section_of(&mut out, module.inits.iter().map(line));

    // 5. Attributes, in overlay order.
    section_of(&mut out, module.attributes.iter().map(line));

    // 6. MUI defines, page macros, `MUI_LANGUAGE`.
    section_of(&mut out, module.mui.iter().map(line));

    // 7. `Var`s. A `Var` used before it is declared is a hard error in NSIS,
    //    unlike a `Function`, which is why these are collected rather than
    //    emitted where they were written (§12).
    section_of(
        &mut out,
        module.vars.iter().map(|name| format!("Var {name}")),
    );

    // 8. Functions. NSIS hoists calls, so these could go anywhere; before the
    //    sections is a readability choice, and that is a sufficient one (§9-6).
    for function in &module.functions {
        blank_line(&mut out);
        out.push_str(&format!("Function {}\n", function.name));
        body(&mut out, &function.body, 1);
        out.push_str("FunctionEnd\n");
    }

    // 9. Sections, in source order — section order is install order, and it is
    //    user-visible, so unlike attributes these are never reordered.
    for item in &module.sections {
        blank_line(&mut out);
        match item {
            ir::SectionItem::Section(section) => section_block(&mut out, section, 0),
            ir::SectionItem::Group(group) => {
                let flag = if group.expanded { " /e" } else { "" };
                let name = argument(&ir::Arg::str(group.name.clone()));
                out.push_str(&format!("SectionGroup{flag} {name}\n"));
                for (index, section) in group.sections.iter().enumerate() {
                    if index > 0 {
                        out.push('\n');
                    }
                    section_block(&mut out, section, 1);
                }
                out.push_str("SectionGroupEnd\n");
            }
        }
    }

    out
}

fn section_of(out: &mut String, lines: impl Iterator<Item = String>) {
    let mut first = true;
    for text in lines {
        if first {
            blank_line(out);
            first = false;
        }
        out.push_str(&text);
        out.push('\n');
    }
}

fn section_block(out: &mut String, section: &ir::Section, depth: usize) {
    let indent = INDENT.repeat(depth);
    let flag = if section.optional { " /o" } else { "" };
    let name = argument(&ir::Arg::str(section.name.clone()));
    out.push_str(&format!("{indent}Section{flag} {name}\n"));
    body(out, &section.body, depth + 1);
    out.push_str(&format!("{indent}SectionEnd\n"));
}

fn blank_line(out: &mut String) {
    if !out.is_empty() {
        out.push('\n');
    }
}

fn body(out: &mut String, body: &crate::cfg::Body, depth: usize) {
    for item in layout::lay_out(body) {
        match item {
            ir::Item::Instruction(instruction) => {
                out.push_str(&INDENT.repeat(depth));
                out.push_str(&line(&instruction));
            }
            // Labels sit one level out from the code they head, as they do in
            // hand-written NSIS — the output is the only debugger anyone has
            // (§9-6).
            ir::Item::Label(label) => {
                out.push_str(&INDENT.repeat(depth.saturating_sub(1)));
                out.push_str(&format!("{label}:"));
            }
        }
        out.push('\n');
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
        // Two shapes need no quotes at all, and leaving them off is the
        // difference between output a human reads and output they flinch at
        // (§9-6). Both are provably safe: NSIS parses `$0` and `"$0"`
        // identically, and an integer has nothing in it to quote.
        ir::Arg::Data { pieces, .. }
            if matches!(
                pieces.as_slice(),
                [ir::Piece::Var(_)] | [ir::Piece::Slot(_)]
            ) =>
        {
            match &pieces[0] {
                ir::Piece::Var(var) => var.clone(),
                ir::Piece::Slot(slot) => slot.nsis(),
                ir::Piece::Text(_) => unreachable!(),
            }
        }
        ir::Arg::Data { .. } if arg.as_text().is_some_and(|text| is_integer(&text)) => {
            arg.as_text().unwrap_or_default()
        }

        ir::Arg::Data { pieces, path } => {
            let mut out = String::from("\"");
            for piece in pieces {
                match piece {
                    ir::Piece::Text(text) => {
                        // `/` is normalised only in a path position, and only in
                        // text: what a register holds is runtime data (§5).
                        let text = if *path {
                            text.replace('/', "\\")
                        } else {
                            text.clone()
                        };
                        escape_into(&mut out, &text);
                    }
                    ir::Piece::Var(var) => out.push_str(var),
                    ir::Piece::Slot(slot) => out.push_str(&slot.nsis()),
                }
            }
            out.push('"');
            out
        }
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
