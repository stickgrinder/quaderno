// SPDX-License-Identifier: GPL-3.0-or-later

//! Backup naming, pruning and due logic (product spec §8).
//!
//! These helpers are GTK-free so they can be unit-tested; the window supplies
//! the folder (default or chosen) and performs the snapshot through
//! [`quaderno_vault::Vault::backup`].

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use jiff::Zoned;
use uuid::Uuid;

/// How many backups to keep per vault (product spec §8).
pub const KEEP: usize = 8;

/// Days after which another automatic backup is due.
pub const INTERVAL_DAYS: i64 = 7;

/// The filename prefix shared by one vault's backups: `journal-<id8>_`.
///
/// The last eight hex characters are used because vault ids are UUIDv7: their
/// leading characters encode a timestamp, so two vaults created in the same
/// millisecond would share a prefix and pruning would cross between them. The
/// tail is random.
pub fn prefix(vault_id: Uuid) -> String {
    let simple = vault_id.simple().to_string();
    format!("journal-{}_", &simple[simple.len() - 8..])
}

/// The backup filename for a snapshot taken at `now` (local time).
pub fn filename(vault_id: Uuid, now: &Zoned) -> String {
    format!(
        "{}{}.quaderno",
        prefix(vault_id),
        now.strftime("%Y%m%dT%H%M%S")
    )
}

/// The backup files for `vault_id` in `dir`, newest first. Unrelated files in
/// the folder are ignored, so a shared or custom folder is safe.
pub fn existing(dir: &Path, vault_id: Uuid) -> Vec<PathBuf> {
    let prefix = prefix(vault_id);
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(&prefix) && name.ends_with(".quaderno"))
        })
        .collect();
    // The timestamp in the name sorts chronologically.
    files.sort();
    files.reverse();
    files
}

/// Whether a backup is due: none exists, or the newest is at least
/// [`INTERVAL_DAYS`] days old.
pub fn is_due(newest: Option<&Path>, now: &Zoned) -> bool {
    let Some(path) = newest else {
        return true;
    };
    let Ok(modified) = fs::metadata(path).and_then(|metadata| metadata.modified()) else {
        return true;
    };
    let Ok(modified) = modified.duration_since(UNIX_EPOCH) else {
        return true;
    };
    let age = now.timestamp().as_second() - modified.as_secs() as i64;
    age >= INTERVAL_DAYS * 86_400
}

/// The files to delete so that at most `keep` remain, oldest first.
pub fn to_prune(files: &[PathBuf], keep: usize) -> Vec<PathBuf> {
    files.iter().skip(keep).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zoned(value: &str) -> Zoned {
        value
            .parse::<jiff::Timestamp>()
            .unwrap()
            .in_tz("UTC")
            .unwrap()
    }

    #[test]
    fn filename_is_prefixed_and_timestamped() {
        let id = Uuid::parse_str("01a0e54f-b000-7fb5-a336-339c06c3a42f").unwrap();
        let name = filename(id, &zoned("2026-09-28T06:41:12Z"));
        assert_eq!(name, "journal-06c3a42f_20260928T064112.quaderno");
    }

    #[test]
    fn existing_filters_by_prefix_and_sorts_newest_first() {
        let id = Uuid::now_v7();
        let other = Uuid::now_v7();
        let dir = tempfile::tempdir().unwrap();
        for name in [
            filename(id, &zoned("2026-01-01T00:00:00Z")),
            filename(id, &zoned("2026-03-01T00:00:00Z")),
            filename(id, &zoned("2026-02-01T00:00:00Z")),
            filename(other, &zoned("2026-04-01T00:00:00Z")),
            "journal-backup.txt".to_string(),
        ] {
            fs::write(dir.path().join(name), b"x").unwrap();
        }
        let files = existing(dir.path(), id);
        let names: Vec<String> = files
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            names,
            vec![
                filename(id, &zoned("2026-03-01T00:00:00Z")),
                filename(id, &zoned("2026-02-01T00:00:00Z")),
                filename(id, &zoned("2026-01-01T00:00:00Z")),
            ]
        );
    }

    #[test]
    fn missing_folder_has_no_backups() {
        let dir = tempfile::tempdir().unwrap();
        assert!(existing(&dir.path().join("nope"), Uuid::now_v7()).is_empty());
    }

    #[test]
    fn is_due_when_there_is_no_backup() {
        assert!(is_due(None, &zoned("2026-09-28T06:41:12Z")));
    }

    #[test]
    fn is_due_only_after_the_interval() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("backup.quaderno");
        fs::write(&path, b"x").unwrap();

        // A freshly written file is not due.
        assert!(!is_due(Some(&path), &Zoned::now()));

        // Exactly the interval later, it is due.
        let now = Zoned::now() + jiff::Span::new().days(INTERVAL_DAYS);
        assert!(is_due(Some(&path), &now));
    }

    #[test]
    fn prune_keeps_the_newest() {
        let files: Vec<PathBuf> = (0..10)
            .map(|index| PathBuf::from(format!("journal-01a0e54f_2026010{index}T000000.quaderno")))
            .collect();
        // Newest first: the last four are pruned.
        let pruned = to_prune(&files, KEEP);
        assert_eq!(pruned.len(), 2);
        assert_eq!(pruned[0], files[8]);
        assert_eq!(pruned[1], files[9]);
    }
}
