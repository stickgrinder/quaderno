// SPDX-License-Identifier: GPL-3.0-or-later

//! The main window: a stack of welcome, unlock, error and main pages, and the
//! logic that moves between them.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gio, glib};
use quaderno_vault::{
    Access, Choice, ChoiceKind, Color, DreamFlag, Entry, EntryType, Rating, Subject, SubjectKind,
    Vault, VaultError,
};
use sourceview5::prelude::*;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::details;
use crate::icon_search;
use crate::icons;
use crate::keyring::Keyring;
use crate::picker;
use crate::state::{CreateForm, CreateValidity, Phase, Throttle};
use crate::timeline::{self, Filter};

fn quaderno_settings() -> gio::Settings {
    gio::Settings::new(crate::application::APP_ID)
}

/// The width the font preview would have at the "wide" line width; the other
/// choices scale it by the same fraction as the editor.
const PREVIEW_WIDTH: f64 = 520.0;

/// A callback that rebuilds a preferences list from the vault.
type Refresh = Rc<dyn Fn()>;
/// A self-referential holder, so a list's refresh can hand itself to its rows.
type RefreshCell = Rc<RefCell<Option<Refresh>>>;

/// The kind of chip whose remove button is being built.
#[derive(Clone, Copy)]
enum ChipKind {
    Choice,
    Subject,
}

/// One rating row: its five toggle buttons plus the row it lives in.
pub struct RatingControl {
    rating: Rating,
    row: adw::ActionRow,
    buttons: Vec<gtk::ToggleButton>,
}

/// One choice chip field (emotions or day activities) and its Add popover.
pub struct ChoiceControl {
    kind: ChoiceKind,
    section: gtk::Box,
    flow: gtk::FlowBox,
    add_button: gtk::Button,
    popover: gtk::Popover,
}

/// One token field (people, places, things, tags) and its suggestion popover.
pub struct TokenControl {
    kind: SubjectKind,
    row: gtk::Box,
    wrap: adw::WrapBox,
    entry: gtk::Text,
    popover: gtk::Popover,
}

/// The widgets of the constructed details panel.
pub struct PanelWidgets {
    date_button: gtk::Button,
    dates_label: gtk::Label,
    felt_group: adw::PreferencesGroup,
    ratings: Vec<RatingControl>,
    choices: Vec<ChoiceControl>,
    colors_section: gtk::Box,
    colors: Vec<(Color, gtk::ToggleButton)>,
    subjects_section: gtk::Box,
    subjects_title: gtk::Label,
    tokens: Vec<TokenControl>,
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
        #[template_child]
        pub main_split: TemplateChild<adw::OverlaySplitView>,
        #[template_child]
        pub navigation_split: TemplateChild<adw::NavigationSplitView>,
        #[template_child]
        pub sidebar_toggle: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub collection_back: TemplateChild<gtk::Button>,
        #[template_child]
        pub new_button: TemplateChild<adw::SplitButton>,
        #[template_child]
        pub search_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub search_bar: TemplateChild<gtk::SearchBar>,
        #[template_child]
        pub search_entry: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub filter_group: TemplateChild<adw::ToggleGroup>,
        #[template_child]
        pub notes_filter_group: TemplateChild<adw::ToggleGroup>,
        #[template_child]
        pub list_page: TemplateChild<adw::NavigationPage>,
        #[template_child]
        pub list_title: TemplateChild<adw::WindowTitle>,
        #[template_child]
        pub sections_list: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub entry_list: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub editor_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub type_badge: TemplateChild<gtk::Label>,
        #[template_child]
        pub flags_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub flag_lucid: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub flag_nightmare: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub flag_recurring: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub date_button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub date_calendar: TemplateChild<gtk::Calendar>,
        #[template_child]
        pub time_entry: TemplateChild<gtk::Entry>,
        #[template_child]
        pub today_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub yesterday_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub save_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub expand_button: TemplateChild<adw::SplitButton>,
        #[template_child]
        pub backdated_banner: TemplateChild<adw::Banner>,
        #[template_child]
        pub source_banner: TemplateChild<adw::Banner>,
        #[template_child]
        pub expanded_card: TemplateChild<gtk::Box>,
        #[template_child]
        pub note_info_card: TemplateChild<gtk::Box>,
        #[template_child]
        pub editor_scroll: TemplateChild<gtk::ScrolledWindow>,
        #[template_child]
        pub footer_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub heading_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub bold_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub italic_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub list_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub quote_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub link_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub entry_menu: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub details_split: TemplateChild<adw::OverlaySplitView>,
        #[template_child]
        pub details_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub details_close: TemplateChild<gtk::Button>,
        #[template_child]
        pub details_box: TemplateChild<gtk::Box>,
        // Non-template state.
        pub editor_buffer: RefCell<Option<sourceview5::Buffer>>,
        pub editor_view: RefCell<Option<sourceview5::View>>,
        pub spell_adapter: RefCell<Option<libspelling::TextBufferAdapter>>,
        pub editor_css: RefCell<Option<gtk::CssProvider>>,
        pub editor_clamp: RefCell<Option<adw::ClampScrollable>>,
        pub preview_clamp: RefCell<Option<adw::Clamp>>,
        pub read_only: Cell<bool>,
        pub selected: RefCell<Option<uuid::Uuid>>,
        pub current_dated: RefCell<Option<jiff::Zoned>>,
        pub current_created: RefCell<Option<jiff::Timestamp>>,
        pub current_type: RefCell<Option<EntryType>>,
        pub source_note: Cell<Option<Uuid>>,
        pub note_filter: Cell<timeline::NoteFilter>,
        pub dirty: Cell<bool>,
        pub loading: Cell<bool>,
        pub save_source: RefCell<Option<glib::SourceId>>,
        pub row_ids: RefCell<Vec<Option<uuid::Uuid>>>,
        pub section_filters: RefCell<Vec<(u32, Filter)>>,
        pub collection_filters: RefCell<Vec<(u32, SubjectKind)>>,
        pub collection_kind: Cell<Option<SubjectKind>>,
        pub collection_subject: RefCell<Option<Uuid>>,
        pub open_popovers: RefCell<Vec<gtk::Popover>>,
        pub preferences: RefCell<Option<adw::PreferencesDialog>>,
        pub search_index: RefCell<Option<crate::timeline::SearchIndex>>,
        pub panel: RefCell<Option<PanelWidgets>>,
        pub panel_loading: Cell<bool>,
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
                main_split: Default::default(),
                navigation_split: Default::default(),
                sidebar_toggle: Default::default(),
                collection_back: Default::default(),
                new_button: Default::default(),
                search_button: Default::default(),
                search_bar: Default::default(),
                search_entry: Default::default(),
                filter_group: Default::default(),
                notes_filter_group: Default::default(),
                list_page: Default::default(),
                list_title: Default::default(),
                sections_list: Default::default(),
                entry_list: Default::default(),
                editor_stack: Default::default(),
                type_badge: Default::default(),
                flags_box: Default::default(),
                flag_lucid: Default::default(),
                flag_nightmare: Default::default(),
                flag_recurring: Default::default(),
                date_button: Default::default(),
                date_calendar: Default::default(),
                time_entry: Default::default(),
                today_button: Default::default(),
                yesterday_button: Default::default(),
                save_label: Default::default(),
                expand_button: Default::default(),
                backdated_banner: Default::default(),
                source_banner: Default::default(),
                expanded_card: Default::default(),
                note_info_card: Default::default(),
                editor_scroll: Default::default(),
                footer_label: Default::default(),
                heading_button: Default::default(),
                bold_button: Default::default(),
                italic_button: Default::default(),
                list_button: Default::default(),
                quote_button: Default::default(),
                link_button: Default::default(),
                editor_buffer: RefCell::new(None),
                editor_view: RefCell::new(None),
                spell_adapter: RefCell::new(None),
                editor_css: RefCell::new(None),
                editor_clamp: RefCell::new(None),
                preview_clamp: RefCell::new(None),
                read_only: Cell::new(false),
                selected: RefCell::new(None),
                current_dated: RefCell::new(None),
                current_created: RefCell::new(None),
                current_type: RefCell::new(None),
                source_note: Cell::new(None),
                note_filter: Cell::new(timeline::NoteFilter::Open),
                dirty: Cell::new(false),
                loading: Cell::new(false),
                save_source: RefCell::new(None),
                row_ids: RefCell::new(Vec::new()),
                section_filters: RefCell::new(Vec::new()),
                collection_filters: RefCell::new(Vec::new()),
                collection_kind: Cell::new(None),
                collection_subject: RefCell::new(None),
                open_popovers: RefCell::new(Vec::new()),
                preferences: RefCell::new(None),
                search_index: RefCell::new(None),
                entry_menu: Default::default(),
                toast_overlay: Default::default(),
                details_split: Default::default(),
                details_button: Default::default(),
                details_close: Default::default(),
                details_box: Default::default(),
                panel: RefCell::new(None),
                panel_loading: Cell::new(false),
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
            obj.setup_editor();
            obj.setup_sections();
            obj.setup_main();
            obj.setup_details();
            obj.setup_notes();
            obj.setup_breakpoints();
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
        let page = gio::Menu::new();
        page.append(
            Some(&gettextrs::gettext("Preferences")),
            Some("win.preferences"),
        );
        page.append(
            Some(&gettextrs::gettext("Keyboard Shortcuts")),
            Some("win.show-help-overlay"),
        );
        page.append(
            Some(&gettextrs::gettext("About Quaderno")),
            Some("win.about"),
        );
        menu.append_section(None, &page);
        menu.append(Some(&gettextrs::gettext("Quit")), Some("app.quit"));
        self.imp().menu_button.set_menu_model(Some(&menu));
        self.register_action("preferences", |window| window.present_preferences());
        self.register_action("about", |window| window.present_about());

        let delete = gio::SimpleAction::new("delete-entry", None);
        let window = self.clone();
        delete.connect_activate(move |_, _| window.delete_selected());
        self.add_action(&delete);
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
        self.imp().read_only.set(access == Access::ReadOnly);
        self.show_phase(Phase::Main);
        self.apply_read_only();
        self.start_idle_timer();
        self.refresh();
    }

    /// Disables editing for a vault that is newer than this client (spec §7.2).
    fn apply_read_only(&self) {
        let editable = !self.imp().read_only.get();
        if let Some(view) = self.imp().editor_view.borrow().as_ref() {
            view.set_editable(editable);
        }
        // A read-only vault disables the whole details panel (product spec §3.2).
        self.imp().details_box.set_sensitive(editable);
        let buttons: [gtk::Widget; 9] = [
            self.imp().new_button.clone().upcast(),
            self.imp().date_button.clone().upcast(),
            self.imp().heading_button.clone().upcast(),
            self.imp().bold_button.clone().upcast(),
            self.imp().italic_button.clone().upcast(),
            self.imp().list_button.clone().upcast(),
            self.imp().quote_button.clone().upcast(),
            self.imp().link_button.clone().upcast(),
            self.imp().entry_menu.clone().upcast(),
        ];
        for button in buttons {
            button.set_sensitive(editable);
        }
        if let Some(action) = self
            .lookup_action("delete-entry")
            .and_downcast::<gio::SimpleAction>()
        {
            action.set_enabled(editable);
        }
    }

    // ---- main page (sections, entry list, editor) ------------------------

    fn setup_editor(&self) {
        let buffer = sourceview5::Buffer::new(None);
        if let Some(language) = sourceview5::LanguageManager::default().language("markdown") {
            buffer.set_language(Some(&language));
        }

        let manager = sourceview5::StyleSchemeManager::default();
        manager.append_search_path("resource:///io/github/stickgrinder/Quaderno/styles");

        let view = sourceview5::View::with_buffer(&buffer);
        view.set_wrap_mode(gtk::WrapMode::WordChar);
        view.set_top_margin(12);
        view.set_bottom_margin(12);
        view.set_left_margin(12);
        view.set_right_margin(12);
        view.add_css_class("entry-text");
        let adapter = crate::spell_checking::attach(&buffer);

        let window = self.clone();
        buffer.connect_changed(move |_| window.schedule_save());
        let focus = gtk::EventControllerFocus::new();
        let window = self.clone();
        focus.connect_leave(move |_| window.flush_save());
        view.add_controller(focus);

        // The line-width preference clamps the text column; the font/size
        // preference is applied through a CSS provider on the view.
        let clamp = adw::ClampScrollable::new();
        clamp.set_child(Some(&view));
        self.imp().editor_scroll.set_child(Some(&clamp));
        self.imp().editor_clamp.replace(Some(clamp));

        // Re-apply the line width whenever the editor pane is resized.
        let window = self.clone();
        self.imp()
            .editor_scroll
            .hadjustment()
            .connect_changed(move |_| window.apply_line_width());

        let css = gtk::CssProvider::new();
        if let Some(display) = gtk::gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &css,
                gtk::STYLE_PROVIDER_PRIORITY_USER,
            );
        }
        self.imp().editor_css.replace(Some(css));

        self.imp().editor_view.replace(Some(view));
        self.imp().editor_buffer.replace(Some(buffer));
        self.imp().spell_adapter.replace(Some(adapter));

        self.apply_style_scheme();
        self.apply_writing_font();
        self.apply_line_width();
        self.apply_spell_languages();
        let window = self.clone();
        adw::StyleManager::default().connect_dark_notify(move |_| window.apply_style_scheme());
    }

    /// Applies the light or dark Quaderno scheme to match the system style.
    fn apply_style_scheme(&self) {
        let id = if adw::StyleManager::default().is_dark() {
            "quaderno-dark"
        } else {
            "quaderno"
        };
        let manager = sourceview5::StyleSchemeManager::default();
        if let (Some(scheme), Some(buffer)) = (
            manager.scheme(id),
            self.imp().editor_buffer.borrow().as_ref(),
        ) {
            buffer.set_style_scheme(Some(&scheme));
        }
    }

    /// The CSS rule for the current writing font and text size.
    fn writer_font_css(&self) -> String {
        let settings = &self.imp().settings;
        let families = match settings.string("writing-font").as_str() {
            "sans" => "sans-serif".to_owned(),
            "mono" => "monospace".to_owned(),
            "custom" => {
                let family = settings.string("custom-font");
                if family.is_empty() {
                    "\"Source Serif 4\", serif".to_owned()
                } else {
                    format!("\"{family}\", serif")
                }
            }
            _ => "\"Source Serif 4\", serif".to_owned(),
        };
        let size = settings.int("text-size").clamp(14, 26);
        format!(
            "textview.entry-text {{ font-family: {families}; font-size: {size}px; }}\n\
             .writing-preview {{ font-family: {families}; font-size: {size}px; }}"
        )
    }

    fn apply_writing_font(&self) {
        if let Some(css) = self.imp().editor_css.borrow().as_ref() {
            css.load_from_string(&self.writer_font_css());
        }
    }

    /// The fraction of the editor width used by the Line width setting. A
    /// fraction, rather than a fixed number of pixels, so the choice always
    /// takes effect whatever the window size.
    fn line_width_fraction(&self) -> f64 {
        match self.imp().settings.string("line-width").as_str() {
            "narrow" => 0.6,
            "wide" => 0.95,
            _ => 0.78,
        }
    }

    fn apply_line_width(&self) {
        let fraction = self.line_width_fraction();
        if let Some(clamp) = self.imp().editor_clamp.borrow().as_ref() {
            let pane = f64::from(self.imp().editor_scroll.width());
            if pane > 0.0 {
                clamp.set_maximum_size((pane * fraction).round() as i32);
            }
        }
        if let Some(preview) = self.imp().preview_clamp.borrow().as_ref() {
            preview.set_maximum_size((PREVIEW_WIDTH * fraction).round() as i32);
        }
    }

    fn apply_spell_languages(&self) {
        let adapter_ref = self.imp().spell_adapter.borrow();
        let Some(adapter) = adapter_ref.as_ref() else {
            return;
        };
        let languages = self.imp().settings.string("spell-languages");
        let checker = if languages.is_empty() {
            libspelling::Checker::new(None, None)
        } else {
            libspelling::Checker::new(None, Some(languages.as_str()))
        };
        adapter.set_checker(&checker);
    }

    fn setup_sections(&self) {
        let sections = [
            (
                "document-open-recent-symbolic",
                gettextrs::gettext("Timeline"),
                Filter::All,
            ),
            (
                "x-office-document-symbolic",
                gettextrs::gettext("Journal pages"),
                Filter::Journal,
            ),
            (
                "weather-few-clouds-night-symbolic",
                gettextrs::gettext("Dreams"),
                Filter::Dreams,
            ),
            (
                "text-editor-symbolic",
                gettextrs::gettext("Quick notes"),
                Filter::Notes,
            ),
        ];
        let mut mapping = Vec::new();
        for (icon, label, filter) in sections {
            let row = self.section_row(icon, &label);
            self.imp().sections_list.append(&row);
            mapping.push((row.index() as u32, filter));
        }
        self.imp().section_filters.replace(mapping);

        let collections = gtk::ListBoxRow::new();
        collections.set_selectable(false);
        collections.set_activatable(false);
        let heading = gtk::Label::new(Some(&gettextrs::gettext("Collections")));
        heading.set_xalign(0.0);
        heading.add_css_class("dim-label");
        heading.set_margin_top(12);
        heading.set_margin_bottom(4);
        heading.set_margin_start(12);
        collections.set_child(Some(&heading));
        self.imp().sections_list.append(&collections);

        for (icon, label, kind) in [
            (
                "avatar-default-symbolic",
                gettextrs::gettext("People"),
                SubjectKind::Person,
            ),
            (
                "mark-location-symbolic",
                gettextrs::gettext("Locations"),
                SubjectKind::Place,
            ),
            (
                "package-x-generic-symbolic",
                gettextrs::gettext("Things"),
                SubjectKind::Thing,
            ),
            ("tag-symbolic", gettextrs::gettext("Tags"), SubjectKind::Tag),
        ] {
            let row = self.section_row(icon, &label);
            self.imp().sections_list.append(&row);
            self.imp()
                .collection_filters
                .borrow_mut()
                .push((row.index() as u32, kind));
        }

        let window = self.clone();
        self.imp()
            .sections_list
            .connect_row_activated(move |_, row| {
                let index = row.index() as u32;
                let filter = window
                    .imp()
                    .section_filters
                    .borrow()
                    .iter()
                    .find(|(idx, _)| *idx == index)
                    .map(|(_, filter)| *filter);
                if let Some(filter) = filter {
                    window.set_filter(filter);
                    return;
                }
                let collection = window
                    .imp()
                    .collection_filters
                    .borrow()
                    .iter()
                    .find(|(idx, _)| *idx == index)
                    .map(|(_, kind)| *kind);
                if let Some(kind) = collection {
                    window.open_collection(kind);
                }
            });

        let window = self.clone();
        self.imp()
            .collection_back
            .connect_clicked(move |_| window.close_subject_filter());
    }

    fn section_row(&self, icon: &str, label: &str) -> gtk::ListBoxRow {
        let row = gtk::ListBoxRow::new();
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        content.set_margin_top(6);
        content.set_margin_bottom(6);
        content.set_margin_start(6);
        let image = gtk::Image::from_icon_name(icon);
        content.append(&image);
        let text = gtk::Label::new(Some(label));
        text.set_xalign(0.0);
        text.set_hexpand(true);
        content.append(&text);
        row.set_child(Some(&content));
        row
    }

    fn setup_main(&self) {
        let window = self.clone();
        self.imp().sidebar_toggle.connect_toggled(move |button| {
            window.imp().main_split.set_show_sidebar(button.is_active());
        });
        let window = self.clone();
        self.imp()
            .main_split
            .connect_show_sidebar_notify(move |split| {
                window
                    .imp()
                    .sidebar_toggle
                    .set_active(split.shows_sidebar());
            });

        let window = self.clone();
        self.imp().search_button.connect_toggled(move |button| {
            window.imp().search_bar.set_search_mode(button.is_active());
        });
        self.imp()
            .search_bar
            .connect_entry(&*self.imp().search_entry);

        let window = self.clone();
        self.imp().search_entry.connect_search_changed(move |_| {
            window.refresh();
        });

        let window = self.clone();
        self.imp()
            .filter_group
            .connect_active_name_notify(move |group| {
                if let Some(name) = group.active_name() {
                    window.set_filter(match name.as_str() {
                        "journal" => Filter::Journal,
                        "dreams" => Filter::Dreams,
                        "notes" => Filter::Notes,
                        _ => Filter::All,
                    });
                }
            });

        let window = self.clone();
        self.imp().entry_list.connect_row_activated(move |_, row| {
            let index = row.index();
            let id = window
                .imp()
                .row_ids
                .borrow()
                .get(index as usize)
                .copied()
                .flatten();
            let Some(id) = id else {
                return;
            };
            // In a collection list, a row opens the timeline filtered to it.
            match window.imp().collection_kind.get() {
                Some(kind) if window.imp().collection_subject.borrow().is_none() => {
                    window.open_subject(kind, id);
                }
                _ => window.select_entry(id),
            }
        });

        let window = self.clone();
        self.connect_close_request(move |_| {
            window.depart_current();
            glib::Propagation::Proceed
        });

        self.setup_date_callbacks();
        self.setup_formatting();
        self.setup_shortcuts();
    }

    fn setup_shortcuts(&self) {
        self.register_action("search", |window| window.open_search());
        self.register_action("previous", |window| window.select_relative(-1));
        self.register_action("next", |window| window.select_relative(1));
        self.register_action("toggle-sidebar", |window| window.toggle_sidebar());
        self.register_action("heading-1", |window| window.prefix_line("# "));
        self.register_action("heading-2", |window| window.prefix_line("## "));
        self.register_action("heading-3", |window| window.prefix_line("### "));
        self.register_action("bold", |window| window.wrap_selection("**", "**"));
        self.register_action("italic", |window| window.wrap_selection("*", "*"));
        self.register_action("toggle-details", |window| window.toggle_details());
    }

    fn register_action(&self, name: &str, run: fn(&QuadernoWindow)) {
        let action = gio::SimpleAction::new(name, None);
        let window = self.clone();
        action.connect_activate(move |_, _| run(&window));
        self.add_action(&action);
    }

    fn open_search(&self) {
        self.imp().search_button.set_active(true);
        self.imp().search_entry.grab_focus();
    }

    fn toggle_sidebar(&self) {
        let toggle = &self.imp().sidebar_toggle;
        toggle.set_active(!toggle.is_active());
    }

    fn select_relative(&self, delta: i32) {
        let ids: Vec<Uuid> = self
            .imp()
            .row_ids
            .borrow()
            .iter()
            .flatten()
            .copied()
            .collect();
        if ids.is_empty() {
            return;
        }
        let index = match *self.imp().selected.borrow() {
            Some(current) => ids
                .iter()
                .position(|id| *id == current)
                .map(|index| (index as i32 + delta).clamp(0, ids.len() as i32 - 1) as usize)
                .unwrap_or(0),
            None => 0,
        };
        self.select_entry(ids[index]);
    }

    fn setup_formatting(&self) {
        let window = self.clone();
        self.imp()
            .heading_button
            .connect_clicked(move |_| window.prefix_line("# "));
        let window = self.clone();
        self.imp()
            .bold_button
            .connect_clicked(move |_| window.wrap_selection("**", "**"));
        let window = self.clone();
        self.imp()
            .italic_button
            .connect_clicked(move |_| window.wrap_selection("*", "*"));
        let window = self.clone();
        self.imp()
            .list_button
            .connect_clicked(move |_| window.prefix_line("- "));
        let window = self.clone();
        self.imp()
            .quote_button
            .connect_clicked(move |_| window.prefix_line("> "));
        let window = self.clone();
        self.imp()
            .link_button
            .connect_clicked(move |_| window.wrap_selection("[", "](url)"));
    }

    // ---- details panel (ui-spec §3.6, product spec §5) -------------------

    fn setup_details(&self) {
        let imp = self.imp();

        let window = self.clone();
        imp.details_button.connect_toggled(move |button| {
            window
                .imp()
                .details_split
                .set_show_sidebar(button.is_active());
        });
        let window = self.clone();
        imp.details_split.connect_show_sidebar_notify(move |split| {
            window
                .imp()
                .details_button
                .set_active(split.shows_sidebar());
        });
        let window = self.clone();
        imp.details_close.connect_clicked(move |_| {
            window.imp().details_split.set_show_sidebar(false);
        });

        self.build_details();
    }

    fn toggle_details(&self) {
        let button = &self.imp().details_button;
        button.set_active(!button.is_active());
    }

    /// Builds the panel once; `refresh_details` keeps it in sync afterwards.
    fn build_details(&self) {
        let container = &self.imp().details_box;
        while let Some(child) = container.first_child() {
            container.remove(&child);
        }

        // Entry date + created/updated caption.
        let date_button = gtk::Button::new();
        date_button.add_css_class("flat");
        let window = self.clone();
        date_button.connect_clicked(move |_| {
            window.imp().date_button.popup();
        });
        let date_row = adw::ActionRow::builder()
            .title(gettextrs::gettext("Entry date"))
            .build();
        date_row.add_suffix(&date_button);
        let date_group = adw::PreferencesGroup::new();
        date_group.add(&date_row);
        let dates_label = gtk::Label::new(None);
        dates_label.set_xalign(0.0);
        dates_label.set_wrap(true);
        dates_label.add_css_class("dim-label");
        dates_label.add_css_class("caption");
        let date_section = gtk::Box::new(gtk::Orientation::Vertical, 6);
        date_section.append(&date_group);
        date_section.append(&dates_label);
        container.append(&date_section);

        // How it felt: four rows of five icon toggles. `AdwToggleGroup` cannot
        // deselect, so plain linked toggle buttons implement "click the active
        // value to clear it" (ui-spec §3.6).
        let felt_group = adw::PreferencesGroup::builder()
            .title(gettextrs::gettext("How it felt"))
            .build();
        let mut ratings = Vec::new();
        for rating in [
            Rating::Mood,
            Rating::Energy,
            Rating::CognitiveLoad,
            Rating::Sleep,
        ] {
            let row = adw::ActionRow::new();
            let buttons_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            buttons_box.add_css_class("linked");
            buttons_box.set_valign(gtk::Align::Center);

            let mut buttons = Vec::new();
            for value in 1..=5u8 {
                let button = gtk::ToggleButton::new();
                if let Some(icon) = details::rating_icon(rating, value) {
                    button.set_icon_name(&icons::resolve(icon));
                }
                if let Some(label) = details::rating_label(rating, value) {
                    button.set_tooltip_text(Some(&label));
                    button.update_property(&[gtk::accessible::Property::Label(&label)]);
                }
                buttons.push(button.clone());
                buttons_box.append(&button);
            }
            for (index, button) in buttons.iter().enumerate() {
                let value = index as u8 + 1;
                let buttons = buttons.clone();
                let window = self.clone();
                button.connect_toggled(move |button| {
                    if window.imp().panel_loading.get() {
                        return;
                    }
                    if !button.is_active() {
                        if buttons.iter().all(|button| !button.is_active()) {
                            window.write_rating(rating, None);
                        }
                        return;
                    }
                    for other in &buttons {
                        if other != button {
                            other.set_active(false);
                        }
                    }
                    window.write_rating(rating, Some(value));
                });
            }
            row.add_suffix(&buttons_box);
            felt_group.add(&row);
            ratings.push(RatingControl {
                rating,
                row,
                buttons,
            });
        }
        container.append(&felt_group);

        // Emotions and day activities.
        let emotions =
            self.build_choice_section(ChoiceKind::Emotion, &gettextrs::gettext("Emotions"));
        let activities =
            self.build_choice_section(ChoiceKind::Activity, &gettextrs::gettext("Day activities"));

        // Colours of the day.
        let colors_section = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let colors_heading = gtk::Label::new(Some(&gettextrs::gettext("Colors of the day")));
        colors_heading.set_xalign(0.0);
        colors_heading.add_css_class("heading");
        let colors_flow = gtk::FlowBox::new();
        colors_flow.set_selection_mode(gtk::SelectionMode::None);
        colors_flow.set_column_spacing(6);
        colors_flow.set_row_spacing(6);
        colors_flow.set_valign(gtk::Align::Start);
        let mut colors = Vec::new();
        for color in Color::ALL {
            let button = gtk::ToggleButton::new();
            button.add_css_class("color-swatch");
            button.add_css_class(color_css_class(color));
            // A hard size, so the swatch stays circular whatever the theme's
            // minimum button size is.
            button.set_size_request(24, 24);
            let label = details::color_label(color);
            button.set_tooltip_text(Some(&label));
            button.update_property(&[gtk::accessible::Property::Label(&label)]);
            let window = self.clone();
            button.connect_toggled(move |button| {
                if window.imp().panel_loading.get() {
                    return;
                }
                window.toggle_color(color, button.is_active());
            });
            colors.push((color, button.clone()));
            colors_flow.append(&button);
        }
        colors_section.append(&colors_heading);
        colors_section.append(&colors_flow);
        container.append(&colors_section);

        // People, places, things, tags.
        let subjects_section = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let subjects_title = gtk::Label::new(None);
        subjects_title.set_xalign(0.0);
        subjects_title.add_css_class("heading");
        let subjects_group = adw::PreferencesGroup::new();
        let mut tokens = Vec::new();
        for kind in [
            SubjectKind::Person,
            SubjectKind::Place,
            SubjectKind::Thing,
            SubjectKind::Tag,
        ] {
            let token = self.build_token_field(kind);
            subjects_group.add(&token.row);
            tokens.push(token);
        }
        subjects_section.append(&subjects_title);
        subjects_section.append(&subjects_group);
        container.append(&subjects_section);

        self.imp().panel.replace(Some(PanelWidgets {
            date_button,
            dates_label,
            felt_group,
            ratings,
            choices: vec![emotions, activities],
            colors_section,
            colors,
            subjects_section,
            subjects_title,
            tokens,
        }));
    }

    fn build_choice_section(&self, kind: ChoiceKind, title: &str) -> ChoiceControl {
        let section = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let heading = gtk::Label::new(Some(title));
        heading.set_xalign(0.0);
        heading.add_css_class("heading");

        let flow = gtk::FlowBox::new();
        flow.set_selection_mode(gtk::SelectionMode::None);
        flow.set_column_spacing(6);
        flow.set_row_spacing(6);
        flow.set_valign(gtk::Align::Start);

        let add_button = gtk::Button::builder()
            .label(gettextrs::gettext("Add"))
            .icon_name("list-add-symbolic")
            .build();
        add_button.add_css_class("pill");
        add_button.set_tooltip_text(Some(&gettextrs::gettext("Add")));

        let (list, selection, model) = picker::list_view();
        let search = gtk::SearchEntry::new();
        search.set_placeholder_text(Some(&gettextrs::gettext("Search")));
        let scrolled = gtk::ScrolledWindow::builder()
            .child(&list)
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build();
        let content = gtk::Box::new(gtk::Orientation::Vertical, 6);
        content.set_size_request(280, 320);
        content.append(&search);
        content.append(&scrolled);
        let popover = gtk::Popover::new();
        popover.set_child(Some(&content));
        popover.set_parent(&add_button);

        {
            let window = self.clone();
            let search = search.clone();
            let model = model.clone();
            let popover = popover.clone();
            add_button.connect_clicked(move |_| {
                search.set_text("");
                window.populate_choice_picker(kind, "", &model);
                popover.popup();
                search.grab_focus();
            });
        }
        {
            let window = self.clone();
            let model = model.clone();
            search.connect_search_changed(move |search| {
                window.populate_choice_picker(kind, &search.text(), &model);
            });
        }
        {
            let window = self.clone();
            let search = search.clone();
            let model = model.clone();
            let selection = selection.clone();
            list.connect_activate(move |_, _| {
                let Some(row) = selection
                    .selected_item()
                    .and_downcast::<picker::PickerRow>()
                else {
                    return;
                };
                if row.is_create() {
                    return;
                }
                let Ok(id) = Uuid::parse_str(&row.id()) else {
                    return;
                };
                window.link_choice(id);
                window.populate_choice_picker(kind, &search.text(), &model);
            });
        }

        section.append(&heading);
        section.append(&flow);
        self.imp().details_box.append(&section);

        ChoiceControl {
            kind,
            section,
            flow,
            add_button,
            popover,
        }
    }

    fn build_token_field(&self, kind: SubjectKind) -> TokenControl {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        row.set_margin_top(6);
        row.set_margin_bottom(6);
        row.set_margin_start(6);
        row.set_margin_end(6);
        let icon = gtk::Image::from_icon_name(subject_icon(kind));
        icon.set_valign(gtk::Align::Center);
        row.append(&icon);

        let wrap = adw::WrapBox::new();
        wrap.set_child_spacing(6);
        wrap.set_line_spacing(6);
        wrap.set_hexpand(true);
        wrap.set_valign(gtk::Align::Center);
        let entry = gtk::Text::new();
        entry.set_placeholder_text(Some(&details::subject_placeholder(kind)));
        entry.set_hexpand(true);
        entry.set_width_request(80);
        wrap.append(&entry);
        row.append(&wrap);

        let (list, selection, model) = picker::list_view();
        let scrolled = gtk::ScrolledWindow::builder()
            .child(&list)
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build();
        let content = gtk::Box::new(gtk::Orientation::Vertical, 6);
        content.set_size_request(280, 300);
        content.append(&scrolled);
        let popover = gtk::Popover::new();
        popover.set_child(Some(&content));
        popover.set_parent(&entry);
        // A completion popover must not take focus: with `autohide` (the GTK
        // default) the popover grabs input and moves keyboard focus onto itself,
        // so the entry stops receiving keystrokes. Keep it non-modal, keep the
        // list unfocusable, dismiss on focus loss, and drive navigation from the
        // entry instead.
        popover.set_autohide(false);
        popover.set_can_focus(false);
        list.set_can_focus(false);
        {
            let popover = popover.clone();
            let focus = gtk::EventControllerFocus::new();
            focus.connect_leave(move |_| popover.popdown());
            entry.add_controller(focus);
        }

        {
            let window = self.clone();
            let model = model.clone();
            let popover = popover.clone();
            entry.connect_changed(move |entry| {
                window.populate_token_suggestions(kind, &entry.text(), &model, &popover);
            });
        }
        {
            let window = self.clone();
            let list = list.clone();
            let selection = selection.clone();
            let popover = popover.clone();
            let keys = gtk::EventControllerKey::new();
            // Capture, so a printable key like the comma reaches us before the
            // `GtkText` input method commits it as text.
            keys.set_propagation_phase(gtk::PropagationPhase::Capture);
            {
                let entry = entry.clone();
                keys.connect_key_pressed(move |_, key, _, state| match key {
                    gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter => {
                        let shift = state.contains(gtk::gdk::ModifierType::SHIFT_MASK);
                        window.accept_token_suggestion(kind, shift, &entry, &selection, &popover);
                        glib::Propagation::Stop
                    }
                    gtk::gdk::Key::BackSpace if entry.text().is_empty() => {
                        window.remove_last_subject(kind);
                        glib::Propagation::Stop
                    }
                    gtk::gdk::Key::comma => {
                        window.commit_token(kind, &entry, &popover);
                        glib::Propagation::Stop
                    }
                    gtk::gdk::Key::Down => {
                        move_selection(&selection, &list, 1);
                        glib::Propagation::Stop
                    }
                    gtk::gdk::Key::Up => {
                        move_selection(&selection, &list, -1);
                        glib::Propagation::Stop
                    }
                    gtk::gdk::Key::Escape => {
                        popover.popdown();
                        glib::Propagation::Proceed
                    }
                    _ => glib::Propagation::Proceed,
                });
            }
            entry.add_controller(keys);
        }
        {
            let window = self.clone();
            let entry = entry.clone();
            let selection = selection.clone();
            let popover = popover.clone();
            list.connect_activate(move |_, _| {
                let Some(row) = selection
                    .selected_item()
                    .and_downcast::<picker::PickerRow>()
                else {
                    return;
                };
                if row.is_create() {
                    let name = entry.text().trim().to_string();
                    if !name.is_empty() {
                        window.create_and_link_subject(kind, &name);
                    }
                } else if let Ok(id) = Uuid::parse_str(&row.id()) {
                    window.link_subject(id);
                }
                clear_token_entry(&entry, &popover);
            });
        }

        TokenControl {
            kind,
            row,
            wrap,
            entry,
            popover,
        }
    }

    /// Rebuilds the panel for the selected entry.
    fn refresh_details(&self) {
        let Some(id) = *self.imp().selected.borrow() else {
            return;
        };
        let (entry, last_updated) = {
            let vault = self.imp().vault.borrow();
            let Some(vault) = vault.as_ref() else {
                return;
            };
            let Ok(entry) = vault.entry(id) else {
                return;
            };
            (entry, vault.entry_last_updated(id).ok())
        };

        self.imp().panel_loading.set(true);
        let panel_ref = self.imp().panel.borrow();
        let Some(panel) = panel_ref.as_ref() else {
            self.imp().panel_loading.set(false);
            return;
        };

        panel
            .date_button
            .set_label(&entry.dated_at.strftime("%d %b %Y").to_string());
        panel
            .dates_label
            .set_label(&dates_caption(entry.created_at, last_updated));

        let sections = details::sections_for(entry.entry_type);
        panel.felt_group.set_visible(sections.ratings);
        for control in &panel.ratings {
            control
                .row
                .set_title(&details::rating_title(control.rating, entry.entry_type));
            let value = rating_value(&entry, control.rating);
            let label = value
                .and_then(|value| details::rating_label(control.rating, value))
                .unwrap_or_default();
            control.row.set_subtitle(&label);
            for (index, button) in control.buttons.iter().enumerate() {
                button.set_active(value == Some(index as u8 + 1));
            }
        }

        for control in &panel.choices {
            control.section.set_visible(match control.kind {
                ChoiceKind::Emotion => sections.emotions,
                ChoiceKind::Activity => sections.activities,
            });
            self.refresh_choice_control(control);
        }

        panel.colors_section.set_visible(sections.colors);
        let selected_colors = {
            let vault = self.imp().vault.borrow();
            match vault.as_ref() {
                Some(vault) => vault.entry_colors(id).unwrap_or_default(),
                None => Vec::new(),
            }
        };
        for (color, button) in &panel.colors {
            button.set_active(selected_colors.contains(color));
        }

        panel
            .subjects_section
            .set_visible(sections.subjects || sections.tags);
        panel
            .subjects_title
            .set_label(&details::subjects_group_title(entry.entry_type));
        for token in &panel.tokens {
            token.row.set_visible(match token.kind {
                SubjectKind::Person | SubjectKind::Place | SubjectKind::Thing => sections.subjects,
                SubjectKind::Tag => sections.tags,
            });
            self.refresh_token_control(token);
        }

        drop(panel_ref);
        self.imp().panel_loading.set(false);
    }

    fn refresh_choice_control(&self, control: &ChoiceControl) {
        let flow = &control.flow;
        flow.remove_all();
        let Some(id) = *self.imp().selected.borrow() else {
            flow.append(&control.add_button);
            return;
        };
        let choices = {
            let vault = self.imp().vault.borrow();
            match vault.as_ref() {
                Some(vault) => vault.entry_choices(id).unwrap_or_default(),
                None => Vec::new(),
            }
        };
        for choice in choices {
            if choice.kind == control.kind {
                let chip = self.build_choice_chip(&choice);
                flow.append(&chip);
            }
        }
        flow.append(&control.add_button);
    }

    fn build_choice_chip(&self, choice: &Choice) -> gtk::Widget {
        let label = details::choice_display_label(choice);
        let chip = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        chip.add_css_class("chip");
        chip.set_valign(gtk::Align::Center);
        chip.append(&gtk::Image::from_icon_name(&icons::resolve(&choice.icon)));
        chip.append(&gtk::Label::new(Some(&label)));
        chip.append(&self.remove_button(choice.id, ChipKind::Choice));
        chip.upcast()
    }

    fn refresh_token_control(&self, control: &TokenControl) {
        let wrap = &control.wrap;
        // The popover is parented to the entry; removing the entry would leave
        // the popover orphaned outside the window tree, where the lock handler
        // cannot find it. Pop it down before the entry is detached.
        control.popover.popdown();
        while let Some(child) = wrap.first_child() {
            wrap.remove(&child);
        }
        if let Some(id) = *self.imp().selected.borrow() {
            let subjects = {
                let vault = self.imp().vault.borrow();
                match vault.as_ref() {
                    Some(vault) => vault
                        .entry_subjects(id, Some(control.kind))
                        .unwrap_or_default(),
                    None => Vec::new(),
                }
            };
            for subject in subjects {
                let chip = self.build_subject_chip(&subject);
                wrap.append(&chip);
            }
        }
        wrap.append(&control.entry);
    }

    fn build_subject_chip(&self, subject: &Subject) -> gtk::Widget {
        let chip = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        chip.add_css_class("chip");
        chip.set_valign(gtk::Align::Center);
        chip.append(&gtk::Label::new(Some(&subject.name)));
        chip.append(&self.remove_button(subject.id, ChipKind::Subject));
        chip.upcast()
    }

    /// A small flat remove button for a chip, wired to unlink with an undo toast.
    fn remove_button(&self, id: Uuid, kind: ChipKind) -> gtk::Button {
        let button = gtk::Button::from_icon_name("window-close-symbolic");
        button.add_css_class("flat");
        button.add_css_class("circular");
        let tooltip = gettextrs::gettext("Remove");
        button.set_tooltip_text(Some(&tooltip));
        button.update_property(&[gtk::accessible::Property::Label(&tooltip)]);
        let window = self.clone();
        button.connect_clicked(move |_| match kind {
            ChipKind::Choice => window.remove_choice(id),
            ChipKind::Subject => window.remove_subject(id),
        });
        button
    }

    fn populate_choice_picker(&self, kind: ChoiceKind, query: &str, model: &gio::ListStore) {
        model.remove_all();
        let Some(id) = *self.imp().selected.borrow() else {
            return;
        };
        let vault = self.imp().vault.borrow();
        let Some(vault) = vault.as_ref() else {
            return;
        };
        let linked: HashSet<Uuid> = vault
            .entry_choices(id)
            .unwrap_or_default()
            .into_iter()
            .map(|choice| choice.id)
            .collect();
        for choice in vault.choices(kind, false).unwrap_or_default() {
            if linked.contains(&choice.id) {
                continue;
            }
            let label = details::choice_display_label(&choice);
            if !details::token_matches(query, &label) {
                continue;
            }
            model.append(&picker::PickerRow::new(
                &choice.id.to_string(),
                &label,
                "",
                &icons::resolve(&choice.icon),
                false,
            ));
        }
    }

    fn populate_token_suggestions(
        &self,
        kind: SubjectKind,
        query: &str,
        model: &gio::ListStore,
        popover: &gtk::Popover,
    ) {
        model.remove_all();
        let Some(entry_id) = *self.imp().selected.borrow() else {
            popover.popdown();
            return;
        };
        let (usages, linked) = {
            let vault = self.imp().vault.borrow();
            let Some(vault) = vault.as_ref() else {
                return;
            };
            let linked: HashSet<Uuid> = vault
                .entry_subjects(entry_id, Some(kind))
                .unwrap_or_default()
                .into_iter()
                .map(|subject| subject.id)
                .collect();
            (vault.subjects_with_usage(kind).unwrap_or_default(), linked)
        };
        for usage in &usages {
            if linked.contains(&usage.subject.id) {
                continue;
            }
            if !details::token_matches(query, &usage.subject.name) {
                continue;
            }
            let detail = details::usage_detail(usage.entry_count, usage.last_used.as_ref())
                .unwrap_or_default();
            model.append(&picker::PickerRow::new(
                &usage.subject.id.to_string(),
                &usage.subject.name,
                &detail,
                subject_icon(kind),
                false,
            ));
        }
        let names: Vec<&str> = usages
            .iter()
            .map(|usage| usage.subject.name.as_str())
            .collect();
        if details::show_create_row(query, names.iter().copied()) {
            model.append(&picker::PickerRow::new(
                "",
                &details::create_label(kind, query.trim()),
                "",
                "",
                true,
            ));
        }
        if model.n_items() > 0 {
            popover.popup();
        } else {
            popover.popdown();
        }
    }

    fn accept_token_suggestion(
        &self,
        kind: SubjectKind,
        shift: bool,
        entry: &gtk::Text,
        selection: &gtk::SingleSelection,
        popover: &gtk::Popover,
    ) {
        let query = entry.text().trim().to_string();
        let selected = selection
            .selected_item()
            .and_downcast::<picker::PickerRow>();
        let create = shift || selected.as_ref().is_some_and(|row| row.is_create());
        if create {
            if !query.is_empty() {
                self.create_and_link_subject(kind, &query);
                clear_token_entry(entry, popover);
            }
            return;
        }
        if let Some(row) = selected {
            if let Ok(id) = Uuid::parse_str(&row.id()) {
                self.link_subject(id);
                clear_token_entry(entry, popover);
            }
        }
    }

    /// Commits the text typed in a token field: links an exact existing match
    /// (case/accent-insensitive) or creates a new subject. Used by the comma
    /// shortcut, so a list of names can be typed without leaving the field.
    fn commit_token(&self, kind: SubjectKind, entry: &gtk::Text, popover: &gtk::Popover) {
        let text = entry.text();
        let name = text.trim();
        if name.is_empty() {
            popover.popdown();
            return;
        }
        let existing = {
            let vault = self.imp().vault.borrow();
            match vault.as_ref() {
                Some(vault) => vault
                    .subjects_with_usage(kind)
                    .unwrap_or_default()
                    .into_iter()
                    .find(|usage| details::token_is_exact(name, &usage.subject.name))
                    .map(|usage| usage.subject.id),
                None => None,
            }
        };
        match existing {
            Some(id) => self.link_subject(id),
            None => self.create_and_link_subject(kind, name),
        }
        clear_token_entry(entry, popover);
    }

    fn write_rating(&self, rating: Rating, value: Option<u8>) {
        let Some(id) = *self.imp().selected.borrow() else {
            return;
        };
        let saved = {
            let mut vault = self.imp().vault.borrow_mut();
            match vault.as_mut() {
                Some(vault) => vault.set_entry_rating(id, rating, value).is_ok(),
                None => false,
            }
        };
        if saved {
            self.refresh_details();
        }
    }

    fn toggle_color(&self, color: Color, linked: bool) {
        let Some(id) = *self.imp().selected.borrow() else {
            return;
        };
        let saved = {
            let mut vault = self.imp().vault.borrow_mut();
            match vault.as_mut() {
                Some(vault) if linked => vault.link_color(id, color).is_ok(),
                Some(vault) => vault.unlink_color(id, color).is_ok(),
                None => false,
            }
        };
        if saved {
            self.refresh();
        }
    }

    fn link_choice(&self, choice: Uuid) {
        let Some(id) = *self.imp().selected.borrow() else {
            return;
        };
        let linked = {
            let mut vault = self.imp().vault.borrow_mut();
            match vault.as_mut() {
                Some(vault) => vault.link_choice(id, choice).is_ok(),
                None => false,
            }
        };
        if linked {
            self.refresh_details();
            self.refresh();
        }
    }

    fn remove_choice(&self, choice: Uuid) {
        let Some(id) = *self.imp().selected.borrow() else {
            return;
        };
        let unlinked = {
            let mut vault = self.imp().vault.borrow_mut();
            match vault.as_mut() {
                Some(vault) => vault.unlink_choice(id, choice).is_ok(),
                None => false,
            }
        };
        if !unlinked {
            return;
        }
        self.refresh_details();
        self.refresh();
        self.undo_toast(move |window| window.link_choice(choice));
    }

    fn link_subject(&self, subject: Uuid) {
        let Some(id) = *self.imp().selected.borrow() else {
            return;
        };
        let linked = {
            let mut vault = self.imp().vault.borrow_mut();
            match vault.as_mut() {
                Some(vault) => vault.link_subject(id, subject).is_ok(),
                None => false,
            }
        };
        if linked {
            self.refresh_details();
            self.refresh();
        }
    }

    fn remove_subject(&self, subject: Uuid) {
        let Some(id) = *self.imp().selected.borrow() else {
            return;
        };
        let unlinked = {
            let mut vault = self.imp().vault.borrow_mut();
            match vault.as_mut() {
                Some(vault) => vault.unlink_subject(id, subject).is_ok(),
                None => false,
            }
        };
        if !unlinked {
            return;
        }
        self.refresh_details();
        self.refresh();
        self.undo_toast(move |window| window.link_subject(subject));
    }

    fn create_and_link_subject(&self, kind: SubjectKind, name: &str) {
        let created = {
            let mut vault = self.imp().vault.borrow_mut();
            match vault.as_mut() {
                Some(vault) => vault.create_subject(kind, name).ok(),
                None => None,
            }
        };
        if let Some(id) = created {
            self.link_subject(id);
        }
    }

    fn remove_last_subject(&self, kind: SubjectKind) {
        let Some(id) = *self.imp().selected.borrow() else {
            return;
        };
        let last = {
            let vault = self.imp().vault.borrow();
            match vault.as_ref() {
                Some(vault) => vault
                    .entry_subjects(id, Some(kind))
                    .unwrap_or_default()
                    .last()
                    .map(|subject| subject.id),
                None => None,
            }
        };
        if let Some(subject) = last {
            self.remove_subject(subject);
        }
    }

    fn undo_toast(&self, restore: impl Fn(&QuadernoWindow) + 'static) {
        let toast = adw::Toast::new(&gettextrs::gettext("Removed"));
        toast.set_button_label(Some(&gettextrs::gettext("Undo")));
        toast.set_timeout(10);
        let window = self.clone();
        toast.connect_button_clicked(move |_| restore(&window));
        self.imp().toast_overlay.add_toast(toast);
    }

    // ---- dreams and quick notes (M5) ------------------------------------

    fn setup_notes(&self) {
        let imp = self.imp();

        // New split button: primary follows the New entry type preference, the
        // menu offers every type (product spec §4.1, §8).
        let window = self.clone();
        imp.new_button
            .connect_clicked(move |_| window.new_entry_of(window.default_new_type()));
        let new_menu = gio::Menu::new();
        new_menu.append(
            Some(&gettextrs::gettext("Journal page")),
            Some("win.new-journal"),
        );
        new_menu.append(Some(&gettextrs::gettext("Dream")), Some("win.new-dream"));
        new_menu.append(
            Some(&gettextrs::gettext("Quick note")),
            Some("win.new-note"),
        );
        imp.new_button.set_menu_model(Some(&new_menu));
        self.register_action("new-journal", |window| {
            window.new_entry_of(EntryType::Journal)
        });
        self.register_action("new-dream", |window| window.new_entry_of(EntryType::Dream));
        self.register_action("new-note", |window| window.new_entry_of(EntryType::Note));

        // Quick notes Open / Expanded / All (ui-spec §3.7).
        imp.notes_filter_group.set_active_name(Some("open"));
        let window = self.clone();
        imp.notes_filter_group
            .connect_active_name_notify(move |group| {
                window
                    .imp()
                    .note_filter
                    .set(match group.active_name().as_deref() {
                        Some("expanded") => timeline::NoteFilter::Expanded,
                        Some("all") => timeline::NoteFilter::All,
                        _ => timeline::NoteFilter::Open,
                    });
                window.refresh();
            });

        // Dream flag chips (ui-spec §3.5): on is 1, off clears to NULL.
        for (flag, button) in [
            (DreamFlag::Lucid, imp.flag_lucid.clone()),
            (DreamFlag::Nightmare, imp.flag_nightmare.clone()),
            (DreamFlag::Recurring, imp.flag_recurring.clone()),
        ] {
            button.set_icon_name(&icons::resolve(details::dream_flag_icon(flag)));
            let label = details::dream_flag_label(flag);
            button.set_tooltip_text(Some(&label));
            button.update_property(&[gtk::accessible::Property::Label(&label)]);
            let window = self.clone();
            button.connect_toggled(move |button| {
                if window.imp().panel_loading.get() {
                    return;
                }
                window.set_dream_flag(flag, button.is_active());
            });
        }

        // Expand into entry (product spec §6).
        let window = self.clone();
        imp.expand_button
            .connect_clicked(move |_| window.expand_selected(EntryType::Journal));
        let expand_menu = gio::Menu::new();
        expand_menu.append(
            Some(&gettextrs::gettext("Journal page")),
            Some("win.expand-journal"),
        );
        expand_menu.append(Some(&gettextrs::gettext("Dream")), Some("win.expand-dream"));
        imp.expand_button.set_menu_model(Some(&expand_menu));
        self.register_action("expand-journal", |window| {
            window.expand_selected(EntryType::Journal)
        });
        self.register_action("expand-dream", |window| {
            window.expand_selected(EntryType::Dream)
        });

        // "From quick note" banner.
        let window = self.clone();
        imp.source_banner.connect_button_clicked(move |_| {
            if let Some(id) = window.imp().source_note.get() {
                window.select_entry(id);
            }
        });

        // The note hint card.
        let info_icon = gtk::Image::from_icon_name("dialog-information-symbolic");
        info_icon.set_valign(gtk::Align::Start);
        let info_label = gtk::Label::new(Some(&gettextrs::gettext(
            "Quick notes are not counted in Insights. Expanding creates a new entry, copies the text and tags, and links both ways.",
        )));
        info_label.set_wrap(true);
        info_label.set_xalign(0.0);
        info_label.add_css_class("dim-label");
        info_label.add_css_class("caption");
        info_label.set_margin_top(6);
        info_label.set_margin_bottom(6);
        info_label.set_margin_start(6);
        info_label.set_margin_end(6);
        imp.note_info_card.append(&info_icon);
        imp.note_info_card.append(&info_label);

        self.apply_view_mode();
    }

    /// Switches the list pane between the timeline and the quick-notes view
    /// (ui-spec §3.7).
    fn apply_view_mode(&self) {
        let collection = self.imp().collection_kind.get();
        let subject = *self.imp().collection_subject.borrow();
        let notes = collection.is_none() && self.active_filter() == Filter::Notes;
        let title = match (collection, subject.is_some()) {
            (Some(_), true) => gettextrs::gettext("Timeline"),
            (Some(kind), false) => collection_title(kind),
            (None, _) if notes => gettextrs::gettext("Quick notes"),
            _ => gettextrs::gettext("Timeline"),
        };
        self.imp().list_title.set_title(&title);
        self.imp().list_page.set_title(&title);
        self.imp()
            .filter_group
            .set_visible(collection.is_none() && !notes);
        self.imp().notes_filter_group.set_visible(notes);
        self.imp().collection_back.set_visible(subject.is_some());
    }

    /// Opens a collection (People, Locations, Things or Tags) in the list pane.
    fn open_collection(&self, kind: SubjectKind) {
        self.imp().collection_subject.replace(None);
        self.imp().collection_kind.set(Some(kind));
        self.refresh();
    }

    /// Opens the timeline filtered to one subject (product spec §7).
    fn open_subject(&self, kind: SubjectKind, subject: Uuid) {
        self.imp().collection_kind.set(Some(kind));
        self.imp().collection_subject.replace(Some(subject));
        self.refresh();
    }

    fn close_subject_filter(&self) {
        self.imp().collection_subject.replace(None);
        self.refresh();
    }

    fn rename_subject_value(&self, id: Uuid, name: &str) {
        let renamed = {
            let mut vault = self.imp().vault.borrow_mut();
            match vault.as_mut() {
                Some(vault) => vault.rename_subject(id, name).is_ok(),
                None => false,
            }
        };
        if renamed {
            self.refresh();
        }
    }

    fn delete_subject(&self, id: Uuid) {
        let deleted = {
            let mut vault = self.imp().vault.borrow_mut();
            match vault.as_mut() {
                Some(vault) => vault.soft_delete_subject(id).is_ok(),
                None => false,
            }
        };
        if deleted {
            self.refresh();
        } else {
            self.toast(&gettextrs::gettext(
                "This item is used by entries. Merge it instead of deleting it.",
            ));
        }
    }

    fn merge_subject_into(&self, keep: Uuid, merge: Uuid) {
        let merged = {
            let mut vault = self.imp().vault.borrow_mut();
            match vault.as_mut() {
                Some(vault) => vault.merge_subjects(keep, merge).is_ok(),
                None => false,
            }
        };
        if merged {
            self.refresh();
            self.toast(&gettextrs::gettext("Items merged"));
        }
    }

    /// A dialog to choose which subject to merge another into.
    fn present_merge(&self, kind: SubjectKind, merge: Uuid, candidates: Vec<(Uuid, String)>) {
        let dialog = adw::AlertDialog::new(
            Some(&gettextrs::gettext("Merge into…")),
            Some(&gettextrs::gettext(
                "Every entry linked to this item will link to the one you keep, and all its entries are kept.",
            )),
        );
        dialog.add_response("cancel", &gettextrs::gettext("Cancel"));
        dialog.add_response("merge", &gettextrs::gettext("Merge"));
        dialog.set_response_appearance("merge", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("merge"));
        dialog.set_close_response("cancel");

        let labels: Vec<&str> = candidates.iter().map(|(_, name)| name.as_str()).collect();
        let list = gtk::StringList::new(&labels);
        let dropdown = gtk::DropDown::new(Some(list), gtk::Expression::NONE);
        dropdown.set_selected(0);
        let group = adw::PreferencesGroup::new();
        let row = adw::ActionRow::builder()
            .title(gettextrs::gettext("Keep"))
            .build();
        row.add_suffix(&dropdown);
        group.add(&row);
        dialog.set_extra_child(Some(&group));

        let window = self.clone();
        dialog.connect_response(None, move |_, response| {
            if response == "merge"
                && let Some((keep, _)) = candidates.get(dropdown.selected() as usize)
            {
                let keep = *keep;
                let _ = kind;
                window.merge_subject_into(keep, merge);
            }
        });
        dialog.present(Some(self));
    }

    /// Fills the list pane with a collection's subjects.
    fn populate_collection(&self, kind: SubjectKind) {
        let usages = {
            let vault = self.imp().vault.borrow();
            vault
                .as_ref()
                .and_then(|vault| vault.subjects_with_usage(kind).ok())
                .unwrap_or_default()
        };
        let candidates: Vec<(Uuid, String)> = usages
            .iter()
            .map(|usage| (usage.subject.id, usage.subject.name.clone()))
            .collect();

        let list = &self.imp().entry_list;
        while let Some(child) = list.first_child() {
            list.remove(&child);
        }
        let mut row_ids: Vec<Option<Uuid>> = Vec::new();
        for usage in usages {
            let row = adw::ActionRow::new();
            row.set_use_markup(false);
            row.set_title(&usage.subject.name);
            row.set_subtitle(&choice_subtitle(usage.entry_count));
            row.set_activatable(true);

            let menu_button = gtk::MenuButton::builder()
                .icon_name("view-more-symbolic")
                .valign(gtk::Align::Center)
                .build();
            let menu = gtk::Box::new(gtk::Orientation::Vertical, 0);
            menu.set_margin_top(6);
            menu.set_margin_bottom(6);
            menu.set_margin_start(6);
            menu.set_margin_end(6);
            let subject = usage.subject.clone();
            menu.append(&self.menu_button(&gettextrs::gettext("Rename…"), {
                let window = self.clone();
                move || window.present_rename_subject(&subject)
            }));
            let others: Vec<(Uuid, String)> = candidates
                .iter()
                .filter(|(id, _)| *id != usage.subject.id)
                .cloned()
                .collect();
            if !others.is_empty() {
                menu.append(&self.menu_button(&gettextrs::gettext("Merge into…"), {
                    let window = self.clone();
                    let id = usage.subject.id;
                    move || window.present_merge(kind, id, others.clone())
                }));
            }
            menu.append(&self.menu_button(&gettextrs::gettext("Delete…"), {
                let window = self.clone();
                let id = usage.subject.id;
                let used = usage.entry_count > 0;
                move || {
                    if used {
                        window.toast(&gettextrs::gettext(
                            "This item is used by entries. Merge it instead of deleting it.",
                        ));
                    } else {
                        window.delete_subject(id);
                    }
                }
            }));
            let popover = gtk::Popover::new();
            popover.set_child(Some(&menu));
            menu_button.set_popover(Some(&popover));
            row.add_suffix(&menu_button);

            let list_row = gtk::ListBoxRow::new();
            list_row.set_child(Some(&row));
            list.append(&list_row);
            row_ids.push(Some(usage.subject.id));
        }
        self.imp().row_ids.replace(row_ids);
    }

    fn present_rename_subject(&self, subject: &Subject) {
        let dialog = adw::AlertDialog::new(Some(&gettextrs::gettext("Rename")), None);
        dialog.add_response("cancel", &gettextrs::gettext("Cancel"));
        dialog.add_response("rename", &gettextrs::gettext("Rename"));
        dialog.set_response_appearance("rename", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("rename"));
        dialog.set_close_response("cancel");
        let entry = gtk::Entry::new();
        entry.set_text(&subject.name);
        entry.set_margin_top(6);
        dialog.set_extra_child(Some(&entry));
        {
            let dialog = dialog.clone();
            let entry = entry.clone();
            entry.connect_changed(move |entry| {
                dialog.set_response_enabled("rename", !entry.text().trim().is_empty());
            });
        }
        let window = self.clone();
        let id = subject.id;
        dialog.connect_response(None, move |_, response| {
            if response == "rename" {
                window.rename_subject_value(id, &entry.text());
            }
        });
        dialog.present(Some(self));
    }

    /// The entry type the New button creates: a dream before 10:00 when the
    /// preference says so, otherwise a journal page (product spec §8).
    fn default_new_type(&self) -> EntryType {
        if self.imp().settings.string("new-entry-type") == "dream-before-10"
            && jiff::Zoned::now().hour() < 10
        {
            return EntryType::Dream;
        }
        EntryType::Journal
    }

    fn new_entry_of(&self, entry_type: EntryType) {
        let created = {
            let mut vault = self.imp().vault.borrow_mut();
            let Some(vault) = vault.as_mut() else {
                return;
            };
            let now = jiff::Zoned::now();
            vault.create_entry(entry_type, "", &now).ok()
        };
        if let Some(id) = created {
            self.imp().selected.replace(None);
            self.refresh();
            self.select_entry(id);
        }
    }

    fn set_dream_flag(&self, flag: DreamFlag, on: bool) {
        let Some(id) = *self.imp().selected.borrow() else {
            return;
        };
        let _ = {
            let mut vault = self.imp().vault.borrow_mut();
            match vault.as_mut() {
                Some(vault) => vault.set_dream_flag(id, flag, on.then_some(true)).is_ok(),
                None => false,
            }
        };
    }

    /// Expands the selected quick note into a journal page or dream
    /// (product spec §6, vault spec §5.2).
    fn expand_selected(&self, target: EntryType) {
        let Some(note) = *self.imp().selected.borrow() else {
            return;
        };
        let created = {
            let mut vault = self.imp().vault.borrow_mut();
            match vault.as_mut() {
                Some(vault) => vault.expand_note(note, target).ok(),
                None => None,
            }
        };
        if let Some(id) = created {
            self.refresh();
            self.select_entry(id);
            let toast = adw::Toast::new(&gettextrs::gettext("Entry created"));
            self.imp().toast_overlay.add_toast(toast);
        }
    }

    /// Rebuilds the entry menu for the selected type; notes get "Expand into"
    /// (ui-spec §3.5).
    fn rebuild_entry_menu(&self, entry_type: EntryType) {
        let menu = gio::Menu::new();
        menu.append(
            Some(&gettextrs::gettext("Delete")),
            Some("win.delete-entry"),
        );
        if entry_type == EntryType::Note {
            let expand = gio::Menu::new();
            expand.append(
                Some(&gettextrs::gettext("Journal page")),
                Some("win.expand-journal"),
            );
            expand.append(Some(&gettextrs::gettext("Dream")), Some("win.expand-dream"));
            menu.append_submenu(Some(&gettextrs::gettext("Expand into")), &expand);
        }
        self.imp().entry_menu.set_menu_model(Some(&menu));
    }

    /// Refreshes the type-specific editor header: dream flags, the expand
    /// button, the "From quick note" banner and the "Expanded into" card.
    fn update_entry_header(&self, entry: &Entry) {
        let imp = self.imp();
        imp.panel_loading.set(true);

        let is_dream = entry.entry_type == EntryType::Dream;
        let is_note = entry.entry_type == EntryType::Note;
        imp.flags_box.set_visible(is_dream);
        imp.flag_lucid.set_active(entry.lucid == Some(true));
        imp.flag_nightmare.set_active(entry.nightmare == Some(true));
        imp.flag_recurring.set_active(entry.recurring == Some(true));
        imp.expand_button.set_visible(is_note);
        imp.note_info_card.set_visible(is_note);

        // Source note link.
        match entry.source_id {
            Some(note_id) => {
                imp.source_note.set(Some(note_id));
                let note = {
                    let vault = imp.vault.borrow();
                    vault.as_ref().and_then(|vault| vault.entry(note_id).ok())
                };
                match note {
                    Some(note) => {
                        imp.source_banner.set_title(&format!(
                            "{} {}",
                            gettextrs::gettext("From quick note:"),
                            quaderno_vault::text::title(&note.content)
                        ));
                        imp.source_banner
                            .set_button_label(Some(&gettextrs::gettext("Open")));
                    }
                    None => {
                        imp.source_banner
                            .set_title(&gettextrs::gettext("From a deleted note"));
                        imp.source_banner.set_button_label(None);
                    }
                }
                imp.source_banner.set_revealed(true);
            }
            None => {
                imp.source_note.set(None);
                imp.source_banner.set_revealed(false);
            }
        }

        // "Expanded into" card (notes only).
        let card = &imp.expanded_card;
        while let Some(child) = card.first_child() {
            card.remove(&child);
        }
        card.set_visible(false);
        if is_note {
            let expanded = {
                let vault = imp.vault.borrow();
                match vault.as_ref() {
                    Some(vault) => vault.expanded_entries(entry.id).unwrap_or_default(),
                    None => Vec::new(),
                }
            };
            if !expanded.is_empty() {
                let title = gtk::Label::new(Some(&gettextrs::gettext("Expanded into")));
                title.set_xalign(0.0);
                title.add_css_class("heading");
                title.set_margin_top(6);
                title.set_margin_start(6);
                title.set_margin_end(6);
                card.append(&title);
                for expanded_id in expanded {
                    let summary = {
                        let vault = imp.vault.borrow();
                        vault
                            .as_ref()
                            .and_then(|vault| vault.entry(expanded_id).ok())
                            .map(|entry| {
                                (
                                    entry_type_label(entry.entry_type),
                                    quaderno_vault::text::title(&entry.content),
                                )
                            })
                    };
                    if let Some((kind, title)) = summary {
                        let button = gtk::Button::with_label(&format!("{kind} · {title}"));
                        button.add_css_class("flat");
                        button.set_halign(gtk::Align::Start);
                        button.set_margin_start(6);
                        button.set_margin_end(6);
                        button.set_margin_bottom(6);
                        let window = self.clone();
                        button.connect_clicked(move |_| window.select_entry(expanded_id));
                        card.append(&button);
                    }
                }
                card.set_visible(true);
            }
        }

        imp.panel_loading.set(false);
    }

    // ---- preferences (M6, General page) ---------------------------------

    fn present_about(&self) {
        let about = adw::AboutDialog::builder()
            .application_name("Quaderno")
            .application_icon(crate::application::APP_ID)
            .version(env!("CARGO_PKG_VERSION"))
            .developer_name("Paolo Pustorino")
            .developers(["Paolo Pustorino <stickgrinder@gmail.com>"])
            .website("https://github.com/stickgrinder/quaderno")
            .license_type(gtk::License::Gpl30)
            .build();
        about.present(Some(self));
    }

    fn build_general_page(&self) -> adw::PreferencesPage {
        let page = adw::PreferencesPage::new();
        page.set_title(&gettextrs::gettext("General"));
        page.set_icon_name(Some("preferences-system-symbolic"));

        page.add(&self.build_writing_group());

        let content_group = adw::PreferencesGroup::new();

        let size_adjustment = gtk::Adjustment::new(
            f64::from(self.imp().settings.int("text-size").clamp(14, 26)),
            14.0,
            26.0,
            1.0,
            4.0,
            0.0,
        );
        let size_row = adw::SpinRow::new(Some(&size_adjustment), 1.0, 0);
        size_row.set_title(&gettextrs::gettext("Text size"));
        size_row.set_subtitle(&gettextrs::gettext("Entry text size, in points"));
        {
            let window = self.clone();
            size_row.connect_notify_local(Some("value"), move |row, _| {
                let value = row.value().round() as i32;
                let _ = window
                    .imp()
                    .settings
                    .set_int("text-size", value.clamp(14, 26));
                window.apply_writing_font();
            });
        }
        content_group.add(&size_row);

        let width_row = adw::ActionRow::builder()
            .title(gettextrs::gettext("Line width"))
            .subtitle(gettextrs::gettext("Maximum width of the text column"))
            .build();
        let width_group = adw::ToggleGroup::new();
        width_group.set_valign(gtk::Align::Center);
        for (name, label) in [
            ("narrow", gettextrs::gettext("Narrow")),
            ("comfortable", gettextrs::gettext("Comfortable")),
            ("wide", gettextrs::gettext("Wide")),
        ] {
            let toggle = adw::Toggle::new();
            toggle.set_name(Some(name));
            toggle.set_label(Some(&label));
            width_group.add(toggle);
        }
        width_group.set_active_name(Some(&self.imp().settings.string("line-width")));
        {
            let window = self.clone();
            width_group.connect_active_name_notify(move |group| {
                if let Some(name) = group.active_name() {
                    let _ = window.imp().settings.set_string("line-width", &name);
                    window.apply_line_width();
                }
            });
        }
        width_row.add_suffix(&width_group);
        content_group.add(&width_row);

        let symbols = gtk::StringList::new(&[&gettextrs::gettext("Dimmed")]);
        let symbols_row = adw::ComboRow::builder()
            .title(gettextrs::gettext("Markdown symbols"))
            .subtitle(gettextrs::gettext("Hidden comes in a later version"))
            .model(&symbols)
            .sensitive(false)
            .build();
        symbols_row.set_selected(0);
        content_group.add(&symbols_row);

        content_group.add(&self.build_spell_row());
        page.add(&content_group);

        page.add(&self.build_behavior_group());
        page
    }

    fn build_writing_group(&self) -> adw::PreferencesGroup {
        let group = adw::PreferencesGroup::builder()
            .title(gettextrs::gettext("Writing font"))
            .description(gettextrs::gettext(
                "Used for entry text and quick notes. Interface text always follows the system font.",
            ))
            .build();

        let flow = gtk::FlowBox::new();
        flow.set_selection_mode(gtk::SelectionMode::None);
        flow.set_column_spacing(6);
        flow.set_row_spacing(6);
        flow.set_max_children_per_line(4);
        flow.set_homogeneous(true);

        let current = self.imp().settings.string("writing-font").to_string();
        let mut cards: Vec<(String, gtk::ToggleButton)> = Vec::new();
        for (value, name, subtitle, sample_class) in [
            (
                "serif",
                gettextrs::gettext("Serif"),
                gettextrs::gettext("Source Serif 4 · bundled"),
                "font-sample-serif",
            ),
            (
                "sans",
                gettextrs::gettext("Sans"),
                gettextrs::gettext("System document font"),
                "font-sample-sans",
            ),
            (
                "mono",
                gettextrs::gettext("Mono"),
                gettextrs::gettext("System monospace font"),
                "font-sample-mono",
            ),
            (
                "custom",
                gettextrs::gettext("Custom…"),
                gettextrs::gettext("Any installed font"),
                "font-sample-serif",
            ),
        ] {
            let button = gtk::ToggleButton::new();
            button.set_hexpand(true);
            button.add_css_class("font-card");
            button.set_active(current == value);
            let content = gtk::Box::new(gtk::Orientation::Vertical, 2);
            content.set_margin_top(9);
            content.set_margin_bottom(9);
            content.set_margin_start(9);
            content.set_margin_end(9);
            let sample = gtk::Label::new(Some("Aa"));
            sample.add_css_class("font-sample");
            sample.add_css_class(sample_class);
            sample.set_xalign(0.0);
            content.append(&sample);
            let title = gtk::Label::new(Some(&name));
            title.set_xalign(0.0);
            title.add_css_class("heading");
            content.append(&title);
            let note = gtk::Label::new(Some(&subtitle));
            note.set_xalign(0.0);
            note.set_wrap(true);
            note.add_css_class("dim-label");
            note.add_css_class("caption");
            content.append(&note);
            button.set_child(Some(&content));
            flow.append(&button);
            cards.push((value.to_owned(), button));
        }

        for (value, button) in &cards {
            let value = value.clone();
            let cards = cards.clone();
            let window = self.clone();
            button.connect_clicked(move |button| {
                if !button.is_active() {
                    return;
                }
                for (_, other) in &cards {
                    if other != button {
                        other.set_active(false);
                    }
                }
                if value == "custom" {
                    window.choose_custom_font();
                } else {
                    let _ = window.imp().settings.set_string("writing-font", &value);
                    window.apply_writing_font();
                }
            });
        }
        group.add(&flow);

        let preview = gtk::Label::new(Some(
            "Woke up late for once, nine hours of sleep and no alarm.\n\
             Marta suggested we take the train to Lecco and just walk along the water until we got hungry.",
        ));
        preview.set_wrap(true);
        preview.set_xalign(0.0);
        preview.add_css_class("writing-preview");
        preview.set_margin_top(12);
        preview.set_margin_bottom(12);
        preview.set_margin_start(12);
        preview.set_margin_end(12);
        let preview_clamp = adw::Clamp::new();
        preview_clamp.set_maximum_size((PREVIEW_WIDTH * self.line_width_fraction()).round() as i32);
        preview_clamp.set_hexpand(true);
        preview_clamp.add_css_class("card");
        preview_clamp.set_margin_top(6);
        preview_clamp.set_margin_bottom(6);
        preview_clamp.set_margin_start(6);
        preview_clamp.set_margin_end(6);
        preview_clamp.set_child(Some(&preview));
        self.imp()
            .preview_clamp
            .replace(Some(preview_clamp.clone()));
        group.add(&preview_clamp);

        group
    }

    /// Opens a font chooser and stores the chosen family as the custom font
    /// (the size stays governed by the Text size setting).
    fn choose_custom_font(&self) {
        let dialog = gtk::FontDialog::new();
        let window = self.clone();
        dialog.choose_font(
            Some(self),
            None::<&gtk::pango::FontDescription>,
            gio::Cancellable::NONE,
            move |result| {
                let Ok(font) = result else {
                    return;
                };
                if let Some(family) = font.family() {
                    let _ = window
                        .imp()
                        .settings
                        .set_string("custom-font", family.as_str());
                    let _ = window.imp().settings.set_string("writing-font", "custom");
                    window.apply_writing_font();
                }
            },
        );
    }

    fn build_spell_row(&self) -> adw::ActionRow {
        let row = adw::ActionRow::builder()
            .title(gettextrs::gettext("Spell checking"))
            .subtitle(gettextrs::gettext("Languages from the system dictionaries"))
            .build();
        let button = gtk::MenuButton::new();
        button.set_valign(gtk::Align::Center);

        let provider = libspelling::Provider::default();
        let languages = provider.list_languages();
        let mut checks: Vec<(String, gtk::CheckButton)> = Vec::new();
        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::None);
        let system = gtk::CheckButton::with_label(&gettextrs::gettext("System default"));
        system.set_margin_top(6);
        system.set_margin_bottom(6);
        system.set_margin_start(6);
        system.set_margin_end(6);
        let system_row = gtk::ListBoxRow::new();
        system_row.set_child(Some(&system));
        list.append(&system_row);
        for index in 0..languages.n_items() {
            let Some(language) = languages
                .item(index)
                .and_downcast::<libspelling::Language>()
            else {
                continue;
            };
            let Some(code) = language.code() else {
                continue;
            };
            let label = language
                .name()
                .or_else(|| language.group())
                .unwrap_or_else(|| code.clone());
            let check = gtk::CheckButton::with_label(&label);
            check.set_margin_top(6);
            check.set_margin_bottom(6);
            check.set_margin_start(6);
            check.set_margin_end(6);
            let list_row = gtk::ListBoxRow::new();
            list_row.set_child(Some(&check));
            list.append(&list_row);
            checks.push((code.to_string(), check));
        }
        let checks = Rc::new(checks);
        let loading = Rc::new(Cell::new(false));

        let stored = self.imp().settings.string("spell-languages").to_string();
        if stored.is_empty() {
            system.set_active(true);
        } else {
            for (code, check) in checks.iter() {
                if stored.split(',').any(|part| part == code) {
                    check.set_active(true);
                }
            }
        }
        update_spell_label(&button, &stored, &checks);

        let refresh = {
            let window = self.clone();
            let refresh_checks = checks.clone();
            let loading = loading.clone();
            let button = button.clone();
            let system = system.clone();
            Rc::new(move || {
                if loading.get() {
                    return;
                }
                loading.set(true);
                if system.is_active() {
                    for (_, check) in refresh_checks.iter() {
                        check.set_active(false);
                    }
                }
                let mut codes: Vec<String> = refresh_checks
                    .iter()
                    .filter(|(_, check)| check.is_active())
                    .map(|(code, _)| code.clone())
                    .collect();
                if system.is_active() || codes.is_empty() {
                    system.set_active(true);
                } else {
                    system.set_active(false);
                }
                codes.sort();
                let joined = codes.join(",");
                let _ = window.imp().settings.set_string("spell-languages", &joined);
                window.apply_spell_languages();
                update_spell_label(&button, &joined, &refresh_checks);
                loading.set(false);
            })
        };
        for (_, check) in checks.iter() {
            let refresh = refresh.clone();
            check.connect_toggled(move |_| refresh());
        }
        {
            let refresh = refresh.clone();
            system.connect_toggled(move |_| refresh());
        }

        let scrolled = gtk::ScrolledWindow::builder()
            .child(&list)
            .min_content_height(260)
            .min_content_width(240)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build();
        let popover = gtk::Popover::new();
        popover.set_child(Some(&scrolled));
        button.set_popover(Some(&popover));
        row.add_suffix(&button);
        row
    }

    fn build_behavior_group(&self) -> adw::PreferencesGroup {
        let group = adw::PreferencesGroup::builder()
            .title(gettextrs::gettext("Behavior"))
            .build();

        let new_types = gtk::StringList::new(&[
            &gettextrs::gettext("Journal page"),
            &gettextrs::gettext("Before 10:00 Dream, then Journal page"),
        ]);
        let new_row = adw::ComboRow::builder()
            .title(gettextrs::gettext("New entry type"))
            .model(&new_types)
            .build();
        new_row.set_selected(
            if self.imp().settings.string("new-entry-type") == "dream-before-10" {
                1
            } else {
                0
            },
        );
        {
            let window = self.clone();
            new_row.connect_selected_notify(move |row| {
                let value = if row.selected() == 1 {
                    "dream-before-10"
                } else {
                    "journal"
                };
                let _ = window.imp().settings.set_string("new-entry-type", value);
            });
        }
        group.add(&new_row);

        let end_values: Vec<String> = (0..=12)
            .map(|step| {
                let hours = step / 2;
                let minutes = (step % 2) * 30;
                format!("{hours:02}:{minutes:02}")
            })
            .collect();
        let end_list =
            gtk::StringList::new(&end_values.iter().map(String::as_str).collect::<Vec<_>>());
        let end_row = adw::ComboRow::builder()
            .title(gettextrs::gettext("A day ends at"))
            .subtitle(gettextrs::gettext(
                "Entries written before this time count for the previous day",
            ))
            .model(&end_list)
            .build();
        let day_end = {
            let vault = self.imp().vault.borrow();
            vault.as_ref().and_then(|vault| vault.day_end().ok())
        };
        if let Some((hours, minutes)) = day_end {
            let index = u32::from(hours) * 2 + u32::from(minutes) / 30;
            end_row.set_selected(index.min(12));
        }
        end_row.set_sensitive(!self.imp().read_only.get() && day_end.is_some());
        {
            let window = self.clone();
            end_row.connect_selected_notify(move |row| {
                let selected = row.selected();
                let hours = (selected / 2) as u8;
                let minutes = ((selected % 2) * 30) as u8;
                let saved = {
                    let mut vault = window.imp().vault.borrow_mut();
                    match vault.as_mut() {
                        Some(vault) => vault.set_day_end((hours, minutes)).is_ok(),
                        None => false,
                    }
                };
                if saved {
                    window.refresh();
                }
            });
        }
        group.add(&end_row);

        group
    }

    fn build_privacy_page(&self) -> adw::PreferencesPage {
        let page = adw::PreferencesPage::new();
        page.set_title(&gettextrs::gettext("Privacy & data"));
        page.set_icon_name(Some("security-high-symbolic"));

        // Unlocking.
        let unlocking = adw::PreferencesGroup::builder()
            .title(gettextrs::gettext("Unlocking"))
            .build();

        let remember = adw::SwitchRow::builder()
            .title(gettextrs::gettext("Remember passphrase on this device"))
            .subtitle(gettextrs::gettext(
                "Stored in GNOME Keyring; the journal opens without asking",
            ))
            .build();
        let loading = Rc::new(Cell::new(true));
        if let (Some(path), Some(keyring)) = (self.imp().path.borrow().clone(), self.keyring()) {
            let remember = remember.clone();
            let loading = loading.clone();
            let keyring_available = keyring.available();
            glib::spawn_future_local(async move {
                if !matches!(keyring_available.await, Ok(true)) {
                    remember.set_sensitive(false);
                    remember.set_subtitle(&gettextrs::gettext(
                        "No keyring is available on this device",
                    ));
                    loading.set(false);
                    return;
                }
                if let Ok(Ok(Some(_))) = keyring.lookup(path.to_string_lossy().into_owned()).await {
                    remember.set_active(true);
                }
                loading.set(false);
            });
        } else {
            remember.set_sensitive(false);
            loading.set(false);
        }
        {
            let window = self.clone();
            let loading = loading.clone();
            remember.connect_active_notify(move |row| {
                if loading.get() {
                    return;
                }
                let keyring = window.keyring();
                let path = window.imp().path.borrow().clone();
                if row.is_active() {
                    let row = row.clone();
                    let keyring = keyring.clone();
                    let path = path.clone();
                    window.prompt_passphrase(
                        gettextrs::gettext("Remember passphrase"),
                        gettextrs::gettext(
                            "Enter the passphrase to store it in GNOME Keyring. The journal will open without asking on this device.",
                        ),
                        Rc::new(move |passphrase| {
                            if let (Some(keyring), Some(path)) = (keyring.clone(), path.clone()) {
                                let stored = keyring.store(
                                    path.to_string_lossy().into_owned(),
                                    Zeroizing::new(passphrase.into_bytes()),
                                );
                                let row = row.clone();
                                glib::spawn_future_local(async move {
                                    if !matches!(stored.await, Ok(Ok(()))) {
                                        row.set_active(false);
                                    }
                                });
                            }
                        }),
                    );
                } else if let (Some(keyring), Some(path)) = (keyring.clone(), path.clone()) {
                    let receiver = keyring.delete(path.to_string_lossy().into_owned());
                    glib::spawn_future_local(async move {
                        let _ = receiver.await;
                    });
                }
            });
        }
        unlocking.add(&remember);

        let lock_minutes = [0, 1, 2, 5, 10, 15, 30, 60];
        let lock_labels: Vec<String> = lock_minutes.iter().map(|m| lock_label(*m)).collect();
        let lock_list =
            gtk::StringList::new(&lock_labels.iter().map(String::as_str).collect::<Vec<_>>());
        let lock_row = adw::ComboRow::builder()
            .title(gettextrs::gettext("Lock after"))
            .model(&lock_list)
            .build();
        let current_minutes = self.imp().settings.int("auto-lock-minutes");
        let index = lock_minutes
            .iter()
            .position(|m| *m == current_minutes)
            .unwrap_or(3);
        lock_row.set_selected(index as u32);
        {
            let settings = self.imp().settings.clone();
            lock_row.connect_selected_notify(move |row| {
                if let Some(minutes) = lock_minutes.get(row.selected() as usize) {
                    let _ = settings.set_int("auto-lock-minutes", *minutes);
                }
            });
        }
        unlocking.add(&lock_row);

        let session_lock = adw::SwitchRow::builder()
            .title(gettextrs::gettext("Lock when the session locks"))
            .build();
        session_lock.set_active(self.imp().settings.boolean("lock-on-session-lock"));
        {
            let settings = self.imp().settings.clone();
            session_lock.connect_active_notify(move |row| {
                let _ = settings.set_boolean("lock-on-session-lock", row.is_active());
            });
        }
        unlocking.add(&session_lock);

        let passphrase_row = adw::ActionRow::builder()
            .title(gettextrs::gettext("Passphrase"))
            .subtitle(gettextrs::gettext(
                "The journal is always encrypted. A forgotten passphrase cannot be recovered.",
            ))
            .build();
        let change_button = gtk::Button::builder()
            .label(gettextrs::gettext("Change…"))
            .valign(gtk::Align::Center)
            .build();
        {
            let window = self.clone();
            change_button.connect_clicked(move |_| window.present_change_passphrase());
        }
        passphrase_row.add_suffix(&change_button);
        unlocking.add(&passphrase_row);
        page.add(&unlocking);

        // Data.
        let data = adw::PreferencesGroup::builder()
            .title(gettextrs::gettext("Data"))
            .build();
        let file_row = adw::ActionRow::builder()
            .title(gettextrs::gettext("Journal file"))
            .build();
        if let Some(path) = self.imp().path.borrow().clone() {
            file_row.set_subtitle(&pretty_path(&path));
        }
        let move_button = gtk::Button::builder()
            .label(gettextrs::gettext("Move…"))
            .valign(gtk::Align::Center)
            .build();
        {
            let window = self.clone();
            let file_row = file_row.clone();
            move_button.connect_clicked(move |_| window.present_move_vault(file_row.clone()));
        }
        file_row.add_suffix(&move_button);
        data.add(&file_row);
        page.add(&data);

        page
    }

    /// Shows a one-field dialog asking for the vault passphrase.
    fn prompt_passphrase(&self, title: String, body: String, action: Rc<dyn Fn(String)>) {
        let dialog = adw::AlertDialog::new(Some(&title), Some(&body));
        dialog.add_response("cancel", &gettextrs::gettext("Cancel"));
        dialog.add_response("confirm", &gettextrs::gettext("Continue"));
        dialog.set_response_appearance("confirm", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("confirm"));
        dialog.set_close_response("cancel");

        let entry = adw::PasswordEntryRow::new();
        entry.set_title(&gettextrs::gettext("Passphrase"));
        let message = message_label();
        let group = adw::PreferencesGroup::new();
        group.add(&entry);
        group.add(&message);
        dialog.set_extra_child(Some(&group));
        dialog.set_response_enabled("confirm", false);

        let path = self.imp().path.borrow().clone();
        let window = self.clone();
        let dialog_for_entry = dialog.clone();
        let message_for_entry = message.clone();
        let entry_for_entry = entry.clone();
        let source: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
        entry.connect_changed(move |row| {
            if let Some(source) = source.borrow_mut().take() {
                source.remove();
            }
            dialog_for_entry.set_response_enabled("confirm", false);
            let value = row.text().to_string();
            if value.is_empty() {
                set_message(&message_for_entry, "", false);
                return;
            }
            let Some(path) = path.clone() else {
                // No vault path to verify against: accept any non-empty value.
                set_message(&message_for_entry, "", false);
                dialog_for_entry.set_response_enabled("confirm", true);
                return;
            };
            set_message(&message_for_entry, &gettextrs::gettext("Checking…"), false);
            let window = window.clone();
            let entry = entry_for_entry.clone();
            let message = message_for_entry.clone();
            let dialog = dialog_for_entry.clone();
            let source2 = source.clone();
            let id = glib::timeout_add_local(Duration::from_millis(500), move || {
                source2.borrow_mut().take();
                let entry = entry.clone();
                let value = value.clone();
                window.verify_passphrase(path.clone(), value.clone(), {
                    let message = message.clone();
                    let dialog = dialog.clone();
                    Rc::new(move |ok| {
                        if entry.text() != value {
                            return;
                        }
                        if ok {
                            set_message(&message, "", false);
                        } else {
                            set_message(&message, &gettextrs::gettext("Wrong passphrase."), true);
                        }
                        dialog.set_response_enabled("confirm", ok);
                    })
                });
                glib::ControlFlow::Break
            });
            *source.borrow_mut() = Some(id);
        });

        dialog.connect_response(None, move |_, response| {
            if response == "confirm" {
                action(entry.text().to_string());
            }
        });
        dialog.present(Some(self));
    }

    fn present_change_passphrase(&self) {
        let dialog = adw::AlertDialog::new(
            Some(&gettextrs::gettext("Change passphrase")),
            Some(&gettextrs::gettext(
                "The journal is rebuilt with the new passphrase. A forgotten passphrase cannot be recovered.",
            )),
        );
        dialog.add_response("cancel", &gettextrs::gettext("Cancel"));
        dialog.add_response("change", &gettextrs::gettext("Change"));
        dialog.set_response_appearance("change", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("change"));
        dialog.set_close_response("cancel");

        let current = adw::PasswordEntryRow::new();
        current.set_title(&gettextrs::gettext("Current passphrase"));
        let new = adw::PasswordEntryRow::new();
        new.set_title(&gettextrs::gettext("New passphrase"));
        let repeat = adw::PasswordEntryRow::new();
        repeat.set_title(&gettextrs::gettext("Repeat new passphrase"));
        let current_message = message_label();
        let new_message = message_label();
        let repeat_message = message_label();
        let group = adw::PreferencesGroup::new();
        group.add(&current);
        group.add(&current_message);
        group.add(&new);
        group.add(&new_message);
        group.add(&repeat);
        group.add(&repeat_message);
        dialog.set_extra_child(Some(&group));
        dialog.set_response_enabled("change", false);

        let current_ok = Rc::new(Cell::new(false));
        let refresh: Rc<dyn Fn()> = {
            let dialog = dialog.clone();
            let new = new.clone();
            let repeat = repeat.clone();
            let new_message = new_message.clone();
            let repeat_message = repeat_message.clone();
            let current_ok = current_ok.clone();
            Rc::new(move || {
                let new_text = new.text();
                let length = new_text.chars().count();
                if length > 0 && length < crate::state::MIN_PASSPHRASE_CHARS {
                    set_message(
                        &new_message,
                        &gettextrs::gettext("At least eight characters"),
                        true,
                    );
                } else {
                    set_message(&new_message, "", false);
                }
                if !repeat.text().is_empty() && repeat.text() != new_text {
                    set_message(
                        &repeat_message,
                        &gettextrs::gettext("The passphrases do not match"),
                        true,
                    );
                } else {
                    set_message(&repeat_message, "", false);
                }
                let valid = current_ok.get()
                    && length >= crate::state::MIN_PASSPHRASE_CHARS
                    && new_text == repeat.text();
                dialog.set_response_enabled("change", valid);
            })
        };

        if let Some(path) = self.imp().path.borrow().clone() {
            let window = self.clone();
            let dialog = dialog.clone();
            let current_widget = current.clone();
            let current_message = current_message.clone();
            let current_ok = current_ok.clone();
            let refresh = refresh.clone();
            let source: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
            current.connect_changed(move |row| {
                if let Some(source) = source.borrow_mut().take() {
                    source.remove();
                }
                let value = row.text().to_string();
                current_ok.set(false);
                dialog.set_response_enabled("change", false);
                if value.is_empty() {
                    set_message(&current_message, "", false);
                    return;
                }
                set_message(&current_message, &gettextrs::gettext("Checking…"), false);
                let window = window.clone();
                let path = path.clone();
                let current_widget = current_widget.clone();
                let current_message = current_message.clone();
                let current_ok = current_ok.clone();
                let refresh = refresh.clone();
                let source2 = source.clone();
                let id = glib::timeout_add_local(Duration::from_millis(500), move || {
                    source2.borrow_mut().take();
                    let current_widget = current_widget.clone();
                    let value = value.clone();
                    window.verify_passphrase(path.clone(), value.clone(), {
                        let current_message = current_message.clone();
                        let current_ok = current_ok.clone();
                        let refresh = refresh.clone();
                        Rc::new(move |ok| {
                            if current_widget.text() != value {
                                return;
                            }
                            current_ok.set(ok);
                            if ok {
                                set_message(&current_message, "", false);
                            } else {
                                set_message(
                                    &current_message,
                                    &gettextrs::gettext("Wrong passphrase."),
                                    true,
                                );
                            }
                            refresh();
                        })
                    });
                    glib::ControlFlow::Break
                });
                *source.borrow_mut() = Some(id);
            });
        }

        {
            let refresh = refresh.clone();
            new.connect_changed(move |_| refresh());
        }
        repeat.connect_changed(move |_| refresh());
        {
            let window = self.clone();
            let current = current.clone();
            let new = new.clone();
            dialog.connect_response(None, move |_, response| {
                if response == "change" {
                    window.change_passphrase(current.text().to_string(), new.text().to_string());
                }
            });
        }
        dialog.present(Some(self));
    }

    /// Changes the passphrase: validate the current one, rebuild the file, update
    /// the keyring, reopen (vault-spec §2.4).
    fn change_passphrase(&self, current: String, new: String) {
        let Some(path) = self.imp().path.borrow().clone() else {
            return;
        };
        self.depart_current();
        let window = self.clone();
        glib::spawn_future_local(async move {
            let valid = gio::spawn_blocking({
                let path = path.clone();
                let current = current.clone();
                move || Vault::validate(&path, &current).is_ok()
            })
            .await
            .unwrap_or(false);
            if !valid {
                window.toast(&gettextrs::gettext("Wrong passphrase."));
                return;
            }

            // Close the open connection before the file is rebuilt.
            let _ = window.imp().vault.borrow_mut().take();

            let outcome = gio::spawn_blocking({
                let path = path.clone();
                let current = current.clone();
                let new = new.clone();
                move || Vault::change_passphrase(&path, &current, &new)
            })
            .await;
            let passphrase = if matches!(outcome, Ok(Ok(()))) {
                new
            } else {
                window.toast(&gettextrs::gettext("The passphrase could not be changed."));
                current
            };

            // Update the keyring entry, if any, to the passphrase now in force.
            // Reopen first, so a slow or unavailable keyring never stalls the UI.
            let reopened = gio::spawn_blocking({
                let path = path.clone();
                let passphrase = passphrase.clone();
                move || Vault::open(&path, &passphrase)
            })
            .await;
            match reopened {
                Ok(Ok(vault)) => {
                    let access = vault.access();
                    window.imp().vault.replace(Some(vault));
                    window.show_main(access);
                    window.toast(&gettextrs::gettext("Passphrase changed"));
                    if let Some(keyring) = window.keyring() {
                        let key = path.to_string_lossy().into_owned();
                        glib::spawn_future_local(async move {
                            if let Ok(Ok(Some(_))) = keyring.lookup(key.clone()).await {
                                let stored =
                                    keyring.store(key, Zeroizing::new(passphrase.into_bytes()));
                                let _ = stored.await;
                            }
                        });
                    }
                }
                _ => window.show_error(&gettextrs::gettext("The journal could not be reopened.")),
            }
        });
    }

    fn present_move_vault(&self, file_row: adw::ActionRow) {
        if self.imp().path.borrow().is_none() {
            return;
        }
        let dialog = gtk::FileDialog::builder()
            .title(gettextrs::gettext("Move journal file"))
            .initial_name("journal.quaderno")
            .build();
        let window = self.clone();
        dialog.save(Some(self), gio::Cancellable::NONE, move |result| {
            if let Ok(file) = result
                && let Some(dest) = file.path()
            {
                let dest = dest.clone();
                let caller = window.clone();
                let file_row = file_row.clone();
                caller.prompt_passphrase(
                    gettextrs::gettext("Move journal file"),
                    gettextrs::gettext(
                        "Enter the passphrase to verify the copy before the journal is moved.",
                    ),
                    Rc::new(move |passphrase| {
                        window
                            .clone()
                            .move_vault(dest.clone(), passphrase, file_row.clone())
                    }),
                );
            }
        });
    }

    /// Copies the vault to `dest`, verifies the copy, removes the original and
    /// reopens at the new path (product spec §8).
    fn move_vault(&self, dest: PathBuf, passphrase: String, file_row: adw::ActionRow) {
        let Some(old) = self.imp().path.borrow().clone() else {
            return;
        };
        self.depart_current();
        let window = self.clone();
        glib::spawn_future_local(async move {
            let moved = gio::spawn_blocking({
                let old = old.clone();
                let dest = dest.clone();
                let passphrase = passphrase.clone();
                move || {
                    let vault = Vault::open(&old, &passphrase)?;
                    let temporary = dest.with_extension("quaderno-moving");
                    let result = (|| {
                        vault.backup(&temporary, &passphrase)?;
                        Vault::validate(&temporary, &passphrase)?;
                        std::fs::rename(&temporary, &dest)
                            .map_err(|error| VaultError::io(dest.clone(), error))?;
                        std::fs::remove_file(&old)
                            .map_err(|error| VaultError::io(old.clone(), error))?;
                        Ok::<(), VaultError>(())
                    })();
                    if result.is_err() {
                        let _ = std::fs::remove_file(&temporary);
                    }
                    result
                }
            })
            .await;
            if !matches!(moved, Ok(Ok(()))) {
                window.toast(&gettextrs::gettext("The journal could not be moved."));
                return;
            }

            // The old connection now points at a removed file; reopen the copy
            // first so a slow keyring never stalls the UI.
            let _ = window.imp().vault.borrow_mut().take();
            let _ = window
                .imp()
                .settings
                .set_string("vault-path", &dest.to_string_lossy());

            let reopened = gio::spawn_blocking({
                let dest = dest.clone();
                let passphrase = passphrase.clone();
                move || Vault::open(&dest, &passphrase)
            })
            .await;
            match reopened {
                Ok(Ok(vault)) => {
                    let access = vault.access();
                    window.imp().path.replace(Some(dest.clone()));
                    window.imp().vault.replace(Some(vault));
                    window.show_main(access);
                    file_row.set_subtitle(&pretty_path(&dest));
                    window.toast(&gettextrs::gettext("Journal moved"));
                    if let Some(keyring) = window.keyring() {
                        let old_key = old.to_string_lossy().into_owned();
                        let new_key = dest.to_string_lossy().into_owned();
                        glib::spawn_future_local(async move {
                            if let Ok(Ok(Some(_))) = keyring.lookup(old_key.clone()).await {
                                let stored =
                                    keyring.store(new_key, Zeroizing::new(passphrase.into_bytes()));
                                let _ = stored.await;
                            }
                            let _ = keyring.delete(old_key).await;
                        });
                    }
                }
                _ => window.show_error(&gettextrs::gettext("The journal could not be reopened.")),
            }
        });
    }

    fn toast(&self, message: &str) {
        let toast = adw::Toast::new(message);
        if let Some(dialog) = self.imp().preferences.borrow().as_ref()
            && dialog.is_visible()
        {
            dialog.add_toast(toast);
            return;
        }
        self.imp().toast_overlay.add_toast(toast);
    }

    /// Checks a passphrase against the vault file off the main thread.
    fn verify_passphrase(&self, path: PathBuf, passphrase: String, on_result: Rc<dyn Fn(bool)>) {
        glib::spawn_future_local(async move {
            let ok = gio::spawn_blocking(move || Vault::open(&path, &passphrase).is_ok())
                .await
                .unwrap_or(false);
            on_result(ok);
        });
    }

    fn present_preferences(&self) {
        let dialog = adw::PreferencesDialog::new();
        dialog.set_title(&gettextrs::gettext("Preferences"));
        dialog.add(&self.build_general_page());
        dialog.add(&self.build_entries_page());
        dialog.add(&self.build_privacy_page());
        // Toasts raised while the dialog is open (e.g. a finished Move) are shown
        // on the dialog, above its content, not behind it.
        self.imp().preferences.replace(Some(dialog.clone()));
        let window = self.clone();
        dialog.connect_closed(move |_| {
            window.imp().preferences.replace(None);
        });
        dialog.present(Some(self));
    }

    // ---- preferences: Entries (list management, icon picker) -------------

    fn build_entries_page(&self) -> adw::PreferencesPage {
        let page = adw::PreferencesPage::new();
        page.set_title(&gettextrs::gettext("Entries"));
        page.set_icon_name(Some("view-list-symbolic"));
        page.add(&self.build_choice_group(ChoiceKind::Emotion, &gettextrs::gettext("Emotions")));
        page.add(
            &self.build_choice_group(ChoiceKind::Activity, &gettextrs::gettext("Day activities")),
        );
        page.add(&self.fixed_scales_note());
        page
    }

    fn fixed_scales_note(&self) -> adw::PreferencesGroup {
        let group = adw::PreferencesGroup::new();
        let row = adw::ActionRow::builder()
            .title(gettextrs::gettext("About these lists"))
            .subtitle(gettextrs::gettext(
                "Mood, energy, cognitive load, sleep and colours are fixed scales so statistics stay comparable. People, places, things and tags are managed from their Collections pages.",
            ))
            .build();
        row.add_prefix(&gtk::Image::from_icon_name("dialog-information-symbolic"));
        group.add(&row);
        group
    }

    /// Builds a group of a kind's choices with a refresh closure that rebuilds
    /// its rows after a change.
    fn build_choice_group(&self, kind: ChoiceKind, title: &str) -> adw::PreferencesGroup {
        let group = adw::PreferencesGroup::builder().title(title).build();
        let rows: Rc<RefCell<Vec<gtk::Widget>>> = Rc::new(RefCell::new(Vec::new()));
        let refresh: Rc<dyn Fn()> = {
            let window = self.clone();
            let group = group.clone();
            let rows = rows.clone();
            let cell: RefreshCell = Rc::new(RefCell::new(None));
            let built = {
                let cell = cell.clone();
                Rc::new(move || {
                    let Some(refresh) = cell.borrow().clone() else {
                        return;
                    };
                    for row in rows.borrow().iter() {
                        group.remove(row);
                    }
                    rows.borrow_mut().clear();
                    for choice in window.choices(kind) {
                        let row = window.build_choice_row(&choice, kind, refresh.clone());
                        group.add(&row);
                        rows.borrow_mut().push(row.upcast());
                    }
                })
            };
            cell.replace(Some(built.clone()));
            built
        };
        let add_button = gtk::Button::builder()
            .label(gettextrs::gettext("Add"))
            .icon_name("list-add-symbolic")
            .build();
        {
            let window = self.clone();
            let refresh = refresh.clone();
            add_button.connect_clicked(move |_| window.present_add_choice(kind, refresh.clone()));
        }
        group.set_header_suffix(Some(&add_button));
        refresh();
        group
    }

    fn build_choice_row(
        &self,
        choice: &Choice,
        kind: ChoiceKind,
        refresh: Rc<dyn Fn()>,
    ) -> adw::ActionRow {
        let label = details::choice_display_label(choice);
        let usage = self.choices_usage(choice.id);
        // Choice names are plain text (they can contain `&`), so markup must be
        // off before the title is set.
        let row = adw::ActionRow::new();
        row.set_use_markup(false);
        row.set_title(&label);

        let prefix = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let handle = gtk::Image::from_icon_name("list-drag-handle-symbolic");
        handle.add_css_class("dim-label");
        prefix.append(&handle);
        let icon_button = gtk::Button::new();
        icon_button.add_css_class("flat");
        icon_button.add_css_class("circular");
        icon_button.set_child(Some(&gtk::Image::from_icon_name(&icons::resolve(
            &choice.icon,
        ))));
        icon_button.set_tooltip_text(Some(&gettextrs::gettext("Change icon")));
        {
            let window = self.clone();
            let on_pick = refresh.clone();
            let id = choice.id;
            let current = choice.icon.clone();
            let used: HashSet<String> = self
                .choices(kind)
                .into_iter()
                .filter(|other| other.id != id)
                .map(|other| other.icon)
                .collect();
            icon_button.connect_clicked(move |button| {
                let window2 = window.clone();
                let on_pick = on_pick.clone();
                window.present_icon_picker(
                    button,
                    &current,
                    used.clone(),
                    Rc::new(move |icon| {
                        window2.set_choice_icon_value(id, &icon);
                        on_pick();
                    }),
                );
            });
        }
        row.add_prefix(&prefix);

        row.set_subtitle(&choice_subtitle(usage));

        let menu_button = gtk::MenuButton::builder()
            .icon_name("view-more-symbolic")
            .valign(gtk::Align::Center)
            .build();
        let menu = gtk::Box::new(gtk::Orientation::Vertical, 0);
        menu.set_margin_top(6);
        menu.set_margin_bottom(6);
        menu.set_margin_start(6);
        menu.set_margin_end(6);
        menu.append(&self.menu_button(&gettextrs::gettext("Rename…"), {
            let window = self.clone();
            let choice = choice.clone();
            let refresh = refresh.clone();
            move || window.present_rename_choice(&choice, refresh.clone())
        }));
        if choice.label.is_some() {
            menu.append(&self.menu_button(&gettextrs::gettext("Reset name"), {
                let window = self.clone();
                let id = choice.id;
                let refresh = refresh.clone();
                move || {
                    window.reset_choice_name(id);
                    refresh();
                }
            }));
        }
        menu.append(&self.menu_button(&gettextrs::gettext("Move up"), {
            let window = self.clone();
            let id = choice.id;
            let refresh = refresh.clone();
            move || {
                window.move_choice(kind, id, -1);
                refresh();
            }
        }));
        menu.append(&self.menu_button(&gettextrs::gettext("Move down"), {
            let window = self.clone();
            let id = choice.id;
            let refresh = refresh.clone();
            move || {
                window.move_choice(kind, id, 1);
                refresh();
            }
        }));
        let hide_label = if choice.hidden {
            gettextrs::gettext("Show")
        } else {
            gettextrs::gettext("Hide")
        };
        menu.append(&self.menu_button(&hide_label, {
            let window = self.clone();
            let id = choice.id;
            let hidden = choice.hidden;
            let refresh = refresh.clone();
            move || {
                window.set_choice_hidden_value(id, !hidden);
                refresh();
            }
        }));
        menu.append(&self.menu_button(&gettextrs::gettext("Delete…"), {
            let window = self.clone();
            let id = choice.id;
            let can_delete = usage == 0;
            let refresh = refresh.clone();
            move || {
                if can_delete {
                    window.confirm_delete_choice(id, refresh.clone());
                } else {
                    window.toast(&gettextrs::gettext(
                        "This item is used by entries. Hide it, or merge it from its collection.",
                    ));
                }
            }
        }));
        let popover = gtk::Popover::new();
        popover.set_child(Some(&menu));
        menu_button.set_popover(Some(&popover));
        row.add_suffix(&menu_button);

        if choice.hidden {
            let badge = gtk::Label::new(Some(&gettextrs::gettext("Hidden")));
            badge.add_css_class("dim-label");
            badge.add_css_class("caption");
            badge.set_valign(gtk::Align::Center);
            row.add_suffix(&badge);
        }
        row
    }

    fn menu_button(&self, label: &str, action: impl Fn() + 'static) -> gtk::Button {
        let button = gtk::Button::with_label(label);
        button.add_css_class("flat");
        button.set_halign(gtk::Align::Fill);
        let action = Rc::new(action);
        button.connect_clicked(move |button| {
            action();
            if let Some(popover) = button
                .ancestor(gtk::Popover::static_type())
                .and_downcast::<gtk::Popover>()
            {
                popover.popdown();
            }
        });
        button
    }

    fn choices(&self, kind: ChoiceKind) -> Vec<Choice> {
        let vault = self.imp().vault.borrow();
        vault
            .as_ref()
            .and_then(|vault| vault.choices(kind, true).ok())
            .unwrap_or_default()
    }

    fn choices_usage(&self, id: Uuid) -> i64 {
        let vault = self.imp().vault.borrow();
        vault
            .as_ref()
            .and_then(|vault| vault.choice_usage(id).ok())
            .unwrap_or(0)
    }

    fn set_choice_icon_value(&self, id: Uuid, icon: &str) {
        let saved = {
            let mut vault = self.imp().vault.borrow_mut();
            match vault.as_mut() {
                Some(vault) => vault.set_choice_icon(id, icon).is_ok(),
                None => false,
            }
        };
        if saved {
            self.refresh();
        }
    }

    fn reset_choice_name(&self, id: Uuid) {
        let reset = {
            let mut vault = self.imp().vault.borrow_mut();
            match vault.as_mut() {
                Some(vault) => vault.reset_choice_label(id).is_ok(),
                None => false,
            }
        };
        if reset {
            self.refresh();
        }
    }

    fn set_choice_hidden_value(&self, id: Uuid, hidden: bool) {
        let saved = {
            let mut vault = self.imp().vault.borrow_mut();
            match vault.as_mut() {
                Some(vault) => vault.set_choice_hidden(id, hidden).is_ok(),
                None => false,
            }
        };
        if saved {
            self.refresh();
        }
    }

    /// Moves a choice one position up or down by renumbering the kind's order.
    fn move_choice(&self, kind: ChoiceKind, id: Uuid, delta: i32) {
        let mut choices = self.choices(kind);
        let Some(index) = choices.iter().position(|choice| choice.id == id) else {
            return;
        };
        let target = index as i32 + delta;
        if target < 0 || target as usize >= choices.len() {
            return;
        }
        choices.swap(index, target as usize);
        let saved = {
            let mut vault = self.imp().vault.borrow_mut();
            match vault.as_mut() {
                Some(vault) => {
                    let mut ok = true;
                    for (position, choice) in choices.iter().enumerate() {
                        ok &= vault
                            .set_choice_sort_order(choice.id, (position as i64 + 1) * 10)
                            .is_ok();
                    }
                    ok
                }
                None => false,
            }
        };
        if saved {
            self.refresh();
        }
    }

    fn confirm_delete_choice(&self, id: Uuid, refresh: Rc<dyn Fn()>) {
        let dialog = adw::AlertDialog::new(
            Some(&gettextrs::gettext("Delete this item?")),
            Some(&gettextrs::gettext(
                "It is not used by any entry, so deleting it cannot change past entries.",
            )),
        );
        dialog.add_response("cancel", &gettextrs::gettext("Cancel"));
        dialog.add_response("delete", &gettextrs::gettext("Delete"));
        dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let window = self.clone();
        dialog.connect_response(None, move |_, response| {
            if response == "delete" {
                let deleted = {
                    let mut vault = window.imp().vault.borrow_mut();
                    match vault.as_mut() {
                        Some(vault) => vault.soft_delete_choice(id).is_ok(),
                        None => false,
                    }
                };
                if deleted {
                    window.refresh();
                    refresh();
                }
            }
        });
        dialog.present(Some(self));
    }

    fn present_rename_choice(&self, choice: &Choice, refresh: Rc<dyn Fn()>) {
        let dialog = adw::AlertDialog::new(Some(&gettextrs::gettext("Rename")), None);
        dialog.add_response("cancel", &gettextrs::gettext("Cancel"));
        dialog.add_response("rename", &gettextrs::gettext("Rename"));
        dialog.set_response_appearance("rename", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("rename"));
        dialog.set_close_response("cancel");
        let entry = gtk::Entry::new();
        entry.set_text(&details::choice_display_label(choice));
        entry.set_margin_top(6);
        dialog.set_extra_child(Some(&entry));
        {
            let dialog = dialog.clone();
            let entry = entry.clone();
            entry.connect_changed(move |entry| {
                dialog.set_response_enabled("rename", !entry.text().trim().is_empty());
            });
        }
        let window = self.clone();
        let id = choice.id;
        let refresh = refresh.clone();
        dialog.connect_response(None, move |_, response| {
            if response == "rename" {
                let name = entry.text().to_string();
                let renamed = {
                    let mut vault = window.imp().vault.borrow_mut();
                    match vault.as_mut() {
                        Some(vault) => vault.rename_choice(id, &name).is_ok(),
                        None => false,
                    }
                };
                if renamed {
                    window.refresh();
                    refresh();
                }
            }
        });
        dialog.present(Some(self));
    }

    fn present_add_choice(&self, kind: ChoiceKind, refresh: Rc<dyn Fn()>) {
        let dialog = adw::AlertDialog::new(Some(&gettextrs::gettext("Add an item")), None);
        dialog.add_response("cancel", &gettextrs::gettext("Cancel"));
        dialog.add_response("add", &gettextrs::gettext("Add"));
        dialog.set_response_appearance("add", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("add"));
        dialog.set_close_response("cancel");
        let entry = gtk::Entry::new();
        entry.set_placeholder_text(Some(&gettextrs::gettext("Name")));
        entry.set_margin_top(6);
        dialog.set_extra_child(Some(&entry));
        dialog.set_response_enabled("add", false);
        {
            let dialog = dialog.clone();
            let entry = entry.clone();
            entry.connect_changed(move |entry| {
                dialog.set_response_enabled("add", !entry.text().trim().is_empty());
            });
        }
        let window = self.clone();
        dialog.connect_response(None, move |_, response| {
            if response == "add" {
                let name = entry.text().trim().to_string();
                if name.is_empty() {
                    return;
                }
                // Suggest an icon from the name (product spec §9).
                let icon = icon_search::IconIndex::shared()
                    .suggest(&name)
                    .unwrap_or_else(|| "question".to_owned());
                let created = {
                    let mut vault = window.imp().vault.borrow_mut();
                    match vault.as_mut() {
                        Some(vault) => vault.create_choice(kind, &name, &icon).ok(),
                        None => None,
                    }
                };
                if created.is_some() {
                    window.refresh();
                    refresh();
                }
            }
        });
        dialog.present(Some(self));
    }

    /// The icon picker popover: search + a grid, matching per product spec §9.
    fn present_icon_picker(
        &self,
        anchor: &gtk::Button,
        current: &str,
        used: HashSet<String>,
        on_pick: Rc<dyn Fn(String)>,
    ) {
        let popover = gtk::Popover::new();
        let store = gio::ListStore::new::<gtk::StringObject>();
        let selection = gtk::SingleSelection::new(Some(store.clone()));
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(|_, item| {
            if let Some(item) = item.downcast_ref::<gtk::ListItem>() {
                let button = gtk::Button::new();
                button.add_css_class("flat");
                button.set_size_request(40, 40);
                item.set_child(Some(&button));
            }
        });
        factory.connect_bind(|_, item| {
            let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
                return;
            };
            let Some(data) = item.item().and_downcast::<gtk::StringObject>() else {
                return;
            };
            let Some(button) = item.child().and_downcast::<gtk::Button>() else {
                return;
            };
            let name = data.string().to_string();
            button.set_child(Some(&gtk::Image::from_icon_name(&icons::resolve(&name))));
            button.set_tooltip_text(Some(&name));
        });
        let grid = gtk::GridView::new(Some(selection), Some(factory));
        grid.set_single_click_activate(true);
        grid.set_max_columns(8);
        grid.set_min_columns(4);
        grid.set_valign(gtk::Align::Start);
        {
            let on_pick = on_pick.clone();
            let popover = popover.clone();
            grid.connect_activate(move |grid, position| {
                if let Some(object) = grid.model().and_then(|model| model.item(position))
                    && let Some(data) = object.downcast_ref::<gtk::StringObject>()
                {
                    on_pick(data.string().to_string());
                    popover.popdown();
                }
            });
        }

        let search = gtk::SearchEntry::new();
        search.set_placeholder_text(Some(&gettextrs::gettext("Search icons")));
        let mut labels = vec![gettextrs::gettext("All icons")];
        labels.extend(icon_search::IconIndex::shared().categories());
        let category_list =
            gtk::StringList::new(&labels.iter().map(String::as_str).collect::<Vec<_>>());
        let dropdown = gtk::DropDown::new(Some(category_list), gtk::Expression::NONE);
        // Plain labels: category names contain `&`, which markup would reject.
        let dropdown_factory = gtk::SignalListItemFactory::new();
        dropdown_factory.connect_setup(|_, item| {
            if let Some(item) = item.downcast_ref::<gtk::ListItem>() {
                item.set_child(Some(&gtk::Label::new(None)));
            }
        });
        dropdown_factory.connect_bind(|_, item| {
            let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
                return;
            };
            let Some(data) = item.item().and_downcast::<gtk::StringObject>() else {
                return;
            };
            let Some(label) = item.child().and_downcast::<gtk::Label>() else {
                return;
            };
            label.set_label(&data.string());
        });
        dropdown.set_factory(Some(&dropdown_factory));
        dropdown.set_selected(0);

        let update: Rc<dyn Fn()> = {
            let store = store.clone();
            let search = search.clone();
            let dropdown = dropdown.clone();
            let index = icon_search::IconIndex::shared();
            let used = used.clone();
            Rc::new(move || {
                store.remove_all();
                let query = search.text();
                let icons = if !query.trim().is_empty() {
                    index.search(&query, &used)
                } else {
                    let selected = dropdown.selected();
                    if selected == 0 {
                        index.search("", &used)
                    } else {
                        let category = dropdown
                            .selected_item()
                            .and_downcast::<gtk::StringObject>()
                            .map(|object| object.string().to_string())
                            .unwrap_or_default();
                        index
                            .by_category()
                            .into_iter()
                            .find(|(name, _)| *name == category)
                            .map(|(_, icons)| icons)
                            .unwrap_or_default()
                    }
                };
                for name in icons {
                    store.append(&gtk::StringObject::new(&name));
                }
            })
        };
        {
            let update = update.clone();
            search.connect_search_changed(move |_| update());
        }
        {
            let update = update.clone();
            dropdown.connect_selected_notify(move |_| update());
        }
        update();

        let heading = gtk::Label::new(Some(&gettextrs::gettext("Matching icons")));
        heading.set_xalign(0.0);
        heading.add_css_class("heading");
        if !current.is_empty() {
            search.set_text(current);
        }
        let scrolled = gtk::ScrolledWindow::builder()
            .child(&grid)
            .min_content_width(360)
            .min_content_height(340)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build();
        let content = gtk::Box::new(gtk::Orientation::Vertical, 6);
        content.set_size_request(380, -1);
        content.append(&search);
        content.append(&heading);
        content.append(&dropdown);
        content.append(&scrolled);
        popover.set_child(Some(&content));
        popover.set_parent(anchor);
        // Track it, so lock can pop it even if its anchor is later rebuilt and
        // the widget tree no longer reaches it.
        self.track_popover(&popover);
        popover.popup();
        search.grab_focus();
    }

    /// Keeps a reference to an open popover for the window's lifetime, dropping
    /// any that have already closed.
    fn track_popover(&self, popover: &gtk::Popover) {
        let mut open = self.imp().open_popovers.borrow_mut();
        open.retain(|existing| existing.parent().is_some() && existing.is_visible());
        open.push(popover.clone());
    }

    fn setup_breakpoints(&self) {
        if let Ok(condition) = adw::BreakpointCondition::parse("max-width: 1400sp") {
            let breakpoint = adw::Breakpoint::new(condition);
            breakpoint.add_setter(&*self.imp().main_split, "collapsed", Some(&true.to_value()));
            adw::prelude::AdwApplicationWindowExt::add_breakpoint(self, breakpoint);
        }
        if let Ok(condition) = adw::BreakpointCondition::parse("max-width: 1100sp") {
            let breakpoint = adw::Breakpoint::new(condition);
            breakpoint.add_setter(
                &*self.imp().details_split,
                "collapsed",
                Some(&true.to_value()),
            );
            adw::prelude::AdwApplicationWindowExt::add_breakpoint(self, breakpoint);
        }
        if let Ok(condition) = adw::BreakpointCondition::parse("max-width: 760sp") {
            let breakpoint = adw::Breakpoint::new(condition);
            breakpoint.add_setter(
                &*self.imp().navigation_split,
                "collapsed",
                Some(&true.to_value()),
            );
            adw::prelude::AdwApplicationWindowExt::add_breakpoint(self, breakpoint);
        }
    }

    /// Wraps the selection (or the cursor) with Markdown marks.
    fn wrap_selection(&self, before: &str, after: &str) {
        let Some(buffer) = self.imp().editor_buffer.borrow().as_ref().cloned() else {
            return;
        };
        if let Some((mut start, mut end)) = buffer.selection_bounds() {
            let text = buffer.text(&start, &end, false);
            buffer.delete(&mut start, &mut end);
            buffer.insert(&mut start, &format!("{before}{text}{after}"));
        } else {
            buffer.insert_at_cursor(&format!("{before}{after}"));
        }
    }

    /// Prefixes the line at the cursor with a Markdown marker.
    fn prefix_line(&self, prefix: &str) {
        let Some(buffer) = self.imp().editor_buffer.borrow().as_ref().cloned() else {
            return;
        };
        let mut iter = buffer.iter_at_offset(buffer.cursor_position());
        iter.set_line_offset(0);
        buffer.insert(&mut iter, prefix);
    }

    fn set_filter(&self, filter: Filter) {
        let name = match filter {
            Filter::All => "all",
            Filter::Journal => "journal",
            Filter::Dreams => "dreams",
            Filter::Notes => "notes",
        };
        self.imp().filter_group.set_active_name(Some(name));
        self.imp().collection_kind.set(None);
        self.imp().collection_subject.replace(None);
        self.apply_view_mode();
        self.refresh();
    }

    fn active_filter(&self) -> Filter {
        match self.imp().filter_group.active_name().as_deref() {
            Some("journal") => Filter::Journal,
            Some("dreams") => Filter::Dreams,
            Some("notes") => Filter::Notes,
            _ => Filter::All,
        }
    }

    /// Rebuilds the entry list and search index from the vault.
    pub fn refresh(&self) {
        self.apply_view_mode();
        let borrow = self.imp().vault.borrow();
        let Some(vault) = borrow.as_ref() else {
            return;
        };

        // A collection with no subject chosen is its list of items.
        if let Some(kind) = self.imp().collection_kind.get()
            && self.imp().collection_subject.borrow().is_none()
        {
            drop(borrow);
            self.populate_collection(kind);
            return;
        }
        let subject_filter = *self.imp().collection_subject.borrow();

        let query = self.imp().search_entry.text().to_string();
        let notes_mode =
            self.active_filter() == Filter::Notes && self.imp().collection_kind.get().is_none();
        let mut index = timeline::SearchIndex::new();
        let mut rows: Vec<(Entry, Vec<String>)> = Vec::new();
        for entry in vault.entries(false).unwrap_or_default() {
            let subjects = vault.entry_subjects(entry.id, None).unwrap_or_default();
            let choices = vault.entry_choices(entry.id).unwrap_or_default();
            let colors = vault.entry_colors(entry.id).unwrap_or_default();

            let tags: Vec<String> = subjects
                .iter()
                .filter(|subject| subject.kind == SubjectKind::Tag)
                .map(|subject| subject.name.clone())
                .collect();

            let mut parts = vec![entry.content.clone()];
            parts.extend(subjects.iter().map(|subject| subject.name.clone()));
            parts.extend(
                choices
                    .iter()
                    .map(|choice| choice.display_label().to_owned()),
            );
            parts.extend(colors.iter().map(|color| color.as_str().to_owned()));
            index.add(entry.id, parts);

            if let Some(subject) = subject_filter {
                if subjects.iter().any(|s| s.id == subject) {
                    rows.push((entry, tags));
                }
            } else if notes_mode {
                if entry.entry_type == EntryType::Note {
                    rows.push((entry, tags));
                }
            } else if timeline::passes(&entry, self.active_filter()) {
                rows.push((entry, tags));
            }
        }

        if !query.is_empty() {
            let matching: HashSet<Uuid> = index.search(&query).into_iter().collect();
            rows.retain(|(entry, _)| matching.contains(&entry.id));
        }

        let expanded: HashSet<Uuid> = vault
            .expanded_note_ids()
            .unwrap_or_default()
            .into_iter()
            .collect();
        let day_end = vault.day_end().unwrap_or((3, 0));
        let today = jiff::Zoned::now().date();
        self.imp().search_index.replace(Some(index));
        drop(borrow);

        if notes_mode {
            let notes = timeline::note_list(rows, &expanded, self.imp().note_filter.get());
            self.populate_note_list(notes, today);
        } else {
            let groups = timeline::group(rows, day_end, today);
            self.populate_list(groups);
        }

        if self.imp().selected.borrow().is_none()
            && let Some(id) = self.imp().row_ids.borrow().iter().flatten().next().copied()
        {
            self.select_entry(id);
        }
    }

    /// Builds the quick-notes card list (ui-spec §3.7).
    fn populate_note_list(&self, notes: Vec<timeline::EntrySummary>, today: jiff::civil::Date) {
        let list = &self.imp().entry_list;
        while let Some(child) = list.first_child() {
            list.remove(&child);
        }
        let mut row_ids: Vec<Option<Uuid>> = Vec::new();
        for summary in notes {
            let card = gtk::Box::new(gtk::Orientation::Vertical, 4);
            card.add_css_class("card");
            card.set_margin_top(6);
            card.set_margin_bottom(6);
            card.set_margin_start(6);
            card.set_margin_end(6);
            let body = if summary.excerpt.is_empty() {
                summary.title.clone()
            } else {
                format!("{}\n{}", summary.title, summary.excerpt)
            };
            let text = gtk::Label::new(Some(&body));
            text.set_wrap(true);
            text.set_lines(3);
            text.set_ellipsize(gtk::pango::EllipsizeMode::End);
            text.set_xalign(0.0);
            card.append(&text);

            let footer = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            let date = gtk::Label::new(Some(&timeline::note_date_label(&summary.dated_at, today)));
            date.add_css_class("dim-label");
            date.add_css_class("caption");
            footer.append(&date);
            for tag in &summary.tags {
                let tag_label = gtk::Label::new(Some(&format!("#{tag}")));
                tag_label.add_css_class("dim-label");
                tag_label.add_css_class("caption");
                footer.append(&tag_label);
            }
            card.append(&footer);

            let row = gtk::ListBoxRow::new();
            row.set_child(Some(&card));
            list.append(&row);
            row_ids.push(Some(summary.id));
        }
        self.imp().row_ids.replace(row_ids);
    }

    fn populate_list(&self, groups: Vec<timeline::DayGroup>) {
        let list = &self.imp().entry_list;
        while let Some(child) = list.first_child() {
            list.remove(&child);
        }
        let mut row_ids: Vec<Option<Uuid>> = Vec::new();
        for group in groups {
            let header = gtk::ListBoxRow::new();
            header.set_selectable(false);
            header.set_activatable(false);
            let label = gtk::Label::new(Some(&group.label));
            label.set_xalign(0.0);
            label.add_css_class("heading");
            label.set_margin_top(8);
            label.set_margin_bottom(4);
            label.set_margin_start(12);
            header.set_child(Some(&label));
            list.append(&header);
            row_ids.push(None);

            for summary in group.entries {
                let action = adw::ActionRow::builder()
                    .title(&summary.title)
                    .subtitle(&summary.excerpt)
                    .activatable(true)
                    .build();
                let image = gtk::Image::from_icon_name(type_icon(summary.entry_type));
                image.set_valign(gtk::Align::Start);
                action.add_prefix(&image);
                let time = summary.dated_at.strftime("%H:%M").to_string();
                let suffix = gtk::Label::new(Some(&time));
                suffix.add_css_class("dim-label");
                suffix.add_css_class("caption");
                suffix.set_valign(gtk::Align::Start);
                action.add_suffix(&suffix);
                if let Some(tag) = summary.tags.first() {
                    let tag_label = gtk::Label::new(Some(&format!("#{tag}")));
                    tag_label.add_css_class("dim-label");
                    tag_label.add_css_class("caption");
                    tag_label.set_valign(gtk::Align::Start);
                    action.add_suffix(&tag_label);
                }
                let row = gtk::ListBoxRow::new();
                row.set_child(Some(&action));
                list.append(&row);
                row_ids.push(Some(summary.id));
            }
        }
        self.imp().row_ids.replace(row_ids);
    }

    fn select_entry(&self, id: Uuid) {
        if *self.imp().selected.borrow() == Some(id) {
            return;
        }
        self.depart_current();

        let entry = {
            let vault = self.imp().vault.borrow();
            let Some(vault) = vault.as_ref() else {
                return;
            };
            let Ok(entry) = vault.entry(id) else {
                return;
            };
            entry
        };

        self.imp().loading.set(true);
        if let Some(buffer) = self.imp().editor_buffer.borrow().as_ref() {
            buffer.set_text(&entry.content);
        }
        self.imp()
            .date_calendar
            .select_day(&gdatetime_for(entry.dated_at.date()));
        self.imp()
            .time_entry
            .set_text(&entry.dated_at.strftime("%H:%M").to_string());
        self.imp().loading.set(false);
        self.imp().dirty.set(false);

        self.imp()
            .type_badge
            .set_label(&entry_type_label(entry.entry_type));
        self.imp()
            .date_button
            .set_label(&entry.dated_at.strftime("%d %b %Y").to_string());
        self.imp()
            .save_label
            .set_label(&gettextrs::gettext("Saved"));
        self.imp()
            .current_dated
            .replace(Some(entry.dated_at.clone()));
        self.imp().current_created.replace(Some(entry.created_at));
        self.imp().current_type.replace(Some(entry.entry_type));
        self.imp().selected.replace(Some(id));
        self.imp().editor_stack.set_visible_child_name("entry");
        self.imp().navigation_split.set_show_content(true);
        self.update_banner();
        self.set_footer();
        self.update_entry_header(&entry);
        self.rebuild_entry_menu(entry.entry_type);
        self.refresh_details();
    }

    fn setup_date_callbacks(&self) {
        let window = self.clone();
        self.imp()
            .date_calendar
            .connect_day_selected(move |_| window.apply_date());
        let window = self.clone();
        self.imp()
            .time_entry
            .connect_activate(move |_| window.apply_date());
        let window = self.clone();
        self.imp().today_button.connect_clicked(move |_| {
            window.set_calendar_to(jiff::Zoned::now().date());
        });
        let window = self.clone();
        self.imp().yesterday_button.connect_clicked(move |_| {
            if let Ok(yesterday) = jiff::Zoned::now().date().yesterday() {
                window.set_calendar_to(yesterday);
            }
        });
        self.imp()
            .backdated_banner
            .set_button_label(Some(&gettextrs::gettext("Use creation date")));
        let window = self.clone();
        self.imp()
            .backdated_banner
            .connect_button_clicked(move |_| window.use_creation_date());
    }

    fn set_calendar_to(&self, date: jiff::civil::Date) {
        self.imp().date_calendar.select_day(&gdatetime_for(date));
    }

    /// Applies the calendar and time fields to the selected entry, keeping the
    /// stored offset (product spec §4.3, §5.5).
    fn apply_date(&self) {
        if self.imp().loading.get() {
            return;
        }
        let Some(id) = *self.imp().selected.borrow() else {
            return;
        };
        let Some(current) = self.imp().current_dated.borrow().clone() else {
            return;
        };

        let selected_date = self.imp().date_calendar.date();
        let Ok(date) = jiff::civil::Date::new(
            selected_date.year() as i16,
            selected_date.month() as i8,
            selected_date.day_of_month() as i8,
        ) else {
            return;
        };
        let time =
            timeline::parse_time(&self.imp().time_entry.text()).unwrap_or_else(|| current.time());
        let Ok(new_dated) = timeline::shift_datetime(&current, date, time) else {
            return;
        };

        let saved = {
            let mut vault = self.imp().vault.borrow_mut();
            match vault.as_mut() {
                Some(vault) => vault.set_entry_dated_at(id, &new_dated).is_ok(),
                None => false,
            }
        };
        if !saved {
            return;
        }
        self.imp().current_dated.replace(Some(new_dated.clone()));
        self.imp()
            .date_button
            .set_label(&new_dated.strftime("%d %b %Y").to_string());
        self.imp()
            .time_entry
            .set_text(&new_dated.strftime("%H:%M").to_string());
        self.update_banner();
        self.set_footer();
        self.refresh();
        self.refresh_details();
    }

    fn use_creation_date(&self) {
        let Some(created) = *self.imp().current_created.borrow() else {
            return;
        };
        let created = created.to_zoned(jiff::tz::TimeZone::UTC);
        self.set_calendar_to(created.date());
    }

    fn update_banner(&self) {
        // Quick notes have no date info bar (ui-spec §3.5).
        if *self.imp().current_type.borrow() == Some(EntryType::Note) {
            self.imp().backdated_banner.set_revealed(false);
            return;
        }
        let (Some(dated), Some(created)) = (
            self.imp().current_dated.borrow().clone(),
            *self.imp().current_created.borrow(),
        ) else {
            self.imp().backdated_banner.set_revealed(false);
            return;
        };
        let created_date = created.to_zoned(jiff::tz::TimeZone::UTC).date();
        let dated_date = dated.date();
        if dated_date == created_date {
            self.imp().backdated_banner.set_revealed(false);
            return;
        }

        let days = timeline::day_difference(created_date, dated_date);
        let direction = if days >= 0 {
            gettextrs::gettext("later")
        } else {
            gettextrs::gettext("earlier")
        };
        let title = format!(
            "{} {}, {} {} {}",
            gettextrs::gettext("Entry date set to"),
            dated_date.strftime("%a %d %b"),
            gettextrs::gettext("written"),
            days.abs(),
            direction,
        );
        self.imp().backdated_banner.set_title(&title);
        self.imp().backdated_banner.set_revealed(true);
    }

    fn buffer_text(&self) -> Option<String> {
        self.imp().editor_buffer.borrow().as_ref().map(|buffer| {
            buffer
                .text(&buffer.start_iter(), &buffer.end_iter(), false)
                .to_string()
        })
    }

    fn set_footer(&self) {
        let words = self
            .buffer_text()
            .map(|text| quaderno_vault::text::word_count(&text))
            .unwrap_or(0);
        let words_label = gettextrs::ngettext("{n} word", "{n} words", words as u32)
            .replace("{n}", &words.to_string());
        let date = self
            .imp()
            .current_dated
            .borrow()
            .as_ref()
            .map(|dated| dated.strftime("%d %b %Y, %H:%M").to_string())
            .unwrap_or_default();
        self.imp()
            .footer_label
            .set_label(&format!("{words_label} · {date}"));
    }

    /// Schedules an autosave one second after the last change (product spec §4.2).
    fn schedule_save(&self) {
        if self.imp().loading.get() || self.imp().selected.borrow().is_none() {
            return;
        }
        self.imp().dirty.set(true);
        self.imp()
            .save_label
            .set_label(&gettextrs::gettext("Saving…"));
        if let Some(source) = self.imp().save_source.borrow_mut().take() {
            source.remove();
        }
        let window = self.clone();
        let source = glib::timeout_add_seconds_local(1, move || {
            window.imp().save_source.borrow_mut().take();
            window.flush_save();
            glib::ControlFlow::Break
        });
        self.imp().save_source.replace(Some(source));
    }

    /// Persists the editor's text to the selected entry now.
    ///
    /// A single-column `UPDATE` runs on the main thread; the heavier vault work
    /// (open, create, backup) is already off-thread from earlier milestones.
    fn flush_save(&self) {
        if let Some(source) = self.imp().save_source.borrow_mut().take() {
            source.remove();
        }
        if !self.imp().dirty.get() {
            return;
        }
        let Some(id) = *self.imp().selected.borrow() else {
            return;
        };
        let Some(text) = self.buffer_text() else {
            return;
        };

        let saved = {
            let mut vault = self.imp().vault.borrow_mut();
            match vault.as_mut() {
                Some(vault) => vault.set_entry_content(id, &text).is_ok(),
                None => false,
            }
        };
        if saved {
            self.imp().dirty.set(false);
            self.imp()
                .save_label
                .set_label(&gettextrs::gettext("Saved"));
            self.set_footer();
            self.refresh();
            self.refresh_details();
        }
    }

    /// Called before leaving the current entry: save it, or discard it when it
    /// is an empty draft with no information (product spec §4.2).
    fn depart_current(&self) {
        let Some(id) = *self.imp().selected.borrow() else {
            return;
        };
        let empty = self
            .buffer_text()
            .map(|text| timeline::is_empty_draft(&text))
            .unwrap_or(true);
        if empty {
            let discarded = {
                let mut vault = self.imp().vault.borrow_mut();
                match vault.as_mut() {
                    Some(vault) => vault.discard_entry(id).is_ok(),
                    None => false,
                }
            };
            if discarded {
                self.imp().dirty.set(false);
                return;
            }
        }
        self.flush_save();
    }

    /// Soft-deletes the selected entry and offers an undo toast (product spec §4.4).
    fn delete_selected(&self) {
        let Some(id) = *self.imp().selected.borrow() else {
            return;
        };
        let deleted = {
            let mut vault = self.imp().vault.borrow_mut();
            match vault.as_mut() {
                Some(vault) => vault.soft_delete_entry(id).is_ok(),
                None => false,
            }
        };
        if !deleted {
            return;
        }
        if let Some(source) = self.imp().save_source.borrow_mut().take() {
            source.remove();
        }
        self.imp().dirty.set(false);
        self.imp().selected.replace(None);
        self.imp().editor_stack.set_visible_child_name("empty");
        self.refresh();

        let toast = adw::Toast::new(&gettextrs::gettext("Entry deleted"));
        toast.set_button_label(Some(&gettextrs::gettext("Undo")));
        toast.set_timeout(10);
        let window = self.clone();
        toast.connect_button_clicked(move |_| window.restore_entry(id));
        self.imp().toast_overlay.add_toast(toast);
    }

    fn restore_entry(&self, id: Uuid) {
        let restored = {
            let mut vault = self.imp().vault.borrow_mut();
            match vault.as_mut() {
                Some(vault) => vault.restore_entry(id).is_ok(),
                None => false,
            }
        };
        if restored {
            self.refresh();
            self.select_entry(id);
        }
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

    /// Locks the vault: saves pending edits, closes it and shows the unlock screen.
    pub fn lock(&self) {
        self.depart_current();
        if self.imp().vault.borrow_mut().take().is_none() {
            return; // nothing open
        }
        self.stop_idle_timer();
        self.imp().search_index.replace(None);
        self.imp().search_entry.set_text("");
        self.imp().search_button.set_active(false);
        // Dismiss any dialogs or popovers still open, so nothing sits on top of
        // the unlock page.
        self.close_transient_windows();
        self.show_unlock();
    }

    /// Closes every dialog and popover attached to this window.
    fn close_transient_windows(&self) {
        // `AdwDialog` (Preferences, AlertDialogs, the font chooser) is a widget
        // overlaid on the window, not a toplevel, so it must be closed through
        // the window's dialog list.
        for dialog in adw::prelude::AdwApplicationWindowExt::dialogs(self)
            .iter::<adw::Dialog>()
            .flatten()
        {
            dialog.force_close();
        }

        let me = self.upcast_ref::<gtk::Window>();
        for widget in gtk::Window::list_toplevels() {
            if let Some(other) = widget.downcast_ref::<gtk::Window>()
                && let Some(transient) = other.transient_for()
                && transient == *me
            {
                other.set_visible(false);
                other.destroy();
            }
        }
        // The details panel's pickers, whose anchor widgets are rebuilt, so a
        // widget-tree walk can miss them.
        if let Some(panel) = self.imp().panel.borrow().as_ref() {
            for choice in &panel.choices {
                choice.popover.popdown();
            }
            for token in &panel.tokens {
                token.popover.popdown();
            }
        }
        // Popovers tracked while open (the icon picker and any other anchored to
        // a widget that may have been rebuilt since).
        for popover in self.imp().open_popovers.borrow().iter() {
            popover.popdown();
        }
        self.imp().open_popovers.borrow_mut().clear();
        // Every other popover in the window's widget tree.
        self.close_popovers(self.upcast_ref::<gtk::Widget>());
    }

    fn close_popovers(&self, widget: &gtk::Widget) {
        if let Some(popover) = widget.downcast_ref::<gtk::Popover>() {
            popover.popdown();
        }
        let mut child = widget.first_child();
        while let Some(current) = child {
            child = current.next_sibling();
            self.close_popovers(&current);
        }
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

/// The title of a collection (ui-spec §3.3).
fn collection_title(kind: SubjectKind) -> String {
    match kind {
        SubjectKind::Person => gettextrs::gettext("People"),
        SubjectKind::Place => gettextrs::gettext("Locations"),
        SubjectKind::Thing => gettextrs::gettext("Things"),
        SubjectKind::Tag => gettextrs::gettext("Tags"),
    }
}

/// The subtitle of a choice row: how many entries use it.
fn choice_subtitle(usage: i64) -> String {
    gettextrs::ngettext("Used in {n} entry", "Used in {n} entries", usage as u32)
        .replace("{n}", &usage.to_string())
}

/// A small, hidden-by-default validation message label.
fn message_label() -> gtk::Label {
    let label = gtk::Label::new(None);
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.add_css_class("caption");
    label.add_css_class("dim-label");
    label.set_margin_start(12);
    label.set_margin_end(12);
    label.set_margin_bottom(6);
    label.set_visible(false);
    label
}

/// Sets a validation message; `error` styles it as an error, otherwise dim.
fn set_message(label: &gtk::Label, text: &str, error: bool) {
    if text.is_empty() {
        label.set_visible(false);
        return;
    }
    label.set_label(text);
    label.set_visible(true);
    if error {
        label.add_css_class("error");
        label.remove_css_class("dim-label");
    } else {
        label.remove_css_class("error");
        label.add_css_class("dim-label");
    }
}

/// The label of an auto-lock duration.
fn lock_label(minutes: i32) -> String {
    match minutes {
        0 => gettextrs::gettext("Never"),
        1 => gettextrs::gettext("1 minute"),
        60 => gettextrs::gettext("1 hour"),
        minutes => gettextrs::ngettext("{n} minute", "{n} minutes", minutes as u32)
            .replace("{n}", &minutes.to_string()),
    }
}

/// A path with the home directory shortened to `~`.
fn pretty_path(path: &std::path::Path) -> String {
    let text = path.to_string_lossy();
    match glib::home_dir().to_str() {
        Some(home) => match text.strip_prefix(home) {
            Some(rest) => format!("~{rest}"),
            None => text.into_owned(),
        },
        None => text.into_owned(),
    }
}

/// The label of the spell-checking menu button: the stored languages' names,
/// or "System default".
fn update_spell_label(
    button: &gtk::MenuButton,
    stored: &str,
    checks: &[(String, gtk::CheckButton)],
) {
    if stored.is_empty() {
        button.set_label(&gettextrs::gettext("System default"));
        return;
    }
    let names: Vec<String> = checks
        .iter()
        .filter(|(code, _)| stored.split(',').any(|part| part == code))
        .filter_map(|(_, check)| check.label().map(|label| label.to_string()))
        .collect();
    if names.is_empty() {
        button.set_label(&gettextrs::gettext("System default"));
    } else {
        button.set_label(&names.join(", "));
    }
}

fn type_icon(entry_type: EntryType) -> &'static str {
    match entry_type {
        EntryType::Journal => "x-office-document-symbolic",
        EntryType::Dream => "weather-few-clouds-night-symbolic",
        EntryType::Note => "text-editor-symbolic",
    }
}

/// The leading icon of a token field (ui-spec §3.6).
fn subject_icon(kind: SubjectKind) -> &'static str {
    match kind {
        SubjectKind::Person => "avatar-default-symbolic",
        SubjectKind::Place => "mark-location-symbolic",
        SubjectKind::Thing => "package-x-generic-symbolic",
        SubjectKind::Tag => "tag-symbolic",
    }
}

/// Clears a token field after a subject was linked or created, closes its
/// popover and puts focus back in the field so several names can be typed in a
/// row. Linking rebuilds the field's chips (which briefly unfocuses the entry),
/// hence the explicit re-grab.
fn clear_token_entry(entry: &gtk::Text, popover: &gtk::Popover) {
    entry.set_text("");
    popover.popdown();
    entry.grab_focus();
}

/// Moves the highlight in a picker list by `delta`, clamped, and scrolls it
/// into view.
fn move_selection(selection: &gtk::SingleSelection, list: &gtk::ListView, delta: i32) {
    let current = match selection.selected() {
        gtk::INVALID_LIST_POSITION => None,
        index => Some(index as usize),
    };
    let Some(next) = details::step_index(current, selection.n_items() as usize, delta) else {
        return;
    };
    let next = next as u32;
    selection.set_selected(next);
    list.scroll_to(next, gtk::ListScrollFlags::NONE, None);
}

/// The CSS class that paints a colour swatch (Appendix C).
fn color_css_class(color: Color) -> &'static str {
    match color {
        Color::Red => "color-red",
        Color::Orange => "color-orange",
        Color::Yellow => "color-yellow",
        Color::Green => "color-green",
        Color::Teal => "color-teal",
        Color::Blue => "color-blue",
        Color::Purple => "color-purple",
        Color::Pink => "color-pink",
        Color::Brown => "color-brown",
        Color::Gray => "color-gray",
        Color::Black => "color-black",
        Color::White => "color-white",
    }
}

/// The stored rating value for a scale.
fn rating_value(entry: &Entry, rating: Rating) -> Option<u8> {
    match rating {
        Rating::Mood => entry.mood,
        Rating::Energy => entry.energy,
        Rating::CognitiveLoad => entry.cognitive_load,
        Rating::Sleep => entry.sleep,
    }
}

/// The created/updated caption of the details panel (vault-spec §5.6).
fn dates_caption(created: jiff::Timestamp, updated: Option<jiff::Timestamp>) -> String {
    let created = local_datetime(created);
    match updated {
        Some(updated) => format!(
            "{} {} · {} {}",
            gettextrs::gettext("Created"),
            created,
            gettextrs::gettext("Updated"),
            local_datetime(updated)
        ),
        None => format!("{} {}", gettextrs::gettext("Created"), created),
    }
}

/// Formats a technical timestamp in the system time zone.
fn local_datetime(timestamp: jiff::Timestamp) -> String {
    timestamp
        .to_zoned(jiff::tz::TimeZone::system())
        .strftime("%d %b %Y, %H:%M")
        .to_string()
}

fn entry_type_label(entry_type: EntryType) -> String {
    match entry_type {
        EntryType::Journal => gettextrs::gettext("Journal page"),
        EntryType::Dream => gettextrs::gettext("Dream"),
        EntryType::Note => gettextrs::gettext("Quick note"),
    }
}

/// A `GDateTime` at midnight UTC for the given civil date, for `GtkCalendar`.
fn gdatetime_for(date: jiff::civil::Date) -> glib::DateTime {
    glib::DateTime::new(
        &glib::TimeZone::utc(),
        date.year() as i32,
        date.month() as i32,
        date.day() as i32,
        0,
        0,
        0.0,
    )
    // The components come from a valid `jiff` date, so this cannot fail.
    .expect("a valid civil date")
}
