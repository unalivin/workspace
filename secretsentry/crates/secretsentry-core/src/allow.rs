use regex::Regex;

use crate::error::{AppError, Result};
use crate::fingerprint;
use crate::rules;

pub const LINE_MARKER: &str = "secretsentry:allow";
pub const MAX_GLOB_LENGTH: usize = 300;
pub const MAX_NOTE_LENGTH: usize = 400;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Path,
    Value,
    Fingerprint,
}

impl Kind {
    pub const ALL: [Kind; 3] = [Kind::Path, Kind::Value, Kind::Fingerprint];

    pub fn parse(text: &str) -> Option<Kind> {
        let wanted = text.trim().to_lowercase();
        Kind::ALL.into_iter().find(|kind| kind.as_str() == wanted)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Path => "path",
            Kind::Value => "value",
            Kind::Fingerprint => "fingerprint",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Kind::Path => "Path pattern",
            Kind::Value => "Value pattern",
            Kind::Fingerprint => "Fingerprint",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntrySpec {
    pub id: i64,
    pub kind: Kind,
    pub pattern: String,
    pub rule_id: String,
    pub note: String,
    pub enabled: bool,
}

impl Default for EntrySpec {
    fn default() -> Self {
        EntrySpec {
            id: 0,
            kind: Kind::Path,
            pattern: String::new(),
            rule_id: String::new(),
            note: String::new(),
            enabled: true,
        }
    }
}

#[derive(Debug, Clone)]
enum Matcher {
    Path(Regex),
    Value(Regex),
    Fingerprint(String),
}

#[derive(Debug, Clone)]
struct Entry {
    id: i64,
    rule_id: String,
    matcher: Matcher,
}

#[derive(Debug, Clone, Default)]
pub struct Allowlist {
    entries: Vec<Entry>,
}

pub fn glob_to_pattern(glob: &str) -> Result<String> {
    let trimmed = glob.trim();
    if trimmed.is_empty() {
        return Err(AppError::invalid("pattern", "the path pattern is empty", "Enter a pattern such as tests/** or *.pem"));
    }
    if trimmed.chars().count() > MAX_GLOB_LENGTH {
        return Err(AppError::invalid(
            "pattern",
            format!("the path pattern is longer than {MAX_GLOB_LENGTH} characters"),
            "Shorten the pattern",
        ));
    }
    if trimmed.chars().any(char::is_control) || trimmed.contains('\\') {
        return Err(AppError::invalid(
            "pattern",
            "the path pattern contains a backslash or a control character",
            "Separate directories with / on every operating system",
        ));
    }
    let rooted = trimmed.starts_with('/') || trimmed.starts_with("./");
    let anchored = trimmed.trim_start_matches("./").trim_start_matches('/');
    let body = anchored.trim_end_matches('/');
    if body.is_empty() {
        return Err(AppError::invalid("pattern", "the path pattern names no file or directory", "Enter a pattern such as tests/**"));
    }
    let any_directory = !rooted && !body.contains('/') && !anchored.ends_with('/');
    let letters: Vec<char> = body.chars().collect();
    let mut pattern = String::from(if any_directory { "^(?:.*/)?" } else { "^" });
    let mut index = 0;
    while index < letters.len() {
        let letter = letters[index];
        if letter == '*' && letters.get(index + 1) == Some(&'*') {
            let mut next = index + 2;
            while letters.get(next) == Some(&'*') {
                next += 1;
            }
            if letters.get(next) == Some(&'/') && (index == 0 || letters[index - 1] == '/') {
                pattern.push_str("(?:.*/)?");
                next += 1;
            } else {
                pattern.push_str(".*");
            }
            index = next;
            continue;
        }
        match letter {
            '*' => pattern.push_str("[^/]*"),
            '?' => pattern.push_str("[^/]"),
            other => pattern.push_str(&regex::escape(&other.to_string())),
        }
        index += 1;
    }
    if anchored.ends_with('/') {
        pattern.push_str("/.*");
    }
    pattern.push('$');
    Ok(pattern)
}

fn compile_entry(spec: &EntrySpec) -> Result<Entry> {
    let matcher = match spec.kind {
        Kind::Path => {
            let pattern = glob_to_pattern(&spec.pattern)?;
            Matcher::Path(Regex::new(&pattern).map_err(|error| {
                AppError::invalid("pattern", format!("the path pattern cannot be used: {error}"), "Simplify the pattern")
            })?)
        }
        Kind::Value => Matcher::Value(rules::compile_pattern("pattern", spec.pattern.trim())?),
        Kind::Fingerprint => {
            let text = spec.pattern.trim().to_lowercase();
            if !fingerprint::is_fingerprint(&text) {
                return Err(AppError::invalid(
                    "pattern",
                    "the fingerprint is not 64 hexadecimal digits",
                    "Copy the fingerprint from the detail of a finding",
                ));
            }
            Matcher::Fingerprint(text)
        }
    };
    if spec.note.chars().count() > MAX_NOTE_LENGTH || spec.note.chars().any(char::is_control) {
        return Err(AppError::invalid(
            "note",
            "the note is too long or contains a control character",
            format!("Enter at most {MAX_NOTE_LENGTH} characters on one line"),
        ));
    }
    Ok(Entry { id: spec.id, rule_id: spec.rule_id.trim().to_string(), matcher })
}

pub fn validate(spec: &EntrySpec) -> Result<EntrySpec> {
    compile_entry(spec)?;
    let pattern = match spec.kind {
        Kind::Fingerprint => spec.pattern.trim().to_lowercase(),
        Kind::Path | Kind::Value => spec.pattern.trim().to_string(),
    };
    Ok(EntrySpec { pattern, rule_id: spec.rule_id.trim().to_string(), note: spec.note.trim().to_string(), ..spec.clone() })
}

pub fn normalize_path(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches("./").trim_start_matches('/').to_string()
}

impl Allowlist {
    pub fn compile(specs: &[EntrySpec]) -> Result<Allowlist> {
        let mut entries = Vec::new();
        for spec in specs.iter().filter(|spec| spec.enabled) {
            entries.push(compile_entry(spec)?);
        }
        Ok(Allowlist { entries })
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn matching(&self, rule_id: &str, path: &str, value: &str, fingerprint: &str) -> Option<i64> {
        let path = normalize_path(path);
        self.entries
            .iter()
            .find(|entry| {
                (entry.rule_id.is_empty() || entry.rule_id == rule_id)
                    && match &entry.matcher {
                        Matcher::Path(matcher) => matcher.is_match(&path),
                        Matcher::Value(matcher) => matcher.is_match(value),
                        Matcher::Fingerprint(expected) => expected == fingerprint,
                    }
            })
            .map(|entry| entry.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn entry(id: i64, kind: Kind, pattern: &str) -> EntrySpec {
        EntrySpec { id, kind, pattern: pattern.to_string(), ..EntrySpec::default() }
    }

    fn path_matches(glob: &str, path: &str) -> bool {
        Allowlist::compile(&[entry(1, Kind::Path, glob)]).unwrap().matching("rule", path, "value", "").is_some()
    }

    #[test]
    fn path_patterns_follow_the_documented_rules() {
        let cases = [
            ("*.pem", "key.pem", true),
            ("*.pem", "deep/down/key.pem", true),
            ("*.pem", "key.pem.txt", false),
            ("key.pem", "a/key.pem", true),
            ("key.pem", "a/monkey.pem", false),
            ("tests/**", "tests/a/b.txt", true),
            ("tests/**", "tests/b.txt", true),
            ("tests/**", "src/tests/b.txt", false),
            ("tests/", "tests/a/b.txt", true),
            ("tests/", "tests", false),
            ("testdata/", "pkg/testdata/key", false),
            ("**/testdata/**", "pkg/testdata/key", true),
            ("**/testdata/**", "testdata/key", true),
            ("**/testdata/**", "pkg/mytestdata/key", false),
            ("src/*.rs", "src/main.rs", true),
            ("src/*.rs", "src/deep/main.rs", false),
            ("src/**/*.rs", "src/main.rs", true),
            ("src/**/*.rs", "src/a/b/main.rs", true),
            ("/config.yml", "config.yml", true),
            ("/config.yml", "a/config.yml", false),
            ("./config.yml", "config.yml", true),
            ("./config.yml", "a/config.yml", false),
            ("file?.txt", "file1.txt", true),
            ("file?.txt", "file12.txt", false),
            ("file?.txt", "file/.txt", false),
            ("a+b(1).txt", "a+b(1).txt", true),
            ("a+b(1).txt", "aab1.txt", false),
            ("docs/**/key*", "docs/x/y/key-old.pem", true),
            ("a**b", "a/x/y/b", true),
        ];
        for (glob, path, expected) in cases {
            assert_eq!(path_matches(glob, path), expected, "{glob} on {path}");
        }
        assert!(path_matches("tests/**", "tests\\windows\\style.txt"));
        assert!(path_matches("tests/**", "./tests/a.txt"));
    }

    #[test]
    fn unusable_entries_name_the_problem_and_the_fix() {
        let cases = [
            (entry(1, Kind::Path, " "), "pattern: the path pattern is empty"),
            (entry(1, Kind::Path, "/"), "pattern: the path pattern names no file or directory"),
            (entry(1, Kind::Path, "a\\b"), "pattern: the path pattern contains a backslash"),
            (entry(1, Kind::Path, &"a".repeat(301)), "pattern: the path pattern is longer than 300 characters"),
            (entry(1, Kind::Value, "(open"), "pattern: the pattern is not valid at character 1: unclosed group"),
            (entry(1, Kind::Value, "a*"), "pattern: the pattern matches empty text"),
            (entry(1, Kind::Fingerprint, "abc"), "pattern: the fingerprint is not 64 hexadecimal digits"),
            (EntrySpec { note: "n".repeat(401), ..entry(1, Kind::Path, "a") }, "note: the note is too long"),
        ];
        for (spec, expected) in cases {
            let message = validate(&spec).unwrap_err().to_string();
            assert!(message.starts_with(expected), "{message:?} does not start with {expected:?}");
            assert!(Allowlist::compile(&[spec]).is_err());
        }
    }

    #[test]
    fn entries_match_by_value_fingerprint_and_rule() {
        let print = "ab".repeat(32);
        let list = Allowlist::compile(&[
            entry(1, Kind::Value, "^AKIA[A-Z0-9]{12}DEMO$"),
            EntrySpec { rule_id: "only-this".to_string(), ..entry(2, Kind::Path, "vendor/**") },
            entry(3, Kind::Fingerprint, &print.to_uppercase()),
            EntrySpec { enabled: false, ..entry(4, Kind::Path, "**") },
        ])
        .unwrap();
        assert_eq!(list.len(), 3);
        assert!(!list.is_empty());
        assert_eq!(list.matching("any", "src/a", &["AKIA", "ABCDEFGH1234", "DEMO"].concat(), ""), Some(1));
        assert_eq!(list.matching("only-this", "vendor/lib/x", "value", ""), Some(2));
        assert_eq!(list.matching("another", "vendor/lib/x", "value", ""), None);
        assert_eq!(list.matching("any", "src/a", "value", &print), Some(3));
        assert_eq!(list.matching("any", "src/a", "value", &"cd".repeat(32)), None);
        assert!(Allowlist::default().is_empty());
    }

    #[test]
    fn validation_trims_and_keeps_the_identity() {
        let cleaned = validate(&EntrySpec {
            id: 9,
            kind: Kind::Fingerprint,
            pattern: format!("  {}  ", "AB".repeat(32)),
            rule_id: " rule ".to_string(),
            note: " known demo key ".to_string(),
            enabled: false,
        })
        .unwrap();
        assert_eq!(cleaned.id, 9);
        assert_eq!(cleaned.pattern, "ab".repeat(32));
        assert_eq!(cleaned.rule_id, "rule");
        assert_eq!(cleaned.note, "known demo key");
        assert!(!cleaned.enabled);
        for kind in Kind::ALL {
            assert_eq!(Kind::parse(kind.as_str()), Some(kind));
            assert!(!kind.label().is_empty());
        }
        assert_eq!(Kind::parse("regex"), None);
    }

    proptest! {
        #[test]
        fn any_path_pattern_is_translated_or_refused_without_a_panic(glob in ".{0,60}", path in ".{0,60}") {
            if let Ok(pattern) = glob_to_pattern(&glob) {
                let matcher = Regex::new(&pattern);
                prop_assert!(matcher.is_ok(), "{} gives the unusable pattern {}", glob, pattern);
                let _ = matcher.unwrap().is_match(&path);
            }
        }

        #[test]
        fn a_literal_path_matches_itself_and_nothing_longer(path in "[a-z0-9_.+()-]{1,12}(/[a-z0-9_.+()-]{1,12}){1,3}") {
            prop_assert!(path_matches(&path, &path));
            let longer = format!("{}x", path);
            prop_assert!(!path_matches(&path, &longer));
            let below = format!("{}/x", path);
            prop_assert!(!path_matches(&path, &below));
        }
    }
}
