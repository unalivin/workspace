use rusqlite::{Connection, OptionalExtension, Row};

use crate::clock::{Clock, Timestamp, Zone};
use crate::error::{AppError, Result};
use crate::theme::Severity;

pub const EVENT_RECORD_URGENT: &str = "record.urgent";
pub const EVENT_RECORD_COMPLETED: &str = "record.completed";
pub const MAX_TITLE_LENGTH: usize = 120;

const SQL_SELECT_RULE: &str =
    "SELECT enabled, min_severity, quiet_start, quiet_end, dedup_minutes FROM notification_rules WHERE event_type = ?1";
const SQL_UPSERT_RULE: &str = "INSERT INTO notification_rules (event_type, enabled, min_severity, quiet_start, quiet_end, dedup_minutes, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) ON CONFLICT (event_type) DO UPDATE SET enabled = excluded.enabled, min_severity = excluded.min_severity, quiet_start = excluded.quiet_start, quiet_end = excluded.quiet_end, dedup_minutes = excluded.dedup_minutes, updated_at = excluded.updated_at";
const SQL_DELETE_RULE: &str = "DELETE FROM notification_rules WHERE event_type = ?1";
const SQL_LAST_SIMILAR: &str =
    "SELECT created_at FROM notifications WHERE event_type = ?1 AND dedup_key = ?2 ORDER BY created_at DESC LIMIT 1";
const SQL_INSERT: &str = "INSERT INTO notifications (event_type, severity, title, record_kind, record_id, dedup_key, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)";
const SQL_LIST: &str = "SELECT id, event_type, severity, title, record_kind, record_id, created_at, read_at FROM notifications WHERE (?1 = 0 OR read_at IS NULL) AND (?2 = '' OR severity = ?2) AND (?3 = '' OR event_type = ?3) ORDER BY created_at DESC, id DESC LIMIT ?4";
const SQL_UNREAD_COUNT: &str = "SELECT unread_notifications FROM overview_attention";
const SQL_MARK_READ: &str = "UPDATE notifications SET read_at = ?1 WHERE id = ?2 AND read_at IS NULL";
const SQL_MARK_ALL_READ: &str = "UPDATE notifications SET read_at = ?1 WHERE read_at IS NULL";
const SQL_AFTER: &str = "SELECT id, event_type, severity, title, record_kind, record_id, created_at, read_at FROM notifications WHERE id > ?1 ORDER BY id LIMIT ?2";
const SQL_HIGHEST_ID: &str = "SELECT coalesce(max(id), 0) FROM notifications";
const SQL_NOTIFICATION_ID: &str = "SELECT id FROM notifications WHERE id = ?1";

pub const HOT_QUERIES: &[(&str, &str, usize)] = &[
    ("notification de-duplication lookup", SQL_LAST_SIMILAR, 2),
    ("notifications after cursor", SQL_AFTER, 2),
    ("mark notification read", SQL_MARK_READ, 2),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventType {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
}

pub const EVENT_TYPES: &[EventType] = &[
    EventType {
        id: EVENT_RECORD_URGENT,
        label: "Record needs attention",
        description: "An open record has high or critical severity.",
    },
    EventType { id: EVENT_RECORD_COMPLETED, label: "Record completed", description: "A record was marked as done." },
];

pub fn event_type(id: &str) -> Option<&'static EventType> {
    EVENT_TYPES.iter().find(|event| event.id == id)
}

fn unknown_event(id: &str) -> AppError {
    AppError::invalid(
        "event",
        format!("unknown event type \"{id}\""),
        "List the notification rules to see the event types",
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub event_type: String,
    pub enabled: bool,
    pub min_severity: Severity,
    pub quiet_start: String,
    pub quiet_end: String,
    pub dedup_minutes: i64,
    pub customized: bool,
}

pub fn parse_clock(text: &str) -> Option<i64> {
    let bytes = text.as_bytes();
    if bytes.len() != 5 || bytes[2] != b':' || ![0, 1, 3, 4].iter().all(|index| bytes[*index].is_ascii_digit()) {
        return None;
    }
    let hours = i64::from(bytes[0] - b'0') * 10 + i64::from(bytes[1] - b'0');
    let minutes = i64::from(bytes[3] - b'0') * 10 + i64::from(bytes[4] - b'0');
    (hours < 24 && minutes < 60).then_some(hours * 60 + minutes)
}

impl Rule {
    pub fn default_for(event_type: &str) -> Rule {
        Rule {
            event_type: event_type.to_string(),
            enabled: true,
            min_severity: Severity::Info,
            quiet_start: String::new(),
            quiet_end: String::new(),
            dedup_minutes: 60,
            customized: false,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if event_type(&self.event_type).is_none() {
            return Err(unknown_event(&self.event_type));
        }
        if self.quiet_start.is_empty() != self.quiet_end.is_empty() {
            return Err(AppError::invalid(
                "quiet hours",
                "quiet hours need both a start and an end",
                "Give both times as HH:MM or leave both empty",
            ));
        }
        for clock in [&self.quiet_start, &self.quiet_end] {
            if !clock.is_empty() && parse_clock(clock).is_none() {
                return Err(AppError::invalid(
                    "quiet hours",
                    format!("\"{clock}\" is not a time of day"),
                    "Use 24-hour HH:MM, for example 22:30",
                ));
            }
        }
        if !(0..=10080).contains(&self.dedup_minutes) {
            return Err(AppError::invalid(
                "de-duplication",
                format!("de-duplication window {} is out of range", self.dedup_minutes),
                "Use 0 to 10080 minutes (0 turns it off)",
            ));
        }
        Ok(())
    }

    pub fn in_quiet_hours(&self, minute_of_day: i64) -> bool {
        let (Some(start), Some(end)) = (parse_clock(&self.quiet_start), parse_clock(&self.quiet_end)) else {
            return false;
        };
        if start == end {
            false
        } else if start < end {
            minute_of_day >= start && minute_of_day < end
        } else {
            minute_of_day >= start || minute_of_day < end
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Delivered,
    SuppressedDisabled,
    SuppressedSeverity,
    SuppressedQuietHours,
    SuppressedDuplicate,
}

impl Outcome {
    pub fn label(self) -> &'static str {
        match self {
            Outcome::Delivered => "delivered",
            Outcome::SuppressedDisabled => "suppressed: rule disabled",
            Outcome::SuppressedSeverity => "suppressed: below severity threshold",
            Outcome::SuppressedQuietHours => "suppressed: quiet hours",
            Outcome::SuppressedDuplicate => "suppressed: duplicate",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub event_type: String,
    pub severity: Severity,
    pub title: String,
    pub record_kind: String,
    pub record_id: i64,
    pub dedup_key: String,
}

pub fn evaluate(
    rule: &Rule,
    severity: Severity,
    now: Timestamp,
    minute_of_day: i64,
    last_similar: Option<Timestamp>,
) -> Outcome {
    if !rule.enabled {
        return Outcome::SuppressedDisabled;
    }
    if severity < rule.min_severity {
        return Outcome::SuppressedSeverity;
    }
    if rule.in_quiet_hours(minute_of_day) {
        return Outcome::SuppressedQuietHours;
    }
    if let Some(last) = last_similar {
        let elapsed = now.unix() - last.unix();
        if rule.dedup_minutes > 0 && elapsed >= 0 && elapsed < rule.dedup_minutes * 60 {
            return Outcome::SuppressedDuplicate;
        }
    }
    Outcome::Delivered
}

fn secret_like(letter: char) -> bool {
    letter.is_ascii_alphanumeric() || matches!(letter, '+' | '/' | '_' | '=' | '-')
}

pub fn safe_title(title: &str) -> String {
    let spaced: String = title.chars().map(|letter| if letter.is_control() { ' ' } else { letter }).collect();
    let mut words = Vec::new();
    for word in spaced.split_whitespace() {
        let mut cleaned = String::new();
        let mut run = String::new();
        for letter in word.chars().chain(std::iter::once(' ')) {
            if secret_like(letter) {
                run.push(letter);
                continue;
            }
            if run.chars().count() >= 24 {
                cleaned.push_str("[redacted]");
            } else {
                cleaned.push_str(&run);
            }
            run.clear();
            if letter != ' ' {
                cleaned.push(letter);
            }
        }
        words.push(cleaned);
    }
    let mut cleaned = words.join(" ");
    if cleaned.chars().count() > MAX_TITLE_LENGTH {
        cleaned = cleaned.chars().take(MAX_TITLE_LENGTH - 1).collect::<String>() + "…";
    }
    if cleaned.is_empty() { "Notification".to_string() } else { cleaned }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    pub id: i64,
    pub event_type: String,
    pub severity: Severity,
    pub title: String,
    pub record_kind: String,
    pub record_id: i64,
    pub created_at: Timestamp,
    pub read_at: Option<Timestamp>,
}

impl Notification {
    pub fn unread(&self) -> bool {
        self.read_at.is_none()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filter {
    pub unread_only: bool,
    pub severity: Option<Severity>,
    pub event_type: String,
    pub limit: i64,
}

fn load_rule(conn: &Connection, event: &str) -> Result<Rule> {
    let found = conn
        .query_row(SQL_SELECT_RULE, [event], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
            ))
        })
        .optional()?;
    let mut rule = Rule::default_for(event);
    if let Some((enabled, severity, quiet_start, quiet_end, dedup_minutes)) = found {
        rule.enabled = enabled == 1;
        rule.min_severity = Severity::parse(&severity).unwrap_or(Severity::Info);
        rule.quiet_start = quiet_start;
        rule.quiet_end = quiet_end;
        rule.dedup_minutes = dedup_minutes;
        rule.customized = true;
    }
    Ok(rule)
}

pub fn publish_with(conn: &Connection, now: Timestamp, zone: Zone, event: &Event) -> Result<(Outcome, i64)> {
    if event_type(&event.event_type).is_none() {
        return Err(unknown_event(&event.event_type));
    }
    let dedup_key = if event.dedup_key.is_empty() {
        format!("{}:{}", event.record_kind, event.record_id)
    } else {
        event.dedup_key.clone()
    };
    let rule = load_rule(conn, &event.event_type)?;
    let last: Option<String> =
        conn.query_row(SQL_LAST_SIMILAR, (&event.event_type, &dedup_key), |row| row.get(0)).optional()?;
    let last_similar = last.as_deref().and_then(Timestamp::parse);
    let outcome = evaluate(&rule, event.severity, now, zone.minute_of_day(now), last_similar);
    if outcome != Outcome::Delivered {
        return Ok((outcome, 0));
    }
    conn.execute(
        SQL_INSERT,
        (
            &event.event_type,
            event.severity.as_str(),
            safe_title(&event.title),
            &event.record_kind,
            event.record_id,
            &dedup_key,
            now.to_rfc3339(),
        ),
    )?;
    Ok((Outcome::Delivered, conn.last_insert_rowid()))
}

type RawNotification = (i64, String, String, String, String, i64, String, Option<String>);

fn scan(row: &Row<'_>) -> rusqlite::Result<RawNotification> {
    Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?))
}

fn collect(rows: impl Iterator<Item = rusqlite::Result<RawNotification>>) -> Result<Vec<Notification>> {
    let mut list = Vec::new();
    for row in rows {
        let (id, event_type, severity, title, record_kind, record_id, created, read) = row?;
        let created_at = Timestamp::parse(&created)
            .ok_or_else(|| AppError::Storage(format!("notification {id} has an unreadable timestamp")))?;
        list.push(Notification {
            id,
            event_type,
            severity: Severity::parse(&severity).unwrap_or(Severity::Info),
            title,
            record_kind,
            record_id,
            created_at,
            read_at: read.as_deref().and_then(Timestamp::parse),
        });
    }
    Ok(list)
}

pub struct Notifications<'a> {
    conn: &'a Connection,
    clock: Clock,
    zone: Zone,
}

impl<'a> Notifications<'a> {
    pub fn new(conn: &'a Connection, clock: Clock, zone: Zone) -> Self {
        Notifications { conn, clock, zone }
    }

    pub fn rule(&self, event: &str) -> Result<Rule> {
        if event_type(event).is_none() {
            return Err(unknown_event(event));
        }
        load_rule(self.conn, event)
    }

    pub fn rules(&self) -> Result<Vec<Rule>> {
        EVENT_TYPES.iter().map(|event| load_rule(self.conn, event.id)).collect()
    }

    pub fn set_rule(&self, rule: &Rule) -> Result<()> {
        rule.validate()?;
        self.conn.execute(
            SQL_UPSERT_RULE,
            (
                &rule.event_type,
                i64::from(rule.enabled),
                rule.min_severity.as_str(),
                &rule.quiet_start,
                &rule.quiet_end,
                rule.dedup_minutes,
                self.clock.now().to_rfc3339(),
            ),
        )?;
        Ok(())
    }

    pub fn reset_rule(&self, event: &str) -> Result<()> {
        if event_type(event).is_none() {
            return Err(unknown_event(event));
        }
        self.conn.execute(SQL_DELETE_RULE, [event])?;
        Ok(())
    }

    pub fn publish(&self, event: &Event) -> Result<(Outcome, i64)> {
        publish_with(self.conn, self.clock.now(), self.zone, event)
    }

    pub fn list(&self, filter: &Filter) -> Result<Vec<Notification>> {
        let limit = if filter.limit <= 0 || filter.limit > 1000 { 200 } else { filter.limit };
        let severity = filter.severity.map(Severity::as_str).unwrap_or("");
        let mut statement = self.conn.prepare(SQL_LIST)?;
        let rows = statement.query_map((i64::from(filter.unread_only), severity, &filter.event_type, limit), scan)?;
        collect(rows)
    }

    pub fn after(&self, after_id: i64, limit: i64) -> Result<Vec<Notification>> {
        let limit = if limit <= 0 || limit > 500 { 50 } else { limit };
        let mut statement = self.conn.prepare(SQL_AFTER)?;
        let rows = statement.query_map((after_id, limit), scan)?;
        collect(rows)
    }

    pub fn highest_id(&self) -> Result<i64> {
        Ok(self.conn.query_row(SQL_HIGHEST_ID, [], |row| row.get(0))?)
    }

    pub fn unread_count(&self) -> Result<i64> {
        Ok(self.conn.query_row(SQL_UNREAD_COUNT, [], |row| row.get(0))?)
    }

    pub fn mark_read(&self, id: i64) -> Result<()> {
        let found: Option<i64> = self.conn.query_row(SQL_NOTIFICATION_ID, [id], |row| row.get(0)).optional()?;
        if found.is_none() {
            return Err(AppError::NotFound(format!(
                "notification {id} (list the notifications to see valid identifiers)"
            )));
        }
        self.conn.execute(SQL_MARK_READ, (self.clock.now().to_rfc3339(), id))?;
        Ok(())
    }

    pub fn mark_all_read(&self) -> Result<usize> {
        Ok(self.conn.execute(SQL_MARK_ALL_READ, [self.clock.now().to_rfc3339()])?)
    }
}

pub fn summary(items: &[Notification], show_titles: bool) -> (String, String) {
    let title = crate::meta::APP_NAME.to_string();
    match items {
        [] => (title, String::new()),
        [only] if show_titles => (title, safe_title(&only.title)),
        [_] => (title, "1 new notification".to_string()),
        [first, rest @ ..] => {
            let mut body = format!("{} new notifications", items.len());
            if show_titles {
                let top = rest.iter().fold(first, |top, item| if item.severity > top.severity { item } else { top });
                body.push_str(&format!(", most severe: {}", safe_title(&top.title)));
            }
            (title, body)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;
    use crate::testkit::{TempDir, fixed_time};
    use proptest::prelude::*;

    fn event(severity: Severity, record_id: i64) -> Event {
        Event {
            event_type: EVENT_RECORD_URGENT.to_string(),
            severity,
            title: "A record needs attention".to_string(),
            record_kind: "record".to_string(),
            record_id,
            dedup_key: String::new(),
        }
    }

    fn rule() -> Rule {
        Rule::default_for(EVENT_RECORD_URGENT)
    }

    #[test]
    fn evaluation_checks_enabled_severity_quiet_hours_then_duplicates() {
        let now = fixed_time();
        let minute = Zone::Utc.minute_of_day(now);
        assert_eq!(evaluate(&rule(), Severity::Info, now, minute, None), Outcome::Delivered);
        let disabled = Rule { enabled: false, ..rule() };
        assert_eq!(evaluate(&disabled, Severity::Critical, now, minute, None), Outcome::SuppressedDisabled);
        let strict = Rule { min_severity: Severity::High, ..rule() };
        assert_eq!(evaluate(&strict, Severity::Medium, now, minute, None), Outcome::SuppressedSeverity);
        assert_eq!(evaluate(&strict, Severity::High, now, minute, None), Outcome::Delivered);
        let quiet = Rule { quiet_start: "09:00".to_string(), quiet_end: "10:00".to_string(), ..rule() };
        assert_eq!(evaluate(&quiet, Severity::Critical, now, minute, None), Outcome::SuppressedQuietHours);
        let recent = Some(now.plus_seconds(-59 * 60));
        assert_eq!(evaluate(&rule(), Severity::Critical, now, minute, recent), Outcome::SuppressedDuplicate);
        let old = Some(now.plus_seconds(-60 * 60));
        assert_eq!(evaluate(&rule(), Severity::Critical, now, minute, old), Outcome::Delivered);
        let no_window = Rule { dedup_minutes: 0, ..rule() };
        assert_eq!(evaluate(&no_window, Severity::Critical, now, minute, recent), Outcome::Delivered);
        let future = Some(now.plus_seconds(600));
        assert_eq!(evaluate(&rule(), Severity::Critical, now, minute, future), Outcome::Delivered);
        assert_eq!(Outcome::SuppressedQuietHours.label(), "suppressed: quiet hours");
    }

    #[test]
    fn quiet_hours_may_cross_midnight() {
        let night = Rule { quiet_start: "22:30".to_string(), quiet_end: "06:00".to_string(), ..rule() };
        for (clock, expected) in [
            ("22:29", false),
            ("22:30", true),
            ("23:59", true),
            ("00:00", true),
            ("05:59", true),
            ("06:00", false),
            ("12:00", false),
        ] {
            assert_eq!(night.in_quiet_hours(parse_clock(clock).unwrap()), expected, "{clock}");
        }
        let same = Rule { quiet_start: "08:00".to_string(), quiet_end: "08:00".to_string(), ..rule() };
        assert!(!same.in_quiet_hours(8 * 60));
        assert!(!rule().in_quiet_hours(0));
    }

    #[test]
    fn rules_are_validated_before_they_are_stored() {
        let cases = [
            (Rule { event_type: "nope".to_string(), ..rule() }, "unknown event type"),
            (Rule { quiet_start: "22:00".to_string(), ..rule() }, "both a start and an end"),
            (Rule { quiet_start: "25:00".to_string(), quiet_end: "06:00".to_string(), ..rule() }, "not a time of day"),
            (Rule { quiet_start: "9:00".to_string(), quiet_end: "06:00".to_string(), ..rule() }, "not a time of day"),
            (Rule { dedup_minutes: -1, ..rule() }, "out of range"),
            (Rule { dedup_minutes: 10081, ..rule() }, "out of range"),
        ];
        for (candidate, expected) in cases {
            let error = candidate.validate().unwrap_err().to_string();
            assert!(error.contains(expected), "{error}");
        }
        rule().validate().unwrap();
    }

    #[test]
    fn publishing_stores_delivers_and_suppresses_duplicates() {
        let directory = TempDir::new("notify-publish");
        let store = Store::open_ready(&directory.path().join("app.db")).unwrap();
        let (clock, hand) = Clock::manual(fixed_time());
        let service = Notifications::new(store.conn(), clock, Zone::Utc);
        let (outcome, id) = service.publish(&event(Severity::High, 7)).unwrap();
        assert_eq!(outcome, Outcome::Delivered);
        assert!(id > 0);
        assert_eq!(service.publish(&event(Severity::High, 7)).unwrap(), (Outcome::SuppressedDuplicate, 0));
        assert_eq!(service.publish(&event(Severity::High, 8)).unwrap().0, Outcome::Delivered);
        hand.advance(61 * 60);
        assert_eq!(service.publish(&event(Severity::High, 7)).unwrap().0, Outcome::Delivered);
        assert_eq!(service.unread_count().unwrap(), 3);
        assert_eq!(service.highest_id().unwrap(), 3);
        let listed = service.list(&Filter::default()).unwrap();
        assert_eq!(listed.iter().map(|item| item.id).collect::<Vec<_>>(), vec![3, 2, 1]);
        assert_eq!(service.after(1, 0).unwrap().iter().map(|item| item.id).collect::<Vec<_>>(), vec![2, 3]);
        service.mark_read(1).unwrap();
        assert!(service.mark_read(99).unwrap_err().to_string().contains("notification 99"));
        assert_eq!(service.list(&Filter { unread_only: true, ..Filter::default() }).unwrap().len(), 2);
        assert_eq!(service.list(&Filter { severity: Some(Severity::Low), ..Filter::default() }).unwrap().len(), 0);
        assert_eq!(service.mark_all_read().unwrap(), 2);
        assert_eq!(service.unread_count().unwrap(), 0);
        let mut unknown = event(Severity::High, 1);
        unknown.event_type = "nope".to_string();
        assert!(service.publish(&unknown).is_err());
    }

    #[test]
    fn stored_rules_change_what_is_delivered() {
        let directory = TempDir::new("notify-rules");
        let store = Store::open_ready(&directory.path().join("app.db")).unwrap();
        let service = Notifications::new(store.conn(), Clock::fixed(fixed_time()), Zone::Fixed(60));
        assert!(!service.rule(EVENT_RECORD_URGENT).unwrap().customized);
        service.set_rule(&Rule { min_severity: Severity::Critical, ..rule() }).unwrap();
        let stored = service.rule(EVENT_RECORD_URGENT).unwrap();
        assert!(stored.customized && stored.min_severity == Severity::Critical);
        assert_eq!(service.publish(&event(Severity::High, 1)).unwrap().0, Outcome::SuppressedSeverity);
        service.set_rule(&Rule { quiet_start: "10:00".to_string(), quiet_end: "11:00".to_string(), ..rule() }).unwrap();
        assert_eq!(service.publish(&event(Severity::High, 1)).unwrap().0, Outcome::SuppressedQuietHours);
        service.reset_rule(EVENT_RECORD_URGENT).unwrap();
        assert_eq!(service.publish(&event(Severity::High, 1)).unwrap().0, Outcome::Delivered);
        assert_eq!(service.rules().unwrap().len(), EVENT_TYPES.len());
        assert!(service.rule("nope").is_err() && service.reset_rule("nope").is_err());
        assert!(service.set_rule(&Rule { dedup_minutes: -5, ..rule() }).is_err());
    }

    #[test]
    fn titles_are_cleaned_shortened_and_never_carry_secrets() {
        assert_eq!(safe_title("  two\twords\n"), "two words");
        assert_eq!(safe_title(""), "Notification");
        assert_eq!(safe_title("token ghp_abcdefghijklmnopqrstuvwxyz0123 leaked"), "token [redacted] leaked");
        assert_eq!(safe_title("key=AAAAAAAAAAAAAAAAAAAAAAAAAAAA."), "[redacted].");
        assert_eq!(safe_title("short-word stays"), "short-word stays");
        let long = safe_title(&"word ".repeat(60));
        assert_eq!(long.chars().count(), MAX_TITLE_LENGTH);
        assert!(long.ends_with('…'));
    }

    #[test]
    fn the_system_notification_shows_a_count_and_titles_only_on_request() {
        let item = |id: i64, title: &str, severity: Severity| Notification {
            id,
            event_type: EVENT_RECORD_URGENT.to_string(),
            severity,
            title: title.to_string(),
            record_kind: "record".to_string(),
            record_id: id,
            created_at: fixed_time(),
            read_at: None,
        };
        let items =
            vec![item(1, "one", Severity::Low), item(2, "two", Severity::Critical), item(3, "three", Severity::High)];
        assert_eq!(summary(&[], true), (crate::meta::APP_NAME.to_string(), String::new()));
        assert_eq!(summary(&items[..1], false).1, "1 new notification");
        assert_eq!(summary(&items[..1], true).1, "one");
        assert_eq!(summary(&items, false).1, "3 new notifications");
        assert_eq!(summary(&items, true).1, "3 new notifications, most severe: two");
        assert!(items[0].unread());
    }

    proptest! {
        #[test]
        fn safe_titles_are_bounded_single_line_and_free_of_long_tokens(title in ".{0,300}") {
            let cleaned = safe_title(&title);
            prop_assert!(cleaned.chars().count() <= MAX_TITLE_LENGTH);
            prop_assert!(!cleaned.is_empty());
            prop_assert!(!cleaned.chars().any(char::is_control));
            let mut run = 0;
            for letter in cleaned.chars() {
                run = if secret_like(letter) { run + 1 } else { 0 };
                prop_assert!(run < 24);
            }
        }

        #[test]
        fn clock_parsing_accepts_exactly_valid_times(text in "[0-9:]{0,6}") {
            if let Some(minutes) = parse_clock(&text) {
                prop_assert!((0..1440).contains(&minutes));
                prop_assert_eq!(format!("{:02}:{:02}", minutes / 60, minutes % 60), text);
            }
        }
    }
}
