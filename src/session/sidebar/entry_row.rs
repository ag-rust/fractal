use adw::subclass::prelude::BinImpl;
use gtk::{self, glib, prelude::*, subclass::prelude::*, CompositeTemplate};

use crate::session::sidebar::Entry;

mod imp {
    use std::cell::RefCell;

    use glib::subclass::InitializingObject;

    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/org/gnome/Fractal/sidebar-entry-row.ui")]
    pub struct EntryRow {
        pub entry: RefCell<Option<Entry>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for EntryRow {
        const NAME: &'static str = "SidebarEntryRow";
        type Type = super::EntryRow;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for EntryRow {
        fn properties() -> &'static [glib::ParamSpec] {
            use once_cell::sync::Lazy;
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpecObject::new(
                    "entry",
                    "Entry",
                    "The entry of this row",
                    Entry::static_type(),
                    glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                )]
            });

            PROPERTIES.as_ref()
        }

        fn set_property(
            &self,
            obj: &Self::Type,
            _id: usize,
            value: &glib::Value,
            pspec: &glib::ParamSpec,
        ) {
            match pspec.name() {
                "entry" => obj.set_entry(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "entry" => obj.entry().to_value(),
                _ => unimplemented!(),
            }
        }
    }

    impl WidgetImpl for EntryRow {}
    impl BinImpl for EntryRow {}
}

glib::wrapper! {
    pub struct EntryRow(ObjectSubclass<imp::EntryRow>)
        @extends gtk::Widget, adw::Bin, @implements gtk::Accessible;
}

impl EntryRow {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create EntryRow")
    }

    pub fn entry(&self) -> Option<Entry> {
        self.imp().entry.borrow().clone()
    }

    pub fn set_entry(&self, entry: Option<Entry>) {
        if self.entry() == entry {
            return;
        }

        self.imp().entry.replace(entry);
        self.notify("entry");
    }
}

impl Default for EntryRow {
    fn default() -> Self {
        Self::new()
    }
}
