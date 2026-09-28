//! `for path in glob(…)` against a tree built here, so the tree the assertions
//! mean is written next to them rather than kept as a fixture beside a golden.

use std::path::PathBuf;

use installua::diag::Diagnostics;

/// `assets/a.txt`, `assets/sub/b.txt`, `assets/sub/deeper/c.txt` and
/// `assets/other/d.bin`, under a directory of the test's own.
fn tree(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("installua-glob-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for file in [
        "assets/a.txt",
        "assets/sub/b.txt",
        "assets/sub/deeper/c.txt",
        "assets/other/d.bin",
    ] {
        let path = root.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "").unwrap();
    }
    root
}

fn compile(name: &str, pattern: &str) -> (Option<String>, Vec<String>) {
    let root = tree(name);
    let source = format!(
        "attributes {{ name = \"g\", outFile = \"g.exe\" }}\n\
         installer {{ section(\"s\", function()\n\
           for path in glob(\"{pattern}\") do detailPrint(path) end\n\
         end) }}\n"
    );
    let options = installua::Options::for_file(&root.join("install.lua"));
    let mut diags = Diagnostics::new();
    let output = installua::build_with(&source, &options, &mut diags);
    let _ = std::fs::remove_dir_all(&root);
    (output, diags.iter().map(|d| d.message.clone()).collect())
}

/// What the loop unrolled to, one path per `DetailPrint`.
fn paths(name: &str, pattern: &str) -> Vec<String> {
    let (output, messages) = compile(name, pattern);
    assert_eq!(messages, Vec::<String>::new());
    output
        .unwrap()
        .lines()
        .filter_map(|line| line.trim().strip_prefix("DetailPrint "))
        .map(|path| path.trim_matches('"').to_string())
        .collect()
}

#[test]
fn a_star_matches_files_in_one_folder() {
    assert_eq!(paths("star", "assets/*.txt"), ["assets/a.txt"]);
}

#[test]
fn a_double_star_walks_every_folder_below() {
    assert_eq!(
        paths("double", "assets/**/*.txt"),
        [
            "assets/a.txt",
            "assets/sub/b.txt",
            "assets/sub/deeper/c.txt"
        ]
    );
}

#[test]
fn a_trailing_double_star_is_every_file_below() {
    assert_eq!(
        paths("trailing", "assets/**"),
        [
            "assets/a.txt",
            "assets/other/d.bin",
            "assets/sub/b.txt",
            "assets/sub/deeper/c.txt",
        ]
    );
}

#[test]
fn a_trailing_slash_asks_for_folders() {
    assert_eq!(
        paths("folders", "assets/*/"),
        ["assets/other", "assets/sub"]
    );
}

#[test]
fn a_wildcard_can_stand_for_a_folder() {
    assert_eq!(paths("middle", "assets/*/*.txt"), ["assets/sub/b.txt"]);
}

/// The folder it tried, not the source's directory — which is empty when the
/// source sits in the working directory.
#[test]
fn a_missing_folder_is_named_in_the_error() {
    let (_, messages) = compile("missing", "nope/**/*.txt");
    assert_eq!(
        messages,
        ["`glob` cannot read `nope`: No such file or directory (os error 2)"]
    );
}
