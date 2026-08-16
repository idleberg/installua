//! CLI (§9-7): no shelling out to `makensis`, real exit codes, `--stdout`.

use std::path::PathBuf;
use std::process::ExitCode;

use luis_poc::compile;
use luis_poc::diag::Diagnostics;

const USAGE: &str = "usage: luis <input.lua> [-o <output.nsi> | --stdout]";

fn main() -> ExitCode {
    let mut input: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut to_stdout = false;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--stdout" => to_stdout = true,
            "-o" => match args.next() {
                Some(path) => output = Some(PathBuf::from(path)),
                None => return usage_error("-o requires a path"),
            },
            "-h" | "--help" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            other if other.starts_with('-') => {
                return usage_error(&format!("unknown option `{other}`"));
            }
            other if input.is_none() => input = Some(PathBuf::from(other)),
            other => return usage_error(&format!("unexpected argument `{other}`")),
        }
    }

    let Some(input) = input else {
        return usage_error("no input file");
    };

    let source = match std::fs::read_to_string(&input) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("luis: cannot read {}: {error}", input.display());
            return ExitCode::from(2);
        }
    };

    let mut diags = Diagnostics::default();
    let result = compile(&source, &mut diags);

    if !diags.is_empty() {
        eprint!("{}", diags.render(&input.display().to_string()));
    }

    let Some(nsi) = result else {
        return ExitCode::FAILURE;
    };

    if to_stdout {
        print!("{nsi}");
        return ExitCode::SUCCESS;
    }

    let output = output.unwrap_or_else(|| input.with_extension("nsi"));
    if let Err(error) = std::fs::write(&output, nsi) {
        eprintln!("luis: cannot write {}: {error}", output.display());
        return ExitCode::from(2);
    }

    ExitCode::SUCCESS
}

fn usage_error(message: &str) -> ExitCode {
    eprintln!("luis: {message}\n{USAGE}");
    ExitCode::from(2)
}
