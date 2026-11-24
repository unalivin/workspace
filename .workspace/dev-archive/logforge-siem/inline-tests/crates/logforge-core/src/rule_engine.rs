#[cfg(test)]
mod tests {
    use super::*;

    fn rule(pattern: &str, enabled: bool) -> DetectionRule {
        DetectionRule {
            id: 1,
            name: "test".into(),
            severity: "warn".into(),
            pattern: pattern.into(),
            enabled,
            created_at: String::new(),
        }
    }

    #[test]
    fn supports_case_insensitive_contains_boundaries_boolean_groups_and_negation() {
        assert!(evaluate_rule(&rule("contains:FAILED", true), "login failed").unwrap());
        assert!(evaluate_rule(&rule("starts:login", true), "Login from host").unwrap());
        assert!(evaluate_rule(&rule("ends:denied", true), "request denied").unwrap());
        assert!(evaluate_rule(&rule("equals:HEALTH", true), "health").unwrap());
        assert!(evaluate_rule(
            &rule("all:contains:login&contains:denied", true),
            "login denied"
        )
        .unwrap());
        assert!(evaluate_rule(
            &rule("any:contains:login&contains:blocked", true),
            "login denied"
        )
        .unwrap());
        assert!(evaluate_rule(&rule("not:contains:allowed", true), "login denied").unwrap());
    }

    #[test]
    fn malformed_or_disabled_rules_never_match() {
        assert_eq!(validate_pattern(""), Err(PatternError::Empty));
        assert!(validate_pattern("unknown:value").is_err());
        assert!(!evaluate_rule(&rule("contains:secret", false), "secret").unwrap());
        assert!(evaluate_rules(&[rule("bad:", true)], "bad").is_empty());
    }
}
