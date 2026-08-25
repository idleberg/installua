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
        #[arg(short = 'D', value_name = "NAME=VALUE")]
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

    /// installua.toml, .luarc.json, selene.toml
    Init {
        /// Where to write them
        #[arg(value_name = "DIR", default_value = ".")]
        dir: PathBuf,
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
    #[arg(short = 'D', value_name = "NAME=VALUE")]
    define: Vec<String>,
}

/// `installua generate`: the maintainer's half, kept out of the help.
///
/// One arm, where there were four. Two were NSIS scrapers and moved into the
/// drift tests that already read what they write; the third generated
/// `LANGUAGE.md`, which `docs/reference-map.md` and `installua coverage`
/// between them had already replaced.
///
/// What is left is the one that generates *Rust*, and it stays a command for
/// the reason the others could stop being one: its output has to compile before
/// the test that checks it can run, so it cannot live inside that test.
///
/// Hidden rather than removed, because `cargo test` fails when
/// `src/table/generated.rs` and the snapshot drift, and the failure has to name
/// the command that fixes it.
#[derive(Subcommand)]
enum Generate {
    /// src/table/generated.rs, printed to stdout
    Table {
        /// `makensis -CMDHELP`, as checked in
        #[arg(value_name = "CMDHELP.TXT")]
        snapshot: PathBuf,
    },
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Check { files, define } => check(&files, &define),
        Command::Emit { args, stdout } => build(&args, stdout, false),
        Command::Build { args, stdout } => build(&args, stdout, true),
        Command::Coverage => coverage(),
        Command::Init { dir } => init(&dir),
        Command::Stubs { dir } => stubs(&dir),
        Command::Generate(Generate::Table { snapshot }) => table(&snapshot),
    }
}

/// The options a command compiles under, or `None` when the project's own
/// declarations are unreadable.
///
/// A malformed `.installua/headers/*.toml` stops the command rather than
/// warning: the file exists to tell the compiler what a plugin's arity is, and
/// a program checked without it would be checked against a language missing
/// whatever it declared. Every diagnostic that followed would be about the
/// wrong thing.
fn options(input: &Path, define: &[String]) -> Option<installua::Options> {
    let (mut options, problems) = installua::Options::for_project(input);
    if !problems.is_empty() {
        for problem in &problems {
            eprintln!("installua: {problem}");
        }
        return None;
    }
    options.params = defines(define)?;
    Some(options)
}

/// `-D NAME=VALUE`, split.
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
            eprintln!("installua: `-D {entry}` has no value; write `-D {entry}=…`");
            return None;
        };
        if let Some(previous) = params.insert(name.to_string(), value.to_string()) {
            eprintln!("installua: `-D {name}` was given twice, as `{previous}` and `{value}`");
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
        eprintln!("installua: cannot write {}: {error}", output.display());
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
            eprintln!("installua: cannot run `{makensis}`: {error}");
            eprintln!("installua: the script was written to {}", output.display());
            ExitCode::from(2)
        }
        Ok(assembly) if assembly.ok && assembly.problems.is_empty() => ExitCode::SUCCESS,
        Ok(assembly) => {
            for problem in &assembly.problems {
                eprintln!("{problem}");
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

/// `installua init [dir]`: the four config files, written once.
///
/// Nothing is overwritten. A `.luarc.json` a user has edited is worth more than
/// a fresh one, and `init` being safe to re-run is what makes "run init again
/// after upgrading" reasonable advice.
fn init(root: &Path) -> ExitCode {
    let name = root
        .canonicalize()
        .ok()
        .and_then(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| "installer".to_string());

    let files = [
        ("installua.toml", installua::stubs::project_toml(&name)),
        (".luarc.json", installua::stubs::luarc()),
        ("selene.toml", installua::stubs::selene_toml()),
    ];

    for (name, contents) in files {
        let path = root.join(name);
        if path.exists() {
            println!("installua: {} exists, left alone", path.display());
            continue;
        }
        if let Err(error) = std::fs::write(&path, contents) {
            eprintln!("installua: cannot write {}: {error}", path.display());
            return ExitCode::from(2);
        }
        println!("installua: wrote {}", path.display());
    }

    println!("installua: now run `installua stubs` to generate the editor's meta files");
    ExitCode::SUCCESS
}

/// `installua stubs [dir]`: the `---@meta` files and the selene std.
///
/// Three files, because they serve two tools and one of them is about *this
/// project* rather than about the language: `lua-language-server` cannot follow
/// `include`, so the names a project's own sources declare have to be generated
/// too.
fn stubs(root: &Path) -> ExitCode {
    let meta = root.join(".installua/meta");
    if let Err(error) = std::fs::create_dir_all(&meta) {
        eprintln!("installua: cannot create {}: {error}", meta.display());
        return ExitCode::from(2);
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
            eprintln!("installua: cannot read {}: {error}", root.display());
            return ExitCode::from(2);
        }
    }

    // The project's own declarations, so a third-party plugin is typed in the
    // editor by the same file that makes it compile. A malformed one stops the
    // command for the reason it stops a build: stubs generated without it would
    // quietly leave out whatever it declared.
    let (declarations, problems) =
        installua::headers::Declarations::load(&root.join(installua::headers::DIRECTORY));
    if !problems.is_empty() {
        for problem in &problems {
            eprintln!("installua: {problem}");
        }
        return ExitCode::from(2);
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
            eprintln!("installua: cannot write {}: {error}", path.display());
            return ExitCode::from(2);
        }
        println!("installua: wrote {}", path.display());
    }

    ExitCode::SUCCESS
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

fn read(path: &Path) -> Option<String> {
    match std::fs::read_to_string(path) {
        Ok(source) => Some(source),
        Err(error) => {
            eprintln!("installua: cannot read {}: {error}", path.display());
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
    eprintln!("installua: {message}");
    ExitCode::from(2)
}
