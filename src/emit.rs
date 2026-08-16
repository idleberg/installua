//! IR → `.nsi` text. Deterministic, because §14 diffs goldens.

use crate::ir;

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
            .map(|header| format!("!include {}", quote(header))),
    );

    // 4. Header init lines.
    section_of(&mut out, module.inits.iter().map(line));

    // 5. Attributes, in overlay order.
    section_of(&mut out, module.attributes.iter().map(line));

    // 6. MUI defines, page macros, `MUI_LANGUAGE`.
    section_of(&mut out, module.mui.iter().map(line));

    // 7. `Var`s.
    section_of(
        &mut out,
        module.vars.iter().map(|name| format!("Var {name}")),
    );

    // 8. Functions.
    for function in &module.functions {
        blank_line(&mut out);
        out.push_str(&format!("Function {}\n", function.name));
        body(&mut out, &function.body, 1);
        out.push_str("FunctionEnd\n");
    }

    // 9. Sections, in source order.
    for item in &module.sections {
        blank_line(&mut out);
        match item {
            ir::SectionItem::Section(section) => section_block(&mut out, section, 0),
            ir::SectionItem::Group(group) => {
                let flag = if group.expanded { " /e" } else { "" };
                out.push_str(&format!("SectionGroup{flag} {}\n", quote(&group.name)));
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
    out.push_str(&format!("{indent}Section{flag} {}\n", quote(&section.name)));
    body(out, &section.body, depth + 1);
    out.push_str(&format!("{indent}SectionEnd\n"));
}

fn blank_line(out: &mut String) {
    if !out.is_empty() {
        out.push('\n');
    }
}

fn body(out: &mut String, items: &[ir::Item], depth: usize) {
    for item in items {
        match item {
            ir::Item::Instruction(instruction) => {
                out.push_str(&INDENT.repeat(depth));
                out.push_str(&line(instruction));
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

fn argument(arg: &ir::Arg) -> String {
    match arg {
        ir::Arg::Str(value) => quote(value),
        ir::Arg::Path(value) => quote(&value.replace('/', "\\")),
        ir::Arg::Raw(value) => value.clone(),
    }
}

fn boolean(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

/// NSIS quoting for **data** (§12, §15.1).
///
/// `$` is doubled unconditionally, which is the whole string model: a literal is
/// data and never a template, so an unknown `${NOPE}` cannot reach the output
/// and ship as warning 6000. The remaining escapes are the four NSIS has.
fn quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
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
    out.push('"');
    out
}
