// SPDX-License-Identifier: GPL-3.0-or-later

//! The list view used by the metadata pickers (ui-spec §3.6): a small data
//! object plus a `GtkListView` of `Adw.ActionRow`s.

use gtk::gio;
use gtk::glib;
use gtk::glib::subclass::prelude::*;
use gtk::prelude::*;

mod imp {
    use super::*;
    use std::cell::{Cell, RefCell};

    #[derive(Default)]
    pub struct PickerRow {
        pub id: RefCell<String>,
        pub title: RefCell<String>,
        pub subtitle: RefCell<String>,
        pub icon: RefCell<String>,
        pub create: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PickerRow {
        const NAME: &'static str = "QuadernoPickerRow";
        type Type = super::PickerRow;
    }

    impl ObjectImpl for PickerRow {
        fn properties() -> &'static [glib::ParamSpec] {
            use std::sync::OnceLock;
            static PROPERTIES: OnceLock<Vec<glib::ParamSpec>> = OnceLock::new();
            PROPERTIES.get_or_init(|| {
                vec![
                    glib::ParamSpecString::builder("id").build(),
                    glib::ParamSpecString::builder("title").build(),
                    glib::ParamSpecString::builder("subtitle").build(),
                    glib::ParamSpecString::builder("icon").build(),
                    glib::ParamSpecBoolean::builder("create").build(),
                ]
            })
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                "id" => *self.id.borrow_mut() = value.get().unwrap_or_default(),
                "title" => *self.title.borrow_mut() = value.get().unwrap_or_default(),
                "subtitle" => *self.subtitle.borrow_mut() = value.get().unwrap_or_default(),
                "icon" => *self.icon.borrow_mut() = value.get().unwrap_or_default(),
                "create" => self.create.set(value.get().unwrap_or_default()),
                // Only the properties above are registered.
                _ => unreachable!(),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "id" => self.id.borrow().to_value(),
                "title" => self.title.borrow().to_value(),
                "subtitle" => self.subtitle.borrow().to_value(),
                "icon" => self.icon.borrow().to_value(),
                "create" => self.create.get().to_value(),
                // Only the properties above are registered.
                _ => unreachable!(),
            }
        }
    }
}

glib::wrapper! {
    /// One row of a picker: a choice, a subject suggestion, or a "Create …" row.
    pub struct PickerRow(ObjectSubclass<imp::PickerRow>);
}

impl PickerRow {
    /// Builds a row. `create` marks the "Create …" row.
    pub fn new(id: &str, title: &str, subtitle: &str, icon: &str, create: bool) -> Self {
        glib::Object::builder()
            .property("id", id)
            .property("title", title)
            .property("subtitle", subtitle)
            .property("icon", icon)
            .property("create", create)
            .build()
    }

    /// The stored id (empty for the create row).
    pub fn id(&self) -> String {
        self.property("id")
    }

    /// The main label.
    pub fn title(&self) -> String {
        self.property("title")
    }

    /// The dim second line, if any.
    pub fn subtitle(&self) -> String {
        self.property("subtitle")
    }

    /// The Phosphor icon name (empty for the create row).
    pub fn icon(&self) -> String {
        self.property("icon")
    }

    /// Whether this is the create row.
    pub fn is_create(&self) -> bool {
        self.property("create")
    }
}

/// Builds the list view used by a picker, returning it with its selection model
/// and store so the caller can repopulate it.
pub fn list_view() -> (gtk::ListView, gtk::SingleSelection, gio::ListStore) {
    let store = gio::ListStore::new::<PickerRow>();
    let selection = gtk::SingleSelection::new(Some(store.clone()));
    selection.set_autoselect(true);
    selection.set_can_unselect(true);

    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(|_, item| {
        if let Some(item) = item.downcast_ref::<gtk::ListItem>() {
            item.set_child(Some(&gtk::Box::new(gtk::Orientation::Horizontal, 12)));
        }
    });
    factory.connect_bind(|_, item| {
        let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        let Some(data) = item.item().and_downcast::<PickerRow>() else {
            return;
        };
        let Some(content) = item.child().and_downcast::<gtk::Box>() else {
            return;
        };
        while let Some(child) = content.first_child() {
            content.remove(&child);
        }

        let icon_name = if data.is_create() {
            "list-add-symbolic".to_owned()
        } else {
            data.icon()
        };
        content.append(&gtk::Image::from_icon_name(&icon_name));

        let labels = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let title = gtk::Label::new(Some(&data.title()));
        title.set_xalign(0.0);
        labels.append(&title);
        let subtitle = data.subtitle();
        if !subtitle.is_empty() {
            let subtitle = gtk::Label::new(Some(&subtitle));
            subtitle.set_xalign(0.0);
            subtitle.add_css_class("dim-label");
            subtitle.add_css_class("caption");
            labels.append(&subtitle);
        }
        content.append(&labels);
    });

    let list = gtk::ListView::new(Some(selection.clone()), Some(factory));
    list.add_css_class("navigation-sidebar");
    (list, selection, store)
}
