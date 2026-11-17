use regex::{Regex, RegexBuilder};

use crate::error::{AppError, Result};
use crate::theme::Severity;

pub const MAX_ID_LENGTH: usize = 60;
pub const MAX_NAME_LENGTH: usize = 80;
pub const MAX_DESCRIPTION_LENGTH: usize = 400;
pub const MAX_PATTERN_LENGTH: usize = 1000;
pub const MAX_KEYWORDS: usize = 16;
pub const MIN_KEYWORD_LENGTH: usize = 2;
pub const MAX_KEYWORD_LENGTH: usize = 40;
pub const MAX_ENTROPY_TENTHS: u32 = 80;
pub const MAX_CONFIDENCE: u8 = 100;
pub const COMPILED_SIZE_LIMIT: usize = 4 << 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleSpec {
    pub id: String,
    pub name: String,
    pub description: String,
    pub pattern: String,
    pub secret_group: usize,
    pub keywords: Vec<String>,
    pub entropy_tenths: u32,
    pub severity: Severity,
    pub confidence: u8,
    pub enabled: bool,
    pub builtin: bool,
}

impl Default for RuleSpec {
    fn default() -> Self {
        RuleSpec {
            id: String::new(),
            name: String::new(),
            description: String::new(),
            pattern: String::new(),
            secret_group: 0,
            keywords: Vec::new(),
            entropy_tenths: 0,
            severity: Severity::Medium,
            confidence: 60,
            enabled: true,
            builtin: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Rule {
    spec: RuleSpec,
    matcher: Regex,
}

fn has_control(text: &str) -> bool {
    text.chars().any(char::is_control)
}

fn pattern_problem(error: &regex::Error) -> String {
    let text = error.to_string();
    let reason = text
        .lines()
        .rev()
        .find_map(|line| line.trim().strip_prefix("error: "))
        .map_or_else(|| text.lines().next_back().unwrap_or("it cannot be compiled").trim().to_string(), str::to_string);
    let position = text
        .lines()
        .find(|line| line.contains('^') && line.chars().all(|letter| letter == ' ' || letter == '^'))
        .map(|line| line.chars().take_while(|letter| *letter == ' ').count().saturating_sub(4) + 1);
    match position {
        Some(position) => format!("the pattern is not valid at character {position}: {reason}"),
        None => format!("the pattern is not valid: {reason}"),
    }
}

pub fn compile_pattern(field: &str, pattern: &str) -> Result<Regex> {
    if pattern.is_empty() {
        return Err(AppError::invalid(field, "the pattern is empty", "Enter a regular expression"));
    }
    if pattern.chars().count() > MAX_PATTERN_LENGTH {
        return Err(AppError::invalid(
            field,
            format!("the pattern is longer than {MAX_PATTERN_LENGTH} characters"),
            "Shorten the pattern",
        ));
    }
    if has_control(pattern) {
        return Err(AppError::invalid(
            field,
            "the pattern contains a control character",
            "Write line breaks and tabs as \\n, \\r and \\t",
        ));
    }
    let matcher = RegexBuilder::new(pattern).size_limit(COMPILED_SIZE_LIMIT).build().map_err(|error| {
        AppError::invalid(
            field,
            pattern_problem(&error),
            "Correct the pattern; it uses the syntax of the Rust regex crate",
        )
    })?;
    if matcher.is_match("") {
        return Err(AppError::invalid(
            field,
            "the pattern matches empty text",
            "Require at least one character, for example with + instead of *",
        ));
    }
    Ok(matcher)
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ID_LENGTH
        && id.bytes().all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !id.starts_with('-')
        && !id.ends_with('-')
}

fn normalize_keywords(keywords: &[String]) -> Result<Vec<String>> {
    let mut cleaned: Vec<String> = Vec::new();
    for keyword in keywords {
        let word = keyword.trim().to_lowercase();
        if word.is_empty() {
            continue;
        }
        let length = word.chars().count();
        if !(MIN_KEYWORD_LENGTH..=MAX_KEYWORD_LENGTH).contains(&length) || has_control(&word) {
            return Err(AppError::invalid(
                "keywords",
                format!("the keyword \"{}\" is not usable", word.escape_default()),
                format!(
                    "Use keywords of {MIN_KEYWORD_LENGTH} to {MAX_KEYWORD_LENGTH} characters without control characters"
                ),
            ));
        }
        if !cleaned.contains(&word) {
            cleaned.push(word);
        }
    }
    if cleaned.len() > MAX_KEYWORDS {
        return Err(AppError::invalid(
            "keywords",
            format!("there are more than {MAX_KEYWORDS} keywords"),
            "Remove keywords; one keyword that every match contains is enough",
        ));
    }
    Ok(cleaned)
}

pub fn parse_keywords(text: &str) -> Vec<String> {
    text.split(',').map(str::trim).filter(|word| !word.is_empty()).map(str::to_string).collect()
}

impl Rule {
    pub fn compile(spec: &RuleSpec) -> Result<Rule> {
        let id = spec.id.trim().to_string();
        if !valid_id(&id) {
            return Err(AppError::invalid(
                "id",
                "the identifier is not usable",
                format!(
                    "Use 1 to {MAX_ID_LENGTH} lower-case letters, digits and hyphens, for example internal-api-token"
                ),
            ));
        }
        let name = spec.name.trim().to_string();
        if name.is_empty() || name.chars().count() > MAX_NAME_LENGTH || has_control(&name) {
            return Err(AppError::invalid(
                "name",
                "the name is empty, too long or contains a control character",
                format!("Enter a name of 1 to {MAX_NAME_LENGTH} characters"),
            ));
        }
        let description = spec.description.trim().to_string();
        if description.chars().count() > MAX_DESCRIPTION_LENGTH || has_control(&description) {
            return Err(AppError::invalid(
                "description",
                "the description is too long or contains a control character",
                format!("Enter at most {MAX_DESCRIPTION_LENGTH} characters on one line"),
            ));
        }
        let matcher = compile_pattern("pattern", &spec.pattern)?;
        if spec.secret_group >= matcher.captures_len() {
            return Err(AppError::invalid(
                "secret group",
                format!(
                    "the pattern has {} capture groups and group {} was asked for",
                    matcher.captures_len() - 1,
                    spec.secret_group
                ),
                "Use 0 for the whole match or the number of a group in parentheses",
            ));
        }
        if spec.entropy_tenths > MAX_ENTROPY_TENTHS {
            return Err(AppError::invalid(
                "entropy",
                "the entropy threshold is above 8.0 bits per character",
                "Enter a value from 0.0 (no check) to 8.0",
            ));
        }
        if spec.confidence == 0 || spec.confidence > MAX_CONFIDENCE {
            return Err(AppError::invalid(
                "confidence",
                "the starting confidence is outside 1 to 100",
                "Enter a number from 1 to 100",
            ));
        }
        let keywords = normalize_keywords(&spec.keywords)?;
        Ok(Rule { spec: RuleSpec { id, name, description, keywords, ..spec.clone() }, matcher })
    }

    pub fn spec(&self) -> &RuleSpec {
        &self.spec
    }

    pub fn matcher(&self) -> &Regex {
        &self.matcher
    }

    pub fn may_match(&self, lowered_content: &str) -> bool {
        self.spec.keywords.is_empty()
            || self.spec.keywords.iter().any(|keyword| lowered_content.contains(keyword.as_str()))
    }
}

struct Builtin {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    pattern: &'static str,
    secret_group: usize,
    keywords: &'static [&'static str],
    entropy_tenths: u32,
    severity: Severity,
    confidence: u8,
}

const BUILTIN: &[Builtin] = &[
    Builtin {
        id: "aws-access-key-id",
        name: "AWS access key ID",
        description: "Access key identifier of an AWS user or of temporary AWS credentials",
        pattern: r"\b((?:AKIA|ASIA)[A-Z2-7]{16})\b",
        secret_group: 1,
        keywords: &["akia", "asia"],
        entropy_tenths: 0,
        severity: Severity::High,
        confidence: 80,
    },
    Builtin {
        id: "aws-secret-access-key",
        name: "AWS secret access key",
        description: "Forty characters assigned to a name that contains aws and secret",
        pattern: r#"(?im)aws[a-z0-9_ .-]{0,24}secret[a-z0-9_ .-]{0,24}["']?\s*[:=]\s*["']?([a-z0-9/+]{40})(?:["'\s]|$)"#,
        secret_group: 1,
        keywords: &["aws"],
        entropy_tenths: 42,
        severity: Severity::Critical,
        confidence: 85,
    },
    Builtin {
        id: "github-token",
        name: "GitHub token",
        description: "Personal, OAuth, user-to-server, server-to-server or refresh token of GitHub",
        pattern: r"\b(gh[pousr]_[A-Za-z0-9]{36,255})\b",
        secret_group: 1,
        keywords: &["ghp_", "gho_", "ghu_", "ghs_", "ghr_"],
        entropy_tenths: 0,
        severity: Severity::Critical,
        confidence: 90,
    },
    Builtin {
        id: "github-fine-grained-token",
        name: "GitHub fine-grained token",
        description: "Fine-grained personal access token of GitHub",
        pattern: r"\b(github_pat_[A-Za-z0-9_]{82})\b",
        secret_group: 1,
        keywords: &["github_pat_"],
        entropy_tenths: 0,
        severity: Severity::Critical,
        confidence: 90,
    },
    Builtin {
        id: "gitlab-token",
        name: "GitLab personal access token",
        description: "Personal access token of GitLab",
        pattern: r"\b(glpat-[A-Za-z0-9_-]{20,64})",
        secret_group: 1,
        keywords: &["glpat-"],
        entropy_tenths: 0,
        severity: Severity::Critical,
        confidence: 85,
    },
    Builtin {
        id: "slack-token",
        name: "Slack token",
        description: "Bot, user, app or legacy token of Slack",
        pattern: r"\b(xox[abeoprs]-[A-Za-z0-9-]{10,250})",
        secret_group: 1,
        keywords: &["xoxa-", "xoxb-", "xoxe-", "xoxo-", "xoxp-", "xoxr-", "xoxs-"],
        entropy_tenths: 0,
        severity: Severity::High,
        confidence: 85,
    },
    Builtin {
        id: "slack-webhook-url",
        name: "Slack incoming webhook",
        description: "Address of a Slack incoming webhook, which accepts messages without further credentials",
        pattern: r"(https://hooks\.slack\.com/services/T[A-Za-z0-9_]{8,12}/B[A-Za-z0-9_]{8,12}/[A-Za-z0-9_]{24})",
        secret_group: 1,
        keywords: &["hooks.slack.com"],
        entropy_tenths: 0,
        severity: Severity::Medium,
        confidence: 85,
    },
    Builtin {
        id: "stripe-secret-key",
        name: "Stripe live secret key",
        description: "Secret or restricted key of Stripe in live mode",
        pattern: r"\b((?:sk|rk)_live_[A-Za-z0-9]{24,99})\b",
        secret_group: 1,
        keywords: &["sk_live_", "rk_live_"],
        entropy_tenths: 0,
        severity: Severity::Critical,
        confidence: 90,
    },
    Builtin {
        id: "google-api-key",
        name: "Google API key",
        description: "API key of Google Cloud or Firebase",
        pattern: r"\b(AIza[A-Za-z0-9_-]{35})",
        secret_group: 1,
        keywords: &["aiza"],
        entropy_tenths: 0,
        severity: Severity::Medium,
        confidence: 75,
    },
    Builtin {
        id: "npm-token",
        name: "npm access token",
        description: "Access token of the npm registry",
        pattern: r"\b(npm_[A-Za-z0-9]{36})\b",
        secret_group: 1,
        keywords: &["npm_"],
        entropy_tenths: 0,
        severity: Severity::High,
        confidence: 85,
    },
    Builtin {
        id: "pypi-token",
        name: "PyPI upload token",
        description: "API token of the Python Package Index",
        pattern: r"\b(pypi-AgEIcHlwaS5vcmc[A-Za-z0-9_-]{50,1000})",
        secret_group: 1,
        keywords: &["pypi-ageichlwas5vcmc"],
        entropy_tenths: 0,
        severity: Severity::High,
        confidence: 95,
    },
    Builtin {
        id: "sendgrid-api-key",
        name: "SendGrid API key",
        description: "API key of SendGrid",
        pattern: r"\b(SG\.[A-Za-z0-9_-]{22}\.[A-Za-z0-9_-]{43})",
        secret_group: 1,
        keywords: &["sg."],
        entropy_tenths: 0,
        severity: Severity::High,
        confidence: 90,
    },
    Builtin {
        id: "private-key",
        name: "Private key",
        description: "Unencrypted private key in PEM or OpenSSH form, also when the line breaks are written as \\n",
        pattern: r"-----BEGIN (?:[A-Z0-9]+ ){0,2}PRIVATE KEY(?: BLOCK)?-----(?:\\[nr]|\s)+((?:[A-Za-z0-9+/=]{16,76}(?:\\[nr]|\s)+){1,60})",
        secret_group: 1,
        keywords: &["private key"],
        entropy_tenths: 0,
        severity: Severity::Critical,
        confidence: 95,
    },
    Builtin {
        id: "json-web-token",
        name: "JSON Web Token",
        description: "Signed token with a JSON header and a JSON payload",
        pattern: r"\b(eyJ[A-Za-z0-9_-]{8,}\.eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{16,})",
        secret_group: 1,
        keywords: &["eyj"],
        entropy_tenths: 0,
        severity: Severity::Medium,
        confidence: 60,
    },
    Builtin {
        id: "password-in-url",
        name: "Password in a URL",
        description: "Password between the user name and the host of a URL",
        pattern: r#"\b[a-zA-Z][a-zA-Z0-9+.-]{1,15}://[^\s/:@"'<>]{1,64}:([^\s/:@"'<>]{3,120})@[A-Za-z0-9\[]"#,
        secret_group: 1,
        keywords: &["://"],
        entropy_tenths: 0,
        severity: Severity::High,
        confidence: 60,
    },
    Builtin {
        id: "credential-assignment",
        name: "Credential assigned in code or configuration",
        description: "Quoted value assigned to a name such as password, secret, token or api_key",
        pattern: r#"(?i)\b[a-z0-9_.-]{0,30}(?:password|passwd|pwd|secret|token|api[_-]?key|access[_-]?key|auth[_-]?key|private[_-]?key)["']?\s*(?::=|=>|=|:)\s*["']([^"'\s]{8,120})["']"#,
        secret_group: 1,
        keywords: &["password", "passwd", "pwd", "secret", "token", "key"],
        entropy_tenths: 30,
        severity: Severity::Medium,
        confidence: 50,
    },
];

pub fn builtin() -> Vec<RuleSpec> {
    BUILTIN
        .iter()
        .map(|rule| RuleSpec {
            id: rule.id.to_string(),
            name: rule.name.to_string(),
            description: rule.description.to_string(),
            pattern: rule.pattern.to_string(),
            secret_group: rule.secret_group,
            keywords: rule.keywords.iter().map(|keyword| (*keyword).to_string()).collect(),
            entropy_tenths: rule.entropy_tenths,
            severity: rule.severity,
            confidence: rule.confidence,
            enabled: true,
            builtin: true,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn custom(pattern: &str) -> RuleSpec {
        RuleSpec {
            id: "custom".to_string(),
            name: "Custom".to_string(),
            pattern: pattern.to_string(),
            ..RuleSpec::default()
        }
    }

    fn problem(spec: &RuleSpec) -> String {
        Rule::compile(spec).unwrap_err().to_string()
    }

    #[test]
    fn every_built_in_rule_compiles_and_is_described() {
        let rules = builtin();
        assert_eq!(rules.len(), 16);
        let mut seen = std::collections::BTreeSet::new();
        for spec in &rules {
            let rule = Rule::compile(spec).unwrap_or_else(|error| panic!("{}: {error}", spec.id));
            assert_eq!(rule.spec(), spec, "{} is changed by validation", spec.id);
            assert!(spec.builtin && spec.enabled);
            assert!(!spec.description.is_empty());
            assert!(!spec.keywords.is_empty(), "{} has no keyword", spec.id);
            assert!(seen.insert(spec.id.clone()), "{} is defined twice", spec.id);
        }
    }

    #[test]
    fn invalid_rules_name_the_field_and_the_fix() {
        let cases: Vec<(RuleSpec, &str)> = vec![
            (RuleSpec { id: "Bad Id".to_string(), ..custom("a+") }, "id: the identifier is not usable"),
            (RuleSpec { id: "-edge".to_string(), ..custom("a+") }, "id: "),
            (RuleSpec { id: "x".repeat(61), ..custom("a+") }, "id: "),
            (RuleSpec { name: " ".to_string(), ..custom("a+") }, "name: "),
            (RuleSpec { name: "bell\u{7}".to_string(), ..custom("a+") }, "name: "),
            (RuleSpec { description: "d".repeat(401), ..custom("a+") }, "description: "),
            (custom(""), "pattern: the pattern is empty"),
            (custom(&"a".repeat(1001)), "pattern: the pattern is longer than 1000 characters"),
            (custom("a\nb"), "pattern: the pattern contains a control character"),
            (custom("a*"), "pattern: the pattern matches empty text"),
            (custom("(abc"), "pattern: the pattern is not valid at character 1: unclosed group"),
            (custom("ab[c"), "pattern: the pattern is not valid at character 3: unclosed character class"),
            (
                RuleSpec { secret_group: 2, ..custom("(a)b") },
                "secret group: the pattern has 1 capture groups and group 2",
            ),
            (RuleSpec { entropy_tenths: 81, ..custom("a+") }, "entropy: "),
            (RuleSpec { confidence: 0, ..custom("a+") }, "confidence: "),
            (RuleSpec { confidence: 101, ..custom("a+") }, "confidence: "),
            (RuleSpec { keywords: vec!["x".to_string()], ..custom("a+") }, "keywords: the keyword \"x\" is not usable"),
            (
                RuleSpec { keywords: (0..17).map(|number| format!("word{number}")).collect(), ..custom("a+") },
                "keywords: there are more than 16 keywords",
            ),
        ];
        for (spec, expected) in cases {
            let message = problem(&spec);
            assert!(message.starts_with(expected), "{message:?} does not start with {expected:?}");
        }
    }

    #[test]
    fn a_pattern_that_would_need_too_much_memory_is_refused() {
        let message = problem(&custom(r"(?:\pL{500}){500}"));
        assert!(message.starts_with("pattern: the pattern is not valid"), "{message}");
    }

    #[test]
    fn validation_trims_text_and_normalizes_keywords() {
        let spec = RuleSpec {
            id: " custom ".to_string(),
            name: "  Custom rule ".to_string(),
            description: " What it finds ".to_string(),
            keywords: vec![" TOKEN ".to_string(), "token".to_string(), String::new(), "Key".to_string()],
            ..custom("tok_[a-z]{8}")
        };
        let rule = Rule::compile(&spec).unwrap();
        assert_eq!(rule.spec().id, "custom");
        assert_eq!(rule.spec().name, "Custom rule");
        assert_eq!(rule.spec().description, "What it finds");
        assert_eq!(rule.spec().keywords, vec!["token".to_string(), "key".to_string()]);
        assert!(rule.may_match("the token is here"));
        assert!(!rule.may_match("nothing relevant"));
        assert!(Rule::compile(&custom("a+")).unwrap().may_match("anything"));
        assert_eq!(parse_keywords(" a, b ,,c "), vec!["a".to_string(), "b".to_string(), "c".to_string()]);
    }

    proptest! {
        #[test]
        fn any_pattern_is_compiled_or_refused_without_a_panic(pattern in ".{0,60}") {
            if let Ok(rule) = Rule::compile(&custom(&pattern)) {
                prop_assert!(!rule.matcher().is_match(""));
            }
        }
    }
}
