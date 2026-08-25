//! Build parameters: a `<const>` whose value the invocation may set.
//!
//! `local VERSION <const> = param("VERSION", "1.4.2")` is the whole surface, and
//! `installua build … -D VERSION=2.0.0` is the other half. NSIS spells it
//! `!ifndef VERSION / !define VERSION "1.4.2" / !endif`, and the reason that
//! idiom is being retired rather than translated is its failure mode: a misspelt
//! `-DVERSOIN` defines a second thing nobody reads, so the build succeeds, the
//! default ships, and nothing says so.
//!
//! Everything asserted here follows from one decision — **the declaration is in
//! the source**. That is what gives the compiler a set of names to check a `-D`
//! against, and a *default* to read the command line's text as. Both are things
//! a `[params]` table in `installua.toml` would have had too; what it would not
//! have had is the declaration sitting next to the use.
//!
//! A parameter introduces no ordering question at all: it folds in resolution
//! exactly as `<const>` does, so by the time anything is bucketed there is no
//! parameter left. Nothing in `src/emit.rs` knows this feature exists.

use std::collections::BTreeMap;

use installua::diag::{Code, Diagnostics};

/// Compiles `source` with the given `-D`s, asserting it was accepted.
fn build(source: &str, params: &[(&str, &str)]) -> String {
    let mut diags = Diagnostics::new();
    let output = installua::build_with(source, &options(params), &mut diags);
    assert!(diags.is_empty(), "{}", diags.render("<test>"));
    output.expect("compiles")
}

/// The diagnostics, for the cases that are meant to raise one.
fn errors(source: &str, params: &[(&str, &str)]) -> Diagnostics {
    let mut diags = Diagnostics::new();
    installua::build_with(source, &options(params), &mut diags);
    diags
}

fn options(params: &[(&str, &str)]) -> installua::Options {
    installua::Options {
        params: params
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect::<BTreeMap<_, _>>(),
        ..installua::Options::default()
    }
}

/// A program with a parameter and a second `<const>` derived from it, so that
/// an override is visible both where it was declared and one fold further on.
const VERSIONED: &str = "local VERSION <const> = param(\"VERSION\", \"1.4.2\")\n\
                         local FULL <const> = VERSION .. \"-win64\"\n\
                         attributes { name = \"a\", outFile = FULL .. \".exe\" }\n\
                         installer { section(\"Core\", function() detailPrint(VERSION) end) }\n";

/// No `-D`: the default is the value, and the parameter is an ordinary
/// `!define` in the output. Nothing in the emitted script says this constant
/// was settable — which is the point, since the preprocessor is not where the
/// decision was made.
#[test]
fn the_default_is_the_value_a_plain_build_gets() {
    let output = build(VERSIONED, &[]);
    assert!(output.contains("!define VERSION \"1.4.2\""), "{output}");
    assert!(output.contains("!define FULL \"1.4.2-win64\""), "{output}");
    assert!(!output.contains("!ifndef"), "{output}");
}

/// The flag replaces it, everywhere the constant was read — including inside
/// the `<const>` built from it, which is what makes composition work without
/// `param` being an expression.
#[test]
fn a_define_replaces_the_default() {
    let output = build(VERSIONED, &[("VERSION", "2.0.0")]);
    assert!(output.contains("!define VERSION \"2.0.0\""), "{output}");
    assert!(output.contains("!define FULL \"2.0.0-win64\""), "{output}");
    assert!(!output.contains("1.4.2"), "{output}");
}

/// Order-free, like every other top-level name: the parameter is declared below
/// the block that reads it, and the worklist in `resolve` closes over it.
#[test]
fn a_parameter_may_be_declared_below_its_use() {
    let output = build(
        "attributes { name = \"a\", outFile = NAME }\n\
         local NAME <const> = param(\"NAME\", \"a.exe\")\n\
         installer { section(\"Core\", function() end) }\n",
        &[("NAME", "b.exe")],
    );
    assert!(output.contains("!define NAME \"b.exe\""), "{output}");
    assert!(output.contains("OutFile \"${NAME}\""), "{output}");
}

/// The `-D` name and the local are two different things, and are not required
/// to agree: the string is an interface to whatever runs the build, and the
/// local is the program's own name for the value.
#[test]
fn the_flag_name_and_the_binding_are_separate() {
    let output = build(
        "local channel <const> = param(\"RELEASE_CHANNEL\", \"stable\")\n\
         attributes { name = channel, outFile = \"a.exe\" }\n\
         installer { section(\"Core\", function() end) }\n",
        &[("RELEASE_CHANNEL", "nightly")],
    );
    assert!(output.contains("!define channel \"nightly\""), "{output}");
    assert!(output.contains("Name \"${channel}\""), "{output}");
}

/// The default is the type declaration, and there is nowhere else for one to
/// be: an integer parameter folds as an integer, so it reaches arithmetic
/// rather than a `StrCmp`.
#[test]
fn an_integer_default_makes_an_integer_parameter() {
    let output = build(
        "local PORT <const> = param(\"PORT\", 8080)\n\
         local NEXT <const> = \"next is \" .. (PORT + 1)\n\
         attributes { name = \"a\", outFile = \"a.exe\" }\n\
         installer { section(\"Core\", function() detailPrint(NEXT) end) }\n",
        &[("PORT", "9090")],
    );
    assert!(output.contains("!define PORT 9090"), "{output}");
    assert!(output.contains("!define NEXT \"next is 9091\""), "{output}");
}

/// And the same declaration is what rejects a value that is not one. Without
/// it, `abc` would reach an `IntOp` as a string and NSIS would silently read it
/// as zero.
#[test]
fn a_value_that_is_not_the_declared_type_is_an_error() {
    let diags = errors(
        "local PORT <const> = param(\"PORT\", 8080)\n\
         attributes { name = \"a\", outFile = \"a.exe\" }\n\
         installer { section(\"Core\", function() end) }\n",
        &[("PORT", "abc")],
    );
    assert!(diags.contains(Code::BadFieldValue), "{diags:?}");
    assert!(
        diags.render("<test>").contains("is not an integer"),
        "{}",
        diags.render("<test>")
    );
}

/// A boolean parameter takes the two words that fold to one, so a feature flag
/// is a `bool` in the language rather than a string compared against `"true"`.
#[test]
fn a_boolean_default_takes_true_and_false() {
    let output = build(
        "local SIGNED <const> = param(\"SIGNED\", false)\n\
         attributes { name = \"a\", outFile = \"a.exe\", requestExecutionLevel = \"admin\" }\n\
         installer { section(\"Core\", function()\n\
           if SIGNED then detailPrint(\"signed\") end\n\
         end) }\n",
        &[("SIGNED", "true")],
    );
    assert!(output.contains("DetailPrint \"signed\""), "{output}");
}

/// The whole reason parameters are declared rather than merely defined: a name
/// nothing declares is an error naming the ones that exist, instead of a build
/// that quietly used the default.
#[test]
fn an_unknown_define_is_an_error_that_names_what_is_declared() {
    let diags = errors(VERSIONED, &[("VERSOIN", "2.0.0")]);
    assert!(diags.contains(Code::UnknownParam), "{diags:?}");
    let rendered = diags.render("<test>");
    assert!(rendered.contains("`VERSION`"), "{rendered}");
}

/// Declared twice is a genuine ambiguity: two defaults for one flag have no
/// answer, and resolution being order-free means there is no later one to win.
#[test]
fn one_flag_has_one_default() {
    let diags = errors(
        "local A <const> = param(\"V\", \"1\")\n\
         local B <const> = param(\"V\", \"2\")\n\
         attributes { name = \"a\", outFile = \"a.exe\" }\n\
         installer { section(\"Core\", function() end) }\n",
        &[],
    );
    assert!(diags.contains(Code::DuplicateBlock), "{diags:?}");
}

/// `param` is a declaration, not an expression, and the two ways of forgetting
/// that are worth their own message rather than the generic one each would
/// otherwise get.
mod form {
    use super::*;

    /// In a body it would be an unknown function, which says nothing about the
    /// construct whose entire job is to be a declaration.
    #[test]
    fn param_in_a_body_says_what_it_is() {
        let diags = errors(
            "attributes { name = \"a\", outFile = \"a.exe\" }\n\
             installer { section(\"Core\", function() detailPrint(param(\"X\", \"1\")) end) }\n",
            &[],
        );
        assert!(diags.contains(Code::ParamForm), "{diags:?}");
    }

    /// Composed into a larger initialiser it would be "not a build-time
    /// constant", which points at the wrong half of the line — a `param(…)` is
    /// build-time, it simply has to be readable before anything folds.
    #[test]
    fn param_composed_into_an_expression_says_to_split_it() {
        let diags = errors(
            "local V <const> = param(\"V\", \"1\") .. \"-beta\"\n\
             attributes { name = \"a\", outFile = \"a.exe\" }\n\
             installer { section(\"Core\", function() end) }\n",
            &[],
        );
        assert!(diags.contains(Code::ParamForm), "{diags:?}");
        assert!(
            diags.render("<test>").contains("split it"),
            "{}",
            diags.render("<test>")
        );
    }

    /// The name has to be a literal for the same reason: it is what `-D` is
    /// checked against, before there is anything to fold it from.
    #[test]
    fn the_name_has_to_be_a_literal() {
        let diags = errors(
            "local WHICH <const> = \"V\"\n\
             local V <const> = param(WHICH, \"1\")\n\
             attributes { name = \"a\", outFile = \"a.exe\" }\n\
             installer { section(\"Core\", function() end) }\n",
            &[],
        );
        assert!(diags.contains(Code::ParamForm), "{diags:?}");
    }

    /// A default is not optional. There is no `nil` here for a missing one to
    /// be, and a parameter with no default would make every build that forgot
    /// the flag a different kind of failure.
    #[test]
    fn a_default_is_required() {
        let diags = errors(
            "local V <const> = param(\"V\")\n\
             attributes { name = \"a\", outFile = \"a.exe\" }\n\
             installer { section(\"Core\", function() end) }\n",
            &[],
        );
        assert!(diags.contains(Code::ParamForm), "{diags:?}");
    }

    /// And a default that is not itself constant is reported as the default it
    /// is, rather than as the `<const>` it was bound to — the mistake is beside
    /// the name, not at the binding.
    #[test]
    fn a_default_that_does_not_fold_names_the_parameter() {
        let diags = errors(
            "local V <const> = param(\"V\", INSTDIR)\n\
             attributes { name = \"a\", outFile = \"a.exe\" }\n\
             installer { section(\"Core\", function() end) }\n",
            &[],
        );
        let rendered = diags.render("<test>");
        assert!(rendered.contains("the default for `V`"), "{rendered}");
    }
}
