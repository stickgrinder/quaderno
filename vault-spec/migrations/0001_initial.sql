-- Quaderno vault format — migration 0001 (initial schema)
-- Target: schema_version 1. Run inside one transaction on a freshly keyed, empty SQLCipher database.
-- Clients MUST run with PRAGMA foreign_keys = ON.

-- ---------------------------------------------------------------------------
-- Vault identity and versioning (exactly one row)
-- ---------------------------------------------------------------------------
CREATE TABLE vault (
  singleton      INTEGER PRIMARY KEY CHECK (singleton = 1),
  id             TEXT    NOT NULL,              -- UUID of this journal; never changes
  format         TEXT    NOT NULL CHECK (format = 'quaderno'),
  schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
  read_compat    INTEGER NOT NULL,              -- lowest schema version a client must implement to read
  write_compat   INTEGER NOT NULL,              -- lowest schema version a client must implement to write
  created_at     TEXT    NOT NULL,
  CHECK (read_compat <= write_compat AND write_compat <= schema_version)
);

-- ---------------------------------------------------------------------------
-- Entries: journal pages, dreams, quick notes
-- ---------------------------------------------------------------------------
CREATE TABLE entry (
  id          TEXT    PRIMARY KEY,
  type        TEXT    NOT NULL CHECK (type IN ('journal', 'dream', 'note')),
  content     TEXT    NOT NULL,                 -- Markdown, UTF-8, NFC
  dated_at    TEXT    NOT NULL,                 -- editorial time, RFC 3339 with offset
  mood        INTEGER CHECK (mood           BETWEEN 1 AND 5),
  energy      INTEGER CHECK (energy         BETWEEN 1 AND 5),
  cognitive_load INTEGER CHECK (cognitive_load BETWEEN 1 AND 5),
  sleep       INTEGER CHECK (sleep          BETWEEN 1 AND 5),
  lucid       INTEGER CHECK (lucid     IN (0, 1)),
  nightmare   INTEGER CHECK (nightmare IN (0, 1)),
  recurring   INTEGER CHECK (recurring IN (0, 1)),
  source_id   TEXT    REFERENCES entry(id),     -- the quick note this entry was expanded from
  conflict_of TEXT    REFERENCES entry(id),     -- set on a copy created by sync conflict resolution
  created_at  TEXT    NOT NULL,
  updated_at  TEXT    NOT NULL,
  deleted_at  TEXT
);

-- ---------------------------------------------------------------------------
-- Configurable lists: emotions and day activities
-- ---------------------------------------------------------------------------
CREATE TABLE choice (
  id           TEXT    PRIMARY KEY,           -- fixed for defaults (Appendix A), UUIDv7 for user items
  kind         TEXT    NOT NULL CHECK (kind IN ('emotion', 'activity')),
  label        TEXT,                          -- defaults only: frozen English label, translated with gettext
                                              -- (context = kind). NULL for user-created items.
  custom_label TEXT,                          -- user's own name; when set, shown as-is, never translated
  icon         TEXT    NOT NULL,              -- Phosphor icon name, e.g. 'flower-lotus'
  sort_order   INTEGER NOT NULL,
  hidden       INTEGER NOT NULL DEFAULT 0 CHECK (hidden IN (0, 1)),
  created_at   TEXT    NOT NULL,
  updated_at   TEXT    NOT NULL,
  deleted_at   TEXT,
  CHECK (label IS NOT NULL OR custom_label IS NOT NULL),
  CHECK (custom_label IS NULL OR length(trim(custom_label)) > 0)
);
CREATE UNIQUE INDEX choice_default_label ON choice(kind, label) WHERE label IS NOT NULL;

-- ---------------------------------------------------------------------------
-- User-populated collections: people, places, things, tags
-- ---------------------------------------------------------------------------
CREATE TABLE subject (
  id         TEXT PRIMARY KEY,
  kind       TEXT NOT NULL CHECK (kind IN ('person', 'place', 'thing', 'tag')),
  name       TEXT NOT NULL CHECK (length(trim(name)) > 0),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT
);

-- ---------------------------------------------------------------------------
-- Links from entries. Rows are never physically removed: unlinking sets deleted_at,
-- re-linking clears it. Composite primary keys make links mergeable across devices.
-- ---------------------------------------------------------------------------
CREATE TABLE entry_choice (
  entry_id   TEXT NOT NULL REFERENCES entry(id),
  choice_id  TEXT NOT NULL REFERENCES choice(id),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT,
  PRIMARY KEY (entry_id, choice_id)
) WITHOUT ROWID;

CREATE TABLE entry_subject (
  entry_id   TEXT NOT NULL REFERENCES entry(id),
  subject_id TEXT NOT NULL REFERENCES subject(id),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT,
  PRIMARY KEY (entry_id, subject_id)
) WITHOUT ROWID;

CREATE TABLE entry_color (
  entry_id   TEXT NOT NULL REFERENCES entry(id),
  color      TEXT NOT NULL CHECK (color IN ('red', 'orange', 'yellow', 'green', 'teal', 'blue',
                                            'purple', 'pink', 'brown', 'gray', 'black', 'white')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT,
  PRIMARY KEY (entry_id, color)
) WITHOUT ROWID;

-- ---------------------------------------------------------------------------
-- Settings that change the meaning of data and must be the same on every device.
-- Not for UI preferences.
-- ---------------------------------------------------------------------------
CREATE TABLE setting (
  key        TEXT PRIMARY KEY CHECK (key IN ('day_end')),
  value      TEXT NOT NULL,
  updated_at TEXT NOT NULL
) WITHOUT ROWID;

-- ---------------------------------------------------------------------------
-- Indexes
-- ---------------------------------------------------------------------------
CREATE INDEX entry_dated      ON entry(dated_at);
CREATE INDEX entry_type_dated ON entry(type, dated_at);
CREATE INDEX entry_updated    ON entry(updated_at);
CREATE INDEX entry_source     ON entry(source_id) WHERE source_id IS NOT NULL;
CREATE INDEX choice_updated   ON choice(updated_at);
CREATE INDEX subject_kind     ON subject(kind, name);
CREATE INDEX subject_updated  ON subject(updated_at);
CREATE INDEX entry_choice_c   ON entry_choice(choice_id);
CREATE INDEX entry_subject_s  ON entry_subject(subject_id);

-- ---------------------------------------------------------------------------
-- Rules by entry type. Named rule_* so later migrations can drop and recreate them.
-- ---------------------------------------------------------------------------
CREATE TRIGGER rule_entry_type_immutable
BEFORE UPDATE OF type ON entry WHEN NEW.type <> OLD.type
BEGIN SELECT RAISE(ABORT, 'rule_entry_type_immutable'); END;

CREATE TRIGGER rule_dream_flags_insert
BEFORE INSERT ON entry
WHEN NEW.type <> 'dream' AND (NEW.lucid IS NOT NULL OR NEW.nightmare IS NOT NULL OR NEW.recurring IS NOT NULL)
BEGIN SELECT RAISE(ABORT, 'rule_dream_flags'); END;

CREATE TRIGGER rule_dream_flags_update
BEFORE UPDATE OF lucid, nightmare, recurring ON entry
WHEN NEW.type <> 'dream' AND (NEW.lucid IS NOT NULL OR NEW.nightmare IS NOT NULL OR NEW.recurring IS NOT NULL)
BEGIN SELECT RAISE(ABORT, 'rule_dream_flags'); END;

CREATE TRIGGER rule_note_no_ratings_insert
BEFORE INSERT ON entry
WHEN NEW.type = 'note' AND (NEW.mood IS NOT NULL OR NEW.energy IS NOT NULL OR NEW.cognitive_load IS NOT NULL OR NEW.sleep IS NOT NULL)
BEGIN SELECT RAISE(ABORT, 'rule_note_no_ratings'); END;

CREATE TRIGGER rule_note_no_ratings_update
BEFORE UPDATE OF mood, energy, cognitive_load, sleep ON entry
WHEN NEW.type = 'note' AND (NEW.mood IS NOT NULL OR NEW.energy IS NOT NULL OR NEW.cognitive_load IS NOT NULL OR NEW.sleep IS NOT NULL)
BEGIN SELECT RAISE(ABORT, 'rule_note_no_ratings'); END;

CREATE TRIGGER rule_source_insert
BEFORE INSERT ON entry WHEN NEW.source_id IS NOT NULL AND (
  NEW.type = 'note' OR (SELECT type FROM entry WHERE id = NEW.source_id) IS NOT 'note')
BEGIN SELECT RAISE(ABORT, 'rule_source'); END;

CREATE TRIGGER rule_source_update
BEFORE UPDATE OF source_id ON entry WHEN NEW.source_id IS NOT NULL AND (
  NEW.type = 'note' OR (SELECT type FROM entry WHERE id = NEW.source_id) IS NOT 'note')
BEGIN SELECT RAISE(ABORT, 'rule_source'); END;

CREATE TRIGGER rule_entry_choice_not_note
BEFORE INSERT ON entry_choice WHEN (SELECT type FROM entry WHERE id = NEW.entry_id) = 'note'
BEGIN SELECT RAISE(ABORT, 'rule_entry_choice_not_note'); END;

CREATE TRIGGER rule_entry_color_not_note
BEFORE INSERT ON entry_color WHEN (SELECT type FROM entry WHERE id = NEW.entry_id) = 'note'
BEGIN SELECT RAISE(ABORT, 'rule_entry_color_not_note'); END;

CREATE TRIGGER rule_entry_subject_note_tags
BEFORE INSERT ON entry_subject
WHEN (SELECT type FROM entry WHERE id = NEW.entry_id) = 'note'
 AND (SELECT kind FROM subject WHERE id = NEW.subject_id) <> 'tag'
BEGIN SELECT RAISE(ABORT, 'rule_entry_subject_note_tags'); END;

CREATE TRIGGER rule_subject_kind_immutable
BEFORE UPDATE OF kind ON subject WHEN NEW.kind <> OLD.kind
BEGIN SELECT RAISE(ABORT, 'rule_subject_kind_immutable'); END;

CREATE TRIGGER rule_choice_label_frozen
BEFORE UPDATE OF label ON choice WHEN NEW.label IS NOT OLD.label
BEGIN SELECT RAISE(ABORT, 'rule_choice_label_frozen'); END;

CREATE TRIGGER rule_choice_kind_immutable
BEFORE UPDATE OF kind ON choice WHEN NEW.kind <> OLD.kind
BEGIN SELECT RAISE(ABORT, 'rule_choice_kind_immutable'); END;

-- ---------------------------------------------------------------------------
-- Default emotions and activities. IDs are fixed and identical in every vault,
-- so two devices seeding independently never create duplicates.
-- ---------------------------------------------------------------------------
INSERT INTO choice (id, kind, label, custom_label, icon, sort_order, hidden, created_at, updated_at, deleted_at) VALUES
  ('01a0e54f-b000-7fb5-a336-339c06c3a42f', 'emotion', 'Joy', NULL, 'sun', 10, 0, '2026-09-28T00:00:00Z', '2026-09-28T00:00:00Z', NULL),
  ('01a0e54f-b001-7559-a22a-ffe581c89d9d', 'emotion', 'Gratitude', NULL, 'hand-heart', 20, 0, '2026-09-28T00:00:00Z', '2026-09-28T00:00:00Z', NULL),
  ('01a0e54f-b002-7f29-bd09-fc096a90db83', 'emotion', 'Calm', NULL, 'flower-lotus', 30, 0, '2026-09-28T00:00:00Z', '2026-09-28T00:00:00Z', NULL),
  ('01a0e54f-b003-78b5-969f-199867984ade', 'emotion', 'Love', NULL, 'heart', 40, 0, '2026-09-28T00:00:00Z', '2026-09-28T00:00:00Z', NULL),
  ('01a0e54f-b004-7fb4-a335-5026cad417e6', 'emotion', 'Pride', NULL, 'medal', 50, 0, '2026-09-28T00:00:00Z', '2026-09-28T00:00:00Z', NULL),
  ('01a0e54f-b005-7fb7-9cd5-737230e26061', 'emotion', 'Hope', NULL, 'sparkle', 60, 0, '2026-09-28T00:00:00Z', '2026-09-28T00:00:00Z', NULL),
  ('01a0e54f-b006-7db0-bd8d-1eaa4d6b64b5', 'emotion', 'Sadness', NULL, 'cloud-rain', 70, 0, '2026-09-28T00:00:00Z', '2026-09-28T00:00:00Z', NULL),
  ('01a0e54f-b007-74d1-988a-2eb50d027ebb', 'emotion', 'Anger', NULL, 'fire', 80, 0, '2026-09-28T00:00:00Z', '2026-09-28T00:00:00Z', NULL),
  ('01a0e54f-b008-70a4-864f-ecdfc2fadf45', 'emotion', 'Anxiety', NULL, 'lightning', 90, 0, '2026-09-28T00:00:00Z', '2026-09-28T00:00:00Z', NULL),
  ('01a0e54f-b009-771d-8b2f-7243f7c1a93f', 'emotion', 'Loneliness', NULL, 'user-minus', 100, 0, '2026-09-28T00:00:00Z', '2026-09-28T00:00:00Z', NULL),
  ('01a0e54f-b00a-71d1-91dd-11163195ed02', 'emotion', 'Boredom', NULL, 'hourglass-medium', 110, 0, '2026-09-28T00:00:00Z', '2026-09-28T00:00:00Z', NULL),
  ('01a0e54f-b00b-798e-af1b-11ef2129c5fd', 'emotion', 'Nostalgia', NULL, 'clock-counter-clockwise', 120, 0, '2026-09-28T00:00:00Z', '2026-09-28T00:00:00Z', NULL),
  ('01a0e54f-b00c-7db7-8e30-cb9d36f00e43', 'activity', 'Work', NULL, 'briefcase', 10, 0, '2026-09-28T00:00:00Z', '2026-09-28T00:00:00Z', NULL),
  ('01a0e54f-b00d-7997-8a7a-1e45368c0bd4', 'activity', 'Exercise', NULL, 'barbell', 20, 0, '2026-09-28T00:00:00Z', '2026-09-28T00:00:00Z', NULL),
  ('01a0e54f-b00e-7e98-8488-c59b5b1e5c36', 'activity', 'Walk', NULL, 'person-simple-walk', 30, 0, '2026-09-28T00:00:00Z', '2026-09-28T00:00:00Z', NULL),
  ('01a0e54f-b00f-7376-b0d5-7169aac16696', 'activity', 'Reading', NULL, 'book-open', 40, 0, '2026-09-28T00:00:00Z', '2026-09-28T00:00:00Z', NULL),
  ('01a0e54f-b010-7a50-9164-445a3e69de85', 'activity', 'Cooking', NULL, 'cooking-pot', 50, 0, '2026-09-28T00:00:00Z', '2026-09-28T00:00:00Z', NULL),
  ('01a0e54f-b011-708d-9ed4-05d688df30ef', 'activity', 'Friends & family', NULL, 'users-three', 60, 0, '2026-09-28T00:00:00Z', '2026-09-28T00:00:00Z', NULL),
  ('01a0e54f-b012-7cf8-a8e2-76fe92463718', 'activity', 'Music', NULL, 'music-notes', 70, 0, '2026-09-28T00:00:00Z', '2026-09-28T00:00:00Z', NULL),
  ('01a0e54f-b013-7e6b-9c40-1c9f21e4734c', 'activity', 'Side project', NULL, 'code', 80, 0, '2026-09-28T00:00:00Z', '2026-09-28T00:00:00Z', NULL),
  ('01a0e54f-b014-7f60-b70f-f04f9b13a162', 'activity', 'Nature', NULL, 'tree', 90, 0, '2026-09-28T00:00:00Z', '2026-09-28T00:00:00Z', NULL),
  ('01a0e54f-b015-7aee-9cf2-c0dc7cb9f319', 'activity', 'Meditation', NULL, 'person-simple-tai-chi', 100, 0, '2026-09-28T00:00:00Z', '2026-09-28T00:00:00Z', NULL),
  ('01a0e54f-b016-7307-b266-54e22ea1518a', 'activity', 'Gaming', NULL, 'game-controller', 110, 0, '2026-09-28T00:00:00Z', '2026-09-28T00:00:00Z', NULL),
  ('01a0e54f-b017-7d3b-a756-3e9a6fa79642', 'activity', 'Travel', NULL, 'airplane', 120, 0, '2026-09-28T00:00:00Z', '2026-09-28T00:00:00Z', NULL),
  ('01a0e54f-b018-7c91-a5ba-b36dd93bd4db', 'activity', 'Screen time', NULL, 'device-mobile', 130, 0, '2026-09-28T00:00:00Z', '2026-09-28T00:00:00Z', NULL),
  ('01a0e54f-b019-7a3c-97cd-61cb08775256', 'activity', 'Chores', NULL, 'broom', 140, 0, '2026-09-28T00:00:00Z', '2026-09-28T00:00:00Z', NULL);;

INSERT INTO setting (key, value, updated_at) VALUES ('day_end', '03:00', '2026-09-28T00:00:00Z');

-- The client inserts the vault row itself, with a newly generated id and the current time:
-- INSERT INTO vault VALUES (1, '<new UUIDv7>', 'quaderno', 1, 1, 1, '<now>');
