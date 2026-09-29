# Quaderno for GNOME

The GTK4/libadwaita app (binary `quaderno`, app ID `io.github.stickgrinder.Quaderno`). Version 1.0 is the current target.

| Folder | Contents |
|---|---|
| `src/` | Rust sources of the app crate |
| `ui/` | Blueprint (`.blp`) UI files |
| `data/` | desktop file, metainfo, GSettings schema, icons (Phosphor symbolic set + LICENSE), `icon-synonyms.tsv` |
| `po/` | gettext catalogs (`LINGUAS` starts with `it`), including the default-list labels |
| `docs/` | GNOME UI spec and design PNGs |
| `build-aux/` | Flatpak manifest (`io.github.stickgrinder.Quaderno.json`) plus its `cargo-sources.json`; `aur/` holds the PKGBUILD templates for `quaderno` and `quaderno-git` |

`meson.build` and `Cargo.toml` drive the build from the repo root workspace.

All vault access goes through `crates/quaderno-vault`; this app never runs SQL itself.
