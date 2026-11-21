use crate::model::DetectionRule;

#[derive(Debug, Clone)]
pub struct RuleMatch<'a> {
    pub rule: &'a DetectionRule,
    pub matched_pattern: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatternError {
    Empty,
    UnknownOperator(String),
    EmptyOperand,
}

impl std::fmt::Display for PatternError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => formatter.write_str("rule pattern cannot be empty"),
            Self::UnknownOperator(operator) => {
                write!(formatter, "unknown rule operator: {operator}")
            }
            Self::EmptyOperand => formatter.write_str("rule expression contains an empty operand"),
        }
    }
}

impl std::error::Error for PatternError {}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Predicate {
    Contains(String),
    StartsWith(String),
    EndsWith(String),
    Equals(String),
    All(Vec<Predicate>),
    Any(Vec<Predicate>),
    Not(Box<Predicate>),
}

impl Predicate {
    fn matches(&self, text: &str) -> bool {
        match self {
            Self::Contains(value) => text.to_lowercase().contains(&value.to_lowercase()),
            Self::StartsWith(value) => text.to_lowercase().starts_with(&value.to_lowercase()),
            Self::EndsWith(value) => text.to_lowercase().ends_with(&value.to_lowercase()),
            Self::Equals(value) => text.eq_ignore_ascii_case(value),
            Self::All(parts) => parts.iter().all(|part| part.matches(text)),
            Self::Any(parts) => parts.iter().any(|part| part.matches(text)),
            Self::Not(part) => !part.matches(text),
        }
    }
}

fn split_escaped(input: &str, delimiter: char) -> Vec<String> {
    let mut parts = vec![String::new()];
    let mut escaped = false;
    for character in input.chars() {
        if escaped {
            parts.last_mut().expect("one initial part").push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == delimiter {
            parts.push(String::new());
        } else {
            parts.last_mut().expect("one initial part").push(character);
        }
    }
    if escaped {
        parts.last_mut().expect("one initial part").push('\\');
    }
    parts
}

fn parse_predicate(pattern: &str) -> Result<Predicate, PatternError> {
    let pattern = pattern.trim();
    if pattern.is_empty() {
        return Err(PatternError::Empty);
    }
    let (operator, argument) = pattern.split_once(':').unwrap_or(("contains", pattern));
    match operator.to_ascii_lowercase().as_str() {
        "contains" | "starts" | "ends" | "equals" => {
            if argument.is_empty() {
                return Err(PatternError::EmptyOperand);
            }
            Ok(match operator.to_ascii_lowercase().as_str() {
                "starts" => Predicate::StartsWith(argument.to_string()),
                "ends" => Predicate::EndsWith(argument.to_string()),
                "equals" => Predicate::Equals(argument.to_string()),
                _ => Predicate::Contains(argument.to_string()),
            })
        }
        "all" | "any" => {
            let parts = split_escaped(argument, '&');
            if parts.len() < 2 || parts.iter().any(|part| part.trim().is_empty()) {
                return Err(PatternError::EmptyOperand);
            }
            let predicates = parts
                .iter()
                .map(|part| parse_predicate(part))
                .collect::<Result<Vec<_>, _>>()?;
            if operator.eq_ignore_ascii_case("all") {
                Ok(Predicate::All(predicates))
            } else {
                Ok(Predicate::Any(predicates))
            }
        }
        "not" => Ok(Predicate::Not(Box::new(parse_predicate(argument)?))),
        other => Err(PatternError::UnknownOperator(other.to_string())),
    }
}

pub fn validate_pattern(pattern: &str) -> Result<(), PatternError> {
    parse_predicate(pattern).map(|_| ())
}

pub fn evaluate_rule(rule: &DetectionRule, message: &str) -> Result<bool, PatternError> {
    if !rule.enabled {
        return Ok(false);
    }
    Ok(parse_predicate(&rule.pattern)?.matches(message))
}

pub fn evaluate_rules<'a>(rules: &'a [DetectionRule], message: &str) -> Vec<&'a DetectionRule> {
    rules
        .iter()
        .filter(|rule| evaluate_rule(rule, message).unwrap_or(false))
        .collect()
}

pub fn evaluate_with_reasons<'a>(rules: &'a [DetectionRule], message: &str) -> Vec<RuleMatch<'a>> {
    rules
        .iter()
        .filter_map(|rule| {
            evaluate_rule(rule, message)
                .ok()
                .filter(|matched| *matched)
                .map(|_| RuleMatch {
                    rule,
                    matched_pattern: rule.pattern.clone(),
                })
        })
        .collect()
}

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
        assert!(!evaluate_rule(&rule("not:contains:allowed", true), "login denied").unwrap());
    }

    #[test]
    fn malformed_or_disabled_rules_never_match() {
        assert_eq!(validate_pattern(""), Err(PatternError::Empty));
        assert!(validate_pattern("unknown:value").is_err());
        assert!(!evaluate_rule(&rule("contains:secret", false), "secret").unwrap());
        assert!(evaluate_rules(&[rule("bad:", true)], "bad").is_empty());
    }
}
