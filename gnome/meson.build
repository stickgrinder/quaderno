# SPDX-License-Identifier: GPL-3.0-or-later

project('quaderno',
  version: '0.1.0',
  meson_version: '>= 1.4',
  license: 'GPL-3.0-or-later',
  default_options: ['warning_level=2'],
)

gnome = import('gnome')
i18n = import('i18n')

bindir = get_option('bindir')
datadir = get_option('datadir')
localedir = get_option('localedir')

cargo = find_program('cargo', required: true)
cargo_build = find_program(meson.project_source_root() / 'build-aux' / 'cargo-build.sh')

# Cargo drives the Rust build; this target produces the `quaderno` binary and
# installs it. `depend_files` makes ninja rebuild when a Rust or UI source
# changes.
cargo_target_dir = meson.current_build_dir() / 'cargo-target'

custom_target('quaderno',
  output: 'quaderno',
  depend_files: files(
    'Cargo.toml',
    'build.rs',
    'data/quaderno.css',
    # The vendored Phosphor SVGs are only ever added as a whole set, so meson
    # does not track the directory; `build.rs` re-converts them whenever cargo
    # is re-run.
    'src/main.rs',
    'src/application.rs',
    'src/details.rs',
    'src/icons.rs',
    'src/keyring.rs',
    'src/picker.rs',
    'src/spell_checking.rs',
    'src/state.rs',
    'src/timeline.rs',
    'src/window.rs',
    'ui/quaderno-window.blp',
    'ui/quaderno.gresource.xml',
  ),
  command: [
    cargo_build,
    cargo,
    meson.project_source_root() / 'Cargo.toml',
    cargo_target_dir,
    '@OUTPUT@',
  ],
  env: ['QUADERNO_LOCALEDIR=' + localedir],
  build_by_default: true,
  install: true,
  install_dir: bindir,
)

subdir('data')
subdir('po')

gnome.post_install(
  glib_compile_schemas: true,
  gtk_update_icon_cache: true,
  update_desktop_database: true,
)
