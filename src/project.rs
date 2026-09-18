//! `installua.toml`: the file that marks a workspace and lists its projects.
//!
//! It does two things, and both follow from where it sits:
//!
//! - **It bounds the declaration search.** A compile reads the
//!   `.installua/declarations` beside its source and the one beside the
//!   nearest `installua.toml` above it, so several installers in one checkout
//!   share one declaration of a vendored plugin instead of copying the same
//!   five lines into each.
//! - **It names the programs a command runs on** when it was handed none:
//!   each `[[project]]` is a `name` and an `entry`, so `installua build` in a
//!   one-project checkout needs no file, and `installua build -p pro` picks one
//!   out of several.
//!
//! Absence is the common case and changes nothing: no file anywhere leaves the
//! source's own directory as the whole search scope, and a command that was
//! named a file never looks for one.
//!
//! **The nearest file is the workspace, and the walk stops there.** Installua
//! 0.1 cascaded the way EditorConfig does — every marker up to one saying
//! `root = true`, each adding its directory. That was dropped rather than
//! carried over to the project list: merging several lists of projects has no
//! meaning, which is why Cargo refuses a workspace nested in a workspace, and
//! the layering anyone used is still here in two levels — the workspace's
//! declarations as the default, a project's own as the override. A third level
//! would be an explicit list of directories, the way `tsconfig` and Biome
//! `extends`, and would not break any file written under this rule.
//!
//! The walk stops at the file, at a directory holding `.git`, because a
//! checkout is a boundary whether or not anyone wrote a file at its top, and at
//! the filesystem root, without which a stray `installua.toml` in `$HOME` would
//! join every build on the machine.
//!
//! Parsed here rather than by a dependency, for the reason
//! [`crate::declarations`] parses its own files: one table and two keys do not
//! need a deserialiser, and the messages can name the key that was wrong.

use std::path::{Component, Path, PathBuf};

use crate::declarations::{DIRECTORY, Problem};

/// The file's name.
pub const MARKER: &str = "installua.toml";

/// One `installua.toml`, read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Workspace {
    /// The directory the file sits in, spelt from the directory the search
    /// started in — `../..` from a relative start, so a path built on it reads
    /// the way the user would have typed it. Absolute only when the relative
    /// spelling would cross a symlink and mean a different directory.
    pub dir: PathBuf,
    /// In the order written, which is the order `check` runs them in.
    pub projects: Vec<Project>,
}

/// One `[[project]]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Project {
    /// Optional while the file lists one project, since there is nothing to
    /// pick between; required, and unique, once it lists two.
    pub name: Option<String>,
    /// The `.lua` file, relative to the workspace directory — a file rather
    /// than a directory, because a directory would need a rule for which of its
    /// sources is the program.
    pub entry: String,
}

impl Workspace {
    /// The file itself, for messages.
    pub fn file(&self) -> PathBuf {
        under(&self.dir, MARKER)
    }

    /// `project`'s program, as a path the caller can open.
    pub fn entry(&self, project: &Project) -> PathBuf {
        under(&self.dir, &project.entry)
    }

    /// The project a command runs on, given the `-p` it was passed or none.
    ///
    /// No name is an answer only when there is one project to give. With
    /// several, picking the first would build something the user did not ask
    /// for, so it is an error that lists what they could have asked for.
    pub fn select(&self, name: Option<&str>) -> Result<&Project, String> {
        let file = self.file();
        let file = file.display();
        match name {
            Some(name) => self
                .projects
                .iter()
                .find(|project| project.name.as_deref() == Some(name))
                .ok_or_else(|| match self.names() {
                    names if names.is_empty() => {
                        format!("{file} has no project named `{name}`; it names none")
                    }
                    names => format!("{file} has no project named `{name}`; it has {names}"),
                }),
            None => match self.projects.as_slice() {
                [only] => Ok(only),
                [] => Err(format!(
                    "{file} lists no projects; name a file, or add a `[[project]]`"
                )),
                _ => Err(format!(
                    "{file} lists {} projects; pick one with `-p`: {}",
                    self.projects.len(),
                    self.names()
                )),
            },
        }
    }

    /// The names, backticked and comma-separated, for a message.
    fn names(&self) -> String {
        self.projects
            .iter()
            .filter_map(|project| project.name.as_deref())
            .map(|name| format!("`{name}`"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// The nearest `installua.toml` at or above `dir`, and whatever it got wrong.
///
/// A file with problems still comes back when it could be read, holding the
/// projects that were well formed: the problems are what the CLI stops on, and
/// a caller that wants to know *where* the workspace is should not have to
/// care whether it is valid.
pub fn find(dir: &Path) -> (Option<Workspace>, Vec<Problem>) {
    // Canonical, because the walk is `parent()` in a loop: a relative `dir`
    // runs out of components after two steps and would stop inside the
    // checkout it was supposed to climb. A directory that cannot be
    // canonicalised does not exist, and the read that follows says so better
    // than this could. An empty `dir` is the parent of a bare file name, and
    // means `.`.
    let here = if dir.as_os_str().is_empty() {
        Path::new(".")
    } else {
        dir
    };
    let Ok(start) = here.canonicalize() else {
        return (None, Vec::new());
    };

    let mut current: &Path = &start;
    let mut steps = 0;
    loop {
        let path = current.join(MARKER);
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                let dir = spell(dir, steps, current);
                let file = under(&dir, MARKER).display().to_string();
                let mut problems = Vec::new();
                let projects = parse(&file, &text, &mut problems);
                return (Some(Workspace { dir, projects }), problems);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            // A file that exists and cannot be read is a problem rather than an
            // absence — that is a permission or a broken link, and walking on
            // past it would silently pick a different workspace.
            Err(error) => {
                let problem = Problem {
                    file: path.display().to_string(),
                    line: 0,
                    message: format!("cannot read: {error}"),
                };
                return (None, vec![problem]);
            }
        }
        if current.join(".git").exists() {
            return (None, Vec::new());
        }
        match current.parent() {
            Some(parent) => {
                current = parent;
                steps += 1;
            }
            None => return (None, Vec::new()),
        }
    }
}

/// Every directory a compile from `dir` reads declarations from, outermost
/// first: the workspace's, then `dir`'s own.
///
/// `dir` itself is always there, workspace or no workspace. That is what makes
/// the file purely additive: a project with no `installua.toml` anywhere reads
/// exactly the one directory it read before this module existed.
pub fn declaration_dirs(dir: &Path) -> (Vec<PathBuf>, Vec<Problem>) {
    let (workspace, problems) = find(dir);
    let mut dirs = Vec::new();
    // The workspace is often `dir` itself — the file beside the sources — and
    // then it is one directory, not two: loading it twice would report every
    // declaration in it as declared twice.
    if let Some(workspace) = workspace.filter(|workspace| workspace.dir != dir) {
        dirs.push(workspace.dir.join(DIRECTORY));
    }
    dirs.push(dir.join(DIRECTORY));
    (dirs, problems)
}

/// `rest` under `dir`, without the `./` a join onto `.` or `` would put in
/// front of every path the CLI prints.
fn under(dir: &Path, rest: &str) -> PathBuf {
    if dir.as_os_str().is_empty() || dir == Path::new(".") {
        PathBuf::from(rest)
    } else {
        dir.join(rest)
    }
}

/// The directory `steps` levels above `start`, spelt from `start` as it was
/// written: `installers/pro` two levels up is ``, `.` two levels up is `../..`,
/// and an absolute path loses components rather than gaining `..`s.
///
/// Lexically, and then checked against the canonical walk: `installers/pro`
/// being a symlink makes its lexical parent a different directory from its
/// real one, and in that case the absolute `found` is the only honest answer.
fn spell(start: &Path, steps: usize, found: &Path) -> PathBuf {
    if steps == 0 {
        return start.to_path_buf();
    }
    let mut spelt = if start == Path::new(".") {
        PathBuf::new()
    } else {
        start.to_path_buf()
    };
    for _ in 0..steps {
        match spelt.components().next_back() {
            Some(Component::Normal(_)) => {
                spelt.pop();
            }
            _ => spelt.push(".."),
        }
    }
    let target = if spelt.as_os_str().is_empty() {
        Path::new(".")
    } else {
        &spelt
    };
    match target.canonicalize() {
        Ok(canonical) if canonical == found => spelt,
        _ => found.to_path_buf(),
    }
}

/// A `[[project]]` as it is being read: both keys optional until the table
/// ends, so a missing one is reported once, at its header.
struct Draft {
    line: usize,
    name: Option<String>,
    entry: Option<String>,
}

/// What the line being read belongs to.
enum Table {
    /// Above the first header.
    Top,
    Project(Draft),
    /// Installua 0.1's `[project]`, already reported. Its keys are that
    /// format's too, and a second message per line would bury the first.
    Retired,
}

/// The format: `[[project]]` tables of `name` and `entry`, nothing else.
///
/// An unknown key is a problem, and the CLI stops on it. A typo'd `entyr` that
/// silently did nothing would leave the project it belongs to without a
/// program, and is the `-DVERSOIN` failure mode that
/// [`crate::diag::Code::UnknownParam`] exists to reject.
fn parse(file: &str, text: &str, problems: &mut Vec<Problem>) -> Vec<Project> {
    let mut drafts = Vec::new();
    let mut table = Table::Top;
    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        let content = strip(raw);
        if content.is_empty() {
            continue;
        }

        let mut complain = |message: String| {
            problems.push(Problem {
                file: file.to_string(),
                line,
                message,
            });
        };

        if content.starts_with('[') {
            if let Table::Project(draft) = std::mem::replace(&mut table, Table::Top) {
                drafts.push(draft);
            }
            match content {
                "[[project]]" => {
                    table = Table::Project(Draft {
                        line,
                        name: None,
                        entry: None,
                    })
                }
                // What `init --workspace` wrote in 0.1, so the one file most
                // likely to be read under the new rule gets told what changed
                // rather than that it is malformed.
                "[project]" => {
                    complain(String::from(
                        "`[project]` with `root = true` is Installua 0.1's format; the file \
                         itself marks the workspace now, so delete both lines and list \
                         projects as `[[project]]`",
                    ));
                    table = Table::Retired;
                }
                other => complain(format!(
                    "`{other}` is not a table here; the only one is `[[project]]`"
                )),
            }
            continue;
        }

        let Some((key, value)) = content.split_once('=') else {
            complain(format!(
                "`{content}` is neither `[[project]]` nor `key = value`"
            ));
            continue;
        };
        let (key, value) = (key.trim(), value.trim());

        let draft = match &mut table {
            Table::Retired => continue,
            Table::Top if key == "root" => {
                complain(String::from(
                    "`root` is Installua 0.1's; the file itself marks the workspace now, \
                     so delete the line",
                ));
                continue;
            }
            Table::Top => {
                complain(format!("`{key}` is outside any `[[project]]`"));
                continue;
            }
            Table::Project(draft) => draft,
        };

        let slot = match key {
            "name" => &mut draft.name,
            "entry" => &mut draft.entry,
            other => {
                complain(format!(
                    "`{other}` is not a project field; the fields are `name` and `entry`"
                ));
                continue;
            }
        };
        let Some(text) = string(value) else {
            complain(format!("`{key}` wants a quoted string"));
            continue;
        };
        if slot.is_some() {
            complain(format!("`{key}` is set twice in one `[[project]]`"));
            continue;
        }
        *slot = Some(text);
    }
    if let Table::Project(draft) = table {
        drafts.push(draft);
    }

    check(file, drafts, problems)
}

/// The rules about the list rather than a line: every project has a program,
/// and once there are two, each has a name nobody else has.
fn check(file: &str, drafts: Vec<Draft>, problems: &mut Vec<Problem>) -> Vec<Project> {
    let several = drafts.len() > 1;
    let mut projects = Vec::new();
    // Every name seen, including a project's refused for other reasons: a
    // second `pro` is a second `pro` whether or not the first had an entry.
    let mut names: Vec<String> = Vec::new();
    for draft in drafts {
        let mut complain = |message: String| {
            problems.push(Problem {
                file: file.to_string(),
                line: draft.line,
                message,
            });
        };
        if several && draft.name.is_none() {
            complain(String::from(
                "this `[[project]]` needs a `name`: the file lists several, and `-p` picks \
                 one by it",
            ));
        }
        if let Some(name) = &draft.name {
            if names.contains(name) {
                complain(format!("a second project is named `{name}`"));
                continue;
            }
            names.push(name.clone());
        }
        let Some(entry) = draft.entry else {
            complain(String::from(
                "this `[[project]]` needs an `entry`: the `.lua` file it builds",
            ));
            continue;
        };
        projects.push(Project {
            name: draft.name,
            entry,
        });
    }
    projects
}

/// A basic TOML string, without escapes: a `"` inside one is refused rather
/// than guessed at, as [`crate::declarations`] refuses it.
fn string(value: &str) -> Option<String> {
    value
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .filter(|text| !text.contains('"'))
        .map(str::to_string)
}

/// The line with its comment removed, as [`crate::declarations`] does it: a
/// `#` inside a quoted string is text.
fn strip(line: &str) -> &str {
    let mut quoted = false;
    for (index, byte) in line.bytes().enumerate() {
        match byte {
            b'"' => quoted = !quoted,
            b'#' if !quoted => return line[..index].trim(),
            _ => {}
        }
    }
    line.trim()
}
