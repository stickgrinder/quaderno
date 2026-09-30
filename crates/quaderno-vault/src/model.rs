// SPDX-License-Identifier: GPL-3.0-or-later

//! Typed rows from the vault schema (`vault-spec/format.md` §4).

use jiff::{Timestamp, Zoned};
use uuid::Uuid;

/// The kind of an entry (spec §4.2). It cannot change after creation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryType {
    /// A journal page.
    Journal,
    /// A dream.
    Dream,
    /// A quick note.
    Note,
}

impl EntryType {
    /// The value stored in `entry.type`.
    pub fn as_str(self) -> &'static str {
        match self {
            EntryType::Journal => "journal",
            EntryType::Dream => "dream",
            EntryType::Note => "note",
        }
    }

    /// Parses a stored `entry.type`.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "journal" => Some(EntryType::Journal),
            "dream" => Some(EntryType::Dream),
            "note" => Some(EntryType::Note),
            _ => None,
        }
    }
}

/// One of the four rating scales (spec §4.2), each `1..=5` or unset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rating {
    /// How good the day felt (on a dream, mood on waking).
    Mood,
    /// Energy.
    Energy,
    /// Cognitive load.
    CognitiveLoad,
    /// Sleep quality.
    Sleep,
}

impl Rating {
    /// The column that stores this rating.
    pub fn column(self) -> &'static str {
        match self {
            Rating::Mood => "mood",
            Rating::Energy => "energy",
            Rating::CognitiveLoad => "cognitive_load",
            Rating::Sleep => "sleep",
        }
    }
}

/// One of the three dream flags (spec §4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DreamFlag {
    /// A lucid dream.
    Lucid,
    /// A nightmare.
    Nightmare,
    /// A recurring dream.
    Recurring,
}

impl DreamFlag {
    /// The column that stores this flag.
    pub fn column(self) -> &'static str {
        match self {
            DreamFlag::Lucid => "lucid",
            DreamFlag::Nightmare => "nightmare",
            DreamFlag::Recurring => "recurring",
        }
    }
}

/// A row of `entry`.
#[derive(Debug, Clone)]
pub struct Entry {
    /// The entry's id.
    pub id: Uuid,
    /// Journal page, dream or quick note.
    pub entry_type: EntryType,
    /// Markdown content, NFC-normalized.
    pub content: String,
    /// The date the entry is about.
    pub dated_at: Zoned,
    /// Mood rating (`1..=5`).
    pub mood: Option<u8>,
    /// Energy rating (`1..=5`).
    pub energy: Option<u8>,
    /// Cognitive-load rating (`1..=5`).
    pub cognitive_load: Option<u8>,
    /// Sleep rating (`1..=5`).
    pub sleep: Option<u8>,
    /// Dream flag.
    pub lucid: Option<bool>,
    /// Dream flag.
    pub nightmare: Option<bool>,
    /// Dream flag.
    pub recurring: Option<bool>,
    /// The quick note this entry was expanded from.
    pub source_id: Option<Uuid>,
    /// The entry this one is a sync-conflict copy of.
    pub conflict_of: Option<Uuid>,
    /// Creation time, UTC.
    pub created_at: Timestamp,
    /// Last edit time, UTC.
    pub updated_at: Timestamp,
    /// Soft-deletion time, UTC.
    pub deleted_at: Option<Timestamp>,
}

/// The kind of a configurable list item (spec §4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChoiceKind {
    /// An emotion.
    Emotion,
    /// A day activity.
    Activity,
}

impl ChoiceKind {
    /// The value stored in `choice.kind`.
    pub fn as_str(self) -> &'static str {
        match self {
            ChoiceKind::Emotion => "emotion",
            ChoiceKind::Activity => "activity",
        }
    }

    /// Parses a stored `choice.kind`.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "emotion" => Some(ChoiceKind::Emotion),
            "activity" => Some(ChoiceKind::Activity),
            _ => None,
        }
    }
}

/// A row of `choice`: an emotion or a day activity (spec §4.3).
#[derive(Debug, Clone)]
pub struct Choice {
    /// The item's id.
    pub id: Uuid,
    /// Emotion or activity.
    pub kind: ChoiceKind,
    /// For defaults: the frozen English label to translate. `None` for items
    /// the user created.
    pub label: Option<String>,
    /// The user's own name, shown as-is when set.
    pub custom_label: Option<String>,
    /// A Phosphor icon name.
    pub icon: String,
    /// Display order.
    pub sort_order: i64,
    /// Hidden items stay on past entries and in statistics.
    pub hidden: bool,
    /// Creation time, UTC.
    pub created_at: Timestamp,
    /// Last edit time, UTC.
    pub updated_at: Timestamp,
    /// Soft-deletion time, UTC.
    pub deleted_at: Option<Timestamp>,
}

impl Choice {
    /// The name to show: `custom_label` if set, otherwise the frozen `label`.
    pub fn display_label(&self) -> &str {
        self.custom_label
            .as_deref()
            .or(self.label.as_deref())
            .unwrap_or("")
    }
}

/// The kind of a subject (spec §4.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubjectKind {
    /// A person.
    Person,
    /// A place.
    Place,
    /// A thing.
    Thing,
    /// A tag.
    Tag,
}

impl SubjectKind {
    /// The value stored in `subject.kind`.
    pub fn as_str(self) -> &'static str {
        match self {
            SubjectKind::Person => "person",
            SubjectKind::Place => "place",
            SubjectKind::Thing => "thing",
            SubjectKind::Tag => "tag",
        }
    }

    /// Parses a stored `subject.kind`.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "person" => Some(SubjectKind::Person),
            "place" => Some(SubjectKind::Place),
            "thing" => Some(SubjectKind::Thing),
            "tag" => Some(SubjectKind::Tag),
            _ => None,
        }
    }
}

/// A row of `subject`: a person, place, thing or tag (spec §4.4).
#[derive(Debug, Clone)]
pub struct Subject {
    /// The subject's id.
    pub id: Uuid,
    /// Person, place, thing or tag.
    pub kind: SubjectKind,
    /// The name, NFC-normalized and trimmed.
    pub name: String,
    /// Creation time, UTC.
    pub created_at: Timestamp,
    /// Last edit time, UTC.
    pub updated_at: Timestamp,
    /// Soft-deletion time, UTC.
    pub deleted_at: Option<Timestamp>,
}

/// A subject together with how it is used, for autocomplete ordering
/// (product spec §5).
#[derive(Debug, Clone)]
pub struct SubjectUsage {
    /// The subject itself.
    pub subject: Subject,
    /// The number of live entries linked to it.
    pub entry_count: i64,
    /// The `dated_at` of the most recent live entry linked to it.
    pub last_used: Option<Zoned>,
}

/// The twelve fixed colours (spec Appendix C).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    /// Red.
    Red,
    /// Orange.
    Orange,
    /// Yellow.
    Yellow,
    /// Green.
    Green,
    /// Teal.
    Teal,
    /// Blue.
    Blue,
    /// Purple.
    Purple,
    /// Pink.
    Pink,
    /// Brown.
    Brown,
    /// Gray.
    Gray,
    /// Black.
    Black,
    /// White.
    White,
}

impl Color {
    /// All twelve colours, in the schema's order.
    pub const ALL: [Color; 12] = [
        Color::Red,
        Color::Orange,
        Color::Yellow,
        Color::Green,
        Color::Teal,
        Color::Blue,
        Color::Purple,
        Color::Pink,
        Color::Brown,
        Color::Gray,
        Color::Black,
        Color::White,
    ];

    /// The value stored in `entry_color.color`.
    pub fn as_str(self) -> &'static str {
        match self {
            Color::Red => "red",
            Color::Orange => "orange",
            Color::Yellow => "yellow",
            Color::Green => "green",
            Color::Teal => "teal",
            Color::Blue => "blue",
            Color::Purple => "purple",
            Color::Pink => "pink",
            Color::Brown => "brown",
            Color::Gray => "gray",
            Color::Black => "black",
            Color::White => "white",
        }
    }

    /// Parses a stored `entry_color.color`.
    pub fn parse(value: &str) -> Option<Self> {
        Color::ALL.into_iter().find(|color| color.as_str() == value)
    }
}
