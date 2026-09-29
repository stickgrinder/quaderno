# SPDX-License-Identifier: GPL-3.0-or-later

gnome.compile_schemas(
  depend_files: files('io.github.stickgrinder.Quaderno.gschema.xml'),
)

install_data(
  'io.github.stickgrinder.Quaderno.gschema.xml',
  install_dir: datadir / 'glib-2.0/schemas',
)

install_data(
  'io.github.stickgrinder.Quaderno.metainfo.xml',
  install_dir: datadir / 'metainfo',
)

# Installed as-is for now; translated through `i18n.merge_file` once the
# gettext catalogs are wired up.
install_data(
  'io.github.stickgrinder.Quaderno.desktop.in',
  install_dir: datadir / 'applications',
  rename: 'io.github.stickgrinder.Quaderno.desktop',
)

install_data(
  'icons/hicolor/scalable/apps/io.github.stickgrinder.Quaderno.svg',
  install_dir: datadir / 'icons/hicolor/scalable/apps',
)
