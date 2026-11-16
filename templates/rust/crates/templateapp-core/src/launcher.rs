use std::io::{BufRead, Write};

use crate::meta;
use crate::platform::{Env, Os};

pub const MODE_GUI: &str = "gui";
pub const MODE_TUI: &str = "tui";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    RunCommand,
    OpenGui,
    OpenTui,
    PromptTerminal,
    ShowChooser,
    OpenTerminalWindow,
    PrintHelp,
    Fail,
}

impl Action {
    pub fn label(self) -> &'static str {
        match self {
            Action::RunCommand => "run command",
            Action::OpenGui => "open dashboard",
            Action::OpenTui => "open terminal UI",
            Action::PromptTerminal => "prompt in terminal",
            Action::ShowChooser => "show chooser",
            Action::OpenTerminalWindow => "open terminal window",
            Action::PrintHelp => "print help",
            Action::Fail => "fail",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Input {
    pub ui_flag: String,
    pub ask: bool,
    pub no_prompt: bool,
    pub has_subcommand: bool,
    pub env_ui: String,
    pub remembered: String,
    pub stdin_interactive: bool,
    pub terminal_output: bool,
    pub ci: bool,
    pub headless: bool,
    pub desktop_launch: bool,
    pub gui_available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    pub action: Action,
    pub default: String,
    pub reason: String,
}

fn valid_mode(value: &str) -> bool {
    value == MODE_GUI || value == MODE_TUI
}

fn decision(action: Action, default: &str, reason: impl Into<String>) -> Decision {
    Decision { action, default: default.to_string(), reason: reason.into() }
}

pub fn decide(input: &Input) -> Decision {
    if input.has_subcommand {
        return decision(Action::RunCommand, "", "an explicit subcommand never opens the chooser");
    }
    let flag = input.ui_flag.trim().to_lowercase();
    let env = input.env_ui.trim().to_lowercase();
    if !flag.is_empty() && !valid_mode(&flag) {
        return decision(Action::Fail, "", format!("--ui \"{}\" is not valid: use --ui gui or --ui tui", input.ui_flag));
    }
    if flag.is_empty() && !env.is_empty() && !valid_mode(&env) {
        return decision(
            Action::Fail,
            "",
            format!("{}=\"{}\" is not valid: use gui or tui", meta::env_name("ui"), input.env_ui),
        );
    }
    let terminal = input.stdin_interactive && input.terminal_output;
    let (explicit, origin) = if flag.is_empty() { (env, meta::env_name("ui")) } else { (flag, "--ui".to_string()) };
    if explicit == MODE_GUI {
        if !input.gui_available {
            return decision(
                Action::Fail,
                "",
                format!("{origin} asks for the dashboard but this build has no dashboard: use --ui tui or a build with the GUI"),
            );
        }
        if input.headless {
            return decision(
                Action::Fail,
                "",
                format!("{origin} asks for the dashboard but no display is available: use --ui tui"),
            );
        }
        return decision(Action::OpenGui, "", format!("requested with {origin}"));
    }
    if explicit == MODE_TUI {
        if terminal {
            return decision(Action::OpenTui, "", format!("requested with {origin}"));
        }
        if input.desktop_launch && !input.headless {
            return decision(Action::OpenTerminalWindow, "", format!("requested with {origin} from a desktop launch"));
        }
        return decision(
            Action::Fail,
            "",
            format!("{origin} asks for the terminal UI but this session is not an interactive terminal: run a subcommand instead"),
        );
    }
    let fallback = if input.gui_available && !input.headless { MODE_GUI } else { MODE_TUI };
    let remembered = valid_mode(&input.remembered);
    let preferred =
        if remembered && (input.remembered == MODE_TUI || fallback == MODE_GUI) { input.remembered.as_str() } else { fallback };
    let open = |mode: &str, reason: &str| -> Decision {
        if mode == MODE_GUI {
            decision(Action::OpenGui, mode, reason)
        } else if terminal {
            decision(Action::OpenTui, mode, reason)
        } else if input.desktop_launch && !input.headless {
            decision(Action::OpenTerminalWindow, mode, reason)
        } else {
            decision(
                Action::PrintHelp,
                mode,
                "this session is not interactive, so nothing was opened: run a subcommand or pass --ui",
            )
        }
    };
    if input.headless {
        return open(MODE_TUI, "no display is available, opening the terminal UI");
    }
    if input.ci {
        if input.desktop_launch {
            return open(preferred, "automation environment, not prompting");
        }
        return decision(
            Action::PrintHelp,
            preferred,
            "automation environment detected, so nothing was opened: run a subcommand or pass --ui",
        );
    }
    if input.desktop_launch {
        if !input.gui_available {
            return open(MODE_TUI, "this build has no dashboard");
        }
        if remembered && !input.ask {
            return open(preferred, "remembered choice");
        }
        if input.no_prompt {
            return open(preferred, "--no-prompt given");
        }
        return decision(Action::ShowChooser, preferred, "desktop launch");
    }
    if !terminal {
        return decision(
            Action::PrintHelp,
            preferred,
            "standard input is not interactive, so nothing was opened: run a subcommand or pass --ui",
        );
    }
    if input.no_prompt {
        return open(preferred, "--no-prompt given");
    }
    if remembered && !input.ask {
        return open(preferred, "remembered choice");
    }
    if !input.gui_available {
        return open(MODE_TUI, "this build has no dashboard");
    }
    decision(Action::PromptTerminal, preferred, "interactive terminal")
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PromptResult {
    pub mode: String,
    pub remember: bool,
    pub quit: bool,
}

pub fn prompt_terminal(input: &mut dyn BufRead, output: &mut dyn Write, default_mode: &str) -> std::io::Result<PromptResult> {
    let default_mode = if valid_mode(default_mode) { default_mode } else { MODE_GUI };
    let marker = |mode: &str| if mode == default_mode { " (default)" } else { "" };
    writeln!(output, "How do you want to open {}?", meta::APP_NAME)?;
    writeln!(output, "  1  Dashboard window{}", marker(MODE_GUI))?;
    writeln!(output, "  2  Terminal UI{}", marker(MODE_TUI))?;
    writeln!(output, "  r  Toggle \"remember my choice\"")?;
    writeln!(output, "  q  Quit")?;
    let quit = PromptResult { quit: true, ..PromptResult::default() };
    let mut remember = false;
    for _ in 0..5 {
        let key = if default_mode == MODE_GUI { "1" } else { "2" };
        write!(output, "Choice [Enter = {key}, remember: {}]: ", if remember { "on" } else { "off" })?;
        output.flush()?;
        let mut line = Vec::new();
        let read = std::io::Read::take(&mut *input, 4096).read_until(b'\n', &mut line)?;
        let ended = read == 0 || line.last() != Some(&b'\n');
        let answer = String::from_utf8_lossy(&line).trim().to_lowercase();
        if ended && answer.is_empty() {
            writeln!(output)?;
            return Ok(quit);
        }
        match answer.as_str() {
            "" => return Ok(PromptResult { mode: default_mode.to_string(), remember, quit: false }),
            "1" | "d" | "dashboard" | MODE_GUI => {
                return Ok(PromptResult { mode: MODE_GUI.to_string(), remember, quit: false });
            }
            "2" | "t" | "terminal" | MODE_TUI => {
                return Ok(PromptResult { mode: MODE_TUI.to_string(), remember, quit: false });
            }
            "r" | "remember" => remember = !remember,
            "q" | "quit" | "exit" => return Ok(quit),
            _ => writeln!(output, "Enter 1, 2, r or q.")?,
        }
        if ended {
            return Ok(quit);
        }
    }
    writeln!(output, "No valid choice after several attempts.")?;
    Ok(quit)
}

const AUTOMATION_VARIABLES: [&str; 7] = ["CI", "GITHUB_ACTIONS", "GITLAB_CI", "JENKINS_URL", "BUILDKITE", "TF_BUILD", "TEAMCITY_VERSION"];

#[derive(Clone)]
pub struct Session {
    pub os: Os,
    pub env: Env,
    pub stdin_terminal: bool,
    pub stdout_terminal: bool,
    pub stderr_terminal: bool,
    pub console_owners: u32,
}

impl Session {
    pub fn ci(&self) -> bool {
        AUTOMATION_VARIABLES.iter().any(|name| {
            self.env.get(name).is_some_and(|value| {
                let lowered = value.to_lowercase();
                lowered != "false" && lowered != "0"
            })
        })
    }

    pub fn headless(&self) -> bool {
        match self.os {
            Os::Windows => false,
            Os::Macos => self.env.get("SSH_CONNECTION").is_some() || self.env.get("SSH_TTY").is_some(),
            Os::Linux => self.env.get("DISPLAY").is_none() && self.env.get("WAYLAND_DISPLAY").is_none(),
        }
    }

    pub fn desktop_launch(&self) -> bool {
        if self.headless() || self.ci() {
            return false;
        }
        if self.os == Os::Windows {
            return self.console_owners <= 1;
        }
        if self.stdin_terminal || self.stdout_terminal || self.stderr_terminal {
            return false;
        }
        self.env.get("TERM").is_none_or(|term| term == "dumb")
    }

    pub fn input(&self, base: Input) -> Input {
        Input {
            stdin_interactive: self.stdin_terminal,
            terminal_output: self.stdout_terminal,
            ci: self.ci(),
            headless: self.headless(),
            desktop_launch: self.desktop_launch(),
            ..base
        }
    }
}

pub fn requests_terminal(arguments: &[String]) -> bool {
    for (index, argument) in arguments.iter().enumerate() {
        if argument == "--" {
            return false;
        }
        if argument == "--ui=tui" || (argument == "--ui" && arguments.get(index + 1).is_some_and(|next| next == MODE_TUI)) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terminal() -> Input {
        Input { stdin_interactive: true, terminal_output: true, gui_available: true, ..Input::default() }
    }

    fn desktop() -> Input {
        Input { desktop_launch: true, gui_available: true, ..Input::default() }
    }

    #[test]
    fn a_subcommand_always_runs_and_never_prompts() {
        for base in [terminal(), desktop(), Input::default()] {
            let made = decide(&Input { has_subcommand: true, ui_flag: "nonsense".to_string(), ..base });
            assert_eq!(made.action, Action::RunCommand);
        }
    }

    #[test]
    fn the_decision_table_holds() {
        let cases: Vec<(&str, Input, Action, &str)> = vec![
            ("terminal first run", terminal(), Action::PromptTerminal, "gui"),
            ("terminal remembered tui", Input { remembered: "tui".to_string(), ..terminal() }, Action::OpenTui, "tui"),
            ("terminal remembered gui", Input { remembered: "gui".to_string(), ..terminal() }, Action::OpenGui, "gui"),
            ("terminal ask again", Input { remembered: "tui".to_string(), ask: true, ..terminal() }, Action::PromptTerminal, "tui"),
            ("terminal no prompt", Input { no_prompt: true, ..terminal() }, Action::OpenGui, "gui"),
            ("terminal without dashboard", Input { gui_available: false, ..terminal() }, Action::OpenTui, "tui"),
            ("headless terminal", Input { headless: true, ..terminal() }, Action::OpenTui, "tui"),
            ("headless remembered gui", Input { headless: true, remembered: "gui".to_string(), ..terminal() }, Action::OpenTui, "tui"),
            ("headless pipe", Input { headless: true, gui_available: true, ..Input::default() }, Action::PrintHelp, "tui"),
            ("automation", Input { ci: true, ..terminal() }, Action::PrintHelp, "gui"),
            ("pipe", Input { gui_available: true, terminal_output: true, ..Input::default() }, Action::PrintHelp, "gui"),
            ("desktop first run", desktop(), Action::ShowChooser, "gui"),
            ("desktop remembered gui", Input { remembered: "gui".to_string(), ..desktop() }, Action::OpenGui, "gui"),
            ("desktop remembered tui", Input { remembered: "tui".to_string(), ..desktop() }, Action::OpenTerminalWindow, "tui"),
            ("desktop ask again", Input { remembered: "tui".to_string(), ask: true, ..desktop() }, Action::ShowChooser, "tui"),
            ("desktop no prompt", Input { no_prompt: true, ..desktop() }, Action::OpenGui, "gui"),
            ("desktop without dashboard", Input { gui_available: false, ..desktop() }, Action::OpenTerminalWindow, "tui"),
            ("flag gui", Input { ui_flag: "gui".to_string(), ..terminal() }, Action::OpenGui, ""),
            ("flag tui", Input { ui_flag: " TUI ".to_string(), ..terminal() }, Action::OpenTui, ""),
            ("flag tui from desktop", Input { ui_flag: "tui".to_string(), ..desktop() }, Action::OpenTerminalWindow, ""),
            ("flag tui in a pipe", Input { ui_flag: "tui".to_string(), gui_available: true, ..Input::default() }, Action::Fail, ""),
            ("flag gui headless", Input { ui_flag: "gui".to_string(), headless: true, ..terminal() }, Action::Fail, ""),
            ("flag gui without dashboard", Input { ui_flag: "gui".to_string(), gui_available: false, ..terminal() }, Action::Fail, ""),
            ("flag invalid", Input { ui_flag: "web".to_string(), ..terminal() }, Action::Fail, ""),
            ("env tui", Input { env_ui: "tui".to_string(), ..terminal() }, Action::OpenTui, ""),
            ("env invalid", Input { env_ui: "web".to_string(), ..terminal() }, Action::Fail, ""),
            ("flag beats env", Input { ui_flag: "gui".to_string(), env_ui: "tui".to_string(), ..terminal() }, Action::OpenGui, ""),
            ("flag beats invalid env", Input { ui_flag: "tui".to_string(), env_ui: "web".to_string(), ..terminal() }, Action::OpenTui, ""),
        ];
        for (name, input, action, default) in cases {
            let made = decide(&input);
            assert_eq!((made.action, made.default.as_str()), (action, default), "{name}: {}", made.reason);
            assert!(!made.reason.is_empty(), "{name}");
        }
        assert_eq!(Action::ShowChooser.label(), "show chooser");
    }

    fn ask(typed: &str, default_mode: &str) -> (PromptResult, String) {
        let mut input = typed.as_bytes();
        let mut output = Vec::new();
        let result = prompt_terminal(&mut input, &mut output, default_mode).unwrap();
        (result, String::from_utf8(output).unwrap())
    }

    #[test]
    fn the_terminal_prompt_accepts_keys_words_and_enter() {
        assert_eq!(ask("\n", "tui").0, PromptResult { mode: "tui".to_string(), remember: false, quit: false });
        assert_eq!(ask("1\n", "tui").0.mode, "gui");
        assert_eq!(ask("Terminal\n", "gui").0.mode, "tui");
        assert_eq!(ask("r\n2\n", "gui").0, PromptResult { mode: "tui".to_string(), remember: true, quit: false });
        assert_eq!(ask("r\nr\n\n", "gui").0, PromptResult { mode: "gui".to_string(), remember: false, quit: false });
        assert!(ask("q\n", "gui").0.quit);
        assert!(ask("", "gui").0.quit);
        assert_eq!(ask("2", "gui").0.mode, "tui");
        let (result, shown) = ask("x\ny\nz\nw\nv\n", "gui");
        assert!(result.quit && shown.contains("Enter 1, 2, r or q.") && shown.contains("No valid choice"));
        let (_, shown) = ask("\n", "anything");
        assert!(shown.contains("Dashboard window (default)") && shown.contains("remember: off"), "{shown}");
    }

    fn session(os: Os, pairs: &[(&str, &str)], terminal: bool, console_owners: u32) -> Session {
        Session {
            os,
            env: Env::from_pairs(pairs),
            stdin_terminal: terminal,
            stdout_terminal: terminal,
            stderr_terminal: terminal,
            console_owners,
        }
    }

    #[test]
    fn sessions_are_classified_per_platform() {
        let linux_terminal = session(Os::Linux, &[("DISPLAY", ":0"), ("TERM", "xterm")], true, 0);
        assert!(!linux_terminal.headless() && !linux_terminal.desktop_launch() && !linux_terminal.ci());
        let linux_menu = session(Os::Linux, &[("WAYLAND_DISPLAY", "wayland-0")], false, 0);
        assert!(linux_menu.desktop_launch());
        let linux_server = session(Os::Linux, &[("TERM", "xterm")], true, 0);
        assert!(linux_server.headless() && !linux_server.desktop_launch());
        let linux_pipe = session(Os::Linux, &[("DISPLAY", ":0"), ("TERM", "xterm")], false, 0);
        assert!(!linux_pipe.desktop_launch());
        let mac_remote = session(Os::Macos, &[("SSH_TTY", "/dev/ttys001")], true, 0);
        assert!(mac_remote.headless());
        let mac_finder = session(Os::Macos, &[], false, 0);
        assert!(mac_finder.desktop_launch());
        let windows_explorer = session(Os::Windows, &[], false, 1);
        assert!(windows_explorer.desktop_launch() && !windows_explorer.headless());
        let windows_shell = session(Os::Windows, &[], true, 2);
        assert!(!windows_shell.desktop_launch());
        let automated = session(Os::Linux, &[("DISPLAY", ":0"), ("CI", "true")], false, 0);
        assert!(automated.ci() && !automated.desktop_launch());
        assert!(!session(Os::Linux, &[("CI", "false")], false, 0).ci());
        let filled = linux_menu.input(Input { gui_available: true, ..Input::default() });
        assert!(filled.desktop_launch && filled.gui_available && !filled.stdin_interactive);
    }

    #[test]
    fn the_terminal_request_is_read_from_the_arguments() {
        let words = |text: &str| text.split(' ').map(str::to_string).collect::<Vec<_>>();
        assert!(requests_terminal(&words("--ui tui")));
        assert!(requests_terminal(&words("--db x --ui=tui")));
        assert!(!requests_terminal(&words("--ui gui")));
        assert!(!requests_terminal(&words("-- --ui tui")));
        assert!(!requests_terminal(&words("--ui")));
    }
}
