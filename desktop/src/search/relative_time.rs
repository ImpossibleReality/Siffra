//! Compact "when was this last edited" labels for a search list. Recent edits
//! read as elapsed time, older ones fall back to a date, so the column stays
//! short enough to sit beside a title.

use chrono::{Datelike, Local, TimeZone};

const MINUTE: i64 = 60_000;
const HOUR: i64 = 60 * MINUTE;
const DAY: i64 = 24 * HOUR;

/// Formats `timestamp` (milliseconds since the epoch) relative to `now`.
pub fn relative_time(timestamp: i64, now: i64) -> String {
    let elapsed = now - timestamp;

    // A clock that has run backwards must not read as an age.
    if elapsed < MINUTE {
        return "just now".to_string();
    }
    if elapsed < HOUR {
        return format!("{}m ago", elapsed / MINUTE);
    }
    if elapsed < DAY {
        return format!("{}h ago", elapsed / HOUR);
    }
    if elapsed < 2 * DAY {
        return "yesterday".to_string();
    }
    if elapsed < 7 * DAY {
        return format!("{}d ago", elapsed / DAY);
    }

    let Some(date) = Local.timestamp_millis_opt(timestamp).single() else {
        return String::new();
    };
    let same_year = Local
        .timestamp_millis_opt(now)
        .single()
        .is_some_and(|today| today.year() == date.year());

    if same_year {
        date.format("%b %-d").to_string()
    } else {
        date.format("%b %-d, %Y").to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-07-28T12:00:00Z.
    const NOW: i64 = 1_785_240_000_000;

    #[test]
    fn very_recent_edits_read_as_just_now() {
        assert_eq!(relative_time(NOW, NOW), "just now");
        assert_eq!(relative_time(NOW - 30_000, NOW), "just now");
        // Clock skew must not produce a negative age.
        assert_eq!(relative_time(NOW + 5 * MINUTE, NOW), "just now");
    }

    #[test]
    fn minutes_hours_and_days_are_counted() {
        assert_eq!(relative_time(NOW - 5 * MINUTE, NOW), "5m ago");
        assert_eq!(relative_time(NOW - 3 * HOUR, NOW), "3h ago");
        assert_eq!(relative_time(NOW - 30 * HOUR, NOW), "yesterday");
        assert_eq!(relative_time(NOW - 4 * DAY, NOW), "4d ago");
    }

    #[test]
    fn anything_older_than_a_week_falls_back_to_a_date() {
        let older = relative_time(NOW - 30 * DAY, NOW);
        assert!(!older.contains("ago"));
        assert!(!older.is_empty());

        // A different year is spelled out, so two Junes cannot read alike.
        assert!(relative_time(NOW - 400 * DAY, NOW).contains("2025"));
    }
}
