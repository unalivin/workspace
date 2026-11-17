use serde::{Deserialize, Serialize};

use crate::entropy;
use crate::rules::RuleSpec;

pub const HIGH_FROM: u8 = 75;
pub const MEDIUM_FROM: u8 = 45;
pub const ENTROPY_EFFECT: i32 = 5;
pub const CREDENTIAL_WORD_EFFECT: i32 = 10;
pub const PLACEHOLDER_EFFECT: i32 = -40;
pub const TEST_PATH_EFFECT: i32 = -20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    Low,
    Medium,
    High,
}

impl Level {
    pub const ALL: [Level; 3] = [Level::Low, Level::Medium, Level::High];

    pub fn of(score: u8) -> Level {
        if score >= HIGH_FROM {
            Level::High
        } else if score >= MEDIUM_FROM {
            Level::Medium
        } else {
            Level::Low
        }
    }

    pub fn parse(text: &str) -> Option<Level> {
        let wanted = text.trim().to_lowercase();
        Level::ALL.into_iter().find(|level| level.as_str() == wanted)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Level::Low => "low",
            Level::Medium => "medium",
            Level::High => "high",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Level::Low => "Low",
            Level::Medium => "Medium",
            Level::High => "High",
        }
    }

    pub fn lowest_score(self) -> u8 {
        match self {
            Level::Low => 0,
            Level::Medium => MEDIUM_FROM,
            Level::High => HIGH_FROM,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reason {
    pub effect: i32,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assessment {
    pub score: u8,
    pub reasons: Vec<Reason>,
}

pub struct Evidence<'a> {
    pub rule: &'a RuleSpec,
    pub value: &'a str,
    pub surroundings: &'a str,
    pub path: &'a str,
    pub entropy_tenths: u32,
}

const PLACEHOLDER_PARTS: &[&str] = &[
    "example",
    "sample",
    "dummy",
    "changeme",
    "change_me",
    "change-me",
    "placeholder",
    "redacted",
    "your_",
    "your-",
    "yourkey",
    "yourtoken",
    "yourpassword",
    "xxxx",
    "0000000",
    "todo",
    "fake",
    "insert",
    "replace",
    "${",
    "{{",
    "%(",
    "...",
    "<",
    ">",
];

const DUMMY_VALUES: &[&str] =
    &["password", "passwd", "password1", "password123", "secret", "pass", "admin", "root", "guest", "test", "user"];

const CREDENTIAL_WORDS: &[&str] =
    &["secret", "token", "password", "passwd", "credential", "api_key", "apikey", "api-key", "private", "auth"];

const TEST_DIRECTORIES: &[&str] = &[
    "test",
    "tests",
    "testing",
    "testdata",
    "test-data",
    "__tests__",
    "fixture",
    "fixtures",
    "spec",
    "specs",
    "example",
    "examples",
    "sample",
    "samples",
    "mock",
    "mocks",
    "doc",
    "docs",
    "e2e",
];

const TEST_FILE_PARTS: &[&str] = &["_test.", ".test.", "-test.", "_spec.", ".spec.", "-spec."];
const TEST_FILE_ENDINGS: &[&str] = &[".md", ".rst", ".adoc", ".example", ".sample", ".template", ".dist"];

fn is_sequence(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 6
        && (bytes.windows(2).all(|pair| pair[1] == pair[0].wrapping_add(1))
            || bytes.windows(2).all(|pair| pair[0] == pair[1].wrapping_add(1)))
}

pub fn looks_like_placeholder(value: &str) -> bool {
    let lowered = value.to_lowercase();
    let distinct = lowered.chars().collect::<std::collections::BTreeSet<char>>().len();
    lowered.starts_with('$')
        || distinct <= 3
        || is_sequence(&lowered)
        || DUMMY_VALUES.contains(&lowered.as_str())
        || PLACEHOLDER_PARTS.iter().any(|part| lowered.contains(part))
}

pub fn looks_like_test_path(path: &str) -> bool {
    let lowered = path.to_lowercase().replace('\\', "/");
    let mut segments: Vec<&str> = lowered.split('/').filter(|segment| !segment.is_empty()).collect();
    let Some(file) = segments.pop() else {
        return false;
    };
    segments.iter().any(|segment| TEST_DIRECTORIES.contains(segment))
        || file.starts_with("test_")
        || TEST_FILE_PARTS.iter().any(|part| file.contains(part))
        || TEST_FILE_ENDINGS.iter().any(|ending| file.ends_with(ending))
}

pub fn credential_word(surroundings: &str) -> Option<&'static str> {
    let lowered = surroundings.to_lowercase();
    CREDENTIAL_WORDS.iter().copied().find(|word| lowered.contains(word))
}

pub fn assess(evidence: &Evidence<'_>) -> Assessment {
    let rule = evidence.rule;
    let mut reasons = vec![Reason {
        effect: i32::from(rule.confidence),
        text: format!("The text matches the rule \"{}\"", rule.name),
    }];
    if rule.entropy_tenths > 0 {
        reasons.push(Reason {
            effect: ENTROPY_EFFECT,
            text: format!(
                "The value has {} bits of entropy per character, the rule asks for {} or more",
                entropy::show(evidence.entropy_tenths),
                entropy::show(rule.entropy_tenths)
            ),
        });
    }
    if let Some(word) = credential_word(evidence.surroundings) {
        reasons.push(Reason {
            effect: CREDENTIAL_WORD_EFFECT,
            text: format!("The line names a credential (\"{word}\")"),
        });
    }
    if looks_like_placeholder(evidence.value) {
        reasons.push(Reason {
            effect: PLACEHOLDER_EFFECT,
            text: "The value looks like a placeholder, an example or a reference to a variable".to_string(),
        });
    }
    if looks_like_test_path(evidence.path) {
        reasons.push(Reason {
            effect: TEST_PATH_EFFECT,
            text: "The path looks like test, example or documentation content".to_string(),
        });
    }
    let total: i32 = reasons.iter().map(|reason| reason.effect).sum();
    Assessment { score: u8::try_from(total.clamp(1, 100)).unwrap_or(1), reasons }
}

pub fn reasons_to_json(reasons: &[Reason]) -> String {
    serde_json::to_string(reasons).unwrap_or_else(|_| "[]".to_string())
}

pub fn reasons_from_json(text: &str) -> Vec<Reason> {
    serde_json::from_str(text).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn rule(confidence: u8, entropy_tenths: u32) -> RuleSpec {
        RuleSpec {
            id: "sample-rule".to_string(),
            name: "Sample rule".to_string(),
            pattern: "x+".to_string(),
            confidence,
            entropy_tenths,
            ..RuleSpec::default()
        }
    }

    fn score(rule: &RuleSpec, value: &str, surroundings: &str, path: &str) -> Assessment {
        assess(&Evidence { rule, value, surroundings, path, entropy_tenths: entropy::tenths(value) })
    }

    #[test]
    fn levels_follow_the_documented_borders() {
        assert_eq!(Level::of(0), Level::Low);
        assert_eq!(Level::of(44), Level::Low);
        assert_eq!(Level::of(45), Level::Medium);
        assert_eq!(Level::of(74), Level::Medium);
        assert_eq!(Level::of(75), Level::High);
        assert_eq!(Level::of(100), Level::High);
        for level in Level::ALL {
            assert_eq!(Level::parse(level.as_str()), Some(level));
            assert_eq!(Level::parse(&format!(" {} ", level.label())), Some(level));
            assert_eq!(Level::of(level.lowest_score()), level);
        }
        assert_eq!(Level::parse("certain"), None);
        assert!(Level::Low < Level::Medium && Level::Medium < Level::High);
    }

    #[test]
    fn each_reason_changes_the_score_by_its_documented_effect() {
        let plain = rule(60, 0);
        let random = "q7Zp2LmX9vKc4RtB";
        let base = score(&plain, random, "value = ", "src/main.rs");
        assert_eq!(base.score, 60);
        assert_eq!(base.reasons.len(), 1);
        assert_eq!(base.reasons[0], Reason { effect: 60, text: "The text matches the rule \"Sample rule\"".to_string() });

        let named = score(&plain, random, "API_TOKEN = ", "src/main.rs");
        assert_eq!(named.score, 70);
        assert_eq!(named.reasons[1].text, "The line names a credential (\"token\")");

        let placeholder = score(&plain, "your_token_here_please", "value = ", "src/main.rs");
        assert_eq!(placeholder.score, 20);

        let tested = score(&plain, random, "value = ", "tests/fixtures/config.yml");
        assert_eq!(tested.score, 40);

        let measured = score(&rule(60, 30), random, "value = ", "src/main.rs");
        assert_eq!(measured.score, 65);
        assert_eq!(
            measured.reasons[1].text,
            "The value has 4.0 bits of entropy per character, the rule asks for 3.0 or more"
        );

        let everything = score(&rule(100, 30), random, "password", "src/main.rs");
        assert_eq!(everything.score, 100);
        let nothing = score(&rule(10, 0), "changeme", "", "docs/readme.md");
        assert_eq!(nothing.score, 1);
        assert_eq!(nothing.reasons.iter().map(|reason| reason.effect).collect::<Vec<_>>(), vec![10, -40, -20]);
    }

    #[test]
    fn placeholders_are_recognized_and_real_looking_values_are_not() {
        for value in [
            "AKIAIOSFODNN7EXAMPLE",
            "changeme",
            "<your-token>",
            "${DB_PASSWORD}",
            "$DB_PASSWORD",
            "{{ secret }}",
            "xxxxxxxxxxxxxxxx",
            "aaaaaaaaaaaa",
            "abababababab",
            "123456789",
            "abcdefgh",
            "987654321",
            "password",
            "Password123",
            "REDACTED",
            "insert-key-here",
        ] {
            assert!(looks_like_placeholder(value), "{value} should be a placeholder");
        }
        for value in ["q7Zp2LmX9vKc4RtB", "S3cr3t!Value9", "correct-horse-battery", "hunter2hunter2"] {
            assert!(!looks_like_placeholder(value), "{value} should not be a placeholder");
        }
    }

    #[test]
    fn test_and_documentation_paths_are_recognized() {
        for path in [
            "tests/config.yml",
            "src/__tests__/login.js",
            "pkg/auth/login_test.go",
            "web/login.spec.ts",
            "test_settings.py",
            "app/testdata/key.pem",
            "docs/setup.txt",
            "README.md",
            "config/settings.yml.example",
            "Examples\\Demo\\app.config",
        ] {
            assert!(looks_like_test_path(path), "{path} should be a test path");
        }
        for path in ["src/main.rs", "config/production.yml", "contest/entry.py", "latest/build.sh", "", "docs"] {
            assert!(!looks_like_test_path(path), "{path} should not be a test path");
        }
    }

    #[test]
    fn reasons_survive_storage_as_json() {
        let reasons = vec![
            Reason { effect: 60, text: "The text matches the rule \"A \\ b\"".to_string() },
            Reason { effect: -40, text: "Second".to_string() },
        ];
        assert_eq!(reasons_from_json(&reasons_to_json(&reasons)), reasons);
        assert_eq!(reasons_from_json("not json"), Vec::new());
        assert_eq!(reasons_to_json(&[]), "[]");
    }

    proptest! {
        #[test]
        fn the_score_is_always_between_one_and_one_hundred(
            confidence in 1u8..=100,
            threshold in 0u32..=80,
            value in ".{0,60}",
            surroundings in ".{0,60}",
            path in ".{0,60}",
        ) {
            let assessment = score(&rule(confidence, threshold), &value, &surroundings, &path);
            prop_assert!((1..=100).contains(&assessment.score));
            let total: i32 = assessment.reasons.iter().map(|reason| reason.effect).sum();
            prop_assert_eq!(i32::from(assessment.score), total.clamp(1, 100));
        }
    }
}
