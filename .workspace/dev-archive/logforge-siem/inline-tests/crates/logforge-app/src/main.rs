#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launcher_decision_table() {
        assert_eq!(
            decide_launch_mode(Some(UiMode::Gui), None, None, false, false, true, true),
            LaunchDecision::Gui
        );
        assert_eq!(
            decide_launch_mode(Some(UiMode::Tui), None, None, false, false, true, true),
            LaunchDecision::Tui
        );
        assert_eq!(
            decide_launch_mode(None, Some("tui"), None, false, false, true, true),
            LaunchDecision::Tui
        );
        assert_eq!(
            decide_launch_mode(None, None, Some(UiMode::Gui), false, false, false, false),
            LaunchDecision::Tui
        );
        assert_eq!(
            decide_launch_mode(None, None, Some(UiMode::Gui), true, false, true, true),
            LaunchDecision::Prompt
        );
        assert_eq!(
            decide_launch_mode(None, None, None, false, true, true, true),
            LaunchDecision::Tui
        );
    }

    #[test]
    fn demo_load_is_a_supported_nested_command() {
        assert!(Cli::try_parse_from(["logforge-siem", "demo", "load"]).is_ok());
        assert!(Cli::try_parse_from(["logforge-siem", "demo", "unknown"]).is_err());
    }
}
