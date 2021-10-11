use adw::subclass::prelude::*;
use gtk::gdk;
use gtk::gio::prelude::*;
use gtk::glib::clone;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{glib, CompositeTemplate};
use std::cell::Cell;
use std::future::Future;

use crate::session::Session;
use crate::session::UserExt;
use crate::RUNTIME;

use matrix_sdk::{
    ruma::api::{
        client::{
            error::ErrorBody,
            r0::uiaa::{
                AuthData as MatrixAuthData,
                FallbackAcknowledgement as MatrixFallbackAcknowledgement,
                Password as MatrixPassword, UiaaInfo, UiaaResponse, UserIdentifier,
            },
        },
        error::{FromHttpResponseError, ServerError},
        OutgoingRequest,
    },
    ruma::assign,
    HttpError,
    HttpError::UiaaError,
    HttpResult,
};

use std::fmt::Debug;

pub struct Password {
    pub user_id: String,
    pub password: String,
    pub session: Option<String>,
}

pub struct FallbackAcknowledgement {
    pub session: String,
}

// FIXME: we can't move the ruma AuthData between threads
// because it's not owned data and doesn't live long enough.
// Therefore we have our own AuthData.
pub enum AuthData {
    Password(Password),
    FallbackAcknowledgement(FallbackAcknowledgement),
}

impl AuthData {
    pub fn as_matrix_auth_data(&self) -> MatrixAuthData {
        match self {
            AuthData::Password(Password {
                user_id,
                password,
                session,
            }) => MatrixAuthData::Password(assign!(MatrixPassword::new(
                                UserIdentifier::MatrixId(user_id),
                                password,
                            ), { session: session.as_deref() })),
            AuthData::FallbackAcknowledgement(FallbackAcknowledgement { session }) => {
                MatrixAuthData::FallbackAcknowledgement(MatrixFallbackAcknowledgement::new(session))
            }
        }
    }
}

mod imp {
    use super::*;
    use glib::object::WeakRef;
    use glib::subclass::{InitializingObject, Signal};
    use glib::SignalHandlerId;
    use once_cell::sync::{Lazy, OnceCell};
    use std::cell::RefCell;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/org/gnome/FractalNext/components-auth-dialog.ui")]
    pub struct AuthDialog {
        pub session: OnceCell<WeakRef<Session>>,
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub password: TemplateChild<gtk::PasswordEntry>,
        #[template_child]
        pub error: TemplateChild<gtk::Label>,

        #[template_child]
        pub button_cancel: TemplateChild<gtk::Button>,
        #[template_child]
        pub button_ok: TemplateChild<gtk::Button>,

        #[template_child]
        pub open_browser_btn: TemplateChild<gtk::Button>,
        pub open_browser_btn_handler: RefCell<Option<SignalHandlerId>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AuthDialog {
        const NAME: &'static str = "ComponentsAuthDialog";
        type Type = super::AuthDialog;
        type ParentType = adw::Window;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            let response = glib::Variant::from_tuple(&[false.to_variant()]);
            klass.add_binding_signal(
                gdk::keys::constants::Escape,
                gdk::ModifierType::empty(),
                "response",
                Some(&response),
            );
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for AuthDialog {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpec::new_object(
                    "session",
                    "Session",
                    "The session",
                    Session::static_type(),
                    glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY,
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
                "session" => self
                    .session
                    .set(value.get::<Session>().unwrap().downgrade())
                    .unwrap(),
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "session" => obj.session().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            self.button_cancel
                .connect_clicked(clone!(@weak obj => move |_| {
                    obj.emit_by_name("response", &[&false]).unwrap();
                }));

            self.button_ok
                .connect_clicked(clone!(@weak obj => move |_| {
                    obj.emit_by_name("response", &[&true]).unwrap();
                }));

            obj.connect_close_request(
                clone!(@weak obj => @default-return gtk::Inhibit(false), move |_| {
                    obj.emit_by_name("response", &[&false]).unwrap();
                    gtk::Inhibit(false)
                }),
            );
        }

        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![Signal::builder(
                    "response",
                    &[bool::static_type().into()],
                    <()>::static_type().into(),
                )
                .action()
                .build()]
            });
            SIGNALS.as_ref()
        }
    }
    impl WidgetImpl for AuthDialog {}
    impl WindowImpl for AuthDialog {}
    impl AdwWindowImpl for AuthDialog {}
}

glib::wrapper! {
    /// Button showing a spinner, revealing its label once loaded.
    pub struct AuthDialog(ObjectSubclass<imp::AuthDialog>)
        @extends gtk::Widget, adw::Window, gtk::Dialog, gtk::Window, @implements gtk::Accessible;
}

impl AuthDialog {
    pub fn new(transient_for: Option<&impl IsA<gtk::Window>>, session: &Session) -> Self {
        glib::Object::new(&[("transient-for", &transient_for), ("session", session)])
            .expect("Failed to create AuthDialog")
    }

    pub fn session(&self) -> Session {
        let priv_ = imp::AuthDialog::from_instance(self);
        priv_.session.get().unwrap().upgrade().unwrap()
    }

    pub async fn authenticate<
        Request: Send + 'static,
        F1: Future<Output = HttpResult<Request::IncomingResponse>> + Send + 'static,
        FN: Fn(Option<AuthData>) -> F1 + Send + Sync + 'static + Clone,
    >(
        &self,
        callback: FN,
    ) -> Option<HttpResult<Request::IncomingResponse>>
    where
        Request: OutgoingRequest + Debug,
        Request::IncomingResponse: Send,
        HttpError: From<FromHttpResponseError<Request::EndpointError>>,
    {
        let priv_ = imp::AuthDialog::from_instance(self);
        let mut auth_data = None;

        loop {
            let callback_clone = callback.clone();
            let (sender, receiver) = futures::channel::oneshot::channel();
            RUNTIME.spawn(async move { sender.send(callback_clone(auth_data).await) });
            let response = receiver.await.unwrap();

            let uiaa_info: UiaaInfo = match response {
                Ok(result) => return Some(Ok(result)),
                Err(UiaaError(FromHttpResponseError::Http(ServerError::Known(
                    UiaaResponse::AuthResponse(uiaa_info),
                )))) => uiaa_info,
                Err(error) => return Some(Err(error)),
            };

            self.show_auth_error(&uiaa_info.auth_error);

            // Find the first flow that matches the completed flow
            let flow = uiaa_info
                .flows
                .iter()
                .find(|flow| flow.stages.starts_with(&uiaa_info.completed))?;

            match flow.stages[uiaa_info.completed.len()].as_str() {
                "m.login.password" => {
                    priv_.stack.set_visible_child_name("m.login.password");
                    if self.show_and_wait_for_response().await {
                        let user_id = self.session().user().unwrap().user_id().to_string();
                        let password = priv_.password.text().to_string();
                        let session = uiaa_info.session;

                        auth_data = Some(AuthData::Password(Password {
                            user_id,
                            password,
                            session,
                        }));

                        continue;
                    }
                }
                // TODO implement other authentication types
                // See: https://gitlab.gnome.org/GNOME/fractal/-/issues/835
                _ => {
                    if let Some(session) = uiaa_info.session {
                        priv_.stack.set_visible_child_name("fallback");

                        let client = self.session().client();
                        let (sender, receiver) = futures::channel::oneshot::channel();
                        RUNTIME.spawn(async move { sender.send(client.homeserver().await) });
                        let homeserver = receiver.await.unwrap();
                        self.setup_fallback_page(
                            homeserver.as_str(),
                            flow.stages.first()?,
                            &session,
                        );
                        if self.show_and_wait_for_response().await {
                            auth_data =
                                Some(AuthData::FallbackAcknowledgement(FallbackAcknowledgement {
                                    session,
                                }));

                            continue;
                        }
                    }
                }
            }

            return None;
        }
    }

    async fn show_and_wait_for_response(&self) -> bool {
        let (sender, receiver) = futures::channel::oneshot::channel();
        let sender = Cell::new(Some(sender));

        let handler_id = self.connect_response(move |_, response| {
            if let Some(sender) = sender.take() {
                sender.send(response).unwrap();
            }
        });

        self.show();

        let result = receiver.await.unwrap();
        self.disconnect(handler_id);
        self.close();

        result
    }

    fn show_auth_error(&self, auth_error: &Option<ErrorBody>) {
        let priv_ = imp::AuthDialog::from_instance(self);

        if let Some(auth_error) = auth_error {
            priv_.error.set_label(&auth_error.message);
            priv_.error.show();
        } else {
            priv_.error.hide();
        }
    }

    fn setup_fallback_page(&self, homeserver: &str, auth_type: &str, session: &str) {
        let priv_ = imp::AuthDialog::from_instance(self);

        if let Some(handler) = priv_.open_browser_btn_handler.take() {
            priv_.open_browser_btn.disconnect(handler);
        }

        let uri = format!(
            "{}_matrix/client/r0/auth/{}/fallback/web?session={}",
            homeserver, auth_type, session
        );

        let handler =
            priv_
                .open_browser_btn
                .connect_clicked(clone!(@weak self as obj => move |_| {
                    gtk::show_uri(obj.transient_for().as_ref(), &uri, gdk::CURRENT_TIME);
                }));

        priv_.open_browser_btn_handler.replace(Some(handler));
    }

    pub fn connect_response<F: Fn(&Self, bool) + 'static>(&self, f: F) -> glib::SignalHandlerId {
        self.connect_local("response", true, move |values| {
            let obj: Self = values[0].get().unwrap();
            let response = values[1].get::<bool>().unwrap();

            f(&obj, response);

            None
        })
        .unwrap()
    }
}
