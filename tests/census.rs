//! The census: every `-CMDHELP` line lands in exactly one bucket, and the join
//! between the generated and hand-written halves is total in both directions.
//!
//! This is the test that makes coverage a computed number rather than a
//! document somebody remembers to update. An unclassified command fails the
//! build; so does an overlay row for a command NSIS does not have, and so does
//! an annotation list that is not as long as the parameter list it annotates.
//!
//! Two of these run against the **checked-in snapshot**, so CI needs no NSIS at
//! all, and one runs against whatever `makensis` is on `PATH` and skips cleanly
//! when there is none.

use std::collections::BTreeSet;
use std::process::Command;

use installua::table::{self, Class, Dir, Kind, Note, overlay};

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
fn every_exposed_row_judges_every_flag() {
    // The same failure as the annotations above, one column over: a judgement
    // list one short leaves the *last* flag `Unoffered`, which reads as "nobody
    // has looked at it" when in fact nobody can any more — and unlike a missing
    // type, nothing downstream goes wrong loudly.
    //
    // Saying "not this one, and here is why" is a legal answer. `File`'s `/x`
    // takes a value and repeats; `MessageBox`'s `/SD` belongs with
    // `messageBox`. What is refused is silence.
    let mismatched: Vec<String> = table::table()
        .iter()
        .filter(|entry| entry.class == Class::Exposed)
        .filter_map(|entry| {
            let row = overlay::lookup(entry.nsis)?;
            (row.options.len() != entry.options.len()).then(|| {
                format!(
                    "{}: {} judgements for {} flags",
                    entry.nsis,
                    row.options.len(),
                    entry.options.len()
                )
            })
        })
        .collect();

    assert_eq!(mismatched, Vec::<String>::new());

    // One name, one thing. The two halves of the options table are looked up in
    // order, so a flag sharing a name with an optional position would be
    // silently unreachable rather than an error.
    for entry in table::table() {
        let names = entry.option_names();
        let unique: BTreeSet<&&str> = names.iter().collect();
        assert_eq!(
            names.len(),
            unique.len(),
            "{}: two options share a name: {names:?}",
            entry.nsis
        );
    }
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
    // There are seven buckets and seven `Class` variants, and the whole point
    // is that they are the same seven. A variant whose `bucket()` is not in
    // `BUCKETS` would be invisible to `coverage`.
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
                "{} is a preprocessor command",
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
         with `cargo run -q -- generate table tables/cmdhelp-3.12.txt`",
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
            assert_eq!(parsed.open, shape.open, "{}", skeleton.nsis);
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

/// The five commands that decide what `-CMDHELP`'s spaces mean.
///
/// A space separates positions and also joins the words of one name, and the
/// parser tells them apart by what else is in the bracket. These five are the
/// evidence for that rule and the only thing that would catch it drifting: two
/// where the words are one name, two where they are separate positions, and one
/// where the parentheses are a sentence.
#[test]
fn the_notation_is_not_the_parameter_list() {
    let by_name = |nsis: &str| {
        table::cmdhelp::parse(SNAPSHOT)
            .into_iter()
            .find(|parsed| parsed.nsis == nsis)
            .unwrap_or_else(|| panic!("{nsis} is in the snapshot"))
    };
    let names = |nsis: &str| {
        by_name(nsis)
            .params
            .iter()
            .map(|param| param.name.clone())
            .collect::<Vec<_>>()
    };

    // `[text (can contain $0)] [text without ignore (can contain $0)]`: the
    // parenthesised half is commentary and the unparenthesised half is a name.
    assert_eq!(names("FileErrorText"), ["text", "text_without_ignore"]);
    assert_eq!(names("CompletedText"), ["completed_text"]);

    // `[height weight /ITALIC /UNDERLINE /STRIKE]` goes on to list flags, so
    // its words are positions; `[return_check label_to_goto_if_equal […]]`
    // spells its names with underscores and opens a further optional.
    assert!(names("CreateFont").contains(&"height".to_string()));
    assert!(names("CreateFont").contains(&"weight".to_string()));
    assert!(names("MessageBox").contains(&"return_check".to_string()));
    assert!(
        names("MessageBox").contains(&"label_to_goto_if_equal".to_string()),
        "{:?}",
        names("MessageBox")
    );

    // `none|all|…|Win10|{GUID}`: seven keywords and a placeholder, which is a
    // set worth offering and not one worth enforcing.
    let supported = by_name("ManifestSupportedOS");
    let param = &supported.params[0];
    assert!(param.open, "the placeholder opens the set");
    assert_eq!(
        param.members,
        ["none", "all", "WinVista", "Win7", "Win8", "Win8.1", "Win10"]
    );

    // The braces around `flag={smooth|colored}` wrap the whole list instead,
    // and mean nothing at all.
    let gradient = by_name("BGGradient");
    assert!(
        gradient.params.iter().all(|param| !param.open),
        "a wrapped list is not a placeholder"
    );
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
    // the join found it a shape.
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
    // Two of the table's distinctions, as one claim about arity: an output is a
    // Lua *return* rather than an argument, and a `Kind::Label` position is the
    // compiler's. `readRegStr` takes three arguments where `ReadRegStr` takes
    // four, and `fileExists` takes one where `IfFileExists` takes three.
    let cases = [("readRegStr", 3..=3), ("fileExists", 1..=1)];
    for (name, expected) in cases {
        let entry = installua::builtins::lookup(name).expect(name);
        assert_eq!(entry.arity(), expected, "{name}");
    }

    // And the brackets are real optionality, which is now said by *name* rather
    // than by counting: `CreateShortcut` takes exactly two arguments and six
    // named options, so no call site has to know that the description is the
    // ninth position.
    let shortcut = installua::builtins::lookup("createShortcut").expect("createShortcut");
    assert_eq!(shortcut.arity(), 2..=2);
    let names: Vec<&str> = shortcut.fields().map(|(field, _)| field.name).collect();
    assert_eq!(
        names,
        [
            "parameters",
            "iconFile",
            "iconIndex",
            "showMode",
            "hotkey",
            "comment"
        ]
    );

    // A repeated tail is the one place a count is still the caller's.
    let file = installua::builtins::lookup("file").expect("file");
    assert_eq!(file.arity(), 1..=usize::MAX);
}

#[test]
fn every_optional_position_is_reachable() {
    // An optional position reachable neither positionally nor by name is a
    // position no call site can write, and the census is where that is a
    // failure rather than a mystery.
    //
    // Which of the two it is comes from the row's shape rather than from the
    // overlay: a single trailing optional is unambiguous and stays an argument
    // (`abort("stopped")`), and anything else is a named field. The name is
    // written either way, because it is what the stub and the error message
    // call the position.
    let mut unreachable: Vec<String> = Vec::new();
    for entry in table::table() {
        if entry.class != Class::Exposed {
            continue;
        }
        let tail = entry.tail_optional().map(|param| param.shape.name);
        for param in entry.surface() {
            let reachable = param.required()
                || tail == Some(param.shape.name)
                || entry
                    .fields()
                    .any(|(_, named)| named.shape.name == param.shape.name);
            if !reachable {
                unreachable.push(format!("{}: {}", entry.nsis, param.shape.name));
            }
        }
    }
    assert_eq!(unreachable, Vec::<String>::new());

    // And the two shapes are really two: `Abort [message]` counts, and
    // `CreateShortcut`'s six optionals cannot.
    let abort = installua::builtins::lookup("abort").expect("abort");
    assert_eq!(abort.arity(), 0..=1);
    assert_eq!(abort.fields().count(), 0);
    let shortcut = installua::builtins::lookup("createShortcut").expect("createShortcut");
    assert!(shortcut.tail_optional().is_none());
    assert_eq!(shortcut.fields().count(), 6);
}

#[test]
fn an_optional_input_before_anything_else_can_be_filled() {
    // This replaces `an_exposed_rows_outputs_come_first`, which refused any row
    // whose output was not leading because the emitter concatenated
    // `[dest] ++ inputs`. The emitter now places by table position, so the
    // ordering restriction is gone and a narrower one takes its place.
    //
    // Reaching an output means writing every position before it, including the
    // optional ones the caller declined. `FileSeek handle offset [mode]
    // [$(user_var: new position)]` is the case: NSIS parses `FileSeek $1 0 $0`
    // with `$0` as the *mode* and rejects the line, so the compiler has to
    // supply a mode nobody named. It can only do that from `Ann::fill`.
    //
    // A row that needs a fill and has none emits a line one token short, which
    // NSIS may accept — `FileReadByte $1 $0` is well-formed whichever way round
    // the registers go — so this is checked here rather than left to tier 3.
    //
    // Now that the optional positions are named, an output is no longer the
    // only thing that can follow one: `createShortcut(link, target, { comment =
    // … })` writes the ninth position and declines the four before it. So the
    // rule is every optional input that precedes *any* emitted position, with
    // one exception — a `toggle` is a `/FLAG`, which NSIS tells from the next
    // argument lexically rather than by counting.
    for entry in table::table() {
        if entry.class != Class::Exposed {
            continue;
        }
        let emitted = |param: &table::Param| {
            param.dir() == Dir::Out || (param.kind != Kind::Label && param.kind != Kind::Fused)
        };
        let last = entry.params.iter().rposition(emitted);
        let Some(last) = last else {
            continue;
        };
        for param in &entry.params[..last] {
            let toggle = param.field.is_some_and(|field| field.toggle.is_some());
            assert!(
                param.required()
                    || param.fill.is_some()
                    || toggle
                    || param.kind == Kind::Label
                    || param.kind == Kind::Fused,
                "{}: `{}` is optional and something after it is emitted, so the emitter \
                 needs a fill for it — reaching that position means writing this one",
                entry.nsis,
                param.shape.name
            );
        }
    }
}

#[test]
fn coverage_matches_its_golden() {
    // The golden is the burndown: a PR that moves twelve commands out of `todo`
    // shows exactly which twelve here.
    let expected = include_str!("golden/coverage.txt");
    assert_eq!(table::coverage(), expected);
}
