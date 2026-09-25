//! The position matrix: every kind of value in every position.
//!
//! The census counts the vocabulary, and a word is only ever checked in the
//! form its own test wrote it in. Every bug the pimpbot port found had the
//! other shape — a value that worked in one position and not in the next: a
//! page listed but not bound, a control through a `local` but not a global, a
//! field written on a name but not on a call. This counts the grammar the way
//! `src/mui/` counts MUI2: one cell per kind and position, and the two "no"
//! classes carry their reason.
//!
//! Each position passes the value through and then *uses* it the way its kind
//! is used — so `global` is not "may this be assigned" but "does it still work
//! after it was".

use std::process::Command;

use installua::diag::{Code, Diagnostics};

/// No `Todo`: the first run found no cell that is a real gap rather than a
/// bug, and a generic `not-yet-implemented` is a finding, listed in `KNOWN`.
#[derive(Clone, Copy, Debug)]
enum Class {
    /// No diagnostics, and `makensis -WX` builds the output.
    Accepted,
    /// Every diagnostic has this code, and one says this — the compiler's own
    /// wording is the stated reason.
    Rejected(Code, &'static str),
}

use Class::{Accepted, Rejected};

struct Kind {
    name: &'static str,
    /// Top-level declarations the value needs.
    decls: &'static str,
    /// Entries the value adds to its half's block.
    listed: &'static str,
    /// The expression that names the value.
    value: &'static str,
    /// One field read, one field write and one method call on it.
    read: &'static str,
    write: &'static str,
    call: &'static str,
    /// The ordinary use of the value, with `{}` standing for it.
    uses: &'static str,
    /// A page control only exists inside its own page's callbacks.
    control: bool,
}

const KINDS: &[Kind] = &[
    Kind {
        name: "control",
        decls: "local d = text { \"\", y = 0, height = 12 }",
        listed: "",
        value: "d",
        read: "value",
        write: "value = \"x\"",
        call: ".focus()",
        uses: "local r = {}.value",
        control: true,
    },
    Kind {
        name: "found window",
        decls: "",
        listed: "",
        value: "getDlgItem(HWNDPARENT, 2)",
        read: "enabled",
        write: "enabled = false",
        call: ".focus()",
        uses: "{}.enabled = false",
        control: false,
    },
    Kind {
        name: "section",
        decls: "local core = section(\"Core\", function() end)",
        listed: "core,",
        value: "core",
        read: "selected",
        write: "selected = true",
        call: ".nope()",
        uses: "local r = {}.selected",
        control: false,
    },
    Kind {
        name: "group",
        decls: "local core = section(\"Core\", function() end)\n\
                local tools = group { \"Tools\", sections = { core } }",
        listed: "tools,",
        value: "tools",
        read: "expanded",
        write: "expanded = true",
        call: ".nope()",
        uses: "local r = {}.expanded",
        control: false,
    },
    Kind {
        name: "page",
        decls: "local p = page.directory {}",
        listed: "p,",
        value: "p",
        read: "text",
        write: "text = \"x\"",
        call: ".nope()",
        uses: "local r = {}",
        control: false,
    },
    Kind {
        name: "start menu page",
        decls: "local menu = page.startMenu { defaultFolder = \"A\" }",
        listed: "menu,",
        value: "menu",
        read: "folder",
        write: "folder = \"x\"",
        call: ".write(function() end)",
        uses: "local r = {}.folder",
        control: false,
    },
    Kind {
        name: "string",
        decls: "",
        listed: "",
        value: "(\"s\")",
        read: "len",
        write: "len = 1",
        call: ":upper()",
        uses: "detailPrint({})",
        control: false,
    },
    Kind {
        name: "int",
        decls: "",
        listed: "",
        value: "(1)",
        read: "x",
        write: "x = 1",
        call: ":abs()",
        uses: "local r = {} + 1",
        control: false,
    },
    Kind {
        name: "bool",
        decls: "",
        listed: "",
        value: "(true)",
        read: "x",
        write: "x = 1",
        call: ":nope()",
        uses: "if {} then detailPrint(\"y\") end",
        control: false,
    },
    Kind {
        name: "file handle",
        decls: "",
        listed: "",
        value: "fileOpen(INSTDIR .. \"/a.txt\", \"w\")",
        read: "size",
        write: "size = 1",
        call: ":close()",
        uses: "{}:close()",
        control: false,
    },
];

const POSITIONS: &[&str] = &[
    "local",
    "global",
    "field read",
    "field write",
    "method call",
    "argument",
    "call base",
    "raw",
    "uninstaller",
    "page callback",
];

const SECTION: Class = Rejected(Code::TypeMismatch, "is a section, not a value");
const GROUP: Class = Rejected(Code::TypeMismatch, "is a group, not a value");
const PAGE: Class = Rejected(Code::TypeMismatch, "is a page, not a value");
const PAGE_FIELD: Class = Rejected(Code::UnknownField, "a page has no fields");
const MENU: Class = Rejected(Code::TypeMismatch, "is a start menu page, not a value");
const NO_FIELD: Class = Rejected(Code::UnknownField, "`nope` is not a field");
const FILE_FIELD: Class = Rejected(Code::UnknownField, "a file handle");
const STRING_FIELD: Class = Rejected(Code::TypeMismatch, "a string has no fields");
const INT_FIELD: Class = Rejected(Code::TypeMismatch, "an int has no fields");
const BOOL_FIELD: Class = Rejected(Code::TypeMismatch, "a bool has no fields");

/// One row per kind and position, classified by hand.
#[rustfmt::skip]
const CELLS: &[(&str, &str, Class)] = &[
    ("control", "local", Accepted),
    ("control", "global", Accepted),
    ("control", "field read", Accepted),
    ("control", "field write", Accepted),
    ("control", "method call", Accepted),
    ("control", "argument", Accepted),
    ("control", "call base", Accepted),
    ("control", "raw", Accepted),
    ("control", "uninstaller", Accepted),
    ("control", "page callback", Accepted),

    ("found window", "local", Accepted),
    ("found window", "global", Accepted),
    ("found window", "field read", Rejected(Code::UnknownField, "can be written and not read")),
    ("found window", "field write", Accepted),
    ("found window", "method call", Accepted),
    ("found window", "argument", Accepted),
    ("found window", "call base", Accepted),
    ("found window", "raw", Accepted),
    ("found window", "uninstaller", Accepted),
    ("found window", "page callback", Accepted),

    ("section", "local", SECTION),
    ("section", "global", SECTION),
    ("section", "field read", Accepted),
    ("section", "field write", Accepted),
    ("section", "method call", NO_FIELD),
    ("section", "argument", SECTION),
    ("section", "call base", SECTION),
    ("section", "raw", SECTION),
    ("section", "uninstaller", Accepted),
    ("section", "page callback", Accepted),

    ("group", "local", GROUP),
    ("group", "global", GROUP),
    ("group", "field read", Accepted),
    ("group", "field write", Accepted),
    ("group", "method call", NO_FIELD),
    ("group", "argument", GROUP),
    ("group", "call base", GROUP),
    ("group", "raw", GROUP),
    ("group", "uninstaller", Accepted),
    ("group", "page callback", Accepted),

    ("page", "local", PAGE),
    ("page", "global", PAGE),
    ("page", "field read", PAGE_FIELD),
    ("page", "field write", PAGE_FIELD),
    ("page", "method call", PAGE_FIELD),
    ("page", "argument", PAGE),
    ("page", "call base", PAGE),
    ("page", "raw", PAGE),
    ("page", "uninstaller", PAGE),
    ("page", "page callback", PAGE),

    ("start menu page", "local", MENU),
    ("start menu page", "global", MENU),
    ("start menu page", "field read", Accepted),
    ("start menu page", "field write", Rejected(Code::BadFieldValue, "is not something to assign to")),
    ("start menu page", "method call", Accepted),
    ("start menu page", "argument", MENU),
    ("start menu page", "call base", MENU),
    ("start menu page", "raw", MENU),
    ("start menu page", "uninstaller", Rejected(Code::UnknownField, "there is no uninstaller `startMenu` page")),
    ("start menu page", "page callback", Accepted),

    ("string", "local", Accepted),
    ("string", "global", Accepted),
    ("string", "field read", STRING_FIELD),
    ("string", "field write", STRING_FIELD),
    ("string", "method call", Rejected(Code::TypeMismatch, "a string has no methods")),
    ("string", "argument", Accepted),
    ("string", "call base", Accepted),
    ("string", "raw", Accepted),
    ("string", "uninstaller", Accepted),
    ("string", "page callback", Accepted),

    ("int", "local", Accepted),
    ("int", "global", Accepted),
    ("int", "field read", INT_FIELD),
    ("int", "field write", INT_FIELD),
    ("int", "method call", Rejected(Code::TypeMismatch, "an int has no methods")),
    ("int", "argument", Accepted),
    ("int", "call base", Accepted),
    ("int", "raw", Accepted),
    ("int", "uninstaller", Accepted),
    ("int", "page callback", Accepted),

    ("bool", "local", Accepted),
    ("bool", "global", Accepted),
    ("bool", "field read", BOOL_FIELD),
    ("bool", "field write", BOOL_FIELD),
    ("bool", "method call", Rejected(Code::TypeMismatch, "a bool has no methods")),
    ("bool", "argument", Accepted),
    ("bool", "call base", Accepted),
    ("bool", "raw", Accepted),
    ("bool", "uninstaller", Accepted),
    ("bool", "page callback", Accepted),

    ("file handle", "local", Accepted),
    ("file handle", "global", Accepted),
    ("file handle", "field read", FILE_FIELD),
    ("file handle", "field write", FILE_FIELD),
    ("file handle", "method call", Accepted),
    ("file handle", "argument", Accepted),
    ("file handle", "call base", Accepted),
    ("file handle", "raw", Accepted),
    ("file handle", "uninstaller", Accepted),
    ("file handle", "page callback", Accepted),
];

/// Open findings (BUG-HUNTING.md, step 3): cells that do not yet match their
/// class. Each must still mismatch, so a fix fails the test until its cell
/// leaves this list.
#[rustfmt::skip]
const KNOWN: &[(&str, &str)] = &[
    // 1. a declaration used as a value is rightly refused, and then errors again
    //    where the value lands: `x` undefined, a field "of a control", or a
    //    `func` that "returns 0 values"
    ("section", "local"),
    ("section", "global"),
    ("section", "argument"),
    ("section", "call base"),
    ("group", "local"),
    ("group", "global"),
    ("group", "argument"),
    ("group", "call base"),
    ("page", "local"),
    ("page", "call base"),
    ("start menu page", "local"),
    ("start menu page", "global"),
    ("start menu page", "argument"),
    ("start menu page", "call base"),
    // 2. a field on a file handle is reported as a field of a control
    ("file handle", "field read"),
    ("file handle", "field write"),
    // 3. a field on a scalar is `not-yet-implemented`
    ("string", "field read"),
    ("string", "field write"),
    ("int", "field read"),
    ("int", "field write"),
    ("bool", "field read"),
    ("bool", "field write"),
    // 4. a method on a section, group or page is `not-yet-implemented`
    ("section", "method call"),
    ("group", "method call"),
    ("page", "method call"),
    // 5. "a int has no methods"
    ("int", "method call"),
];

fn uses(kind: &Kind, value: &str) -> String {
    kind.uses.replace("{}", value)
}

/// The whole program for one cell.
fn program(kind: &Kind, position: &str) -> String {
    let v = kind.value;
    let (extra, body) = match position {
        "local" => (String::new(), format!("local x = {v}\n{}", uses(kind, "x"))),
        "global" => (String::new(), format!("g = {v}\n{}", uses(kind, "g"))),
        "field read" => (String::new(), format!("local r = {v}.{}", kind.read)),
        "field write" => (String::new(), format!("{v}.{}", kind.write)),
        "method call" => (String::new(), format!("{v}{}", kind.call)),
        "argument" => (
            format!("func(\"take\", function(x)\n{}\nend)", uses(kind, "x")),
            format!("take({v})"),
        ),
        "call base" => (
            format!("func(\"make\", function() return {v} end)"),
            uses(kind, "make()"),
        ),
        "raw" => (
            String::new(),
            format!("g = {v}\nraw [[ DetailPrint \"$g\" ]]"),
        ),
        "uninstaller" | "page callback" => (String::new(), uses(kind, v)),
        other => panic!("no position {other}"),
    };

    let callback = position == "page callback";
    let (page, section) = if kind.control {
        let at = if callback { "leave" } else { "show" };
        (
            format!("page.custom {{ controls = {{ d }}, {at} = function()\n{body}\nend }},"),
            "section(\"Main\", function() end),".to_string(),
        )
    } else if callback {
        (
            format!("page.directory {{ leave = function()\n{body}\nend }},"),
            "section(\"Main\", function() end),".to_string(),
        )
    } else {
        (
            String::new(),
            format!("section(\"Main\", function()\n{body}\nend),"),
        )
    };
    let half = format!("{}\n{page}\npage.instFiles {{}},\n{section}", kind.listed);
    let blocks = if position == "uninstaller" {
        format!(
            "installer {{ page.instFiles {{}}, section(\"Setup\", function() \
             writeUninstaller(INSTDIR .. \"/u.exe\") end) }}\n\
             uninstaller {{\n{half}\n}}"
        )
    } else {
        format!("installer {{\n{half}\n}}")
    };

    format!(
        "attributes {{ outFile = \"a.exe\", name = \"a\" }}\n{}\n{extra}\n{blocks}\n",
        kind.decls
    )
}

fn kind(name: &str) -> Option<&'static Kind> {
    KINDS.iter().find(|kind| kind.name == name)
}

/// What a cell does today, or `None` when it matches its class.
fn mismatch(kind: &Kind, position: &str, class: Class) -> Option<String> {
    let mut diags = Diagnostics::new();
    installua::build(&program(kind, position), &mut diags);
    let got: Vec<String> = diags
        .iter()
        .map(|d| format!("{} {}", d.code.slug(), d.message))
        .collect();
    let matches = match class {
        Accepted => diags.is_empty(),
        Rejected(code, why) => {
            !diags.is_empty()
                && diags.iter().all(|d| d.code == code)
                && diags.iter().any(|d| d.message.contains(why))
        }
    };
    (!matches).then(|| {
        if got.is_empty() {
            "accepted".to_string()
        } else {
            got.join("; ")
        }
    })
}

/// A new kind or position fails here until every cell for it is classified.
#[test]
fn every_cell_is_classified_once() {
    let mut problems = Vec::new();
    for kind in KINDS {
        for position in POSITIONS {
            let rows = CELLS
                .iter()
                .filter(|(k, p, _)| *k == kind.name && p == position)
                .count();
            if rows != 1 {
                problems.push(format!("{} / {position}: {rows} rows", kind.name));
            }
        }
    }
    for (k, p, _) in CELLS {
        if kind(k).is_none() || !POSITIONS.contains(p) {
            problems.push(format!("{k} / {p}: no such kind or position"));
        }
    }
    for (k, p) in KNOWN {
        if !CELLS.iter().any(|(ck, cp, _)| ck == k && cp == p) {
            problems.push(format!("{k} / {p}: known, but no such cell"));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn every_cell_matches_its_class() {
    let mut problems = Vec::new();
    for &(k, position, class) in CELLS {
        let kind = kind(k).expect("a kind");
        let known = KNOWN.contains(&(k, position));
        match (mismatch(kind, position, class), known) {
            (Some(got), false) => problems.push(format!("{k} / {position}: {got}")),
            (None, true) => problems.push(format!("{k} / {position}: fixed, drop it from KNOWN")),
            _ => {}
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// Tier 3 over every accepted cell, with an empty warning allowlist.
#[test]
fn accepted_cells_assemble_under_wx() {
    let Some(makensis) = makensis() else {
        eprintln!("skipping: `makensis` is not installed");
        return;
    };
    let directory =
        std::env::temp_dir().join(format!("installua-positions-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("temp directory");

    let mut problems = Vec::new();
    for &(k, position, class) in CELLS {
        if !matches!(class, Accepted) {
            continue;
        }
        let mut diags = Diagnostics::new();
        let Some(output) =
            installua::build(&program(kind(k).expect("a kind"), position), &mut diags)
        else {
            continue; // every_cell_matches_its_class reports it
        };
        let script = directory.join("a.nsi");
        std::fs::write(&script, output).expect("write the script");
        let run = Command::new(&makensis)
            .arg("-WX")
            .arg(&script)
            .current_dir(&directory)
            .output()
            .expect("run makensis");
        if !run.status.success() {
            problems.push(format!(
                "{k} / {position}:\n{}",
                String::from_utf8_lossy(&run.stdout)
            ));
        }
    }
    let _ = std::fs::remove_dir_all(&directory);
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

fn makensis() -> Option<String> {
    let name = std::env::var("MAKENSIS").unwrap_or_else(|_| "makensis".to_string());
    Command::new(&name)
        .arg("-VERSION")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|_| name)
}
