// SPDX-License-Identifier: GPL-3.0-or-later

//! Application setup: actions, resources and the session-lock listener.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::gio;
use gtk::prelude::*;

use crate::keyring::Keyring;
use crate::window::QuadernoWindow;

/// The application ID. Windows live under `/io/github/stickgrinder/Quaderno`.
pub const APP_ID: &str = "io.github.stickgrinder.Quaderno";

/// Builds the application.
pub fn build() -> adw::Application {
    let app = adw::Application::builder()
        .application_id(APP_ID)
        .resource_base_path("/io/github/stickgrinder/Quaderno")
        .build();

    app.connect_startup(|_| register_resources());

    let window: Rc<RefCell<Option<QuadernoWindow>>> = Rc::new(RefCell::new(None));
    let keyring = Keyring::spawn();

    let lock = gio::SimpleAction::new("lock", None);
    {
        let window = window.clone();
        lock.connect_activate(move |_, _| {
            if let Some(window) = window.borrow().as_ref() {
                window.lock();
            }
        });
    }
    app.add_action(&lock);
    app.set_accels_for_action("app.lock", &["<Control>l"]);

    let quit = gio::SimpleAction::new("quit", None);
    {
        let app = app.clone();
        quit.connect_activate(move |_, _| app.quit());
    }
    app.add_action(&quit);

    {
        let window = window.clone();
        app.connect_activate(move |app| {
            if let Some(existing) = window.borrow().as_ref() {
                existing.present();
                return;
            }
            let created = QuadernoWindow::new(app, keyring.clone());
            window.borrow_mut().replace(created.clone());
            created.present();
            created.start();
        });
    }

    connect_session_lock(window);
    app
}

fn register_resources() {
    // The blob is embedded by `glib-build-tools` at compile time.
    gio::resources_register_include!("quaderno.gresource")
        .expect("the compiled GResource is embedded in the binary");
}

/// Locks the vault when the GNOME session locks (product spec §3.3).
fn connect_session_lock(window: Rc<RefCell<Option<QuadernoWindow>>>) {
    let Ok(connection) = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE) else {
        return;
    };
    let subscription = connection.subscribe_to_signal(
        Some("org.gnome.ScreenSaver"),
        Some("org.gnome.ScreenSaver"),
        Some("ActiveChanged"),
        Some("/org/gnome/ScreenSaver"),
        None,
        gio::DBusSignalFlags::NONE,
        move |signal: gio::DBusSignalRef| {
            let active = signal.parameters.get::<bool>().unwrap_or(false);
            if active {
                if let Some(window) = window.borrow().as_ref() {
                    window.on_session_locked();
                }
            }
        },
    );
    // Keep the subscription (and its connection) for the process lifetime.
    std::mem::forget(subscription);
}
