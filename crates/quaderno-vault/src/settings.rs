// SPDX-License-Identifier: GPL-3.0-or-later

//! Vault settings that change what the data means (`vault-spec/format.md`
//! §4.6).

use rusqlite::params;

use crate::error::VaultError;
use crate::time;
use crate::vault::Vault;

impl Vault {
    /// The `day_end` setting, `HH:MM` from `00:00` to `06:00`.
    pub fn day_end(&self) -> Result<(u8, u8), VaultError> {
        let value: String = self.connection().query_row(
            "SELECT value FROM setting WHERE key = 'day_end'",
            [],
            |row| row.get(0),
        )?;
        time::parse_day_end(&value)
    }

    /// Sets `day_end` (spec §4.6).
    pub fn set_day_end(&mut self, day_end: (u8, u8)) -> Result<(), VaultError> {
        let value = time::format_day_end(day_end);
        let (hours, minutes) = time::parse_day_end(&value)?;
        let connection = self.require_writable()?;
        connection.execute(
            "INSERT INTO setting (key, value, updated_at)
             VALUES ('day_end', ?1, ?2)
             ON CONFLICT(key)
             DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![
                time::format_day_end((hours, minutes)),
                time::format_timestamp(time::now())
            ],
        )?;
        Ok(())
    }
}
