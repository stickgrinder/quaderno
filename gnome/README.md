# Quaderno for GNOME

The GTK4/libadwaita app (binary `quaderno`, app ID `io.github.stickgrinder.Quaderno`). Version 1.0 is the current target.

| Folder | Contents |
|---|---|
| `src/` | Rust sources of the app crate |
| `ui/` | Blueprint (`.blp`) UI files |
| `data/` | desktop file, metainfo, GSettings schema, icons (Phosphor symbolic set + LICENSE), `icon-synonyms.tsv` |
| `build-aux/` | Flatpak manifest; `aur/` holds the PKGBUILD templates for `quaderno` and `quaderno-git` |

`meson.build` and `Cargo.toml` for the app are created in milestone M0 (`docs/plan.md`). Build and test commands are in `AGENTS.md`.

All vault access goes through `crates/quaderno-vault`; this app never runs SQL itself.
