//! Phase 4: IR -> `.nsi` text. Deterministic, so golden files stay diffable.

use crate::ir;

const INDENT: &str = "  ";

pub fn emit(module: &ir::Module) -> String {
    let mut out = String::new();

    for include in &module.includes {
        out.push_str(&format!("!include {}\n", quote(include)));
    }

    if !module.inits.is_empty() {
        blank_line(&mut out);
        for init in &module.inits {
            out.push_str(&line(init));
            out.push('\n');
        }
    }

    if !module.directives.is_empty() {
        blank_line(&mut out);
        for directive in &module.directives {
            out.push_str(&line(directive));
            out.push('\n');
        }
    }

    if !module.attributes.is_empty() {
        blank_line(&mut out);
        for attribute in &module.attributes {
            out.push_str(&line(attribute));
            out.push('\n');
        }
    }

    for function in &module.functions {
        blank_line(&mut out);
        out.push_str(&format!("Function {}\n", function.name));
        body(&mut out, &function.body, 1);
        out.push_str("FunctionEnd\n");
    }

    for item in &module.sections {
        match item {
            ir::SectionItem::Section(section) => {
                blank_line(&mut out);
                section_block(&mut out, section, 0);
            }
            ir::SectionItem::Group(group) => {
                blank_line(&mut out);
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
            // hand-written NSIS — the output is the only debugger anyone has.
            ir::Item::Label(label) => {
                out.push_str(&INDENT.repeat(depth.saturating_sub(1)));
                out.push_str(&format!("{label}:"));
            }
        }
        out.push('\n');
    }
}

fn line(instruction: &ir::Instruction) -> String {
    let mut out = instruction.name.clone();
    for arg in &instruction.args {
        out.push(' ');
        match arg {
            ir::Arg::Str(value) => out.push_str(&quote(value)),
            ir::Arg::Raw(value) => out.push_str(value),
        }
    }
    out
}

/// NSIS quoting. `$` is left alone: `$VAR` interpolation is a language feature,
/// and escaping it is part of the string-model decision this PoC defers.
fn quote(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "$\\\""))
}
