use std::path::PathBuf;
use std::process::ExitCode;

use app_core::{brand, meta};

fn main() -> ExitCode {
    let mut directory = PathBuf::from("assets");
    let mut check = false;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.trim_start_matches('-') {
            "check" => check = true,
            "out" => match arguments.next() {
                Some(value) => directory = PathBuf::from(value),
                None => {
                    eprintln!("error: -out needs a directory");
                    return ExitCode::from(2);
                }
            },
            _ => {
                eprintln!("error: unknown argument {argument}. Usage: genassets [-out DIRECTORY] [-check]");
                return ExitCode::from(2);
            }
        }
    }
    let files = brand::files(meta::APP_NAME, meta::TAGLINE, meta::SLUG);
    match brand::write_files(&directory, &files, check) {
        Ok(written) => {
            for path in written {
                println!("wrote {path}");
            }
            ExitCode::SUCCESS
        }
        Err(problem) => {
            eprintln!("error: {problem}");
            ExitCode::FAILURE
        }
    }
}
