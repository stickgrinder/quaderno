# Changelog

All notable changes to Quaderno are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[semantic versioning](https://semver.org/): 0.x releases are development
releases.

## [Unreleased]

## [0.1.0] - 2026-10-23

### Added

- A private, encrypted journal stored as one SQLCipher file on your device.
- Journal pages, dreams and quick notes.
- Optional metadata: four ratings, emotions, day activities, colors, people,
  places, things and tags.
- A timeline grouped by journal day, with type filters and in-memory search.
- Collections for people, places, things and tags, with rename, merge and
  safe deletion.
- A details panel with autocomplete token fields and quick-add pickers.
- Preferences for the writing font (bundled Source Serif 4), text size, line
  width, spell checking, default entry type and day end.
- Automatic weekly encrypted backups, "Back up now", and opening a backup from
  the error screen.
- Markdown and JSON export, with a warning that exports are not encrypted.
- Italian translation, including the AppStream metadata.
- Light and dark styles following the system, with no network access of any
  kind.

### Changed

- The repository was flattened: the GNOME app now lives at the repository root
  and the shared vault library stays in `crates/quaderno-vault`.

### Fixed

- The app finds its translations next to the executable, so Flatpak, deb/rpm
  packages and a relocated tarball all load the catalog.
