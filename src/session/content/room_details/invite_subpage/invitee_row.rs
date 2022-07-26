use adw::subclass::prelude::BinImpl;
use gtk::{glib, prelude::*, subclass::prelude::*, CompositeTemplate};

use super::Invitee;

mod imp {
    use std::cell::RefCell;

    use glib::subclass::InitializingObject;
    use once_cell::sync::Lazy;

    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/org/gnome/Fractal/content-invitee-row.ui")]
    pub struct InviteeRow {
        pub user: RefCell<Option<Invitee>>,
        pub binding: RefCell<Option<glib::Binding>>,
        #[template_child]
        pub check_button: TemplateChild<gtk::CheckButton>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for InviteeRow {
        const NAME: &'static str = "ContentInviteInviteeRow";
        type Type = super::InviteeRow;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for InviteeRow {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpecObject::new(
                    "user",
                    "User",
                    "The user this row is showing",
                    Invitee::static_type(),
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
                "user" => {
                    obj.set_user(value.get().unwrap());
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "user" => obj.user().to_value(),
                _ => unimplemented!(),
            }
        }
    }
    impl WidgetImpl for InviteeRow {}
    impl BinImpl for InviteeRow {}
}

glib::wrapper! {
    pub struct InviteeRow(ObjectSubclass<imp::InviteeRow>)
        @extends gtk::Widget, adw::Bin, @implements gtk::Accessible;
}

impl InviteeRow {
    pub fn new(user: &Invitee) -> Self {
        glib::Object::new(&[("user", user)]).expect("Failed to create InviteeRow")
    }

    pub fn user(&self) -> Option<Invitee> {
        self.imp().user.borrow().clone()
    }

    pub fn set_user(&self, user: Option<Invitee>) {
        let priv_ = self.imp();

        if self.user() == user {
            return;
        }

        if let Some(binding) = priv_.binding.take() {
            binding.unbind();
        }

        if let Some(ref user) = user {
            // We can't use `gtk::Expression` because we need a bidirectional binding
            let binding = user
                .bind_property("invited", &*priv_.check_button, "active")
                .flags(glib::BindingFlags::BIDIRECTIONAL | glib::BindingFlags::SYNC_CREATE)
                .build();

            priv_.binding.replace(Some(binding));
        }

        priv_.user.replace(user);
        self.notify("user");
    }
}
