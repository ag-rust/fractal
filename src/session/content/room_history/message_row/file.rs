use adw::subclass::prelude::*;
use gtk::{glib, prelude::*, subclass::prelude::*, CompositeTemplate};

mod imp {
    use std::cell::{Cell, RefCell};

    use glib::subclass::InitializingObject;
    use once_cell::sync::Lazy;

    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/org/gnome/Fractal/content-message-file.ui")]
    pub struct MessageFile {
        /// The filename of the file
        pub filename: RefCell<Option<String>>,
        /// Whether this file should be displayed in a compact format.
        pub compact: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MessageFile {
        const NAME: &'static str = "ContentMessageFile";
        type Type = super::MessageFile;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for MessageFile {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecString::new(
                        "filename",
                        "Filename",
                        "The filename of the file",
                        None,
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpecBoolean::new(
                        "compact",
                        "Compact",
                        "Whether this file should be displayed in a compact format",
                        false,
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                ]
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
                "filename" => obj.set_filename(value.get().unwrap()),
                "compact" => obj.set_compact(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "filename" => obj.filename().to_value(),
                "compact" => obj.compact().to_value(),
                _ => unimplemented!(),
            }
        }
    }

    impl WidgetImpl for MessageFile {}

    impl BinImpl for MessageFile {}
}

glib::wrapper! {
    /// A widget displaying an interface to download or open the content of a file message.
    pub struct MessageFile(ObjectSubclass<imp::MessageFile>)
        @extends gtk::Widget, adw::Bin, @implements gtk::Accessible;
}

impl MessageFile {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create MessageFile")
    }

    pub fn set_filename(&self, filename: Option<String>) {
        let priv_ = self.imp();

        let name = filename.filter(|name| !name.is_empty());

        if name.as_ref() == priv_.filename.borrow().as_ref() {
            return;
        }

        priv_.filename.replace(name);
        self.notify("filename");
    }

    pub fn filename(&self) -> Option<String> {
        self.imp().filename.borrow().to_owned()
    }

    pub fn set_compact(&self, compact: bool) {
        if self.compact() == compact {
            return;
        }

        self.imp().compact.set(compact);
        self.notify("compact");
    }

    pub fn compact(&self) -> bool {
        self.imp().compact.get()
    }
}

impl Default for MessageFile {
    fn default() -> Self {
        Self::new()
    }
}
