// SPDX-License-Identifier: GPL-3.0-or-later

//! Passphrase handling and SQLCipher keying (`vault-spec/format.md` §2).

use std::ffi::c_int;

use rusqlite::Connection;
use zeroize::Zeroizing;

use crate::error::VaultError;
use crate::text::nfc;

/// Maximum passphrase length in bytes after NFC encoding (spec §2.2).
pub const MAX_PASSPHRASE_BYTES: usize = 1024;

/// NFC-normalizes `passphrase` and returns its UTF-8 bytes, which zeroize when
/// dropped (spec §2.2).
pub fn normalize_passphrase(passphrase: &str) -> Zeroizing<Vec<u8>> {
    Zeroizing::new(nfc(passphrase).into_bytes())
}

/// Keys `connection` and sets the SQLCipher parameters of spec §2.1.
///
/// The key must be the NFC-normalized UTF-8 passphrase. This must run before
/// any other statement on the connection.
pub fn key_connection(connection: &Connection, key: &[u8]) -> Result<(), VaultError> {
    if key.is_empty() || key.len() > MAX_PASSPHRASE_BYTES {
        return Err(VaultError::InvalidPassphrase);
    }

    // SAFETY: `connection` owns a live sqlite3 handle, no statement has run on
    // it yet, and `key` is valid for `key.len()` bytes. The spec requires
    // `sqlite3_key` rather than a `PRAGMA key` string (§2.2).
    let result = unsafe {
        rusqlite::ffi::sqlite3_key(connection.handle(), key.as_ptr().cast(), key.len() as c_int)
    };
    if result != rusqlite::ffi::SQLITE_OK {
        return Err(VaultError::Database(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(result),
            None,
        )));
    }

    connection.execute_batch(
        "PRAGMA cipher_page_size = 4096;
         PRAGMA cipher_kdf_algorithm = PBKDF2_HMAC_SHA512;
         PRAGMA kdf_iter = 256000;
         PRAGMA cipher_hmac_algorithm = HMAC_SHA512;
         PRAGMA cipher_plaintext_header_size = 0;
         PRAGMA foreign_keys = ON;",
    )?;
    Ok(())
}
