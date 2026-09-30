// SPDX-License-Identifier: GPL-3.0-or-later

//! The main window: a stack of welcome, unlock, error and main pages, and the
//! logic that moves between them.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gio, glib};
use quaderno_vault::{Access, EntryType, SubjectKind, Vault, VaultError};
use sourceview5::prelude::*;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::keyring::Keyring;
use crate::state::{CreateForm, CreateValidity, Phase, Throttle};
use crate::timeline::{self, Filter};

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
        #[template_child]
        pub main_split: TemplateChild<adw::OverlaySplitView>,
        #[template_child]
        pub navigation_split: TemplateChild<adw::NavigationSplitView>,
        #[template_child]
        pub sidebar_toggle: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub new_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub search_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub search_bar: TemplateChild<gtk::SearchBar>,
        #[template_child]
        pub search_entry: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub filter_group: TemplateChild<adw::ToggleGroup>,
        #[template_child]
        pub sections_list: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub entry_list: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub editor_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub type_badge: TemplateChild<gtk::Label>,
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
        pub backdated_banner: TemplateChild<adw::Banner>,
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
        // Non-template state.
        pub editor_buffer: RefCell<Option<sourceview5::Buffer>>,
        pub spell_adapter: RefCell<Option<libspelling::TextBufferAdapter>>,
        pub selected: RefCell<Option<uuid::Uuid>>,
        pub current_dated: RefCell<Option<jiff::Zoned>>,
        pub current_created: RefCell<Option<jiff::Timestamp>>,
        pub dirty: Cell<bool>,
        pub loading: Cell<bool>,
        pub save_source: RefCell<Option<glib::SourceId>>,
        pub row_ids: RefCell<Vec<Option<uuid::Uuid>>>,
        pub section_filters: RefCell<Vec<(u32, Filter)>>,
        pub search_index: RefCell<Option<crate::timeline::SearchIndex>>,
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
                new_button: Default::default(),
                search_button: Default::default(),
                search_bar: Default::default(),
                search_entry: Default::default(),
                filter_group: Default::default(),
                sections_list: Default::default(),
                entry_list: Default::default(),
                editor_stack: Default::default(),
                type_badge: Default::default(),
                date_button: Default::default(),
                date_calendar: Default::default(),
                time_entry: Default::default(),
                today_button: Default::default(),
                yesterday_button: Default::default(),
                save_label: Default::default(),
                backdated_banner: Default::default(),
                editor_scroll: Default::default(),
                footer_label: Default::default(),
                heading_button: Default::default(),
                bold_button: Default::default(),
                italic_button: Default::default(),
                list_button: Default::default(),
                quote_button: Default::default(),
                link_button: Default::default(),
                editor_buffer: RefCell::new(None),
                spell_adapter: RefCell::new(None),
                selected: RefCell::new(None),
                current_dated: RefCell::new(None),
                current_created: RefCell::new(None),
                dirty: Cell::new(false),
                loading: Cell::new(false),
                save_source: RefCell::new(None),
                row_ids: RefCell::new(Vec::new()),
                section_filters: RefCell::new(Vec::new()),
                search_index: RefCell::new(None),
                entry_menu: Default::default(),
                toast_overlay: Default::default(),
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
        menu.append(Some(&gettextrs::gettext("Quit")), Some("app.quit"));
        self.imp().menu_button.set_menu_model(Some(&menu));

        let entry_menu = gio::Menu::new();
        entry_menu.append(
            Some(&gettextrs::gettext("Delete")),
            Some("win.delete-entry"),
        );
        self.imp().entry_menu.set_menu_model(Some(&entry_menu));

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
        self.show_phase(Phase::Main);
        self.start_idle_timer();
        self.refresh();
    }

    // ---- main page (sections, entry list, editor) ------------------------

    fn setup_editor(&self) {
        let buffer = sourceview5::Buffer::new(None);
        if let Some(language) = sourceview5::LanguageManager::default().language("markdown") {
            buffer.set_language(Some(&language));
        }

        let manager = sourceview5::StyleSchemeManager::default();
        manager.append_search_path("resource:///io/github/stickgrinder/Quaderno/styles");
        if let Some(scheme) = manager.scheme("quaderno") {
            buffer.set_style_scheme(Some(&scheme));
        }

        let view = sourceview5::View::with_buffer(&buffer);
        view.set_wrap_mode(gtk::WrapMode::WordChar);
        view.set_top_margin(12);
        view.set_bottom_margin(12);
        view.set_left_margin(12);
        view.set_right_margin(12);
        let adapter = crate::spell_checking::attach(&buffer);

        let window = self.clone();
        buffer.connect_changed(move |_| window.schedule_save());
        let focus = gtk::EventControllerFocus::new();
        let window = self.clone();
        focus.connect_leave(move |_| window.flush_save());
        view.add_controller(focus);

        self.imp().editor_scroll.set_child(Some(&view));
        self.imp().editor_buffer.replace(Some(buffer));
        self.imp().spell_adapter.replace(Some(adapter));
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

        for (icon, label) in [
            ("avatar-default-symbolic", gettextrs::gettext("People")),
            ("mark-location-symbolic", gettextrs::gettext("Locations")),
            ("package-x-generic-symbolic", gettextrs::gettext("Things")),
            ("tag-symbolic", gettextrs::gettext("Tags")),
        ] {
            let row = self.section_row(icon, &label);
            row.set_sensitive(false);
            self.imp().sections_list.append(&row);
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
                }
            });
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
        self.imp().new_button.connect_clicked(move |_| {
            window.new_entry();
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
            if let Some(id) = id {
                window.select_entry(id);
            }
        });

        let window = self.clone();
        self.connect_close_request(move |_| {
            window.depart_current();
            glib::Propagation::Proceed
        });

        self.setup_date_callbacks();
        self.setup_formatting();
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

    fn setup_breakpoints(&self) {
        if let Ok(condition) = adw::BreakpointCondition::parse("max-width: 1400sp") {
            let breakpoint = adw::Breakpoint::new(condition);
            breakpoint.add_setter(&*self.imp().main_split, "collapsed", Some(&true.to_value()));
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
        let borrow = self.imp().vault.borrow();
        let Some(vault) = borrow.as_ref() else {
            return;
        };

        let query = self.imp().search_entry.text().to_string();
        let mut index = timeline::SearchIndex::new();
        let mut rows = Vec::new();
        for entry in vault.entries(false).unwrap_or_default() {
            let subjects = vault.entry_subjects(entry.id, None).unwrap_or_default();
            let choices = vault.entry_choices(entry.id).unwrap_or_default();
            let colors = vault.entry_colors(entry.id).unwrap_or_default();

            let first_tag = subjects
                .iter()
                .find(|subject| subject.kind == SubjectKind::Tag)
                .map(|subject| subject.name.clone());

            let mut parts = vec![entry.content.clone()];
            parts.extend(subjects.iter().map(|subject| subject.name.clone()));
            parts.extend(
                choices
                    .iter()
                    .map(|choice| choice.display_label().to_owned()),
            );
            parts.extend(colors.iter().map(|color| color.as_str().to_owned()));
            index.add(entry.id, parts);

            if timeline::passes(&entry, self.active_filter()) {
                rows.push((entry, first_tag));
            }
        }

        if !query.is_empty() {
            let matching: std::collections::HashSet<Uuid> =
                index.search(&query).into_iter().collect();
            rows.retain(|(entry, _)| matching.contains(&entry.id));
        }

        let day_end = vault.day_end().unwrap_or((3, 0));
        let today = jiff::Zoned::now().date();
        let groups = timeline::group(rows, day_end, today);
        self.imp().search_index.replace(Some(index));
        drop(borrow);

        self.populate_list(groups);

        if self.imp().selected.borrow().is_none()
            && let Some(id) = self.imp().row_ids.borrow().iter().flatten().next().copied()
        {
            self.select_entry(id);
        }
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
                if let Some(tag) = &summary.first_tag {
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

    fn new_entry(&self) {
        let created = {
            let mut vault = self.imp().vault.borrow_mut();
            let Some(vault) = vault.as_mut() else {
                return;
            };
            let now = jiff::Zoned::now();
            vault.create_entry(EntryType::Journal, "", &now).ok()
        };
        if let Some(id) = created {
            self.imp().selected.replace(None);
            self.refresh();
            self.select_entry(id);
        }
    }

    fn select_entry(&self, id: Uuid) {
        if *self.imp().selected.borrow() == Some(id) {
            return;
        }
        self.depart_current();

        let (content, entry_type, dated_at, created_at) = {
            let vault = self.imp().vault.borrow();
            let Some(vault) = vault.as_ref() else {
                return;
            };
            let Ok(entry) = vault.entry(id) else {
                return;
            };
            (
                entry.content,
                entry.entry_type,
                entry.dated_at,
                entry.created_at,
            )
        };

        self.imp().loading.set(true);
        if let Some(buffer) = self.imp().editor_buffer.borrow().as_ref() {
            buffer.set_text(&content);
        }
        self.imp()
            .date_calendar
            .select_day(&gdatetime_for(dated_at.date()));
        self.imp()
            .time_entry
            .set_text(&dated_at.strftime("%H:%M").to_string());
        self.imp().loading.set(false);
        self.imp().dirty.set(false);

        self.imp()
            .type_badge
            .set_label(&entry_type_label(entry_type));
        self.imp()
            .date_button
            .set_label(&dated_at.strftime("%d %b %Y").to_string());
        self.imp()
            .save_label
            .set_label(&gettextrs::gettext("Saved"));
        self.imp().current_dated.replace(Some(dated_at));
        self.imp().current_created.replace(Some(created_at));
        self.imp().selected.replace(Some(id));
        self.imp().editor_stack.set_visible_child_name("entry");
        self.imp().navigation_split.set_show_content(true);
        self.update_banner();
        self.set_footer();
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
        let time = parse_time(&self.imp().time_entry.text()).unwrap_or_else(|| current.time());
        let Ok(new_dated) = current.with().date(date).time(time).build() else {
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
    }

    fn use_creation_date(&self) {
        let Some(created) = *self.imp().current_created.borrow() else {
            return;
        };
        let created = created.to_zoned(jiff::tz::TimeZone::UTC);
        self.set_calendar_to(created.date());
    }

    fn update_banner(&self) {
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

        let days = day_difference(created_date, dated_date);
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
            .map(|text| text.trim().is_empty())
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

fn type_icon(entry_type: EntryType) -> &'static str {
    match entry_type {
        EntryType::Journal => "x-office-document-symbolic",
        EntryType::Dream => "weather-few-clouds-night-symbolic",
        EntryType::Note => "text-editor-symbolic",
    }
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

fn parse_time(text: &str) -> Option<jiff::civil::Time> {
    let (hours, minutes) = text.split_once(':')?;
    jiff::civil::Time::new(
        hours.trim().parse().ok()?,
        minutes.trim().parse().ok()?,
        0,
        0,
    )
    .ok()
}

fn day_difference(from: jiff::civil::Date, to: jiff::civil::Date) -> i64 {
    let start = from.at(0, 0, 0, 0).to_zoned(jiff::tz::TimeZone::UTC);
    let end = to.at(0, 0, 0, 0).to_zoned(jiff::tz::TimeZone::UTC);
    match (start, end) {
        (Ok(start), Ok(end)) => {
            (end.timestamp().as_second() - start.timestamp().as_second()) / 86_400
        }
        _ => 0,
    }
}
