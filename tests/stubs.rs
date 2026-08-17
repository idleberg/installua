//! `installua stubs`, checked against the compiler rather than against itself.
//!
//! A golden file would only say the generator is deterministic. What matters is
//! that the editor and the compiler agree: every field the stub offers is a
//! field `attributes {}` accepts, every function it declares is callable, and
//! nothing a `LoweringTarget` would teach appears in it at all (§15.23).

use installua::diag::{Code, Diagnostics};
use installua::{stubs, table};

fn meta() -> String {
    stubs::meta()
}

/// The `---@field` names of one `---@class`.
fn fields(meta: &str, class: &str) -> Vec<String> {
    meta.split(&format!("---@class (exact) {class}\n"))
        .nth(1)
        .unwrap_or_default()
        .lines()
        .take_while(|line| line.starts_with("---@field"))
        .filter_map(|line| line.split_whitespace().nth(1))
        .map(|field| field.trim_end_matches('?').to_string())
        .collect()
}

#[test]
fn every_attribute_the_stub_offers_is_one_the_compiler_accepts() {
    // The block field list is the one name written twice — once in the overlay
    // as an `Attribute` row, once in `stubs::blocks` as a Lua type — and this
    // is what keeps the two honest. A field the editor completes and the
    // compiler rejects is worse than no completion at all.
    let meta = meta();
    let fields = fields(&meta, "installua.Attributes");
    assert!(!fields.is_empty(), "no fields were parsed out of the stub");

    for field in fields {
        // A value of the wrong shape is fine here: `bad-field-value` means the
        // name was recognised, which is the claim under test.
        let source = format!("attributes {{ outFile = \"a.exe\", {field} = \"x\" }}");
        let mut diags = Diagnostics::new();
        installua::compile(&source, &mut diags);

        assert!(
            !diags
                .iter()
                .any(|diagnostic| diagnostic.code == Code::UnknownField),
            "the stub offers `{field}`, which `attributes {{}}` rejects:\n{}",
            diags.render("stub.lua")
        );
    }
}

#[test]
fn nothing_the_compiler_would_reject_is_completed() {
    // A `LoweringTarget` in the stub is the failure §15.23 names: it would put
    // `intCmp(a, b, "yes", "no", "maybe")` back into completion and bypass §8's
    // condition design on day one. Checked against the retired table, since
    // that is the same set spelled the way a user would type it.
    let meta = meta();
    for row in installua::retired::all() {
        let declaration = format!("function {}(", row.installua);
        assert!(
            !meta.contains(&declaration),
            "`{}` is retired and the stub declares it",
            row.installua
        );
    }
}

#[test]
fn every_exposed_row_reaches_the_stub() {
    // The other direction: a row classified `exposed` that the generator drops
    // is a name the compiler accepts and the editor calls undefined, which is
    // the warning-on-correct-code §1 exists to prevent.
    let meta = meta();
    for entry in table::table().iter().filter(|entry| {
        entry.class == table::Class::Exposed && entry.installua.is_some_and(|n| !n.contains(':'))
    }) {
        let name = entry.installua.unwrap();
        assert!(
            meta.contains(&format!("function {name}(")),
            "`{name}` is exposed and the stub does not declare it"
        );
    }
}

#[test]
fn no_alias_is_declared_twice() {
    // `-CMDHELP` spells one parameter two ways — `rootkey` and `root_key` — and
    // two aliases with the same members is a reader's problem, not the
    // compiler's, which is exactly the kind of thing nobody notices.
    let mut names: Vec<&str> = meta()
        .lines()
        .filter_map(|line| line.strip_prefix("---@alias "))
        .map(|name| name.trim())
        .map(|name| Box::leak(name.to_string().into_boxed_str()) as &str)
        .collect();
    let count = names.len();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), count, "duplicate ---@alias in the stub");
}

#[test]
fn every_alias_the_stub_uses_is_one_it_declares() {
    let meta = meta();
    // Aliases and classes are one namespace here: either declares the name.
    // A class line can carry a parent — `---@class (exact) X : Y` — so the
    // name is the first word after the optional `(exact)` and not the rest of
    // the line.
    let declared: Vec<String> = meta
        .lines()
        .filter_map(|line| {
            let rest = line
                .strip_prefix("---@alias ")
                .or_else(|| line.strip_prefix("---@class (exact) "))
                .or_else(|| line.strip_prefix("---@class "))?;
            rest.split_whitespace().next().map(str::to_string)
        })
        .collect();

    for line in meta.lines() {
        let Some(rest) = line
            .strip_prefix("---@param ")
            .or_else(|| line.strip_prefix("---@field "))
            .or_else(|| line.strip_prefix("---@return "))
        else {
            continue;
        };
        // `,`, `{` and `}` because an options table is written inline —
        // `---@param options? { showMode: installua.ShowMode, … }` — and the
        // parentheses because a field can hold a function type:
        // `---@field directory fun(options?: installua.Page.Directory)`. The
        // type is one word inside either of them, like anywhere else.
        for word in rest.split([' ', '|', ',', '{', '}', '(', ')']) {
            // `installua.Manifestsupportedos[]` is a list of the alias, and it
            // is the alias that has to be declared.
            let word = word.trim().trim_end_matches("[]");
            if !word.starts_with("installua.") || word.contains('<') {
                continue;
            }
            assert!(
                declared.iter().any(|name| name == word),
                "the stub uses `{word}` and never declares it"
            );
        }
    }
}

#[test]
fn the_project_meta_declares_what_a_project_declares() {
    // §15.28: `include` is frontend-only, so every name an included file
    // contributes would be an unknown global without this.
    let source = "\
        func(\"kib\", function(bytes) return bytes // 1024 end)\n\
        state = \"fresh-install\"\n\
        attributes { outFile = \"a.exe\" }\n";

    let meta = stubs::project_meta(&[("strings.lua".to_string(), source.to_string())]);

    assert!(meta.starts_with("---@meta\n"), "{meta}");
    assert!(meta.contains("function kib(bytes) end"), "{meta}");
    assert!(meta.contains("state = nil"), "{meta}");
    assert!(meta.contains("-- strings.lua"), "{meta}");
}

#[test]
fn a_source_that_does_not_parse_contributes_nothing_and_fails_nothing() {
    // The generator runs in an editor's workflow, where a file is half-written
    // most of the time. Refusing to produce stubs because one file is mid-edit
    // would break exactly the tool it exists to serve.
    let meta = stubs::project_meta(&[
        ("broken.lua".to_string(), "func(".to_string()),
        (
            "good.lua".to_string(),
            "func(\"ok\", function() end)".to_string(),
        ),
    ]);

    assert!(meta.contains("function ok() end"), "{meta}");
}

#[test]
fn the_selene_std_names_a_replacement_for_every_rejection() {
    // §14's rule, as a lint: `deprecated = "deny"` in selene.toml makes the
    // replacement text a failure rather than advice, so a rejection with no
    // replacement is a lint that only says no.
    let std = stubs::selene_std();
    for name in ["require", "pcall", "print", "pairs", "math.floor"] {
        let entry = std
            .split(&format!("\n  {name}:\n"))
            .nth(1)
            .unwrap_or_else(|| panic!("`{name}` is missing from the selene std"));
        let message = entry
            .lines()
            .find(|line| line.trim_start().starts_with("message:"))
            .unwrap_or_else(|| panic!("`{name}` has no message"));
        assert!(
            message.len() > "      message: ''".len(),
            "{name}: {message}"
        );
    }

    // And the retired NSIS instructions, from the same table the compiler's own
    // diagnostic reads — one text, two tools.
    assert!(std.contains("\n  strCmp:\n"), "{std}");
    assert!(std.contains("\n  intOp:\n"), "{std}");
}

#[test]
fn the_selene_std_and_the_stub_agree_about_what_exists() {
    let std = stubs::selene_std();
    let meta = meta();

    for entry in table::table().iter().filter(|entry| {
        entry.class == table::Class::Exposed && entry.installua.is_some_and(|n| !n.contains(':'))
    }) {
        let name = entry.installua.unwrap();
        assert!(
            std.contains(&format!("\n  {name}:\n")),
            "`{name}` is in the stub and not in the selene std"
        );
        assert!(meta.contains(&format!("function {name}(")));
    }
}
