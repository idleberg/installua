//! `installua stubs`, checked against the compiler rather than against itself.
//!
//! A golden file would only say the generator is deterministic. What matters is
//! that the editor and the compiler agree: every field the stub offers is a
//! field `attributes {}` accepts, every function it declares is callable, and
//! nothing a `LoweringTarget` would teach appears in it at all.

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

/// The `---@field` names of one `---@class`, **including its parent's**.
///
/// The page classes inherit — `installua.Page.License : installua.Page.Headed`
/// — because the three hooks and the header strip are shared, so a check that
/// stopped at the class's own lines would call `pre` missing from every page.
fn inherited(meta: &str, class: &str) -> Vec<String> {
    let lines: Vec<&str> = meta.lines().collect();
    let at = lines
        .iter()
        .position(|line| {
            line.strip_prefix("---@class (exact) ")
                .or_else(|| line.strip_prefix("---@class "))
                .is_some_and(|rest| rest.split(' ').next() == Some(class))
        })
        .unwrap_or_else(|| panic!("the stub declares no `---@class {class}`"));

    let mut out = match lines[at].split_once(" : ") {
        Some((_, parent)) => inherited(meta, parent.trim()),
        None => Vec::new(),
    };
    for line in &lines[at + 1..] {
        let Some(rest) = line.strip_prefix("---@field ") else {
            break;
        };
        if let Some(name) = rest.split_whitespace().next() {
            out.push(name.trim_end_matches('?').to_string());
        }
    }
    out
}

/// `page.license` ⇒ `installua.Page.License`, read off the `installua.Pages`
/// class rather than spelled a second time here.
fn page_classes(meta: &str) -> Vec<(String, String)> {
    let lines: Vec<&str> = meta.lines().collect();
    let at = lines
        .iter()
        .position(|line| line.trim() == "---@class installua.Pages")
        .expect("the stub declares `installua.Pages`");

    let mut out = Vec::new();
    for line in &lines[at + 1..] {
        let Some(rest) = line.strip_prefix("---@field ") else {
            break;
        };
        let mut words = rest.split_whitespace();
        let name = words.next().expect("a field has a name").to_string();
        // `fun(options?: installua.Page.StartMenu): installua.StartMenu` — the
        // options class is the first type inside the parentheses, and the
        // return type after them is not it.
        let class = rest
            .split_once(": ")
            .and_then(|(_, tail)| tail.split(')').next())
            .unwrap_or_else(|| panic!("`page.{name}` takes no options table"))
            .trim()
            .to_string();
        out.push((name, class));
    }
    out
}

/// A program in which `page.<name> { <field> = "x" }` is the page under test,
/// written into the half that has one.
fn page_program(page: &str, halves: [bool; 2], field: &str) -> String {
    // The start menu page is the one bound to a local, because MUI2 names it
    // from install-time code — so it is declared outside the block and listed
    // inside it.
    let (declare, entry) = if page == "startMenu" {
        ("local it = ".to_string(), "it".to_string())
    } else {
        (String::new(), String::new())
    };
    // `license` is the one page with a required field, and leaving it out is a
    // different error than the one under test.
    let required = if page == "license" {
        "file = \"LICENSE.txt\", "
    } else {
        ""
    };
    let declaration = format!("{declare}page.{page} {{ {required}{field} = \"x\" }}");
    let (block, listed) = match halves {
        [_, true] if page == "confirm" => ("uninstaller", "page.instFiles {}"),
        _ => ("installer", "page.instFiles {}"),
    };
    if entry.is_empty() {
        format!(
            "attributes {{ outFile = \"a.exe\", name = \"a\" }}\n\
             {block} {{ {listed}, {declaration} }}\n"
        )
    } else {
        format!(
            "attributes {{ outFile = \"a.exe\", name = \"a\" }}\n\
             {declaration}\n\
             {block} {{ {listed}, {entry} }}\n"
        )
    }
}

#[test]
fn every_page_the_compiler_has_is_one_the_stub_offers() {
    // The direction that failed silently: `page.startMenu` compiled, and the
    // stub had never heard of it, so the editor called correct code undefined —
    // the failure the stubs exist to prevent, in the one part of the surface
    // that is hand-written.
    let meta = meta();
    let classes = page_classes(&meta);

    for (page, _, expected) in installua::lower::v1_page_surface() {
        let (_, class) = classes
            .iter()
            .find(|(name, _)| name == page)
            .unwrap_or_else(|| panic!("`page.{page}` exists and `installua.Pages` has no field"));
        let offered = inherited(&meta, class);
        for field in expected {
            assert!(
                offered.iter().any(|name| name == field),
                "`page.{page} {{ {field} = … }}` compiles and `{class}` does not offer `{field}`"
            );
        }
    }

    for (name, _) in &classes {
        assert!(
            installua::lower::v1_page_surface()
                .iter()
                .any(|(page, _, _)| page == name),
            "the stub offers `page.{name}` and there is no such page"
        );
    }
}

#[test]
fn every_page_field_the_stub_offers_is_one_the_compiler_accepts() {
    // And the other direction, compiled rather than compared: a field the
    // editor completes and the compiler rejects is worse than no completion at
    // all. Top-level names only — what a field's *table* holds is checked where
    // that table is declared, as `installua.Colors` is below.
    let meta = meta();
    for (page, halves, _) in installua::lower::v1_page_surface() {
        let (_, class) = page_classes(&meta)
            .into_iter()
            .find(|(name, _)| *name == page)
            .expect("the class, which the test above insists on");

        for field in inherited(&meta, &class) {
            // The positional entry, which is a title and not a named field.
            if field.starts_with('[') {
                continue;
            }
            let source = page_program(page, halves, &field);
            let mut diags = Diagnostics::new();
            installua::compile(&source, &mut diags);
            assert!(
                !diags
                    .iter()
                    .any(|diagnostic| diagnostic.code == Code::UnknownField),
                "`{class}` offers `{field}`, which `page.{page}` rejects:\n{}",
                diags.render("stub.lua")
            );
        }
    }
}

#[test]
fn the_two_colours_are_spelled_the_way_the_compiler_spells_them() {
    // The bug this is here for: the stub said `back` and the compiler wants
    // `background`, so an editor completed a name that fails to compile — and
    // the class is reached from a control, a page and a block, which is three
    // places to be wrong at once.
    let meta = meta();
    for field in inherited(&meta, "installua.Colors") {
        let source = format!(
            "attributes {{ outFile = \"a.exe\", name = \"a\" }}\n\
             installer {{ page.instFiles {{}}, headerColors = {{ {field} = \"000000\" }} }}\n"
        );
        let mut diags = Diagnostics::new();
        installua::compile(&source, &mut diags);
        assert!(
            !diags
                .iter()
                .any(|diagnostic| diagnostic.code == Code::UnknownField),
            "`installua.Colors` offers `{field}`, which the compiler rejects:\n{}",
            diags.render("stub.lua")
        );
    }
}

#[test]
fn every_block_field_the_compiler_has_is_one_the_stub_offers() {
    // `installer {}` and `uninstaller {}` are the other hand-written class, and
    // six of their fields had gone missing the same way the start menu page did.
    let meta = meta();
    let offered = inherited(&meta, "installua.Installer");
    for field in installua::lower::v1_installer_fields() {
        assert!(
            offered.iter().any(|name| name == field),
            "`installer {{ {field} = … }}` compiles and `installua.Installer` does not offer it"
        );
    }
    for field in &offered {
        assert!(
            installua::lower::v1_installer_fields().contains(&field.as_str()),
            "the stub offers `installer {{ {field} = … }}` and there is no such field"
        );
    }
}

#[test]
fn every_file_method_reaches_the_stub() {
    // `every_exposed_row_reaches_the_stub` skips the `f:` rows, because a method
    // is not a `function name(`. That skip was the hole: three of the ten were
    // written by hand and the other seven — `readByte`, `seek`, the UTF-16 pair
    // — compiled and completed nowhere. They are generated now, and this is what
    // says so.
    let meta = meta();
    for entry in table::table().iter().filter(|entry| {
        entry.class == table::Class::Exposed
            && entry.installua.is_some_and(|name| name.starts_with("f:"))
    }) {
        let method = entry.installua.unwrap().trim_start_matches("f:");
        assert!(
            meta.contains(&format!("function File:{method}(")),
            "`f:{method}` is exposed and the stub does not declare it"
        );
    }
}

#[test]
fn a_group_is_offered_the_fields_a_group_has() {
    // The drift: `installua.Group` inherited `installua.Section`, so completion
    // offered a heading a `size` to charge and an install type to belong to —
    // and a group has neither, because what it holds is sections and each of
    // those answers for itself.
    let meta = meta();
    let section = inherited(&meta, "installua.Section");
    let group = inherited(&meta, "installua.Group");

    for (field, on_section, on_group) in installua::lower::handle_field_surface() {
        assert_eq!(
            section.iter().any(|name| name == field),
            on_section,
            "`installua.Section` and the compiler disagree about `{field}`"
        );
        assert_eq!(
            group.iter().any(|name| name == field),
            on_group,
            "`installua.Group` and the compiler disagree about `{field}`"
        );
    }
}

#[test]
fn the_declaration_options_are_the_ones_the_compiler_lists() {
    // `description` was missing from both, which is the failure that has no
    // symptom: the field compiles, and an editor that never offers it is how a
    // feature goes unused. The lists are the compiler's own — the same ones its
    // errors read out — so there is one place to add the next option.
    let meta = meta();
    for (class, options, holds) in [
        (
            "installua.SectionOptions",
            installua::lower::SECTION_OPTIONS,
            "body",
        ),
        (
            "installua.GroupOptions",
            installua::lower::GROUP_OPTIONS,
            "sections",
        ),
    ] {
        // `[1]` is the name and `holds` is what the declaration declares:
        // neither is an option, and both belong in the class.
        let offered: Vec<String> = inherited(&meta, class)
            .into_iter()
            .filter(|field| field != "[1]" && field != holds)
            .collect();
        for option in options {
            assert!(
                offered.iter().any(|field| field == option),
                "`{option}` is an option and `{class}` does not offer it"
            );
        }
        for field in &offered {
            assert!(
                options.contains(&field.as_str()),
                "`{class}` offers `{field}` and there is no such option"
            );
        }
    }
}

#[test]
fn the_version_info_fields_are_the_ones_the_compiler_takes() {
    // `file` went the other way: the census had `VIFileVersion` as an attribute
    // and the stub offered it, and the lowerer had never grown the arm — so the
    // one name the editor completed here was the one that did not compile.
    let meta = meta();
    let offered = inherited(&meta, "installua.VersionInfo");
    for field in installua::lower::VERSION_INFO_FIELDS {
        assert!(
            offered.iter().any(|name| name == field),
            "`versionInfo.{field}` compiles and the stub does not offer it"
        );
    }
    for field in &offered {
        assert!(
            installua::lower::VERSION_INFO_FIELDS.contains(&field.as_str()),
            "the stub offers `versionInfo.{field}` and there is no such field"
        );
    }
}

#[test]
fn every_nested_group_the_stub_offers_is_the_one_the_compiler_reads() {
    // The class per group and the field that points at it, both ways. A group
    // is the one construct where a stub can be *plausible* and wrong — the
    // fields all exist, and only the table they are written in is different —
    // so a rename that reached the overlay and not the stub would look right in
    // an editor and fail at the compiler.
    let meta = meta();
    for group in installua::lower::attribute_groups() {
        let mut chars = group.chars();
        let class = format!(
            "installua.{}{}",
            chars
                .next()
                .expect("a group has a name")
                .to_ascii_uppercase(),
            chars.as_str()
        );

        assert!(
            inherited(&meta, "installua.Attributes")
                .iter()
                .any(|field| field == group),
            "`{group} = {{ … }}` compiles and `installua.Attributes` does not offer it"
        );

        let offered = inherited(&meta, &class);
        assert!(!offered.is_empty(), "`{class}` is offered by nothing");
        for field in installua::lower::group_fields(group) {
            assert!(
                offered.iter().any(|name| name == field),
                "`{group}.{field}` compiles and `{class}` does not offer it"
            );
        }
        for field in &offered {
            assert!(
                installua::lower::group_fields(group).contains(&field.as_str()),
                "`{class}` offers `{field}` and there is no `{group}.{field}`"
            );
        }
    }
}

#[test]
fn every_control_field_and_option_the_stub_offers_is_one_the_compiler_accepts() {
    // The two control classes, from the tables the lowering reads. Nothing had
    // drifted here — which is the point of checking: this is the surface a
    // fifteenth control kind would be added to.
    let meta = meta();
    let offered = inherited(&meta, "installua.Control");
    let mut expected: Vec<&str> = installua::lower::control::CONTROL_FIELDS.to_vec();
    expected.sort_unstable();
    let mut found: Vec<&str> = offered.iter().map(String::as_str).collect();
    found.sort_unstable();
    assert_eq!(found, expected, "`installua.Control` is not the seven");

    // A `link` is the kind with every option: it is drawn with text, it is
    // clicked, and it is the one `url` belongs to.
    for field in inherited(&meta, "installua.ControlOptions") {
        if field == "[1]" || field == "items" || field == "image" || field == "onChange" {
            continue;
        }
        let source = format!(
            "attributes {{ outFile = \"a.exe\", name = \"a\" }}\n\
             local it = link {{ \"Site\", {field} = \"1\" }}\n\
             installer {{ page.custom {{ \"Extras\", controls = {{ it }} }}, \
             page.instFiles {{}} }}\n"
        );
        let mut diags = Diagnostics::new();
        installua::compile(&source, &mut diags);
        assert!(
            !diags
                .iter()
                .any(|diagnostic| diagnostic.code == Code::UnknownField),
            "`installua.ControlOptions` offers `{field}`, which a `link` rejects:\n{}",
            diags.render("stub.lua")
        );
    }
}

#[test]
fn nothing_the_compiler_would_reject_is_completed() {
    // A `LoweringTarget` in the stub is the failure the table names: it would
    // put `intCmp(a, b, "yes", "no", "maybe")` back into completion and bypass
    // the condition design on day one. Checked against the retired table, since
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
    // the warning-on-correct-code the stubs exist to prevent.
    let meta = meta();
    for entry in table::table().iter().filter(|entry| {
        entry.class == table::Class::Exposed
            && entry.installua.is_some_and(|n| !n.contains(':'))
            && !entry.bound()
    }) {
        let name = entry.installua.unwrap();
        assert!(
            meta.contains(&format!("function {name}(")),
            "`{name}` is exposed and the stub does not declare it"
        );
    }

    // The same claim for the rows that are not functions. A `Kind::Bound`
    // position means the row is reached through a name: a field of the
    // `installua.Section` class, or `currentInstType` itself. Dropping one of
    // those from the stub is the same warning-on-correct-code failure, one
    // shape over.
    for entry in table::table().iter().filter(|entry| entry.bound()) {
        let name = entry.installua.expect("a bound row has a spelling");
        let declaration = match name.split_once('.') {
            Some((_, field)) => format!("---@field {field} "),
            None => format!("\n{name} = nil\n"),
        };
        assert!(
            meta.contains(&declaration),
            "`{name}` is reached through a name and the stub declares no `{declaration}`"
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
    // `include` is frontend-only, so every name an included file contributes
    // would be an unknown global without this.
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
    // Warnings are failures, as a lint: `deprecated = "deny"` in selene.toml
    // makes the replacement text a failure rather than advice, so a rejection
    // with no replacement is a lint that only says no.
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
        entry.class == table::Class::Exposed
            && entry.installua.is_some_and(|n| !n.contains(':'))
            && !entry.bound()
    }) {
        let name = entry.installua.unwrap();
        assert!(
            std.contains(&format!("\n  {name}:\n")),
            "`{name}` is in the stub and not in the selene std"
        );
        assert!(meta.contains(&format!("function {name}(")));
    }

    // `currentInstType` is a global to selene as much as `$INSTDIR` is, and
    // unlike `$INSTDIR` it is assigned to: `SetCurInstType` *is* the write.
    assert!(
        std.contains("\n  currentInstType:\n    property: full-write\n"),
        "{std}"
    );
}
