// SPDX-License-Identifier: GPL-3.0-or-later

//! Entry API and entry-type rules (spec §5.1).

use quaderno_vault::{DreamFlag, EntryType, Rating, Vault, VaultError};
use tempfile::TempDir;

fn new_vault() -> (TempDir, Vault) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("journal.quaderno");
    let vault = Vault::create(&path, "passphrase").unwrap();
    (dir, vault)
}

fn dated(value: &str) -> jiff::Zoned {
    quaderno_vault::time::parse_dated(value).unwrap()
}

#[test]
fn create_read_update_entry() {
    let (_dir, mut vault) = new_vault();
    let id = vault
        .create_entry(
            EntryType::Journal,
            "# A slow Saturday\nWoke up late.",
            &dated("2026-09-28T21:14:00+02:00"),
        )
        .unwrap();

    let entry = vault.entry(id).unwrap();
    assert_eq!(entry.entry_type, EntryType::Journal);
    assert_eq!(entry.content, "# A slow Saturday\nWoke up late.");
    assert_eq!(
        quaderno_vault::text::title(&entry.content),
        "A slow Saturday"
    );
    assert_eq!(quaderno_vault::text::word_count(&entry.content), 7);
    assert_eq!(entry.dated_at, dated("2026-09-28T21:14:00+02:00"));
    assert!(entry.created_at <= entry.updated_at);

    let before = entry.updated_at;
    vault.set_entry_content(id, "changed").unwrap();
    let changed = vault.entry(id).unwrap();
    assert_eq!(changed.content, "changed");
    assert!(changed.updated_at >= before);
}

#[test]
fn content_is_stored_nfc_normalized() {
    let (_dir, mut vault) = new_vault();
    let id = vault
        .create_entry(
            EntryType::Journal,
            "caffe\u{0300}",
            &dated("2026-09-28T21:14:00+02:00"),
        )
        .unwrap();
    assert_eq!(vault.entry(id).unwrap().content, "caffè");
}

#[test]
fn ratings_work_on_journal_pages_but_not_notes() {
    let (_dir, mut vault) = new_vault();
    let journal = vault
        .create_entry(
            EntryType::Journal,
            "day",
            &dated("2026-09-28T21:14:00+02:00"),
        )
        .unwrap();
    vault
        .set_entry_rating(journal, Rating::Mood, Some(4))
        .unwrap();
    assert_eq!(vault.entry(journal).unwrap().mood, Some(4));
    vault.set_entry_rating(journal, Rating::Mood, None).unwrap();
    assert_eq!(vault.entry(journal).unwrap().mood, None);

    let note = vault
        .create_entry(
            EntryType::Note,
            "quick",
            &dated("2026-09-28T21:14:00+02:00"),
        )
        .unwrap();
    let error = vault
        .set_entry_rating(note, Rating::Mood, Some(3))
        .unwrap_err();
    assert!(matches!(error, VaultError::RuleViolation(_)));
}

#[test]
fn dream_flags_belong_to_dreams_only() {
    let (_dir, mut vault) = new_vault();
    let dream = vault
        .create_entry(
            EntryType::Dream,
            "flying",
            &dated("2026-09-28T06:55:00+02:00"),
        )
        .unwrap();
    vault
        .set_dream_flag(dream, DreamFlag::Lucid, Some(true))
        .unwrap();
    assert_eq!(vault.entry(dream).unwrap().lucid, Some(true));

    let journal = vault
        .create_entry(
            EntryType::Journal,
            "day",
            &dated("2026-09-28T21:14:00+02:00"),
        )
        .unwrap();
    let error = vault
        .set_dream_flag(journal, DreamFlag::Nightmare, Some(true))
        .unwrap_err();
    assert!(matches!(error, VaultError::RuleViolation(_)));
}

#[test]
fn soft_delete_then_restore() {
    let (_dir, mut vault) = new_vault();
    let id = vault
        .create_entry(
            EntryType::Journal,
            "day",
            &dated("2026-09-28T21:14:00+02:00"),
        )
        .unwrap();

    vault.soft_delete_entry(id).unwrap();
    assert!(matches!(vault.entry(id), Err(VaultError::NotFound)));
    assert!(vault.entries(false).unwrap().is_empty());
    assert_eq!(vault.entries(true).unwrap().len(), 1);

    vault.restore_entry(id).unwrap();
    assert_eq!(vault.entries(false).unwrap().len(), 1);
}
