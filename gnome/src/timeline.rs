// SPDX-License-Identifier: GPL-3.0-or-later

//! GTK-free view logic for the timeline and search (product spec §4.5–4.6).

use jiff::civil::Date;
use quaderno_vault::{Entry, EntryType};
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};
use uuid::Uuid;

/// The entry-list filter (ui-spec §3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    /// Every entry.
    All,
    /// Journal pages only.
    Journal,
    /// Dreams only.
    Dreams,
    /// Quick notes only.
    Notes,
}

/// Whether an entry passes a filter.
pub fn passes(entry: &Entry, filter: Filter) -> bool {
    match filter {
        Filter::All => true,
        Filter::Journal => entry.entry_type == EntryType::Journal,
        Filter::Dreams => entry.entry_type == EntryType::Dream,
        Filter::Notes => entry.entry_type == EntryType::Note,
    }
}

/// A row in the entry list.
#[derive(Debug, Clone)]
pub struct EntrySummary {
    /// The entry's id.
    pub id: Uuid,
    /// The derived title.
    pub title: String,
    /// A short plain-text excerpt.
    pub excerpt: String,
    /// The editorial date.
    pub dated_at: jiff::Zoned,
    /// The first tag linked to the entry, if any.
    pub first_tag: Option<String>,
    /// The entry type (used for the filter chips soon).
    pub entry_type: EntryType,
}

/// Entries grouped under a journal day, newest day first.
#[derive(Debug, Clone)]
pub struct DayGroup {
    /// The journal day.
    pub date: Date,
    /// A human label: "Today", "Yesterday", or a weekday and date.
    pub label: String,
    /// The day's entries, newest first.
    pub entries: Vec<EntrySummary>,
}

/// Builds the list model: groups entries by journal day, newest first.
pub fn group(
    entries: Vec<(Entry, Option<String>)>,
    day_end: (u8, u8),
    today: Date,
) -> Vec<DayGroup> {
    let mut rows: Vec<(Date, EntrySummary)> = entries
        .into_iter()
        .map(|(entry, tag)| {
            let day = quaderno_vault::time::journal_day(&entry.dated_at, day_end)
                .unwrap_or_else(|_| entry.dated_at.date());
            (day, summarize(entry, tag))
        })
        .collect();
    rows.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.dated_at.cmp(&a.1.dated_at)));

    let mut groups: Vec<DayGroup> = Vec::new();
    for (date, summary) in rows {
        if groups.last().map(|group| group.date) != Some(date) {
            groups.push(DayGroup {
                date,
                label: day_label(date, today),
                entries: Vec::new(),
            });
        }
        if let Some(group) = groups.last_mut() {
            group.entries.push(summary);
        }
    }
    groups
}

fn summarize(entry: Entry, first_tag: Option<String>) -> EntrySummary {
    let (title, excerpt) = title_and_excerpt(&entry.content);
    EntrySummary {
        id: entry.id,
        title,
        excerpt,
        dated_at: entry.dated_at,
        first_tag,
        entry_type: entry.entry_type,
    }
}

/// The derived title and a short excerpt (product spec §4.2).
pub fn title_and_excerpt(content: &str) -> (String, String) {
    let title = quaderno_vault::text::title(content);
    let mut excerpt = String::new();
    for line in content.lines() {
        let line = strip_markers(line);
        if line.is_empty() || line == title {
            if excerpt.is_empty() {
                continue;
            }
            break;
        }
        if !excerpt.is_empty() {
            excerpt.push(' ');
        }
        excerpt.push_str(&line);
        if excerpt.chars().count() >= 140 {
            break;
        }
    }
    let excerpt: String = excerpt.chars().take(140).collect();
    (title, excerpt)
}

fn strip_markers(line: &str) -> String {
    let trimmed = line.trim();
    let without_prefix = trimmed.trim_start_matches(['#', '>', '-', '*', '+']).trim();
    without_prefix
        .replace("**", "")
        .replace(['*', '_', '`'], "")
}

fn day_label(date: Date, today: Date) -> String {
    if date == today {
        return gettextrs::gettext("Today");
    }
    if today
        .yesterday()
        .map(|yesterday| yesterday == date)
        .unwrap_or(false)
    {
        return gettextrs::gettext("Yesterday");
    }
    date.strftime("%A %d %B").to_string()
}

/// A tiny in-memory, case- and accent-insensitive search index (spec §4.6).
///
/// It lives only in memory and is dropped when the vault locks.
#[derive(Debug, Default)]
pub struct SearchIndex {
    records: Vec<Record>,
}

#[derive(Debug)]
struct Record {
    id: Uuid,
    haystack: String,
}

impl SearchIndex {
    /// An empty index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an entry with the text it should match on (content, subject names,
    /// choice labels, colour names).
    pub fn add<I: IntoIterator<Item = String>>(&mut self, id: Uuid, parts: I) {
        let mut haystack = String::new();
        for part in parts {
            haystack.push(' ');
            haystack.push_str(&normalize(&part));
        }
        self.records.push(Record { id, haystack });
    }

    /// Returns the ids whose index text contains `query`.
    pub fn search(&self, query: &str) -> Vec<Uuid> {
        let needle = normalize(query);
        if needle.is_empty() {
            return Vec::new();
        }
        self.records
            .iter()
            .filter(|record| record.haystack.contains(&needle))
            .map(|record| record.id)
            .collect()
    }

    /// Whether the index is empty.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

/// Lowercases and strips accents so matching ignores case and diacritics.
pub fn normalize(text: &str) -> String {
    text.nfkd()
        .filter(|character| !is_combining_mark(*character))
        .flat_map(char::to_lowercase)
        .collect::<String>()
}

/// Moves `dated_at` to `date` and `time`, keeping its stored offset
/// (product spec §4.3, vault spec §5.5).
pub fn shift_datetime(
    dated_at: &jiff::Zoned,
    date: Date,
    time: jiff::civil::Time,
) -> Result<jiff::Zoned, jiff::Error> {
    dated_at.with().date(date).time(time).build()
}

/// Parses a `HH:MM` time field.
pub fn parse_time(text: &str) -> Option<jiff::civil::Time> {
    let (hours, minutes) = text.split_once(':')?;
    let hours: i8 = hours.trim().parse().ok()?;
    let minutes: i8 = minutes.trim().parse().ok()?;
    jiff::civil::Time::new(hours, minutes, 0, 0).ok()
}

/// Whether text makes an entry an empty draft (product spec §4.2).
pub fn is_empty_draft(text: &str) -> bool {
    text.trim().is_empty()
}

/// Whole days from `from` to `to`.
pub fn day_difference(from: Date, to: Date) -> i64 {
    let start = from.at(0, 0, 0, 0).to_zoned(jiff::tz::TimeZone::UTC);
    let end = to.at(0, 0, 0, 0).to_zoned(jiff::tz::TimeZone::UTC);
    match (start, end) {
        (Ok(start), Ok(end)) => {
            (end.timestamp().as_second() - start.timestamp().as_second()) / 86_400
        }
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dated(value: &str) -> jiff::Zoned {
        quaderno_vault::time::parse_dated(value).unwrap()
    }

    fn entry(id: u128, content: &str, dated_at: &str) -> Entry {
        Entry {
            id: Uuid::from_u128(id),
            entry_type: EntryType::Journal,
            content: content.to_owned(),
            dated_at: dated(dated_at),
            mood: None,
            energy: None,
            cognitive_load: None,
            sleep: None,
            lucid: None,
            nightmare: None,
            recurring: None,
            source_id: None,
            conflict_of: None,
            created_at: jiff::Timestamp::from_second(0).unwrap(),
            updated_at: jiff::Timestamp::from_second(0).unwrap(),
            deleted_at: None,
        }
    }

    #[test]
    fn groups_entries_by_journal_day_newest_first() {
        let today = "2026-09-29".parse::<Date>().unwrap();
        let entries: Vec<(Entry, Option<String>)> = vec![
            (entry(1, "a", "2026-09-28T22:00:00+02:00"), None),
            (entry(2, "b", "2026-09-29T01:00:00+02:00"), None), // journal day 28
            (entry(3, "c", "2026-09-29T09:00:00+02:00"), None), // journal day 29
        ];
        let groups = group(entries, (3, 0), today);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].label, "Today");
        assert_eq!(groups[0].entries.len(), 1);
        assert_eq!(groups[1].label, "Yesterday");
        assert_eq!(groups[1].entries.len(), 2);
    }

    #[test]
    fn derives_title_and_excerpt() {
        let (title, excerpt) =
            title_and_excerpt("# A slow Saturday\n\nWoke up late, **nine hours** of sleep.");
        assert_eq!(title, "A slow Saturday");
        assert_eq!(excerpt, "Woke up late, nine hours of sleep.");
    }

    #[test]
    fn search_is_case_and_accent_insensitive() {
        let mut index = SearchIndex::new();
        index.add(
            Uuid::from_u128(1),
            ["Caffè con Marta".to_owned(), "Calm".to_owned()],
        );
        index.add(Uuid::from_u128(2), ["A walk by the lake".to_owned()]);
        assert_eq!(index.search("CAFFE"), vec![Uuid::from_u128(1)]);
        assert_eq!(index.search("calm"), vec![Uuid::from_u128(1)]);
        assert_eq!(index.search("lake"), vec![Uuid::from_u128(2)]);
        assert!(index.search("missing").is_empty());
    }

    #[test]
    fn filters_and_search_combine() {
        let entries = [
            (
                entry(1, "lake", "2026-09-29T09:00:00+02:00"),
                None::<String>,
            ),
            (
                entry(2, "lake", "2026-09-29T09:00:00+02:00"),
                None::<String>,
            ),
        ];
        // Only journal pages pass the Journal filter.
        assert!(
            entries
                .iter()
                .all(|(entry, _)| passes(entry, Filter::Journal))
        );
        assert!(!passes(&entries[0].0, Filter::Dreams));

        let mut index = SearchIndex::new();
        for (entry, _) in &entries {
            index.add(entry.id, [entry.content.clone()]);
        }
        assert_eq!(index.search("lake").len(), 2);
        assert!(index.search("zzz").is_empty());
    }

    #[test]
    fn shifting_the_date_keeps_time_and_offset() {
        let original = dated("2026-09-22T07:40:00+02:00");
        let target = "2026-10-05".parse::<Date>().unwrap();
        let shifted = shift_datetime(&original, target, original.time()).unwrap();
        assert_eq!(
            format!("{}{}", shifted.datetime(), ""),
            "2026-10-05T07:40:00"
        );
        assert_eq!(shifted.offset().seconds(), 2 * 3600);
    }

    #[test]
    fn parses_times_and_detects_empty_drafts() {
        assert!(parse_time("09:30").is_some());
        assert!(parse_time("9:5").is_some());
        assert!(parse_time("nope").is_none());
        assert!(is_empty_draft("  \n "));
        assert!(!is_empty_draft("x"));
    }

    #[test]
    fn day_difference_counts_days() {
        let a = "2026-09-22".parse::<Date>().unwrap();
        let b = "2026-09-27".parse::<Date>().unwrap();
        assert_eq!(day_difference(a, b), 5);
        assert_eq!(day_difference(b, a), -5);
        assert_eq!(day_difference(a, a), 0);
    }
}
