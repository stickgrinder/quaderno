// SPDX-License-Identifier: GPL-3.0-or-later

//! The error type returned by the vault library.

use std::path::PathBuf;

/// Everything that can go wrong while working with a vault.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum VaultError {
    /// A SQLite or SQLCipher error.
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    /// A filesystem error, with the path it happened on.
    #[error("I/O error at {}: {source}", path.display())]
    Io {
        /// The file the operation was working on.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: std::io::Error,
    },

    /// The passphrase is empty, or longer than 1024 bytes after NFC encoding.
    #[error("the passphrase must be between 1 and 1024 bytes")]
    InvalidPassphrase,

    /// The vault was written by a newer client than this one (spec §7.2).
    #[error("this journal was changed by a newer version of Quaderno; update Quaderno to open it")]
    Refused {
        /// The lowest schema version this client would have to know.
        required_read: i64,
        /// The schema version of the file.
        schema_version: i64,
    },

    /// The file is not a readable Quaderno vault.
    #[error("invalid vault: {0}")]
    Invalid(String),

    /// No row with the requested id exists (or it is soft-deleted).
    #[error("not found")]
    NotFound,

    /// A write was rejected by a database trigger because of the entry-type
    /// rules (spec §5.1).
    #[error("the write breaks a rule for this entry type: {0}")]
    RuleViolation(String),
}

impl VaultError {
    /// Wraps a `rusqlite` error, turning the `rule_*` trigger messages into
    /// [`VaultError::RuleViolation`].
    pub fn from_sqlite(error: rusqlite::Error) -> Self {
        if let rusqlite::Error::SqliteFailure(_, Some(message)) = &error {
            if message.starts_with("rule_") {
                return VaultError::RuleViolation(message.clone());
            }
        }
        VaultError::Database(error)
    }

    /// Builds an [`VaultError::Io`] for `path`.
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        VaultError::Io {
            path: path.into(),
            source,
        }
    }
}
