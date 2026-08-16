//! §14's census: every `-CMDHELP` line lands in exactly one bucket, and the
//! join between the generated and hand-written halves is total in both
//! directions.
//!
//! This is the test that makes coverage a computed number rather than a
//! document somebody remembers to update. An unclassified command fails the
//! build; so does an overlay row for a command NSIS does not have, and so does
//! an annotation list that is not as long as the parameter list it annotates.
//!
//! Two of these run against the **checked-in snapshot**, so CI needs no NSIS at
//! all, and one runs against whatever `makensis` is on `PATH` and skips
//! cleanly when there is none (§9-7).

use std::collections::BTreeSet;
use std::process::Command;

use installua::table::{self, Class, Dir, Note, overlay};

const SNAPSHOT: &str = include_str!("../tables/cmdhelp-3.12.txt");

#[test]
fn every_command_is_classified() {
    // The default in `join` is `Todo("no overlay row: …")`, which is the right
    // behaviour for a *compiler* meeting a newer NSIS and the wrong one for a
    // repository: here it means somebody added a snapshot row and stopped.
    let unclassified: Vec<&str> = table::table()
        .iter()
        .filter(|entry| {
            matches!(entry.class, Class::Todo(reason) if reason.starts_with("no overlay row"))
        })
        .map(|entry| entry.nsis)
        .collect();

    assert_eq!(
        unclassified,
        Vec::<&str>::new(),
        "these commands have no row in src/table/overlay.rs; every one needs a \
         bucket, including the bucket that means no"
    );
}

#[test]
fn no_overlay_row_describes_a_command_nsis_does_not_have() {
    let known: BTreeSet<&str> = table::table().iter().map(|entry| entry.nsis).collect();
    let orphans: Vec<&str> = overlay::ROWS
        .iter()
        .map(|row| row.nsis)
        .filter(|nsis| !known.contains(nsis))
        .collect();

    assert_eq!(
        orphans,
        Vec::<&str>::new(),
        "these overlay rows match no `-CMDHELP` line: a typo, or a command NSIS \
         removed"
    );
}

#[test]
fn every_exposed_row_annotates_every_parameter() {
    // The failure this prevents is silent: an annotation list one short does
    // not fail to compile, it gives the *last* parameter `Unknown`/`Value`,
    // which reads as "any type, not a path" — exactly the reading that emits a
    // forward slash into a path position.
    let mismatched: Vec<String> = table::table()
        .iter()
        .filter(|entry| entry.class == Class::Exposed)
        .filter_map(|entry| {
            let row = overlay::lookup(entry.nsis)?;
            (row.params.len() != entry.params.len()).then(|| {
                format!(
                    "{}: {} annotations for {} parameters",
                    entry.nsis,
                    row.params.len(),
                    entry.params.len()
                )
            })
        })
        .collect();

    assert_eq!(mismatched, Vec::<String>::new());
}

#[test]
fn every_exposed_row_has_an_installua_name() {
    let anonymous: Vec<&str> = table::table()
        .iter()
        .filter(|entry| entry.class == Class::Exposed && entry.installua.is_none())
        .map(|entry| entry.nsis)
        .collect();

    assert_eq!(anonymous, Vec::<&str>::new());
}

#[test]
fn the_buckets_and_the_classes_are_one_vocabulary() {
    // §14 lists seven buckets and §15.23 lists seven `Class` variants, and the
    // whole point of the ruling is that they are the same seven. A variant
    // whose `bucket()` is not in `BUCKETS` would be invisible to `coverage`.
    let counted: usize = table::census().iter().map(|(_, count)| count).sum();
    assert_eq!(counted, table::table().len());
}

#[test]
fn prose_and_directive_lines_are_classified_as_what_they_are() {
    // `-CMDHELP` prints English instead of a syntax line for five retired
    // commands, and `!` for thirty-seven preprocessor ones. Both are facts
    // about the snapshot, and both are claims about the overlay: a prose row
    // that ended up `Exposed` would be an entry with no parameter model that
    // something tried to call.
    for entry in table::table() {
        match entry.note {
            Note::Prose => assert!(
                matches!(entry.class, Class::Rejected(_)),
                "{} prints prose instead of a syntax line, so it cannot be {}",
                entry.nsis,
                entry.class.bucket()
            ),
            Note::Directive => assert_eq!(
                entry.class,
                Class::Directive,
                "{} is a preprocessor command (§2)",
                entry.nsis
            ),
            _ => {}
        }
    }
}

#[test]
fn the_generated_table_matches_the_snapshot() {
    // Structural rather than textual: `cargo fmt` owns the layout of
    // `generated.rs` and this test owns its content, so a reformat is not a
    // failure and a changed parameter is.
    let parsed = table::cmdhelp::parse(SNAPSHOT);
    let generated = table::generated::SKELETONS;

    assert_eq!(
        parsed.len(),
        generated.len(),
        "src/table/generated.rs has {} rows, the snapshot has {}: regenerate it \
         with `cargo run -q -- table tables/cmdhelp-3.12.txt`",
        generated.len(),
        parsed.len()
    );

    for (parsed, skeleton) in parsed.iter().zip(generated) {
        assert_eq!(parsed.nsis, skeleton.nsis);
        assert_eq!(parsed.note, skeleton.note, "{}", parsed.nsis);
        assert_eq!(
            parsed.params.len(),
            skeleton.params.len(),
            "{} parameters",
            parsed.nsis
        );
        for (parsed, shape) in parsed.params.iter().zip(skeleton.params) {
            assert_eq!(parsed.name, shape.name, "{}", skeleton.nsis);
            assert_eq!(parsed.dir, shape.dir, "{}", skeleton.nsis);
            assert_eq!(parsed.var, shape.var, "{}", skeleton.nsis);
            assert_eq!(parsed.req, shape.req, "{}", skeleton.nsis);
            assert_eq!(parsed.rep, shape.rep, "{}", skeleton.nsis);
            assert_eq!(parsed.members, shape.members, "{}", skeleton.nsis);
        }
        assert_eq!(
            parsed.options.len(),
            skeleton.options.len(),
            "{} options",
            parsed.nsis
        );
        for (parsed, opt) in parsed.options.iter().zip(skeleton.options) {
            assert_eq!(parsed.nsis, opt.nsis, "{}", skeleton.nsis);
            assert_eq!(parsed.value, opt.value, "{}", skeleton.nsis);
            assert_eq!(parsed.after, opt.after, "{}", skeleton.nsis);
        }
    }
}

#[test]
fn the_snapshot_matches_the_local_makensis() {
    // The snapshot ages, and pinning to one NSIS version is exactly what makes
    // the census meaningful — so the check is a *diff against the local
    // toolchain*, skipped when there is none, rather than a version assertion
    // that would go red on a machine with a newer `makensis` and no way to act
    // on it.
    let makensis = std::env::var("MAKENSIS").unwrap_or_else(|_| "makensis".to_string());
    let Ok(output) = Command::new(&makensis).arg("-CMDHELP").output() else {
        eprintln!("skipping: `{makensis}` is not on PATH");
        return;
    };

    let local = String::from_utf8_lossy(&output.stdout);
    let local: Vec<String> = table::cmdhelp::parse(&local)
        .into_iter()
        .map(|command| command.nsis)
        .collect();
    let snapshot: Vec<String> = table::cmdhelp::parse(SNAPSHOT)
        .into_iter()
        .map(|command| command.nsis)
        .collect();

    assert_eq!(
        local, snapshot,
        "the local `makensis` and tables/cmdhelp-3.12.txt disagree: refresh the \
         snapshot, regenerate src/table/generated.rs, and classify whatever is new"
    );
}

#[test]
fn only_an_exposed_row_is_callable() {
    // The lowerer used to read a second table, and this is the invariant that
    // replaced it: `builtins::lookup` answers from the census, so a row is
    // callable exactly when its bucket says so. Without the filter, every
    // `Todo` row in the table would become a silently working call the moment
    // the join found it a shape (§15.23).
    for entry in table::table() {
        let Some(name) = entry.installua else {
            continue;
        };
        assert_eq!(
            installua::builtins::lookup(name).is_some(),
            entry.class == Class::Exposed,
            "`{name}` is `{}` and lookup disagrees",
            entry.class.bucket()
        );
    }
}

#[test]
fn the_surface_is_not_the_nsis_argument_list() {
    // Two of §15.23's distinctions, as one claim about arity: an output is a
    // Lua *return* rather than an argument, and a `Kind::Label` position is the
    // compiler's. `readRegStr` takes three arguments where `ReadRegStr` takes
    // four, and `fileExists` takes one where `IfFileExists` takes three.
    let cases = [("readRegStr", 3..=3), ("fileExists", 1..=1)];
    for (name, expected) in cases {
        let entry = installua::builtins::lookup(name).expect(name);
        assert_eq!(entry.arity(), expected, "{name}");
    }

    // And the brackets are real optionality rather than decoration, which is
    // what makes an `exposed(…)` row worth more than the hand-written one it
    // replaced: `CreateShortcut link target` and its five trailing options are
    // one row.
    let shortcut = installua::builtins::lookup("createShortcut").expect("createShortcut");
    assert_eq!(*shortcut.arity().start(), 2);
    assert!(*shortcut.arity().end() > 2, "{:?}", shortcut.arity());
}

#[test]
fn an_exposed_rows_outputs_come_first() {
    // The emitter builds `[dest] ++ inputs`, so an output anywhere but the
    // front emits its register in the wrong position — `ExecWait $0 "cmd"` for
    // `ExecWait command_line [$(user_var: return value)]`. The failure is
    // silent: the script assembles and does the wrong thing. Until the emitter
    // places outputs by position, this is the invariant that keeps the grind
    // from writing that row by accident.
    // `FileRead handle $(user_var: output) [maxlen]` is the one exception, and
    // it is one because nothing generic emits it: `for line in lines(f)` is a
    // hand-written lowering that places the register itself. A second exception
    // is a reason to fix the emitter rather than to extend this list.
    for entry in table::table() {
        if entry.class != Class::Exposed || entry.nsis == "FileRead" {
            continue;
        }
        let outputs = entry.params.iter().filter(|param| param.dir() == Dir::Out);
        assert_eq!(
            outputs.count(),
            entry
                .params
                .iter()
                .take_while(|param| param.dir() == Dir::Out)
                .count(),
            "{}: the emitter writes destinations first, so every output must be \
             a leading parameter",
            entry.nsis
        );
    }
}

#[test]
fn coverage_matches_its_golden() {
    // The golden is the burndown: a PR that moves twelve commands out of
    // `todo` shows exactly which twelve here (§14).
    let expected = include_str!("golden/coverage.txt");
    assert_eq!(table::coverage(), expected);
}
