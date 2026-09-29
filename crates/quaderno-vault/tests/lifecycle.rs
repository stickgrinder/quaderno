// SPDX-License-Identifier: GPL-3.0-or-later

//! Creating, opening and locking a vault.

use quaderno_vault::{Access, Vault, VaultError};
use tempfile::TempDir;

fn vault_path(dir: &TempDir) -> std::path::PathBuf {
    dir.path().join("journal.quaderno")
}

#[test]
fn create_then_open_round_trips() {
    let dir = TempDir::new().unwrap();
    let path = vault_path(&dir);

    let created = Vault::create(&path, "correct horse battery staple").unwrap();
    let id = created.id();
    assert_eq!(created.schema_version(), 1);
    assert_eq!(created.access(), Access::ReadWrite);
    drop(created);

    let opened = Vault::open(&path, "correct horse battery staple").unwrap();
    assert_eq!(opened.id(), id);
    assert_eq!(opened.access(), Access::ReadWrite);
}

#[test]
fn the_file_is_encrypted() {
    let dir = TempDir::new().unwrap();
    let path = vault_path(&dir);
    Vault::create(&path, "correct horse battery staple").unwrap();

    let bytes = std::fs::read(&path).unwrap();
    assert!(
        !bytes.starts_with(b"SQLite format 3\0"),
        "the file must not have a plaintext SQLite header"
    );
    assert!(
        !bytes.windows(6).any(|window| window == b"SQLite"),
        "no plaintext SQLite marker may appear in the file"
    );
}

#[test]
fn wrong_passphrase_is_rejected() {
    let dir = TempDir::new().unwrap();
    let path = vault_path(&dir);
    Vault::create(&path, "correct horse battery staple").unwrap();

    let result = Vault::open(&path, "wrong passphrase");
    assert!(matches!(result, Err(VaultError::Invalid(_))));
}

#[test]
fn passphrase_is_nfc_normalized() {
    let dir = TempDir::new().unwrap();
    let path = vault_path(&dir);

    // "caffè" written as e + combining grave accent.
    Vault::create(&path, "caffe\u{0300}").unwrap();
    // Opened with the precomposed code point.
    Vault::open(&path, "caffè").unwrap();
}
