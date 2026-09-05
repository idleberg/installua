//! `installua.toml`: what the marker bounds.
//!
//! Unlike `tests/declarations.rs`, which parses text because the loader is a
//! `read_dir` around a parser, everything here *is* about directories: the
//! feature is a walk up a tree, and a test that faked the tree would be
//! testing nothing. So each case builds a monorepo in a scratch directory and
//! asks what a compile from inside it would read.

use std::path::{Path, PathBuf};

use installua::declarations::Declarations;
use installua::project;

/// A plugin nothing ships declared, so an arity found in the result can only
/// have come from the file this test wrote. `outputs` is what varies between
/// the two copies below: it is the number a declaration exists to state.
fn declaration(outputs: &str) -> String {
    format!(
        "[[plugin]]\n\
         name = \"acme\"\n\
         method = \"install\"\n\
         nsis = \"acme::Install\"\n\
         params = [\"path\"]\n\
         outputs = [{outputs}]\n"
    )
}

/// An empty scratch tree, remade each time so a failed run cannot make the
/// next one pass.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("installua-project-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create the scratch directory");
    dir
}

fn write(path: PathBuf, contents: &str) {
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("create the directory");
    std::fs::write(&path, contents).expect("write the file");
}

/// A directory with a marker in it and, when `outputs` says so, a declaration.
fn project(dir: &Path, marker: &str, outputs: Option<&str>) {
    write(dir.join(project::MARKER), marker);
    if let Some(outputs) = outputs {
        write(
            dir.join(".installua/declarations/acme.toml"),
            &declaration(outputs),
        );
    }
}

/// How many values `acme.install` pushes, according to a compile from `dir`.
/// `None` when nothing reachable declared it.
fn arity(dir: &Path) -> Option<usize> {
    let (dirs, problems) = project::declaration_dirs(dir);
    assert!(problems.is_empty(), "{problems:?}");
    let (declarations, problems) = Declarations::load_all(&dirs);
    assert!(problems.is_empty(), "{problems:?}");
    declarations
        .plugin("acme", "install")
        .map(|method| method.outputs.len())
}

/// The feature, in one assertion: a declaration written once at the top of a
/// checkout is what an installer three directories down compiles against.
#[test]
fn a_workspace_declaration_reaches_a_project_under_it() {
    let root = scratch("shared");
    project(&root, "root = true\n", Some("\"string\""));
    project(&root.join("installers/pro"), "", None);

    assert_eq!(arity(&root.join("installers/pro")), Some(1));
}

/// And the project's own copy wins, by the same "later wins" rule that decides
/// between two files in one directory — which is the property that makes the
/// workspace's declaration a *default* rather than a wall.
#[test]
fn the_nearer_declaration_overrides_the_farther() {
    let root = scratch("nearer");
    project(&root, "root = true\n", Some("\"string\""));
    project(
        &root.join("installers/pro"),
        "",
        Some("\"string\", \"string\""),
    );

    assert_eq!(arity(&root.join("installers/pro")), Some(2));
}

/// `root = true` is where the walk stops, and a declaration above it is
/// somebody else's.
#[test]
fn the_walk_stops_at_a_root_marker() {
    let outer = scratch("stops");
    project(&outer, "", Some("\"string\""));
    let inner = outer.join("checkout");
    project(&inner, "root = true\n", None);
    project(&inner.join("installers/pro"), "", None);

    assert_eq!(arity(&inner.join("installers/pro")), None);
}

/// And a checkout is a scope whether or not anyone wrote a marker at its top:
/// without this, a stray `installua.toml` anywhere above `~/src` would join
/// every build under it.
#[test]
fn the_walk_stops_at_a_git_directory() {
    let outer = scratch("git");
    project(&outer, "", Some("\"string\""));
    let inner = outer.join("checkout");
    std::fs::create_dir_all(inner.join(".git")).expect("create the .git directory");
    project(&inner, "", None);
    project(&inner.join("installers/pro"), "", None);

    assert_eq!(arity(&inner.join("installers/pro")), None);
}

/// The property that makes the whole thing discardable: a project with no
/// marker anywhere reads exactly the one directory it read before this
/// existed.
#[test]
fn no_marker_reads_the_source_directory_alone() {
    let root = scratch("bare");
    std::fs::create_dir_all(root.join(".git")).expect("create the .git directory");
    let dir = root.join("installer");
    write(
        dir.join(".installua/declarations/acme.toml"),
        &declaration("\"string\""),
    );

    let (dirs, problems) = project::declaration_dirs(&dir);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(dirs, vec![dir.join(".installua/declarations")]);
    assert_eq!(arity(&dir), Some(1));
}

/// A marker beside the sources is the common case, and it must not make the
/// directory load twice — two loads of one file would report every method in
/// it as declared a second time.
#[test]
fn a_marker_beside_the_sources_loads_its_directory_once() {
    let root = scratch("once");
    project(&root, "root = true\n", Some("\"string\""));

    let (dirs, problems) = project::declaration_dirs(&root);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(dirs.len(), 1, "{dirs:?}");

    let (_, problems) = Declarations::load_all(&dirs);
    assert!(problems.is_empty(), "{problems:?}");
}

/// A typo'd key is a problem rather than a shrug, and the CLI stops on it: a
/// `rooot = true` that silently did nothing would compile the program against
/// a different set of declarations than the author meant.
#[test]
fn an_unknown_key_is_a_problem() {
    let root = scratch("typo");
    project(&root, "[project]\nrooot = true\n", None);

    let (_, problems) = project::declaration_dirs(&root);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].message.contains("rooot"), "{problems:?}");
}

/// The two keys the marker file accepts and this phase carries nowhere are
/// still keys: written today they must not be reported, or every marker
/// written for a later command is an error until it lands.
#[test]
fn entry_and_name_are_accepted() {
    let root = scratch("reserved");
    project(
        &root,
        "[project]\nroot = true\nname = \"pro\"\nentry = \"setup.lua\"\n",
        None,
    );

    let (_, problems) = project::declaration_dirs(&root);
    assert!(problems.is_empty(), "{problems:?}");
}
