// SPDX-License-Identifier: GPL-3.0-or-later

//! Snapshots, validation and passphrase changes (spec §2.4, §8.1, §8.3).

use quaderno_vault::{EntryType, Vault, VaultError};
use tempfile::TempDir;

fn new_vault(dir: &TempDir) -> std::path::PathBuf {
    let path = dir.path().join("journal.quaderno");
    Vault::create(&path, "old passphrase").unwrap();
    path
}

fn dated(value: &str) -> jiff::Zoned {
    quaderno_vault::time::parse_dated(value).unwrap()
}

#[test]
fn backup_is_a_consistent_openable_copy() {
    let dir = TempDir::new().unwrap();
    let path = new_vault(&dir);
    let mut vault = Vault::open(&path, "old passphrase").unwrap();
    let entry = vault
        .create_entry(
            EntryType::Journal,
            "day",
            &dated("2026-09-28T21:14:00+02:00"),
        )
        .unwrap();
    let id = vault.id();

    let backup_path = dir.path().join("backup.quaderno");
    vault.backup(&backup_path, "old passphrase").unwrap();
    drop(vault);

    let backup = Vault::open(&backup_path, "old passphrase").unwrap();
    assert_eq!(backup.id(), id);
    assert_eq!(backup.entry(entry).unwrap().content, "day");
}

#[test]
fn validate_accepts_a_good_file_and_rejects_a_wrong_passphrase() {
    let dir = TempDir::new().unwrap();
    let path = new_vault(&dir);
    Vault::validate(&path, "old passphrase").unwrap();
    assert!(matches!(
        Vault::validate(&path, "wrong"),
        Err(VaultError::Invalid(_))
    ));
}

#[test]
fn change_passphrase_keeps_the_data_and_switches_the_key() {
    let dir = TempDir::new().unwrap();
    let path = new_vault(&dir);
    let mut vault = Vault::open(&path, "old passphrase").unwrap();
    let entry = vault
        .create_entry(
            EntryType::Dream,
            "flying",
            &dated("2026-09-27T06:55:00+02:00"),
        )
        .unwrap();
    let id = vault.id();
    drop(vault);

    Vault::change_passphrase(&path, "old passphrase", "new passphrase").unwrap();

    assert!(matches!(
        Vault::open(&path, "old passphrase"),
        Err(VaultError::Invalid(_))
    ));
    let reopened = Vault::open(&path, "new passphrase").unwrap();
    assert_eq!(reopened.id(), id);
    assert_eq!(reopened.entry(entry).unwrap().content, "flying");
}

#[test]
fn change_passphrase_normalizes_the_new_passphrase() {
    let dir = TempDir::new().unwrap();
    let path = new_vault(&dir);

    Vault::change_passphrase(&path, "old passphrase", "caffe\u{0300}").unwrap();
    Vault::open(&path, "caffè").unwrap();
}
