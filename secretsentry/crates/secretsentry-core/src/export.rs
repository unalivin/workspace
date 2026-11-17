use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;

use crate::clock::Timestamp;
use crate::confidence::Reason;
use crate::error::{AppError, Result};
use crate::files;
use crate::findings::{Finding, Findings, Occurrence, Query, Status};
use crate::meta;
use crate::scanner::Allowed;
use crate::theme::Severity;

pub const JSON_FORMAT: &str = "secretsentry-findings";
pub const JSON_VERSION: u32 = 1;
pub const SARIF_VERSION: &str = "2.1.0";
pub const SARIF_SCHEMA: &str = "https://json.schemastore.org/sarif-2.1.0.json";
pub const FINGERPRINT_NAME: &str = "secretSentryFinding/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Format {
    Json,
    Sarif,
}

impl Format {
    pub const ALL: [Format; 2] = [Format::Json, Format::Sarif];

    pub fn parse(text: &str) -> Result<Format> {
        match text.trim().to_lowercase().as_str() {
            "json" => Ok(Format::Json),
            "sarif" => Ok(Format::Sarif),
            other => Err(AppError::Usage(format!("unknown format \"{other}\": use json or sarif"))),
        }
    }

    pub fn for_file(name: &str, explicit: &str) -> Result<Format> {
        if !explicit.trim().is_empty() {
            return Format::parse(explicit);
        }
        let lowered = name.to_lowercase();
        if lowered.ends_with(".sarif") || lowered.ends_with(".sarif.json") {
            Ok(Format::Sarif)
        } else if lowered.ends_with(".json") {
            Ok(Format::Json)
        } else {
            Err(AppError::Usage(format!(
                "the format of \"{name}\" is not clear from its name: use a .json or .sarif file name, or pass --format json or --format sarif"
            )))
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Format::Json => "json",
            Format::Sarif => "sarif",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Format::Json => "JSON",
            Format::Sarif => "SARIF 2.1.0",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Format::Json => "json",
            Format::Sarif => "sarif",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub finding: Finding,
    pub occurrences: Vec<Occurrence>,
}

pub fn collect(findings: &Findings<'_>, query: &Query) -> Result<Vec<Item>> {
    let mut items = Vec::new();
    for finding in findings.list(query)? {
        let occurrences = findings.occurrences(finding.id)?;
        items.push(Item { finding, occurrences });
    }
    Ok(items)
}

#[derive(Serialize)]
struct Tool<'a> {
    name: &'a str,
    version: &'a str,
}

#[derive(Serialize)]
struct JsonOccurrence<'a> {
    line: i64,
    column: i64,
    context: &'a str,
    commit: &'a str,
    author: &'a str,
    authored_at: Option<String>,
    subject: &'a str,
}

#[derive(Serialize)]
struct JsonFinding<'a> {
    id: i64,
    fingerprint: &'a str,
    rule_id: &'a str,
    rule_name: &'a str,
    severity: &'a str,
    confidence: &'a str,
    score: u8,
    reasons: &'a [Reason],
    root: &'a str,
    path: &'a str,
    preview: &'a str,
    length: i64,
    state: &'a str,
    status: &'a str,
    note: &'a str,
    baseline: bool,
    allowlisted: bool,
    in_tree: bool,
    in_history: bool,
    demo: bool,
    first_seen_at: String,
    last_seen_at: String,
    occurrences: Vec<JsonOccurrence<'a>>,
}

#[derive(Serialize)]
struct JsonDocument<'a> {
    format: &'a str,
    version: u32,
    tool: Tool<'a>,
    generated_at: String,
    total: usize,
    findings: Vec<JsonFinding<'a>>,
}

fn json(items: &[Item], generated_at: Timestamp) -> Result<String> {
    let findings = items
        .iter()
        .map(|item| {
            let finding = &item.finding;
            JsonFinding {
                id: finding.id,
                fingerprint: &finding.fingerprint,
                rule_id: &finding.rule_id,
                rule_name: &finding.rule_name,
                severity: finding.severity.as_str(),
                confidence: finding.level().as_str(),
                score: finding.score,
                reasons: &finding.reasons,
                root: &finding.root,
                path: &finding.path,
                preview: &finding.preview,
                length: finding.length,
                state: finding.state_label(),
                status: finding.status.as_str(),
                note: &finding.note,
                baseline: finding.baseline,
                allowlisted: finding.allowed != Allowed::No,
                in_tree: finding.in_tree,
                in_history: finding.in_history,
                demo: finding.demo,
                first_seen_at: finding.first_seen_at.to_rfc3339(),
                last_seen_at: finding.last_seen_at.to_rfc3339(),
                occurrences: item
                    .occurrences
                    .iter()
                    .map(|place| JsonOccurrence {
                        line: place.line,
                        column: place.column,
                        context: &place.context,
                        commit: &place.commit_id,
                        author: &place.author,
                        authored_at: place.authored_at.map(Timestamp::to_rfc3339),
                        subject: &place.subject,
                    })
                    .collect(),
            }
        })
        .collect();
    let document = JsonDocument {
        format: JSON_FORMAT,
        version: JSON_VERSION,
        tool: Tool { name: meta::APP_NAME, version: meta::VERSION },
        generated_at: generated_at.to_rfc3339(),
        total: items.len(),
        findings,
    };
    Ok(serde_json::to_string_pretty(&document)? + "\n")
}

#[derive(Serialize)]
struct Text<'a> {
    text: &'a str,
}

#[derive(Serialize)]
struct OwnedText {
    text: String,
}

#[derive(Serialize)]
struct Configuration {
    level: &'static str,
}

#[derive(Serialize)]
struct RuleProperties {
    #[serde(rename = "security-severity")]
    security_severity: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifRule<'a> {
    id: &'a str,
    short_description: Text<'a>,
    default_configuration: Configuration,
    properties: RuleProperties,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Driver<'a> {
    name: &'a str,
    version: &'a str,
    information_uri: String,
    rules: Vec<SarifRule<'a>>,
}

#[derive(Serialize)]
struct SarifTool<'a> {
    driver: Driver<'a>,
}

#[derive(Serialize)]
struct ArtifactLocation {
    uri: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Region {
    start_line: i64,
    start_column: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PhysicalLocation {
    artifact_location: ArtifactLocation,
    region: Region,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Location {
    physical_location: PhysicalLocation,
}

#[derive(Serialize)]
struct Suppression {
    kind: &'static str,
    status: &'static str,
    justification: String,
}

fn blank(text: &&str) -> bool {
    text.is_empty()
}

#[derive(Serialize)]
struct ResultProperties<'a> {
    confidence: &'a str,
    score: u8,
    state: &'a str,
    status: &'a str,
    preview: &'a str,
    length: i64,
    root: &'a str,
    #[serde(skip_serializing_if = "blank")]
    commit: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifResult<'a> {
    rule_id: &'a str,
    rule_index: usize,
    level: &'static str,
    message: OwnedText,
    locations: Vec<Location>,
    partial_fingerprints: BTreeMap<&'static str, &'a str>,
    baseline_state: &'static str,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    suppressions: Vec<Suppression>,
    properties: ResultProperties<'a>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Run<'a> {
    tool: SarifTool<'a>,
    column_kind: &'static str,
    results: Vec<SarifResult<'a>>,
}

#[derive(Serialize)]
struct SarifDocument<'a> {
    #[serde(rename = "$schema")]
    schema: &'a str,
    version: &'a str,
    runs: Vec<Run<'a>>,
}

pub fn sarif_level(severity: Severity) -> &'static str {
    match severity {
        Severity::Critical | Severity::High => "error",
        Severity::Medium => "warning",
        Severity::Low | Severity::Info => "note",
    }
}

fn security_severity(severity: Severity) -> &'static str {
    match severity {
        Severity::Critical => "9.5",
        Severity::High => "8.0",
        Severity::Medium => "5.5",
        Severity::Low => "3.0",
        Severity::Info => "1.0",
    }
}

pub fn uri_path(path: &str) -> String {
    let mut uri = String::with_capacity(path.len());
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/') {
            uri.push(char::from(byte));
        } else {
            uri.push_str(&format!("%{byte:02X}"));
        }
    }
    uri
}

fn suppression(finding: &Finding) -> Option<Suppression> {
    let reason = match (finding.allowed, finding.status) {
        (Allowed::Marker, _) => "Allowlisted by a marker on the line".to_string(),
        (Allowed::Entry(id), _) => format!("Allowlisted by entry {id}"),
        (Allowed::No, Status::FalsePositive | Status::AcceptedRisk) => finding.status.label().to_string(),
        (Allowed::No, _) => return None,
    };
    let justification = if finding.note.is_empty() || finding.allowed != Allowed::No {
        reason
    } else {
        format!("{reason}: {}", finding.note)
    };
    Some(Suppression { kind: "external", status: "accepted", justification })
}

fn sarif(items: &[Item]) -> Result<String> {
    let mut rules: Vec<SarifRule<'_>> = Vec::new();
    let mut indexes: BTreeMap<&str, usize> = BTreeMap::new();
    let mut results = Vec::new();
    for item in items {
        let finding = &item.finding;
        let next = rules.len();
        let rule_index = *indexes.entry(finding.rule_id.as_str()).or_insert(next);
        if rule_index == next {
            rules.push(SarifRule {
                id: &finding.rule_id,
                short_description: Text { text: &finding.rule_name },
                default_configuration: Configuration { level: sarif_level(finding.severity) },
                properties: RuleProperties { security_severity: security_severity(finding.severity) },
            });
        }
        let in_files: Vec<&Occurrence> = item.occurrences.iter().filter(|place| place.commit_id.is_empty()).collect();
        let shown: Vec<&Occurrence> =
            if in_files.is_empty() { item.occurrences.iter().take(1).collect() } else { in_files };
        let commit = shown.first().map_or("", |place| place.commit_id.as_str());
        let mut locations: Vec<Location> = shown
            .iter()
            .map(|place| Location {
                physical_location: PhysicalLocation {
                    artifact_location: ArtifactLocation { uri: uri_path(&finding.path) },
                    region: Region { start_line: place.line.max(1), start_column: place.column.max(1) },
                },
            })
            .collect();
        if locations.is_empty() {
            locations.push(Location {
                physical_location: PhysicalLocation {
                    artifact_location: ArtifactLocation { uri: uri_path(&finding.path) },
                    region: Region { start_line: 1, start_column: 1 },
                },
            });
        }
        let level = finding.level();
        results.push(SarifResult {
            rule_id: &finding.rule_id,
            rule_index,
            level: sarif_level(finding.severity),
            message: OwnedText {
                text: format!(
                    "{} found: {} ({} characters), {} confidence ({} of 100)",
                    finding.rule_name,
                    finding.preview,
                    finding.length,
                    level.as_str(),
                    finding.score
                ),
            },
            locations,
            partial_fingerprints: BTreeMap::from([(FINGERPRINT_NAME, finding.fingerprint.as_str())]),
            baseline_state: if finding.baseline { "unchanged" } else { "new" },
            suppressions: suppression(finding).into_iter().collect(),
            properties: ResultProperties {
                confidence: level.as_str(),
                score: finding.score,
                state: finding.state_label(),
                status: finding.status.as_str(),
                preview: &finding.preview,
                length: finding.length,
                root: &finding.root,
                commit,
            },
        });
    }
    let document = SarifDocument {
        schema: SARIF_SCHEMA,
        version: SARIF_VERSION,
        runs: vec![Run {
            tool: SarifTool {
                driver: Driver {
                    name: meta::APP_NAME,
                    version: meta::VERSION,
                    information_uri: meta::source_url(),
                    rules,
                },
            },
            column_kind: "unicodeCodePoints",
            results,
        }],
    };
    Ok(serde_json::to_string_pretty(&document)? + "\n")
}

pub fn render(format: Format, items: &[Item], generated_at: Timestamp) -> Result<String> {
    match format {
        Format::Json => json(items, generated_at),
        Format::Sarif => sarif(items),
    }
}

pub fn write(path: &Path, format: Format, items: &[Item], generated_at: Timestamp, overwrite: bool) -> Result<usize> {
    let text = render(format, items, generated_at)?;
    files::write_private(path, text.as_bytes(), overwrite)?;
    Ok(items.len())
}

pub fn default_file_name(format: Format, generated_at: Timestamp) -> String {
    let (year, month, day, hour, minute, _) = generated_at.civil(0);
    format!("secretsentry-findings-{year:04}{month:02}{day:02}-{hour:02}{minute:02}.{}", format.extension())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::AtomicBool;

    use serde_json::Value;

    use super::*;
    use crate::allow::{EntrySpec, Kind};
    use crate::allowlists::Allowlists;
    use crate::clock::{Clock, Zone};
    use crate::git::HistoryOptions;
    use crate::scans::Scans;
    use crate::store::Store;
    use crate::testkit::{
        GitFixture, TempDir, fixed_time, sample_cloud_key, sample_hosting_token, sample_package_token,
    };
    use crate::tree;

    struct Fixture {
        directory: TempDir,
        store: Store,
    }

    impl Fixture {
        fn findings(&self) -> Findings<'_> {
            Findings::new(self.store.conn(), Clock::fixed(fixed_time()))
        }

        fn items(&self) -> Vec<Item> {
            let query = Query { sort: "path".to_string(), descending: false, ..Query::default() };
            collect(&self.findings(), &query).unwrap()
        }
    }

    fn fixture(label: &str) -> Fixture {
        let directory = TempDir::new(label);
        let store = Store::open_ready(&directory.path().join("app.db")).unwrap();
        let root = directory.path().join("my project");
        fs::create_dir_all(root.join("tests")).unwrap();
        let git = GitFixture::create(&root);
        let old = git.blob(&format!("NPM_TOKEN={}\n", sample_package_token()));
        let tree_one = git.tree(&[("100644", "old.env", &old)]);
        let tree_two = git.tree(&[]);
        let first = git.commit(&tree_one, &[], 1_700_000_000, "add the environment");
        let second = git.commit(&tree_two, &[&first], 1_700_000_100, "remove it");
        git.set_reference("refs/heads/main", &second);
        fs::write(
            root.join("a b#c.py"),
            format!("KEY = \"{}\"\nKEY2 = \"{}\"\n", sample_cloud_key(), sample_cloud_key()),
        )
        .unwrap();
        fs::write(root.join("deploy.sh"), format!("export TOKEN={}\n", sample_hosting_token())).unwrap();
        fs::write(root.join("tests").join("data.txt"), format!("token {}\n", sample_hosting_token())).unwrap();
        let clock = Clock::fixed(fixed_time());
        Allowlists::new(store.conn(), clock.clone())
            .create(&EntrySpec { kind: Kind::Path, pattern: "tests/**".to_string(), ..EntrySpec::default() })
            .unwrap();
        let scans = Scans::new(store.conn(), clock, Zone::Utc);
        scans.scan_tree(&root, &tree::Options::default(), &AtomicBool::new(false)).unwrap();
        scans.scan_history(&root, &HistoryOptions::default(), &AtomicBool::new(false)).unwrap();
        Fixture { directory, store }
    }

    fn assert_redacted(text: &str) {
        for secret in [sample_cloud_key(), sample_hosting_token(), sample_package_token()] {
            assert!(!text.contains(&secret), "the output holds a secret value");
            assert!(!text.contains(&secret[6..]), "the output holds the tail of a secret value");
        }
    }

    #[test]
    fn formats_are_named_and_chosen_from_the_file_name() {
        assert_eq!(Format::parse(" SARIF ").unwrap(), Format::Sarif);
        assert_eq!(Format::parse("json").unwrap(), Format::Json);
        assert_eq!(Format::parse("csv").unwrap_err().to_string(), "unknown format \"csv\": use json or sarif");
        assert_eq!(Format::for_file("out.SARIF", "").unwrap(), Format::Sarif);
        assert_eq!(Format::for_file("out.sarif.json", "").unwrap(), Format::Sarif);
        assert_eq!(Format::for_file("out.json", "").unwrap(), Format::Json);
        assert_eq!(Format::for_file("out.txt", "sarif").unwrap(), Format::Sarif);
        assert!(Format::for_file("out.txt", "").unwrap_err().to_string().contains("is not clear from its name"));
        assert_eq!(Format::ALL.map(Format::as_str), ["json", "sarif"]);
        assert_eq!(Format::ALL.map(Format::label), ["JSON", "SARIF 2.1.0"]);
        assert_eq!(default_file_name(Format::Sarif, fixed_time()), "secretsentry-findings-20260302-0930.sarif");
        assert!(default_file_name(Format::Json, fixed_time()).ends_with(".json"));
        assert_eq!(uri_path("a b#c/é.py"), "a%20b%23c/%C3%A9.py");
        assert_eq!(
            [Severity::Critical, Severity::Medium, Severity::Info].map(sarif_level),
            ["error", "warning", "note"]
        );
    }

    #[test]
    fn json_lists_every_finding_with_redacted_values() {
        let fixture = fixture("export-json");
        let items = fixture.items();
        assert_eq!(
            items.iter().map(|item| item.finding.path.as_str()).collect::<Vec<_>>(),
            vec!["a b#c.py", "deploy.sh", "old.env", "tests/data.txt"]
        );
        let text = render(Format::Json, &items, fixed_time()).unwrap();
        assert_redacted(&text);
        assert!(text.ends_with("}\n"));
        let document: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(document["format"], "secretsentry-findings");
        assert_eq!(document["version"], 1);
        assert_eq!(document["tool"]["name"], "SecretSentry");
        assert_eq!(document["tool"]["version"], meta::VERSION);
        assert_eq!(document["generated_at"], fixed_time().to_rfc3339());
        assert_eq!(document["total"], 4);
        let findings = document["findings"].as_array().unwrap();
        assert_eq!(findings.len(), 4);
        let key = &findings[0];
        assert_eq!(key["rule_id"], "aws-access-key-id");
        assert_eq!(key["preview"], "AKIA********");
        assert_eq!(key["length"], 20);
        assert_eq!((key["state"].as_str(), key["status"].as_str()), (Some("New"), Some("open")));
        assert_eq!(key["occurrences"].as_array().unwrap().len(), 2);
        assert_eq!(key["occurrences"][1]["line"], 2);
        assert_eq!(key["occurrences"][0]["commit"], "");
        assert_eq!(key["occurrences"][0]["authored_at"], Value::Null);
        assert_eq!(key["fingerprint"].as_str().unwrap().len(), 64);
        assert!(key["reasons"].as_array().is_some_and(|reasons| !reasons.is_empty()));
        let old = &findings[2];
        assert_eq!((old["in_tree"].as_bool(), old["in_history"].as_bool()), (Some(false), Some(true)));
        assert_eq!(old["occurrences"][0]["commit"].as_str().unwrap().len(), 40);
        assert_eq!(old["occurrences"][0]["subject"], "add the environment");
        assert_eq!(old["occurrences"][0]["authored_at"], "2023-11-14T22:13:20Z");
        assert_eq!(findings[3]["allowlisted"], true);
        assert_eq!(findings[3]["state"], "Allowlisted");
        assert_eq!(render(Format::Json, &items, fixed_time()).unwrap(), text);
    }

    #[test]
    fn sarif_has_the_same_findings_with_rules_locations_and_states() {
        let fixture = fixture("export-sarif");
        let findings = fixture.findings();
        let listed = fixture.items();
        findings
            .decide(listed[1].finding.id, listed[1].finding.revision, Status::AcceptedRisk, "rotated on Monday")
            .unwrap();
        findings.accept_baseline("").unwrap();
        let items = fixture.items();
        let text = render(Format::Sarif, &items, fixed_time()).unwrap();
        assert_redacted(&text);
        let document: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(document["$schema"], SARIF_SCHEMA);
        assert_eq!(document["version"], "2.1.0");
        let runs = document["runs"].as_array().unwrap();
        assert_eq!(runs.len(), 1);
        let driver = &runs[0]["tool"]["driver"];
        assert_eq!(driver["name"], "SecretSentry");
        assert_eq!(driver["informationUri"], meta::source_url());
        let rules = driver["rules"].as_array().unwrap();
        assert_eq!(
            rules.iter().map(|rule| rule["id"].as_str().unwrap()).collect::<Vec<_>>(),
            vec!["aws-access-key-id", "github-token", "npm-token"]
        );
        assert_eq!(rules[1]["shortDescription"]["text"], "GitHub token");
        assert_eq!(rules[1]["defaultConfiguration"]["level"], "error");
        assert_eq!(rules[1]["properties"]["security-severity"], "9.5");
        let results = runs[0]["results"].as_array().unwrap();
        assert_eq!(results.len(), items.len());
        for (result, item) in results.iter().zip(&items) {
            let index = result["ruleIndex"].as_u64().unwrap() as usize;
            assert_eq!(rules[index]["id"], result["ruleId"]);
            assert_eq!(result["ruleId"], item.finding.rule_id.as_str());
            assert_eq!(result["partialFingerprints"][FINGERPRINT_NAME], item.finding.fingerprint.as_str());
            assert_eq!(result["properties"]["preview"], item.finding.preview.as_str());
        }
        let key = &results[0];
        assert_eq!(key["level"], "error");
        let expected = format!(
            "AWS access key ID found: AKIA******** (20 characters), {} confidence ({} of 100)",
            items[0].finding.level().as_str(),
            items[0].finding.score
        );
        assert_eq!(key["message"]["text"], expected);
        assert_eq!(key["locations"].as_array().unwrap().len(), 2);
        assert_eq!(key["locations"][0]["physicalLocation"]["artifactLocation"]["uri"], "a%20b%23c.py");
        assert_eq!(key["locations"][1]["physicalLocation"]["region"]["startLine"], 2);
        assert_eq!(key["locations"][1]["physicalLocation"]["region"]["startColumn"], 9);
        assert_eq!(key["baselineState"], "unchanged");
        assert_eq!(key.get("suppressions"), None);
        assert_eq!(key["properties"].get("commit"), None);
        assert_eq!(results[1]["suppressions"][0]["justification"], "Accepted risk: rotated on Monday");
        assert_eq!(results[1]["suppressions"][0]["kind"], "external");
        assert_eq!(results[2]["properties"]["commit"].as_str().unwrap().len(), 40);
        assert_eq!(results[2]["locations"].as_array().unwrap().len(), 1);
        assert!(results[3]["suppressions"][0]["justification"].as_str().unwrap().starts_with("Allowlisted by entry "));
        assert_eq!(runs[0]["columnKind"], "unicodeCodePoints");

        let empty: Value = serde_json::from_str(&render(Format::Sarif, &[], fixed_time()).unwrap()).unwrap();
        assert_eq!(empty["runs"][0]["results"].as_array().unwrap().len(), 0);
        assert_eq!(empty["runs"][0]["tool"]["driver"]["rules"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn a_file_is_written_privately_and_not_replaced_without_consent() {
        let fixture = fixture("export-file");
        let items = fixture.items();
        let path = fixture.directory.path().join("out").join("findings.sarif");
        assert_eq!(write(&path, Format::Sarif, &items, fixed_time(), false).unwrap(), 4);
        let first = fs::read_to_string(&path).unwrap();
        assert_redacted(&first);
        let kept = write(&path, Format::Json, &items, fixed_time(), false).unwrap_err().to_string();
        assert!(kept.contains("already exists and was not changed"), "{kept}");
        assert_eq!(fs::read_to_string(&path).unwrap(), first);
        write(&path, Format::Json, &items, fixed_time(), true).unwrap();
        assert!(fs::read_to_string(&path).unwrap().starts_with("{\n  \"format\": \"secretsentry-findings\""));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        }
    }
}
