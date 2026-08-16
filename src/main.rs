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
  installua build <file.lua> [options]   compile to .nsi

options:
  -o <file.nsi>   write here instead of alongside the input
  --stdout        write to stdout
  -h, --help      this";

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
        Some((&"build", rest)) => build(rest),
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

fn build(args: &[&str]) -> ExitCode {
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

    let mut diags = Diagnostics::new();
    let result = installua::build(&source, &mut diags);
    report(&diags, &input);

    let Some(nsi) = result else {
        return ExitCode::FAILURE;
    };

    if to_stdout {
        print!("{nsi}");
        return ExitCode::SUCCESS;
    }

    let output = output.unwrap_or_else(|| input.with_extension("nsi"));
    if let Err(error) = std::fs::write(&output, nsi) {
        eprintln!("installua: cannot write {}: {error}", output.display());
        return ExitCode::from(2);
    }

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

fn usage_error(message: &str) -> ExitCode {
    eprintln!("installua: {message}\n\n{USAGE}");
    ExitCode::from(2)
}
