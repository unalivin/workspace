use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;

use app_core::clock::{Clock, Zone};
use app_core::platform::{Env, Os};
use app_core::png;
use app_core::service::{self, App};
use app_core::testkit::{TempDir, fixed_time};
use app_gui::{Options, shots};

fn render(directory: &PathBuf) -> Result<usize, String> {
    let home = TempDir::new("screens");
    let folder = home.path().to_string_lossy().into_owned();
    let env = Env::from_pairs(&[("HOME", folder.as_str())]);
    let mut app = App::open(service::Options {
        os: Os::Linux,
        env: env.clone(),
        config_dir: None,
        database_path: None,
        overrides: BTreeMap::new(),
        clock: Clock::fixed(fixed_time()),
        zone: Zone::Utc,
        read_only: false,
    })
    .map_err(|error| error.to_string())?;
    let options =
        Options { env, system_dark: false, autostart: None, in_terminal: false, open_terminal: None, open_url: None };
    std::fs::create_dir_all(directory).map_err(|error| format!("{}: {error}", directory.display()))?;
    let pictures = shots::screens(&mut app, options);
    for (name, picture) in &pictures {
        let file = directory.join(format!("{name}.png"));
        let data = png::encode(picture.width, picture.height, &picture.pixels);
        std::fs::write(&file, data).map_err(|error| format!("{}: {error}", file.display()))?;
    }
    Ok(pictures.len())
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let directory = match arguments.as_slice() {
        [flag, directory] if flag == "-out" => PathBuf::from(directory),
        [] => PathBuf::from("dist/screens"),
        _ => {
            eprintln!("usage: screenshots [-out DIRECTORY]");
            return ExitCode::from(2);
        }
    };
    match render(&directory) {
        Ok(count) => {
            println!("wrote {count} screens to {}", directory.display());
            ExitCode::SUCCESS
        }
        Err(problem) => {
            eprintln!("screenshots: {problem}");
            ExitCode::FAILURE
        }
    }
}
