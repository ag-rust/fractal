use adw::subclass::prelude::*;
use gtk::{glib, glib::clone, prelude::*, subclass::prelude::*, CompositeTemplate};

use crate::session::user::UserExt;
use crate::session::verification::{IdentityVerification, VerificationMode};
use gettextrs::gettext;
mod imp {
    use super::*;
    use glib::subclass::InitializingObject;
    use glib::SignalHandlerId;
    use std::cell::RefCell;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/org/gnome/FractalNext/content-verification-info-bar.ui")]
    pub struct VerificationInfoBar {
        #[template_child]
        pub revealer: TemplateChild<gtk::Revealer>,
        #[template_child]
        pub label: TemplateChild<gtk::Label>,
        #[template_child]
        pub button: TemplateChild<gtk::Button>,
        pub request: RefCell<Option<IdentityVerification>>,
        pub mode_handler: RefCell<Option<SignalHandlerId>>,
        pub user_handler: RefCell<Option<SignalHandlerId>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for VerificationInfoBar {
        const NAME: &'static str = "ContentVerificationInfoBar";
        type Type = super::VerificationInfoBar;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("infobar");
            Self::bind_template(klass);

            klass.set_accessible_role(gtk::AccessibleRole::Group);

            klass.install_action("verification.accept", None, move |widget, _, _| {
                let request = widget.request().unwrap();
                request.accept();
                request.session().select_item(Some(request.upcast()));
            });

            klass.install_action("verification.decline", None, move |widget, _, _| {
                widget.request().unwrap().cancel();
            });
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for VerificationInfoBar {
        fn properties() -> &'static [glib::ParamSpec] {
            use once_cell::sync::Lazy;
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpec::new_object(
                    "request",
                    "Request",
                    "The verification request this InfoBar is showing",
                    IdentityVerification::static_type(),
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
                "request" => obj.set_request(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "request" => obj.request().to_value(),
                _ => unimplemented!(),
            }
        }
    }
    impl WidgetImpl for VerificationInfoBar {}
    impl BinImpl for VerificationInfoBar {}
}

glib::wrapper! {
    pub struct VerificationInfoBar(ObjectSubclass<imp::VerificationInfoBar>)
        @extends gtk::Widget, adw::Bin, @implements gtk::Accessible;
}

impl VerificationInfoBar {
    pub fn new(label: String) -> Self {
        glib::Object::new(&[("label", &label)]).expect("Failed to create VerificationInfoBar")
    }

    pub fn request(&self) -> Option<IdentityVerification> {
        let priv_ = imp::VerificationInfoBar::from_instance(self);
        priv_.request.borrow().clone()
    }

    pub fn set_request(&self, request: Option<IdentityVerification>) {
        let priv_ = imp::VerificationInfoBar::from_instance(self);

        if let Some(old_request) = &*priv_.request.borrow() {
            if Some(old_request) == request.as_ref() {
                return;
            }

            if let Some(handler) = priv_.mode_handler.take() {
                old_request.disconnect(handler);
            }

            if let Some(handler) = priv_.user_handler.take() {
                old_request.user().disconnect(handler);
            }
        }

        if let Some(ref request) = request {
            let handler = request.connect_notify_local(
                Some("mode"),
                clone!(@weak self as obj => move |_, _| {
                    obj.update_view();
                }),
            );

            priv_.mode_handler.replace(Some(handler));

            let handler = request.user().connect_notify_local(
                Some("display-name"),
                clone!(@weak self as obj => move |_, _| {
                    obj.update_view();
                }),
            );

            priv_.user_handler.replace(Some(handler));
        }

        priv_.request.replace(request);

        self.update_view();
        self.notify("request");
    }

    pub fn update_view(&self) {
        let priv_ = imp::VerificationInfoBar::from_instance(self);
        let visible = if let Some(request) = self.request() {
            if request.is_finished() {
                false
            } else if matches!(request.mode(), VerificationMode::Requested) {
                // Translators: The value is the display name of the user who wants to be verified
                priv_.label.set_markup(&gettext!(
                    "<b>{}</b> wants to be verified",
                    request.user().display_name()
                ));
                priv_.button.set_label(&gettext("Verify"));
                true
            } else {
                priv_.label.set_label(&gettext("Verification in progess"));
                priv_.button.set_label(&gettext("Continue"));
                true
            }
        } else {
            false
        };

        priv_.revealer.set_reveal_child(visible);
    }
}