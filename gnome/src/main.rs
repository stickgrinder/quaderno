// SPDX-License-Identifier: GPL-3.0-or-later

mod application;
mod details;
mod icons;
mod keyring;
mod picker;
mod spell_checking;
mod state;
mod timeline;
mod window;

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
    let localedir = option_env!("QUADERNO_LOCALEDIR").unwrap_or("/app/share/locale");
    let _ = gettextrs::bindtextdomain("quaderno", localedir);
    let _ = gettextrs::textdomain("quaderno");
}
