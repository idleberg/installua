//! The CLI: a thin shell over the library.
//!
//! Nothing is decided here. Argument parsing, file reading and exit codes are
//! the whole of it — every question about the language is answered by a
//! `installua::` call that an editor could make just as well.
//!
//! Exit codes: 0 success, 1 the source was rejected, 2 the invocation was.
//! `clap` exits 2 on a bad invocation of its own accord, which is the same
//! number this file used before it parsed its own arguments.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};

use installua::diag::Diagnostics;

mod log;

#[derive(Parser)]
#[command(
    name = "installua",
    about = "A Lua-shaped language that compiles to NSIS.",
    version,
    // No command is not an error worth a bare message: the list of commands is
    // the answer to what someone typing `installua` wanted to know.
    arg_required_else_help = true,
    after_help = "\
`emit` is for wiring Installua into an existing build; `build` owns the \
`makensis` invocation, which is what lets it rewrite `makensis`'s diagnostics \
back onto the Lua source."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Everything `build` would say, writing nothing
    Check {
        /// The programs to check
        #[arg(required = true, value_name = "FILE.LUA")]
        files: Vec<PathBuf>,

        /// Set a build parameter, as `build` would
        #[arg(short = 'D', long = "param", value_name = "NAME=VALUE")]
        define: Vec<String>,
    },

    /// Compile to .nsi and stop
    Emit {
        #[command(flatten)]
        args: BuildArgs,

        /// Write to stdout
        #[arg(long)]
        stdout: bool,
    },

    /// Compile, then run `makensis -WX`
    Build {
        #[command(flatten)]
        args: BuildArgs,

        /// Declared, so the refusal below can explain itself rather than
        /// leaving clap to say "unexpected argument"; hidden, so `build --help`
        /// never offers a flag `build` will not honour. `hide` keeps it out of
        /// the help and nothing else — it still parses, which is the whole
        /// point.
        #[arg(long, hide = true)]
        stdout: bool,
    },

    /// `-CMDHELP` bucket counts
    Coverage,

    /// .luarc.json, selene.toml
    Init {
        /// Where to write them
        #[arg(value_name = "DIR", default_value = ".")]
        dir: PathBuf,

        /// Also offer the stubs, the editor's tasks and the .gitignore entries
        #[arg(short = 'i', long)]
        interactive: bool,

        /// Overwrite what is already there, without asking
        #[arg(short = 'f', long)]
        force: bool,

        /// Write installua.toml instead: the marker that says the declarations
        /// under this directory are shared by every installer below it
        #[arg(short = 'w', long)]
        workspace: bool,
    },

    /// .installua/meta/*.lua and the selene std
    Stubs {
        /// The project to read
        #[arg(value_name = "DIR", default_value = ".")]
        dir: PathBuf,
    },

    /// The maintainer's half, kept out of the help (see [`Generate`]).
    #[command(hide = true, subcommand)]
    Generate(Generate),
}

/// The arguments `emit` and `build` share.
///
/// Shared rather than written twice so the two cannot drift. `--stdout` is
/// *not* here: it is the one argument the two commands treat differently, so
/// each declares its own and the difference is visible where it is decided.
#[derive(Args)]
struct BuildArgs {
    /// The program to compile
    #[arg(value_name = "FILE.LUA")]
    input: PathBuf,

    /// Write here instead of alongside the input
    #[arg(short, long, value_name = "FILE.NSI")]
    output: Option<PathBuf>,

    /// Set a build parameter declared with `param(…)`
    ///
    /// Repeatable. A name the program does not declare is an error, not a
    /// shrug — see the `unknown-param` diagnostic.
    //
    // The long name is `--param`, not `--define` or `--declare`: the source
    // declares a parameter and the invocation sets one, and a flag named for
    // the declaring half would read as doing the thing `unknown-param` says it
    // does not — "sets a parameter this program does not declare". `-D` stays
    // for the `makensis -D` reflex, which is where the muscle memory comes
    // from even though the semantics differ. Kept out of the doc comment
    // because clap prints that in `--help`, and this is a note to whoever
    // renames it next.
    #[arg(short = 'D', long = "param", value_name = "NAME=VALUE")]
    define: Vec<String>,
}

/// `installua generate`: the maintainer's half, kept out of the help.
///
/// Two arms, where there were four. Two of the originals were NSIS scrapers and
/// moved into the drift tests that already read what they write; the third
/// generated `LANGUAGE.md`, which the command reference and `installua
/// coverage` between them had already replaced.
///
/// What is left is one that generates *Rust* and one that generates a
/// documentation page. Both stay commands for the same reason the others could
/// stop being one: `table`'s output has to compile before the test that checks
/// it can run, and `cli`'s source is the `clap` declarations in this file, which
/// nothing outside this binary can see.
///
/// Hidden rather than removed, because `cargo test` fails when either output
/// and its source drift, and the failure has to name the command that fixes it.
#[derive(Subcommand)]
enum Generate {
    /// src/table/generated.rs, printed to stdout
    Table {
        /// `makensis -CMDHELP`, as checked in
        #[arg(value_name = "CMDHELP.TXT")]
        snapshot: PathBuf,
    },

    /// The CLI reference page, printed to stdout
    Cli,
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Check { files, define } => check(&files, &define),
        Command::Emit { args, stdout } => build(&args, stdout, false),
        Command::Build { args, stdout } => build(&args, stdout, true),
        Command::Coverage => coverage(),
        Command::Init {
            dir,
            interactive,
            force,
            workspace,
        } => init(&dir, interactive, force, workspace),
        Command::Stubs { dir } => stubs(&dir).err().unwrap_or(ExitCode::SUCCESS),
        Command::Generate(Generate::Table { snapshot }) => table(&snapshot),
        Command::Generate(Generate::Cli) => cli_page(),
    }
}

/// The options a command compiles under, or `None` when the project's own
/// declarations are unreadable.
///
/// A malformed `.installua/declarations/*.toml` stops the command rather than
/// warning: the file exists to tell the compiler what a plugin's arity is, and
/// a program checked without it would be checked against a language missing
/// whatever it declared. Every diagnostic that followed would be about the
/// wrong thing.
fn options(input: &Path, define: &[String]) -> Option<installua::Options> {
    let (mut options, problems) = installua::Options::for_project(input);
    if !problems.is_empty() {
        for problem in &problems {
            log::error(problem.to_string());
        }
        return None;
    }
    options.params = defines(define)?;
    Some(options)
}

/// `-D NAME=VALUE` — equivalently `--param NAME=VALUE` — split.
///
/// The messages here name the short form whichever way it was written: `-D` is
/// the shorter thing to read back, and the two spellings reach this function
/// indistinguishably.
///
/// Only the split is done here: whether `NAME` is declared and whether `VALUE`
/// is the type the declaration wants are both questions about the program, so
/// they are the compiler's and arrive as ordinary diagnostics. What this
/// rejects is the shape, which is a question about the invocation — hence the
/// same exit code a bad flag gets.
///
/// The `=` is required. NSIS's `-DNAME` defines a bare name, but a parameter
/// here is typed by its default, and "no value" would have to mean `true` for
/// one and `""` for another.
fn defines(define: &[String]) -> Option<std::collections::BTreeMap<String, String>> {
    let mut params = std::collections::BTreeMap::new();
    for entry in define {
        let Some((name, value)) = entry.split_once('=') else {
            log::error(format!("`-D {entry}` has no value; write `-D {entry}=…`"));
            return None;
        };
        if let Some(previous) = params.insert(name.to_string(), value.to_string()) {
            log::error(format!(
                "`-D {name}` was given twice, as `{previous}` and `{value}`"
            ));
            return None;
        }
    }
    Some(params)
}

fn check(files: &[PathBuf], define: &[String]) -> ExitCode {
    let mut failed = false;
    for path in files {
        let Some(source) = read(path) else {
            return ExitCode::from(2);
        };

        // Compiled and thrown away, rather than parsed and checked. Half the
        // language's diagnostics are raised by the lowering — every unknown
        // field, every retired instruction, every page and control error — so
        // a `check` that stopped at the frontend exited 0 on programs `build`
        // rejects, which is the one thing a check must never do.
        //
        // The cost is the lowering, which is the cheap half of a build: what
        // `build` spends its time on is `makensis`, and that is exactly what
        // this does not run.
        //
        // Following `include` for the same reason it always did: a name an
        // included file declares is not an error, and a `check` that said it
        // was would be worse than no `check` at all.
        // The same `-D`s a build would pass, because a program whose parameters
        // are overridden is a different program to check: `check` is the gate
        // for exactly the build CI is about to run.
        let Some(options) = options(path, define) else {
            return ExitCode::from(2);
        };

        let mut diags = Diagnostics::new();
        installua::compile_with(&source, &options, &mut diags);
        report(&diags, path);
        failed |= diags.has_errors();
    }

    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn build(args: &BuildArgs, stdout: bool, assemble: bool) -> ExitCode {
    // Before the compile, not after it: the invocation is wrong whatever the
    // program says, and reporting a program's diagnostics first would bury the
    // one message that is actually actionable.
    if stdout && assemble {
        return usage_error("`--stdout` has no script for `makensis` to read; use `emit`");
    }

    let input = &args.input;
    let Some(source) = read(input) else {
        return ExitCode::from(2);
    };

    // Relative paths in the source resolve against the *source's* directory,
    // not the shell's: a `glob` means the same thing wherever the build is run
    // from, which is what makes the output reproducible.
    let Some(options) = options(input, &args.define) else {
        return ExitCode::from(2);
    };

    let mut diags = Diagnostics::new();
    let result = installua::build_mapped(&source, &options, &mut diags);
    report(&diags, input);

    let Some((nsi, map)) = result else {
        return ExitCode::FAILURE;
    };

    if stdout {
        print!("{nsi}");
        return ExitCode::SUCCESS;
    }

    let output = args
        .output
        .clone()
        .unwrap_or_else(|| input.with_extension("nsi"));
    if let Err(error) = std::fs::write(&output, nsi) {
        log::error(format!("cannot write {}: {error}", output.display()));
        return ExitCode::from(2);
    }
    if !assemble {
        return ExitCode::SUCCESS;
    }

    // The invocation is ours because the map cannot travel with the artifact:
    // NSIS can read its own line number and cannot be told a different one.
    let makensis = std::env::var("MAKENSIS").unwrap_or_else(|_| "makensis".to_string());
    let source_name = input.display().to_string();
    match installua::assemble::assemble(&output, &map, &source_name, diags.files(), &makensis) {
        Err(error) => {
            log::error(format!("cannot run `{makensis}`: {error}"));
            log::log(format!("the script was written to {}", output.display()));
            ExitCode::from(2)
        }
        Ok(assembly) if assembly.ok && assembly.problems.is_empty() => ExitCode::SUCCESS,
        Ok(assembly) => {
            for problem in &assembly.problems {
                log::error(problem);
            }
            // Nothing was recognised, so the log is the only thing there is to
            // show: a message shape this compiler has not seen is still the
            // user's problem to read.
            if assembly.problems.is_empty() {
                eprint!("{}", assembly.log);
            }
            ExitCode::FAILURE
        }
    }
}

/// `installua coverage`: the census, printed.
///
/// The output is a golden file, so a PR that moves twelve commands out of
/// `todo` shows exactly which twelve in its diff. That is the whole reason the
/// names are printed and not only the counts — a count going down says work
/// happened, a name disappearing says which work.
fn coverage() -> ExitCode {
    print!("{}", installua::table::coverage());
    ExitCode::SUCCESS
}

/// Stop here, with the code to exit under; `Ok(())` carries on.
///
/// `Err(ExitCode::SUCCESS)` is not a contradiction: abandoning a prompt is a
/// legitimate way to end an interactive run, and the files written before it
/// stay written. It is the only reason this is not a `bool`.
type Stop = Result<(), ExitCode>;

/// How a write answers a file that is already there.
///
/// The three of them are the three ways `init` can be invoked, and they meet in
/// [`write_or_ask`] and nowhere else.
#[derive(Clone, Copy, PartialEq, Eq)]
enum OnCollision {
    /// Report it and move on. `init` has already refused over this, so reaching
    /// it here means the file appeared between that check and this write.
    Refuse,
    /// Ask. There is a human at the other end of an interactive run, and they
    /// are better placed than this program to know whether their `.luarc.json`
    /// is worth keeping.
    Ask,
    /// `--force`. Write over it, say so, ask nothing.
    Force,
}

/// `installua init [dir]`: the two config files.
///
/// `--workspace` writes one file instead, and a different one: the
/// `root = true` marker that bounds a monorepo's declaration search. It is not
/// a variant of the same command — a workspace directory holds no `.lua` file
/// of its own, so an editor configuration there would be about nothing.
///
/// **A directory that has them already is refused**, listing every file in the
/// way and writing none of them — checked before the first write, so a refusal
/// leaves the directory exactly as it found it. This used to be a shrug: it
/// printed "left alone" and exited 0, which meant an upgrade that should have
/// refreshed a stale `.luarc.json` was indistinguishable from one that did.
/// `--force` overwrites; `--interactive` asks per file.
fn init(root: &Path, interactive: bool, force: bool, workspace: bool) -> ExitCode {
    // One file with one line in it has nothing to ask about, and every question
    // `--interactive` asks — stubs, the editor's tasks, the `.gitignore` — is
    // about a directory holding sources, which a workspace root is not.
    if workspace && interactive {
        return usage_error("`--workspace` writes one file and asks nothing about it");
    }

    let on = match (interactive, force) {
        (_, true) => OnCollision::Force,
        (true, false) => OnCollision::Ask,
        (false, false) => OnCollision::Refuse,
    };

    // Where, before what. The argument defaults to `.`, and `.` is the one
    // value most likely to be wrong by accident — so an interactive run shows
    // it and lets it be edited before a single file is written, rather than
    // after.
    let root = &match interactive {
        false => root.to_path_buf(),
        true => {
            clark::intro("Initialize a new project");
            match where_to(root) {
                Ok(chosen) => chosen,
                Err(code) => return code,
            }
        }
    };

    let files: Vec<(&'static str, String)> = if workspace {
        vec![("installua.toml", installua::stubs::workspace_toml())]
    } else {
        vec![
            (".luarc.json", installua::stubs::luarc()),
            ("selene.toml", installua::stubs::selene_toml()),
        ]
    };

    // Every collision at once, before any write. One at a time would leave a
    // directory half initialised and a user re-running the command to find the
    // next objection.
    if on == OnCollision::Refuse {
        let existing: Vec<PathBuf> = files
            .iter()
            .map(|(name, _)| root.join(name))
            .filter(|path| path.exists())
            .collect();
        if !existing.is_empty() {
            for path in &existing {
                log::error(format!("{} exists", path.display()));
            }
            log::log("nothing written; `--force` overwrites, `--interactive` asks per file");
            return ExitCode::from(2);
        }
    }

    let outcome = if interactive {
        interactively(root, &files, on)
    } else {
        for (name, contents) in &files {
            if let Err(code) = write_or_ask(&root.join(name), contents, on) {
                return code;
            }
        }
        if workspace {
            log::info("shared declarations go in .installua/declarations here");
            log::log("every installer below this directory reads them");
        } else {
            log::info("now run `installua stubs` to generate the editor's meta files");
            log::log("or `installua init --interactive` to be offered it, and more");
        }
        Ok(())
    };

    outcome.err().unwrap_or(ExitCode::SUCCESS)
}

/// The directory, confirmed before anything goes into it.
///
/// Seeded with whatever the argument was — usually `.` — so pressing return is
/// the same answer the non-interactive command would have given without asking.
///
/// A directory that is not there yet is created rather than refused: "installer"
/// is a perfectly good answer to "where" from someone starting a project rather
/// than adopting one. Which is also why this is a plain text field and not
/// `clark::path` — that one browses a filesystem, and warns about a path it
/// cannot find, which is the normal case here rather than a mistake.
fn where_to(root: &Path) -> Result<PathBuf, ExitCode> {
    let chosen = clark::text("Confirm directory:")
        .initial_value(root.display().to_string())
        .interact()
        .map_err(cancelled("nothing written"))?;

    let chosen = PathBuf::from(chosen);
    if let Err(error) = std::fs::create_dir_all(&chosen) {
        log::error(format!("cannot create {}: {error}", chosen.display()));
        return Err(ExitCode::from(2));
    }
    Ok(chosen)
}

/// The questions, and then the writes they settled.
///
/// Asked first and written afterwards, so the whole shape of the run is decided
/// before anything lands. Only the extras are asked about: the config files a
/// bare `init` writes are written here too, but as rows they were unselectable
/// lines above every real choice. Each is named by `write_or_ask`
/// as it lands, so what happened is still on screen afterwards.
///
/// One question per answer, rather than one multiselect for all three. The
/// editor is genuinely exclusive — a project has one `tasks.json` shape, not a
/// set — and a multiselect cannot say that, while a yes/no that defaults to yes
/// is a shorter answer than finding a row and pressing space.
fn interactively(root: &Path, files: &[(&'static str, String)], on: OnCollision) -> Stop {
    const NONE: &str = "none";
    const VSCODE: &str = "vscode";
    const ZED: &str = "zed";

    let stubs_too = clark::confirm("Create stubs? (`installua stubs`, run here)")
        .initial_value(true)
        .interact()
        .map_err(cancelled("nothing written"))?;

    let editor = clark::select("Which editor should it configure?")
        .choice(clark::SelectOption::labelled(NONE, "None"))
        .choice(
            clark::SelectOption::labelled(VSCODE, "VS Code")
                .with_hint(".vscode/extensions.json and tasks.json"),
        )
        .choice(clark::SelectOption::labelled(ZED, "Zed").with_hint(".zed/tasks.json"))
        .interact()
        .map_err(cancelled("nothing written"))?;

    let gitignore_too = clark::confirm("Update .gitignore? (what the generators write)")
        .initial_value(true)
        .interact()
        .map_err(cancelled("nothing written"))?;

    for (name, contents) in files {
        write_or_ask(&root.join(name), contents, on)?;
    }

    if stubs_too {
        stubs(root)?;
    }
    match editor {
        VSCODE => vscode(root, on)?,
        ZED => zed(root, on)?,
        _ => {}
    }
    if gitignore_too {
        gitignore(root)?;
    }

    clark::outro("done");
    Ok(())
}

/// Turn a clark failure into a [`Stop`], with what to say if it was a cancel.
///
/// Abandoning a prompt is not an error — it is the user answering "none of
/// this" — so it exits 0. A terminal that cannot be read is, and exits 2 like
/// any other bad invocation.
fn cancelled(message: &'static str) -> impl Fn(clark::ClackError) -> ExitCode {
    move |error| match error {
        clark::ClackError::Cancelled => {
            clark::cancel(message);
            ExitCode::SUCCESS
        }
        error => {
            log::error(error.to_string());
            ExitCode::from(2)
        }
    }
}

/// Write, unless something is already there and [`OnCollision`] says not to.
fn write_or_ask(path: &Path, contents: &str, on: OnCollision) -> Stop {
    if path.exists() {
        let overwrite = match on {
            OnCollision::Force => true,
            OnCollision::Refuse => false,
            OnCollision::Ask => clark::confirm(format!("{} exists. Overwrite?", path.display()))
                .initial_value(false)
                .interact()
                .map_err(cancelled("stopped"))?,
        };
        if !overwrite {
            log::warn(format!("{} exists, left alone", path.display()));
            return Ok(());
        }
    }

    if let Err(error) = std::fs::write(path, contents) {
        log::error(format!("cannot write {}: {error}", path.display()));
        return Err(ExitCode::from(2));
    }
    log::success(format!("wrote {}", path.display()));
    Ok(())
}

/// One file an editor reads: what to write if it is not there, and how to add
/// ourselves to it if it is.
type EditorFile = (
    &'static str,
    fn() -> String,
    fn(&str) -> installua::stubs::Merge,
);

/// `.vscode/extensions.json` and `.vscode/tasks.json`.
fn vscode(root: &Path, on: OnCollision) -> Stop {
    editor_config(
        root,
        on,
        ".vscode",
        &[
            (
                "extensions.json",
                installua::stubs::vscode_extensions,
                installua::stubs::merge_extensions,
            ),
            (
                "tasks.json",
                installua::stubs::vscode_tasks,
                installua::stubs::merge_tasks,
            ),
        ],
    )
}

/// `.zed/tasks.json`, which is the whole of what Zed takes per project.
fn zed(root: &Path, on: OnCollision) -> Stop {
    editor_config(
        root,
        on,
        ".zed",
        &[(
            "tasks.json",
            installua::stubs::zed_tasks,
            installua::stubs::merge_zed_tasks,
        )],
    )
}

/// The editor's files, merged rather than replaced.
///
/// A user's `tasks.json` is theirs — it has their tasks in it, and quite
/// possibly their comments. Adding two tasks to it is an insertion, and
/// [`installua::stubs::merge`] does it without touching a byte of the rest.
fn editor_config(root: &Path, on: OnCollision, in_dir: &str, files: &[EditorFile]) -> Stop {
    let dir = root.join(in_dir);
    if let Err(error) = std::fs::create_dir_all(&dir) {
        log::error(format!("cannot create {}: {error}", dir.display()));
        return Err(ExitCode::from(2));
    }

    for (name, template, merge) in files {
        let path = dir.join(name);
        let Ok(existing) = std::fs::read_to_string(&path) else {
            // Unreadable is treated as absent, and `write_or_ask` will find out
            // which it was: a missing file is written, and one that exists but
            // cannot be read is a collision it asks about.
            write_or_ask(&path, &template(), on)?;
            continue;
        };

        match merge(&existing) {
            installua::stubs::Merge::Present => {
                log::warn(format!("{} already has it, left alone", path.display()));
            }
            installua::stubs::Merge::Merged(merged) => {
                if let Err(error) = std::fs::write(&path, merged) {
                    log::error(format!("cannot write {}: {error}", path.display()));
                    return Err(ExitCode::from(2));
                }
                log::success(format!("merged into {}", path.display()));
            }
            // Not a shape this can edit. Replacing it would throw away whatever
            // is in there, so that is the user's call and nobody else's.
            installua::stubs::Merge::Unrecognised => {
                let replace = match on {
                    OnCollision::Force => true,
                    OnCollision::Refuse => false,
                    OnCollision::Ask => {
                        clark::confirm(format!("cannot merge {}. Replace it?", path.display()))
                            .initial_value(false)
                            .interact()
                            .map_err(cancelled("stopped"))?
                    }
                };
                if replace {
                    write_or_ask(&path, &template(), OnCollision::Force)?;
                } else {
                    log::warn(format!(
                        "cannot merge {}, add this by hand:",
                        path.display()
                    ));
                    // Under no symbol and at no indent: this is a block meant
                    // to be copied out of the terminal and pasted into a file.
                    log::log(template());
                }
            }
        }
    }
    Ok(())
}

/// The ignore entries, appended.
///
/// Appended and never rewritten: a `.gitignore` is a file every other tool in
/// the project also appends to, and the marker comment is what keeps this from
/// adding its block twice.
fn gitignore(root: &Path) -> Stop {
    let path = root.join(".gitignore");
    let block = installua::stubs::gitignore();

    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    if existing.contains(installua::stubs::GITIGNORE_MARKER) {
        log::warn(format!("{} already has it, left alone", path.display()));
        return Ok(());
    }

    // One blank line between whatever was there and this block. Twice around at
    // most: a file ending in neither a newline nor a blank line needs both.
    let mut updated = existing;
    while !updated.is_empty() && !updated.ends_with("\n\n") {
        updated.push('\n');
    }
    updated.push_str(&block);

    if let Err(error) = std::fs::write(&path, updated) {
        log::error(format!("cannot write {}: {error}", path.display()));
        return Err(ExitCode::from(2));
    }
    log::success(format!("updated {}", path.display()));
    Ok(())
}

/// `installua stubs [dir]`: the `---@meta` files and the selene std.
///
/// Three files, because they serve two tools and one of them is about *this
/// project* rather than about the language: `lua-language-server` cannot follow
/// `include`, so the names a project's own sources declare have to be generated
/// too.
///
/// Returns a [`Stop`] rather than an `ExitCode` only so `init --interactive`
/// can call it and know whether it worked; an `ExitCode` cannot be compared
/// against anything. What the command does is unchanged, overwriting included:
/// these three files are generated, and nothing in them is a user's to keep.
fn stubs(root: &Path) -> Stop {
    let meta = root.join(".installua/meta");
    if let Err(error) = std::fs::create_dir_all(&meta) {
        log::error(format!("cannot create {}: {error}", meta.display()));
        return Err(ExitCode::from(2));
    }

    let mut sources: Vec<(String, String)> = Vec::new();
    match std::fs::read_dir(root) {
        Ok(entries) => {
            let mut paths: Vec<PathBuf> = entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "lua"))
                .collect();
            paths.sort();
            for path in paths {
                if let Ok(source) = std::fs::read_to_string(&path) {
                    // The bare file name: the generated comment is read beside
                    // the project it describes, and an absolute path there is
                    // one more thing that changes when the checkout moves.
                    let name = path
                        .file_name()
                        .map(|name| name.to_string_lossy().to_string())
                        .unwrap_or_else(|| path.display().to_string());
                    sources.push((name, source));
                }
            }
        }
        Err(error) => {
            log::error(format!("cannot read {}: {error}", root.display()));
            return Err(ExitCode::from(2));
        }
    }

    // The project's own declarations, so a third-party plugin is typed in the
    // editor by the same file that makes it compile. A malformed one stops the
    // command for the reason it stops a build: stubs generated without it would
    // quietly leave out whatever it declared.
    // The whole cascade, not just this directory's, so the editor types a
    // shared plugin exactly where the compiler accepts one.
    let (dirs, mut problems) = installua::project::declaration_dirs(root);
    let (declarations, mut found) = installua::declarations::Declarations::load_all(&dirs);
    problems.append(&mut found);
    if !problems.is_empty() {
        for problem in &problems {
            log::error(problem.to_string());
        }
        return Err(ExitCode::from(2));
    }

    let files = [
        (
            meta.join("installua.lua"),
            installua::stubs::meta(&declarations),
        ),
        (
            meta.join("project.lua"),
            installua::stubs::project_meta(&sources),
        ),
        (
            root.join(".installua/installua.yml"),
            installua::stubs::selene_std(),
        ),
    ];

    for (path, contents) in files {
        if let Err(error) = std::fs::write(&path, contents) {
            log::error(format!("cannot write {}: {error}", path.display()));
            return Err(ExitCode::from(2));
        }
        log::success(format!("wrote {}", path.display()));
    }

    Ok(())
}

/// `installua generate table <cmdhelp.txt>`: the generator half of the join.
///
/// Prints Rust source; the workflow is a shell redirect into
/// `src/table/generated.rs`, and `cargo test` fails if the checked-in file and
/// the snapshot ever disagree.
fn table(snapshot: &Path) -> ExitCode {
    let Some(text) = read(snapshot) else {
        return ExitCode::from(2);
    };
    print!("{}", installua::table::cmdhelp::generate(&text));
    ExitCode::SUCCESS
}

/// `installua generate cli`: the CLI reference page, printed to stdout.
///
/// Generated rather than written, because every fact on it — a flag's two
/// spellings, its value name, its one-line help — is already stated once in the
/// `clap` declarations above, and a hand-written page would be a second copy
/// that drifts the first time a flag is renamed. `mise run docs:cli` redirects
/// this into the page and `cargo test` fails when the two disagree, the same
/// join `src/table/generated.rs` sits in.
///
/// Prose that is *not* derivable from `clap` — what a command is for, what it
/// leaves behind — belongs on the surrounding hand-written pages, not here.
/// This one is the signature list.
fn cli_page() -> ExitCode {
    use clap::CommandFactory;

    let mut cli = Cli::command();
    // Fills in the inherited pieces — `help`/`version`, the bin name on each
    // subcommand's usage line — which are absent until `clap` builds the tree.
    cli.build();

    let mut page = String::from(
        "---\n\
         title: CLI\n\
         description: Every installua subcommand, its arguments and its flags.\n\
         ---\n\n\
         <!-- Generated by `mise run docs:cli`. Do not hand-edit: the source is\n\
              the clap declarations in src/main.rs. -->\n\n",
    );
    if let Some(about) = cli.get_about() {
        page.push_str(&format!("{about}\n\n"));
    }
    page.push_str(
        "Exit codes: `0` success, `1` the source was rejected, `2` the invocation was.\n",
    );

    for sub in cli.get_subcommands() {
        // Hidden is hidden. `generate` is the maintainer's half and is kept out
        // of `--help` for a reason that applies here word for word. `help` is
        // clap's own, and a page is what a reader is already reading.
        if sub.is_hide_set() || sub.get_name() == "help" {
            continue;
        }

        page.push_str(&format!("\n## installua {}\n\n", sub.get_name()));
        if let Some(about) = sub.get_about() {
            page.push_str(&format!("{about}.\n\n"));
        }
        // Without the "Usage:" clap prefixes, so the fence holds a line that can
        // be pasted into a shell rather than read out of a help screen.
        let usage = sub.clone().render_usage().to_string();
        let usage = usage.strip_prefix("Usage: ").unwrap_or(&usage).to_string();
        page.push_str(&format!("```sh\n{usage}\n```\n"));

        let rows: Vec<(String, String)> = sub
            .get_arguments()
            .filter(|arg| !arg.is_hide_set() && arg.get_id() != "help")
            .map(|arg| (spelling(arg), help_of(arg)))
            .collect();
        if !rows.is_empty() {
            page.push_str("\n| | |\n| --- | --- |\n");
            for (spelling, help) in rows {
                page.push_str(&format!("| {spelling} | {help} |\n"));
            }
        }
    }

    print!("{page}");
    ExitCode::SUCCESS
}

/// One argument as a reader types it: `<DIR>`, or every flag spelling it has.
fn spelling(arg: &clap::Arg) -> String {
    // The declared `value_name` where there is one — `<FILE.LUA>`, not the
    // field's name upper-cased, which is what the usage line already shows.
    let value = arg
        .get_value_names()
        .and_then(<[_]>::first)
        .map(ToString::to_string)
        .unwrap_or_else(|| arg.get_id().to_string().to_uppercase());

    if arg.is_positional() {
        return format!("`<{value}>`");
    }

    let mut flags = Vec::new();
    if let Some(short) = arg.get_short() {
        flags.push(format!("-{short}"));
    }
    if let Some(long) = arg.get_long() {
        flags.push(format!("--{long}"));
    }
    let mut spelled = flags.join(", ");
    // A switch takes no value, so naming one would invite `--force true`.
    if arg.get_num_args().is_none_or(|range| range.takes_values()) {
        spelled.push_str(&format!(" <{value}>"));
    }
    format!("`{spelled}`")
}

/// The one-line help, with what `clap` prints beside it: a default, and whether
/// the argument repeats. Both change how the argument is used, so neither can be
/// left to the reader to infer from the usage line.
fn help_of(arg: &clap::Arg) -> String {
    let mut help = arg
        .get_help()
        .map(ToString::to_string)
        .unwrap_or_default()
        .replace('\n', " ");
    if !help.is_empty() && !help.ends_with(['.', '!', '?']) {
        help.push('.');
    }
    // A switch's default is `false` and saying so is noise; a value's default is
    // what the reader gets when they leave it out.
    if let Some(default) = arg
        .get_default_values()
        .first()
        .filter(|_| arg.get_num_args().is_none_or(|range| range.takes_values()))
    {
        help.push_str(&format!(" Defaults to `{}`.", default.to_string_lossy()));
    }
    if matches!(
        arg.get_action(),
        clap::ArgAction::Append | clap::ArgAction::Count
    ) {
        help.push_str(" Repeatable.");
    }
    help.trim().to_string()
}

fn read(path: &Path) -> Option<String> {
    match std::fs::read_to_string(path) {
        Ok(source) => Some(source),
        Err(error) => {
            log::error(format!("cannot read {}: {error}", path.display()));
            None
        }
    }
}

/// Every diagnostic, not just the first.
fn report(diags: &Diagnostics, path: &Path) {
    if !diags.is_empty() {
        eprint!("{}", diags.render(&path.display().to_string()));
    }
}

/// The one invocation error clap cannot raise, because it is about which
/// subcommand a legal flag was given to.
fn usage_error(message: &str) -> ExitCode {
    log::error(message);
    ExitCode::from(2)
}
