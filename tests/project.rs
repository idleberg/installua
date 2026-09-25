//! `installua.toml`: what the workspace bounds, and what it lists.
//!
//! Unlike `tests/declarations.rs`, which parses text because the loader is a
//! `read_dir` around a parser, everything here *is* about directories: the
//! feature is a walk up a tree, and a test that faked the tree would be
//! testing nothing. So each case builds a monorepo in a scratch directory and
//! asks what a compile from inside it would read.

use std::path::{Path, PathBuf};

use installua::declarations::{self, Declarations};
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

/// A directory with an `installua.toml` in it and, when `outputs` says so, a
/// declaration.
fn workspace(dir: &Path, marker: &str, outputs: Option<&str>) {
    write(dir.join(project::MARKER), marker);
    declare(dir, outputs);
}

/// A directory with, when `outputs` says so, a declaration — and no
/// `installua.toml`, which is what a project under a workspace has.
fn declare(dir: &Path, outputs: Option<&str>) {
    std::fs::create_dir_all(dir).expect("create the directory");
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
    workspace(&root, "", Some("\"string\""));
    declare(&root.join("installers/pro"), None);

    assert_eq!(arity(&root.join("installers/pro")), Some(1));
}

/// And the project's own copy wins, by the same "later wins" rule that decides
/// between two files in one directory — which is the property that makes the
/// workspace's declaration a *default* rather than a wall.
#[test]
fn the_nearer_declaration_overrides_the_farther() {
    let root = scratch("nearer");
    workspace(&root, "", Some("\"string\""));
    declare(&root.join("installers/pro"), Some("\"string\", \"string\""));

    assert_eq!(arity(&root.join("installers/pro")), Some(2));
}

/// The nearest file is the workspace and the walk stops at it: there is no
/// cascade, so a declaration beside an `installua.toml` further up is
/// somebody else's.
#[test]
fn the_nearest_file_is_the_workspace() {
    let outer = scratch("nearest");
    workspace(&outer, "", Some("\"string\""));
    let inner = outer.join("checkout");
    workspace(&inner, "", None);
    declare(&inner.join("installers/pro"), None);

    assert_eq!(arity(&inner.join("installers/pro")), None);
}

/// And a checkout is a scope whether or not anyone wrote a file at its top:
/// without this, a stray `installua.toml` anywhere above `~/src` would join
/// every build under it.
#[test]
fn the_walk_stops_at_a_git_directory() {
    let outer = scratch("git");
    workspace(&outer, "", Some("\"string\""));
    let inner = outer.join("checkout");
    std::fs::create_dir_all(inner.join(".git")).expect("create the .git directory");
    declare(&inner.join("installers/pro"), None);

    assert_eq!(arity(&inner.join("installers/pro")), None);
}

/// The property that makes the whole thing discardable: a project with no
/// `installua.toml` anywhere reads exactly the one directory it read before
/// this existed.
#[test]
fn no_file_reads_the_source_directory_alone() {
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

/// A file beside the sources is the common case, and it must not make the
/// directory load twice — two loads of one file would report every method in
/// it as declared a second time.
#[test]
fn a_file_beside_the_sources_loads_its_directory_once() {
    let root = scratch("once");
    workspace(&root, "", Some("\"string\""));

    let (dirs, problems) = project::declaration_dirs(&root);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(dirs.len(), 1, "{dirs:?}");

    let (_, problems) = Declarations::load_all(&dirs);
    assert!(problems.is_empty(), "{problems:?}");
}

/// The workspace from `dir`, asserting there was one.
fn found(dir: &Path) -> (project::Workspace, Vec<declarations::Problem>) {
    let (workspace, problems) = project::find(dir);
    (workspace.expect("no workspace found"), problems)
}

/// The messages, one per problem.
fn messages(problems: &[declarations::Problem]) -> Vec<&str> {
    problems
        .iter()
        .map(|problem| problem.message.as_str())
        .collect()
}

/// What the file is for, read back: every project, in the order written.
#[test]
fn projects_are_read_in_order() {
    let root = scratch("projects");
    workspace(
        &root,
        "[[project]]\nname = \"pro\"\nentry = \"installers/pro/install.lua\"\n\n\
         [[project]] # the cheap one\nname = \"lite\"\nentry = \"installers/lite/install.lua\"\n",
        None,
    );

    let (workspace, problems) = found(&root);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(
        workspace.projects,
        vec![
            project::Project {
                name: Some("pro".into()),
                entry: "installers/pro/install.lua".into(),
            },
            project::Project {
                name: Some("lite".into()),
                entry: "installers/lite/install.lua".into(),
            },
        ]
    );
}

/// An entry is relative to the file that names it, not to wherever the search
/// started — and spelt from there, so the path a message prints is one the
/// user could have typed.
#[test]
fn an_entry_is_relative_to_the_workspace() {
    let root = scratch("entry");
    workspace(
        &root,
        "[[project]]\nentry = \"installers/pro/install.lua\"\n",
        None,
    );
    declare(&root.join("installers/pro"), None);

    let start = root.join("installers/pro");
    let (workspace, problems) = found(&start);
    assert!(problems.is_empty(), "{problems:?}");
    let entry = workspace.entry(workspace.select(None).expect("one project"));
    assert_eq!(entry, root.join("installers/pro/install.lua"));
}

/// One project needs no name and no `-p`; with none, there is nothing to
/// build and the message says what would fix it.
#[test]
fn one_project_is_selected_without_a_name() {
    let root = scratch("one");
    workspace(&root, "[[project]]\nentry = \"setup.lua\"\n", None);
    let (workspace, problems) = found(&root);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(
        workspace.select(None).map(|project| project.entry.as_str()),
        Ok("setup.lua")
    );

    let empty = scratch("none");
    self::workspace(&empty, "", None);
    let (workspace, problems) = found(&empty);
    assert!(problems.is_empty(), "{problems:?}");
    let refused = workspace
        .select(None)
        .expect_err("an empty file selected something");
    assert!(refused.contains("lists no projects"), "{refused}");
}

/// Several projects and no `-p` is a refusal that lists the names, never the
/// first one: building something the user did not ask for is the worse
/// failure. A name that is not there is refused the same way.
#[test]
fn several_projects_need_a_name() {
    let root = scratch("several");
    workspace(
        &root,
        "[[project]]\nname = \"pro\"\nentry = \"pro.lua\"\n\
         [[project]]\nname = \"lite\"\nentry = \"lite.lua\"\n",
        None,
    );
    let (workspace, problems) = found(&root);
    assert!(problems.is_empty(), "{problems:?}");

    let refused = workspace.select(None).expect_err("picked one of two");
    assert!(refused.contains("`pro`, `lite`"), "{refused}");
    assert_eq!(
        workspace
            .select(Some("lite"))
            .map(|project| project.entry.as_str()),
        Ok("lite.lua")
    );
    let refused = workspace
        .select(Some("free"))
        .expect_err("found a missing name");
    assert!(refused.contains("`pro`, `lite`"), "{refused}");
}

/// The rules about the list rather than a line: a project without a program,
/// an unnamed one among several, and two with one name are each a problem.
#[test]
fn a_malformed_list_is_a_problem() {
    let root = scratch("list");
    workspace(
        &root,
        "[[project]]\nname = \"pro\"\n\
         [[project]]\nentry = \"lite.lua\"\n\
         [[project]]\nname = \"pro\"\nentry = \"again.lua\"\n",
        None,
    );

    let (workspace, problems) = found(&root);
    assert_eq!(
        messages(&problems),
        vec![
            "this `[[project]]` needs an `entry`: the `.lua` file it builds",
            "this `[[project]]` needs a `name`: the file lists several, and `-p` picks \
             one by it",
            "a second project is named `pro`",
        ]
    );
    assert_eq!(
        problems
            .iter()
            .map(|problem| problem.line)
            .collect::<Vec<_>>(),
        vec![1, 3, 5]
    );
    assert_eq!(workspace.projects.len(), 1, "{:?}", workspace.projects);
}

/// A typo'd key is a problem rather than a shrug, and the CLI stops on it: an
/// `entyr` that silently did nothing would leave its project without a
/// program.
#[test]
fn an_unknown_key_is_a_problem() {
    let root = scratch("typo");
    workspace(&root, "[[project]]\nentyr = \"setup.lua\"\n", None);

    let (_, problems) = found(&root);
    assert!(
        messages(&problems)
            .first()
            .is_some_and(|message| message.contains("entyr")),
        "{problems:?}"
    );
}

/// The file `init --workspace` wrote in 0.1 is told what changed, once, rather
/// than that each of its two lines is malformed.
#[test]
fn the_0_1_format_is_one_problem_that_says_what_changed() {
    let root = scratch("retired");
    workspace(&root, "[project]\nroot = true\n", None);

    let (_, problems) = found(&root);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0].line, 1);
    assert!(problems[0].message.contains("0.1"), "{problems:?}");

    let bare = scratch("retired-bare");
    workspace(&bare, "root = true\n", None);
    let (_, problems) = found(&bare);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].message.contains("0.1"), "{problems:?}");
}
