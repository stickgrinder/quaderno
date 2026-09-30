// SPDX-License-Identifier: GPL-3.0-or-later

//! GTK-free view logic for the details panel (ui-spec §3.6, product spec §5,
//! vault-spec §5.1). Kept apart from the widgets so it can be unit-tested.

use jiff::Zoned;
use quaderno_vault::{Choice, Color, EntryType, Rating, SubjectKind};

/// Which sections the details panel offers for an entry type (spec §5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sections {
    /// The four rating rows.
    pub ratings: bool,
    /// The emotions chip field.
    pub emotions: bool,
    /// The day-activities chip field.
    pub activities: bool,
    /// The twelve fixed colours.
    pub colors: bool,
    /// The people, places and things token fields.
    pub subjects: bool,
    /// The tags token field.
    pub tags: bool,
}

/// The sections shown for an entry type: journal pages and dreams get
/// everything the schema allows; quick notes only tags (vault-spec §5.1).
pub fn sections_for(entry_type: EntryType) -> Sections {
    match entry_type {
        EntryType::Journal | EntryType::Dream => Sections {
            ratings: true,
            emotions: true,
            activities: true,
            colors: true,
            subjects: true,
            tags: true,
        },
        EntryType::Note => Sections {
            ratings: false,
            emotions: false,
            activities: false,
            colors: false,
            subjects: false,
            tags: true,
        },
    }
}

/// The title of a rating row. On a dream the mood rating is mood on waking
/// (spec §4.2).
pub fn rating_title(rating: Rating, entry_type: EntryType) -> String {
    match rating {
        Rating::Mood if entry_type == EntryType::Dream => gettextrs::gettext("Mood on waking"),
        Rating::Mood => gettextrs::gettext("Mood"),
        Rating::Energy => gettextrs::gettext("Energy"),
        Rating::CognitiveLoad => gettextrs::gettext("Cognitive load"),
        Rating::Sleep => gettextrs::gettext("Sleep quality"),
    }
}

/// The translated label of a rating value, with the scale's gettext context
/// (vault-spec §6.2, Appendix B).
pub fn rating_label(rating: Rating, value: u8) -> Option<String> {
    let (context, label) = match rating {
        Rating::Mood => (
            "rating.mood",
            match value {
                1 => "Awful",
                2 => "Bad",
                3 => "Okay",
                4 => "Good",
                5 => "Great",
                _ => return None,
            },
        ),
        Rating::Energy => (
            "rating.energy",
            match value {
                1 => "Drained",
                2 => "Low",
                3 => "Steady",
                4 => "High",
                5 => "Charged",
                _ => return None,
            },
        ),
        Rating::CognitiveLoad => (
            "rating.cognitive_load",
            match value {
                1 => "Idle",
                2 => "Light",
                3 => "Moderate",
                4 => "Heavy",
                5 => "Overloaded",
                _ => return None,
            },
        ),
        Rating::Sleep => (
            "rating.sleep",
            match value {
                1 => "Sleepless",
                2 => "Poor",
                3 => "Fair",
                4 => "Good",
                5 => "Deep",
                _ => return None,
            },
        ),
    };
    Some(gettextrs::pgettext(context, label))
}

/// The Phosphor icon name of a rating value (vault-spec Appendix B).
pub fn rating_icon(rating: Rating, value: u8) -> Option<&'static str> {
    let icons: [&str; 5] = match rating {
        Rating::Mood => [
            "smiley-sad",
            "smiley-nervous",
            "smiley-meh",
            "smiley",
            "smiley-wink",
        ],
        Rating::Energy => [
            "battery-empty",
            "battery-low",
            "battery-medium",
            "battery-high",
            "battery-full",
        ],
        Rating::CognitiveLoad => [
            "cell-signal-none",
            "cell-signal-low",
            "cell-signal-medium",
            "cell-signal-high",
            "cell-signal-full",
        ],
        Rating::Sleep => ["eye", "alarm", "cloud-moon", "moon", "bed"],
    };
    value
        .checked_sub(1)
        .and_then(|index| icons.get(usize::from(index)).copied())
}

/// The translated name of one of the twelve fixed colours, with the `color`
/// gettext context (vault-spec §6.2, Appendix C).
pub fn color_label(color: Color) -> String {
    let name = match color {
        Color::Red => "Red",
        Color::Orange => "Orange",
        Color::Yellow => "Yellow",
        Color::Green => "Green",
        Color::Teal => "Teal",
        Color::Blue => "Blue",
        Color::Purple => "Purple",
        Color::Pink => "Pink",
        Color::Brown => "Brown",
        Color::Gray => "Gray",
        Color::Black => "Black",
        Color::White => "White",
    };
    gettextrs::pgettext("color", name)
}

/// The translated name of a choice: `custom_label` if set, otherwise the
/// translation of the frozen default `label` with the kind as gettext context
/// (vault-spec §4.3, §6.2).
pub fn choice_display_label(choice: &Choice) -> String {
    if let Some(custom) = &choice.custom_label {
        return custom.clone();
    }
    match &choice.label {
        Some(label) => gettextrs::pgettext(choice.kind.as_str(), label),
        None => String::new(),
    }
}

/// The singular noun for a subject kind, used in the token placeholder and the
/// create row.
pub fn subject_noun(kind: SubjectKind) -> String {
    match kind {
        SubjectKind::Person => gettextrs::gettext("person"),
        SubjectKind::Place => gettextrs::gettext("place"),
        SubjectKind::Thing => gettextrs::gettext("thing"),
        SubjectKind::Tag => gettextrs::gettext("tag"),
    }
}

/// The placeholder of a token field, e.g. "Add person…".
pub fn subject_placeholder(kind: SubjectKind) -> String {
    gettextrs::gettext("Add {kind}…").replace("{kind}", &subject_noun(kind))
}

/// The heading of the token group: "People, places & things", or "Tags" when
/// only tags are offered (a quick note).
pub fn subjects_group_title(entry_type: EntryType) -> String {
    if entry_type == EntryType::Note {
        gettextrs::gettext("Tags")
    } else {
        gettextrs::gettext("People, places & things")
    }
}

/// The label of the "Create …" suggestion row.
pub fn create_label(kind: SubjectKind, name: &str) -> String {
    gettextrs::gettext("Create {kind} “{name}”")
        .replace("{kind}", &subject_noun(kind))
        .replace("{name}", name)
}

/// A dim usage line for a suggestion row: "14 entries · last 12 Sep", or `None`
/// when the subject is unused.
pub fn usage_detail(entry_count: i64, last_used: Option<&Zoned>) -> Option<String> {
    if entry_count <= 0 {
        return None;
    }
    let count = gettextrs::ngettext("{n} entry", "{n} entries", entry_count as u32)
        .replace("{n}", &entry_count.to_string());
    match last_used {
        Some(last_used) => Some(format!(
            "{count} · {} {}",
            gettextrs::gettext("last"),
            last_used.strftime("%d %b")
        )),
        None => Some(count),
    }
}

/// Case- and accent-insensitive matching for token suggestions, per
/// vault-spec §5.3. An empty query matches everything.
pub fn token_matches(query: &str, name: &str) -> bool {
    let query = crate::timeline::normalize(query.trim());
    if query.is_empty() {
        return true;
    }
    crate::timeline::normalize(name).contains(&query)
}

/// Whether the query is exactly a name, ignoring case, accents and surrounding
/// whitespace (vault-spec §5.3).
pub fn token_is_exact(query: &str, name: &str) -> bool {
    let query = crate::timeline::normalize(query.trim());
    !query.is_empty() && query == crate::timeline::normalize(name)
}

/// Whether the "Create …" row should be offered: the query is non-empty and no
/// existing subject of the kind matches it exactly (product spec §5, §5.3).
pub fn show_create_row<'a>(query: &str, names: impl Iterator<Item = &'a str>) -> bool {
    !query.trim().is_empty() && !names.into_iter().any(|name| token_is_exact(query, name))
}

/// The next highlighted index in a picker list after pressing Up/Down, clamped
/// to the list. `None` means nothing is selected.
pub fn step_index(current: Option<usize>, count: usize, delta: i32) -> Option<usize> {
    if count == 0 {
        return None;
    }
    match current {
        Some(index) => Some((index as i32 + delta).clamp(0, count as i32 - 1) as usize),
        None if delta >= 0 => Some(0),
        None => Some(count - 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_only_offer_tags() {
        let note = sections_for(EntryType::Note);
        assert!(!note.ratings);
        assert!(!note.emotions);
        assert!(!note.activities);
        assert!(!note.colors);
        assert!(!note.subjects);
        assert!(note.tags);

        for entry_type in [EntryType::Journal, EntryType::Dream] {
            let sections = sections_for(entry_type);
            assert!(sections.ratings && sections.emotions && sections.activities);
            assert!(sections.colors && sections.subjects && sections.tags);
        }
    }

    #[test]
    fn mood_row_is_mood_on_waking_for_dreams() {
        assert_eq!(
            rating_title(Rating::Mood, EntryType::Dream),
            "Mood on waking"
        );
        assert_eq!(rating_title(Rating::Mood, EntryType::Journal), "Mood");
        assert_eq!(
            rating_title(Rating::Sleep, EntryType::Dream),
            "Sleep quality"
        );
    }

    #[test]
    fn rating_icons_and_labels_cover_five_values() {
        for rating in [
            Rating::Mood,
            Rating::Energy,
            Rating::CognitiveLoad,
            Rating::Sleep,
        ] {
            for value in 1..=5 {
                assert!(rating_label(rating, value).is_some());
                assert!(rating_icon(rating, value).is_some());
            }
            assert!(rating_label(rating, 0).is_none());
            assert!(rating_label(rating, 6).is_none());
            assert!(rating_icon(rating, 6).is_none());
        }
        assert_eq!(rating_icon(Rating::Mood, 5), Some("smiley-wink"));
        assert_eq!(rating_icon(Rating::Sleep, 5), Some("bed"));
    }

    #[test]
    fn token_matching_ignores_case_and_accents() {
        assert!(token_matches("caf", "Caffè"));
        assert!(token_matches("CAFFE", "Caffè"));
        assert!(token_matches("", "anything"));
        assert!(!token_matches("zzz", "Caffè"));

        assert!(token_is_exact("Caffe", "Caffè"));
        assert!(token_is_exact("  caffè ", "Caffè"));
        assert!(!token_is_exact("caf", "Caffè"));
        assert!(!token_is_exact("", ""));
    }

    #[test]
    fn create_row_is_hidden_on_an_exact_match() {
        let names = ["Marta", "Caffè"];
        assert!(show_create_row("Mar", names.iter().copied()));
        assert!(!show_create_row("marta", names.iter().copied()));
        assert!(!show_create_row("CAFFE", names.iter().copied()));
        assert!(!show_create_row("   ", names.iter().copied()));
    }

    fn choice(label: Option<&str>, custom: Option<&str>) -> Choice {
        Choice {
            id: uuid::Uuid::nil(),
            kind: quaderno_vault::ChoiceKind::Emotion,
            label: label.map(str::to_owned),
            custom_label: custom.map(str::to_owned),
            icon: "sun".into(),
            sort_order: 0,
            hidden: false,
            created_at: jiff::Timestamp::from_second(0).unwrap(),
            updated_at: jiff::Timestamp::from_second(0).unwrap(),
            deleted_at: None,
        }
    }

    #[test]
    fn choice_label_prefers_custom_then_translated_default() {
        assert_eq!(
            choice_display_label(&choice(Some("Calm"), Some("Sereno"))),
            "Sereno"
        );
        assert_eq!(choice_display_label(&choice(Some("Calm"), None)), "Calm");
        assert_eq!(choice_display_label(&choice(None, None)), "");
    }

    #[test]
    fn step_index_clamps_and_wraps_to_the_ends() {
        assert_eq!(step_index(None, 0, 1), None);
        assert_eq!(step_index(None, 3, 1), Some(0));
        assert_eq!(step_index(None, 3, -1), Some(2));
        assert_eq!(step_index(Some(0), 3, 1), Some(1));
        assert_eq!(step_index(Some(2), 3, 1), Some(2));
        assert_eq!(step_index(Some(0), 3, -1), Some(0));
        assert_eq!(step_index(Some(3), 3, 1), Some(2));
    }

    #[test]
    fn usage_detail_pluralizes_and_dates() {
        let last = quaderno_vault::time::parse_dated("2026-09-12T09:00:00+02:00").unwrap();
        assert_eq!(
            usage_detail(1, Some(&last)).as_deref(),
            Some("1 entry · last 12 Sep")
        );
        assert_eq!(
            usage_detail(14, Some(&last)).as_deref(),
            Some("14 entries · last 12 Sep")
        );
        assert_eq!(usage_detail(0, None), None);
    }
}
