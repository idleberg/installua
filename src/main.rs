//! The CLI: a thin shell over the library (§15.12).
//!
//! Nothing is decided here. Argument parsing, file reading and exit codes are
//! the whole of it — every question about the language is answered by a
//! `installua::` call that an editor could make just as well.
//!
//! Exit codes: 0 success, 1 the source was rejected, 2 the invocation was.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use installua::diag::Diagnostics;

const USAGE: &str = "\
usage:
  installua check <file.lua>...          parse and check, emit nothing
  installua emit  <file.lua> [options]   compile to .nsi and stop
  installua build <file.lua> [options]   compile, then run `makensis -WX`

options:
  -o <file.nsi>   write here instead of alongside the input
  --stdout        write to stdout (`emit` only)
  -h, --help      this

`emit` is for wiring Installua into an existing build; `build` owns the
`makensis` invocation, which is what lets it rewrite `makensis`\'s diagnostics
back onto the Lua source (\u{a7}15.22).";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();

    match args.split_first() {
        None => usage_error("no command"),
        Some((&"-h" | &"--help", _)) => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some((&"check", rest)) => check(rest),
        Some((&"emit", rest)) => build(rest, false),
        Some((&"build", rest)) => build(rest, true),
        Some((other, _)) => usage_error(&format!("unknown command `{other}`")),
    }
}

fn check(args: &[&str]) -> ExitCode {
    if args.is_empty() {
        return usage_error("`check` needs at least one file");
    }

    let mut failed = false;
    for arg in args {
        if arg.starts_with('-') {
            return usage_error(&format!("`check` takes no options, got `{arg}`"));
        }
        let path = PathBuf::from(arg);
        let Some(source) = read(&path) else {
            return ExitCode::from(2);
        };

        let mut diags = Diagnostics::new();
        installua::check(&source, &mut diags);
        report(&diags, &path);
        failed |= diags.has_errors();
    }

    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn build(args: &[&str], assemble: bool) -> ExitCode {
    let mut input: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut to_stdout = false;

    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match *arg {
            "--stdout" => to_stdout = true,
            "-o" => match args.next() {
                Some(path) => output = Some(PathBuf::from(path)),
                None => return usage_error("`-o` needs a path"),
            },
            other if other.starts_with('-') => {
                return usage_error(&format!("unknown option `{other}`"));
            }
            other if input.is_none() => input = Some(PathBuf::from(other)),
            other => return usage_error(&format!("unexpected argument `{other}`")),
        }
    }

    let Some(input) = input else {
        return usage_error("`build` needs an input file");
    };
    let Some(source) = read(&input) else {
        return ExitCode::from(2);
    };

    // Relative paths in the source resolve against the *source's* directory,
    // not the shell's: a `glob` means the same thing wherever the build is run
    // from, which is what makes the output reproducible (§14).
    let options = installua::Options {
        base: Some(input.parent().unwrap_or(Path::new(".")).to_path_buf()),
    };

    let mut diags = Diagnostics::new();
    let result = installua::build_mapped(&source, &options, &mut diags);
    report(&diags, &input);

    let Some((nsi, map)) = result else {
        return ExitCode::FAILURE;
    };

    if to_stdout {
        if assemble {
            return usage_error("`--stdout` has no script for `makensis` to read; use `emit`");
        }
        print!("{nsi}");
        return ExitCode::SUCCESS;
    }

    let output = output.unwrap_or_else(|| input.with_extension("nsi"));
    if let Err(error) = std::fs::write(&output, nsi) {
        eprintln!("installua: cannot write {}: {error}", output.display());
        return ExitCode::from(2);
    }
    if !assemble {
        return ExitCode::SUCCESS;
    }

    // The invocation is ours because the map cannot travel with the artifact:
    // NSIS can read its own line number and cannot be told a different one
    // (\u{a7}15.22).
    let makensis = std::env::var("MAKENSIS").unwrap_or_else(|_| "makensis".to_string());
    let source_name = input.display().to_string();
    match installua::assemble::assemble(&output, &map, &source_name, &makensis) {
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

fn usage_error(message: &str) -> ExitCode {
    eprintln!("installua: {message}\n\n{USAGE}");
    ExitCode::from(2)
}
