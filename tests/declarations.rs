//! `.installua/declarations/*.toml`: what a project declares for itself.
//!
//! What ships declared is a handful, and an installer reaches past it almost
//! immediately. A plugin becomes ordinary by being *declared* — whether that
//! happens in `src/declarations/*.toml` here or in a project's own directory, since
//! both go through one parser — so what these check is that one declaration
//! reaches all three readers: the compiler's arity check, the emitted line, and
//! the editor stub.
//!
//! Everything here parses text rather than reading a directory. The loader is
//! a `read_dir` around [`Declarations::parse`], and a test that wrote files
//! would be testing `std::fs`.

use installua::declarations::{Declarations, Problem};
use installua::diag::Diagnostics;
use installua::{Options, stubs};

/// One declaration file, with both shapes a project writes.
///
/// `nsisunz` deliberately: it is a real plugin (21 of the 984 corpus scripts)
/// that this compiler does **not** ship a declaration for, so the file below is
/// one a project would actually have to write. The example used to be `Nsis7z`,
/// which now ships declared — and a test whose "undeclared" plugin is declared
/// proves the opposite of what it says.
///
/// `unzipToLog` rather than `unzip`, which is the same rule one level down: the
/// two differ only in whether each extracted name reaches the details log, and
/// the corpus writes `UnzipToLog` in eighteen files against `Unzip`'s one. An
/// example is copied whole, so it names the method a reader will actually want.
const DECLARED: &str = r#"
# A plugin someone shipped: two inputs, one value pushed.
[[plugin]]
name = "nsisunz"
method = "unzipToLog"
nsis = "nsisunz::UnzipToLog"
params = ["path", "path"]
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
         local nsisunz = plugin \"nsisunz\"\n\
         local textFunc = import \"TextFunc\"\n\
         installer {{\n\
           section(\"Core\", function() {body} end),\n\
         }}\n"
    )
}

/// The line a declared plugin call becomes, exactly. A plugin takes its
/// arguments inline and pushes its outputs, so it is one line plus a `Pop`
/// each — and the `path` positions are why both arguments come out with
/// backslashes they were not written with.
#[test]
fn a_declared_plugin_call_emits_the_line_the_declaration_names() {
    let output = build(&program(
        "local out = nsisunz.unzipToLog(\"data/archive.zip\", \"out/here\")\ndetailPrint(out)",
    ));
    let lines: Vec<&str> = output.lines().map(str::trim).collect();
    assert!(
        lines.contains(&"nsisunz::UnzipToLog \"data\\archive.zip\" \"out\\here\""),
        "{output}"
    );
}

/// `raw` in argument position, which is the only position where `raw` is not a
/// statement. The text lands in the line untouched — no quoting, and none of
/// the backslash conversion the `path` position above applies — and the call is
/// still a declared call, so the declared output still binds.
#[test]
fn a_raw_argument_splices_into_the_line_and_leaves_the_arity_alone() {
    let output = build(&program(
        "local out = nsisunz.unzipToLog(raw \"/noextractpath data/archive.zip\", \"out/here\")\n\
         detailPrint(out)",
    ));
    let lines: Vec<&str> = output.lines().map(str::trim).collect();
    assert!(
        lines.contains(&"nsisunz::UnzipToLog /noextractpath data/archive.zip \"out\\here\""),
        "{output}"
    );
    assert_eq!(
        lines.iter().filter(|line| line.starts_with("Pop ")).count(),
        1,
        "{output}"
    );
}

/// However many tokens it spells, a spliced argument is one argument. That is
/// what separates the hatch from a variadic tail: the position count stays the
/// declaration's, and so does the count of `Pop`s that follow it.
#[test]
fn a_raw_argument_is_one_position_however_many_words_it_holds() {
    let errors = errors(&program(
        "nsisunz.unzipToLog(raw \"/noextractpath a.zip out\")",
    ));
    assert!(
        errors.contains("takes 2 argument(s), and 1 were given"),
        "{errors}"
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
        "local a, b = nsisunz.unzipToLog(\"data/x.zip\", \"out\")",
    ));
    assert!(
        rendered.contains("pushes 1 value(s), and 2 are being bound"),
        "{rendered}"
    );
}

/// The undeclared method is `unzipToStack` on purpose: it is the one method of
/// this plugin that *cannot* be declared in this format at all — it pushes one
/// value per file in the archive — so the diagnostic below is the honest end of
/// the road rather than a declaration somebody forgot to write.
#[test]
fn a_method_nobody_declared_names_the_directory_that_would_declare_it() {
    let rendered = errors(&program("nsisunz.unzipToStack(\"data/x.zip\", \"out\")"));
    assert!(
        rendered.contains("`nsisunz` declares no `unzipToStack`"),
        "{rendered}"
    );
    assert!(rendered.contains("it declares `unzipToLog`"), "{rendered}");

    // And a plugin nobody has declared at all names the file that would fix it,
    // which is the whole discoverability of the format.
    let rendered = errors(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         local other = plugin \"Whatever\"\n\
         installer { section(\"Core\", function() other.go() end) }\n",
    );
    assert!(rendered.contains(".installua/declarations/"), "{rendered}");
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
        meta.contains("---@class installua.Plugin.nsisunz\n"),
        "{meta}"
    );
    assert!(
        meta.contains("function installua_Plugin_nsisunz.unzipToLog(a1, a2) end"),
        "{meta}"
    );
    // The literal-typed overload is what makes `plugin "nsisunz"` return that
    // class rather than a bare `table`.
    assert!(
        meta.contains("---@overload fun(name: '\"nsisunz\"'): installua.Plugin.nsisunz"),
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
        .split("-- `nsisunz::UnzipToLog`\n")
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

// -- tagged outputs: an arity that depends on the outcome -------------------

/// The shape `tagged` exists for, as the exact lines it becomes.
///
/// `StartMenu::Select` rather than a synthetic plugin, because the polarity is
/// the whole point and this one has the *unusual* polarity: the extra value
/// follows `"success"`, not a failure. Read from `Contrib/StartMenu/StartMenu.c`
/// — the readme's "pushes the folder after success" is one sentence describing
/// two arities.
///
/// Three lines carry the design and none of them is the `Pop`:
///
///   * the `StrCpy` comes **first**, so the tail slot is written on every path
///     and the allocator never sees a conditional definition;
///   * the test is `StrCmpS`, so a folder that differs from the tag only in
///     case is a different value;
///   * the target is a **label**, not `+2`, because a later pass inserting a
///     line is exactly how a relative jump goes silently wrong.
#[test]
fn a_tagged_plugin_pops_its_tail_only_when_the_tag_matches() {
    let output = build(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         local startMenu = plugin \"StartMenu\"\n\
         installer {\n\
           section(\"Core\", function()\n\
             local outcome, folder = startMenu.select(\"Example\")\n\
             detailPrint(outcome .. folder)\n\
           end),\n\
         }\n",
    );
    let lines: Vec<&str> = output.lines().map(str::trim).collect();
    let start = lines
        .iter()
        .position(|line| line.starts_with("StartMenu::Select"))
        .expect("the call is emitted");
    assert_eq!(
        &lines[start..start + 6],
        [
            "StartMenu::Select \"Example\"",
            "Pop $0",
            "StrCpy $1 \"\"",
            "StrCmpS $0 \"success\" 0 __GENERATED_tail_0",
            "Pop $1",
            "__GENERATED_tail_0:",
        ],
        "{output}"
    );
}

/// A tail the caller does not bind still comes off the stack.
///
/// The same rule as an ignored plain output, and for a stronger reason: the
/// plugin pushed it on that path, so leaving it there shifts every later `Pop`
/// by one and NSIS reports none of it. What the caller wanted has no bearing
/// on what the stack holds.
#[test]
fn an_unbound_tail_is_still_popped() {
    let output = build(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         local startMenu = plugin \"StartMenu\"\n\
         installer {\n\
           section(\"Core\", function()\n\
             local outcome = startMenu.select(\"Example\")\n\
             detailPrint(outcome)\n\
           end),\n\
         }\n",
    );
    assert_eq!(
        output
            .lines()
            .filter(|line| line.trim().starts_with("Pop "))
            .count(),
        2,
        "{output}"
    );
}

/// Both halves of the tail are part of the Lua arity, so binding past them is
/// the same error binding past `outputs` is — and the note says which of the
/// values only sometimes exist, since that is the part a reader cannot infer
/// from the count.
#[test]
fn binding_past_the_tail_names_the_tag() {
    let rendered = errors(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         local startMenu = plugin \"StartMenu\"\n\
         installer {\n\
           section(\"Core\", function()\n\
             local a, b, c = startMenu.select(\"Example\")\n\
           end),\n\
         }\n",
    );
    assert!(
        rendered.contains("pushes 2 value(s), and 3 are being bound"),
        "{rendered}"
    );
    assert!(
        rendered.contains("follow only when the first is `success`"),
        "{rendered}"
    );
}

/// The three ways a `tagged` block can be written and mean nothing.
///
/// Synthetic declarations on purpose: what is under test is the parser's
/// refusal, and a real plugin would only add a name to argue about. Each of
/// these would otherwise *compile* — into an unbalanced stack rather than into
/// a diagnostic, which is the failure mode the whole file format exists to
/// prevent.
#[test]
fn half_a_tag_is_refused() {
    let problems = |text: &str| -> Vec<Problem> {
        let mut declarations = Declarations::builtin();
        let mut problems = Vec::new();
        declarations.parse("test.toml", text, &mut problems);
        problems
    };

    // `tagged` with nothing to pop, and `more` with nothing to test.
    for half in ["tagged = [\"error\"]", "more = [\"string\"]"] {
        let found = problems(&format!(
            "[[plugin]]\nname = \"P\"\nmethod = \"m\"\nnsis = \"P::M\"\n\
             params = []\noutputs = [\"string\"]\n{half}\n"
        ));
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            found[0].message.contains("one field in two halves"),
            "{found:?}"
        );
    }

    // A tag with no first value to be.
    let found = problems(
        "[[plugin]]\nname = \"P\"\nmethod = \"m\"\nnsis = \"P::M\"\n\
         params = []\noutputs = []\ntagged = [\"error\"]\nmore = [\"string\"]\n",
    );
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].message.contains("`outputs` has to declare one"),
        "{found:?}"
    );

    // And a macro cannot have one at all: its outputs are registers written on
    // every path, so there is no first value and no arity to vary.
    let found = problems(
        "[[header]]\nname = \"H\"\nmethod = \"m\"\nnsis = \"M\"\n\
         params = []\noutputs = [\"string\"]\ntagged = [\"error\"]\nmore = [\"string\"]\n",
    );
    assert!(
        found
            .iter()
            .any(|problem| problem.message.contains("is a plugin field")),
        "{found:?}"
    );
}

/// A terminator is emitted last, after the flags *and* after the arguments —
/// the one thing in a declaration that lands to the right of `params`.
///
/// `inetc::get` is why it exists: it reads url/file pairs off the stack until
/// it pops `/END`, and the value under the last argument here is a caller-save.
/// So the token is not a call-site choice, and this test is that the call site
/// cannot influence it — the source names no terminator and the line has one.
#[test]
fn a_terminator_is_emitted_after_the_arguments() {
    let mut declarations = Declarations::builtin();
    let mut problems = Vec::new();
    declarations.parse(
        "test.toml",
        "[[plugin]]\nname = \"Looper\"\nmethod = \"fetch\"\nnsis = \"Looper::Fetch\"\n\
         params = [\"string\", \"path\"]\noutputs = [\"string\"]\nterminator = \"/END\"\n\
         flags = [{ name = \"silent\", nsis = \"/SILENT\" }]\n",
        &mut problems,
    );
    assert!(problems.is_empty(), "{problems:?}");

    let mut diags = Diagnostics::new();
    let output = installua::build_with(
        "attributes { outFile = \"a.exe\", name = \"a\" }\n\
         local looper = plugin \"Looper\"\n\
         installer {\n\
           section(\"Core\", function()\n\
             local out = looper.fetch(\"http://example.com/x\", \"out/x\", { silent = true })\n\
             detailPrint(out)\n\
           end),\n\
         }\n",
        &Options {
            declarations,
            ..Options::default()
        },
        &mut diags,
    );
    assert!(diags.is_empty(), "{}", diags.render("<test>"));
    let output = output.expect("compiles");
    let lines: Vec<&str> = output.lines().map(str::trim).collect();
    assert!(
        lines.contains(&"Looper::Fetch /SILENT \"http://example.com/x\" \"out\\x\" /END"),
        "{output}"
    );
}

/// A terminator has to be a switch, and has to be a plugin's.
///
/// Both refusals are about the same confusion: a token with no `/` is one the
/// plugin reads as an argument, and a macro's arguments are counted by
/// `!insertmacro` rather than marked.
#[test]
fn a_terminator_that_is_not_a_switch_is_refused() {
    let problems = |text: &str| -> Vec<Problem> {
        let mut declarations = Declarations::builtin();
        let mut problems = Vec::new();
        declarations.parse("test.toml", text, &mut problems);
        problems
    };

    let found = problems(
        "[[plugin]]\nname = \"P\"\nmethod = \"m\"\nnsis = \"P::M\"\n\
         params = []\noutputs = []\nterminator = \"END\"\n",
    );
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].message.contains("wants a switch"), "{found:?}");

    let found = problems(
        "[[header]]\nname = \"H\"\nmethod = \"m\"\nnsis = \"M\"\n\
         params = []\noutputs = []\nterminator = \"/END\"\n",
    );
    assert!(
        found
            .iter()
            .any(|problem| problem.message.contains("is a plugin field")),
        "{found:?}"
    );
}
