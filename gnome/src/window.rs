// SPDX-License-Identifier: GPL-3.0-or-later

//! The main window: a stack of welcome, unlock, error and main pages, and the
//! logic that moves between them.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gio, glib};
use quaderno_vault::{Access, Vault, VaultError};
use zeroize::Zeroizing;

use crate::keyring::Keyring;
use crate::state::{CreateForm, CreateValidity, Phase, Throttle};

fn quaderno_settings() -> gio::Settings {
    gio::Settings::new(crate::application::APP_ID)
}

mod imp {
    use super::*;

    #[derive(gtk::CompositeTemplate)]
    #[template(resource = "/io/github/stickgrinder/Quaderno/quaderno-window.ui")]
    pub struct QuadernoWindow {
        pub settings: gio::Settings,
        pub keyring: RefCell<Option<Keyring>>,
        pub vault: RefCell<Option<Vault>>,
        pub path: RefCell<Option<PathBuf>>,
        pub throttle: RefCell<Throttle>,
        pub last_activity: Cell<Instant>,
        pub idle_source: RefCell<Option<glib::SourceId>>,

        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub passphrase_row: TemplateChild<adw::PasswordEntryRow>,
        #[template_child]
        pub repeat_row: TemplateChild<adw::PasswordEntryRow>,
        #[template_child]
        pub strength_bar: TemplateChild<gtk::LevelBar>,
        #[template_child]
        pub strength_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub location_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub change_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub remember_row: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub confirm_check: TemplateChild<gtk::CheckButton>,
        #[template_child]
        pub create_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub open_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub unlock_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub unlock_entry: TemplateChild<adw::PasswordEntryRow>,
        #[template_child]
        pub unlock_error: TemplateChild<gtk::Label>,
        #[template_child]
        pub unlock_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub error_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub locate_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub main_banner: TemplateChild<adw::Banner>,
        #[template_child]
        pub menu_button: TemplateChild<gtk::MenuButton>,
    }

    impl Default for QuadernoWindow {
        fn default() -> Self {
            Self {
                settings: quaderno_settings(),
                keyring: RefCell::new(None),
                vault: RefCell::new(None),
                path: RefCell::new(None),
                throttle: RefCell::new(Throttle::new()),
                last_activity: Cell::new(Instant::now()),
                idle_source: RefCell::new(None),
                stack: Default::default(),
                passphrase_row: Default::default(),
                repeat_row: Default::default(),
                strength_bar: Default::default(),
                strength_label: Default::default(),
                location_row: Default::default(),
                change_button: Default::default(),
                remember_row: Default::default(),
                confirm_check: Default::default(),
                create_button: Default::default(),
                open_button: Default::default(),
                unlock_page: Default::default(),
                unlock_entry: Default::default(),
                unlock_error: Default::default(),
                unlock_button: Default::default(),
                error_page: Default::default(),
                locate_button: Default::default(),
                main_banner: Default::default(),
                menu_button: Default::default(),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for QuadernoWindow {
        const NAME: &'static str = "QuadernoWindow";
        type Type = super::QuadernoWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for QuadernoWindow {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.setup_callbacks();
            obj.setup_menu();
            obj.setup_activity();
        }
    }

    impl WidgetImpl for QuadernoWindow {}
    impl WindowImpl for QuadernoWindow {}
    impl ApplicationWindowImpl for QuadernoWindow {}
    impl adw::subclass::application_window::AdwApplicationWindowImpl for QuadernoWindow {}
    impl adw::subclass::window::AdwWindowImpl for QuadernoWindow {}
}

glib::wrapper! {
    pub struct QuadernoWindow(ObjectSubclass<imp::QuadernoWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow, adw::Window,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget,
                    gtk::Native, gtk::Root, gtk::ShortcutManager,
                    gio::ActionGroup, gio::ActionMap;
}

impl QuadernoWindow {
    pub fn new(app: &adw::Application, keyring: Keyring) -> Self {
        let window: Self = glib::Object::builder().property("application", app).build();
        window.imp().keyring.replace(Some(keyring));
        window
    }

    fn keyring(&self) -> Option<Keyring> {
        self.imp().keyring.borrow().clone()
    }

    fn setup_menu(&self) {
        let menu = gio::Menu::new();
        menu.append(Some(&gettextrs::gettext("Lock")), Some("app.lock"));
        menu.append(Some(&gettextrs::gettext("Quit")), Some("app.quit"));
        self.imp().menu_button.set_menu_model(Some(&menu));
    }

    fn setup_callbacks(&self) {
        let imp = self.imp();

        for row in [&imp.passphrase_row, &imp.repeat_row] {
            let window = self.clone();
            row.connect_changed(move |_| window.refresh_create_form());
        }
        let window = self.clone();
        imp.confirm_check
            .connect_toggled(move |_| window.refresh_create_form());
        let window = self.clone();
        imp.create_button
            .connect_clicked(move |_| window.create_vault());
        let window = self.clone();
        imp.open_button
            .connect_clicked(move |_| window.choose_file());
        let window = self.clone();
        imp.change_button
            .connect_clicked(move |_| window.choose_location());
        let window = self.clone();
        imp.unlock_button.connect_clicked(move |_| window.unlock());
        let window = self.clone();
        imp.unlock_entry
            .connect_entry_activated(move |_| window.unlock());
        let window = self.clone();
        imp.locate_button
            .connect_clicked(move |_| window.choose_file());

        self.refresh_create_form();
    }

    fn setup_activity(&self) {
        let motion = gtk::EventControllerMotion::new();
        let window = self.clone();
        motion.connect_motion(move |_, _, _| window.note_activity());
        self.add_controller(motion);

        let keys = gtk::EventControllerKey::new();
        let window = self.clone();
        keys.connect_key_pressed(move |_, _, _, _| {
            window.note_activity();
            glib::Propagation::Proceed
        });
        self.add_controller(keys);
    }

    fn note_activity(&self) {
        self.imp().last_activity.set(Instant::now());
    }

    /// Decides the first page to show.
    pub fn start(&self) {
        let path = self.imp().settings.string("vault-path").to_string();
        if path.is_empty() {
            self.show_welcome();
            return;
        }
        let path = PathBuf::from(&path);
        self.imp().path.replace(Some(path.clone()));
        if !path.exists() {
            self.show_error(&gettextrs::gettext("The journal file could not be found."));
            return;
        }
        self.try_keyring_then_unlock(path);
    }

    fn show_phase(&self, phase: Phase) {
        let name = match phase {
            Phase::Welcome => "welcome",
            Phase::Unlock => "unlock",
            Phase::Error => "error",
            Phase::Main => "main",
        };
        self.imp().stack.set_visible_child_name(name);
    }

    fn show_welcome(&self) {
        let location = glib::user_data_dir()
            .join("quaderno")
            .join("journal.quaderno");
        self.imp()
            .location_row
            .set_subtitle(&location.to_string_lossy());
        self.show_phase(Phase::Welcome);
    }

    fn show_unlock(&self) {
        if let Some(path) = self.imp().path.borrow().as_ref() {
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            self.imp().unlock_page.set_description(Some(&name));
        }
        self.imp().unlock_entry.set_text("");
        self.imp().unlock_error.set_visible(false);
        self.set_default_widget(Some(&*self.imp().unlock_button));
        self.show_phase(Phase::Unlock);
        self.imp().unlock_entry.grab_focus();
    }

    fn show_error(&self, message: &str) {
        self.imp().error_page.set_description(Some(message));
        self.show_phase(Phase::Error);
    }

    fn show_main(&self, access: Access) {
        self.imp()
            .main_banner
            .set_revealed(access == Access::ReadOnly);
        self.show_phase(Phase::Main);
        self.start_idle_timer();
    }

    fn refresh_create_form(&self) {
        let imp = self.imp();
        let form = CreateForm {
            passphrase: imp.passphrase_row.text().to_string(),
            repeat: imp.repeat_row.text().to_string(),
            confirmed: imp.confirm_check.is_active(),
        };
        imp.create_button.set_sensitive(form.can_create());

        let strength = form.strength();
        imp.strength_bar.set_value(strength);
        let message = match form.validity() {
            CreateValidity::Empty => String::new(),
            CreateValidity::TooShort => gettextrs::gettext("At least eight characters"),
            CreateValidity::Mismatch => gettextrs::gettext("The passphrases do not match"),
            CreateValidity::Ready if strength < 0.4 => gettextrs::gettext("Weak passphrase"),
            CreateValidity::Ready if strength < 0.75 => gettextrs::gettext("Fair passphrase"),
            CreateValidity::Ready => gettextrs::gettext("Strong passphrase"),
        };
        imp.strength_label.set_label(&message);
    }

    fn location(&self) -> PathBuf {
        let subtitle = self
            .imp()
            .location_row
            .subtitle()
            .map(|subtitle| subtitle.to_string())
            .unwrap_or_default();
        PathBuf::from(subtitle)
    }

    fn create_vault(&self) {
        let imp = self.imp();
        let form = CreateForm {
            passphrase: imp.passphrase_row.text().to_string(),
            repeat: imp.repeat_row.text().to_string(),
            confirmed: imp.confirm_check.is_active(),
        };
        if !form.can_create() {
            return;
        }
        let path = self.location();
        let passphrase = form.passphrase;
        let remember = imp.remember_row.is_active();

        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let window = self.clone();
        glib::spawn_future_local(async move {
            let outcome = gio::spawn_blocking({
                let path = path.clone();
                let passphrase = passphrase.clone();
                move || Vault::create(&path, &passphrase)
            })
            .await;

            match outcome {
                Ok(Ok(vault)) => {
                    window.adopt_vault(path, vault, remember.then_some(passphrase));
                }
                Ok(Err(error)) => window.show_error(&format!("{error}")),
                Err(_) => {
                    window.show_error(&gettextrs::gettext("The journal could not be created."))
                }
            }
        });
    }

    fn choose_file(&self) {
        let dialog = gtk::FileDialog::builder()
            .title(gettextrs::gettext("Open Existing Journal…"))
            .build();
        let window = self.clone();
        dialog.open(Some(self), gio::Cancellable::NONE, move |result| {
            if let Ok(file) = result {
                if let Some(path) = file.path() {
                    window.begin_open(path);
                }
            }
        });
    }

    fn choose_location(&self) {
        let dialog = gtk::FileDialog::builder()
            .title(gettextrs::gettext("Choose a location"))
            .initial_name("journal.quaderno")
            .build();
        let window = self.clone();
        dialog.save(Some(self), gio::Cancellable::NONE, move |result| {
            if let Ok(file) = result {
                if let Some(path) = file.path() {
                    window
                        .imp()
                        .location_row
                        .set_subtitle(&path.to_string_lossy());
                }
            }
        });
    }

    fn begin_open(&self, path: PathBuf) {
        if !path.exists() {
            self.show_error(&gettextrs::gettext("The journal file could not be found."));
            return;
        }
        let _ = self
            .imp()
            .settings
            .set_string("vault-path", &path.to_string_lossy());
        self.imp().path.replace(Some(path.clone()));
        self.try_keyring_then_unlock(path);
    }

    fn try_keyring_then_unlock(&self, path: PathBuf) {
        let Some(keyring) = self.keyring() else {
            self.show_unlock();
            return;
        };
        let receiver = keyring.lookup(path.to_string_lossy().into_owned());
        let window = self.clone();
        glib::spawn_future_local(async move {
            let stored = match receiver.await {
                Ok(Ok(Some(bytes))) => String::from_utf8_lossy(&bytes).into_owned(),
                _ => {
                    window.show_unlock();
                    return;
                }
            };
            window.open_with(path, stored, true);
        });
    }

    fn unlock(&self) {
        if !self.imp().throttle.borrow().is_allowed(self.elapsed()) {
            return;
        }
        let Some(path) = self.imp().path.borrow().clone() else {
            return;
        };
        let passphrase = self.imp().unlock_entry.text().to_string();
        if passphrase.is_empty() {
            return;
        }
        self.imp().unlock_error.set_visible(false);
        self.open_with(path, passphrase, false);
    }

    fn open_with(&self, path: PathBuf, passphrase: String, from_keyring: bool) {
        let window = self.clone();
        glib::spawn_future_local(async move {
            let opened = gio::spawn_blocking({
                let path = path.clone();
                let passphrase = passphrase.clone();
                move || Vault::open(&path, &passphrase)
            })
            .await;

            match opened {
                Ok(Ok(vault)) => {
                    if from_keyring {
                        window.imp().throttle.borrow_mut().reset();
                    }
                    window.adopt_vault(path, vault, None);
                }
                Ok(Err(VaultError::Refused { .. })) => {
                    window.show_error(&gettextrs::gettext(
                        "This journal was changed by a newer version of Quaderno.",
                    ));
                }
                Ok(Err(error)) => {
                    if from_keyring {
                        // The remembered passphrase no longer works; ask.
                        window.imp().throttle.borrow_mut().reset();
                        window.show_unlock();
                    } else {
                        window.unlock_failed(&format!("{error}"));
                    }
                }
                Err(_) => {
                    window.show_error(&gettextrs::gettext("The journal could not be opened."))
                }
            }
        });
    }

    fn unlock_failed(&self, message: &str) {
        let now = self.elapsed();
        let blocked = self.imp().throttle.borrow_mut().record_failure(now);
        let text = if blocked {
            gettextrs::gettext("Too many attempts. Wait 30 seconds and try again.")
        } else {
            format!("{} {message}", gettextrs::gettext("Wrong passphrase."),)
        };
        self.imp().unlock_error.set_label(&text);
        self.imp().unlock_error.set_visible(true);
    }

    fn adopt_vault(&self, path: PathBuf, vault: Vault, remember: Option<String>) {
        let access = vault.access();
        let _ = self
            .imp()
            .settings
            .set_string("vault-path", &path.to_string_lossy());
        self.imp().path.replace(Some(path.clone()));
        self.imp().vault.replace(Some(vault));

        if let (Some(passphrase), Some(keyring)) = (remember, self.keyring()) {
            let receiver = keyring.store(
                path.to_string_lossy().into_owned(),
                Zeroizing::new(passphrase.into_bytes()),
            );
            glib::spawn_future_local(async move {
                let _ = receiver.await;
            });
        }
        self.show_main(access);
    }

    /// Locks the vault: closes it and shows the unlock screen.
    pub fn lock(&self) {
        if self.imp().vault.borrow_mut().take().is_none() {
            return; // nothing open
        }
        self.stop_idle_timer();
        self.show_unlock();
    }

    /// Called when the session locks: locks the vault unless the setting is off.
    pub fn on_session_locked(&self) {
        if self.imp().settings.boolean("lock-on-session-lock") {
            self.lock();
        }
    }

    fn elapsed(&self) -> Duration {
        // A monotonic clock. `Instant` cannot be constructed from a value, so
        // the throttle counts from process start.
        static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
        let start = *START.get_or_init(Instant::now);
        start.elapsed()
    }

    fn start_idle_timer(&self) {
        self.stop_idle_timer();
        self.note_activity();
        let window = self.clone();
        let source = glib::timeout_add_seconds_local(15, move || {
            window.check_idle();
            glib::ControlFlow::Continue
        });
        self.imp().idle_source.replace(Some(source));
    }

    fn stop_idle_timer(&self) {
        if let Some(source) = self.imp().idle_source.borrow_mut().take() {
            source.remove();
        }
    }

    fn check_idle(&self) {
        if self.imp().vault.borrow().is_none() {
            return;
        }
        let minutes = self.imp().settings.int("auto-lock-minutes");
        if minutes <= 0 {
            return;
        }
        let idle = self.imp().last_activity.get().elapsed();
        if idle >= Duration::from_secs(minutes as u64 * 60) {
            self.lock();
        }
    }
}
