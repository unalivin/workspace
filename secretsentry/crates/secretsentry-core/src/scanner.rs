use std::ops::Range;

use crate::allow::{self, Allowlist, EntrySpec};
use crate::confidence::{self, Evidence, Reason};
use crate::entropy;
use crate::error::{AppError, Result};
use crate::fingerprint;
use crate::redact;
use crate::rules::{Rule, RuleSpec};

pub const MAX_HITS_PER_FILE: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Allowed {
    No,
    Marker,
    Entry(i64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub rule_id: String,
    pub path: String,
    pub line: usize,
    pub column: usize,
    pub preview: String,
    pub length: usize,
    pub entropy_tenths: u32,
    pub fingerprint: String,
    pub value_digest: String,
    pub context: String,
    pub score: u8,
    pub reasons: Vec<Reason>,
    pub allowed: Allowed,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FileReport {
    pub candidates: Vec<Candidate>,
    pub truncated: bool,
}

#[derive(Debug, Clone)]
pub struct Detector {
    rules: Vec<Rule>,
    allowlist: Allowlist,
    key: Vec<u8>,
}

struct Hit {
    rule: usize,
    whole: Range<usize>,
    secret: Range<usize>,
    entropy_tenths: u32,
    kept: bool,
}

struct Lines {
    starts: Vec<usize>,
    end: usize,
}

impl Lines {
    fn new(content: &str) -> Lines {
        let mut starts = vec![0];
        starts.extend(content.bytes().enumerate().filter(|(_, byte)| *byte == b'\n').map(|(index, _)| index + 1));
        Lines { starts, end: content.len() }
    }

    fn locate(&self, content: &str, offset: usize) -> (usize, Range<usize>) {
        let index = self.starts.partition_point(|start| *start <= offset).saturating_sub(1);
        let start = self.starts[index];
        let mut end = self.starts.get(index + 1).map_or(self.end, |next| next - 1);
        if end > start && content.as_bytes()[end - 1] == b'\r' {
            end -= 1;
        }
        (index + 1, start..end.max(start))
    }
}

fn overlaps(first: &Range<usize>, second: &Range<usize>) -> bool {
    first.start < second.end && second.start < first.end
}

impl Detector {
    pub fn new(rules: &[RuleSpec], allowlist: &[EntrySpec], key: &[u8]) -> Result<Detector> {
        let mut compiled = Vec::new();
        for spec in rules.iter().filter(|spec| spec.enabled) {
            compiled.push(Rule::compile(spec).map_err(|error| match error {
                AppError::Validation { field, problem, fix } => {
                    AppError::Validation { field: format!("rule {}, {field}", spec.id), problem, fix }
                }
                other => other,
            })?);
        }
        Ok(Detector { rules: compiled, allowlist: Allowlist::compile(allowlist)?, key: key.to_vec() })
    }

    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    fn hits(&self, content: &str) -> (Vec<Hit>, bool) {
        let lowered = content.to_ascii_lowercase();
        let mut hits: Vec<Hit> = Vec::new();
        let mut truncated = false;
        for (index, rule) in self.rules.iter().enumerate() {
            if !rule.may_match(&lowered) {
                continue;
            }
            for captures in rule.matcher().captures_iter(content) {
                let (Some(whole), Some(secret)) = (captures.get(0), captures.get(rule.spec().secret_group)) else {
                    continue;
                };
                if secret.is_empty() {
                    continue;
                }
                if hits.len() >= MAX_HITS_PER_FILE {
                    truncated = true;
                    break;
                }
                let entropy_tenths = entropy::tenths(secret.as_str());
                hits.push(Hit {
                    rule: index,
                    whole: whole.range(),
                    secret: secret.range(),
                    entropy_tenths,
                    kept: entropy_tenths >= rule.spec().entropy_tenths,
                });
            }
        }
        let mut order: Vec<usize> = (0..hits.len()).filter(|index| hits[*index].kept).collect();
        order.sort_by_key(|index| {
            let hit = &hits[*index];
            (std::cmp::Reverse(self.rules[hit.rule].spec().confidence), hit.rule, hit.secret.start)
        });
        let mut accepted: Vec<Range<usize>> = Vec::new();
        for index in order {
            if accepted.iter().any(|range| overlaps(range, &hits[index].secret)) {
                hits[index].kept = false;
            } else {
                accepted.push(hits[index].secret.clone());
            }
        }
        hits.sort_by_key(|hit| (hit.secret.start, hit.secret.end, hit.rule));
        (hits, truncated)
    }

    fn context(content: &str, line: &Range<usize>, hits: &[Hit], focus: &Range<usize>) -> String {
        let mut shown = String::new();
        let mut focus_at = 0;
        let mut cursor = line.start;
        for hit in hits {
            let start = hit.secret.start.max(cursor);
            let end = hit.secret.end.min(line.end);
            if start >= end {
                continue;
            }
            shown.push_str(&redact::mask_random_words(&content[cursor..start]));
            if hit.secret == *focus {
                focus_at = shown.chars().count();
            }
            if start == hit.secret.start {
                shown.push_str(&redact::preview(&content[hit.secret.clone()]));
            } else {
                shown.push_str(redact::MASK);
            }
            cursor = end;
        }
        if cursor < line.end {
            shown.push_str(&redact::mask_random_words(&content[cursor..line.end]));
        }
        redact::window(&redact::printable(&shown), focus_at, redact::MAX_CONTEXT_LENGTH)
    }

    fn surroundings(content: &str, line: &Range<usize>, secret: &Range<usize>) -> String {
        let before_end = secret.start.clamp(line.start, line.end);
        let after_start = secret.end.clamp(before_end, line.end);
        format!("{} {}", &content[line.start..before_end], &content[after_start..line.end])
    }

    pub fn scan_text(&self, path: &str, content: &str) -> FileReport {
        let path = allow::normalize_path(path);
        let (hits, truncated) = self.hits(content);
        let lines = Lines::new(content);
        let mut candidates = Vec::new();
        for hit in hits.iter().filter(|hit| hit.kept) {
            let rule = self.rules[hit.rule].spec();
            let value = &content[hit.secret.clone()];
            let (number, line) = lines.locate(content, hit.whole.start);
            let column = content[line.start..hit.whole.start.clamp(line.start, line.end)].chars().count() + 1;
            let print = fingerprint::finding(&self.key, &rule.id, &path, value);
            let assessment = confidence::assess(&Evidence {
                rule,
                value,
                surroundings: &Detector::surroundings(content, &line, &hit.secret),
                path: &path,
                entropy_tenths: hit.entropy_tenths,
            });
            let allowed = if content[line.clone()].contains(allow::LINE_MARKER) {
                Allowed::Marker
            } else {
                self.allowlist.matching(&rule.id, &path, value, &print).map_or(Allowed::No, Allowed::Entry)
            };
            candidates.push(Candidate {
                rule_id: rule.id.clone(),
                path: path.clone(),
                line: number,
                column,
                preview: redact::preview(value),
                length: redact::length(value),
                entropy_tenths: hit.entropy_tenths,
                fingerprint: print,
                value_digest: fingerprint::value(&self.key, value),
                context: Detector::context(content, &line, &hits, &hit.secret),
                score: assessment.score,
                reasons: assessment.reasons,
                allowed,
            });
        }
        candidates.sort_by(|first, second| {
            (first.line, first.column, &first.rule_id).cmp(&(second.line, second.column, &second.rule_id))
        });
        FileReport { candidates, truncated }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::LazyLock;

    use super::*;
    use crate::allow::Kind;
    use crate::rules;
    use proptest::prelude::*;

    const KEY: [u8; 32] = [3u8; 32];
    const PATH: &str = "src/config.txt";

    static BUILTIN: LazyLock<Detector> = LazyLock::new(|| Detector::new(&rules::builtin(), &[], &KEY).unwrap());

    fn fake(parts: &[&str]) -> String {
        parts.concat()
    }

    fn letters(length: usize) -> String {
        "q7Zp2LmX9vKc4RtB8nWd3FgH6jS1uYeOaIxNhTbVcMzQrPl5".chars().cycle().take(length).collect()
    }

    fn sample(rule: &str) -> String {
        match rule {
            "aws-access-key-id" => fake(&["AK", "IA", "Q4WZ7PLM2K6XTR3N"]),
            "aws-secret-access-key" => fake(&["wJa1rXUtnFEMI/K7MDENG", "/bPxRfiCYzQ4pL9vT2s"]),
            "github-token" => fake(&["gh", "p_", &letters(36)]),
            "github-fine-grained-token" => fake(&["github", "_pat_", &letters(82)]),
            "gitlab-token" => fake(&["gl", "pat-", &letters(20)]),
            "slack-token" => fake(&["xo", "xb-", "123456789012-", &letters(24)]),
            "slack-webhook-url" => {
                fake(&["https://hooks.", "slack.com/services/", "T12345678/B12345678/", &letters(24)])
            }
            "stripe-secret-key" => fake(&["sk", "_live_", &letters(24)]),
            "google-api-key" => fake(&["AI", "za", &letters(35)]),
            "npm-token" => fake(&["np", "m_", &letters(36)]),
            "pypi-token" => fake(&["pypi-", "AgEIcHlwaS5vcmc", &letters(60)]),
            "sendgrid-api-key" => fake(&["SG", ".", &letters(22), ".", &letters(43)]),
            "json-web-token" => fake(&["eyJ", &letters(20), ".eyJ", &letters(30), ".", &letters(43)]),
            "password-in-url" => fake(&["S3cr3t", "Pw9"]),
            "credential-assignment" => fake(&["S3cr3t", "!Value9"]),
            "private-key" => format!("{}\n{}\n", letters(64), letters(64)),
            other => panic!("no sample for {other}"),
        }
    }

    fn private_key_block(body: &str) -> String {
        format!("{}{}\n{body}{}{}\n", "-----BEGIN RSA PRIV", "ATE KEY-----", "-----END RSA PRIV", "ATE KEY-----")
    }

    fn embed(rule: &str, value: &str) -> String {
        match rule {
            "aws-secret-access-key" => format!("AWS_SECRET_ACCESS_KEY = \"{value}\"\n"),
            "password-in-url" => format!("url: postgres://app:{value}@10.0.0.5:5432/main\n"),
            "credential-assignment" => format!("DB_PASSWORD = \"{value}\"\n"),
            "private-key" => private_key_block(value),
            _ => format!("value: {value}\n"),
        }
    }

    fn scan(content: &str) -> FileReport {
        BUILTIN.scan_text(PATH, content)
    }

    #[test]
    fn every_built_in_rule_reports_its_sample_once_with_the_exact_value() {
        for spec in rules::builtin() {
            let value = sample(&spec.id);
            let report = scan(&embed(&spec.id, &value));
            assert_eq!(report.candidates.len(), 1, "{}: {:?}", spec.id, report.candidates);
            let found = &report.candidates[0];
            assert_eq!(found.rule_id, spec.id);
            assert_eq!(found.fingerprint, fingerprint::finding(&KEY, &spec.id, PATH, &value), "{}", spec.id);
            assert_eq!(found.value_digest, fingerprint::value(&KEY, &value));
            assert_eq!(found.length, value.chars().count());
            assert_eq!(found.path, PATH);
            assert_eq!(found.line, 1);
            assert_eq!(found.allowed, Allowed::No);
            assert!(!report.truncated);
        }
    }

    #[test]
    fn values_never_appear_in_a_report() {
        for spec in rules::builtin() {
            let value = sample(&spec.id);
            let shown = format!("{:?}", scan(&embed(&spec.id, &value)));
            for part in value.lines() {
                assert!(!shown.contains(part), "{} shows its value", spec.id);
            }
        }
    }

    #[test]
    fn near_misses_report_nothing() {
        let texts = [
            format!("value: {}\n", fake(&["AK", "IA", "Q4WZ7PLM2K6"])),
            format!("value: {}\n", sample("aws-access-key-id").to_lowercase()),
            format!("value: X{}\n", sample("aws-access-key-id")),
            "aws_secret_access_key=da39a3ee5e6b4b0d3255bfef95601890afd80709\n".to_string(),
            format!("value: {}\n", fake(&["gh", "p_", &letters(35)])),
            format!("value: {}\n", fake(&["sk", "_test_", &letters(24)])),
            format!("value: {}\n", fake(&["np", "m_", &letters(20)])),
            format!("{}\n{}\n{}\n", "-----BEGIN PUBLIC KEY-----", letters(64), "-----END PUBLIC KEY-----"),
            "value: eyJhbGci.short.x\n".to_string(),
            "url: http://localhost:8080/path\n".to_string(),
            "password = getenv(\"DB_PASSWORD\")\n".to_string(),
            "password = \"aabbaabbaabb\"\n".to_string(),
            "tokenizer = \"wordpiece-base\"\n".to_string(),
            "secret_name: \"prod/db/main\"\n".to_string(),
            "password: \"short\"\n".to_string(),
            String::new(),
            "\n\n\n".to_string(),
        ];
        for text in texts {
            let report = scan(&text);
            assert!(report.candidates.is_empty(), "{text:?} reports {:?}", report.candidates);
        }
    }

    #[test]
    fn the_more_specific_rule_wins_when_two_rules_match_one_value() {
        let token = sample("github-token");
        let report = scan(&format!("API_TOKEN = \"{token}\"\n"));
        assert_eq!(report.candidates.len(), 1);
        assert_eq!(report.candidates[0].rule_id, "github-token");
        assert_eq!(report.candidates[0].score, 100);
        assert_eq!(report.candidates[0].context, "API_TOKEN = \"ghp_********\"");
    }

    #[test]
    fn every_value_on_a_line_is_masked_in_every_context() {
        let github = sample("github-token");
        let stripe = sample("stripe-secret-key");
        let report = scan(&format!("first {github} second {stripe} checksum {}\n", letters(40)));
        assert_eq!(report.candidates.len(), 2);
        for candidate in &report.candidates {
            assert_eq!(candidate.context, "first ghp_******** second sk_l******** checksum ********");
        }
        assert_eq!(report.candidates[0].rule_id, "github-token");
        assert_eq!(report.candidates[0].column, 7);
        assert_eq!(report.candidates[1].rule_id, "stripe-secret-key");
        assert_eq!(report.candidates[0].preview, "ghp_********");
        assert_eq!(report.candidates[1].preview, "sk_l********");
    }

    #[test]
    fn a_value_dropped_by_the_entropy_check_is_still_masked_in_contexts() {
        let token = sample("github-token");
        let report = scan(&format!("password = \"aabbaabbaabb\"; other = {token}\n"));
        assert_eq!(report.candidates.len(), 1);
        assert_eq!(report.candidates[0].context, "password = \"********\"; other = ghp_********");
    }

    #[test]
    fn lines_and_columns_count_characters_and_ignore_carriage_returns() {
        let key = sample("aws-access-key-id");
        let report = scan(&format!("line one\r\n  héllo value: {key}\r\nlast\n"));
        assert_eq!(report.candidates.len(), 1);
        let found = &report.candidates[0];
        assert_eq!((found.line, found.column), (2, 16));
        assert_eq!(found.context, "héllo value: AKIA********");
        let last = scan(&format!("a\nb\nvalue: {key}"));
        assert_eq!((last.candidates[0].line, last.candidates[0].column), (3, 8));
    }

    #[test]
    fn a_private_key_is_reported_at_its_first_line_without_its_body() {
        let body = sample("private-key");
        let report = scan(&format!("notes\n{}", private_key_block(&body)));
        assert_eq!(report.candidates.len(), 1);
        let found = &report.candidates[0];
        assert_eq!((found.line, found.column), (2, 1));
        assert_eq!(found.context, fake(&["-----BEGIN RSA PRIV", "ATE KEY-----"]));
        assert_eq!(found.preview, "q7Zp********");
        assert_eq!(found.score, 100);

        let escaped = body.replace('\n', "\\n");
        let inline = format!("\"private_key\": \"{}\\n{escaped}{}\\n\"\n", "-----BEGIN PRIV", "ATE KEY-----");
        let report = scan(&inline);
        assert_eq!(report.candidates.len(), 1, "{:?}", report.candidates);
        assert_eq!(report.candidates[0].rule_id, "private-key");
        assert!(!report.candidates[0].context.contains(&letters(64)));
    }

    #[test]
    fn long_lines_are_cut_around_the_value() {
        let token = sample("github-token");
        let report = scan(&format!("{} {token} {}\n", "word ".repeat(200), "tail ".repeat(200)));
        let context = &report.candidates[0].context;
        assert!(context.contains("ghp_********"));
        assert!(context.chars().count() <= redact::MAX_CONTEXT_LENGTH + 2);
        assert!(context.starts_with('…') && context.ends_with('…'));
    }

    #[test]
    fn control_characters_never_reach_a_context() {
        let token = sample("github-token");
        let report = scan(&format!("\u{1b}[31mred\u{7}\t{token}\u{1b}[0m\n"));
        assert_eq!(report.candidates[0].context, "[31mred  ghp_******** [0m");
    }

    #[test]
    fn the_number_of_hits_in_one_file_is_limited() {
        let key = sample("aws-access-key-id");
        let content = format!("k: {key}\n").repeat(MAX_HITS_PER_FILE + 100);
        let report = scan(&content);
        assert!(report.truncated);
        assert_eq!(report.candidates.len(), MAX_HITS_PER_FILE);
        assert_eq!(report.candidates[MAX_HITS_PER_FILE - 1].line, MAX_HITS_PER_FILE);
    }

    #[test]
    fn allowlists_and_the_line_marker_mark_candidates_without_hiding_them() {
        let key = sample("aws-access-key-id");
        let print = fingerprint::finding(&KEY, "aws-access-key-id", "other/app.env", &key);
        let entries = [
            EntrySpec { id: 11, kind: Kind::Path, pattern: "fixtures/**".to_string(), ..EntrySpec::default() },
            EntrySpec { id: 12, kind: Kind::Fingerprint, pattern: print, ..EntrySpec::default() },
            EntrySpec {
                id: 13,
                kind: Kind::Value,
                pattern: "^gh[a-z]_".to_string(),
                rule_id: "github-token".to_string(),
                ..EntrySpec::default()
            },
        ];
        let detector = Detector::new(&rules::builtin(), &entries, &KEY).unwrap();
        let content = format!("value: {key}\n");
        assert_eq!(detector.scan_text("fixtures/app.env", &content).candidates[0].allowed, Allowed::Entry(11));
        assert_eq!(detector.scan_text("other/app.env", &content).candidates[0].allowed, Allowed::Entry(12));
        assert_eq!(detector.scan_text("src/app.env", &content).candidates[0].allowed, Allowed::No);
        let token = format!("value: {}\n", sample("github-token"));
        assert_eq!(detector.scan_text("src/app.env", &token).candidates[0].allowed, Allowed::Entry(13));
        let marked = format!("value: {key} # {}\n", allow::LINE_MARKER);
        assert_eq!(detector.scan_text("src/app.env", &marked).candidates[0].allowed, Allowed::Marker);
    }

    #[test]
    fn confidence_follows_the_value_and_the_path() {
        let key = sample("aws-access-key-id");
        let content = format!("value: {key}\n");
        assert_eq!(BUILTIN.scan_text("src/app.env", &content).candidates[0].score, 80);
        assert_eq!(BUILTIN.scan_text("tests/app.env", &content).candidates[0].score, 60);
        let documented = format!("value: {}\n", fake(&["AK", "IA", "IOSFODNN7", "EXAMPLE"]));
        let report = BUILTIN.scan_text("docs/guide.md", &documented);
        assert_eq!(report.candidates[0].score, 20);
        assert_eq!(report.candidates[0].reasons.len(), 3);
        let assigned = scan(&embed("credential-assignment", &sample("credential-assignment")));
        assert_eq!(assigned.candidates[0].score, 65);
        assert_eq!(assigned.candidates[0].entropy_tenths, 35);
    }

    #[test]
    fn only_enabled_rules_run_and_a_broken_rule_is_named() {
        let mut specs = rules::builtin();
        for spec in &mut specs {
            spec.enabled = spec.id == "npm-token";
        }
        let detector = Detector::new(&specs, &[], &KEY).unwrap();
        assert_eq!(detector.rule_count(), 1);
        assert!(detector.scan_text(PATH, &embed("github-token", &sample("github-token"))).candidates.is_empty());
        assert_eq!(detector.scan_text(PATH, &embed("npm-token", &sample("npm-token"))).candidates.len(), 1);

        let broken = RuleSpec {
            id: "broken".to_string(),
            name: "Broken".to_string(),
            pattern: "(".to_string(),
            ..RuleSpec::default()
        };
        let message = Detector::new(&[broken.clone()], &[], &KEY).unwrap_err().to_string();
        assert!(message.starts_with("rule broken, pattern: the pattern is not valid"), "{message}");
        assert!(Detector::new(&[RuleSpec { enabled: false, ..broken }], &[], &KEY).is_ok());
    }

    #[test]
    fn a_custom_rule_uses_its_keyword_and_the_whole_match() {
        let custom = RuleSpec {
            id: "internal-token".to_string(),
            name: "Internal token".to_string(),
            pattern: "tok_[a-z0-9]{12}".to_string(),
            keywords: vec!["tok_".to_string()],
            confidence: 70,
            ..RuleSpec::default()
        };
        let detector = Detector::new(&[custom], &[], &KEY).unwrap();
        let value = fake(&["tok", "_", "a1b2c3d4e5f6"]);
        let report = detector.scan_text("a\\b.txt", &format!("x {value} y {value}\n"));
        assert_eq!(report.candidates.len(), 2);
        assert_eq!(report.candidates[0].path, "a/b.txt");
        assert_eq!(report.candidates[0].fingerprint, report.candidates[1].fingerprint);
        assert_eq!(report.candidates[0].fingerprint, fingerprint::finding(&KEY, "internal-token", "a/b.txt", &value));
        assert_eq!((report.candidates[0].column, report.candidates[1].column), (3, 22));
        assert_eq!(report.candidates[0].context, "x tok_******** y tok_********");
    }

    proptest! {
        #[test]
        fn any_text_is_scanned_without_a_panic_and_within_the_limits(content in "\\PC{0,300}") {
            let report = scan(&content);
            for candidate in &report.candidates {
                prop_assert!(candidate.line >= 1 && candidate.column >= 1);
                prop_assert!(candidate.context.chars().count() <= redact::MAX_CONTEXT_LENGTH + 2);
                prop_assert!((1..=100).contains(&candidate.score));
            }
        }

        #[test]
        fn a_planted_token_is_found_and_never_shown(before in "[ -~\n]{0,80}", after in "[ -~\n]{0,80}") {
            let token = sample("github-token");
            let report = scan(&format!("{before} {token} {after}"));
            prop_assert!(report.candidates.iter().any(|candidate| candidate.rule_id == "github-token"));
            prop_assert!(!format!("{report:?}").contains(&token));
        }

        #[test]
        fn code_like_lines_are_scanned_without_showing_long_random_words(
            name in "[a-z_]{1,12}",
            value in "[A-Za-z0-9+/]{24,60}",
            quote in "[\"']",
        ) {
            let report = scan(&format!("{name}_secret = {quote}{value}{quote}\n"));
            let shown = format!("{report:?}");
            if value.bytes().any(|byte| byte.is_ascii_digit()) && entropy::tenths(&value) >= redact::RANDOM_WORD_ENTROPY_TENTHS {
                prop_assert!(!shown.contains(&value));
            }
        }
    }
}
