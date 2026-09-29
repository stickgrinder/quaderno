# Quaderno

A private, encrypted journal for GNOME. Journal pages, dreams and quick notes,
each with optional metadata (ratings, emotions, activities, people, places,
things, tags, colours), stored in a single encrypted file on your device. It is
a native GTK4/libadwaita application; it will never be a web or Electron app.

## Why it exists

Quaderno is built around a simple promise: what you write is nobody else's business.

- **One encrypted file.** The whole journal is a single SQLCipher database, normally `journal.quaderno`. There is no server and no plaintext mode.
- **No network.** The app makes no network requests of any kind: no telemetry,
- **Nothing derived from your writing touches disk unencrypted.** Search indexes are built in memory after unlock and dropped on lock.
- **Losing the passphrase loses the journal.** There is no recovery key, and the app says so when a vault is created.

The file format is public and documented in [`vault-spec/format.md`](vault-spec/format.md) so that other clients can be written against it. The same file works locally, for backups and for file-based sync.

## Status

Version 1.0 is in development. The current milestone and acceptance criteria are in [`docs/plan.md`](docs/plan.md).

## Repository layout

| Path | Contents |
|---|---|
| `crates/quaderno-vault/` | Shared library: everything that touches the vault file. |
| `gnome/` | The GNOME app |
| `vault-spec/` | Public file-format spec and SQL migrations |
| `docs/` | Product specs, statistics, plan |
| `android/` | Android companion app (TBD) |

## Building and running

### Gnome application

#### Prerequisites

You need a Rust stable toolchain (edition 2024, Rust 1.85+), meson, ninja,
blueprint-compiler, and the GTK 4.18+ / libadwaita 1.7+ development libraries.
On Arch/Manjaro:

```sh
sudo pacman -S rust meson ninja blueprint-compiler gtk4 libadwaita \
  gettext desktop-file-utils
```

Other distributions need the equivalent packages; the minimum library versions
are GTK 4.18 and libadwaita 1.7.

#### Development build

The meson project lives in `gnome/` and drives cargo:

```sh
meson setup build gnome
meson compile -C build
meson devenv -C build ./quaderno
```

#### Flatpak (the way the app ships)

Install `flatpak` and `flatpak-builder`, then the GNOME 50 SDK and the Rust SDK
extension:

```sh
sudo pacman -S flatpak flatpak-builder
flatpak install flathub org.gnome.Platform//50 org.gnome.Sdk//50 \
  org.freedesktop.Sdk.Extension.rust-stable//25.08
```

Build and install it, then run it:

```sh
flatpak-builder --user --install --force-clean build-flatpak \
  gnome/build-aux/io.github.stickgrinder.Quaderno.json
flatpak run io.github.stickgrinder.Quaderno
```

Cargo dependencies are vendored for the sandboxed build; see
`gnome/build-aux/cargo-sources.json`.

## Testing and checks

### Gnome application

All of these must pass before a milestone counts as done:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
meson test -C build
```

The GNOME app is also built in CI inside the Flatpak SDK; the vault library is
checked separately. Workflows live in `.github/workflows/`.

## Documentation

> TBD

## License

GPL-3.0-or-later. See [`LICENSE`](LICENSE).  
Quaderno's file format document is licensed under CC-BY-SA-4.0.
