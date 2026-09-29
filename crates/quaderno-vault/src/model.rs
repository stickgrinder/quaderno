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
