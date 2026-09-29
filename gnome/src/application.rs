// SPDX-License-Identifier: GPL-3.0-or-later

use gtk::gio;
use gtk::prelude::*;

use crate::window::QuadernoWindow;

/// The application ID; must match the desktop file, metainfo and GSettings
/// schema names. Windows live under `/io/github/stickgrinder/Quaderno`.
pub const APP_ID: &str = "io.github.stickgrinder.Quaderno";

/// Builds the application. `AdwApplication` initialises libadwaita for us.
pub fn build() -> adw::Application {
    let app = adw::Application::builder()
        .application_id(APP_ID)
        .resource_base_path("/io/github/stickgrinder/Quaderno")
        .build();

    app.connect_startup(|_| register_resources());
    app.connect_activate(activate);
    app
}

fn register_resources() {
    // The blob is embedded by `glib-build-tools` at compile time: if it were
    // missing from OUT_DIR the build would have failed, so this cannot fail at
    // runtime.
    gio::resources_register_include!("quaderno.gresource")
        .expect("the compiled GResource is embedded in the binary");
}

fn activate(app: &adw::Application) {
    // Milestone M0 shows a single empty window. The welcome/unlock/main pages
    // and window state persistence arrive in later milestones.
    QuadernoWindow::new(app).present();
}
