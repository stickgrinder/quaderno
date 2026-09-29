// SPDX-License-Identifier: GPL-3.0-or-later

//! Snapshots, passphrase changes and validation. See `vault-spec/format.md`
//! §2.4, §8.1 and §8.3.

use std::ffi::c_int;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::Connection;
use rusqlite::backup::Backup;
use uuid::Uuid;

use crate::crypto::{MAX_PASSPHRASE_BYTES, key_connection, normalize_passphrase};
use crate::error::VaultError;
use crate::vault::Vault;

impl Vault {
    /// Writes a consistent encrypted copy of the open vault to `dest`, keyed
    /// with `passphrase` (spec §8.1). Used for local backups and, from 1.1, for
    /// the published sync copy.
    pub fn backup(&self, dest: impl AsRef<Path>, passphrase: &str) -> Result<(), VaultError> {
        let dest = dest.as_ref();
        let key = normalize_passphrase(passphrase);
        let mut destination = Connection::open(dest)?;
        key_connection(&destination, &key)?;

        let backup = Backup::new(self.connection(), &mut destination)?;
        backup.run_to_completion(64, Duration::from_millis(5), None)?;
        Ok(())
    }

    /// Checks a vault file without keeping it open (spec §8.3): it must open
    /// with the passphrase, pass `integrity_check` and `foreign_key_check`.
    pub fn validate(path: impl AsRef<Path>, passphrase: &str) -> Result<(), VaultError> {
        let vault = Vault::open(path, passphrase)?;
        vault.check_integrity()
    }

    /// Runs `integrity_check` and `foreign_key_check` on the open vault.
    pub fn check_integrity(&self) -> Result<(), VaultError> {
        let integrity: String =
            self.connection()
                .query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        if integrity != "ok" {
            return Err(VaultError::Invalid(format!(
                "integrity check failed: {integrity}"
            )));
        }

        let mut statement = self.connection().prepare("PRAGMA foreign_key_check")?;
        let mut rows = statement.query([])?;
        if let Some(row) = rows.next()? {
            let table: String = row.get(0).unwrap_or_default();
            return Err(VaultError::Invalid(format!(
                "foreign key violation in {table}"
            )));
        }
        Ok(())
    }

    /// Changes the passphrase using the safe sequence of spec §2.4: copy,
    /// rekey the copy, validate it, then atomically replace the original.
    pub fn change_passphrase(
        path: impl AsRef<Path>,
        old_passphrase: &str,
        new_passphrase: &str,
    ) -> Result<(), VaultError> {
        let path = path.as_ref();
        let directory = path.parent().unwrap_or_else(|| Path::new("."));
        let temporary = temporary_path(directory);

        let result = (|| {
            {
                let vault = Vault::open(path, old_passphrase)?;
                vault.backup(&temporary, old_passphrase)?;
            }
            rekey_file(&temporary, old_passphrase, new_passphrase)?;
            Vault::validate(&temporary, new_passphrase)?;
            fs::rename(&temporary, path).map_err(|error| VaultError::io(path, error))?;
            Ok(())
        })();

        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}

/// Rekeys `path` from `old_passphrase` to `new_passphrase`.
fn rekey_file(path: &Path, old_passphrase: &str, new_passphrase: &str) -> Result<(), VaultError> {
    let old_key = normalize_passphrase(old_passphrase);
    let new_key = normalize_passphrase(new_passphrase);
    if new_key.is_empty() || new_key.len() > MAX_PASSPHRASE_BYTES {
        return Err(VaultError::InvalidPassphrase);
    }

    let connection = Connection::open(path)?;
    key_connection(&connection, &old_key)?;
    // Read page 1 so SQLCipher derives the key before it is changed.
    let _: i64 = connection.query_row("PRAGMA schema_version", [], |row| row.get(0))?;

    // SAFETY: `connection` owns a live, keyed sqlite3 handle, and `new_key` is
    // valid for `new_key.len()` bytes. The spec changes the passphrase on the
    // copy, so a byte key (not a `PRAGMA rekey` string) is appropriate (§2.4).
    let result = unsafe {
        rusqlite::ffi::sqlite3_rekey(
            connection.handle(),
            new_key.as_ptr().cast(),
            new_key.len() as c_int,
        )
    };
    if result != rusqlite::ffi::SQLITE_OK {
        return Err(VaultError::Database(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(result),
            None,
        )));
    }
    Ok(())
}

fn temporary_path(directory: &Path) -> PathBuf {
    directory.join(format!(".quaderno-rekey-{}.quaderno", Uuid::now_v7()))
}
