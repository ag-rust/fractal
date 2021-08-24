use crate::components::Avatar;
use adw::subclass::prelude::BinImpl;
use gtk::{self, glib, prelude::*, subclass::prelude::*, CompositeTemplate};

mod imp {
    use super::*;
    use glib::subclass::InitializingObject;
    use once_cell::sync::Lazy;
    use std::cell::RefCell;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/org/gnome/FractalNext/user-entry-row.ui")]
    pub struct UserEntryRow {
        #[template_child]
        pub avatar_component: TemplateChild<Avatar>,
        #[template_child]
        pub display_name: TemplateChild<gtk::Label>,
        #[template_child]
        pub user_id: TemplateChild<gtk::Label>,
        pub session_page: RefCell<Option<gtk::StackPage>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for UserEntryRow {
        const NAME: &'static str = "UserEntryRow";
        type Type = super::UserEntryRow;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            Avatar::static_type();
            Self::bind_template(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for UserEntryRow {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpec::new_object(
                    "session-page",
                    "Session StackPage",
                    "The stack page of the session that this entry represents",
                    gtk::StackPage::static_type(),
                    glib::ParamFlags::READWRITE,
                )]
            });

            PROPERTIES.as_ref()
        }

        fn set_property(
            &self,
            _obj: &Self::Type,
            _id: usize,
            value: &glib::Value,
            pspec: &glib::ParamSpec,
        ) {
            match pspec.name() {
                "session-page" => {
                    let session_page = value.get().unwrap();
                    self.session_page.replace(Some(session_page));
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, _obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "session-page" => self.session_page.borrow().to_value(),
                _ => unimplemented!(),
            }
        }
    }

    impl WidgetImpl for UserEntryRow {}
    impl BinImpl for UserEntryRow {}
}

glib::wrapper! {
    pub struct UserEntryRow(ObjectSubclass<imp::UserEntryRow>)
        @extends gtk::Widget, adw::Bin, @implements gtk::Accessible;
}

impl UserEntryRow {
    pub fn new(session_page: &gtk::StackPage) -> Self {
        glib::Object::new(&[("session-page", session_page)]).expect("Failed to create UserEntryRow")
    }
}