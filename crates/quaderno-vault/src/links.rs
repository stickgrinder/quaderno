// SPDX-License-Identifier: GPL-3.0-or-later

//! Links from entries to choices, subjects and colours, and expanding quick
//! notes. See `vault-spec/format.md` §4.5 and §5.2.

use rusqlite::params;
use uuid::Uuid;

use crate::collections::{CHOICE_COLUMNS, SUBJECT_COLUMNS, row_to_choice, row_to_subject};
use crate::error::VaultError;
use crate::model::{Choice, Color, EntryType, Subject, SubjectKind};
use crate::time;
use crate::vault::Vault;

impl Vault {
    // ---- choice links ----------------------------------------------------

    /// Links a choice to an entry, restoring the link if it existed.
    pub fn link_choice(&mut self, entry: Uuid, choice: Uuid) -> Result<(), VaultError> {
        let connection = self.require_writable()?;
        let now = time::format_timestamp(time::now());
        connection
            .execute(
                "INSERT INTO entry_choice (entry_id, choice_id, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?3)
                 ON CONFLICT(entry_id, choice_id)
                 DO UPDATE SET deleted_at = NULL, updated_at = excluded.updated_at",
                params![entry.to_string(), choice.to_string(), now],
            )
            .map_err(VaultError::from_sqlite)?;
        Ok(())
    }

    /// Unlinks a choice from an entry.
    pub fn unlink_choice(&mut self, entry: Uuid, choice: Uuid) -> Result<(), VaultError> {
        let connection = self.require_writable()?;
        let now = time::format_timestamp(time::now());
        let changed = connection.execute(
            "UPDATE entry_choice SET deleted_at = ?3, updated_at = ?3
             WHERE entry_id = ?1 AND choice_id = ?2 AND deleted_at IS NULL",
            params![entry.to_string(), choice.to_string(), now],
        )?;
        found(changed)
    }

    /// The live choices linked to an entry, by `sort_order`.
    pub fn entry_choices(&self, entry: Uuid) -> Result<Vec<Choice>, VaultError> {
        let sql = format!(
            "SELECT {CHOICE_COLUMNS} FROM choice c
             JOIN entry_choice ec ON ec.choice_id = c.id
             WHERE ec.entry_id = ?1 AND ec.deleted_at IS NULL AND c.deleted_at IS NULL
             ORDER BY c.sort_order, c.id"
        );
        let mut statement = self.connection().prepare(&sql)?;
        let rows = statement.query_map([entry.to_string()], row_to_choice)?;
        let mut choices = Vec::new();
        for row in rows {
            choices.push(row?);
        }
        Ok(choices)
    }

    // ---- subject links ---------------------------------------------------

    /// Links a subject to an entry, restoring the link if it existed.
    pub fn link_subject(&mut self, entry: Uuid, subject: Uuid) -> Result<(), VaultError> {
        let connection = self.require_writable()?;
        let now = time::format_timestamp(time::now());
        connection
            .execute(
                "INSERT INTO entry_subject (entry_id, subject_id, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?3)
                 ON CONFLICT(entry_id, subject_id)
                 DO UPDATE SET deleted_at = NULL, updated_at = excluded.updated_at",
                params![entry.to_string(), subject.to_string(), now],
            )
            .map_err(VaultError::from_sqlite)?;
        Ok(())
    }

    /// Unlinks a subject from an entry.
    pub fn unlink_subject(&mut self, entry: Uuid, subject: Uuid) -> Result<(), VaultError> {
        let connection = self.require_writable()?;
        let now = time::format_timestamp(time::now());
        let changed = connection.execute(
            "UPDATE entry_subject SET deleted_at = ?3, updated_at = ?3
             WHERE entry_id = ?1 AND subject_id = ?2 AND deleted_at IS NULL",
            params![entry.to_string(), subject.to_string(), now],
        )?;
        found(changed)
    }

    /// The live subjects linked to an entry, optionally filtered by kind.
    pub fn entry_subjects(
        &self,
        entry: Uuid,
        kind: Option<SubjectKind>,
    ) -> Result<Vec<Subject>, VaultError> {
        let filter = if kind.is_some() {
            "AND s.kind = ?2"
        } else {
            ""
        };
        let sql = format!(
            "SELECT {SUBJECT_COLUMNS} FROM subject s
             JOIN entry_subject es ON es.subject_id = s.id
             WHERE es.entry_id = ?1 AND es.deleted_at IS NULL AND s.deleted_at IS NULL {filter}
             ORDER BY s.kind, s.name COLLATE NOCASE, s.id"
        );
        let mut statement = self.connection().prepare(&sql)?;
        let mut subjects = Vec::new();
        match kind {
            Some(kind) => {
                let rows = statement
                    .query_map(params![entry.to_string(), kind.as_str()], row_to_subject)?;
                for row in rows {
                    subjects.push(row?);
                }
            }
            None => {
                let rows = statement.query_map([entry.to_string()], row_to_subject)?;
                for row in rows {
                    subjects.push(row?);
                }
            }
        }
        Ok(subjects)
    }

    // ---- colour links ----------------------------------------------------

    /// Links a colour to an entry.
    pub fn link_color(&mut self, entry: Uuid, color: Color) -> Result<(), VaultError> {
        let connection = self.require_writable()?;
        let now = time::format_timestamp(time::now());
        connection
            .execute(
                "INSERT INTO entry_color (entry_id, color, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?3)
                 ON CONFLICT(entry_id, color)
                 DO UPDATE SET deleted_at = NULL, updated_at = excluded.updated_at",
                params![entry.to_string(), color.as_str(), now],
            )
            .map_err(VaultError::from_sqlite)?;
        Ok(())
    }

    /// Unlinks a colour from an entry.
    pub fn unlink_color(&mut self, entry: Uuid, color: Color) -> Result<(), VaultError> {
        let connection = self.require_writable()?;
        let now = time::format_timestamp(time::now());
        let changed = connection.execute(
            "UPDATE entry_color SET deleted_at = ?3, updated_at = ?3
             WHERE entry_id = ?1 AND color = ?2 AND deleted_at IS NULL",
            params![entry.to_string(), color.as_str(), now],
        )?;
        found(changed)
    }

    /// The live colours linked to an entry, in the schema's order.
    pub fn entry_colors(&self, entry: Uuid) -> Result<Vec<Color>, VaultError> {
        let mut statement = self.connection().prepare(
            "SELECT color FROM entry_color
             WHERE entry_id = ?1 AND deleted_at IS NULL
             ORDER BY color",
        )?;
        let rows = statement.query_map([entry.to_string()], |row| row.get::<_, String>(0))?;
        let mut colors = Vec::new();
        for row in rows {
            let value = row?;
            if let Some(color) = Color::parse(&value) {
                colors.push(color);
            }
        }
        Ok(colors)
    }

    // ---- expanding notes -------------------------------------------------

    /// Expands a quick note into a journal page or dream (spec §5.2): a new
    /// entry pointing at the note via `source_id`, with the note's text and
    /// tags copied. The note itself is unchanged.
    pub fn expand_note(&mut self, note_id: Uuid, target: EntryType) -> Result<Uuid, VaultError> {
        if target == EntryType::Note {
            return Err(VaultError::Invalid(
                "a quick note cannot be expanded into another note".into(),
            ));
        }
        let note = self.entry(note_id)?;
        if note.entry_type != EntryType::Note {
            return Err(VaultError::Invalid(
                "only quick notes can be expanded".into(),
            ));
        }

        let dated_at = match target {
            EntryType::Dream => note.dated_at.clone(),
            _ => jiff::Zoned::now(),
        };
        let new_id = self.create_entry(target, &note.content, &dated_at)?;

        let connection = self.require_writable()?;
        let now = time::format_timestamp(time::now());
        // Copy the note's tags (quick notes may only have tags).
        connection.execute(
            "INSERT INTO entry_subject (entry_id, subject_id, created_at, updated_at)
             SELECT ?1, subject_id, ?2, ?2 FROM entry_subject
             WHERE entry_id = ?3 AND deleted_at IS NULL
               AND subject_id IN (SELECT id FROM subject WHERE kind = 'tag')",
            params![new_id.to_string(), now, note_id.to_string()],
        )?;
        connection.execute(
            "UPDATE entry SET source_id = ?2, updated_at = ?3 WHERE id = ?1",
            params![new_id.to_string(), note_id.to_string(), now],
        )?;
        Ok(new_id)
    }

    /// The live entries expanded from a quick note.
    pub fn expanded_entries(&self, note_id: Uuid) -> Result<Vec<Uuid>, VaultError> {
        let mut statement = self.connection().prepare(
            "SELECT id FROM entry
             WHERE source_id = ?1 AND deleted_at IS NULL
             ORDER BY dated_at, id",
        )?;
        let rows = statement.query_map([note_id.to_string()], |row| row.get::<_, String>(0))?;
        let mut ids = Vec::new();
        for row in rows {
            let value = row?;
            let id = Uuid::parse_str(&value)
                .map_err(|error| VaultError::Invalid(format!("bad id {value:?}: {error}")))?;
            ids.push(id);
        }
        Ok(ids)
    }
}

fn found(changed: usize) -> Result<(), VaultError> {
    if changed == 0 {
        Err(VaultError::NotFound)
    } else {
        Ok(())
    }
}
