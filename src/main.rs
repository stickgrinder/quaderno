// SPDX-License-Identifier: GPL-3.0-or-later

mod application;
mod backups;
mod details;
mod editor_view;
mod export;
mod icon_search;
mod icons;
mod keyring;
mod picker;
mod spell_checking;
mod state;
mod timeline;
mod window;

use std::path::PathBuf;

use gtk::prelude::*;

fn main() -> gtk::glib::ExitCode {
    setup_gettext();
    application::build().run()
}

/// Binds the gettext domain used by the `_()` strings in the Blueprint files.
fn setup_gettext() {
    // SAFETY: `setlocale` mutates process-global locale state; calling it once
    // at startup, before any threads, is the documented gettext setup.
    unsafe {
        gettextrs::setlocale(gettextrs::LocaleCategory::LcAll, "");
    }
    let _ = gettextrs::bindtextdomain("quaderno", runtime_localedir());
    let _ = gettextrs::textdomain("quaderno");
}

/// Prefers the catalogs next to the binary (`<prefix>/share/locale`), so the
/// Flatpak, the deb/rpm and a relocated tarball all find their translations;
/// otherwise uses the directory baked in by meson at build time.
fn runtime_localedir() -> PathBuf {
    if let Ok(executable) = std::env::current_exe()
        && let Some(bin_dir) = executable.parent()
    {
        let candidate = bin_dir.join("../share/locale");
        if candidate.is_dir() {
            return candidate;
        }
    }
    PathBuf::from(option_env!("QUADERNO_LOCALEDIR").unwrap_or("/app/share/locale"))
}
