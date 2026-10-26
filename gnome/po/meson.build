# SPDX-License-Identifier: GPL-3.0-or-later

# The glib preset covers `_()`, `N_()`, `C_()` and the g_* helpers (used by the
# Blueprint UI). The app calls gettext through gettextrs, so the plain
# gettext/pgettext/ngettext keywords are added on top.
quaderno_i18n = i18n.gettext('quaderno',
  preset: 'glib',
  args: [
    '--keyword=gettext',
    '--keyword=pgettext:1c,2',
    '--keyword=ngettext:1,2',
  ],
)
