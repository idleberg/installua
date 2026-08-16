//! The retired-instruction table (PLAN §5), row by row.
//!
//! PLAN's exit criterion for it: *every retired-instruction row has a test
//! asserting its diagnostic.* Written as one table-driven test over
//! [`installua::retired::all`] rather than as a list of cases, because a list
//! is a thing to forget to add to — the same argument §14 makes for the census
//! and for overlay examples.

use installua::diag::{Code, Diagnostics};
use installua::retired;

/// Wraps a call to `name` in the smallest program that reaches lowering.
fn program(name: &str) -> String {
    format!(
        "attributes {{ outFile = \"a.exe\" }}\n\
         installer {{ section(\"Core\", function()\n\
         {name}(\"a\", \"b\")\n\
         end), }}"
    )
}

#[test]
fn every_retired_row_produces_its_diagnostic() {
    for row in retired::all() {
        let mut diags = Diagnostics::new();
        installua::compile(&program(&row.installua), &mut diags);

        let rendered = diags.render("retired.lua");
        let retired: Vec<_> = diags
            .iter()
            .filter(|diagnostic| diagnostic.code == Code::NsisRetired)
            .collect();

        assert_eq!(
            retired.len(),
            1,
            "`{}` should raise exactly one nsis-retired, got:\n{rendered}",
            row.installua
        );

        // Every rejection names its replacement (§2), and this is the table
        // where that rule is mechanically checkable: the note *is* the class's
        // own text, so a row added to the overlay with an empty reason fails
        // here rather than shipping a diagnostic that says nothing.
        assert!(
            retired[0]
                .notes
                .iter()
                .any(|note| note.contains(row.instead)),
            "`{}` does not name its replacement:\n{rendered}",
            row.installua
        );
        assert!(!row.instead.is_empty());
    }
}

#[test]
fn the_users_own_capitalisation_is_quoted_back() {
    // The table is keyed on `strCmp` and matched case-insensitively, so an NSIS
    // user who typed `StrCmp` gets the row — and the message has to quote what
    // they wrote, or it reads as a correction to a name they did not use.
    let mut diags = Diagnostics::new();
    installua::compile(&program("StrCmp"), &mut diags);

    let messages: Vec<&str> = diags
        .iter()
        .filter(|diagnostic| diagnostic.code == Code::NsisRetired)
        .map(|diagnostic| diagnostic.message.as_str())
        .collect();

    assert_eq!(messages, vec!["`StrCmp` is not a function here"]);
}

#[test]
fn a_retired_name_is_not_an_unknown_name() {
    // The failure this prevents: `strCmp` falling through to `undefined-name`,
    // which tells an NSIS user the compiler has never heard of the instruction
    // they use most (§5).
    let mut diags = Diagnostics::new();
    installua::compile(&program("intOp"), &mut diags);

    assert!(
        !diags
            .iter()
            .any(|diagnostic| diagnostic.code == Code::UndefinedName),
        "{}",
        diags.render("retired.lua")
    );
}

#[test]
fn nothing_retired_is_also_callable() {
    // A name cannot be both retired and exposed: the diagnostic would fire on a
    // name that works, or the call would win and the diagnostic would be dead
    // code. Both halves come from the one table, so this is a check that the
    // classes are disjoint in practice and not only in principle.
    for row in retired::all() {
        assert!(
            installua::builtins::lookup(&row.installua).is_none(),
            "`{}` is both retired and exposed",
            row.installua
        );
    }
}
