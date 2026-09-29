// SPDX-License-Identifier: GPL-3.0-or-later

//! Reading and writing entries (`vault-spec/format.md` §4.2 and §5).

use jiff::{Timestamp, Zoned};
use rusqlite::{Row, params};
use uuid::Uuid;

use crate::error::VaultError;
use crate::model::{DreamFlag, Entry, EntryType, Rating};
use crate::vault::Vault;
use crate::{text::nfc, time};

const ENTRY_COLUMNS: &str = "id, type, content, dated_at, mood, energy, cognitive_load, \
     sleep, lucid, nightmare, recurring, source_id, conflict_of, created_at, updated_at, deleted_at";

impl Vault {
    /// Creates a journal page, dream or quick note and returns its id.
    ///
    /// `content` is stored NFC-normalized; `dated_at` is stored with its offset
    /// and truncated to whole seconds.
    pub fn create_entry(
        &mut self,
        entry_type: EntryType,
        content: &str,
        dated_at: &Zoned,
    ) -> Result<Uuid, VaultError> {
        let connection = self.require_writable()?;
        let id = Uuid::now_v7();
        let now = time::format_timestamp(time::now());
        connection
            .execute(
                "INSERT INTO entry (id, type, content, dated_at, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                params![
                    id.to_string(),
                    entry_type.as_str(),
                    nfc(content),
                    time::format_dated(dated_at),
                    now,
                ],
            )
            .map_err(VaultError::from_sqlite)?;
        Ok(id)
    }

    /// Returns an entry, or [`VaultError::NotFound`] if it is soft-deleted.
    pub fn entry(&self, id: Uuid) -> Result<Entry, VaultError> {
        let connection = self.connection();
        let result = connection.query_row(
            &format!("SELECT {ENTRY_COLUMNS} FROM entry WHERE id = ?1"),
            [id.to_string()],
            row_to_raw,
        );
        match result {
            Ok(raw) => {
                let entry = raw_to_entry(raw)?;
                if entry.deleted_at.is_some() {
                    Err(VaultError::NotFound)
                } else {
                    Ok(entry)
                }
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Err(VaultError::NotFound),
            Err(error) => Err(VaultError::from_sqlite(error)),
        }
    }

    /// Lists entries, newest first. Soft-deleted rows are included only when
    /// `include_deleted` is true.
    pub fn entries(&self, include_deleted: bool) -> Result<Vec<Entry>, VaultError> {
        let connection = self.connection();
        let filter = if include_deleted {
            ""
        } else {
            "WHERE deleted_at IS NULL"
        };
        let sql =
            format!("SELECT {ENTRY_COLUMNS} FROM entry {filter} ORDER BY dated_at DESC, id DESC");
        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map([], row_to_raw)?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(raw_to_entry(row?)?);
        }
        Ok(entries)
    }

    /// Replaces an entry's content.
    pub fn set_entry_content(&mut self, id: Uuid, content: &str) -> Result<(), VaultError> {
        let connection = self.require_writable()?;
        let changed = connection
            .execute(
                "UPDATE entry SET content = ?2, updated_at = ?3
                 WHERE id = ?1 AND deleted_at IS NULL",
                params![
                    id.to_string(),
                    nfc(content),
                    time::format_timestamp(time::now())
                ],
            )
            .map_err(VaultError::from_sqlite)?;
        found(changed)
    }

    /// Changes an entry's `dated_at`, keeping its offset and time of day to
    /// whatever the caller passes (spec §5.5).
    pub fn set_entry_dated_at(&mut self, id: Uuid, dated_at: &Zoned) -> Result<(), VaultError> {
        let connection = self.require_writable()?;
        let changed = connection
            .execute(
                "UPDATE entry SET dated_at = ?2, updated_at = ?3
                 WHERE id = ?1 AND deleted_at IS NULL",
                params![
                    id.to_string(),
                    time::format_dated(dated_at),
                    time::format_timestamp(time::now())
                ],
            )
            .map_err(VaultError::from_sqlite)?;
        found(changed)
    }

    /// Sets or clears a rating. The database rejects ratings on quick notes.
    pub fn set_entry_rating(
        &mut self,
        id: Uuid,
        rating: Rating,
        value: Option<u8>,
    ) -> Result<(), VaultError> {
        let connection = self.require_writable()?;
        let changed = connection
            .execute(
                &format!(
                    "UPDATE entry SET {} = ?2, updated_at = ?3
                     WHERE id = ?1 AND deleted_at IS NULL",
                    rating.column()
                ),
                params![
                    id.to_string(),
                    value.map(i64::from),
                    time::format_timestamp(time::now())
                ],
            )
            .map_err(VaultError::from_sqlite)?;
        found(changed)
    }

    /// Sets or clears a dream flag. The database rejects flags on non-dreams.
    pub fn set_dream_flag(
        &mut self,
        id: Uuid,
        flag: DreamFlag,
        value: Option<bool>,
    ) -> Result<(), VaultError> {
        let connection = self.require_writable()?;
        let changed = connection
            .execute(
                &format!(
                    "UPDATE entry SET {} = ?2, updated_at = ?3
                     WHERE id = ?1 AND deleted_at IS NULL",
                    flag.column()
                ),
                params![
                    id.to_string(),
                    value.map(i64::from),
                    time::format_timestamp(time::now())
                ],
            )
            .map_err(VaultError::from_sqlite)?;
        found(changed)
    }

    /// Soft-deletes an entry (spec §3).
    pub fn soft_delete_entry(&mut self, id: Uuid) -> Result<(), VaultError> {
        let connection = self.require_writable()?;
        let now = time::format_timestamp(time::now());
        let changed = connection
            .execute(
                "UPDATE entry SET deleted_at = ?2, updated_at = ?2
                 WHERE id = ?1 AND deleted_at IS NULL",
                params![id.to_string(), now],
            )
            .map_err(VaultError::from_sqlite)?;
        found(changed)
    }

    /// Restores a soft-deleted entry.
    pub fn restore_entry(&mut self, id: Uuid) -> Result<(), VaultError> {
        let connection = self.require_writable()?;
        let changed = connection
            .execute(
                "UPDATE entry SET deleted_at = NULL, updated_at = ?2
                 WHERE id = ?1 AND deleted_at IS NOT NULL",
                params![id.to_string(), time::format_timestamp(time::now())],
            )
            .map_err(VaultError::from_sqlite)?;
        found(changed)
    }

    /// The last-update time of an entry: the newest `updated_at` among the
    /// entry and its link rows (spec §5.6).
    pub fn entry_last_updated(&self, id: Uuid) -> Result<Timestamp, VaultError> {
        let value: Option<String> = self.connection().query_row(
            "SELECT MAX(updated_at) FROM (
                SELECT updated_at FROM entry WHERE id = ?1
                UNION ALL SELECT updated_at FROM entry_choice WHERE entry_id = ?1
                UNION ALL SELECT updated_at FROM entry_subject WHERE entry_id = ?1
                UNION ALL SELECT updated_at FROM entry_color WHERE entry_id = ?1
             )",
            [id.to_string()],
            |row| row.get(0),
        )?;
        let value = value.ok_or(VaultError::NotFound)?;
        time::parse_timestamp(&value)
    }
}

fn found(changed: usize) -> Result<(), VaultError> {
    if changed == 0 {
        Err(VaultError::NotFound)
    } else {
        Ok(())
    }
}

struct RawEntry {
    id: String,
    entry_type: String,
    content: String,
    dated_at: String,
    mood: Option<i64>,
    energy: Option<i64>,
    cognitive_load: Option<i64>,
    sleep: Option<i64>,
    lucid: Option<i64>,
    nightmare: Option<i64>,
    recurring: Option<i64>,
    source_id: Option<String>,
    conflict_of: Option<String>,
    created_at: String,
    updated_at: String,
    deleted_at: Option<String>,
}

fn row_to_raw(row: &Row<'_>) -> rusqlite::Result<RawEntry> {
    Ok(RawEntry {
        id: row.get(0)?,
        entry_type: row.get(1)?,
        content: row.get(2)?,
        dated_at: row.get(3)?,
        mood: row.get(4)?,
        energy: row.get(5)?,
        cognitive_load: row.get(6)?,
        sleep: row.get(7)?,
        lucid: row.get(8)?,
        nightmare: row.get(9)?,
        recurring: row.get(10)?,
        source_id: row.get(11)?,
        conflict_of: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
        deleted_at: row.get(15)?,
    })
}

fn raw_to_entry(raw: RawEntry) -> Result<Entry, VaultError> {
    Ok(Entry {
        id: parse_uuid(raw.id)?,
        entry_type: EntryType::parse(&raw.entry_type)
            .ok_or_else(|| VaultError::Invalid(format!("bad entry type {:?}", raw.entry_type)))?,
        content: raw.content,
        dated_at: time::parse_dated(&raw.dated_at)?,
        mood: parse_rating(raw.mood)?,
        energy: parse_rating(raw.energy)?,
        cognitive_load: parse_rating(raw.cognitive_load)?,
        sleep: parse_rating(raw.sleep)?,
        lucid: parse_flag(raw.lucid)?,
        nightmare: parse_flag(raw.nightmare)?,
        recurring: parse_flag(raw.recurring)?,
        source_id: raw.source_id.map(parse_uuid).transpose()?,
        conflict_of: raw.conflict_of.map(parse_uuid).transpose()?,
        created_at: time::parse_timestamp(&raw.created_at)?,
        updated_at: time::parse_timestamp(&raw.updated_at)?,
        deleted_at: raw
            .deleted_at
            .map(|value| time::parse_timestamp(&value))
            .transpose()?,
    })
}

fn parse_uuid(value: String) -> Result<Uuid, VaultError> {
    Uuid::parse_str(&value)
        .map_err(|error| VaultError::Invalid(format!("bad id {value:?}: {error}")))
}

fn parse_rating(value: Option<i64>) -> Result<Option<u8>, VaultError> {
    value
        .map(|value| {
            u8::try_from(value)
                .map_err(|_| VaultError::Invalid(format!("bad rating value {value}")))
        })
        .transpose()
}

fn parse_flag(value: Option<i64>) -> Result<Option<bool>, VaultError> {
    value
        .map(|value| match value {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(VaultError::Invalid(format!("bad flag value {value}"))),
        })
        .transpose()
}
