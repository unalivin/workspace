use std::path::PathBuf;
use std::process::ExitCode;

use app_core::service;

fn replace_block(document: &str, name: &str, content: &str) -> Result<String, String> {
    let start = format!("<!-- {name}:start -->\n");
    let end = format!("<!-- {name}:end -->");
    let from = document.find(&start).ok_or_else(|| format!("docs/COMMANDS.md lacks the {name} start marker"))?;
    let to = document.find(&end).ok_or_else(|| format!("docs/COMMANDS.md lacks the {name} end marker"))?;
    if to < from {
        return Err(format!("docs/COMMANDS.md has the {name} markers in the wrong order"));
    }
    Ok(format!("{}{content}{}", &document[..from + start.len()], &document[to..]))
}

fn run(check: bool) -> Result<(), String> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/COMMANDS.md");
    let raw = std::fs::read_to_string(&path).map_err(|problem| format!("docs/COMMANDS.md cannot be read: {problem}"))?;
    let current = raw.replace("\r\n", "\n");
    let wanted = replace_block(&current, "capabilities", &service::capability_table())?;
    let wanted = replace_block(&wanted, "reference", &app_cli::reference())?;
    if wanted == current {
        return Ok(());
    }
    if check {
        return Err("docs/COMMANDS.md is out of date: run make docs".to_string());
    }
    std::fs::write(&path, wanted).map_err(|problem| format!("docs/COMMANDS.md cannot be written: {problem}"))?;
    println!("wrote docs/COMMANDS.md");
    Ok(())
}

fn main() -> ExitCode {
    let mut check = false;
    for argument in std::env::args().skip(1) {
        match argument.trim_start_matches('-') {
            "check" => check = true,
            _ => {
                eprintln!("error: unknown argument {argument}. Usage: gendocs [-check]");
                return ExitCode::from(2);
            }
        }
    }
    match run(check) {
        Ok(()) => ExitCode::SUCCESS,
        Err(problem) => {
            eprintln!("error: {problem}");
            ExitCode::FAILURE
        }
    }
}
