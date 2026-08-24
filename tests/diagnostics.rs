//! The diagnostic registry, as a test rather than a promise.
//!
//! PLAN §2: *every diagnostic code has a test that produces it, enforced by a
//! registry-walking test, and every rejection names its replacement.* All three
//! are checked here, and the table is the mechanism — a code added to
//! `Code::ALL` without a case in `CASES` fails the build, so omission is
//! unrepresentable (§14).

use std::collections::BTreeMap;

use installua::diag::{Code, Diagnostics, Severity};
use installua::frontend::include::Loader;

/// One source per code, each the smallest thing that raises it.
///
/// A case may raise *other* codes too — `detailPrint` at the top level is not a
/// declaration, so the `dollar-in-literal` case also collects a
/// `not-yet-implemented`. That is fine and deliberately not asserted away:
/// pinning the exact diagnostic set of every snippet would make this table a
/// second golden file with none of the leverage.
const CASES: &[(Code, &str)] = &[
    (Code::ParseError, "attributes {"),
    (Code::FloatLiteral, "local x = 1.5"),
    (Code::InvalidEscape, r#"local p = "C:\Program Files""#),
    (Code::DollarInLiteral, r#"detailPrint("into $INSTDIR")"#),
    (Code::OverlongLiteral, OVERLONG),
    (Code::FloatDivision, "local x = 1 / 2"),
    (Code::Exponentiation, "local x = 2 ^ 8"),
    (Code::LengthOperator, "local n = #name"),
    (Code::Goto, "::done::\ngoto done"),
    (Code::NilValue, "local x = nil"),
    (Code::Varargs, "local x = ..."),
    (Code::RepeatLoop, "repeat until true"),
    (Code::FunctionStatement, "function f() end"),
    (Code::ClosureValue, "local f = function() end"),
    (Code::CloseAttribute, "local f <close> = handle"),
    (Code::UnknownAttribute, "local x <mutable> = 1"),
    (Code::IndexExpression, "local x = pages[1]"),
    (Code::UnsupportedIterator, "for k, v in pairs(t) do end"),
    (Code::RuntimeRequire, r#"require("WinVer")"#),
    // Resolution and types. These are the cases that have to reach lowering, so
    // each is a whole program rather than a fragment.
    (
        Code::UndefinedName,
        "attributes { outFile = \"a.exe\" }\n\
         installer { section(\"Core\", function() detailPrint(nope) end), }",
    ),
    (
        Code::NotBool,
        "attributes { outFile = \"a.exe\" }\n\
         installer { section(\"Core\", function()\n\
         local count = 1\n\
         if count then detailPrint(\"reached\") end\n\
         end), }",
    ),
    (
        Code::OrAsValue,
        "attributes { outFile = \"a.exe\" }\n\
         installer { section(\"Core\", function()\n\
         local custom = \"\"\n\
         local dir = custom or \"C:/App\"\n\
         end), }",
    ),
    (
        Code::TypeMismatch,
        "attributes { outFile = \"a.exe\" }\n\
         installer { section(\"Core\", function()\n\
         local n = 1\n\
         local s = \"one\"\n\
         if n == s then detailPrint(\"never\") end\n\
         end), }",
    ),
    (
        Code::TypeConflict,
        "attributes { outFile = \"a.exe\" }\n\
         installer { section(\"Core\", function()\n\
         local n = 1\n\
         n = \"one\"\n\
         end), }",
    ),
    (
        Code::WrongArity,
        "attributes { outFile = \"a.exe\" }\n\
         installer { section(\"Core\", function() detailPrint(\"a\", \"b\") end), }",
    ),
    (
        Code::BreakOutsideLoop,
        "attributes { outFile = \"a.exe\" }\n\
         installer { section(\"Core\", function() break end), }",
    ),
    (
        Code::ContinueOutsideLoop,
        "attributes { outFile = \"a.exe\" }\n\
         installer { section(\"Core\", function() continue() end), }",
    ),
    (
        Code::ReturnArity,
        "attributes { outFile = \"a.exe\" }\n\
         func(\"maybe\", function()\n\
         if fileExists(\"x\") then return 1 end\n\
         end)\n\
         installer { section(\"Core\", function() maybe() end), }",
    ),
    (Code::RegisterExhaustion, EXHAUSTED),
    (
        Code::DeepRecursion,
        "attributes { outFile = \"a.exe\" }\n\
         func(\"countdown\", function(n)\n\
         if n <= 0 then return 0 end\n\
         return countdown(n - 1)\n\
         end)\n\
         installer { section(\"Core\", function()\n\
         local left = countdown(4)\n\
         detailPrint(\"left \" .. left)\n\
         end), }",
    ),
    (
        Code::ConstantAnswer,
        "attributes { outFile = \"a.exe\" }\n\
         installer { section(\"Core\", function()\n\
         local answer = messageBox(\"done\")\n\
         if answer == \"OK\" then detailPrint(\"always\") end\n\
         end), }",
    ),
    // `languages {}` and then `include` were this case in turn, and both are
    // now implemented. `import` is what is left: it is exposed as an
    // *expression* — `local mui = import "MUI2"` — so the message names the
    // position rather than the name, which is the whole of what is missing.
    (
        Code::NotYetImplemented,
        "attributes { outFile = \"a.exe\" }\nimport {}",
    ),
    (Code::UnknownField, r#"attributes { nope = 1 }"#),
    (Code::BadFieldValue, r#"attributes { unicode = "yes" }"#),
    (
        Code::DuplicateBlock,
        "attributes { outFile = \"a.exe\" }\nattributes { name = \"b\" }",
    ),
    (Code::MissingAttribute, r#"attributes { name = "Spine" }"#),
    // The sibling is absent rather than wrong, which is the case that reads
    // least like an error: `compressor` defaults to zlib, and zlib does not read
    // a dictionary size.
    (
        Code::IgnoredSetting,
        r#"attributes { outFile = "a.exe", compressorDictSize = 64 }"#,
    ),
    (
        Code::IncludeNotFound,
        "attributes { outFile = \"a.exe\" }\ninclude(\"missing.lua\")",
    ),
    (
        Code::IncludeCycle,
        "attributes { outFile = \"a.exe\" }\ninclude(\"loop.lua\")",
    ),
    (
        Code::IncludeForm,
        "attributes { outFile = \"a.exe\" }\ninclude(1)",
    ),
    (
        Code::NsisRetired,
        "attributes { outFile = \"a.exe\" }\n\
         installer { section(\"Core\", function()\n\
         strCpy(target, \"C:/App\")\n\
         end), }",
    ),
    (
        Code::WrongPlace,
        "attributes { outFile = \"a.exe\" }\n\
         installer { section(\"Core\", function()\n\
         setSilent(\"silent\")\n\
         end), }",
    ),
];

/// Twenty-one values **live at once** — the last line reads all of them, so no
/// two of their live ranges are disjoint and no two can share a register.
///
/// Twenty-one *declarations* would no longer do it, which is the point: since
/// Phase 3 this diagnostic reports a fact about the program rather than a fact
/// about the allocator (§9-3).
const EXHAUSTED: &str = concat!(
    "attributes { outFile = \"a.exe\" }\n",
    "installer { section(\"Core\", function()\n",
    "local a1 = 1 local a2 = 1 local a3 = 1 local a4 = 1 local a5 = 1\n",
    "local a6 = 1 local a7 = 1 local a8 = 1 local a9 = 1 local a10 = 1\n",
    "local a11 = 1 local a12 = 1 local a13 = 1 local a14 = 1 local a15 = 1\n",
    "local a16 = 1 local a17 = 1 local a18 = 1 local a19 = 1 local a20 = 1\n",
    "local a21 = 1\n",
    "detailPrint(a1 .. a2 .. a3 .. a4 .. a5 .. a6 .. a7 .. a8 .. a9 .. a10\n",
    "  .. a11 .. a12 .. a13 .. a14 .. a15 .. a16 .. a17 .. a18 .. a19 .. a20 .. a21)\n",
    "end), }",
);

/// 1100 characters: verified in Phase 0 to compile clean under `-WX` and
/// measure 1023 at runtime, with no `makensis` diagnostic at any point.
const OVERLONG: &str = concat!(
    r#"detailPrint(""#,
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    r#"")"#,
);

/// Every case compiles against the same two-file in-memory project, so that a
/// case needing a second file has one and no case needs the disk (§9-2).
/// `loop.lua` includes itself, which is the only way to write a cycle small
/// enough to sit in this table.
fn compile(source: &str) -> Diagnostics {
    let sources = BTreeMap::from([
        ("loop.lua".to_string(), "include \"loop.lua\"\n".to_string()),
        (
            "other.lua".to_string(),
            "func(\"helper\", function() detailPrint(\"hi\") end)\n".to_string(),
        ),
    ]);
    let options = installua::Options {
        loader: Loader::Memory(sources),
        ..installua::Options::default()
    };

    let mut diags = Diagnostics::new();
    installua::build_with(source, &options, &mut diags);
    diags
}

/// A parameter's type is a *bound*, not an equation.
///
/// `readMemory`'s address is `Ty::int()` and every integer literal is `nonneg`,
/// so an equality check rejected `readMemory(0, 4)` with *"wants a int, and
/// this is a int"* — a message that cannot be acted on, for a program that is
/// correct. The lattice already knew better: `a.join(b) == b` is what "a fits
/// where b is wanted" means (§15.14).
///
/// The other direction still fails, and must: a `nonneg` position is the one
/// that elides a fixup, so a value merely known to be an `int` does not belong
/// in it.
#[test]
fn a_narrower_type_fits_a_wider_parameter() {
    let widening = compile(
        "attributes { outFile = \"a.exe\" }\n\
         installer { section(\"Core\", function()\n\
         local bytes = readMemory(0, 4)\n\
         detailPrint(bytes)\n\
         end), }",
    );
    assert!(
        !widening.contains(Code::TypeMismatch),
        "a `nonneg` literal belongs in an `int` position:\n{}",
        widening.render("<test>")
    );

    let narrowing = compile(
        "attributes { outFile = \"a.exe\" }\n\
         installer { section(\"Core\", function()\n\
         local index = 1 - 2\n\
         local key = enumRegKey(HKLM, \"Software/Example\", index)\n\
         detailPrint(key)\n\
         end), }",
    );
    assert!(
        narrowing.contains(Code::TypeMismatch),
        "an `int` does not belong in a `nonneg` position"
    );
}

#[test]
fn every_code_has_a_case() {
    let missing: Vec<&str> = Code::ALL
        .iter()
        .filter(|code| !CASES.iter().any(|(cased, _)| cased == *code))
        .map(|code| code.slug())
        .collect();

    assert!(
        missing.is_empty(),
        "these codes have no case in CASES, so nobody has read their wording: {missing:?}"
    );
}

#[test]
fn every_case_raises_its_code() {
    for (code, source) in CASES {
        let diags = compile(source);
        assert!(
            diags.contains(*code),
            "`{}` was not raised by its own case; got {:?}",
            code.slug(),
            diags.iter().map(|d| d.code.slug()).collect::<Vec<_>>()
        );
    }
}

/// PLAN §2: *every rejection names its replacement.* A diagnostic that says
/// only "not supported" sends the user back to the NSIS documentation, which is
/// the thing this compiler exists to stand in front of.
#[test]
fn every_rejection_names_its_replacement() {
    for (code, source) in CASES {
        if matches!(*code, Code::ParseError) {
            // `full-moon` writes these, and its wording is its own.
            continue;
        }

        let diags = compile(source);
        for diagnostic in diags.iter().filter(|d| d.code == *code) {
            assert!(
                !diagnostic.notes.is_empty(),
                "`{}` was raised with no note explaining what to write instead",
                code.slug()
            );
        }
    }
}

/// Collect-don't-throw is only observable when more than one error exists
/// (PLAN §2). Three unrelated mistakes, one run, three diagnostics.
#[test]
fn errors_are_collected_not_thrown() {
    let diags = compile(
        "\
local a = 1 / 2
local b = 2 ^ 8
local c = nil
",
    );

    let codes: Vec<&str> = diags.iter().map(|d| d.code.slug()).collect();
    assert_eq!(
        codes,
        vec!["float-division", "exponentiation", "nil-value"],
        "all three errors should be reported, in source order"
    );
}

/// The two literal checks are warnings rather than errors: both describe output
/// that is legal and probably not what was meant, and neither blocks lowering.
#[test]
fn literal_checks_are_warnings() {
    for code in [Code::DollarInLiteral, Code::OverlongLiteral] {
        let (_, source) = CASES
            .iter()
            .find(|(cased, _)| *cased == code)
            .expect("covered by every_code_has_a_case");
        let diags = compile(source);
        let severity = diags
            .iter()
            .find(|d| d.code == code)
            .map(|d| d.severity)
            .expect("covered by every_case_raises_its_code");
        assert_eq!(severity, Severity::Warning, "{}", code.slug());
    }
}
