//! `.installua/headers/*.toml`: what a project declares for itself.
//!
//! What ships declared is a handful, and an installer reaches past it almost
//! immediately. A plugin becomes ordinary by being *declared* — whether that
//! happens in `src/headers/*.toml` here or in a project's own directory, since
//! both go through one parser — so what these check is that one declaration
//! reaches all three readers: the compiler's arity check, the emitted line, and
//! the editor stub.
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

/// A project file may correct a builtin. What ships is a file someone wrote
/// down, in the same format and as fallible, so a wrong count must not be a
/// wall.
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

/// The shipped files are compiled into the binary, so nothing at run time can
/// report a mistake in one. This is where that mistake is a build failure
/// instead — and it catches the duplicate case too, since two shipped files
/// declaring one method is exactly what `parse` complains about.
#[test]
fn every_declaration_that_ships_parses() {
    let (declarations, problems) = Declarations::shipped();
    assert!(problems.is_empty(), "{problems:?}");
    // Not an empty table dressed up as a clean one: a `SHIPPED` list that had
    // lost its entries would pass every assertion above this line.
    assert!(declarations.plugin("nsExec", "execToStack").is_some());
    assert!(declarations.lookup("FileFunc", "getSize").is_some());
}

/// The third-party plugin that ships declared, end to end. `uint` and not
/// `string` on the code: `pushint` is what the plugin calls, and `if code == 0`
/// is what every caller writes — a string would compile that as a text compare.
#[test]
fn ns_process_ships_declared() {
    let source = "attributes { outFile = \"a.exe\", name = \"a\" }\n\
                  local nsProcess = plugin \"nsProcess\"\n\
                  installer {\n\
                    section(\"Core\", function()\n\
                      local code = nsProcess.findProcess(\"app.exe\")\n\
                      if code == 0 then\n\
                        nsProcess.closeProcess(\"app.exe\")\n\
                      end\n\
                    end),\n\
                  }\n";
    let mut diags = Diagnostics::new();
    let output = installua::build(source, &mut diags).expect("compiles");
    assert!(diags.is_empty(), "{}", diags.render("<test>"));

    let lines: Vec<&str> = output.lines().map(str::trim).collect();
    assert!(
        lines.contains(&"nsProcess::_FindProcess \"app.exe\""),
        "{output}"
    );
    assert!(
        lines.contains(&"nsProcess::_CloseProcess \"app.exe\""),
        "{output}"
    );
    // `IntCmpU` and not `IntCmp` or `StrCmp`: the declaration said `uint`, so
    // the comparison is numeric *and* unsigned, which is the whole of what
    // choosing the type over `string` buys at the call site.
    assert!(
        lines.iter().any(|line| line.starts_with("IntCmpU $")),
        "{output}"
    );
    // The dropped result still comes off the stack — `closeProcess` pushed one
    // whether or not anybody wanted it.
    assert_eq!(
        lines.iter().filter(|line| *line == &"Pop $0").count(),
        2,
        "{output}"
    );
}

/// The builtins are declarations too, and the stub says so — which is what
/// stops the ones that ship here from being a different kind of thing from the
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

// -- callbacks ---------------------------------------------------------------
//
// The six macros NSIS calls back into are the one case a declaration cannot
// describe on its own: the arguments arrive in registers NSIS names, and that
// map lives in `src/lower/callback.rs`. What follows checks the seam between
// the two — that a declaration can say "a function goes here" and nothing
// more, and that every way of getting it wrong says so.

/// A walker's body becomes a `Function`, and the loop's two exits become the
/// two strings that function pushes. There is no loop in the output at all,
/// which is the whole shape of these and the thing a reader has to see once.
#[test]
fn a_walker_becomes_a_function_and_two_pushes() {
    let output = build(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         local fileFunc = import \"FileFunc\"\n\
         installer {\n\
           section(\"Core\", function()\n\
             for path in fileFunc.locate(INSTDIR, \"/L=F\") do\n\
               if path == \"stop.txt\" then break end\n\
               delete(path)\n\
             end\n\
           end),\n\
         }\n",
    );
    let lines: Vec<&str> = output.lines().map(str::trim).collect();
    assert!(
        lines.iter().any(
            |line| line.starts_with("${Locate} $INSTDIR \"/L=F\" __GENERATED_callback_locate_")
        ),
        "{output}"
    );
    // Read before written: the prologue goes through the stack because the
    // allocator may colour a destination onto a register a later input is
    // still sitting in.
    assert!(lines.contains(&"Push $R9"), "{output}");
    assert!(lines.contains(&"Push \"StopLocate\""), "{output}");
    assert!(lines.contains(&"Push \"\""), "{output}");
}

/// `${LineFind}` keeps NSIS's call shape, because its body answers with a
/// value. The line goes back through the register it arrived in, and the
/// sentinel says whether to write it.
#[test]
fn a_rewriting_callback_writes_its_line_back_into_the_register() {
    let output = build(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         local textFunc = import \"TextFunc\"\n\
         installer {\n\
           section(\"Core\", function()\n\
             textFunc.lineFind(\"a.ini\", \"b.ini\", \"1:-1\", function(line)\n\
               if line == \"x\" then return skip end\n\
               return line\n\
             end)\n\
           end),\n\
         }\n",
    );
    let lines: Vec<&str> = output.lines().map(str::trim).collect();
    assert!(lines.contains(&"Push \"SkipWrite\""), "{output}");
    assert!(
        lines.iter().any(|line| line.starts_with("StrCpy $R9 $")),
        "{output}"
    );
}

/// The cost of keeping register maps out of declarations, stated as a test: a
/// project may write `callback` in its own `.toml`, and the compiler will
/// refuse the macro rather than guess which register holds what.
#[test]
fn a_callback_macro_nobody_has_a_protocol_for_is_refused() {
    let mut declarations = Declarations::builtin();
    let mut problems = Vec::new();
    declarations.parse(
        "test.toml",
        "[[header]]\n\
         name = \"Elsewhere\"\n\
         method = \"walk\"\n\
         nsis = \"ElsewhereWalk\"\n\
         params = [\"path\", \"callback\"]\n\
         outputs = []\n",
        &mut problems,
    );
    assert!(problems.is_empty(), "{problems:?}");

    let mut diags = Diagnostics::new();
    installua::build_with(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         local elsewhere = import \"Elsewhere\"\n\
         installer {\n\
           section(\"Core\", function()\n\
             for path in elsewhere.walk(INSTDIR) do detailPrint(path) end\n\
           end),\n\
         }\n",
        &Options {
            declarations,
            ..Options::default()
        },
        &mut diags,
    );
    let rendered = diags.render("<test>");
    assert!(
        rendered.contains("callback protocol is not known"),
        "{rendered}"
    );
    assert!(rendered.contains("named registers"), "{rendered}");
}

/// A walker's body has two answers and no room for a third, so a `return` with
/// a value is refused rather than silently pushed — which would unbalance the
/// stack for everything after the walk.
#[test]
fn returning_a_value_from_a_walker_is_an_error() {
    let rendered = errors(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         local fileFunc = import \"FileFunc\"\n\
         installer {\n\
           section(\"Core\", function()\n\
             for path in fileFunc.locate(INSTDIR, \"\") do return path end\n\
           end),\n\
         }\n",
    );
    assert!(rendered.contains("returns nothing"), "{rendered}");
}

/// The two shapes are not interchangeable, and each error names the other one:
/// a walk written as a call, and a rewrite written as a loop.
#[test]
fn each_callback_shape_names_the_other_one() {
    let call = errors(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         local fileFunc = import \"FileFunc\"\n\
         installer {\n\
           section(\"Core\", function() fileFunc.locate(INSTDIR, \"\") end),\n\
         }\n",
    );
    assert!(call.contains("is a walk, not a call"), "{call}");

    let loop_form = errors(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         local textFunc = import \"TextFunc\"\n\
         installer {\n\
           section(\"Core\", function()\n\
             for line in textFunc.lineFind(\"a\", \"b\", \"\") do detailPrint(line) end\n\
           end),\n\
         }\n",
    );
    assert!(loop_form.contains("is not a loop"), "{loop_form}");
}
