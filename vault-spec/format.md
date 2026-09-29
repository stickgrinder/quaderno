# Quaderno vault format

**Version:** 1 (draft) · **Last updated:** 2026-09-29
**Normative schema:** [`migrations/0001_initial.sql`](migrations/0001_initial.sql)
**License:** this document is CC-BY-SA-4.0. The Quaderno app and the `quaderno-vault` library are GPL-3.0-or-later.

This document describes how Quaderno stores a journal, so that anyone can write a client for another platform without reading Quaderno's code. It describes one application's data, not a generic journal format.

The key words MUST, SHOULD and MAY are used in their usual sense. Everything else is explanation.

---

## 1. Overview

A Quaderno journal is **one encrypted SQLite file** (a *vault*), normally named `*.quaderno`.

- The file is always encrypted with SQLCipher and a passphrase. There is no plaintext mode, no recovery key and no server.
- The schema is fixed and documented here. The SQL migration files are the machine-readable version of this document; if the two disagree, the SQL wins and this document has a bug.
- The same file is used locally, for backups and for sync. Sync works through any service that can store a file: a synced folder (Nextcloud, Syncthing, Dropbox), WebDAV, a USB stick.
- The schema evolves through numbered migration scripts. Old clients can keep reading newer vaults whenever a change is additive.

### Non-goals

Attachments and images, multiple users per journal, a server API, password recovery, full-text search tables inside the vault, and partial or approximate dates.

---

## 2. Encryption

### 2.1 SQLCipher settings

A vault MUST be a SQLCipher 4 database with exactly these settings. Clients MUST set them explicitly rather than rely on library defaults, which change between SQLCipher releases.

| Setting | Value | PRAGMA |
|---|---|---|
| Cipher | AES-256-CBC | (fixed in SQLCipher 4) |
| Page size | 4096 | `PRAGMA cipher_page_size = 4096;` |
| Key derivation | PBKDF2-HMAC-SHA512, 256 000 iterations | `PRAGMA cipher_kdf_algorithm = PBKDF2_HMAC_SHA512;` `PRAGMA kdf_iter = 256000;` |
| Page authentication | HMAC-SHA512 | `PRAGMA cipher_hmac_algorithm = HMAC_SHA512;` |
| Plaintext header | none | `PRAGMA cipher_plaintext_header_size = 0;` |
| Salt | 16 random bytes, managed by SQLCipher | — |

Open sequence: set the key, then the PRAGMAs above, then `PRAGMA foreign_keys = ON;`, then run the first query.

### 2.2 Passphrase

Before being given to SQLCipher, the passphrase MUST be normalized to Unicode NFC and encoded as UTF-8, with no trimming or case changes. After encoding it MUST be 1–1024 bytes long. Pass the bytes with `sqlite3_key()` rather than building a `PRAGMA key` string, so quoting can't alter them.

A forgotten passphrase means the journal is lost. Clients MUST say so clearly when the vault is created.

### 2.3 Remembering the passphrase

A client MAY store the passphrase, or SQLCipher's raw derived key, in the platform's secure storage (GNOME Keyring / Secret Service, Android Keystore, macOS Keychain). It MUST NOT be stored anywhere in plaintext. Opening a vault on a new device MUST only ever require the passphrase.

### 2.4 Changing the passphrase

Copy the vault, run `PRAGMA rekey` on the copy, reopen it with the new passphrase, validate it (§8.3), then atomically replace the original. Changing the passphrase is not an edit: no `updated_at` changes.

### 2.5 No plaintext on disk

Nothing derived from journal content may be written to disk unencrypted by any client. That includes search indexes, statistics caches, thumbnails, previews, logs and crash reports. A search index lives in memory (rebuilt after unlock) or in a separate encrypted file.

---

## 3. Data conventions

| Kind | Rule |
|---|---|
| IDs | UUID as lowercase text with hyphens. New IDs SHOULD be UUIDv7. IDs never change. |
| Technical timestamps (`created_at`, `updated_at`, `deleted_at`) | RFC 3339 in UTC with `Z`, whole seconds: `2026-09-28T04:41:12Z` |
| Editorial time (`dated_at`) | RFC 3339 with the writer's UTC offset, whole seconds: `2026-09-28T06:41:12+02:00`. A full date and time is always required. |
| Text | UTF-8, normalized to NFC before writing. Names and labels are trimmed of leading/trailing whitespace. |
| Booleans | `0` or `1` |
| Deletion | Soft only: set `deleted_at` (and `updated_at`). Rows are never physically removed in version 1. Restoring clears `deleted_at`. |

Every edit to a row MUST set that row's `updated_at` to the current time. Sync depends on it (§8).

---

## 4. Tables

The full definitions, constraints and triggers are in `migrations/0001_initial.sql`. This section explains what each table means.

### 4.1 `vault`

Exactly one row. `id` identifies the journal across all its copies and never changes. Vaults with different `id`s are different journals and are never merged. `schema_version`, `read_compat` and `write_compat` are explained in §7.

### 4.2 `entry`

One row per journal page, dream or quick note.

| Column | Meaning |
|---|---|
| `type` | `journal`, `dream` or `note`. Cannot change after creation. |
| `content` | Markdown. There is no separate title: clients show the first `#` heading, or else the first line. |
| `dated_at` | The date the entry is *about*. Defaults to the creation time; the user can change it (e.g. a dream remembered days later). |
| `mood`, `energy`, `cognitive_load`, `sleep` | Ratings 1–5, or NULL when not set. Labels in Appendix B. On a dream, `mood` means *mood on waking*, and `sleep` is the night the dream came from. Emotions (`entry_choice`) describe the dream's content. |
| `lucid`, `nightmare`, `recurring` | Dream flags: 0/1, or NULL when not set. |
| `source_id` | The quick note this entry was expanded from. |
| `conflict_of` | Set on a copy created when sync found two versions of the same entry (§8.2). |

### 4.3 `choice`

The configurable lists: emotions (`kind = 'emotion'`) and day activities (`kind = 'activity'`).

Every item is identified by its `id`, and entries link to it by that `id`. Names never identify anything.

- **Defaults** have a fixed `id` that is the same in every vault, and an English `label` (e.g. `Calm`) (Appendix A). `label` is frozen: it never changes after release, and the `rule_choice_label_frozen` trigger rejects any change. Clients translate it (§6.2).
- **Renaming** a default sets `custom_label`; `label` stays as it is. Setting `custom_label` back to NULL restores the translated default name.
- **User-created items** have `label` NULL and their name in `custom_label`. Clients MUST NOT put user text in `label`, because it would be sent through translation.
- **Display rule:** show `custom_label` if set, otherwise the translation of `label`, falling back to `label` itself.
- `icon` is a Phosphor icon name (§6.1). `sort_order` sets the display order.
- `hidden = 1` removes an item from pickers while keeping it on past entries and in statistics. This is the normal way to "remove" an item that has been used. `deleted_at` is for real deletion, and clients SHOULD only offer it for items no entry uses.

### 4.4 `subject`

Things users add while writing: people (`person`), places (`place`), things (`thing`) and tags (`tag`). `kind` cannot change. Names are not unique; see §5.3 for duplicates.

### 4.5 Link tables: `entry_choice`, `entry_subject`, `entry_color`

They connect entries to choices, subjects and the 12 fixed colors (Appendix C). The primary key is the pair itself. Removing a link sets `deleted_at`; adding it again clears `deleted_at` and updates `updated_at`. Links are never deleted physically, so an unlink on one device syncs correctly to another.

### 4.6 `setting`

Settings that change what the data *means* and therefore must be the same on every device. UI preferences do not belong here.

| Key | Value | Meaning |
|---|---|---|
| `day_end` | `HH:MM`, from `00:00` to `06:00` | Entries dated before this time count for the previous day (§5.4). Default `03:00`. |

---

## 5. Rules

### 5.1 What each entry type may hold

Enforced by the `rule_*` triggers. A write that breaks a rule is rejected by the database.

| Field | Journal page | Dream | Quick note |
|---|:---:|:---:|:---:|
| Content, dated_at | ✓ | ✓ | ✓ |
| Ratings (mood, energy, cognitive load, sleep) | ✓ | ✓ | — |
| Dream flags | — | ✓ | — |
| Emotions, activities (`entry_choice`) | ✓ | ✓ | — |
| Colors (`entry_color`) | ✓ | ✓ | — |
| People, places, things (`entry_subject`) | ✓ | ✓ | — |
| Tags (`entry_subject`, kind `tag`) | ✓ | ✓ | ✓ |
| `source_id` (must point to a note) | ✓ | ✓ | — |

Every field is optional except `dated_at`. `content` may be the empty string, so an entry can be only its date and some metadata (a dream remembered only as "a nightmare, woke up anxious").

Daily statistics about mood use journal pages only. A dream's mood is mood on waking, a different measure, and is reported separately.

### 5.2 Expanding a quick note

Expanding creates a **new** journal page or dream with `source_id` pointing to the note, copying the note's text and tags. The note is not changed. A note counts as *expanded* when at least one entry with `deleted_at` NULL has it as `source_id`; otherwise it is *open*. The link is stored on the new entry only, so the two sides can't disagree after a sync.

### 5.3 People, places, things, tags

Clients SHOULD compare names without regard to case, after NFC normalization and trimming, both for autocomplete and to warn about duplicates. Two devices can still create the same name while offline. To merge subject B into A: for every active link to B, create or restore the same link to A, soft-delete the link to B, then soft-delete B.

### 5.4 The journal day

An entry's *journal day* is the calendar date of `dated_at` in its own stored offset, after subtracting `day_end`. With `day_end = 03:00`, an entry dated `2026-09-29T01:30:00+02:00` belongs to 28 September. Statistics and day grouping use the journal day. Using the stored offset keeps entries written while travelling on the right day.

### 5.5 Changing the entry date

When the user picks a new date, clients SHOULD keep the original time of day and offset, and change only the date.

### 5.6 "Last updated"

The last-update time shown for an entry is the latest `updated_at` among the entry and its link rows.

---

## 6. Icons and translations

### 6.1 Icons

`choice.icon` holds an icon name from **Phosphor Icons 2.1, regular weight** (MIT license), for example `flower-lotus`. Clients MUST show a generic fallback icon for a name they don't have, and MUST NOT rewrite the stored name. Icons for the fixed ratings and colors are not stored in the vault; Appendix B lists the ones Quaderno uses so that clients can look consistent.

### 6.2 Translating default labels

Default `label` values are English source strings in the gettext sense: the English text is the lookup key, and the translation catalog maps it to other languages.

- **Context:** each label is translated with gettext context equal to the item's `kind` (`emotion` or `activity`), i.e. `pgettext("emotion", "Calm")`. The same English word can then be translated differently as an emotion and as an activity, and translators know which one they are looking at.
- **Fallback:** a language with no translation shows the English `label`. No client ever needs a translation to display a vault.
- **Catalog:** Quaderno's reference catalogs live at `gnome/po/*.po` in the reference repository. Other clients MAY copy or convert them, but each client ships its own translations. Clients whose platform doesn't use gettext (Android `strings.xml`, Windows `.resx`) convert the catalog at build time, deriving resource names from the item's `id` or from context plus label (`emotion_calm`). That naming is internal to each client and never stored in the vault.
- **Wording fixes:** if a default's English wording needs to change, the change goes into the translation catalogs (English included), never into the vault. That avoids a migration touching every user's data.
- Rating labels, dream flags and colors (Appendices B and C) are not stored as text in the vault. Clients translate them with context `rating.mood`, `rating.energy`, `rating.cognitive_load`, `rating.sleep`, `dream_flag` and `color`.

---

## 7. Schema evolution

### 7.1 Migrations

The schema changes only through numbered SQL scripts in `migrations/` (`0001_initial.sql`, `0002_….sql`, …). Each migration:

- runs inside one transaction;
- updates `vault.schema_version` to its own number, plus `read_compat` and `write_compat` when needed;
- replaces `rule_*` triggers by name when type rules change (SQLite cannot alter a `CHECK` constraint without rebuilding the table, so type rules are triggers on purpose).

A client "knows" version N when it contains migrations up to N.

### 7.2 Compatibility

| Vault state | A client that knows version N … |
|---|---|
| `write_compat ≤ N` | reads and writes normally |
| `read_compat ≤ N < write_compat` | opens the vault read-only |
| `N < read_compat` | refuses to open it and asks the user to update |

- **Additive** migrations (new nullable column, new table, relaxed rule) leave `read_compat` and `write_compat` unchanged, so older clients keep working.
- A migration that older clients could write incorrectly raises `write_compat`.
- A migration that changes the meaning of existing data raises `read_compat`.

A client SHOULD ask before migrating a synced vault when the migration raises either value, because that makes older devices read-only or unable to open it.

### 7.3 Keeping data a client doesn't understand

Older clients MUST preserve columns and tables added by newer versions:

- Edit rows with `UPDATE` naming only the columns you change. Never edit by deleting and re-inserting.
- When copying rows between files (sync, import), copy every column that exists, not only the ones the client knows.

---

## 8. Sync

### 8.1 Files

- **Working copy:** the vault the client edits, kept in local app storage. It is never itself placed in a synced folder, because SQLite's `-wal` and `-shm` files and partial writes would corrupt it in transit.
- **Published copy:** a consistent snapshot of the working copy, made with the SQLite backup API or `sqlcipher_export()`. Never copy the working file while it is open. The snapshot is written to a temporary file in the destination folder, then renamed over the published file in one step.
- **Base copy:** the last snapshot this device published or merged, kept locally. It is the common ancestor for detecting edits made on both sides.

When the published file has changed since the client last saw it, the client validates it (§8.3), checks that `vault.id` matches, merges it into the working copy, then publishes the result.

Conflict copies made by sync tools (for example Syncthing's `.sync-conflict-…` files) are vaults like any other. If their `vault.id` matches, the client merges them and MAY then delete them.

### 8.2 Merging

Merging walks every table and matches rows by primary key.

1. A row that exists on only one side is copied.
2. When both sides have a row, the one with the later `updated_at` wins, including its `deleted_at`. On a tie, the version whose column values, joined in column order, compare greater byte by byte wins, so every device picks the same one.
3. **Entry text:** if the base copy shows that `content` changed on *both* sides, the losing version is kept as a new entry: same type, `dated_at` and ratings, `conflict_of` set to the original's ID. The user decides later. Without a base copy (first sync of a device), a conflict copy is made whenever the two contents differ.
4. `setting` rows use rule 2.

If the two vaults have different `schema_version`s, the older one is migrated first. If the client can't do that, it stops and tells the user.

### 8.3 Validation

Before adopting any vault that came from outside (a synced file, a backup, an import), and after a passphrase change, a client MUST check:

- it opens with the passphrase and the settings in §2.1;
- `PRAGMA integrity_check` returns `ok`;
- `PRAGMA foreign_key_check` returns nothing;
- the `vault` row exists and its compatibility values allow the intended access (§7.2).

Routine opening of the working copy needs only the key, the `vault` row and the compatibility check.

---

## 9. Kept outside the vault

These belong to each device and MUST NOT be stored in the vault: UI preferences, window state, the sync folder location, the base copy, remembered passphrases, search indexes and caches. They follow §2.5.

---

## 10. Test material

The repository will provide `fixtures/sample.quaderno`: a vault encrypted with the passphrase `Quaderno sample 1` (NFC, ASCII only), plus `fixtures/sample.json` with its expected contents. A client that opens the sample and reproduces the JSON implements §2 and §4 correctly. *[To be produced with the reference implementation.]*

---

## 11. Open questions

- Is there a maximum `content` size a mobile client can rely on?
- Tombstones (soft-deleted rows) are never purged in version 1. Decide whether a later version adds a purge that all devices agree on.
- Attachments are out of scope for now. If they come, they probably live as encrypted blobs in a separate table, which affects file size and sync.

---

## Appendix A — Default emotions and activities

Seeded by migration 0001 with these fixed IDs. `label` holds the English text below, frozen; `custom_label` starts NULL.

| ID | Kind | `label` | Icon |
|---|---|---|---|
| `01a0e54f-b000-7fb5-a336-339c06c3a42f` | emotion | Joy | `sun` |
| `01a0e54f-b001-7559-a22a-ffe581c89d9d` | emotion | Gratitude | `hand-heart` |
| `01a0e54f-b002-7f29-bd09-fc096a90db83` | emotion | Calm | `flower-lotus` |
| `01a0e54f-b003-78b5-969f-199867984ade` | emotion | Love | `heart` |
| `01a0e54f-b004-7fb4-a335-5026cad417e6` | emotion | Pride | `medal` |
| `01a0e54f-b005-7fb7-9cd5-737230e26061` | emotion | Hope | `sparkle` |
| `01a0e54f-b006-7db0-bd8d-1eaa4d6b64b5` | emotion | Sadness | `cloud-rain` |
| `01a0e54f-b007-74d1-988a-2eb50d027ebb` | emotion | Anger | `fire` |
| `01a0e54f-b008-70a4-864f-ecdfc2fadf45` | emotion | Anxiety | `lightning` |
| `01a0e54f-b009-771d-8b2f-7243f7c1a93f` | emotion | Loneliness | `user-minus` |
| `01a0e54f-b00a-71d1-91dd-11163195ed02` | emotion | Boredom | `hourglass-medium` |
| `01a0e54f-b00b-798e-af1b-11ef2129c5fd` | emotion | Nostalgia | `clock-counter-clockwise` |
| `01a0e54f-b00c-7db7-8e30-cb9d36f00e43` | activity | Work | `briefcase` |
| `01a0e54f-b00d-7997-8a7a-1e45368c0bd4` | activity | Exercise | `barbell` |
| `01a0e54f-b00e-7e98-8488-c59b5b1e5c36` | activity | Walk | `person-simple-walk` |
| `01a0e54f-b00f-7376-b0d5-7169aac16696` | activity | Reading | `book-open` |
| `01a0e54f-b010-7a50-9164-445a3e69de85` | activity | Cooking | `cooking-pot` |
| `01a0e54f-b011-708d-9ed4-05d688df30ef` | activity | Friends & family | `users-three` |
| `01a0e54f-b012-7cf8-a8e2-76fe92463718` | activity | Music | `music-notes` |
| `01a0e54f-b013-7e6b-9c40-1c9f21e4734c` | activity | Side project | `code` |
| `01a0e54f-b014-7f60-b70f-f04f9b13a162` | activity | Nature | `tree` |
| `01a0e54f-b015-7aee-9cf2-c0dc7cb9f319` | activity | Meditation | `person-simple-tai-chi` |
| `01a0e54f-b016-7307-b266-54e22ea1518a` | activity | Gaming | `game-controller` |
| `01a0e54f-b017-7d3b-a756-3e9a6fa79642` | activity | Travel | `airplane` |
| `01a0e54f-b018-7c91-a5ba-b36dd93bd4db` | activity | Screen time | `device-mobile` |
| `01a0e54f-b019-7a3c-97cd-61cb08775256` | activity | Chores | `broom` |

## Appendix B — Rating scales

Values are fixed; labels and icons are what Quaderno shows (informative).

| Value | Mood | Energy | Cognitive load | Sleep |
|---|---|---|---|---|
| 1 | Awful · `smiley-sad` | Drained · `battery-empty` | Idle · `cell-signal-none` | Sleepless · `eye` |
| 2 | Bad · `smiley-nervous` | Low · `battery-low` | Light · `cell-signal-low` | Poor · `alarm` |
| 3 | Okay · `smiley-meh` | Steady · `battery-medium` | Moderate · `cell-signal-medium` | Fair · `cloud-moon` |
| 4 | Good · `smiley` | High · `battery-high` | Heavy · `cell-signal-high` | Good · `moon` |
| 5 | Great · `smiley-wink` | Charged · `battery-full` | Overloaded · `cell-signal-full` | Deep · `bed` |

Dream flag icons: lucid `lightbulb-filament`, nightmare `ghost`, recurring `repeat`.

## Appendix C — Colors

Fixed keys, with the GNOME palette values Quaderno uses (informative):
`red` #e01b24 · `orange` #ff7800 · `yellow` #f6d32d · `green` #33d17a · `teal` #2190a4 · `blue` #3584e4 · `purple` #9141ac · `pink` #d56199 · `brown` #986a44 · `gray` #9a9996 · `black` #241f31 · `white` #ffffff

---

## Changelog

- **2026-09-29** — Reference translation catalogs live at `gnome/po/`; every client ships its own translations.
- **2026-09-28** — Decided: `mood` on a dream is mood on waking and is kept out of daily mood statistics. Empty `content` explicitly allowed.
- **2026-09-28** — `choice`: replaced `key` + translated `label` with a frozen English `label` (gettext source string, context = kind) and `custom_label` for user names. Added §6.2 on translations.
- **2026-09-28** — First draft. Replaces the generic OJPP 1.0 draft with a Quaderno-specific schema.
