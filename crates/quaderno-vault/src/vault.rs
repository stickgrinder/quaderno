// SPDX-License-Identifier: GPL-3.0-or-later

//! The [`Vault`] handle: creating, opening and closing a journal.

use std::path::Path;

use rusqlite::{Connection, params};
use uuid::Uuid;

use crate::crypto::{key_connection, normalize_passphrase};
use crate::error::VaultError;
use crate::{migrations, time};

/// How a vault may be used, from the compatibility rules of spec §7.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// Normal: this client knows the schema and may write.
    ReadWrite,
    /// The file is newer than this client's write support: read only.
    ReadOnly,
}

struct VaultRow {
    id: Uuid,
    schema_version: i64,
    read_compat: i64,
    write_compat: i64,
}

/// An open Quaderno vault.
///
/// Dropping it closes the database. The passphrase is never retained: it is
/// keyed into SQLCipher and the buffer is zeroized immediately.
pub struct Vault {
    connection: Connection,
    id: Uuid,
    schema_version: i64,
    access: Access,
}

impl Vault {
    /// Creates a new vault at `path`, runs the initial migration and leaves it
    /// open.
    pub fn create(path: impl AsRef<Path>, passphrase: &str) -> Result<Vault, VaultError> {
        let key = normalize_passphrase(passphrase);
        let mut connection = Connection::open(path)?;
        key_connection(&connection, &key)?;

        let id = Uuid::now_v7();
        let created_at = time::now();

        let transaction = connection.transaction()?;
        for (version, sql) in migrations::all() {
            transaction.execute_batch(sql)?;
            if *version == 1 {
                insert_vault_row(&transaction, id, created_at)?;
            }
        }
        transaction.commit()?;

        Ok(Vault {
            connection,
            id,
            schema_version: migrations::SCHEMA_VERSION,
            access: Access::ReadWrite,
        })
    }

    /// Opens an existing vault, applying known migrations when the file allows
    /// writing.
    pub fn open(path: impl AsRef<Path>, passphrase: &str) -> Result<Vault, VaultError> {
        let key = normalize_passphrase(passphrase);
        let connection = Connection::open(path)?;
        key_connection(&connection, &key)?;

        let row = read_vault_row(&connection)?;

        if migrations::SCHEMA_VERSION < row.read_compat {
            return Err(VaultError::Refused {
                required_read: row.read_compat,
                schema_version: row.schema_version,
            });
        }

        let access = if migrations::SCHEMA_VERSION < row.write_compat {
            Access::ReadOnly
        } else {
            Access::ReadWrite
        };

        if access == Access::ReadWrite && row.schema_version < migrations::SCHEMA_VERSION {
            apply_pending_migrations(&connection, row.schema_version)?;
        }

        let schema_version = read_vault_row(&connection)?.schema_version;
        Ok(Vault {
            connection,
            id: row.id,
            schema_version,
            access,
        })
    }

    /// The vault's stable journal id.
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// The schema version stored in the file.
    pub fn schema_version(&self) -> i64 {
        self.schema_version
    }

    /// The access mode the file allows.
    pub fn access(&self) -> Access {
        self.access
    }

    /// The open connection (crate-internal).
    pub(crate) fn connection(&self) -> &Connection {
        &self.connection
    }

    /// Returns the connection if this vault can be written to, or
    /// [`VaultError::ReadOnly`] otherwise.
    pub(crate) fn require_writable(&self) -> Result<&Connection, VaultError> {
        if self.access == Access::ReadWrite {
            Ok(&self.connection)
        } else {
            Err(VaultError::ReadOnly)
        }
    }
}

fn insert_vault_row(
    connection: &Connection,
    id: Uuid,
    created_at: jiff::Timestamp,
) -> Result<(), VaultError> {
    connection.execute(
        "INSERT INTO vault
            (singleton, id, format, schema_version, read_compat, write_compat, created_at)
         VALUES (1, ?1, 'quaderno', 1, 1, 1, ?2)",
        params![id.to_string(), time::format_timestamp(created_at)],
    )?;
    Ok(())
}

fn read_vault_row(connection: &Connection) -> Result<VaultRow, VaultError> {
    let result = connection.query_row(
        "SELECT id, schema_version, read_compat, write_compat FROM vault WHERE singleton = 1",
        [],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        },
    );

    match result {
        Ok((id, schema_version, read_compat, write_compat)) => {
            let id = Uuid::parse_str(&id)
                .map_err(|error| VaultError::Invalid(format!("bad vault id {id:?}: {error}")))?;
            Ok(VaultRow {
                id,
                schema_version,
                read_compat,
                write_compat,
            })
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            Err(VaultError::Invalid("the vault row is missing".into()))
        }
        Err(rusqlite::Error::SqliteFailure(error, _))
            if error.code == rusqlite::ErrorCode::NotADatabase =>
        {
            Err(VaultError::Invalid(
                "wrong passphrase, or not a Quaderno vault".into(),
            ))
        }
        Err(error) => Err(VaultError::Database(error)),
    }
}

fn apply_pending_migrations(connection: &Connection, from: i64) -> Result<(), VaultError> {
    let transaction = connection.unchecked_transaction()?;
    for (_, sql) in migrations::after(from) {
        transaction.execute_batch(sql)?;
    }
    transaction.commit()?;
    Ok(())
}
