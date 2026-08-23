//! `ReserveFile`, both halves — the one NSIS command whose two alternatives are
//! answered by two different things (§11).
//!
//! `reserveFile(…)` is the first alternative and an ordinary row: the same
//! parameter list `file` has, minus `/a`. Nothing here is about it that is not
//! also about `file`, so it gets one test.
//!
//! The rest is the second alternative, `/plugin file.dll`, which has no surface
//! spelling at all. A plugin DLL sits in the compressed data block like any
//! other file and `.onInit` runs before a byte of it has been extracted — so
//! whether a plugin call from there works depends on the compressor, and the
//! line that fixes it is one a user has to know to write. The compiler knows
//! every call site and every edge between bodies, so it writes it.
//!
//! **Reachability rather than presence** is the whole of what these assert. A
//! reserved file is excluded from solid compression, so reserving a plugin that
//! only a section calls would cost size for nothing.

use installua::diag::Diagnostics;

fn build(source: &str) -> String {
    let mut diags = Diagnostics::new();
    let output = installua::build(source, &mut diags);
    assert!(diags.is_empty(), "{}", diags.render("<test>"));
    output.expect("compiles")
}

/// A program with both plugins declared, `body` in `.onInit`, and a section
/// holding whatever `section` says.
fn program(body: &str, section: &str) -> String {
    format!(
        "attributes {{ outFile = \"a.exe\", name = \"a\" }}\n\
         local UserInfo = plugin \"UserInfo\"\n\
         local nsExec = plugin \"nsExec\"\n\
         installer {{\n\
           onInit(function() {body} end),\n\
           section(\"Core\", function() {section} end),\n\
         }}\n"
    )
}

const ASK: &str = "local who = UserInfo.getAccountType()\ndetailPrint(who)";
const RUN: &str = "local rc, out = nsExec.execToStack(\"cmd /c ver\")\ndetailPrint(out)";

/// The first alternative, which needed no lowering: three flags, and `/x`
/// repeats because it is the one that holds a list.
#[test]
fn reserve_file_is_files_own_parameter_list() {
    let output = build(&program(
        "detailPrint(\"x\")",
        "reserveFile(\"data/*.pak\", { recursive = true, exclude = { \"*.tmp\", \"*.log\" } })",
    ));
    assert!(
        output.contains("ReserveFile /r /x \"*.tmp\" /x \"*.log\" \"data\\*.pak\""),
        "{output}"
    );
}

/// The call is in `.onInit` itself, which is the case MUI2 hands its users a
/// macro for.
#[test]
fn a_plugin_on_init_calls_is_reserved() {
    let output = build(&program(ASK, "detailPrint(\"x\")"));
    assert!(
        output.contains("ReserveFile /plugin UserInfo.dll"),
        "{output}"
    );
}

/// One edge further out. The reservation is a question about *reachability*, so
/// the call graph answers it and not the text of `.onInit`.
#[test]
fn a_reservation_follows_the_call_graph() {
    let output = build(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         local UserInfo = plugin \"UserInfo\"\n\
         func(\"outer\", function() inner() end)\n\
         func(\"inner\", function()\n\
           local who = UserInfo.getAccountType()\n\
           detailPrint(who)\n\
         end)\n\
         installer { onInit(function() outer() end), section(\"Core\", function() end) }\n",
    );
    assert!(
        output.contains("ReserveFile /plugin UserInfo.dll"),
        "{output}"
    );
}

/// And the other direction, which is why this is not simply "reserve every
/// plugin the program names": a reserved file does not get solid-compressed
/// with the rest, so an unneeded reservation is paid for in bytes.
#[test]
fn a_plugin_only_a_section_calls_is_not_reserved() {
    let output = build(&program("detailPrint(\"x\")", RUN));
    assert!(output.contains("nsExec::ExecToStack"), "{output}");
    assert!(!output.contains("ReserveFile"), "{output}");
}

/// Two plugins, one reachable and one not, in one program — the pair the
/// previous two tests split, asserted together so that neither passes by the
/// program having nothing else in it.
#[test]
fn each_plugin_is_judged_on_its_own_reachability() {
    let output = build(&program(ASK, RUN));
    assert!(
        output.contains("ReserveFile /plugin UserInfo.dll"),
        "{output}"
    );
    assert!(!output.contains("nsExec.dll"), "{output}");
}

/// A set, not a list: the DLL is reserved once however many methods are called
/// on it, and a second `ReserveFile` for the same file is a `!warning` from
/// NSIS at best.
#[test]
fn a_plugin_called_twice_is_reserved_once() {
    let output = build(&program(
        "local a = UserInfo.getAccountType()\n\
         local b = UserInfo.getAccountType()\n\
         detailPrint(a .. b)",
        "detailPrint(\"x\")",
    ));
    assert_eq!(output.matches("ReserveFile /plugin").count(), 1, "{output}");
}

/// The uninstaller's init callback is the other root. NSIS numbers the two
/// halves separately everywhere else, but they share one data block and one
/// `ReserveFile` list.
#[test]
fn un_on_init_is_a_root_too() {
    let output = build(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         local UserInfo = plugin \"UserInfo\"\n\
         installer { section(\"Core\", function() end) }\n\
         uninstaller {\n\
           section(\"Core\", function() end),\n\
           onInit(function()\n\
             local who = UserInfo.getAccountType()\n\
             detailPrint(who)\n\
           end),\n\
         }\n",
    );
    assert!(
        output.contains("ReserveFile /plugin UserInfo.dll"),
        "{output}"
    );
}

/// Where the line goes, which is the half of this that a golden cannot state as
/// a fact: before every section, because the head of the data block is what a
/// reservation is asking for.
#[test]
fn the_reservation_precedes_every_section() {
    let output = build(&program(ASK, RUN));
    let reserve = output.find("ReserveFile /plugin").expect("reserved");
    let section = output.find("Section \"Core\"").expect("a section");
    assert!(reserve < section, "{output}");
}
