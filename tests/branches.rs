//! Build-time `if` at the top level: the branch the compiler takes.
//!
//! NSIS spells this `!if` / `!ifdef`, and 180 corpus files reach for it. What is
//! being added here is deliberately *not* that: a `!if` survives into the script
//! and is decided by the preprocessor, in the same textual pass whose ordering
//! rules the emitter's fixed spine exists to hide. A top-level `if` here folds in
//! resolution, so by the time `lower` hands a module to `emit` the conditional no
//! longer exists — there is nothing left to order, and `src/emit.rs` does not
//! know the feature is there.
//!
//! That is the claim most of this file is about: **the output of a branch is
//! indistinguishable from the output of the program with only that branch
//! written**. The rest is the constraint that makes it hold — the condition has
//! to fold, which is the same rule `<const>` already lives under.

use std::collections::BTreeMap;

use installua::diag::{Code, Diagnostics};

fn build(source: &str, params: &[(&str, &str)]) -> String {
    let mut diags = Diagnostics::new();
    let output = installua::build_with(source, &options(params), &mut diags);
    assert!(diags.is_empty(), "{}", diags.render("<test>"));
    output.expect("it compiles")
}

fn errors(source: &str) -> Diagnostics {
    let mut diags = Diagnostics::new();
    let _ = installua::build_with(source, &options(&[]), &mut diags);
    assert!(!diags.is_empty(), "no diagnostic");
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

fn raised(diags: &Diagnostics, code: Code) -> bool {
    diags.iter().any(|diag| diag.code == code)
}

/// The plan's own example: the corpus idiom this replaces, which is a `-D` and a
/// branch over it.
const ARCHED: &str = "local ARCH <const> = param(\"ARCH\", \"x86\")\n\
                      attributes { name = \"A\", outFile = \"a.exe\" }\n\
                      if ARCH == \"x64\" then\n\
                        installer { section(\"Core64\", function() detailPrint(\"64\") end) }\n\
                      else\n\
                        installer { section(\"Core\", function() detailPrint(\"32\") end) }\n\
                      end\n";

#[test]
fn the_default_takes_the_else() {
    let output = build(ARCHED, &[]);
    assert!(output.contains("Section \"Core\""), "{output}");
    assert!(!output.contains("Core64"), "{output}");
    assert!(!output.contains("DetailPrint \"64\""), "{output}");
}

#[test]
fn a_define_takes_the_other_branch() {
    let output = build(ARCHED, &[("ARCH", "x64")]);
    assert!(output.contains("Section \"Core64\""), "{output}");
    assert!(!output.contains("DetailPrint \"32\""), "{output}");
}

/// The load-bearing claim. A conditional is a branch the compiler takes, not a
/// directive it emits — so nothing in the output records that there was a
/// decision, and the emitter's slots are exactly what they were.
#[test]
fn nothing_of_the_conditional_reaches_the_output() {
    let output = build(ARCHED, &[]);
    assert!(!output.contains("!if"), "{output}");
    assert!(!output.contains("!else"), "{output}");
    assert!(!output.contains("!endif"), "{output}");
    assert_eq!(output.lines().next(), Some("Unicode true"));
}

/// `elseif` needs no handling of its own: the frontend desugars it into a nested
/// `If` in the else branch, so it arrives as a conditional inside the block the
/// outer one selects and the next round of the fixpoint decides it.
#[test]
fn an_elseif_chain_picks_one_arm() {
    let source = "local CHANNEL <const> = param(\"CHANNEL\", \"stable\")\n\
                  attributes { name = \"A\", outFile = \"a.exe\" }\n\
                  if CHANNEL == \"nightly\" then\n\
                    installer { section(\"N\", function() detailPrint(\"nightly\") end) }\n\
                  elseif CHANNEL == \"beta\" then\n\
                    installer { section(\"B\", function() detailPrint(\"beta\") end) }\n\
                  else\n\
                    installer { section(\"S\", function() detailPrint(\"stable\") end) }\n\
                  end\n";
    let output = build(source, &[("CHANNEL", "beta")]);
    assert!(output.contains("DetailPrint \"beta\""), "{output}");
    assert!(!output.contains("nightly"), "{output}");
    assert!(!output.contains("stable"), "{output}");
}

/// A false condition with no `else` selects nothing, which is the `!ifdef` shape
/// the corpus uses for a feature flag.
#[test]
fn a_flag_that_is_off_adds_nothing() {
    let source = "local EXTRAS <const> = param(\"EXTRAS\", false)\n\
                  attributes { name = \"A\", outFile = \"a.exe\" }\n\
                  local core = section(\"Core\", function() detailPrint(\"core\") end)\n\
                  if EXTRAS then\n\
                    raw.tail [[ !finalize 'echo extras' ]]\n\
                  end\n\
                  installer { core }\n";
    assert!(!build(source, &[]).contains("!finalize"));
    assert!(build(source, &[("EXTRAS", "true")]).contains("!finalize 'echo extras'"));
}

/// Every kind of top-level declaration, once the branch is taken, is a top-level
/// declaration — there is no second set of rules for what a branch may contain,
/// because a selected statement is spliced into the top level and walked by the
/// same passes.
mod what_a_branch_may_hold {
    use super::*;

    /// A `<const>` declared inside a branch is visible everywhere, and the one
    /// declared in the branch not taken is not declared at all.
    #[test]
    fn a_const_and_the_define_it_becomes() {
        let source = "local WIDE <const> = param(\"WIDE\", true)\n\
                      if WIDE then\n\
                        local SUFFIX <const> = \"-x64\"\n\
                      else\n\
                        local SUFFIX <const> = \"-x86\"\n\
                      end\n\
                      attributes { name = \"A\" .. SUFFIX, outFile = \"a.exe\" }\n\
                      installer { section(\"Core\", function() end) }\n";
        let output = build(source, &[]);
        assert!(output.contains("!define SUFFIX \"-x64\""), "{output}");
        assert!(!output.contains("-x86"), "{output}");
    }

    /// And it lands where the `if` was written rather than where the fixpoint
    /// got to it: a `<const>` inside a branch is declared a round later than the
    /// statements around it, and `!define` order is source order.
    #[test]
    fn a_define_from_a_branch_keeps_its_position() {
        let source = "local FIRST <const> = \"1\"\n\
                      if FIRST == \"1\" then\n\
                        local SECOND <const> = \"2\"\n\
                      end\n\
                      local THIRD <const> = \"3\"\n\
                      attributes { name = FIRST .. SECOND .. THIRD, outFile = \"a.exe\" }\n\
                      installer { section(\"Core\", function() end) }\n";
        let output = build(source, &[]);
        let defines: Vec<&str> = output
            .lines()
            .filter(|line| line.starts_with("!define"))
            .collect();
        assert_eq!(
            defines,
            ["!define FIRST 1", "!define SECOND 2", "!define THIRD 3"],
            "{output}"
        );
    }

    /// A parameter declared inside a branch is a parameter — and one inside the
    /// branch not taken is not, so a `-D` for it is the same unknown-name error
    /// as a misspelling. That is the honest answer: in this configuration the
    /// program really does not declare it.
    #[test]
    fn a_parameter_in_the_branch_not_taken_is_not_declared() {
        let source = "local WIDE <const> = param(\"WIDE\", false)\n\
                      if WIDE then\n\
                        local SUFFIX <const> = param(\"SUFFIX\", \"-x64\")\n\
                      end\n\
                      attributes { name = \"A\", outFile = \"a.exe\" }\n\
                      installer { section(\"Core\", function() end) }\n";
        assert!(
            build(source, &[("WIDE", "true"), ("SUFFIX", "-w")],).contains("!define SUFFIX \"-w\"")
        );

        let mut diags = Diagnostics::new();
        let _ = installua::build_with(source, &options(&[("SUFFIX", "-w")]), &mut diags);
        assert!(raised(&diags, Code::UnknownParam), "{diags:?}");
    }

    /// A section declared in a branch, listed by a block outside it. Resolution
    /// is order-free and selection happens before any of it, so the two have no
    /// idea they were written at different levels.
    #[test]
    fn a_section_a_block_outside_the_branch_lists() {
        let source = "local EXTRAS <const> = param(\"EXTRAS\", true)\n\
                      attributes { name = \"A\", outFile = \"a.exe\" }\n\
                      local core = section(\"Core\", function() detailPrint(\"core\") end)\n\
                      if EXTRAS then\n\
                        local docs = section(\"Docs\", function() detailPrint(\"docs\") end)\n\
                        installer { core, docs }\n\
                      else\n\
                        installer { core }\n\
                      end\n";
        let output = build(source, &[]);
        assert!(output.contains("Section \"Docs\""), "{output}");
        assert!(!build(source, &[("EXTRAS", "false")]).contains("Docs"));
    }

    /// A `func`, which is collected by a pass of its own — over the selected top
    /// level, so a callback in a branch is callable and one in the branch not
    /// taken is not there to be called.
    #[test]
    fn a_func_and_the_call_that_reaches_it() {
        let source = "local TRACE <const> = param(\"TRACE\", true)\n\
                      attributes { name = \"A\", outFile = \"a.exe\" }\n\
                      if TRACE then\n\
                        func(\"trace\", function() detailPrint(\"tracing\") end)\n\
                      end\n\
                      installer { section(\"Core\", function() if TRACE then trace() end end) }\n";
        let output = build(source, &[]);
        assert!(output.contains("Function trace"), "{output}");
        assert!(output.contains("Call trace"), "{output}");
        assert!(!build(source, &[("TRACE", "false")]).contains("trace"));
    }

    /// A global, which is declared by assigning to it: the `Var` and the
    /// `.onInit` line that initialises it both follow the selection.
    #[test]
    fn a_global_and_its_var() {
        let source = "local DEBUG <const> = param(\"DEBUG\", true)\n\
                      attributes { name = \"A\", outFile = \"a.exe\" }\n\
                      if DEBUG then\n\
                        level = \"verbose\"\n\
                      end\n\
                      installer { section(\"Core\", function() end) }\n";
        let output = build(source, &[]);
        assert!(output.contains("Var level"), "{output}");
        assert!(output.contains("StrCpy $level \"verbose\""), "{output}");
        assert!(!build(source, &[("DEBUG", "false")]).contains("Var level"));
    }
}

/// The constraint that keeps all of the above true: a condition the compiler
/// cannot decide is an error, never a branch deferred to install time.
mod the_condition_folds {
    use super::*;

    /// A global is written at install time, so out here there is nothing to
    /// test — the note points at where the runtime `if` does work.
    #[test]
    fn a_runtime_value_is_an_error_that_names_the_runtime_if() {
        let diags = errors(
            "attributes { name = \"A\", outFile = \"a.exe\" }\n\
             mode = \"full\"\n\
             if mode == \"full\" then\n\
               installer { section(\"Core\", function() end) }\n\
             end\n",
        );
        assert!(raised(&diags, Code::ConstIf), "{}", diags.render("<test>"));
        let rendered = diags.render("<test>");
        assert!(rendered.contains("`section`"), "{rendered}");
    }

    /// An NSIS constant is read at install time too, however constant the name
    /// makes it sound.
    #[test]
    fn an_install_time_constant_is_not_a_build_time_one() {
        let diags = errors(
            "attributes { name = \"A\", outFile = \"a.exe\" }\n\
             if INSTDIR == \"C:\\\\\" then\n\
               installer { section(\"Core\", function() end) }\n\
             end\n",
        );
        assert!(raised(&diags, Code::ConstIf), "{}", diags.render("<test>"));
    }

    /// And a condition that folds to something that is not a `bool` is the same
    /// rejection the runtime `if` makes, for the same reason: Lua's truthiness
    /// would run `if count then` on `0`.
    #[test]
    fn a_folded_non_bool_is_still_not_a_condition() {
        let diags = errors(
            "local COUNT <const> = 0\n\
             attributes { name = \"A\", outFile = \"a.exe\" }\n\
             if COUNT then\n\
               installer { section(\"Core\", function() end) }\n\
             end\n",
        );
        assert!(raised(&diags, Code::NotBool), "{}", diags.render("<test>"));
    }

    /// A `<const>` declared below the `if` that reads it still decides it: the
    /// fixpoint folds what it can, takes the branches that became decidable, and
    /// runs again. Order-freeness does not stop at the conditional.
    #[test]
    fn a_constant_declared_below_the_if_still_decides_it() {
        let output = build(
            "attributes { name = \"A\", outFile = \"a.exe\" }\n\
             if WIDE then\n\
               installer { section(\"Core64\", function() end) }\n\
             else\n\
               installer { section(\"Core\", function() end) }\n\
             end\n\
             local WIDE <const> = true\n",
            &[],
        );
        assert!(output.contains("Section \"Core64\""), "{output}");
    }

    /// A `<const>` declared inside a branch, deciding a *later* branch. Each
    /// round of the fixpoint declares what the last one selected, which is what
    /// makes nesting and sequencing work without either being a special case.
    #[test]
    fn a_branch_may_declare_what_the_next_one_reads() {
        let output = build(
            "local WIDE <const> = true\n\
             attributes { name = \"A\", outFile = \"a.exe\" }\n\
             if WIDE then\n\
               local CPU <const> = \"amd64\"\n\
             end\n\
             if CPU == \"amd64\" then\n\
               installer { section(\"Core64\", function() end) }\n\
             end\n",
            &[],
        );
        assert!(output.contains("Section \"Core64\""), "{output}");
    }
}
