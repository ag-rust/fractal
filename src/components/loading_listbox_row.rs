use glib::subclass::Signal;
use gtk::{glib, glib::clone, prelude::*, subclass::prelude::*, CompositeTemplate};

mod imp {
    use std::cell::Cell;

    use glib::subclass::InitializingObject;
    use once_cell::sync::Lazy;

    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/org/gnome/Fractal/components-loading-listbox-row.ui")]
    pub struct LoadingListBoxRow {
        #[template_child]
        pub spinner: TemplateChild<gtk::Spinner>,
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub error: TemplateChild<gtk::Box>,
        #[template_child]
        pub error_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub retry_button: TemplateChild<gtk::Button>,
        pub is_error: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LoadingListBoxRow {
        const NAME: &'static str = "ComponentsLoadingListBoxRow";
        type Type = super::LoadingListBoxRow;
        type ParentType = gtk::ListBoxRow;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for LoadingListBoxRow {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecBoolean::new(
                        "loading",
                        "Loading",
                        "Whether to show the loading spinner",
                        true,
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpecString::new(
                        "error",
                        "Error",
                        "The error message to show",
                        None,
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
                "loading" => {
                    obj.set_loading(value.get().unwrap());
                }
                "error" => {
                    obj.set_error(value.get().unwrap());
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "loading" => obj.is_loading().to_value(),
                "error" => obj.error().to_value(),
                _ => unimplemented!(),
            }
        }

        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![Signal::builder("retry", &[], <()>::static_type().into()).build()]
            });
            SIGNALS.as_ref()
        }

        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            self.retry_button
                .connect_clicked(clone!(@weak obj => move |_| {
                    obj.emit_by_name::<()>("retry", &[]);
                }));
        }
    }
    impl WidgetImpl for LoadingListBoxRow {}
    impl ListBoxRowImpl for LoadingListBoxRow {}
}

glib::wrapper! {
    /// This is a `ListBoxRow` containing a loading spinner.
    ///
    /// It's also possible to set an error once the loading fails including a retry button.
    pub struct LoadingListBoxRow(ObjectSubclass<imp::LoadingListBoxRow>)
        @extends gtk::Widget, gtk::ListBoxRow, @implements gtk::Accessible;
}

impl Default for LoadingListBoxRow {
    fn default() -> Self {
        Self::new()
    }
}

impl LoadingListBoxRow {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create LoadingListBoxRow")
    }

    pub fn is_loading(&self) -> bool {
        !self.imp().is_error.get()
    }

    pub fn set_loading(&self, loading: bool) {
        let priv_ = self.imp();

        if self.is_loading() == loading {
            return;
        }

        priv_.stack.set_visible_child(&*priv_.spinner);
        priv_.is_error.set(false);

        self.notify("loading");
    }

    pub fn error(&self) -> Option<glib::GString> {
        let message = self.imp().error_label.text();
        if message.is_empty() {
            None
        } else {
            Some(message)
        }
    }

    pub fn set_error(&self, message: Option<&str>) {
        let priv_ = self.imp();

        if let Some(message) = message {
            priv_.is_error.set(true);
            priv_.error_label.set_text(message);
            priv_.stack.set_visible_child(&*priv_.error);
        } else {
            priv_.is_error.set(false);
            priv_.stack.set_visible_child(&*priv_.spinner);
        }
        self.notify("error");
    }

    pub fn connect_retry<F: Fn(&Self) + 'static>(&self, f: F) -> glib::SignalHandlerId {
        self.connect_local("retry", true, move |values| {
            let obj = values[0].get::<Self>().unwrap();
            f(&obj);
            None
        })
    }
}
