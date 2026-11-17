use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior};

use crate::clock::{Clock, Timestamp};
use crate::error::{AppError, Result};
use crate::fingerprint;
use crate::rules::{self, Rule, RuleSpec};
use crate::scanner::Detector;
use crate::store;
use crate::theme::Severity;

pub const MAX_SAMPLE_LENGTH: usize = 20_000;

const SQL_LIST: &str = "SELECT id, name, description, pattern, secret_group, keywords, entropy_tenths, severity, confidence, enabled, revision, created_at, updated_at FROM rules ORDER BY id";
const SQL_GET: &str = "SELECT id, name, description, pattern, secret_group, keywords, entropy_tenths, severity, confidence, enabled, revision, created_at, updated_at FROM rules WHERE id = ?1";
const SQL_INSERT: &str = "INSERT INTO rules (id, name, description, pattern, secret_group, keywords, entropy_tenths, severity, confidence, enabled, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)";
const SQL_UPDATE: &str = "UPDATE rules SET name = ?1, description = ?2, pattern = ?3, secret_group = ?4, keywords = ?5, entropy_tenths = ?6, severity = ?7, confidence = ?8, enabled = ?9, revision = revision + 1, updated_at = ?10 WHERE id = ?11 AND revision = ?12";
const SQL_SET_ENABLED: &str =
    "UPDATE rules SET enabled = ?1, revision = revision + 1, updated_at = ?2 WHERE id = ?3 AND enabled <> ?1";
const SQL_DELETE: &str = "DELETE FROM rules WHERE id = ?1";
const SQL_STATES: &str = "SELECT rule_id, enabled, updated_at FROM rule_states";
const SQL_SET_STATE: &str = "INSERT INTO rule_states (rule_id, enabled, updated_at) VALUES (?1, ?2, ?3) ON CONFLICT (rule_id) DO UPDATE SET enabled = excluded.enabled, updated_at = excluded.updated_at";
const SQL_CLEAR_STATE: &str = "DELETE FROM rule_states WHERE rule_id = ?1";
const SQL_KEY: &str = "SELECT fingerprint_key FROM identity WHERE id = 1";
const SQL_KEY_INSERT: &str = "INSERT OR IGNORE INTO identity (id, fingerprint_key, created_at) VALUES (1, ?1, ?2)";

pub const HOT_QUERIES: &[(&str, &str, usize)] = &[("custom rule by identifier", SQL_GET, 1)];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredRule {
    pub spec: RuleSpec,
    pub revision: i64,
    pub changed: bool,
    pub updated_at: Option<Timestamp>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrialHit {
    pub line: usize,
    pub column: usize,
    pub preview: String,
    pub length: usize,
    pub entropy_tenths: u32,
    pub score: u8,
    pub context: String,
}

type RawRule = (String, String, String, String, i64, String, i64, String, i64, i64, i64, String, String);

fn scan(row: &Row<'_>) -> rusqlite::Result<RawRule> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
        row.get(8)?,
        row.get(9)?,
        row.get(10)?,
        row.get(11)?,
        row.get(12)?,
    ))
}

fn build(raw: RawRule) -> Result<StoredRule> {
    let (id, name, description, pattern, group, keywords, entropy, severity, confidence, enabled, revision, _, updated) =
        raw;
    let unreadable = |what: &str| AppError::Storage(format!("rule {id} has an unreadable {what}"));
    Ok(StoredRule {
        spec: RuleSpec {
            name,
            description,
            pattern,
            secret_group: usize::try_from(group).map_err(|_| unreadable("secret group"))?,
            keywords: rules::parse_keywords(&keywords),
            entropy_tenths: u32::try_from(entropy).map_err(|_| unreadable("entropy threshold"))?,
            severity: Severity::parse(&severity).ok_or_else(|| unreadable("severity"))?,
            confidence: u8::try_from(confidence).map_err(|_| unreadable("confidence"))?,
            enabled: enabled == 1,
            builtin: false,
            id: id.clone(),
        },
        revision,
        changed: false,
        updated_at: Timestamp::parse(&updated),
    })
}

fn not_found(id: &str) -> AppError {
    AppError::NotFound(format!("rule \"{id}\" (list the rules to see valid identifiers)"))
}

fn is_builtin(id: &str) -> bool {
    rules::builtin().iter().any(|spec| spec.id == id)
}

pub fn fingerprint_key(conn: &Connection, now: Timestamp) -> Result<Vec<u8>> {
    let stored: Option<String> = conn.query_row(SQL_KEY, [], |row| row.get(0)).optional()?;
    if let Some(text) = stored {
        return fingerprint::key_from_hex(&text)
            .ok_or_else(|| AppError::Storage("the stored fingerprint key is unreadable".to_string()));
    }
    let fresh = fingerprint::new_key()?;
    conn.execute(SQL_KEY_INSERT, (fingerprint::to_hex(&fresh), now.to_rfc3339()))?;
    let text: String = conn.query_row(SQL_KEY, [], |row| row.get(0))?;
    fingerprint::key_from_hex(&text)
        .ok_or_else(|| AppError::Storage("the stored fingerprint key is unreadable".to_string()))
}

pub struct Rulebook<'a> {
    conn: &'a Connection,
    clock: Clock,
}

impl<'a> Rulebook<'a> {
    pub fn new(conn: &'a Connection, clock: Clock) -> Self {
        Rulebook { conn, clock }
    }

    fn begin(&self) -> Result<Transaction<'a>> {
        Ok(Transaction::new_unchecked(self.conn, TransactionBehavior::Immediate)?)
    }

    fn custom(&self, id: &str) -> Result<Option<StoredRule>> {
        self.conn.query_row(SQL_GET, [id], scan).optional()?.map(build).transpose()
    }

    fn builtin(&self) -> Result<Vec<StoredRule>> {
        let mut statement = self.conn.prepare(SQL_STATES)?;
        let rows = statement
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?, row.get::<_, String>(2)?)))?;
        let mut states = std::collections::BTreeMap::new();
        for row in rows {
            let (id, enabled, updated) = row?;
            states.insert(id, (enabled == 1, updated));
        }
        Ok(rules::builtin()
            .into_iter()
            .map(|spec| match states.get(&spec.id) {
                Some((enabled, updated)) => StoredRule {
                    changed: *enabled != spec.enabled,
                    spec: RuleSpec { enabled: *enabled, ..spec },
                    revision: 1,
                    updated_at: Timestamp::parse(updated),
                },
                None => StoredRule { spec, revision: 1, changed: false, updated_at: None },
            })
            .collect())
    }

    pub fn list(&self) -> Result<Vec<StoredRule>> {
        let mut all = self.builtin()?;
        let mut statement = self.conn.prepare(SQL_LIST)?;
        let rows = statement.query_map([], scan)?;
        for row in rows {
            all.push(build(row?)?);
        }
        Ok(all)
    }

    pub fn get(&self, id: &str) -> Result<StoredRule> {
        if let Some(found) = self.builtin()?.into_iter().find(|rule| rule.spec.id == id) {
            return Ok(found);
        }
        self.custom(id)?.ok_or_else(|| not_found(id))
    }

    pub fn enabled_specs(&self) -> Result<Vec<RuleSpec>> {
        Ok(self.list()?.into_iter().map(|rule| rule.spec).filter(|spec| spec.enabled).collect())
    }

    pub fn create(&self, spec: &RuleSpec) -> Result<StoredRule> {
        let cleaned = Rule::compile(&RuleSpec { builtin: false, ..spec.clone() })?.spec().clone();
        if is_builtin(&cleaned.id) {
            return Err(AppError::invalid(
                "id",
                format!("\"{}\" is the identifier of a built-in rule", cleaned.id),
                "Choose another identifier; a built-in rule can be disabled instead",
            ));
        }
        self.conn
            .execute(
                SQL_INSERT,
                (
                    &cleaned.id,
                    &cleaned.name,
                    &cleaned.description,
                    &cleaned.pattern,
                    cleaned.secret_group as i64,
                    cleaned.keywords.join(","),
                    i64::from(cleaned.entropy_tenths),
                    cleaned.severity.as_str(),
                    i64::from(cleaned.confidence),
                    i64::from(cleaned.enabled),
                    self.clock.now().to_rfc3339(),
                ),
            )
            .map_err(|error| {
                if store::is_constraint(&error) && error.to_string().contains("rules.id") {
                    AppError::invalid(
                        "id",
                        format!("a rule with the identifier \"{}\" already exists", cleaned.id),
                        "Choose another identifier or edit the existing rule",
                    )
                } else {
                    error.into()
                }
            })?;
        self.get(&cleaned.id)
    }

    pub fn update(&self, id: &str, revision: i64, spec: &RuleSpec) -> Result<StoredRule> {
        if is_builtin(id) {
            return Err(AppError::Conflict(format!(
                "\"{id}\" is a built-in rule and cannot be edited: disable it and add your own rule instead"
            )));
        }
        let cleaned = Rule::compile(&RuleSpec { id: id.to_string(), builtin: false, ..spec.clone() })?.spec().clone();
        let transaction = self.begin()?;
        let changed = transaction.execute(
            SQL_UPDATE,
            (
                &cleaned.name,
                &cleaned.description,
                &cleaned.pattern,
                cleaned.secret_group as i64,
                cleaned.keywords.join(","),
                i64::from(cleaned.entropy_tenths),
                cleaned.severity.as_str(),
                i64::from(cleaned.confidence),
                i64::from(cleaned.enabled),
                self.clock.now().to_rfc3339(),
                id,
                revision,
            ),
        )?;
        if changed == 0 {
            let exists = transaction.query_row(SQL_GET, [id], scan).optional()?.is_some();
            return Err(if exists {
                AppError::Conflict(format!(
                    "rule \"{id}\" was changed by another window: reload it and apply your change again"
                ))
            } else {
                not_found(id)
            });
        }
        transaction.commit()?;
        self.get(id)
    }

    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<StoredRule> {
        let now = self.clock.now().to_rfc3339();
        if is_builtin(id) {
            self.conn.execute(SQL_SET_STATE, (id, i64::from(enabled), now))?;
        } else {
            let transaction = self.begin()?;
            if transaction.query_row(SQL_GET, [id], scan).optional()?.is_none() {
                return Err(not_found(id));
            }
            transaction.execute(SQL_SET_ENABLED, (i64::from(enabled), now, id))?;
            transaction.commit()?;
        }
        self.get(id)
    }

    pub fn restore(&self, id: &str) -> Result<StoredRule> {
        if !is_builtin(id) {
            return Err(AppError::Conflict(format!(
                "\"{id}\" is not a built-in rule: only built-in rules can be restored"
            )));
        }
        self.conn.execute(SQL_CLEAR_STATE, [id])?;
        self.get(id)
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        if is_builtin(id) {
            return Err(AppError::Conflict(format!(
                "\"{id}\" is a built-in rule and cannot be removed: disable it instead"
            )));
        }
        if self.conn.execute(SQL_DELETE, [id])? == 0 {
            return Err(not_found(id));
        }
        Ok(())
    }
}

pub fn try_rule(spec: &RuleSpec, sample: &str) -> Result<Vec<TrialHit>> {
    if sample.chars().count() > MAX_SAMPLE_LENGTH {
        return Err(AppError::invalid(
            "sample",
            format!("the sample text is longer than {MAX_SAMPLE_LENGTH} characters"),
            "Try the rule on a shorter piece of text",
        ));
    }
    let trial = RuleSpec { enabled: true, ..spec.clone() };
    let detector = Detector::new(std::slice::from_ref(&trial), &[], &[0u8; fingerprint::KEY_LENGTH])?;
    Ok(detector
        .scan_text("sample.txt", sample)
        .candidates
        .into_iter()
        .map(|candidate| TrialHit {
            line: candidate.line,
            column: candidate.column,
            preview: candidate.preview,
            length: candidate.length,
            entropy_tenths: candidate.entropy_tenths,
            score: candidate.score,
            context: candidate.context,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;
    use crate::testkit::{TempDir, fixed_time};

    struct Fixture {
        _directory: TempDir,
        store: Store,
    }

    fn fixture(label: &str) -> Fixture {
        let directory = TempDir::new(label);
        let store = Store::open_ready(&directory.path().join("app.db")).unwrap();
        Fixture { _directory: directory, store }
    }

    fn book(fixture: &Fixture) -> Rulebook<'_> {
        Rulebook::new(fixture.store.conn(), Clock::fixed(fixed_time()))
    }

    fn custom(id: &str) -> RuleSpec {
        RuleSpec {
            id: id.to_string(),
            name: "Internal token".to_string(),
            description: "Token of the internal API".to_string(),
            pattern: "tok_([a-z0-9]{12})".to_string(),
            secret_group: 1,
            keywords: vec!["tok_".to_string(), "internal".to_string()],
            entropy_tenths: 25,
            severity: Severity::High,
            confidence: 70,
            ..RuleSpec::default()
        }
    }

    #[test]
    fn the_built_in_rules_are_listed_first_and_custom_rules_follow() {
        let fixture = fixture("rulebook-list");
        let book = book(&fixture);
        let builtin = rules::builtin();
        let listed = book.list().unwrap();
        assert_eq!(listed.iter().map(|rule| rule.spec.clone()).collect::<Vec<_>>(), builtin);
        assert!(listed.iter().all(|rule| !rule.changed && rule.updated_at.is_none()));

        let created = book.create(&custom("zz-internal")).unwrap();
        assert_eq!(created.spec, custom("zz-internal"));
        assert_eq!(created.revision, 1);
        assert_eq!(created.updated_at, Some(fixed_time()));
        book.create(&custom("aa-internal")).unwrap();
        let ids: Vec<String> = book.list().unwrap().into_iter().skip(builtin.len()).map(|rule| rule.spec.id).collect();
        assert_eq!(ids, vec!["aa-internal".to_string(), "zz-internal".to_string()]);
        assert_eq!(book.enabled_specs().unwrap().len(), builtin.len() + 2);
    }

    #[test]
    fn a_custom_rule_is_edited_disabled_and_removed() {
        let fixture = fixture("rulebook-edit");
        let book = book(&fixture);
        let created = book.create(&custom("internal")).unwrap();
        let edited = book
            .update(
                "internal",
                created.revision,
                &RuleSpec { name: "Renamed".to_string(), confidence: 80, ..custom("ignored") },
            )
            .unwrap();
        assert_eq!(
            (edited.spec.id.as_str(), edited.spec.name.as_str(), edited.spec.confidence),
            ("internal", "Renamed", 80)
        );
        assert_eq!(edited.revision, 2);

        let stale = book.update("internal", 1, &custom("internal")).unwrap_err();
        assert!(matches!(stale, AppError::Conflict(_)), "{stale}");
        assert!(stale.to_string().contains("changed by another window"));

        let disabled = book.set_enabled("internal", false).unwrap();
        assert!(!disabled.spec.enabled);
        assert_eq!(disabled.revision, 3);
        assert_eq!(book.set_enabled("internal", false).unwrap().revision, 3);
        assert_eq!(book.enabled_specs().unwrap().len(), rules::builtin().len());

        book.delete("internal").unwrap();
        assert!(matches!(book.get("internal").unwrap_err(), AppError::NotFound(_)));
        assert!(matches!(book.delete("internal").unwrap_err(), AppError::NotFound(_)));
        assert!(matches!(book.update("internal", 1, &custom("internal")).unwrap_err(), AppError::NotFound(_)));
        assert!(matches!(book.set_enabled("internal", true).unwrap_err(), AppError::NotFound(_)));
    }

    #[test]
    fn a_built_in_rule_can_be_disabled_and_restored_but_not_edited_or_removed() {
        let fixture = fixture("rulebook-builtin");
        let book = book(&fixture);
        let disabled = book.set_enabled("npm-token", false).unwrap();
        assert!(!disabled.spec.enabled && disabled.changed && disabled.spec.builtin);
        assert_eq!(disabled.updated_at, Some(fixed_time()));
        assert!(book.enabled_specs().unwrap().iter().all(|spec| spec.id != "npm-token"));
        assert!(!book.set_enabled("npm-token", true).unwrap().changed);
        book.set_enabled("npm-token", false).unwrap();
        let restored = book.restore("npm-token").unwrap();
        assert!(restored.spec.enabled && !restored.changed && restored.updated_at.is_none());

        for error in [
            book.delete("npm-token").unwrap_err(),
            book.update("npm-token", 1, &custom("npm-token")).unwrap_err(),
            book.restore("no-such-rule").unwrap_err(),
        ] {
            assert!(matches!(error, AppError::Conflict(_)), "{error}");
        }
        let taken = book.create(&custom("npm-token")).unwrap_err().to_string();
        assert!(taken.starts_with("id: \"npm-token\" is the identifier of a built-in rule"), "{taken}");
    }

    #[test]
    fn invalid_and_duplicate_rules_are_refused_and_nothing_is_stored() {
        let fixture = fixture("rulebook-invalid");
        let book = book(&fixture);
        let broken = book.create(&RuleSpec { pattern: "(".to_string(), ..custom("broken") }).unwrap_err().to_string();
        assert!(broken.starts_with("pattern: the pattern is not valid"), "{broken}");
        book.create(&custom("internal")).unwrap();
        let twice = book.create(&custom("internal")).unwrap_err().to_string();
        assert!(twice.starts_with("id: a rule with the identifier \"internal\" already exists"), "{twice}");
        let edit =
            book.update("internal", 1, &RuleSpec { secret_group: 5, ..custom("internal") }).unwrap_err().to_string();
        assert!(edit.starts_with("secret group: "), "{edit}");
        assert_eq!(book.list().unwrap().len(), rules::builtin().len() + 1);
        assert_eq!(book.get("internal").unwrap().revision, 1);
    }

    #[test]
    fn a_rule_can_be_tried_on_sample_text_before_it_is_saved() {
        let value = ["tok", "_", "a1b2c3d4e5f6"].concat();
        let hits =
            try_rule(&RuleSpec { enabled: false, ..custom("trial") }, &format!("first line\ninternal = {value}\n"))
                .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!((hits[0].line, hits[0].column, hits[0].length), (2, 12, 12));
        assert_eq!(hits[0].preview, "********");
        assert_eq!(hits[0].context, "internal = tok_********");
        assert!(try_rule(&custom("trial"), "nothing here").unwrap().is_empty());
        assert!(try_rule(&RuleSpec { pattern: "[".to_string(), ..custom("trial") }, "text").is_err());
        let long = try_rule(&custom("trial"), &"x".repeat(MAX_SAMPLE_LENGTH + 1)).unwrap_err().to_string();
        assert!(long.starts_with("sample: the sample text is longer than 20000 characters"), "{long}");
    }

    #[test]
    fn the_fingerprint_key_is_created_once_and_kept() {
        let fixture = fixture("rulebook-key");
        let first = fingerprint_key(fixture.store.conn(), fixed_time()).unwrap();
        assert_eq!(first.len(), fingerprint::KEY_LENGTH);
        assert_eq!(fingerprint_key(fixture.store.conn(), fixed_time()).unwrap(), first);
        let other = fixture_key("rulebook-key-other");
        assert_ne!(first, other);
    }

    fn fixture_key(label: &str) -> Vec<u8> {
        let other = fixture(label);
        fingerprint_key(other.store.conn(), fixed_time()).unwrap()
    }

    #[test]
    fn hot_queries_use_an_index() {
        let fixture = fixture("rulebook-plan");
        for (name, sql, parameters) in HOT_QUERIES {
            let plan = fixture.store.query_plan(sql, *parameters).unwrap().join(" | ");
            assert!(plan.contains("USING") && !plan.contains("SCAN"), "{name}: {plan}");
        }
    }
}
