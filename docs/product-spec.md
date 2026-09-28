# Quaderno — product spec

**Scope:** version 1.0. **Last updated:** 2026-09-28.
Data rules live in `vault-spec/format.md`; this document describes behavior. Where the two overlap, the vault spec is authoritative.

## 1. What Quaderno is

A private journal for GNOME. Three kinds of entry:

- **Journal page:** writing about a day, with optional metadata.
- **Dream:** a remembered dream, with optional metadata and three flags (lucid, nightmare, recurring).
- **Quick note:** a thought captured fast, with only text and tags, meant to be expanded later into a full entry.

Everything is stored in one encrypted file. Nothing leaves the device unless the user exports or (from 1.1) syncs.

## 2. Scope

| In 1.0 | 1.1 | Later | Dropped |
|---|---|---|---|
| Vault create / unlock / lock | Insights (`docs/statistics.md`) | Quick capture window + global shortcut (only when unlocked via keyring) | GNOME Shell search provider |
| Journal pages, dreams, quick notes | Sync through a folder | Daily reminder notification | Year in pixels |
| Details panel (all metadata) | "On this day" | "Append note to an existing entry" | "Wet" dream flag |
| Expanding notes | Import | Android companion | |
| Collections (people, places, things, tags) incl. merge | | Hidden-syntax editor mode | |
| Preferences | | | |
| Automatic backups, export | | | |
| Italian and English | | | |

## 3. The vault

### 3.1 First run

When no vault is configured, Quaderno shows the welcome screen with two choices: **Create a new journal** and **Open an existing journal…**.

Creating:
1. The user enters a passphrase twice. The Create button stays disabled until both match and are at least 8 characters. A strength hint is shown (informative, not blocking beyond the 8-character minimum).
2. The user must tick "I understand that a forgotten passphrase cannot be recovered".
3. "Remember passphrase on this device" (off by default) stores it in the keyring.
4. Location defaults to `$XDG_DATA_HOME/quaderno/journal.quaderno`; "Change…" opens a file chooser (portal).
5. The vault is created with migration 0001 and a new vault ID, then opened.

Opening an existing journal: file chooser → unlock screen. A file whose `vault.id` Quaderno hasn't seen before is simply added as the configured vault. Version 1 supports **one vault** at a time; switching replaces the configured one (the old file is not touched).

### 3.2 Unlock

- If the passphrase is in the keyring, the vault opens without a prompt.
- Otherwise the unlock screen asks for it. A wrong passphrase shows an inline error; after 5 wrong attempts, a 30-second delay before the next.
- Vault states on open, per `vault-spec/format.md` §7.2: normal, **read-only** (banner explains that a newer version of Quaderno changed the journal; editing is disabled), or **refused** (message: update Quaderno).
- A file that fails validation shows an error screen offering to open the latest automatic backup.

### 3.3 Locking

- **Lock** action (Ctrl+L, and in the main menu) closes the vault, clears in-memory content and search index, and shows the unlock screen.
- Auto-lock after N minutes idle (default 5, 0 = never) and, optionally, when the session locks (default on).
- Pending edits are saved before locking.

### 3.4 Passphrase

- **Change passphrase** (Preferences › Privacy & data): current passphrase, new twice, same rules as creation. Implemented as copy → rekey → validate → replace (vault spec §2.4). The keyring entry is updated if one exists.
- **Remember on this device** toggle adds or removes the keyring entry.

## 4. Entries

### 4.1 Creating

- The New split button creates a journal page; its menu offers Journal page, Dream, Quick note.
- Shortcuts: Ctrl+N journal page, Ctrl+Shift+N quick note, Ctrl+Alt+N dream.
- New entries get `dated_at` = now with the local offset.

### 4.2 Editing and saving

- Text is Markdown, edited in a plain-text editor with syntax highlighting; Markdown symbols are shown dimmed.
- **Autosave:** 1 second after the last keystroke, when the editor loses focus, when switching entries, and before locking or quitting. The header shows "Saved" / "Saving…". No save button.
- **Title:** derived from the first `#` heading, otherwise the first line, otherwise "Untitled" (display only; nothing stored).
- An entry with empty text and no metadata is discarded when the user leaves it, instead of being saved.
- The footer shows word count, creation time and last-update time (vault spec §5.6).

### 4.3 Entry date

- The date button in the header opens a calendar popover with Today and Yesterday shortcuts.
- Changing the date keeps the time of day and offset (vault spec §5.5). A separate time field is available in the popover for the rare case it matters.
- When the date differs from the creation date, an info bar says so: "Entry date set to Tue 22 Sep, written 5 days later", with a "Use creation date" button.
- Future dates are allowed but shown dimmed in the calendar.

### 4.4 Deleting

- Delete (in the entry menu, or the Delete key in the list) soft-deletes the entry and shows a toast with **Undo** for 10 seconds.
- Deleted entries are not shown anywhere in 1.0. (A "Recently deleted" view may come later; the data is kept.)
- Deleting a quick note that has been expanded is allowed; the expanded entry keeps its `source_id` and shows "From a deleted note".

### 4.5 Timeline and lists

- The list pane groups entries by journal day (vault spec §5.4), newest first, with headers "Today", "Yesterday", then weekday + date.
- Filter chips: All, Journal, Dreams, Notes. Sidebar items (Journal pages, Dreams, Quick notes) open the list pre-filtered.
- Each row: type icon, title, two-line excerpt, time, mood icon if set, first tag.
- Selecting a collection item (a person, place, thing or tag) in its collection page shows the timeline filtered to entries linked to it.

### 4.6 Search

- Ctrl+F or the search button. Matches entry text and the names of linked subjects, choices (translated or custom labels) and colors.
- Case- and accent-insensitive. The index is built in memory after unlock and dropped on lock (hard rule 1).

## 5. Metadata (details panel)

Toggled with the details button (Ctrl+I). Every field is optional. Which fields each type shows follows vault spec §5.1; the panel only shows what the current type allows.

- **Ratings:** four rows of five icon buttons. Clicking the selected value again clears it. On dreams, the mood row is labelled "Mood on waking".
- **Dream flags:** toggle chips in the editor header: Lucid, Nightmare, Recurring.
- **Emotions, activities:** chips of the selected items, plus an Add button opening a searchable picker of visible (not hidden, not deleted) items. Items linked to this entry that were later hidden still show on it.
- **Colors:** the 12 fixed swatches, multi-select.
- **People, places, things, tags:** one token field per kind. Typing shows matches (vault spec §5.3: case-insensitive, trimmed, NFC), sorted by use count, with a "Create …" row. Enter picks the highlighted row; Shift+Enter always creates. Backspace in an empty field removes the last token.

## 6. Quick notes

- The Quick notes view has filter chips **Open**, **Expanded**, **All**; Open is the default.
- **Expand into entry** (split button in the note header): Journal page (dated now) or Dream (dated at the note's `dated_at`). Creates the new entry with `source_id` = the note, copies the text and tags, opens it, and shows a toast. The note is unchanged and moves to Expanded.
- An expanded note shows "Expanded into" with a link to each entry made from it. An entry made from a note shows "From quick note" with a link back.
- Notes are counted only as open vs expanded; no other statistics use them.

## 7. Collections

Sidebar › Collections: People, Locations, Things, Tags.

- Each page lists items with their entry count; selecting one filters the timeline (§4.5).
- **Rename:** edits `subject.name`.
- **Merge:** select two or more items → "Merge into…" → pick the one to keep. Implements vault spec §5.3.
- **Delete:** only for items with no linked entries; otherwise the action explains that merging is the way to remove duplicates.

## 8. Preferences

Three pages. Settings marked *vault* are stored in the vault (the same on every device); the rest are per-device GSettings.

**General**
- Writing font: Serif (bundled Source Serif 4, default), Sans (system document font), Mono (system monospace font), Custom (font chooser). Text size 14–26 px, default 19. Line width: Narrow / Comfortable / Wide.
- Markdown symbols: Dimmed (1.0 offers only this; Hidden comes later).
- Spell checking languages (from system dictionaries, via libspelling).
- Default new entry type: Journal page, or "Dream before 10:00, then journal page".
- *Vault:* A day ends at (`day_end`), 00:00–06:00, default 03:00.
- Appearance follows the system style; no in-app override.

**Entries**
- Emotions and activities: reorder (drag, or Move up/down in the row menu), rename (sets `custom_label`), Reset name (clears it; defaults only), change icon (matching-icons picker, §9), Hide/Show, Add, Delete (only unused items).

**Privacy & data**
- Remember passphrase on this device (keyring).
- Auto-lock after (minutes), Lock when the session locks.
- Change passphrase…
- Journal file: shows path; "Move…" moves the vault file (copy, validate, then remove the old file).
- Automatic backups: on/off (default on), weekly, keep last 8, to `$XDG_DATA_HOME/quaderno/backups/`, or a folder the user picks. Backups are vault snapshots (vault spec §8.1), encrypted with the passphrase valid at the time.
- Back up now…
- Export… (§10).

## 9. Matching icons

In the icon picker, the user types a word and sees matching Phosphor icons, then the full grid by category.

- Matching uses, in order: the icon name, a curated synonym table shipped with the app (English and Italian, e.g. `calm, calma, serene, relaxed → flower-lotus, drop, wave`), Phosphor's own tags, then its categories.
- When creating an item, suggestions come from its name automatically.
- Icons already used by another item of the same kind are listed last. Icons used by the fixed scales are excluded.
- The section is labelled "Matching icons", never "Suggested".

## 10. Export

- **Markdown folder:** one file per entry, `YYYY-MM-DD_<type>_<first 8 chars of id>.md`, with YAML front matter holding all metadata by readable name (e.g. `emotions: [Calm, Joy]`, `people: [Marta]`, `mood: 4`), then the content.
- **JSON:** one file with all tables as arrays, column names as keys.
- Before exporting, a confirmation dialog says the export is **not encrypted**. Export always goes to a location the user picks.

## 11. Errors and edge cases

- Disk full or write failure: toast "Couldn't save — your text is still here", with retry. Unsaved text stays in memory and is not lost when switching entries.
- Vault file missing at startup: error screen with "Locate file…" and "Open a backup…".
- Vault file changed by another program while open (1.0 has no sync): detect by file modification on save; stop editing and offer to reload. Never overwrite blindly.
- Keyring unavailable (e.g. no Secret Service on a non-GNOME desktop): the "remember" option is disabled with an explanation; unlocking always asks.
