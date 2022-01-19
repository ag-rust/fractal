use crate::Error;
use adw::subclass::prelude::*;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib, glib::clone, CompositeTemplate};

mod imp {
    use super::*;
    use glib::{signal::SignalHandlerId, subclass::InitializingObject};
    use std::cell::{Cell, RefCell};

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/org/gnome/FractalNext/in-app-notification.ui")]
    pub struct InAppNotification {
        pub error_list: RefCell<Option<gio::ListStore>>,
        pub handler: RefCell<Option<SignalHandlerId>>,
        #[template_child]
        pub revealer: TemplateChild<gtk::Revealer>,
        #[template_child]
        pub box_: TemplateChild<gtk::Box>,
        pub current_widget: RefCell<Option<gtk::Widget>>,
        pub shows_error: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for InAppNotification {
        const NAME: &'static str = "InAppNotification";
        type Type = super::InAppNotification;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);

            klass.install_action("in-app-notification.close", None, move |widget, _, _| {
                widget.dismiss()
            });
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for InAppNotification {
        fn properties() -> &'static [glib::ParamSpec] {
            use once_cell::sync::Lazy;
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpecObject::new(
                    "error-list",
                    "Error List",
                    "The list of errors to display",
                    gio::ListStore::static_type(),
                    glib::ParamFlags::READWRITE,
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
                "error-list" => obj.set_error_list(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "error-list" => obj.error_list().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);
            self.revealer
                .connect_child_revealed_notify(clone!(@weak obj => move |revealer| {
                    let priv_ = imp::InAppNotification::from_instance(&obj);
                    revealer.set_visible(priv_.shows_error.get());
                }));
        }

        fn dispose(&self, _obj: &Self::Type) {
            if let Some(id) = self.handler.take() {
                self.error_list.borrow().as_ref().unwrap().disconnect(id);
            }
        }
    }

    impl WidgetImpl for InAppNotification {}

    impl BinImpl for InAppNotification {}
}

glib::wrapper! {
    pub struct InAppNotification(ObjectSubclass<imp::InAppNotification>)
        @extends gtk::Widget, adw::Bin, @implements gtk::Accessible;
}

impl InAppNotification {
    pub fn new(error_list: &gio::ListStore) -> Self {
        glib::Object::new(&[("error-list", &error_list)])
            .expect("Failed to create InAppNotification")
    }

    pub fn set_error_list(&self, error_list: Option<gio::ListStore>) {
        let priv_ = imp::InAppNotification::from_instance(self);
        if self.error_list() == error_list {
            return;
        }

        if let Some(id) = priv_.handler.take() {
            priv_.error_list.borrow().as_ref().unwrap().disconnect(id);
        }

        if let Some(ref error_list) = error_list {
            let handler = error_list.connect_items_changed(
                clone!(@weak self as obj => move |_, position, removed, added| {
                        let priv_ = imp::InAppNotification::from_instance(&obj);
                        // If the first error is removed we need to display the next error
                        if position == 0 && removed > 0 {
                                obj.next();
                        }

                        if added > 0  && !priv_.shows_error.get() {
                                obj.next();
                        }

                }),
            );
            priv_.handler.replace(Some(handler));
        }
        priv_.error_list.replace(error_list);

        self.next();
        self.notify("error-list");
    }

    pub fn error_list(&self) -> Option<gio::ListStore> {
        let priv_ = imp::InAppNotification::from_instance(self);
        priv_.error_list.borrow().to_owned()
    }

    /// Show the next message in the `error-list`
    fn next(&self) {
        let priv_ = imp::InAppNotification::from_instance(self);

        let shows_error = if let Some(widget) = priv_
            .error_list
            .borrow()
            .as_ref()
            .and_then(|error_list| error_list.item(0))
            .and_then(|obj| obj.downcast::<Error>().ok())
            .and_then(|error| error.widget())
        {
            if let Some(current_widget) = priv_.current_widget.take() {
                priv_.box_.remove(&current_widget);
            }
            priv_.box_.prepend(&widget);
            priv_.current_widget.replace(Some(widget));
            true
        } else {
            false
        };

        priv_.shows_error.set(shows_error);
        if shows_error {
            priv_.revealer.show();
        }
        priv_.revealer.set_reveal_child(shows_error);
    }

    fn dismiss(&self) {
        let priv_ = imp::InAppNotification::from_instance(self);
        if let Some(error_list) = &*priv_.error_list.borrow() {
            error_list.remove(0);
        }
    }
}
