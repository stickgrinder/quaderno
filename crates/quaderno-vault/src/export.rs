// SPDX-License-Identifier: GPL-3.0-or-later

//! JSON export of the whole vault, in the shape of the product spec §10:
//! a top-level object keyed by table name, each an array of rows with column
//! names as keys. Used by the test fixture and, later, by the app's export.

use rusqlite::types::ValueRef;
use rusqlite::{Connection, Row};

use crate::error::VaultError;
use crate::vault::Vault;

/// Tables in schema order, with the columns that order their rows.
const TABLES: [(&str, &str); 8] = [
    ("vault", "singleton"),
    ("entry", "id"),
    ("choice", "id"),
    ("subject", "id"),
    ("entry_choice", "entry_id, choice_id"),
    ("entry_subject", "entry_id, subject_id"),
    ("entry_color", "entry_id, color"),
    ("setting", "key"),
];

impl Vault {
    /// Serializes every table as pretty JSON.
    pub fn export_json(&self) -> Result<String, VaultError> {
        let mut root = serde_json::Map::new();
        for (table, order) in TABLES {
            root.insert(
                table.to_string(),
                export_table(self.connection(), table, order)?,
            );
        }
        serde_json::to_string_pretty(&serde_json::Value::Object(root))
            .map_err(|error| VaultError::Invalid(format!("cannot serialize JSON: {error}")))
    }
}

fn export_table(
    connection: &Connection,
    table: &str,
    order: &str,
) -> Result<serde_json::Value, VaultError> {
    let columns = table_columns(connection, table)?;
    let sql = format!("SELECT * FROM {table} ORDER BY {order}");
    let mut statement = connection.prepare(&sql)?;
    let mut rows = statement.query([])?;

    let mut array = Vec::new();
    while let Some(row) = rows.next()? {
        let mut object = serde_json::Map::new();
        for (index, column) in columns.iter().enumerate() {
            object.insert(column.clone(), column_value(row, index)?);
        }
        array.push(serde_json::Value::Object(object));
    }
    Ok(serde_json::Value::Array(array))
}

fn table_columns(connection: &Connection, table: &str) -> Result<Vec<String>, VaultError> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let mut rows = statement.query([])?;
    let mut columns = Vec::new();
    while let Some(row) = rows.next()? {
        columns.push(row.get::<_, String>(1)?);
    }
    Ok(columns)
}

fn column_value(row: &Row<'_>, index: usize) -> Result<serde_json::Value, VaultError> {
    Ok(match row.get_ref(index)? {
        ValueRef::Null => serde_json::Value::Null,
        ValueRef::Integer(value) => serde_json::Value::from(value),
        ValueRef::Real(value) => serde_json::Value::from(value),
        ValueRef::Text(bytes) => {
            serde_json::Value::from(String::from_utf8_lossy(bytes).to_string())
        }
        ValueRef::Blob(bytes) => serde_json::Value::from(hex(bytes)),
    })
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut output, byte| {
        let _ = write!(output, "{byte:02x}");
        output
    })
}
