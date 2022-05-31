use std::time::Duration;

use gettextrs::gettext;
use gtk::{glib, glib::clone, prelude::*, subclass::prelude::*};
use log::{debug, error, warn};
use matrix_sdk::{
    encryption::{
        identities::RequestVerificationError,
        verification::{
            CancelInfo, Emoji, QrVerificationData, SasVerification, Verification,
            VerificationRequest,
        },
    },
    ruma::{
        events::key::verification::{cancel::CancelCode, VerificationMethod},
        UserId,
    },
    Client,
};
use qrcode::QrCode;
use tokio::sync::mpsc;

use super::{VERIFICATION_CREATION_TIMEOUT, VERIFICATION_RECEIVE_TIMEOUT};
use crate::{
    contrib::Camera,
    session::{
        sidebar::{SidebarItem, SidebarItemImpl},
        user::UserExt,
        Session, User,
    },
    spawn, spawn_tokio, toast,
};

#[derive(Debug, Eq, PartialEq, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "VerificationState")]
pub enum State {
    Requested,
    RequestSend,
    SasV1,
    QrV1Show,
    QrV1Scan,
    QrV1Scanned,
    Completed,
    Cancelled,
    Dismissed,
    Passive,
    Error,
}

impl Default for State {
    fn default() -> Self {
        Self::Requested
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "VerificationMode")]
pub enum Mode {
    CurrentSession,
    OtherSession,
    User,
}

impl Default for Mode {
    fn default() -> Self {
        Self::User
    }
}

#[glib::flags(name = "VerificationSupportedMethods")]
pub enum SupportedMethods {
    SAS = 0b00000001,
    QR_SHOW = 0b00000010,
    QR_SCAN = 0b00000100,
}

impl From<VerificationMethod> for SupportedMethods {
    fn from(method: VerificationMethod) -> Self {
        match method {
            VerificationMethod::SasV1 => Self::SAS,
            VerificationMethod::QrCodeScanV1 => Self::QR_SHOW,
            VerificationMethod::QrCodeShowV1 => Self::QR_SCAN,
            _ => Self::empty(),
        }
    }
}

impl From<SupportedMethods> for Vec<VerificationMethod> {
    fn from(methods: SupportedMethods) -> Self {
        let mut result = Vec::new();
        if methods.contains(SupportedMethods::SAS) {
            result.push(VerificationMethod::SasV1);
        }

        if methods.contains(SupportedMethods::QR_SHOW) {
            result.push(VerificationMethod::QrCodeShowV1);
        }

        if methods.contains(SupportedMethods::QR_SCAN) {
            result.push(VerificationMethod::QrCodeScanV1);
        }

        if methods.intersects(SupportedMethods::QR_SCAN | SupportedMethods::QR_SHOW) {
            result.push(VerificationMethod::ReciprocateV1);
        }

        result
    }
}

impl SupportedMethods {
    fn with_camera(has_camera: bool) -> Self {
        if has_camera {
            Self::all()
        } else {
            let mut methods = Self::all();
            methods.remove(SupportedMethods::QR_SCAN);
            methods
        }
    }
}

impl Default for SupportedMethods {
    fn default() -> Self {
        Self::empty()
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum UserAction {
    Accept,
    Match,
    NotMatch,
    Cancel,
    StartSas,
    Scanned(Box<QrVerificationData>),
    ConfirmScanning,
}

#[derive(Debug, PartialEq)]
pub enum Message {
    UserAction(UserAction),
    NotifyState,
}

pub enum MainMessage {
    QrCode(QrCode),
    SasData(SasData),
    SupportedMethods(SupportedMethods),
    CancelInfo(CancelInfo),
    State(State),
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum SasData {
    Emoji([Emoji; 7]),
    Decimal((u16, u16, u16)),
}

mod imp {
    use std::cell::{Cell, RefCell};

    use glib::object::WeakRef;
    use once_cell::{sync::Lazy, unsync::OnceCell};

    use super::*;

    #[derive(Default)]
    pub struct IdentityVerification {
        pub user: OnceCell<User>,
        pub session: OnceCell<WeakRef<Session>>,
        pub state: Cell<State>,
        pub mode: OnceCell<Mode>,
        pub supported_methods: Cell<SupportedMethods>,
        pub sync_sender: RefCell<Option<mpsc::Sender<Message>>>,
        pub main_sender: RefCell<Option<glib::SyncSender<MainMessage>>>,
        pub sas_data: OnceCell<SasData>,
        pub qr_code: OnceCell<QrCode>,
        pub cancel_info: OnceCell<CancelInfo>,
        pub flow_id: OnceCell<String>,
        pub start_time: OnceCell<glib::DateTime>,
        pub receive_time: OnceCell<glib::DateTime>,
        pub hide_error: Cell<bool>,
        pub force_current_session: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for IdentityVerification {
        const NAME: &'static str = "IdentityVerification";
        type Type = super::IdentityVerification;
        type ParentType = SidebarItem;
    }

    impl ObjectImpl for IdentityVerification {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecObject::new(
                        "user",
                        "User",
                        "The user to be verified",
                        User::static_type(),
                        glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY,
                    ),
                    glib::ParamSpecObject::new(
                        "session",
                        "Session",
                        "The current session",
                        Session::static_type(),
                        glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY,
                    ),
                    glib::ParamSpecEnum::new(
                        "state",
                        "State",
                        "The current state of this verification",
                        State::static_type(),
                        State::default() as i32,
                        glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY,
                    ),
                    glib::ParamSpecEnum::new(
                        "mode",
                        "Mode",
                        "The mode of this verification",
                        Mode::static_type(),
                        Mode::default() as i32,
                        glib::ParamFlags::READABLE,
                    ),
                    glib::ParamSpecFlags::new(
                        "supported-methods",
                        "Supported Methods",
                        "The supported methods of this verification",
                        SupportedMethods::static_type(),
                        SupportedMethods::default().bits(),
                        glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY,
                    ),
                    glib::ParamSpecString::new(
                        "display-name",
                        "Display name",
                        "The display name of this verification request",
                        None,
                        glib::ParamFlags::READABLE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpecString::new(
                        "flow-id",
                        "Flow Id",
                        "The flow id of this verification request",
                        None,
                        glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY,
                    ),
                    glib::ParamSpecBoxed::new(
                        "start-time",
                        "Start Time",
                        "The time when this verification request was started",
                        glib::DateTime::static_type(),
                        glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY,
                    ),
                    glib::ParamSpecBoxed::new(
                        "receive-time",
                        "Receive Time",
                        "The time when this verification request was received",
                        glib::DateTime::static_type(),
                        glib::ParamFlags::READABLE,
                    ),
                    glib::ParamSpecBoolean::new(
                        "force-current-session",
                        "Force Current Session",
                        "Whether this should be automatically accepted and treated as a Mode::CurrentSession",
                        false,
                        glib::ParamFlags::READWRITE| glib::ParamFlags::EXPLICIT_NOTIFY,
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
                "user" => obj.set_user(value.get().unwrap()),
                "session" => obj.set_session(value.get().unwrap()),
                "state" => obj.set_state(value.get().unwrap()),
                "flow-id" => obj.set_flow_id(value.get().unwrap()),
                "start-time" => obj.set_start_time(value.get().unwrap()),
                "supported-methods" => obj.set_supported_methods(value.get().unwrap()),
                "force-current-session" => obj.set_force_current_session(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "user" => obj.user().to_value(),
                "session" => obj.session().to_value(),
                "state" => obj.state().to_value(),
                "mode" => obj.mode().to_value(),
                "display-name" => obj.display_name().to_value(),
                "flow-id" => obj.flow_id().to_value(),
                "supported-methods" => obj.supported_methods().to_value(),
                "start-time" => obj.start_time().to_value(),
                "receive-time" => obj.receive_time().to_value(),
                "force-current-session" => obj.force_current_session().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            let (main_sender, main_receiver) =
                glib::MainContext::sync_channel::<MainMessage>(Default::default(), 100);

            main_receiver.attach(
                None,
                clone!(@weak obj => @default-return glib::Continue(false), move |message| {
                    let priv_ = obj.imp();
                    match message {
                        MainMessage::QrCode(data) => { let _ = priv_.qr_code.set(data); },
                        MainMessage::CancelInfo(data) => priv_.cancel_info.set(data).unwrap(),
                        MainMessage::SasData(data) => priv_.sas_data.set(data).unwrap(),
                        MainMessage::SupportedMethods(flags) => obj.set_supported_methods(flags),
                        MainMessage::State(state) => obj.set_state(state),
                    }

                    glib::Continue(true)
                }),
            );

            self.main_sender.replace(Some(main_sender));

            // We don't need to track ourselves because we show "Login Request" as name in
            // that case.
            if obj.user() != obj.session().user().unwrap() {
                obj.user().connect_notify_local(
                    Some("display-name"),
                    clone!(@weak obj => move |_, _| {
                        obj.notify("display-name");
                    }),
                );
            }

            self.receive_time
                .set(glib::DateTime::now_local().unwrap())
                .unwrap();
            obj.setup_timeout();
            obj.start_handler();
        }

        fn dispose(&self, obj: &Self::Type) {
            obj.cancel(true);
        }
    }

    impl SidebarItemImpl for IdentityVerification {}
}

glib::wrapper! {
    pub struct IdentityVerification(ObjectSubclass<imp::IdentityVerification>)
        @extends SidebarItem;
}

impl IdentityVerification {
    fn for_error(session: &Session, user: &User, start_time: &glib::DateTime) -> Self {
        glib::Object::new(&[
            ("state", &State::Error),
            ("session", session),
            ("user", user),
            ("start-time", start_time),
        ])
        .expect("Failed to create IdentityVerification")
    }

    /// Create a new object tracking an already existing verification request
    pub fn for_flow_id(
        flow_id: &str,
        session: &Session,
        user: &User,
        start_time: &glib::DateTime,
    ) -> Self {
        glib::Object::new(&[
            ("flow-id", &flow_id),
            ("session", session),
            ("user", user),
            ("supported-methods", &SupportedMethods::with_camera(true)),
            ("start-time", start_time),
        ])
        .expect("Failed to create IdentityVerification")
    }

    /// Creates and send a new verification request
    ///
    /// If `User` is `None` a new session verification is started for our own
    /// user and send to other devices
    pub async fn create(session: &Session, user: Option<&User>) -> Self {
        let user = if let Some(user) = user {
            user
        } else {
            session.user().unwrap()
        };

        let supported_methods =
            SupportedMethods::with_camera(Camera::default().has_camera().await.unwrap_or_default());

        if let Some(identity) = user.crypto_identity().await {
            let handle = spawn_tokio!(async move {
                identity
                    .request_verification_with_methods(supported_methods.into())
                    .await
            });

            match handle.await.unwrap() {
                Ok(request) => {
                    let obj = glib::Object::new(&[
                        ("state", &State::RequestSend),
                        ("supported-methods", &supported_methods),
                        ("flow-id", &request.flow_id()),
                        ("session", session),
                        ("user", user),
                        ("start-time", &glib::DateTime::now_local().unwrap()),
                    ])
                    .expect("Failed to create IdentityVerification");

                    return obj;
                }
                Err(error) => {
                    error!("Starting a verification failed: {}", error);
                }
            }
        } else {
            error!("Starting a verification failed: Crypto identity wasn't found");
        }

        Self::for_error(session, user, &glib::DateTime::now_local().unwrap())
    }

    fn start_handler(&self) {
        let priv_ = self.imp();

        let main_sender = if let Some(main_sender) = priv_.main_sender.take() {
            main_sender
        } else {
            warn!("The verification request was already started");
            return;
        };

        let client = self.session().client();
        let user_id = self.user().user_id();
        let flow_id = self.flow_id().to_owned();

        let (sync_sender, sync_receiver) = mpsc::channel(100);
        priv_.sync_sender.replace(Some(sync_sender));

        let supported_methods = self.supported_methods();

        let handle = spawn_tokio!(async move {
            if let Some(context) = Context::new(
                client,
                &user_id,
                &flow_id,
                main_sender,
                sync_receiver,
                supported_methods,
            )
            .await
            {
                context.start().await
            } else {
                error!("Unable to start verification handler");
                Ok(State::Error)
            }
        });

        let weak_obj = self.downgrade();
        spawn!(async move {
            let result = handle.await.unwrap();
            if let Some(obj) = weak_obj.upgrade() {
                match result {
                    Ok(result) => obj.set_state(result),
                    Err(error) => {
                        // FIXME: report error to the user
                        error!("Verification failed: {}", error);
                        obj.set_state(State::Error);
                    }
                }
                obj.imp().sync_sender.take();
            }
        });
    }

    /// The user to be verified.
    pub fn user(&self) -> &User {
        self.imp().user.get().unwrap()
    }

    fn set_user(&self, user: User) {
        self.imp().user.set(user).unwrap()
    }

    pub fn force_current_session(&self) -> bool {
        self.imp().force_current_session.get()
    }

    /// Force that this `IdentityVerification` is considered a
    /// `Mode::CurrentSession`. This is usfull that incoming requests during
    /// setup are accepted directly.
    pub fn set_force_current_session(&self, force: bool) {
        if self.force_current_session() == force {
            return;
        }

        self.imp().force_current_session.set(force);
        self.accept();
        self.notify("force-current-session");
    }

    /// The current `Session`.
    pub fn session(&self) -> Session {
        self.imp().session.get().unwrap().upgrade().unwrap()
    }

    fn set_session(&self, session: Session) {
        self.imp().session.set(session.downgrade()).unwrap()
    }

    fn setup_timeout(&self) {
        let difference = glib::DateTime::now_local()
            .unwrap()
            .difference(self.start_time())
            .as_seconds();

        if difference < 0 {
            warn!("The verification request was sent in the future.");
            self.cancel(false);
            return;
        }
        let difference = Duration::from_secs(difference as u64);
        let remaining_creation = VERIFICATION_CREATION_TIMEOUT.saturating_sub(difference);

        let remaining_receive = VERIFICATION_RECEIVE_TIMEOUT.saturating_sub(difference);

        let remaining = std::cmp::max(remaining_creation, remaining_receive);

        if remaining.is_zero() {
            self.cancel(false);
            return;
        }

        glib::source::timeout_add_local(
            remaining,
            clone!(@weak self as obj => @default-return glib::Continue(false), move || {
                obj.cancel(false);

                glib::Continue(false)
            }),
        );
    }

    /// The time and date when this verification request was started.
    pub fn start_time(&self) -> &glib::DateTime {
        self.imp().start_time.get().unwrap()
    }

    fn set_start_time(&self, time: glib::DateTime) {
        self.imp().start_time.set(time).unwrap();
    }

    pub fn receive_time(&self) -> &glib::DateTime {
        self.imp().receive_time.get().unwrap()
    }

    fn set_supported_methods(&self, supported_methods: SupportedMethods) {
        if self.supported_methods() == supported_methods {
            return;
        }

        self.imp().supported_methods.set(supported_methods);
        self.notify("supported-methods");
    }

    pub fn supported_methods(&self) -> SupportedMethods {
        self.imp().supported_methods.get()
    }

    pub fn emoji_match(&self) {
        if self.state() == State::SasV1 {
            if let Some(sync_sender) = &*self.imp().sync_sender.borrow() {
                let result = sync_sender.try_send(Message::UserAction(UserAction::Match));

                if let Err(error) = result {
                    error!("Failed to send message to tokio runtime: {}", error);
                }
            }
        }
    }

    pub fn emoji_not_match(&self) {
        if self.state() == State::SasV1 {
            if let Some(sync_sender) = &*self.imp().sync_sender.borrow() {
                let result = sync_sender.try_send(Message::UserAction(UserAction::NotMatch));

                if let Err(error) = result {
                    error!("Failed to send message to tokio runtime: {}", error);
                }
            }
        }
    }

    pub fn confirm_scanning(&self) {
        if self.state() == State::QrV1Scanned {
            if let Some(sync_sender) = &*self.imp().sync_sender.borrow() {
                let result = sync_sender.try_send(Message::UserAction(UserAction::ConfirmScanning));

                if let Err(error) = result {
                    error!("Failed to send message to tokio runtime: {}", error);
                }
            }
        }
    }

    pub fn state(&self) -> State {
        self.imp().state.get()
    }

    fn set_state(&self, state: State) {
        if self.state() == state {
            return;
        }

        match state {
            State::Cancelled | State::Error => self.show_error(),
            _ => {}
        }

        self.imp().state.set(state);
        self.notify("state");
    }

    pub fn mode(&self) -> Mode {
        let session = self.session();
        let our_user = session.user().unwrap();
        if our_user.user_id() == self.user().user_id() {
            if self.force_current_session() {
                Mode::CurrentSession
            } else {
                Mode::OtherSession
            }
        } else {
            Mode::User
        }
    }

    /// Whether this request is finished
    pub fn is_finished(&self) -> bool {
        matches!(
            self.state(),
            State::Error | State::Cancelled | State::Dismissed | State::Completed | State::Passive
        )
    }

    fn hide_error(&self) -> bool {
        self.imp().hide_error.get()
    }

    fn show_error(&self) {
        if self.hide_error() {
            return;
        }

        let error_message = if let Some(info) = self.cancel_info() {
            match info.cancel_code() {
                CancelCode::User => Some(gettext("You cancelled the verification process.")),
                CancelCode::Timeout => Some(gettext(
                    "The verification process failed because it reached a timeout.",
                )),
                CancelCode::Accepted => {
                    Some(gettext("You accepted the request from an other session."))
                }
                _ => match info.cancel_code().as_str() {
                    "m.mismatched_sas" => Some(gettext("The emoji did not match.")),
                    _ => None,
                },
            }
        } else {
            None
        };

        let error_message = error_message.unwrap_or_else(|| {
            gettext("An unknown error occurred during the verification process.")
        });

        toast!(self.session(), error_message);
    }

    pub fn display_name(&self) -> String {
        if self.user() != self.session().user().unwrap() {
            self.user().display_name()
        } else {
            // TODO: give this request a name based on the device
            "Login Request".to_string()
        }
    }

    pub fn flow_id(&self) -> &str {
        self.imp()
            .flow_id
            .get()
            .expect("Flow Id isn't always set on verifications with error state.")
    }

    fn set_flow_id(&self, flow_id: String) {
        self.imp().flow_id.set(flow_id).unwrap();
    }

    /// Get the QrCode for this verification request
    ///
    /// This is only set once the request reached the `State::Ready`
    /// and if QrCode verification is possible
    pub fn qr_code(&self) -> Option<&QrCode> {
        self.imp().qr_code.get()
    }

    /// Get the Emojis for this verification request
    ///
    /// This is only set once the request reached the `State::Ready`
    /// and if a Sas verification was started
    pub fn sas_data(&self) -> Option<&SasData> {
        self.imp().sas_data.get()
    }

    pub fn start_sas(&self) {
        if self.state() != State::SasV1 {
            if let Some(sync_sender) = &*self.imp().sync_sender.borrow() {
                let result = sync_sender.try_send(Message::UserAction(UserAction::StartSas));

                if let Err(error) = result {
                    error!("Failed to send message to tokio runtime: {}", error);
                }
            }
        }
    }

    pub fn scanned_qr_code(&self, data: QrVerificationData) {
        if let Some(sync_sender) = &*self.imp().sync_sender.borrow() {
            let result =
                sync_sender.try_send(Message::UserAction(UserAction::Scanned(Box::new(data))));

            if let Err(error) = result {
                error!("Failed to send message to tokio runtime: {}", error);
            }
        }
    }

    /// Accept an incoming request
    pub fn accept(&self) {
        if self.state() == State::Requested {
            if let Some(sync_sender) = &*self.imp().sync_sender.borrow() {
                let result = sync_sender.try_send(Message::UserAction(UserAction::Accept));
                if let Err(error) = result {
                    error!("Failed to send message to tokio runtime: {}", error);
                }
            }
        }
    }

    pub fn cancel(&self, hide_error: bool) {
        let priv_ = self.imp();

        priv_.hide_error.set(hide_error);

        if let Some(sync_sender) = &*priv_.sync_sender.borrow() {
            let result = sync_sender.try_send(Message::UserAction(UserAction::Cancel));
            if let Err(error) = result {
                error!("Failed to send message to tokio runtime: {}", error);
            }
        }
    }

    pub fn dismiss(&self) {
        self.set_state(State::Dismissed);
    }

    /// Get information about why the request was cancelled
    pub fn cancel_info(&self) -> Option<&CancelInfo> {
        self.imp().cancel_info.get()
    }

    pub fn notify_state(&self) {
        if let Some(sync_sender) = &*self.imp().sync_sender.borrow() {
            let result = sync_sender.try_send(Message::NotifyState);
            if let Err(error) = result {
                error!("Failed to send message to tokio runtime: {}", error);
            }
        }
    }
}

struct Context {
    client: Client,
    main_sender: glib::SyncSender<MainMessage>,
    sync_receiver: mpsc::Receiver<Message>,
    request: VerificationRequest,
    supported_methods: SupportedMethods,
}

macro_rules! wait {
    ( $this:ident $(, $expected:expr )? $(; expect_match $allow_action:ident )? ) => {
        {
            loop {
                // FIXME: add method to the sdk to check if a SAS verification was started
                if let Some(Verification::SasV1(sas)) = $this.client.encryption().get_verification($this.request.other_user_id(), $this.request.flow_id()).await {
                    return $this.continue_sas(sas).await;
                }

                if $this.request.is_passive() {
                    return Ok(State::Passive);
                }

                $(
                    if $expected {
                        break;
                    }
                )?

                match $this.sync_receiver.recv().await.expect("The channel was closed unexpected") {
                    Message::NotifyState if $this.request.is_cancelled() => {
                        if let Some(info) = $this.request.cancel_info() {
                            $this.send_cancel_info(info);
                        }
                        return Ok(State::Cancelled);
                    },
                    Message::UserAction(UserAction::Cancel) => {
                        return $this.cancel_request().await;
                    },
                     Message::UserAction(UserAction::NotMatch) => {
                        return $this.sas_mismatch().await;
                    },
                    Message::UserAction(UserAction::Accept) => {
                        if true $(&& $allow_action)? {
                            break;
                        }
                    },
                    Message::UserAction(UserAction::ConfirmScanning) => {
                        break;
                    },
                    Message::UserAction(UserAction::StartSas) => {
                        if true $(&& $allow_action)? {
                            return $this.start_sas().await;
                        }
                    },
                    Message::UserAction(UserAction::Match) => {
                        if $this.request.is_passive() {
                            return Ok(State::Passive);
                        }

                        // Break only if we are in the expected state
                        if true $(&& $expected)? {
                            break;
                        }
                    },
                    Message::UserAction(UserAction::Scanned(data)) => {
                        if true $(&& $allow_action)? {
                            return $this.finish_scanning(data).await;
                        }
                    },
                    Message::NotifyState => {
                    }
                }
            }
        }
    };
}

// WORKAROUND: since rust thinks that we are creating a recursive async function
macro_rules! wait_without_scanning_sas {
    ( $this:ident $(, $expected:expr )?) => {
        {
            loop {
                if $this.request.is_passive() {
                    return Ok(State::Passive);
                }

                $(
                    if $expected {
                        break;
                    }
                )?

                match $this.sync_receiver.recv().await.expect("The channel was closed unexpected") {
                    Message::NotifyState if $this.request.is_cancelled() => {
                        if let Some(info) = $this.request.cancel_info() {
                            $this.send_cancel_info(info);
                        }
                        return Ok(State::Cancelled);
                    },
                    Message::UserAction(UserAction::Cancel) => {
                        return $this.cancel_request().await;
                    }
                    Message::UserAction(UserAction::NotMatch) => {
                        return $this.sas_mismatch().await;
                    },
                    Message::UserAction(UserAction::Accept) => {
                        break;
                    },
                    Message::UserAction(UserAction::StartSas) => {
                    },
                    Message::UserAction(UserAction::ConfirmScanning) => {
                        break;
                    },
                    Message::UserAction(UserAction::Match) => {
                        if $this.request.is_passive() {
                            return Ok(State::Passive);
                        }

                        // Break only if we are in the expected state
                        if true $(&& $expected)? {
                            break;
                        }
                    },
                    Message::UserAction(UserAction::Scanned(_)) => {
                    },
                    Message::NotifyState => {
                    }
                }
            }
        }
    };
}

impl Context {
    pub async fn new(
        client: Client,
        user_id: &UserId,
        flow_id: &str,
        main_sender: glib::SyncSender<MainMessage>,
        sync_receiver: mpsc::Receiver<Message>,
        supported_methods: SupportedMethods,
    ) -> Option<Self> {
        let request = client
            .encryption()
            .get_verification_request(user_id, flow_id)
            .await?;

        Some(Self {
            client,
            request,
            main_sender,
            sync_receiver,
            supported_methods,
        })
    }

    fn send_state(&self, state: State) {
        self.main_sender.send(MainMessage::State(state)).unwrap();
    }

    fn send_qr_code(&self, qr_code: QrCode) {
        self.main_sender.send(MainMessage::QrCode(qr_code)).unwrap();
    }

    fn send_sas_data(&self, data: SasData) {
        self.main_sender.send(MainMessage::SasData(data)).unwrap();
    }

    fn send_cancel_info(&self, cancel_info: CancelInfo) {
        self.main_sender
            .send(MainMessage::CancelInfo(cancel_info))
            .unwrap();
    }

    fn send_supported_methods(&self, flags: SupportedMethods) {
        self.main_sender
            .send(MainMessage::SupportedMethods(flags))
            .unwrap();
    }

    async fn start(mut self) -> Result<State, RequestVerificationError> {
        if self.request.we_started() {
            debug!("Wait for ready state");
            wait![self, self.request.is_ready()];
        } else {
            // Check if it was started by somebody else already
            if self.request.is_passive() {
                return Ok(State::Passive);
            }

            debug!("Wait for user action accept or cancel");
            // Wait for the user to accept or cancel the request
            wait![self];

            // Check whether we have a camera
            if !Camera::default().has_camera().await.unwrap_or_default() {
                self.supported_methods.remove(SupportedMethods::QR_SCAN);
            }

            self.request
                .accept_with_methods(self.supported_methods.into())
                .await?;
        }

        let their_supported_methods: SupportedMethods = self
            .request
            .their_supported_methods()
            .unwrap()
            .into_iter()
            .map(Into::into)
            .collect();

        self.supported_methods &= their_supported_methods;

        self.send_supported_methods(self.supported_methods);

        let request = if self.supported_methods.contains(SupportedMethods::QR_SHOW) {
            let request = self
                .request
                .generate_qr_code()
                .await
                .map_err(RequestVerificationError::Sdk)?
                .expect("Couldn't create qr-code");

            if let Ok(qr_code) = request.to_qr_code() {
                self.send_qr_code(qr_code);
            } else {
                error!("Unable to generate Qr code for verification");
                return Ok(State::Error);
            }

            self.send_state(State::QrV1Show);

            request
        } else if self.supported_methods.contains(SupportedMethods::QR_SCAN) {
            self.send_state(State::QrV1Scan);

            // Wait for scanned data
            debug!("Wait for user to scan a qr-code");
            wait![self];

            unreachable!();
        } else {
            return self.start_sas().await;
        };

        wait![self, request.has_been_scanned()];

        self.send_state(State::QrV1Scanned);

        wait![self];

        request.confirm().await?;

        debug!("Wait for done state");
        wait![self, request.is_done()];

        Ok(State::Completed)
    }

    async fn finish_scanning(
        mut self,
        data: Box<QrVerificationData>,
    ) -> Result<State, RequestVerificationError> {
        let request = self
            .request
            .scan_qr_code(*data)
            .await?
            .expect("Scanning Qr Code should be supported");

        request.confirm().await?;

        debug!("Wait for done state");
        wait_without_scanning_sas![self, request.is_done()];

        Ok(State::Completed)
    }

    async fn start_sas(self) -> Result<State, RequestVerificationError> {
        let request = self
            .request
            .start_sas()
            .await
            .map_err(RequestVerificationError::Sdk)?
            .expect("Sas should be supported");

        self.continue_sas(request).await
    }

    async fn continue_sas(
        mut self,
        request: SasVerification,
    ) -> Result<State, RequestVerificationError> {
        request.accept().await?;

        debug!("Wait for emojis to be ready");
        wait_without_scanning_sas![self, request.can_be_presented()];

        let sas_data = if let Some(emoji) = request.emoji() {
            SasData::Emoji(emoji)
        } else if let Some(decimal) = request.decimals() {
            SasData::Decimal(decimal)
        } else {
            error!("SAS verification failed because neither emoji nor decimal are supported by the server");
            return Ok(State::Error);
        };

        self.send_sas_data(sas_data);
        self.send_state(State::SasV1);

        // Wait for match user action
        debug!("Wait for user action match or mismatch");
        wait_without_scanning_sas![self];

        request.confirm().await?;

        debug!("Wait for done state");
        wait_without_scanning_sas![self, request.is_done()];

        Ok(State::Completed)
    }

    async fn cancel_request(self) -> Result<State, RequestVerificationError> {
        self.request.cancel().await?;

        if let Some(info) = self.request.cancel_info() {
            self.send_cancel_info(info);
        }

        Ok(State::Cancelled)
    }

    async fn sas_mismatch(self) -> Result<State, RequestVerificationError> {
        if let Some(Verification::SasV1(sas_verification)) = self
            .client
            .encryption()
            .get_verification(self.request.other_user_id(), self.request.flow_id())
            .await
        {
            sas_verification.mismatch().await?;

            if let Some(info) = sas_verification.cancel_info() {
                self.send_cancel_info(info);
            }
        }

        Ok(State::Cancelled)
    }
}
