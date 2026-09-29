// SPDX-License-Identifier: GPL-3.0-or-later

use gtk::gio;
use gtk::glib;

mod imp {
    use super::*;
    use adw::subclass::prelude::*;

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/stickgrinder/Quaderno/quaderno-window.ui")]
    pub struct QuadernoWindow;

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

    impl ObjectImpl for QuadernoWindow {}
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
    pub fn new(app: &adw::Application) -> Self {
        glib::Object::builder().property("application", app).build()
    }
}
