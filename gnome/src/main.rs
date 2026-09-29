// SPDX-License-Identifier: GPL-3.0-or-later

mod application;
mod window;

use gtk::prelude::*;

fn main() -> gtk::glib::ExitCode {
    application::build().run()
}
