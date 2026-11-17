use std::io::IsTerminal;
use std::path::PathBuf;

use app_cli::{Env, Hooks};
use app_core::launcher::Session;
use app_core::notifier::{self, Autostart};
use app_core::platform::{self, Os};
use app_core::service::{App, Options};

fn main() {
    let options = Options::system();
    let os = Os::current();
    let stdin = std::io::stdin();
    let terminals = (stdin.is_terminal(), std::io::stdout().is_terminal(), std::io::stderr().is_terminal());
    let session = Session {
        os,
        env: options.env.clone(),
        stdin_terminal: terminals.0,
        stdout_terminal: terminals.1,
        stderr_terminal: terminals.2,
        console_owners: if terminals.0 || terminals.1 || terminals.2 { 2 } else { 1 },
    };
    let executable = std::env::current_exe().unwrap_or_else(|_| PathBuf::from(app_core::meta::SLUG));
    let environment = options.env.clone();
    let deliver = |title: &str, body: &str| notifier::deliver_with_system(os, title, body);
    let autostart = || Autostart::for_system(os, &environment, &executable);
    let window = |program: &str, arguments: &[String]| {
        platform::open_terminal_window(os, program, arguments, &platform::spawn_detached)
    };
    let open_url = move |url: &str| platform::open_repository_url(os, url, &platform::spawn_detached);
    let open_tui = |app: &mut App| {
        app_tui::run(
            app,
            app_tui::Options {
                env: environment.clone(),
                system_dark: app_tui::screen::dark_background(&environment),
                autostart: Some(autostart()),
                has_dashboard: cfg!(feature = "gui"),
                open_url: Some(Box::new(open_url)),
            },
        )
    };
    #[cfg(feature = "gui")]
    let open_gui = |app: &mut App| {
        let program = executable.to_string_lossy().into_owned();
        app_gui::run(
            app,
            app_gui::Options {
                env: environment.clone(),
                system_dark: false,
                autostart: Some(autostart()),
                in_terminal: terminals.0 && terminals.1,
                open_terminal: Some(Box::new(move |arguments: &[String]| {
                    platform::open_terminal_window(os, &program, arguments, &platform::spawn_detached)
                })),
                open_url: Some(Box::new(open_url)),
            },
        )
    };
    #[cfg(feature = "gui")]
    let hooks = Hooks { open_gui: Some(&open_gui), show_chooser: Some(&app_gui::choose), ..Hooks::default() };
    #[cfg(not(feature = "gui"))]
    let hooks = Hooks::default();
    let mut input = stdin.lock();
    let mut output = std::io::stdout();
    let mut errors = std::io::stderr();
    let code = app_cli::run(&mut Env {
        args: std::env::args().skip(1).collect(),
        stdin: &mut input,
        stdout: &mut output,
        stderr: &mut errors,
        options,
        session,
        executable: executable.to_string_lossy().into_owned(),
        hooks: Hooks {
            deliver: Some(&deliver),
            autostart: Some(&autostart),
            open_terminal_window: Some(&window),
            open_tui: Some(&open_tui),
            ..hooks
        },
    });
    drop(input);
    std::process::exit(code);
}
