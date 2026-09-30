// SPDX-License-Identifier: GPL-3.0-or-later

//! Choices (emotions, activities) and subjects (people, places, things, tags).
//! See `vault-spec/format.md` §4.3, §4.4, §5.3.

use rusqlite::{Row, params};
use uuid::Uuid;

use crate::error::VaultError;
use crate::model::{Choice, ChoiceKind, Subject, SubjectKind, SubjectUsage};
use crate::vault::Vault;
use crate::{text::nfc, time};

pub(crate) const CHOICE_COLUMNS: &str = "c.id, c.kind, c.label, c.custom_label, c.icon, \
     c.sort_order, c.hidden, c.created_at, c.updated_at, c.deleted_at";
pub(crate) const SUBJECT_COLUMNS: &str =
    "s.id, s.kind, s.name, s.created_at, s.updated_at, s.deleted_at";

impl Vault {
    // ---- choices ---------------------------------------------------------

    /// Creates a user-owned emotion or activity. `name` goes to
    /// `custom_label`; `label` stays `NULL` (spec §4.3).
    pub fn create_choice(
        &mut self,
        kind: ChoiceKind,
        name: &str,
        icon: &str,
    ) -> Result<Uuid, VaultError> {
        let name = normalized_name(name, "a label")?;
        let connection = self.require_writable()?;
        let sort_order: i64 = connection.query_row(
            "SELECT COALESCE(MAX(sort_order), 0) + 10 FROM choice WHERE kind = ?1",
            [kind.as_str()],
            |row| row.get(0),
        )?;
        let id = Uuid::now_v7();
        let now = time::format_timestamp(time::now());
        connection
            .execute(
                "INSERT INTO choice
                    (id, kind, label, custom_label, icon, sort_order, hidden, created_at, updated_at)
                 VALUES (?1, ?2, NULL, ?3, ?4, ?5, 0, ?6, ?6)",
                params![id.to_string(), kind.as_str(), name, nfc(icon), sort_order, now],
            )
            .map_err(VaultError::from_sqlite)?;
        Ok(id)
    }

    /// Returns a choice, or [`VaultError::NotFound`] if it is soft-deleted.
    pub fn choice(&self, id: Uuid) -> Result<Choice, VaultError> {
        let result = self.connection().query_row(
            &format!("SELECT {CHOICE_COLUMNS} FROM choice c WHERE c.id = ?1"),
            [id.to_string()],
            row_to_choice,
        );
        match result {
            Ok(choice) if choice.deleted_at.is_none() => Ok(choice),
            Ok(_) | Err(rusqlite::Error::QueryReturnedNoRows) => Err(VaultError::NotFound),
            Err(error) => Err(VaultError::from_sqlite(error)),
        }
    }

    /// Lists a kind's choices by `sort_order`. Hidden ones are included only
    /// when `include_hidden` is true.
    pub fn choices(
        &self,
        kind: ChoiceKind,
        include_hidden: bool,
    ) -> Result<Vec<Choice>, VaultError> {
        let hidden = if include_hidden { "" } else { "AND hidden = 0" };
        let sql = format!(
            "SELECT {CHOICE_COLUMNS} FROM choice c
             WHERE c.kind = ?1 AND c.deleted_at IS NULL {hidden}
             ORDER BY c.sort_order, c.id"
        );
        let mut statement = self.connection().prepare(&sql)?;
        let rows = statement.query_map([kind.as_str()], row_to_choice)?;
        let mut choices = Vec::new();
        for row in rows {
            choices.push(row?);
        }
        Ok(choices)
    }

    /// Sets a choice's `custom_label` (spec §4.3).
    pub fn rename_choice(&mut self, id: Uuid, name: &str) -> Result<(), VaultError> {
        let name = normalized_name(name, "a label")?;
        let connection = self.require_writable()?;
        let changed = connection.execute(
            "UPDATE choice SET custom_label = ?2, updated_at = ?3
             WHERE id = ?1 AND deleted_at IS NULL",
            params![id.to_string(), name, time::format_timestamp(time::now())],
        )?;
        found(changed)
    }

    /// Clears a default choice's `custom_label`, restoring the translated
    /// default name. User-created items cannot be reset.
    pub fn reset_choice_label(&mut self, id: Uuid) -> Result<(), VaultError> {
        let choice = self.choice(id)?;
        if choice.label.is_none() {
            return Err(VaultError::Invalid(
                "only default items have a name to reset".into(),
            ));
        }
        let connection = self.require_writable()?;
        let changed = connection.execute(
            "UPDATE choice SET custom_label = NULL, updated_at = ?2
             WHERE id = ?1 AND deleted_at IS NULL",
            params![id.to_string(), time::format_timestamp(time::now())],
        )?;
        found(changed)
    }

    /// Sets a choice's icon (a Phosphor name).
    pub fn set_choice_icon(&mut self, id: Uuid, icon: &str) -> Result<(), VaultError> {
        let connection = self.require_writable()?;
        let changed = connection.execute(
            "UPDATE choice SET icon = ?2, updated_at = ?3
             WHERE id = ?1 AND deleted_at IS NULL",
            params![
                id.to_string(),
                nfc(icon),
                time::format_timestamp(time::now())
            ],
        )?;
        found(changed)
    }

    /// Sets a choice's display order.
    pub fn set_choice_sort_order(&mut self, id: Uuid, sort_order: i64) -> Result<(), VaultError> {
        let connection = self.require_writable()?;
        let changed = connection.execute(
            "UPDATE choice SET sort_order = ?2, updated_at = ?3
             WHERE id = ?1 AND deleted_at IS NULL",
            params![
                id.to_string(),
                sort_order,
                time::format_timestamp(time::now())
            ],
        )?;
        found(changed)
    }

    /// Hides or shows a choice (spec §4.3).
    pub fn set_choice_hidden(&mut self, id: Uuid, hidden: bool) -> Result<(), VaultError> {
        let connection = self.require_writable()?;
        let changed = connection.execute(
            "UPDATE choice SET hidden = ?2, updated_at = ?3
             WHERE id = ?1 AND deleted_at IS NULL",
            params![
                id.to_string(),
                i64::from(hidden),
                time::format_timestamp(time::now())
            ],
        )?;
        found(changed)
    }

    /// Soft-deletes a choice that no entry uses.
    pub fn soft_delete_choice(&mut self, id: Uuid) -> Result<(), VaultError> {
        if self.choice_usage(id)? > 0 {
            return Err(VaultError::Invalid(
                "hide it instead, or remove it from its entries first".into(),
            ));
        }
        let connection = self.require_writable()?;
        let now = time::format_timestamp(time::now());
        let changed = connection.execute(
            "UPDATE choice SET deleted_at = ?2, updated_at = ?2
             WHERE id = ?1 AND deleted_at IS NULL",
            params![id.to_string(), now],
        )?;
        found(changed)
    }

    /// The number of live entries linked to a choice.
    pub fn choice_usage(&self, id: Uuid) -> Result<i64, VaultError> {
        Ok(self.connection().query_row(
            "SELECT COUNT(*) FROM entry_choice ec
             JOIN entry e ON e.id = ec.entry_id
             WHERE ec.choice_id = ?1 AND ec.deleted_at IS NULL AND e.deleted_at IS NULL",
            [id.to_string()],
            |row| row.get(0),
        )?)
    }

    // ---- subjects --------------------------------------------------------

    /// Creates a person, place, thing or tag. The name is NFC-normalized and
    /// trimmed.
    pub fn create_subject(&mut self, kind: SubjectKind, name: &str) -> Result<Uuid, VaultError> {
        let name = normalized_name(name, "a name")?;
        let connection = self.require_writable()?;
        let id = Uuid::now_v7();
        let now = time::format_timestamp(time::now());
        connection
            .execute(
                "INSERT INTO subject (id, kind, name, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?4)",
                params![id.to_string(), kind.as_str(), name, now],
            )
            .map_err(VaultError::from_sqlite)?;
        Ok(id)
    }

    /// Returns a subject, or [`VaultError::NotFound`] if it is soft-deleted.
    pub fn subject(&self, id: Uuid) -> Result<Subject, VaultError> {
        let result = self.connection().query_row(
            &format!("SELECT {SUBJECT_COLUMNS} FROM subject s WHERE s.id = ?1"),
            [id.to_string()],
            row_to_subject,
        );
        match result {
            Ok(subject) if subject.deleted_at.is_none() => Ok(subject),
            Ok(_) | Err(rusqlite::Error::QueryReturnedNoRows) => Err(VaultError::NotFound),
            Err(error) => Err(VaultError::from_sqlite(error)),
        }
    }

    /// Lists a kind's live subjects, case-insensitively by name.
    pub fn subjects(&self, kind: SubjectKind) -> Result<Vec<Subject>, VaultError> {
        let sql = format!(
            "SELECT {SUBJECT_COLUMNS} FROM subject s
             WHERE s.kind = ?1 AND s.deleted_at IS NULL
             ORDER BY s.name COLLATE NOCASE, s.id"
        );
        let mut statement = self.connection().prepare(&sql)?;
        let rows = statement.query_map([kind.as_str()], row_to_subject)?;
        let mut subjects = Vec::new();
        for row in rows {
            subjects.push(row?);
        }
        Ok(subjects)
    }

    /// Lists a kind's live subjects with their usage, ordered by how many live
    /// entries link to them (most used first), for autocomplete
    /// (product spec §5). Ties are ordered by name, then id.
    pub fn subjects_with_usage(&self, kind: SubjectKind) -> Result<Vec<SubjectUsage>, VaultError> {
        let sql = format!(
            "SELECT {SUBJECT_COLUMNS},
                    (SELECT COUNT(*) FROM entry_subject es
                       JOIN entry e ON e.id = es.entry_id
                      WHERE es.subject_id = s.id AND es.deleted_at IS NULL
                        AND e.deleted_at IS NULL),
                    (SELECT MAX(e.dated_at) FROM entry_subject es
                       JOIN entry e ON e.id = es.entry_id
                      WHERE es.subject_id = s.id AND es.deleted_at IS NULL
                        AND e.deleted_at IS NULL)
               FROM subject s
              WHERE s.kind = ?1 AND s.deleted_at IS NULL
              ORDER BY 7 DESC, s.name COLLATE NOCASE, s.id"
        );
        let mut statement = self.connection().prepare(&sql)?;
        let rows = statement.query_map([kind.as_str()], |row| {
            Ok((
                row_to_subject(row)?,
                row.get::<_, i64>(6)?,
                row.get::<_, Option<String>>(7)?,
            ))
        })?;
        let mut usages = Vec::new();
        for row in rows {
            let (subject, entry_count, last_used) = row?;
            usages.push(SubjectUsage {
                subject,
                entry_count,
                last_used: last_used.as_deref().map(parse_dated).transpose()?,
            });
        }
        Ok(usages)
    }

    /// Renames a subject (spec §4.4).
    pub fn rename_subject(&mut self, id: Uuid, name: &str) -> Result<(), VaultError> {
        let name = normalized_name(name, "a name")?;
        let connection = self.require_writable()?;
        let changed = connection.execute(
            "UPDATE subject SET name = ?2, updated_at = ?3
             WHERE id = ?1 AND deleted_at IS NULL",
            params![id.to_string(), name, time::format_timestamp(time::now())],
        )?;
        found(changed)
    }

    /// Merges `merge` into `keep` (spec §5.3): every active link to `merge`
    /// becomes a link to `keep`, then `merge` is soft-deleted. Both must be of
    /// the same kind.
    pub fn merge_subjects(&mut self, keep: Uuid, merge: Uuid) -> Result<(), VaultError> {
        if keep == merge {
            return Err(VaultError::Invalid(
                "cannot merge an item into itself".into(),
            ));
        }
        let keep_subject = self.subject(keep)?;
        let merge_subject = self.subject(merge)?;
        if keep_subject.kind != merge_subject.kind {
            return Err(VaultError::Invalid(
                "only items of the same kind can be merged".into(),
            ));
        }

        let connection = self.require_writable()?;
        let now = time::format_timestamp(time::now());
        let transaction = connection.unchecked_transaction()?;

        // Add the link to `keep` where it is missing.
        transaction.execute(
            "INSERT OR IGNORE INTO entry_subject (entry_id, subject_id, created_at, updated_at)
             SELECT entry_id, ?1, ?2, ?2 FROM entry_subject
             WHERE subject_id = ?3 AND deleted_at IS NULL",
            params![keep.to_string(), now, merge.to_string()],
        )?;
        // Restore a previously unlinked `keep` link for those entries.
        transaction.execute(
            "UPDATE entry_subject SET deleted_at = NULL, updated_at = ?2
             WHERE subject_id = ?1 AND deleted_at IS NOT NULL
               AND entry_id IN (
                   SELECT entry_id FROM entry_subject
                   WHERE subject_id = ?3 AND deleted_at IS NULL
               )",
            params![keep.to_string(), now, merge.to_string()],
        )?;
        // Unlink and delete the merged subject.
        transaction.execute(
            "UPDATE entry_subject SET deleted_at = ?2, updated_at = ?2
             WHERE subject_id = ?1 AND deleted_at IS NULL",
            params![merge.to_string(), now],
        )?;
        transaction.execute(
            "UPDATE subject SET deleted_at = ?2, updated_at = ?2
             WHERE id = ?1 AND deleted_at IS NULL",
            params![merge.to_string(), now],
        )?;

        transaction.commit()?;
        Ok(())
    }

    /// Soft-deletes a subject that no live entry uses.
    pub fn soft_delete_subject(&mut self, id: Uuid) -> Result<(), VaultError> {
        if self.subject_usage(id)? > 0 {
            return Err(VaultError::Invalid(
                "merge it into another item instead of deleting it".into(),
            ));
        }
        let connection = self.require_writable()?;
        let now = time::format_timestamp(time::now());
        let changed = connection.execute(
            "UPDATE subject SET deleted_at = ?2, updated_at = ?2
             WHERE id = ?1 AND deleted_at IS NULL",
            params![id.to_string(), now],
        )?;
        found(changed)
    }

    /// The number of live entries linked to a subject.
    pub fn subject_usage(&self, id: Uuid) -> Result<i64, VaultError> {
        Ok(self.connection().query_row(
            "SELECT COUNT(*) FROM entry_subject es
             JOIN entry e ON e.id = es.entry_id
             WHERE es.subject_id = ?1 AND es.deleted_at IS NULL AND e.deleted_at IS NULL",
            [id.to_string()],
            |row| row.get(0),
        )?)
    }
}

fn normalized_name(value: &str, what: &str) -> Result<String, VaultError> {
    let normalized = nfc(value);
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        return Err(VaultError::Invalid(format!("{what} cannot be empty")));
    }
    Ok(trimmed.to_string())
}

struct RawChoice {
    id: String,
    kind: String,
    label: Option<String>,
    custom_label: Option<String>,
    icon: String,
    sort_order: i64,
    hidden: i64,
    created_at: String,
    updated_at: String,
    deleted_at: Option<String>,
}

pub(crate) fn row_to_choice(row: &Row<'_>) -> rusqlite::Result<Choice> {
    let raw = RawChoice {
        id: row.get(0)?,
        kind: row.get(1)?,
        label: row.get(2)?,
        custom_label: row.get(3)?,
        icon: row.get(4)?,
        sort_order: row.get(5)?,
        hidden: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
        deleted_at: row.get(9)?,
    };
    Ok(Choice {
        id: Uuid::parse_str(&raw.id).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
        kind: ChoiceKind::parse(&raw.kind).unwrap_or(ChoiceKind::Emotion),
        label: raw.label,
        custom_label: raw.custom_label,
        icon: raw.icon,
        sort_order: raw.sort_order,
        hidden: raw.hidden != 0,
        created_at: parse_timestamp(&raw.created_at)?,
        updated_at: parse_timestamp(&raw.updated_at)?,
        deleted_at: raw.deleted_at.as_deref().map(parse_timestamp).transpose()?,
    })
}

struct RawSubject {
    id: String,
    kind: String,
    name: String,
    created_at: String,
    updated_at: String,
    deleted_at: Option<String>,
}

pub(crate) fn row_to_subject(row: &Row<'_>) -> rusqlite::Result<Subject> {
    let raw = RawSubject {
        id: row.get(0)?,
        kind: row.get(1)?,
        name: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
        deleted_at: row.get(5)?,
    };
    Ok(Subject {
        id: Uuid::parse_str(&raw.id).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
        kind: SubjectKind::parse(&raw.kind).unwrap_or(SubjectKind::Tag),
        name: raw.name,
        created_at: parse_timestamp(&raw.created_at)?,
        updated_at: parse_timestamp(&raw.updated_at)?,
        deleted_at: raw.deleted_at.as_deref().map(parse_timestamp).transpose()?,
    })
}

fn parse_timestamp(value: &str) -> rusqlite::Result<jiff::Timestamp> {
    value.parse().map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })
}

fn parse_dated(value: &str) -> rusqlite::Result<jiff::Zoned> {
    time::parse_dated(value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })
}

fn found(changed: usize) -> Result<(), VaultError> {
    if changed == 0 {
        Err(VaultError::NotFound)
    } else {
        Ok(())
    }
}
