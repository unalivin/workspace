use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(i64);

const SECONDS_PER_DAY: i64 = 86_400;

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let shifted_year = if month <= 2 { year - 1 } else { year };
    let era = shifted_year.div_euclid(400);
    let year_of_era = shifted_year.rem_euclid(400);
    let shifted_month = (month + 9) % 12;
    let day_of_year = (153 * shifted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era = (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 { shifted_month + 3 } else { shifted_month - 9 };
    let year = year_of_era + era * 400 + if month <= 2 { 1 } else { 0 };
    (year, month, day)
}

fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        _ => 28,
    }
}

fn digits(text: &str) -> Option<i64> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

impl Timestamp {
    pub fn from_unix(seconds: i64) -> Self {
        Timestamp(seconds)
    }

    pub fn from_civil(year: i64, month: i64, day: i64, hour: i64, minute: i64, second: i64) -> Option<Self> {
        let valid = (1..=9999).contains(&year)
            && (1..=12).contains(&month)
            && (1..=days_in_month(year, month)).contains(&day)
            && (0..24).contains(&hour)
            && (0..60).contains(&minute)
            && (0..60).contains(&second);
        valid.then(|| {
            Timestamp(days_from_civil(year, month, day) * SECONDS_PER_DAY + hour * 3600 + minute * 60 + second)
        })
    }

    pub fn now() -> Self {
        let seconds = SystemTime::now().duration_since(UNIX_EPOCH).map(|elapsed| elapsed.as_secs() as i64).unwrap_or(0);
        Timestamp(seconds)
    }

    pub fn unix(self) -> i64 {
        self.0
    }

    pub fn plus_seconds(self, seconds: i64) -> Self {
        Timestamp(self.0 + seconds)
    }

    pub fn civil(self, offset_minutes: i64) -> (i64, i64, i64, i64, i64, i64) {
        let local = self.0 + offset_minutes * 60;
        let (year, month, day) = civil_from_days(local.div_euclid(SECONDS_PER_DAY));
        let rest = local.rem_euclid(SECONDS_PER_DAY);
        (year, month, day, rest / 3600, rest % 3600 / 60, rest % 60)
    }

    pub fn to_rfc3339(self) -> String {
        let (year, month, day, hour, minute, second) = self.civil(0);
        format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
    }

    pub fn parse(text: &str) -> Option<Self> {
        let bytes = text.as_bytes();
        if bytes.len() != 20 || !text.is_ascii() {
            return None;
        }
        let separators = [(4, b'-'), (7, b'-'), (10, b'T'), (13, b':'), (16, b':'), (19, b'Z')];
        if separators.iter().any(|(index, expected)| bytes[*index] != *expected) {
            return None;
        }
        Timestamp::from_civil(
            digits(&text[0..4])?,
            digits(&text[5..7])?,
            digits(&text[8..10])?,
            digits(&text[11..13])?,
            digits(&text[14..16])?,
            digits(&text[17..19])?,
        )
    }

    pub fn display(self, offset_minutes: i64) -> String {
        let (year, month, day, hour, minute, _) = self.civil(offset_minutes);
        format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}")
    }

    pub fn minute_of_day(self, offset_minutes: i64) -> i64 {
        let (_, _, _, hour, minute, _) = self.civil(offset_minutes);
        hour * 60 + minute
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zone {
    Utc,
    Local,
    Fixed(i64),
}

impl Zone {
    pub fn offset_minutes(self, moment: Timestamp) -> i64 {
        match self {
            Zone::Utc => 0,
            Zone::Fixed(minutes) => minutes,
            Zone::Local => {
                use chrono::{Offset, TimeZone};
                match chrono::Local.timestamp_opt(moment.unix(), 0) {
                    chrono::LocalResult::Single(local) => i64::from(local.offset().fix().local_minus_utc()) / 60,
                    _ => 0,
                }
            }
        }
    }

    pub fn display(self, moment: Timestamp) -> String {
        let text = moment.display(self.offset_minutes(moment));
        if self == Zone::Utc { format!("{text} UTC") } else { text }
    }

    pub fn minute_of_day(self, moment: Timestamp) -> i64 {
        moment.minute_of_day(self.offset_minutes(moment))
    }
}

#[derive(Clone)]
pub struct Clock(Arc<dyn Fn() -> Timestamp + Send + Sync>);

impl Clock {
    pub fn system() -> Self {
        Clock(Arc::new(Timestamp::now))
    }

    pub fn fixed(moment: Timestamp) -> Self {
        Clock(Arc::new(move || moment))
    }

    pub fn manual(start: Timestamp) -> (Self, ManualClock) {
        let seconds = Arc::new(AtomicI64::new(start.unix()));
        let shared = Arc::clone(&seconds);
        (Clock(Arc::new(move || Timestamp(shared.load(Ordering::SeqCst)))), ManualClock(seconds))
    }

    pub fn now(&self) -> Timestamp {
        (self.0)()
    }
}

#[derive(Clone)]
pub struct ManualClock(Arc<AtomicI64>);

impl ManualClock {
    pub fn advance(&self, seconds: i64) {
        self.0.fetch_add(seconds, Ordering::SeqCst);
    }

    pub fn set(&self, moment: Timestamp) {
        self.0.store(moment.unix(), Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn known_moments_round_trip() {
        let moment = Timestamp::from_civil(2026, 3, 2, 9, 30, 0).unwrap();
        assert_eq!(moment.unix(), 1_772_443_800);
        assert_eq!(moment.to_rfc3339(), "2026-03-02T09:30:00Z");
        assert_eq!(Timestamp::parse("2026-03-02T09:30:00Z"), Some(moment));
        assert_eq!(Timestamp::from_unix(0).to_rfc3339(), "1970-01-01T00:00:00Z");
        assert_eq!(Timestamp::from_unix(951_782_400).to_rfc3339(), "2000-02-29T00:00:00Z");
    }

    #[test]
    fn display_applies_the_offset_and_drops_seconds() {
        let moment = Timestamp::from_civil(2026, 12, 31, 23, 15, 59).unwrap();
        assert_eq!(moment.display(0), "2026-12-31 23:15");
        assert_eq!(moment.display(120), "2027-01-01 01:15");
        assert_eq!(moment.display(-330), "2026-12-31 17:45");
        assert_eq!(moment.minute_of_day(0), 23 * 60 + 15);
    }

    #[test]
    fn parse_rejects_malformed_text() {
        for text in [
            "",
            "2026-03-02",
            "2026-03-02T09:30:00",
            "2026-13-02T09:30:00Z",
            "2026-02-30T09:30:00Z",
            "2026-03-02 09:30:00Z",
            "2026-03-02T24:00:00Z",
            "２026-03-02T09:30:00Z",
        ] {
            assert_eq!(Timestamp::parse(text), None, "{text}");
        }
    }

    #[test]
    fn zones_shift_the_display_and_the_minute_of_day() {
        let moment = Timestamp::from_civil(2026, 3, 2, 23, 30, 0).unwrap();
        assert_eq!(Zone::Utc.display(moment), "2026-03-02 23:30 UTC");
        assert_eq!(Zone::Fixed(60).display(moment), "2026-03-03 00:30");
        assert_eq!(Zone::Fixed(60).minute_of_day(moment), 30);
        assert_eq!(Zone::Utc.minute_of_day(moment), 23 * 60 + 30);
        assert!(Zone::Local.offset_minutes(moment).abs() <= 14 * 60);
    }

    #[test]
    fn manual_clock_moves_only_when_told() {
        let start = Timestamp::from_civil(2026, 3, 2, 9, 30, 0).unwrap();
        let (clock, hand) = Clock::manual(start);
        assert_eq!(clock.now(), start);
        hand.advance(90);
        assert_eq!(clock.now(), start.plus_seconds(90));
        assert_eq!(Clock::fixed(start).now(), start);
    }

    proptest! {
        #[test]
        fn every_second_round_trips_through_text(seconds in -62_135_596_800i64..253_402_300_799i64) {
            let moment = Timestamp::from_unix(seconds);
            prop_assert_eq!(Timestamp::parse(&moment.to_rfc3339()), Some(moment));
        }

        #[test]
        fn parse_never_panics(text in ".{0,40}") {
            let _ = Timestamp::parse(&text);
        }
    }
}
