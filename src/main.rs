//! The CLI: a thin shell over the library (§15.12).
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
back onto the Lua source (§15.22)."
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
    },

    /// Compile to .nsi and stop
    Emit {
        #[command(flatten)]
        args: BuildArgs,
    },

    /// Compile, then run `makensis -WX`
    Build {
        #[command(flatten)]
        args: BuildArgs,
    },

    /// `-CMDHELP` bucket counts (§14)
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
/// Shared rather than written twice so the two cannot drift, and `--stdout`
/// stays declared on `build` even though `build` refuses it: refusing it with
/// the reason (there is no file for `makensis` to read) is a better answer than
/// clap's "unexpected argument", and clap can only give that answer for a flag
/// it knows about.
#[derive(Args)]
struct BuildArgs {
    /// The program to compile
    #[arg(value_name = "FILE.LUA")]
    input: PathBuf,

    /// Write here instead of alongside the input
    #[arg(short, long, value_name = "FILE.NSI")]
    output: Option<PathBuf>,

    /// Write to stdout (`emit` only)
    #[arg(long)]
    stdout: bool,
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
        Command::Check { files } => check(&files),
        Command::Emit { args } => build(&args, false),
        Command::Build { args } => build(&args, true),
        Command::Coverage => coverage(),
        Command::Init { dir } => init(&dir),
        Command::Stubs { dir } => stubs(&dir),
        Command::Generate(Generate::Table { snapshot }) => table(&snapshot),
    }
}

fn check(files: &[PathBuf]) -> ExitCode {
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
        // was would be worse than no `check` at all (§15.28).
        let mut diags = Diagnostics::new();
        installua::compile_with(&source, &installua::Options::for_file(path), &mut diags);
        report(&diags, path);
        failed |= diags.has_errors();
    }

    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn build(args: &BuildArgs, assemble: bool) -> ExitCode {
    // Before the compile, not after it: the invocation is wrong whatever the
    // program says, and reporting a program's diagnostics first would bury the
    // one message that is actually actionable.
    if args.stdout && assemble {
        return usage_error("`--stdout` has no script for `makensis` to read; use `emit`");
    }

    let input = &args.input;
    let Some(source) = read(input) else {
        return ExitCode::from(2);
    };

    // Relative paths in the source resolve against the *source's* directory,
    // not the shell's: a `glob` means the same thing wherever the build is run
    // from, which is what makes the output reproducible (§14).
    let options = installua::Options::for_file(input);

    let mut diags = Diagnostics::new();
    let result = installua::build_mapped(&source, &options, &mut diags);
    report(&diags, input);

    let Some((nsi, map)) = result else {
        return ExitCode::FAILURE;
    };

    if args.stdout {
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
    // NSIS can read its own line number and cannot be told a different one
    // (§15.22).
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

/// `installua coverage`: §14's census, printed.
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
/// too (§15.28).
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

    let files = [
        (meta.join("installua.lua"), installua::stubs::meta()),
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

/// `installua generate table <cmdhelp.txt>`: the generator half of §15.23's join.
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

/// Every diagnostic, not just the first (§9-4).
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
