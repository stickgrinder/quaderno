# Quaderno — plan

One milestone per agent session (or a few). Tick the box when all acceptance criteria are met and every check in `AGENTS.md` passes. Add a one-line note for anything deferred.

## Version 1.0 — capture entries

### M0 · Repository scaffolding
- [ ] Monorepo layout from `AGENTS.md`: Cargo workspace with `crates/quaderno-vault` and `gnome`; meson project in `gnome/` driving cargo; Flatpak manifest in `gnome/build-aux/`; GPL-3.0-or-later `COPYING`; SPDX headers; shared `po/` with `it`; `android/README.md` placeholder.
- [ ] Empty `AdwApplicationWindow` with the app ID, desktop file, metainfo, GSettings schema, placeholder icon.
- [ ] CI: vault workflow (fmt, clippy `-D warnings`, tests) and GNOME workflow (meson build, Flatpak build), each triggered by its paths (`AGENTS.md` §Repository layout).

**Accept:** `flatpak run io.github.stickgrinder.Quaderno` opens an empty window; CI green.

### M1 · Vault library (`quaderno-vault`, no GTK)
- [ ] Create, open, close; SQLCipher settings set explicitly (vault spec §2.1); NFC passphrase handling; `zeroize`.
- [ ] Migration runner embedding `vault-spec/migrations/*.sql`; compatibility states (§7.2).
- [ ] Typed API: entries, choices, subjects, links, settings; all writes set `updated_at`, soft delete, NFC, timestamp formatting; edits as `UPDATE` of named columns.
- [ ] Journal-day computation (§5.4); title derivation; word count.
- [ ] Snapshot (backup API / `sqlcipher_export`), rekey (§2.4), validation (§8.3).
- [ ] Tests for every rule trigger, the compatibility matrix, rekey, snapshot, NFC, journal day at `day_end` edges.
- [ ] `fixtures/sample.quaderno` + `fixtures/sample.json` generator (vault spec §10), and a small independent Python reader in `tools/` that opens the sample using only the spec.

**Accept:** `cargo test -p quaderno-vault` covers all of the above; the Python reader reproduces `sample.json`.

### M2 · Shell: welcome, unlock, lock
- [ ] Welcome / unlock / error / main pages (ui-spec §2, §3.1–3.2).
- [ ] Keyring via `oo7`; graceful fallback without Secret Service.
- [ ] Lock action, auto-lock on idle and on session lock; memory cleared on lock.
- [ ] Read-only banner for newer vaults.

**Accept:** create a vault, quit, reopen with and without keyring, wrong passphrase handling, auto-lock works.

### M3 · Timeline and journal page editor
- [ ] Sections sidebar, entry list with journal-day headers and filters, editor with GtkSourceView and dimmed Markdown.
- [ ] Autosave, discard-empty, delete with undo, entry date popover and backdated info bar.
- [ ] In-memory search.
- [ ] Breakpoints (ui-spec §2.2).

**Accept:** write, edit, re-date, delete/undo and search journal pages; nothing unencrypted appears on disk (check `$XDG_*` dirs and `/tmp`).

### M4 · Details panel
- [ ] Ratings, emotions/activities picker, colors, token fields with autocomplete for people/places/things/tags.
- [ ] Type rules respected in the UI (vault spec §5.1).

**Accept:** all metadata can be set and cleared; database triggers are never hit in normal use.

### M5 · Dreams and quick notes
- [ ] Dream type: flags in header, "Mood on waking".
- [ ] Quick notes view (Open / Expanded / All), expand into journal page or dream, "Expanded into" / "From quick note" links.

**Accept:** product spec §6 works end to end, including deleting an expanded note.

### M6 · Preferences and collections
- [ ] Preferences dialog, all three pages (product spec §8), including day-end setting and change passphrase.
- [ ] List management with icon picker and matching icons (product spec §9).
- [ ] Collections pages with rename, merge, delete-if-unused.

**Accept:** every preference persists and takes effect; merging two people keeps all their entries.

### M7 · Backups, export, release
- [ ] Automatic and manual backups; open-a-backup from the error screen.
- [ ] Markdown and JSON export with the unencrypted warning.
- [ ] Italian translation complete; English strings reviewed.
- [ ] Flatpak manifest ready for Flathub; AUR `PKGBUILD` for `quaderno` and `quaderno-git` in `gnome/build-aux/aur/` (system-SQLCipher feature, meson build).
- [ ] Metainfo with screenshots; root `README.md`; first tags `vault-v1.0.0` and `gnome-v1.0.0`.

**Accept:** a clean install from Flatpak and from the AUR build both create, fill, back up and export a journal.

## Version 1.1
- Insights (`docs/statistics.md`).
- Sync through a folder (vault spec §8).
- "On this day".
- Import from Quaderno's own Markdown/JSON export.

## Later
Quick capture (keyring-unlocked vault only), daily reminder, "append note to existing entry", hidden-syntax editor mode, recently deleted view, Android companion in `android/`, reusing `quaderno-vault` through `crates/quaderno-vault-ffi` (uniffi Kotlin bindings) and the shared `po/` catalogs.
