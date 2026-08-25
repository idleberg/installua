//! `ReserveFile`, both halves — the one NSIS command whose two alternatives are
//! answered by two different things.
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
//!
//! `!addplugindir` is the other compiler-owned plugin line, at the bottom of
//! this file, and it is deliberately *not* reachability: see the module there.

use std::path::PathBuf;

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

/// `!addplugindir`: the line that makes a plugin outside `NSISDIR` reachable at
/// all.
///
/// Two things separate it from the reservations above. Its set is every plugin
/// the program **calls**, not the ones an init callback can reach — a search
/// path is about `makensis` finding the file, and every call site needs that —
/// and its **position is forced**: an untagged `!addplugindir` binds to whichever
/// target is current when the directive is processed, so it has exactly one
/// legal window, under `Unicode` and above every call site.
mod add_plugin_dir {
    use super::*;

    const VENDORED: &str = "[[plugin]]\n\
                            name = \"Nsis7z\"\n\
                            method = \"extract\"\n\
                            nsis = \"Nsis7z::Extract\"\n\
                            params = [\"path\"]\n\
                            dir = \"vendor/plugins\"\n";

    /// Compiles `source` against `toml`, with `/project` as the base — the
    /// directory a relative `dir` is written against.
    fn build_declared(source: &str, toml: &str) -> String {
        let (options, _) = options(toml, Some(PathBuf::from("/project")));
        let mut diags = Diagnostics::new();
        let output = installua::build_with(source, &options, &mut diags);
        assert!(diags.is_empty(), "{}", diags.render("<test>"));
        output.expect("compiles")
    }

    fn options(
        toml: &str,
        base: Option<PathBuf>,
    ) -> (installua::Options, Vec<installua::headers::Problem>) {
        let mut declarations = installua::headers::Declarations::builtin();
        let mut problems = Vec::new();
        declarations.parse("vendor.toml", toml, &mut problems);
        (
            installua::Options {
                base,
                declarations,
                ..installua::Options::default()
            },
            problems,
        )
    }

    /// A program calling the vendored plugin from a section — deliberately the
    /// case [`super::a_plugin_only_a_section_calls_is_not_reserved`] shows gets
    /// no `ReserveFile`, because it still needs the search path.
    fn vendored(section: &str) -> String {
        format!(
            "attributes {{ outFile = \"a.exe\", name = \"a\" }}\n\
             local z = plugin \"Nsis7z\"\n\
             installer {{ section(\"Core\", function() {section} end) }}\n"
        )
    }

    /// The window, stated as the only thing that can state it: the whole head of
    /// the file. Above `Unicode` the directive would bind to the default target
    /// and break every `unicode = false` build; below any call site it is too
    /// late for the lookup that call site does.
    #[test]
    fn the_line_sits_between_unicode_and_everything_else() {
        let output = build_declared(&vendored("z.extract(\"a.7z\")"), VENDORED);
        assert!(
            output.starts_with("Unicode true\n\n!addplugindir \"/project/vendor/plugins\"\n"),
            "{output}"
        );
    }

    /// The difference from a reservation, asserted as the difference: this
    /// program gets the search path and no `ReserveFile` at all.
    #[test]
    fn a_plugin_only_a_section_calls_still_gets_its_directory() {
        let output = build_declared(&vendored("z.extract(\"a.7z\")"), VENDORED);
        assert!(output.contains("!addplugindir"), "{output}");
        assert!(!output.contains("ReserveFile"), "{output}");
    }

    /// Declared but never called is no line. `!addplugindir` costs a directory
    /// scan in `makensis` and names a path that may not exist on the machine
    /// doing the build, so an unused declaration must not emit one.
    #[test]
    fn a_declared_plugin_nobody_calls_emits_nothing() {
        let output = build_declared(&vendored("detailPrint(\"x\")"), VENDORED);
        assert!(!output.contains("!addplugindir"), "{output}");
    }

    /// A plugin in `NSISDIR` needs no `dir`, and the builtins declare none — so
    /// the common program is unchanged by any of this.
    #[test]
    fn a_plugin_without_a_dir_emits_nothing() {
        let output = build(&program("detailPrint(\"x\")", RUN));
        assert!(!output.contains("!addplugindir"), "{output}");
    }

    /// A set, for the same reason the reservations are one: two plugins
    /// vendored into one directory are one search path.
    #[test]
    fn two_plugins_in_one_directory_are_one_line() {
        let toml = format!(
            "{VENDORED}\n\
             [[plugin]]\n\
             name = \"NsProcessX\"\n\
             method = \"find\"\n\
             nsis = \"NsProcessX::Find\"\n\
             params = [\"string\"]\n\
             dir = \"vendor/plugins\"\n"
        );
        let output = build_declared(
            "attributes { outFile = \"a.exe\", name = \"a\" }\n\
             local z = plugin \"Nsis7z\"\n\
             local p = plugin \"NsProcessX\"\n\
             installer { section(\"Core\", function()\n\
               z.extract(\"a.7z\")\n\
               p.find(\"a.exe\")\n\
             end) }\n",
            &toml,
        );
        assert_eq!(output.matches("!addplugindir").count(), 1, "{output}");
    }

    /// With no base there is no project root to resolve against, and a caller
    /// that handed the compiler a string and no directory has already said the
    /// file system is not involved. The declared path goes out as written.
    #[test]
    fn without_a_base_the_path_is_emitted_as_written() {
        let (options, _) = options(VENDORED, None);
        let mut diags = Diagnostics::new();
        let output = installua::build_with(&vendored("z.extract(\"a.7z\")"), &options, &mut diags)
            .expect("compiles");
        assert!(
            output.contains("!addplugindir \"vendor/plugins\""),
            "{output}"
        );
    }

    /// `dir` is a plugin's alone: a header is found along `!addincludedir`,
    /// which is a different directive in a different position.
    #[test]
    fn dir_on_a_header_is_a_problem() {
        let (_, problems) = options(
            "[[header]]\n\
             name = \"TextFunc\"\n\
             method = \"trim\"\n\
             nsis = \"TrimNewLines\"\n\
             dir = \"vendor\"\n",
            None,
        );
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(
            problems[0].message.contains("`dir` is a plugin field"),
            "{problems:?}"
        );
    }
}
