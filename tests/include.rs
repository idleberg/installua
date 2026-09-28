//! `include`: what it merges, what it refuses, and where a diagnostic lands.
//!
//! Most assertions here are about the *second* file — that a name it declares
//! is usable, that a mistake in it is reported against its own path rather than
//! the root's, that including it twice declares nothing twice. The output is
//! covered by `tests/golden/include.lua`, which asserts the thing that is
//! hardest to state as a predicate: the emitted script carries no trace of the
//! split at all.

use std::collections::BTreeMap;

use installua::Options;
use installua::diag::{Code, Diagnostics};
use installua::frontend::include::Loader;

/// A project, in memory. No test here touches the disk: re-entrancy requires
/// that this crate compile against strings, and a second file does not change
/// that.
fn project(files: &[(&str, &str)]) -> Options {
    let sources: BTreeMap<String, String> = files
        .iter()
        .map(|(name, text)| (name.to_string(), text.to_string()))
        .collect();
    Options {
        root: Some("install.lua".into()),
        loader: Loader::Memory(sources),
        ..Options::default()
    }
}

fn compile(root: &str, files: &[(&str, &str)]) -> (Option<String>, Diagnostics) {
    let options = project(files);
    let mut diags = Diagnostics::new();
    let output = installua::build_with(root, &options, &mut diags);
    (output, diags)
}

fn build(root: &str, files: &[(&str, &str)]) -> String {
    let (output, diags) = compile(root, files);
    output.unwrap_or_else(|| panic!("compiles:\n{}", diags.render("install.lua")))
}

fn fails(root: &str, files: &[(&str, &str)], code: Code) -> String {
    let (_, diags) = compile(root, files);
    assert!(
        diags.contains(code),
        "expected `{code}`, got:\n{}",
        diags.render("install.lua")
    );
    diags.render("install.lua")
}

const ATTRIBUTES: &str = "attributes { outFile = \"a.exe\" }\n";

/// The base case, and the whole of the feature: a declaration in one file, its
/// use in another.
#[test]
fn an_included_func_is_callable() {
    let output = build(
        &format!(
            "{ATTRIBUTES}include \"helpers.lua\"\ninstaller {{ section(\"Core\", function() greet() end), }}"
        ),
        &[(
            "helpers.lua",
            "func(\"greet\", function() detailPrint(\"hi\") end)",
        )],
    );
    assert!(output.contains("Function greet"), "{output}");
    assert!(output.contains("Call greet"), "{output}");
}

/// Order-freeness survives the merge, in the direction that has no Lua excuse:
/// the `include` is written *below* the call that needs it.
#[test]
fn a_file_may_be_included_after_it_is_used() {
    let output = build(
        &format!(
            "{ATTRIBUTES}installer {{ section(\"Core\", function() greet() end), }}\ninclude \"helpers.lua\""
        ),
        &[(
            "helpers.lua",
            "func(\"greet\", function() detailPrint(\"hi\") end)",
        )],
    );
    assert!(output.contains("Function greet"), "{output}");
}

/// Nothing in the output says a second file was involved. This is the property
/// that makes `include` a source-layout feature rather than a stage.
#[test]
fn the_output_is_the_same_as_one_file() {
    let split = build(
        &format!(
            "{ATTRIBUTES}include \"helpers.lua\"\ninstaller {{ section(\"Core\", function() greet() end), }}"
        ),
        &[(
            "helpers.lua",
            "func(\"greet\", function() detailPrint(\"hi\") end)",
        )],
    );
    let whole = build(
        &format!(
            "{ATTRIBUTES}func(\"greet\", function() detailPrint(\"hi\") end)\n\
             installer {{ section(\"Core\", function() greet() end), }}"
        ),
        &[],
    );
    assert_eq!(split, whole);
}

/// A file included from two places is loaded once — the set semantics `import`
/// already has. Loading it twice would declare `greet` twice, and the second
/// `include` did not ask for that.
#[test]
fn a_file_is_included_once_however_often_it_is_named() {
    let output = build(
        &format!(
            "{ATTRIBUTES}include \"helpers.lua\"\ninclude \"a.lua\"\n\
             installer {{ section(\"Core\", function() greet() end), }}"
        ),
        &[
            (
                "helpers.lua",
                "func(\"greet\", function() detailPrint(\"hi\") end)",
            ),
            ("a.lua", "include \"helpers.lua\""),
        ],
    );
    assert_eq!(output.matches("Function greet").count(), 1, "{output}");
}

/// Paths resolve against the file that names them, not against the root. The
/// root says `strings/de.lua`; that file says `shared.lua` and means
/// `strings/shared.lua`.
#[test]
fn a_path_is_relative_to_the_file_that_names_it() {
    let output = build(
        &format!(
            "{ATTRIBUTES}include \"strings/de.lua\"\ninstaller {{ section(\"Core\", function() greet() end), }}"
        ),
        &[
            ("strings/de.lua", "include \"shared.lua\""),
            (
                "strings/shared.lua",
                "func(\"greet\", function() detailPrint(\"hallo\") end)",
            ),
        ],
    );
    assert!(output.contains("Function greet"), "{output}");
}

/// `..` climbs, and the key it produces is the same one another spelling of the
/// same file would produce — which is what makes the once-only rule hold.
#[test]
fn a_climbing_path_normalises_to_the_same_file() {
    let output = build(
        &format!(
            "{ATTRIBUTES}include \"helpers.lua\"\ninclude \"strings/de.lua\"\n\
             installer {{ section(\"Core\", function() greet() end), }}"
        ),
        &[
            (
                "helpers.lua",
                "func(\"greet\", function() detailPrint(\"hi\") end)",
            ),
            ("strings/de.lua", "include \"../helpers.lua\""),
        ],
    );
    assert_eq!(output.matches("Function greet").count(), 1, "{output}");
}

/// A block is a declaration like any other, so an included file may hold one —
/// and the at-most-one rule that already exists is what catches a second.
#[test]
fn an_included_file_may_hold_a_block() {
    let output = build(
        &format!(
            "{ATTRIBUTES}include \"un.lua\"\ninstaller {{ page.instFiles {{}}, section(\"Core\", function() setOutPath(INSTDIR) end), }}"
        ),
        &[(
            "un.lua",
            "uninstaller { page.instFiles {}, section(\"Uninstall\", function() rmDir(INSTDIR) end), }",
        )],
    );
    assert!(output.contains("Section \"un.Uninstall\""), "{output}");
}

#[test]
fn a_block_declared_twice_across_files_is_still_a_duplicate() {
    fails(
        &format!("{ATTRIBUTES}include \"more.lua\""),
        &[("more.lua", "attributes { name = \"Twice\" }")],
        Code::DuplicateBlock,
    );
}

/// A `<const>` is a `!define`, so a second one was `makensis`'s error on the
/// output — and across files the included value was silently lost first.
///
#[test]
fn a_const_declared_twice_is_a_duplicate() {
    fails(
        &format!(
            "{ATTRIBUTES}local COLOR <const> = \"red\"\nlocal COLOR <const> = \"blue\"\n\
             installer {{ page.instFiles {{}}, section(\"s\", function() detailPrint(COLOR) end) }}"
        ),
        &[],
        Code::DuplicateBlock,
    );
}

/// Across files it is two names, as in Lua: each file reads its own. The second
/// file's is renamed `COLOR_1` on the way to the one namespace, so the output is
/// that of the program written with the renamed spelling by hand — and the
/// export carries the renamed one too.
#[test]
fn a_local_of_one_name_in_two_files_is_two_locals() {
    let more = "local COLOR <const> = \"red\"\n\
                local paint = section(\"more\", function() detailPrint(COLOR) end)\n\
                return { paint = paint }";
    let split = build(
        &format!(
            "{ATTRIBUTES}local more = include \"more.lua\"\nlocal COLOR <const> = \"blue\"\n\
             installer {{ page.instFiles {{}}, more.paint, \
             section(\"s\", function() detailPrint(COLOR) end) }}"
        ),
        &[("more.lua", more)],
    );
    let whole = build(
        &format!(
            "{ATTRIBUTES}local COLOR_1 <const> = \"red\"\n\
             local paint = section(\"more\", function() detailPrint(COLOR_1) end)\n\
             local COLOR <const> = \"blue\"\n\
             installer {{ page.instFiles {{}}, paint, \
             section(\"s\", function() detailPrint(COLOR) end) }}"
        ),
        &[],
    );
    assert_eq!(split, whole);
}

/// The other half of the same reason: a note that points at a *second* place
/// has to name that place's file too.
///
/// Without it the two lines read as one file — the header says `install.lua:5`
/// and the note says `line 1`, and the reader goes looking for a declaration on
/// line 1 of a file that has an `include` there instead.
#[test]
fn a_note_pointing_into_another_file_names_it() {
    let rendered = fails(
        &format!("{ATTRIBUTES}include \"more.lua\"\nfunc(\"yolo\", function() end)"),
        &[("more.lua", "func(\"yolo\", function() end)")],
        Code::DuplicateBlock,
    );
    assert!(
        rendered.contains("note: the first one is at more.lua:1:"),
        "the note names the other file:\n{rendered}"
    );
}

/// And a note pointing inside the *same* file does not repeat its name — the
/// header already said it, and a note that says it again is noise on every
/// single-file program, which is nearly all of them.
#[test]
fn a_note_pointing_into_the_same_file_gives_only_the_position() {
    let rendered = fails(
        &format!("{ATTRIBUTES}func(\"yolo\", function() end)\nfunc(\"yolo\", function() end)"),
        &[],
        Code::DuplicateBlock,
    );
    assert!(
        rendered.contains("note: the first one is at 2:1"),
        "position without a file:\n{rendered}"
    );
}

/// A diagnostic in an included file names *that* file. This is the whole reason
/// a span carries a file at all.
#[test]
fn a_diagnostic_names_the_file_it_came_from() {
    let rendered = fails(
        &format!("{ATTRIBUTES}include \"strings/de.lua\""),
        &[("strings/de.lua", "local x = 1.5")],
        Code::FloatLiteral,
    );
    assert!(
        rendered.contains("strings/de.lua:1:"),
        "the file and its own line, not the root's:\n{rendered}"
    );
}

#[test]
fn a_missing_file_names_the_path_that_was_written() {
    let rendered = fails(
        &format!("{ATTRIBUTES}include \"strings/de.lua\""),
        &[],
        Code::IncludeNotFound,
    );
    assert!(rendered.contains("strings/de.lua"), "{rendered}");
}

/// A path with no extension is the mistake a module system would have made
/// legal, so the not-found says which half is missing.
#[test]
fn a_path_without_an_extension_says_so() {
    let rendered = fails(
        &format!("{ATTRIBUTES}include \"strings/de\""),
        &[("strings/de.lua", "")],
        Code::IncludeNotFound,
    );
    assert!(rendered.contains("including the extension"), "{rendered}");
}

/// The loop is named, not merely detected: the file the loader noticed it at is
/// rarely the one with the mistake in it.
#[test]
fn a_cycle_names_the_whole_loop() {
    let rendered = fails(
        &format!("{ATTRIBUTES}include \"a.lua\""),
        &[
            ("a.lua", "include \"b.lua\""),
            ("b.lua", "include \"a.lua\""),
        ],
        Code::IncludeCycle,
    );
    assert!(
        rendered.contains("install.lua → a.lua → b.lua → a.lua"),
        "{rendered}"
    );
}

#[test]
fn a_file_that_includes_itself_is_a_cycle() {
    fails(
        &format!("{ATTRIBUTES}include \"a.lua\""),
        &[("a.lua", "include \"a.lua\"")],
        Code::IncludeCycle,
    );
}

/// Inside a body there is no stage that could decide it: the merging is
/// compile-time and the condition would be install-time.
#[test]
fn include_inside_a_body_is_refused() {
    fails(
        &format!(
            "{ATTRIBUTES}installer {{ section(\"Core\", function()\n\
             include \"helpers.lua\"\n\
             end), }}"
        ),
        &[("helpers.lua", "")],
        Code::IncludeForm,
    );
}

#[test]
fn an_expression_path_is_refused() {
    let rendered = fails(
        &format!("{ATTRIBUTES}local NAME <const> = \"helpers\"\ninclude(NAME .. \".lua\")"),
        &[("helpers.lua", "")],
        Code::IncludeForm,
    );
    assert!(rendered.contains("string literal"), "{rendered}");
}

/// A malformed `include` is reported once. It used to survive into lowering,
/// where `include` is not a declaration and never will be, and earn a second
/// and worse diagnostic there.
#[test]
fn a_malformed_include_is_reported_once() {
    let (_, diags) = compile(&format!("{ATTRIBUTES}include(1)"), &[]);
    let codes: Vec<Code> = diags.iter().map(|d| d.code).collect();
    assert_eq!(codes, vec![Code::IncludeForm], "{codes:?}");
}

/// An in-memory source with no directory behind it still compiles. It only
/// cannot `include`, and the diagnostic says which of the two things is missing
/// rather than blaming the path.
#[test]
fn a_source_with_no_directory_says_so() {
    let mut diags = Diagnostics::new();
    installua::build(&format!("{ATTRIBUTES}include \"helpers.lua\""), &mut diags);
    let rendered = diags.render("<source>");
    assert!(diags.contains(Code::IncludeNotFound), "{rendered}");
    assert!(rendered.contains("did not come from a file"), "{rendered}");
}

const INSTALL: &str =
    "installer { page.instFiles {}, section(\"s\", function() detailPrint(COLOR) end) }";

/// Lua's rule, put back on top of the splice: a chunk's `local` is its own.
#[test]
fn a_local_is_private_to_its_file() {
    let rendered = fails(
        &format!("{ATTRIBUTES}include \"colors.lua\"\n{INSTALL}"),
        &[("colors.lua", "local COLOR <const> = \"red\"")],
        Code::NotInScope,
    );
    assert!(rendered.contains("colors.lua:1:7"), "{rendered}");
}

/// And Lua's way to share one: the file returns it, the includer binds the
/// file and reads through the binding. The output is the one-file program's.
#[test]
fn a_returned_name_is_read_through_the_binding() {
    let output = build(
        &format!(
            "{ATTRIBUTES}local colors = include \"colors.lua\"\n\
             installer {{ page.instFiles {{}}, section(\"s\", function() detailPrint(colors.COLOR) end) }}"
        ),
        &[(
            "colors.lua",
            "local COLOR <const> = \"red\"\nreturn { COLOR = COLOR }",
        )],
    );
    assert!(output.contains("!define COLOR \"red\""), "{output}");
    assert!(output.contains("DetailPrint \"${COLOR}\""), "{output}");
}

/// A section from another file, listed in a block: the case `include` exists
/// for, and the one a bare name used to cover.
#[test]
fn a_returned_section_is_listed_through_the_binding() {
    let output = build(
        &format!(
            "{ATTRIBUTES}local parts = include \"parts.lua\"\n\
             installer {{ page.instFiles {{}}, parts.core }}"
        ),
        &[(
            "parts.lua",
            "local core = section(\"Core\", function() end)\nreturn { core = core }",
        )],
    );
    assert!(output.contains("Section \"Core\""), "{output}");
}

/// A file included from two places is loaded once, and both bindings read it.
#[test]
fn two_files_may_bind_the_same_file() {
    build(
        &format!(
            "{ATTRIBUTES}local colors = include \"colors.lua\"\ninclude \"b.lua\"\n\
             installer {{ page.instFiles {{}}, section(\"s\", function() detailPrint(colors.COLOR) end) }}"
        ),
        &[
            (
                "colors.lua",
                "local COLOR <const> = \"red\"\nreturn { COLOR = COLOR }",
            ),
            (
                "b.lua",
                "local c = include \"colors.lua\"\n\
                 func(\"shout\", function() detailPrint(c.COLOR) end)",
            ),
        ],
    );
}

#[test]
fn a_name_the_file_does_not_return_is_refused() {
    let rendered = fails(
        &format!(
            "{ATTRIBUTES}local colors = include \"colors.lua\"\n\
             installer {{ page.instFiles {{}}, section(\"s\", function() detailPrint(colors.SHADE) end) }}"
        ),
        &[(
            "colors.lua",
            "local COLOR <const> = \"red\"\nreturn { COLOR = COLOR }",
        )],
        Code::NotInScope,
    );
    assert!(rendered.contains("SHADE = SHADE"), "{rendered}");
}

/// The binding is a build-time namespace, as `import`'s is: nothing to pass.
#[test]
fn the_binding_is_not_a_value() {
    fails(
        &format!("{ATTRIBUTES}local colors = include \"colors.lua\"\ninstaller {{ colors }}"),
        &[("colors.lua", "return {}")],
        Code::NotInScope,
    );
}

/// A body's own `local` shadows the binding, as any Lua `local` does.
#[test]
fn a_body_local_shadows_the_binding() {
    build(
        &format!(
            "{ATTRIBUTES}local colors = include \"colors.lua\"\n\
             installer {{ page.instFiles {{}}, section(\"s\", function()\n\
             local colors = \"x\"\ndetailPrint(colors)\nend) }}"
        ),
        &[("colors.lua", "return {}")],
    );
}

#[test]
fn a_file_returns_only_its_own_names() {
    fails(
        &format!("{ATTRIBUTES}include \"colors.lua\"\n{INSTALL}\nlocal COLOR <const> = \"red\""),
        &[("colors.lua", "return { COLOR = COLOR }")],
        Code::NotInScope,
    );
    fails(
        &format!("{ATTRIBUTES}include \"colors.lua\""),
        &[("colors.lua", "return { COLOR = \"red\" }")],
        Code::IncludeForm,
    );
    fails(
        &format!("{ATTRIBUTES}include \"colors.lua\""),
        &[("colors.lua", "return 1")],
        Code::IncludeForm,
    );
}

#[test]
fn include_binds_one_name() {
    fails(
        &format!("{ATTRIBUTES}local a, b = include \"colors.lua\""),
        &[("colors.lua", "return {}")],
        Code::IncludeForm,
    );
    fails(
        &format!(
            "{ATTRIBUTES}installer {{ section(\"Core\", function()\n\
             local colors = include \"colors.lua\"\n\
             end), }}"
        ),
        &[("colors.lua", "return {}")],
        Code::IncludeForm,
    );
}
