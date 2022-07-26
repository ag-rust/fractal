use gtk::{gio, glib, glib::clone, prelude::*, subclass::prelude::*};
use log::error;
use matrix_sdk::{
    ruma::{api::client::user_directory::search_users, OwnedUserId, UserId},
    HttpError,
};

use super::Invitee;
use crate::{
    session::{user::UserExt, Room},
    spawn, spawn_tokio,
};

#[derive(Debug, Eq, PartialEq, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "ContentInviteeListState")]
pub enum InviteeListState {
    Initial = 0,
    Loading = 1,
    NoMatching = 2,
    Matching = 3,
    Error = 4,
}

impl Default for InviteeListState {
    fn default() -> Self {
        Self::Initial
    }
}

mod imp {
    use std::{
        cell::{Cell, RefCell},
        collections::HashMap,
    };

    use futures::future::AbortHandle;
    use glib::subclass::Signal;
    use once_cell::{sync::Lazy, unsync::OnceCell};

    use super::*;

    #[derive(Debug, Default)]
    pub struct InviteeList {
        pub list: RefCell<Vec<Invitee>>,
        pub room: OnceCell<Room>,
        pub state: Cell<InviteeListState>,
        pub search_term: RefCell<Option<String>>,
        pub invitee_list: RefCell<HashMap<OwnedUserId, Invitee>>,
        pub abort_handle: RefCell<Option<AbortHandle>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for InviteeList {
        const NAME: &'static str = "InviteeList";
        type Type = super::InviteeList;
        type Interfaces = (gio::ListModel,);
    }

    impl ObjectImpl for InviteeList {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecObject::new(
                        "room",
                        "Room",
                        "The room this invitee list refers to",
                        Room::static_type(),
                        glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY,
                    ),
                    glib::ParamSpecString::new(
                        "search-term",
                        "Search Term",
                        "The search term",
                        None,
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpecBoolean::new(
                        "has-selected",
                        "Has Selected",
                        "Whether the user has selected some users",
                        false,
                        glib::ParamFlags::READABLE,
                    ),
                    glib::ParamSpecEnum::new(
                        "state",
                        "InviteeListState",
                        "The state of the list",
                        InviteeListState::static_type(),
                        InviteeListState::default() as i32,
                        glib::ParamFlags::READABLE,
                    ),
                ]
            });

            PROPERTIES.as_ref()
        }

        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![
                    Signal::builder(
                        "invitee-added",
                        &[Invitee::static_type().into()],
                        <()>::static_type().into(),
                    )
                    .build(),
                    Signal::builder(
                        "invitee-removed",
                        &[Invitee::static_type().into()],
                        <()>::static_type().into(),
                    )
                    .build(),
                ]
            });
            SIGNALS.as_ref()
        }

        fn set_property(
            &self,
            obj: &Self::Type,
            _id: usize,
            value: &glib::Value,
            pspec: &glib::ParamSpec,
        ) {
            match pspec.name() {
                "room" => self.room.set(value.get::<Room>().unwrap()).unwrap(),
                "search-term" => obj.set_search_term(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "room" => obj.room().to_value(),
                "search-term" => obj.search_term().to_value(),
                "has-selected" => obj.has_selected().to_value(),
                "state" => obj.state().to_value(),
                _ => unimplemented!(),
            }
        }
    }

    impl ListModelImpl for InviteeList {
        fn item_type(&self, _list_model: &Self::Type) -> glib::Type {
            Invitee::static_type()
        }
        fn n_items(&self, _list_model: &Self::Type) -> u32 {
            self.list.borrow().len() as u32
        }
        fn item(&self, _list_model: &Self::Type, position: u32) -> Option<glib::Object> {
            self.list
                .borrow()
                .get(position as usize)
                .map(glib::object::Cast::upcast_ref::<glib::Object>)
                .cloned()
        }
    }
}

glib::wrapper! {
    /// List of users matching the `search term`.
    pub struct InviteeList(ObjectSubclass<imp::InviteeList>)
        @implements gio::ListModel;
}

impl InviteeList {
    pub fn new(room: &Room) -> Self {
        glib::Object::new(&[("room", room)]).expect("Failed to create InviteeList")
    }

    pub fn room(&self) -> &Room {
        self.imp().room.get().unwrap()
    }

    pub fn set_search_term(&self, search_term: Option<String>) {
        let priv_ = self.imp();

        if search_term.as_ref() == priv_.search_term.borrow().as_ref() {
            return;
        }

        if search_term.as_ref().map_or(false, |s| s.is_empty()) {
            priv_.search_term.replace(None);
        } else {
            priv_.search_term.replace(search_term);
        }

        self.search_users();
        self.notify("search_term");
    }

    fn search_term(&self) -> Option<String> {
        self.imp().search_term.borrow().clone()
    }

    fn set_state(&self, state: InviteeListState) {
        let priv_ = self.imp();

        if state == self.state() {
            return;
        }

        priv_.state.set(state);
        self.notify("state");
    }

    pub fn state(&self) -> InviteeListState {
        self.imp().state.get()
    }

    fn set_list(&self, users: Vec<Invitee>) {
        let added = users.len();

        let prev_users = self.imp().list.replace(users);

        self.items_changed(0, prev_users.len() as u32, added as u32);
    }

    fn clear_list(&self) {
        self.set_list(Vec::new());
    }

    fn finish_search(
        &self,
        search_term: String,
        response: Result<search_users::v3::Response, HttpError>,
    ) {
        let session = self.room().session();
        let member_list = self.room().members();

        if Some(search_term) != self.search_term() {
            return;
        }

        match response {
            Ok(response) if response.results.is_empty() => {
                self.set_state(InviteeListState::NoMatching);
                self.clear_list();
            }
            Ok(response) => {
                let users: Vec<Invitee> = response
                    .results
                    .into_iter()
                    .filter_map(|item| {
                        // Skip over users that are already in the room
                        if member_list.contains(&item.user_id) {
                            self.remove_invitee(item.user_id);
                            None
                        } else if let Some(user) = self.get_invitee(&item.user_id) {
                            // The avatar or the display name may have changed in the mean time
                            user.set_avatar_url(item.avatar_url);
                            user.set_display_name(item.display_name);
                            Some(user)
                        } else {
                            let user = Invitee::new(
                                &session,
                                &item.user_id,
                                item.display_name.as_deref(),
                                item.avatar_url.as_deref(),
                            );

                            user.connect_notify_local(
                                Some("invited"),
                                clone!(@weak self as obj => move |user, _| {
                                    if user.is_invited() {
                                        obj.add_invitee(user.clone());
                                    } else {
                                        obj.remove_invitee(user.user_id())
                                    }
                                }),
                            );

                            Some(user)
                        }
                    })
                    .collect();

                self.set_list(users);
                self.set_state(InviteeListState::Matching);
            }
            Err(error) => {
                error!("Couldn’t load matching users: {}", error);
                self.set_state(InviteeListState::Error);
                self.clear_list();
            }
        }
    }

    fn search_users(&self) {
        let client = self.room().session().client();
        let search_term = if let Some(search_term) = self.search_term() {
            search_term
        } else {
            // Do nothing for no search term execpt when currently loading
            if self.state() == InviteeListState::Loading {
                self.set_state(InviteeListState::Initial);
            }
            return;
        };

        self.set_state(InviteeListState::Loading);
        self.clear_list();

        let search_term_clone = search_term.clone();
        let handle = spawn_tokio!(async move {
            let request = search_users::v3::Request::new(&search_term_clone);
            client.send(request, None).await
        });

        let (future, handle) = futures::future::abortable(handle);

        if let Some(abort_handle) = self.imp().abort_handle.replace(Some(handle)) {
            abort_handle.abort();
        }

        spawn!(clone!(@weak self as obj => async move {
            if let Ok(result) = future.await {
                obj.finish_search(search_term, result.unwrap());
            }
        }));
    }

    fn get_invitee(&self, user_id: &UserId) -> Option<Invitee> {
        self.imp().invitee_list.borrow().get(user_id).cloned()
    }

    pub fn add_invitee(&self, user: Invitee) {
        user.set_invited(true);
        self.imp()
            .invitee_list
            .borrow_mut()
            .insert(user.user_id(), user.clone());
        self.emit_by_name::<()>("invitee-added", &[&user]);
        self.notify("has-selected");
    }

    pub fn invitees(&self) -> Vec<Invitee> {
        self.imp()
            .invitee_list
            .borrow()
            .values()
            .map(Clone::clone)
            .collect()
    }

    fn remove_invitee(&self, user_id: OwnedUserId) {
        let removed = self.imp().invitee_list.borrow_mut().remove(&user_id);
        if let Some(user) = removed {
            user.set_invited(false);
            self.emit_by_name::<()>("invitee-removed", &[&user]);
            self.notify("has-selected");
        }
    }

    pub fn has_selected(&self) -> bool {
        !self.imp().invitee_list.borrow().is_empty()
    }

    pub fn connect_invitee_added<F: Fn(&Self, &Invitee) + 'static>(
        &self,
        f: F,
    ) -> glib::SignalHandlerId {
        self.connect_local("invitee-added", true, move |values| {
            let obj = values[0].get::<Self>().unwrap();
            let invitee = values[1].get::<Invitee>().unwrap();
            f(&obj, &invitee);
            None
        })
    }

    pub fn connect_invitee_removed<F: Fn(&Self, &Invitee) + 'static>(
        &self,
        f: F,
    ) -> glib::SignalHandlerId {
        self.connect_local("invitee-removed", true, move |values| {
            let obj = values[0].get::<Self>().unwrap();
            let invitee = values[1].get::<Invitee>().unwrap();
            f(&obj, &invitee);
            None
        })
    }
}
