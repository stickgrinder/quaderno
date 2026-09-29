// SPDX-License-Identifier: GPL-3.0-or-later

//! Time handling, always through [`jiff`] (`AGENTS.md`).
//!
//! - technical timestamps (`created_at`, `updated_at`, `deleted_at`) are UTC,
//!   RFC 3339 with `Z` and whole seconds;
//! - editorial time (`dated_at`) is RFC 3339 with the writer's UTC offset and
//!   whole seconds;
//! - the journal day is the calendar date of `dated_at` in its own offset,
//!   after subtracting `day_end` (spec §5.4).

use jiff::civil::Date;
use jiff::tz::{Offset, TimeZone};
use jiff::{Span, Timestamp, Zoned};

use crate::error::VaultError;

/// The current UTC time, truncated to whole seconds.
pub fn now() -> Timestamp {
    // `as_second` is the number of seconds since the epoch, always a valid
    // input for `from_second`.
    Timestamp::from_second(Timestamp::now().as_second())
        .expect("a timestamp's own second is always in range")
}

/// Formats a technical timestamp as UTC RFC 3339 with `Z`.
pub fn format_timestamp(timestamp: Timestamp) -> String {
    timestamp.to_string()
}

/// Parses a technical timestamp.
pub fn parse_timestamp(value: &str) -> Result<Timestamp, VaultError> {
    value
        .parse()
        .map_err(|error| VaultError::Invalid(format!("bad timestamp {value:?}: {error}")))
}

/// Formats `dated_at` as RFC 3339 with its stored UTC offset and whole seconds.
pub fn format_dated(dated_at: &Zoned) -> String {
    let whole = truncate_dated(dated_at);
    format!("{}{}", whole.datetime(), format_offset(whole.offset()))
}

/// Formats a UTC offset as `+HH:MM` / `-HH:MM` (always with minutes).
fn format_offset(offset: Offset) -> String {
    let seconds = offset.seconds();
    let sign = if seconds < 0 { '-' } else { '+' };
    let absolute = seconds.unsigned_abs();
    format!("{sign}{:02}:{:02}", absolute / 3600, (absolute % 3600) / 60)
}

/// Truncates `dated_at` to whole seconds, keeping its offset.
pub fn truncate_dated(dated_at: &Zoned) -> Zoned {
    Timestamp::from_second(dated_at.timestamp().as_second())
        .expect("a timestamp's own second is always in range")
        .to_zoned(dated_at.time_zone().clone())
}

/// Parses `dated_at` (RFC 3339 with an offset). The stored offset is kept as a
/// fixed time zone, so formatting round-trips it.
pub fn parse_dated(value: &str) -> Result<Zoned, VaultError> {
    let timestamp: Timestamp = value
        .parse()
        .map_err(|error| VaultError::Invalid(format!("bad dated_at {value:?}: {error}")))?;
    let offset = offset_from_str(value).ok_or_else(|| {
        VaultError::Invalid(format!(
            "bad dated_at {value:?}: missing or invalid UTC offset"
        ))
    })?;
    Ok(timestamp.to_zoned(TimeZone::fixed(offset)))
}

/// Extracts the UTC offset from an RFC 3339 string (`Z`, `+HH:MM` or `-HH:MM`).
fn offset_from_str(value: &str) -> Option<Offset> {
    let zone = value.get(19..)?;
    if zone.eq_ignore_ascii_case("z") {
        return Some(Offset::UTC);
    }
    let sign = match value.as_bytes().get(19)? {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    if value.as_bytes().get(22) != Some(&b':') {
        return None;
    }
    let hours: i32 = value.get(20..22)?.parse().ok()?;
    let minutes: i32 = value.get(23..25)?.parse().ok()?;
    Offset::from_seconds(sign * (hours * 3600 + minutes * 60)).ok()
}

/// The journal day of `dated_at`: its calendar date after subtracting
/// `day_end`, computed in the offset stored with `dated_at` (spec §5.4).
pub fn journal_day(dated_at: &Zoned, day_end: (u8, u8)) -> Result<Date, VaultError> {
    let span = Span::new()
        .hours(i64::from(day_end.0))
        .minutes(i64::from(day_end.1));
    let shifted = dated_at
        .datetime()
        .checked_sub(span)
        .map_err(|error| VaultError::Invalid(format!("cannot compute the journal day: {error}")))?;
    Ok(shifted.date())
}

/// Parses a `day_end` value, `HH:MM` from `00:00` to `06:00` (spec §4.6).
pub fn parse_day_end(value: &str) -> Result<(u8, u8), VaultError> {
    let invalid = || {
        VaultError::Invalid(format!(
            "day_end must be HH:MM between 00:00 and 06:00, got {value:?}"
        ))
    };
    let (hours, minutes) = value.split_once(':').ok_or_else(invalid)?;
    let hours: u8 = hours.parse().map_err(|_| invalid())?;
    let minutes: u8 = minutes.parse().map_err(|_| invalid())?;
    if hours > 6 || minutes > 59 || (hours == 6 && minutes > 0) {
        return Err(invalid());
    }
    Ok((hours, minutes))
}

/// Formats `day_end` as `HH:MM`.
pub fn format_day_end((hours, minutes): (u8, u8)) -> String {
    format!("{hours:02}:{minutes:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zoned(value: &str) -> Zoned {
        parse_dated(value).unwrap()
    }

    #[test]
    fn technical_timestamps_are_utc_with_z_and_whole_seconds() {
        let value = "2026-09-28T04:41:12Z";
        let timestamp = parse_timestamp(value).unwrap();
        assert_eq!(format_timestamp(timestamp), value);
    }

    #[test]
    fn now_has_whole_seconds() {
        assert_eq!(now().subsec_nanosecond(), 0);
    }

    #[test]
    fn dated_at_keeps_its_offset() {
        let value = "2026-09-28T06:41:12+02:00";
        let dated = parse_dated(value).unwrap();
        assert_eq!(format_dated(&dated), value);
    }

    #[test]
    fn journal_day_shifts_back_over_the_boundary() {
        let day_end = (3, 0);
        assert_eq!(
            journal_day(&zoned("2026-09-29T01:30:00+02:00"), day_end).unwrap(),
            "2026-09-28".parse::<Date>().unwrap()
        );
        assert_eq!(
            journal_day(&zoned("2026-09-29T02:59:59+02:00"), day_end).unwrap(),
            "2026-09-28".parse::<Date>().unwrap()
        );
    }

    #[test]
    fn journal_day_at_the_boundary_stays_on_the_day() {
        let day_end = (3, 0);
        assert_eq!(
            journal_day(&zoned("2026-09-29T03:00:00+02:00"), day_end).unwrap(),
            "2026-09-29".parse::<Date>().unwrap()
        );
    }

    #[test]
    fn day_end_round_trips() {
        for value in ["00:00", "03:00", "05:30", "06:00"] {
            let (hours, minutes) = parse_day_end(value).unwrap();
            assert_eq!(format_day_end((hours, minutes)), value);
        }
    }

    #[test]
    fn day_end_rejects_out_of_range_values() {
        for value in ["", "3", "24", "07:00", "06:01", "03:60"] {
            assert!(
                parse_day_end(value).is_err(),
                "{value:?} should be rejected"
            );
        }
    }
}
