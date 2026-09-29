// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for every `rule_*` trigger and the compatibility matrix (spec §5.1,
//! §7.2). These run against a live connection so they can exercise triggers
//! that the typed API never reaches.

use tempfile::TempDir;
use uuid::Uuid;

use crate::error::VaultError;
use crate::model::{ChoiceKind, EntryType, SubjectKind};
use crate::time;
use crate::vault::{Access, Vault};

fn new_vault() -> (TempDir, Vault) {
    let dir = TempDir::new().unwrap();
    let vault = Vault::create(dir.path().join("journal.quaderno"), "passphrase").unwrap();
    (dir, vault)
}

fn dated(value: &str) -> jiff::Zoned {
    time::parse_dated(value).unwrap()
}

fn expect_rule(error: rusqlite::Error, name: &str) {
    match VaultError::from_sqlite(error) {
        VaultError::RuleViolation(message) => assert_eq!(message, name),
        other => panic!("expected {name}, got {other:?}"),
    }
}

fn entry(vault: &mut Vault, entry_type: EntryType) -> Uuid {
    vault
        .create_entry(entry_type, "text", &dated("2026-09-28T21:14:00+02:00"))
        .unwrap()
}

fn insert_entry(vault: &Vault, entry_type: &str, extra: &str, values: &str) -> rusqlite::Error {
    let id = Uuid::now_v7();
    let sql = format!(
        "INSERT INTO entry (id, type, content, dated_at, created_at, updated_at{extra})
         VALUES ('{id}', '{entry_type}', 'x', '2026-09-28T21:14:00+02:00', \
         '2026-09-28T00:00:00Z', '2026-09-28T00:00:00Z'{values})"
    );
    vault.connection().execute_batch(&sql).unwrap_err()
}

#[test]
fn entry_type_is_immutable() {
    let (_dir, mut vault) = new_vault();
    let id = entry(&mut vault, EntryType::Journal);
    let error = vault
        .connection()
        .execute(
            "UPDATE entry SET type = 'dream' WHERE id = ?1",
            [id.to_string()],
        )
        .unwrap_err();
    expect_rule(error, "rule_entry_type_immutable");
}

#[test]
fn dream_flags_are_rejected_on_insert_for_non_dreams() {
    let (_dir, vault) = new_vault();
    let error = insert_entry(&vault, "journal", ", lucid", ", 1");
    expect_rule(error, "rule_dream_flags");
}

#[test]
fn ratings_are_rejected_on_insert_for_notes() {
    let (_dir, vault) = new_vault();
    let error = insert_entry(&vault, "note", ", mood", ", 3");
    expect_rule(error, "rule_note_no_ratings");
}

#[test]
fn source_must_be_a_note_on_insert() {
    let (_dir, mut vault) = new_vault();
    let journal = entry(&mut vault, EntryType::Journal);
    let error = insert_entry(&vault, "journal", ", source_id", &format!(", '{journal}'"));
    expect_rule(error, "rule_source");
}

#[test]
fn source_must_be_a_note_on_update() {
    let (_dir, mut vault) = new_vault();
    let journal = entry(&mut vault, EntryType::Journal);
    let other = entry(&mut vault, EntryType::Journal);
    let error = vault
        .connection()
        .execute(
            "UPDATE entry SET source_id = ?2 WHERE id = ?1",
            [other.to_string(), journal.to_string()],
        )
        .unwrap_err();
    expect_rule(error, "rule_source");
}

#[test]
fn choice_links_are_rejected_on_notes() {
    let (_dir, mut vault) = new_vault();
    let note = entry(&mut vault, EntryType::Note);
    let choice = vault
        .create_choice(ChoiceKind::Emotion, "Calm", "flower-lotus")
        .unwrap();
    let error = vault.link_choice(note, choice).unwrap_err();
    assert!(matches!(error, VaultError::RuleViolation(_)));
}

#[test]
fn colour_links_are_rejected_on_notes() {
    let (_dir, mut vault) = new_vault();
    let note = entry(&mut vault, EntryType::Note);
    let error = vault
        .link_color(note, crate::model::Color::Green)
        .unwrap_err();
    assert!(matches!(error, VaultError::RuleViolation(_)));
}

#[test]
fn note_subject_links_must_be_tags() {
    let (_dir, mut vault) = new_vault();
    let note = entry(&mut vault, EntryType::Note);
    let person = vault.create_subject(SubjectKind::Person, "Marta").unwrap();
    let error = vault.link_subject(note, person).unwrap_err();
    assert!(matches!(error, VaultError::RuleViolation(_)));
}

#[test]
fn subject_kind_is_immutable() {
    let (_dir, mut vault) = new_vault();
    let person = vault.create_subject(SubjectKind::Person, "Marta").unwrap();
    let error = vault
        .connection()
        .execute(
            "UPDATE subject SET kind = 'tag' WHERE id = ?1",
            [person.to_string()],
        )
        .unwrap_err();
    expect_rule(error, "rule_subject_kind_immutable");
}

#[test]
fn default_choice_label_is_frozen() {
    let (_dir, vault) = new_vault();
    let calm = vault
        .choices(ChoiceKind::Emotion, true)
        .unwrap()
        .into_iter()
        .find(|choice| choice.display_label() == "Calm")
        .unwrap()
        .id;
    let error = vault
        .connection()
        .execute(
            "UPDATE choice SET label = 'Serene' WHERE id = ?1",
            [calm.to_string()],
        )
        .unwrap_err();
    expect_rule(error, "rule_choice_label_frozen");
}

#[test]
fn choice_kind_is_immutable() {
    let (_dir, vault) = new_vault();
    let calm = vault
        .choices(ChoiceKind::Emotion, true)
        .unwrap()
        .into_iter()
        .next()
        .unwrap()
        .id;
    let error = vault
        .connection()
        .execute(
            "UPDATE choice SET kind = 'activity' WHERE id = ?1",
            [calm.to_string()],
        )
        .unwrap_err();
    expect_rule(error, "rule_choice_kind_immutable");
}

// ---- compatibility matrix (spec §7.2) ------------------------------------

fn set_compat(vault: &Vault, schema: i64, read: i64, write: i64) {
    vault
        .connection()
        .execute(
            "UPDATE vault SET schema_version = ?1, read_compat = ?2, write_compat = ?3
             WHERE singleton = 1",
            rusqlite::params![schema, read, write],
        )
        .unwrap();
}

#[test]
fn a_newer_read_compat_is_refused() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("journal.quaderno");
    let vault = Vault::create(&path, "passphrase").unwrap();
    set_compat(&vault, 2, 2, 2);
    drop(vault);

    let result = Vault::open(&path, "passphrase");
    assert!(matches!(
        result,
        Err(VaultError::Refused {
            required_read: 2,
            schema_version: 2
        })
    ));
}

#[test]
fn a_newer_write_compat_is_read_only() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("journal.quaderno");
    let vault = Vault::create(&path, "passphrase").unwrap();
    set_compat(&vault, 2, 1, 2);
    drop(vault);

    let reopened = Vault::open(&path, "passphrase").unwrap();
    assert_eq!(reopened.access(), Access::ReadOnly);
    assert_eq!(reopened.schema_version(), 2);
}

#[test]
fn an_older_write_compat_is_writable() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("journal.quaderno");
    let vault = Vault::create(&path, "passphrase").unwrap();
    set_compat(&vault, 2, 1, 1);
    drop(vault);

    let reopened = Vault::open(&path, "passphrase").unwrap();
    assert_eq!(reopened.access(), Access::ReadWrite);
}
