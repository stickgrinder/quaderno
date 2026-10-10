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

i18n.merge_file(
  input: 'io.github.stickgrinder.Quaderno.desktop.in',
  output: 'io.github.stickgrinder.Quaderno.desktop',
  po_dir: '../po',
  type: 'desktop',
  install: true,
  install_dir: datadir / 'applications',
)

install_data(
  'icons/hicolor/scalable/apps/io.github.stickgrinder.Quaderno.svg',
  install_dir: datadir / 'icons/hicolor/scalable/apps',
)

# Phosphor Icons are MIT licensed (ui-spec §5); ship the license with the app.
install_data(
  'icons/phosphor/LICENSE',
  install_dir: datadir / 'licenses/quaderno',
)

# Source Serif 4, the bundled Serif writing font (product spec §8). It is
# licensed under the SIL Open Font License 1.1.
install_data(
  'fonts/SourceSerif4-Regular.ttf',
  'fonts/SourceSerif4-It.ttf',
  'fonts/SourceSerif4-Bold.ttf',
  'fonts/SourceSerif4-BoldIt.ttf',
  install_dir: datadir / 'fonts',
)

install_data(
  'fonts/LICENSE.md',
  install_dir: datadir / 'licenses/quaderno/SourceSerif4',
)
