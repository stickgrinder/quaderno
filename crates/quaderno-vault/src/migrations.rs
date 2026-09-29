// SPDX-License-Identifier: GPL-3.0-or-later

//! The schema migrations, embedded from `vault-spec/migrations/*.sql` at build
//! time. See `vault-spec/format.md` §7.1.

include!(concat!(env!("OUT_DIR"), "/migrations_generated.rs"));

/// Every migration, ascending by version, as `(version, sql)`.
pub fn all() -> &'static [(i64, &'static str)] {
    MIGRATIONS
}

/// The migrations newer than `after`, ascending.
pub fn after(after: i64) -> impl Iterator<Item = &'static (i64, &'static str)> {
    MIGRATIONS
        .iter()
        .filter(move |(version, _)| *version > after)
}
