# Quaderno — instructions for coding agents

Quaderno is a private, encrypted personal journal for GNOME: journal pages, dreams and quick notes, each with optional metadata (ratings, emotions, activities, people, places, things, tags, colors). It is a native GTK4/libadwaita application. It will never be a web or Electron application.

Read this file completely before starting any task. Then read the documents listed under "Sources of truth" that your task touches.

## Sources of truth

| Document | What it defines |
|---|---|
| `vault-spec/format.md` | The encrypted file format. Public: other people build clients from it. |
| `vault-spec/migrations/*.sql` | The schema. If it disagrees with `format.md`, the SQL wins and the document has a bug: report it. |
| `docs/product-spec.md` | What Quaderno does, as rules, for every platform. Version 1 scope. |
| `gnome/docs/ui-spec.md` | GNOME app: screens, widgets, layouts, shortcuts, states. (Android gets its own `android/docs/ui-spec.md`.) |
| `docs/statistics.md` | Insights definitions (version 1.1). |
| `docs/plan.md` | Milestones and acceptance criteria. Work on one milestone at a time. |
| `gnome/docs/design/*.png` | GNOME design artboards. Visual reference only; the HTML they came from is not code to port. |

If a task needs a decision these documents don't make, **stop and ask**. Don't invent product behavior.

## Stack

- **Language:** Rust, stable toolchain, edition 2024.
- **UI:** `gtk4` and `libadwaita` crates (gtk-rs). Minimum GTK 4.18, libadwaita 1.7. Don't use APIs newer than these.
- **UI definitions:** Blueprint (`.blp`) files compiled by `blueprint-compiler`, loaded from a GResource. Widgets are composite templates (`#[derive(CompositeTemplate)]`).
- **Editor:** GtkSourceView 5 (`sourceview5` crate) with the Markdown language.
- **Database:** `rusqlite` with SQLCipher. Cargo feature `bundled-sqlcipher` (Flatpak) or `system-sqlcipher` (distribution packages, e.g. AUR). Default: bundled.
- **Secrets:** `oo7` (Secret Service, with the Flatpak portal).
- **Other crates:** `uuid` (v7), `unicode-normalization`, `jiff` or `chrono` for time (pick one and use it everywhere), `thiserror` in libraries, `anyhow` only in the binary, `gettext-rs`.
- **Build:** meson (drives cargo), plus a Flatpak manifest against the current stable `org.gnome.Platform`.
- **Icons:** Phosphor Icons 2.1, regular weight (MIT), converted to GTK symbolic icons at build time (see `gnome/docs/ui-spec.md` §Icons).
- **License:** GPL-3.0-or-later for all code. Every source file starts with an SPDX header: `// SPDX-License-Identifier: GPL-3.0-or-later`.
- **App ID:** `io.github.stickgrinder.Quaderno`.

## Repository layout

A monorepo: the shared vault library and spec, plus one folder per app. Each app keeps its own build files inside its folder; the root holds only the Cargo workspace and shared material.

```
AGENTS.md            this file (CLAUDE.md imports it)
Cargo.toml           Cargo workspace: crates/* and gnome
vault-spec/          public format spec + migrations (CC-BY-SA-4.0); the vault crate embeds the SQL with include_str!
crates/
  quaderno-vault/      shared library: everything that touches the vault file. No GTK, no Android.
  quaderno-vault-ffi/  Kotlin/Swift bindings via uniffi. Added when Android work starts; not in 1.0.
po/                  shared gettext catalogs (LINGUAS starts with "it"). Default-list labels live here
                     too, so Android can convert the same translations (vault spec §6.2).
gnome/               the GNOME app (binary `quaderno`)
  meson.build        meson project for the app; drives cargo
  src/
  ui/                Blueprint files
  data/              desktop file, metainfo, GSettings schema, icons, icon synonyms
  build-aux/         Flatpak manifest, aur/ PKGBUILD templates
  docs/              GNOME-specific docs: ui-spec.md, design/ PNGs
android/             the Android app (later). Gradle project, with its own docs/ (ui-spec.md, design/). Empty in 1.0 except README.md.
docs/                shared docs: product spec, statistics, plan
```

**Releases** are tagged per component: `vault-vX.Y.Z` (library and spec), `gnome-vX.Y.Z`, `android-vX.Y.Z`.

**CI** has one workflow per component, triggered by paths: `gnome/**` runs the GNOME build, `android/**` the Android build; changes under `crates/**`, `vault-spec/**` or `po/**` run every workflow that depends on them.

## Commands

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
meson setup build gnome && meson compile -C build && meson test -C build
flatpak-builder --user --install --force-clean build-flatpak gnome/build-aux/io.github.stickgrinder.Quaderno.json
```

All of these must pass before a milestone counts as done. Android commands will be added with the Android app.

## Hard rules

These come from the product's core promise: privacy. Breaking one is a bug, whatever the reason.

1. **Nothing derived from journal content is ever written to disk unencrypted.** That covers search indexes, caches, thumbnails, temp files, logs, crash reports and exports made without explicit user confirmation. Search indexes live in memory.
2. **No network code.** The app makes no network requests of any kind: no telemetry, no update checks, no remote fonts or icons. Sync (version 1.1) works only on files.
3. **Logs never contain entry content, names, labels or passphrases.** Log IDs and counts only.
4. **All vault access goes through `quaderno-vault`.** No app ever runs SQL directly or reimplements vault logic; Android will use the same crate through `quaderno-vault-ffi`.
5. **Schema changes are migrations.** Never edit a migration once a release has shipped it. A schema change means a new `vault-spec/migrations/NNNN_*.sql`, the matching update to `vault-spec/format.md`, and a changelog line there.
6. **Respect the spec's rules for writers:** SQLCipher parameters set explicitly, NFC text, canonical timestamps, soft deletes only, `updated_at` on every edit, edits as `UPDATE` of named columns (never delete-and-reinsert), unknown columns preserved.
7. **Passphrases and keys** are zeroized after use (`zeroize` crate) and never stored except through `oo7`.

## Conventions

- Follow the GNOME Human Interface Guidelines. Prefer stock libadwaita widgets over custom ones; if you think a custom widget is needed, say why in the commit.
- Every user-visible string goes through gettext. Default-list labels from the vault use `pgettext(kind, label)` (see `vault-spec/format.md` §6.2).
- Accessibility: every icon-only button has a tooltip and an accessible label. Keyboard access to every action.
- No `unwrap()`/`expect()` outside tests, except for invariants that are truly impossible, each with a comment saying why.
- Blocking vault work runs off the main thread (`gio::spawn_blocking` or a dedicated worker). The UI never freezes on a save.
- Write tests with the code. The vault crate aims for full coverage of the spec's rules; the app has tests for view-model logic.
- Commits: small, one concern each, imperative subject line.

## How to work

1. Pick up the next milestone in `docs/plan.md`. Read its acceptance criteria first.
2. Read the relevant parts of the specs. If something is ambiguous or missing, ask before writing code.
3. Build in small steps; run the commands above often.
4. When a milestone is done: all checks pass, the acceptance criteria are demonstrably met, and `docs/plan.md` has its box ticked with a one-line note of anything deferred.
5. If you change behavior described in a doc, update the doc in the same commit.
