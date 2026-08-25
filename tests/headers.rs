//! `.installua/headers/*.toml`: what a project declares for itself.
//!
//! The builtins cover three plugin methods and three macros, which is what this
//! repository's own examples reach and nothing like what an installer reaches.
//! A third-party plugin becomes ordinary by being *declared* — so what these
//! check is that one declaration reaches all three readers: the compiler's
//! arity check, the emitted line, and the editor stub.
//!
//! Everything here parses text rather than reading a directory. The loader is
//! a `read_dir` around [`Declarations::parse`], and a test that wrote files
//! would be testing `std::fs`.

use installua::diag::Diagnostics;
use installua::headers::{Declarations, Problem};
use installua::{Options, stubs};

/// One declaration file, with both shapes a project writes.
const DECLARED: &str = r#"
# A plugin someone shipped: two inputs, one value pushed.
[[plugin]]
name = "Nsis7z"
method = "extractWithDetails"
nsis = "Nsis7z::ExtractWithDetails"
params = ["path", "string"]
outputs = ["string"]

[[header]]
name = "TextFunc"
method = "trimNewLines"
nsis = "TrimNewLines"
params = ["string"]
outputs = ["string"]
"#;

fn declared() -> Declarations {
    let mut declarations = Declarations::builtin();
    let mut problems = Vec::new();
    declarations.parse("test.toml", DECLARED, &mut problems);
    assert!(problems.is_empty(), "{problems:?}");
    declarations
}

fn options() -> Options {
    Options {
        declarations: declared(),
        ..Options::default()
    }
}

fn build(source: &str) -> String {
    let mut diags = Diagnostics::new();
    let output = installua::build_with(source, &options(), &mut diags);
    assert!(diags.is_empty(), "{}", diags.render("<test>"));
    output.expect("compiles")
}

fn errors(source: &str) -> String {
    let mut diags = Diagnostics::new();
    installua::build_with(source, &options(), &mut diags);
    assert!(diags.has_errors(), "expected an error");
    diags.render("<test>")
}

/// A program with the declared plugin and header bound, and `body` in a
/// section.
fn program(body: &str) -> String {
    format!(
        "attributes {{ outFile = \"a.exe\", name = \"a\" }}\n\
         local sevenZip = plugin \"Nsis7z\"\n\
         local textFunc = import \"TextFunc\"\n\
         installer {{\n\
           section(\"Core\", function() {body} end),\n\
         }}\n"
    )
}

/// The line a declared plugin call becomes, exactly. A plugin takes its
/// arguments inline and pushes its outputs, so it is one line plus a `Pop`
/// each — and the `path` position is why the first argument comes out with a
/// backslash it was not written with.
#[test]
fn a_declared_plugin_call_emits_the_line_the_declaration_names() {
    let output = build(&program(
        "local out = sevenZip.extractWithDetails(\"data/archive.7z\", \"\")\ndetailPrint(out)",
    ));
    let lines: Vec<&str> = output.lines().map(str::trim).collect();
    assert!(
        lines.contains(&"Nsis7z::ExtractWithDetails \"data\\archive.7z\" \"\""),
        "{output}"
    );
}

/// The same for a header macro, whose convention is the other one: outputs are
/// trailing register arguments, because `!insertmacro` cannot return anything.
#[test]
fn a_declared_macro_takes_its_output_as_a_trailing_register() {
    let output = build(&program(
        "local trimmed = textFunc.trimNewLines(\"a\\n\")\ndetailPrint(trimmed)",
    ));
    assert!(
        output
            .lines()
            .any(|line| line.trim().starts_with("${TrimNewLines} \"a$\\n\" $")),
        "{output}"
    );
    // And the header is `!include`d, once, without anybody asking: `import` is
    // what says which file, and the declaration says what is in it.
    assert_eq!(
        output
            .lines()
            .filter(|line| line.contains("TextFunc.nsh"))
            .count(),
        1,
        "{output}"
    );
}

/// The count is the thing the declaration exists for: binding more values than
/// a plugin pushes unbalances the stack, and NSIS reports none of it.
#[test]
fn binding_more_values_than_the_declaration_pushes_is_an_error() {
    let rendered = errors(&program(
        "local a, b = sevenZip.extractWithDetails(\"data/x.7z\", \"\")",
    ));
    assert!(
        rendered.contains("pushes 1 value(s), and 2 are being bound"),
        "{rendered}"
    );
}

#[test]
fn a_method_nobody_declared_names_the_directory_that_would_declare_it() {
    let rendered = errors(&program("sevenZip.extract(\"data/x.7z\")"));
    assert!(
        rendered.contains("`Nsis7z` declares no `extract`"),
        "{rendered}"
    );
    assert!(
        rendered.contains("it declares `extractWithDetails`"),
        "{rendered}"
    );

    // And a plugin nobody has declared at all names the file that would fix it,
    // which is the whole discoverability of the format.
    let rendered = errors(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         local other = plugin \"Whatever\"\n\
         installer { section(\"Core\", function() other.go() end) }\n",
    );
    assert!(rendered.contains(".installua/headers/"), "{rendered}");
}

/// A project file may correct a builtin. The builtins are three entries someone
/// wrote down, and a wrong count in them must not be a wall.
#[test]
fn a_project_declaration_replaces_a_builtin_without_complaint() {
    let mut declarations = Declarations::builtin();
    let mut problems = Vec::new();
    declarations.parse(
        "test.toml",
        "[[plugin]]\n\
         name = \"UserInfo\"\n\
         method = \"getAccountType\"\n\
         nsis = \"UserInfo::GetOriginalAccountType\"\n\
         params = []\n\
         outputs = [\"string\"]\n",
        &mut problems,
    );
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(
        declarations
            .plugin("UserInfo", "getAccountType")
            .map(|entry| entry.nsis.as_str()),
        Some("UserInfo::GetOriginalAccountType")
    );

    // A second *project* declaration of the same method is a mistake rather
    // than a correction, and says so — while still taking the later one, since
    // refusing would leave the compiler holding whichever file sorted first.
    declarations.parse(
        "other.toml",
        "[[plugin]]\n\
         name = \"UserInfo\"\n\
         method = \"getAccountType\"\n\
         nsis = \"UserInfo::GetAccountType\"\n\
         outputs = [\"string\"]\n",
        &mut problems,
    );
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].message.contains("already declared"),
        "{problems:?}"
    );
}

/// Every way a declaration file can be wrong says which line and which field.
#[test]
fn a_malformed_declaration_names_the_line_and_the_field() {
    let mut declarations = Declarations::builtin();
    let mut problems: Vec<Problem> = Vec::new();
    declarations.parse(
        "bad.toml",
        "[[plugin]]\n\
         name = \"P\"\n\
         method = \"m\"\n\
         nsis = \"P::M\"\n\
         params = [\"widget\"]\n\
         \n\
         [[plugin]]\n\
         name = \"Q\"\n\
         \n\
         [[thing]]\n\
         \n\
         nsis = \"R::S\"\n",
        &mut problems,
    );

    let lines: Vec<usize> = problems.iter().map(|problem| problem.line).collect();
    let messages: Vec<&str> = problems
        .iter()
        .map(|problem| problem.message.as_str())
        .collect();

    assert_eq!(lines, vec![5, 7, 10, 12], "{problems:?}");
    assert!(
        messages[0].contains("`widget` is not a type"),
        "{messages:?}"
    );
    assert!(messages[1].contains("missing one of"), "{messages:?}");
    assert!(
        messages[2].contains("`[[thing]]` is not a declaration"),
        "{messages:?}"
    );
    assert!(
        messages[3].contains("comes before any `[[plugin]]`"),
        "{messages:?}"
    );

    // The block that parsed is still declared: a file with a bad line is not a
    // file with nothing in it, and the CLI stops on the problems either way.
    assert!(declarations.plugin("P", "m").is_some());
}

/// The editor's half. A declaration only the compiler read would leave the call
/// untyped in exactly the place a user needed help: a plugin nobody has heard
/// of.
#[test]
fn a_declared_plugin_is_typed_in_the_stub() {
    let meta = stubs::meta(&declared());

    assert!(
        meta.contains("---@class installua.Plugin.Nsis7z\n"),
        "{meta}"
    );
    assert!(
        meta.contains("function installua_Plugin_Nsis7z.extractWithDetails(a1, a2) end"),
        "{meta}"
    );
    // The literal-typed overload is what makes `plugin "Nsis7z"` return that
    // class rather than a bare `table`.
    assert!(
        meta.contains("---@overload fun(name: '\"Nsis7z\"'): installua.Plugin.Nsis7z"),
        "{meta}"
    );
    assert!(
        meta.contains("---@overload fun(header: '\"TextFunc\"'): installua.Header.TextFunc"),
        "{meta}"
    );

    // One `---@return` per pushed value and one `---@param` per position, so
    // `local a, b = …` is checked by the editor and the compiler with one count
    // between them.
    let method = meta
        .split("-- `Nsis7z::ExtractWithDetails`\n")
        .nth(1)
        .and_then(|rest| rest.split("function ").next())
        .unwrap_or_default();
    assert_eq!(method.matches("---@return string").count(), 1, "{method}");
    assert_eq!(method.matches("---@param ").count(), 2, "{method}");
}

/// The builtins are declarations too, and the stub says so — which is what
/// stops the three that ship here from being a different kind of thing from the
/// ones a project writes.
#[test]
fn the_builtins_reach_the_stub_by_the_same_road() {
    let meta = stubs::meta(&Declarations::builtin());
    assert!(
        meta.contains("---@class installua.Plugin.nsExec\n"),
        "{meta}"
    );
    assert!(
        meta.contains("function installua_Plugin_nsExec.execToStack(a1) end"),
        "{meta}"
    );
    assert!(
        meta.contains("---@overload fun(header: '\"FileFunc\"'): installua.Header.FileFunc"),
        "{meta}"
    );
}
