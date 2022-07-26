use gtk::{glib, prelude::*, subclass::prelude::*};
use matrix_sdk::{
    ruma::{
        events::{
            room::member::{MembershipState, RoomMemberEventContent},
            OriginalSyncStateEvent, StrippedStateEvent,
        },
        OwnedMxcUri, UserId,
    },
    RoomMember,
};

use crate::{
    prelude::*,
    session::{
        room::{
            power_levels::{PowerLevel, POWER_LEVEL_MAX, POWER_LEVEL_MIN},
            MemberRole,
        },
        Room, User,
    },
};

#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "Membership")]
pub enum Membership {
    Leave = 0,
    Join = 1,
    Invite = 2,
    Ban = 3,
    Knock = 4,
    Custom = 5,
}

impl Default for Membership {
    fn default() -> Self {
        Membership::Leave
    }
}

impl From<&MembershipState> for Membership {
    fn from(state: &MembershipState) -> Self {
        match state {
            MembershipState::Leave => Membership::Leave,
            MembershipState::Join => Membership::Join,
            MembershipState::Invite => Membership::Invite,
            MembershipState::Ban => Membership::Ban,
            MembershipState::Knock => Membership::Knock,
            _ => Membership::Custom,
        }
    }
}

impl From<MembershipState> for Membership {
    fn from(state: MembershipState) -> Self {
        Membership::from(&state)
    }
}

mod imp {
    use std::cell::Cell;

    use once_cell::sync::Lazy;

    use super::*;

    #[derive(Debug, Default)]
    pub struct Member {
        pub power_level: Cell<PowerLevel>,
        pub membership: Cell<Membership>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Member {
        const NAME: &'static str = "Member";
        type Type = super::Member;
        type ParentType = User;
    }

    impl ObjectImpl for Member {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecInt64::new(
                        "power-level",
                        "Power level",
                        "Power level of the member in its room.",
                        POWER_LEVEL_MIN,
                        POWER_LEVEL_MAX,
                        0,
                        glib::ParamFlags::READABLE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpecEnum::new(
                        "membership",
                        "Membership",
                        "This member’s membership state.",
                        Membership::static_type(),
                        Membership::default() as i32,
                        glib::ParamFlags::READABLE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                ]
            });

            PROPERTIES.as_ref()
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "power-level" => obj.power_level().to_value(),
                "membership" => obj.membership().to_value(),
                _ => unimplemented!(),
            }
        }
    }
}

glib::wrapper! {
    /// A User in the context of a given room.
    pub struct Member(ObjectSubclass<imp::Member>) @extends User;
}

impl Member {
    pub fn new(room: &Room, user_id: &UserId) -> Self {
        let session = room.session();
        glib::Object::new(&[("session", &session), ("user-id", &user_id.as_str())])
            .expect("Failed to create Member")
    }

    pub fn power_level(&self) -> PowerLevel {
        self.imp().power_level.get()
    }

    fn set_power_level(&self, power_level: PowerLevel) {
        if self.power_level() == power_level {
            return;
        }
        self.imp().power_level.replace(power_level);
        self.notify("power-level");
    }

    pub fn role(&self) -> MemberRole {
        self.power_level().into()
    }

    pub fn is_admin(&self) -> bool {
        self.role().is_admin()
    }

    pub fn is_mod(&self) -> bool {
        self.role().is_mod()
    }

    pub fn is_peasant(&self) -> bool {
        self.role().is_peasant()
    }

    pub fn membership(&self) -> Membership {
        let priv_ = imp::Member::from_instance(self);
        priv_.membership.get()
    }

    fn set_membership(&self, membership: Membership) {
        if self.membership() == membership {
            return;
        }
        let priv_ = imp::Member::from_instance(self);
        priv_.membership.replace(membership);
        self.notify("membership");
    }

    /// Update the user based on the room member.
    pub fn update_from_room_member(&self, member: &RoomMember) {
        if member.user_id() != &*self.user_id() {
            log::error!("Tried Member update from RoomMember with wrong user ID.");
            return;
        };

        self.set_display_name(member.display_name().map(String::from));
        self.avatar()
            .set_url(member.avatar_url().map(std::borrow::ToOwned::to_owned));
        self.set_power_level(member.power_level());
        self.set_membership(member.membership().into());
    }

    /// Update the user based on the room member state event
    pub fn update_from_member_event(&self, event: &impl MemberEvent) {
        if event.sender() != &*self.user_id() {
            log::error!("Tried Member update from MemberEvent with wrong user ID.");
            return;
        };

        self.set_display_name(event.display_name());
        self.avatar().set_url(event.avatar_url());
        self.set_membership((&event.content().membership).into());

        let session = self.session();
        if let Some(user) = session.user() {
            if user.user_id() == self.user_id() {
                session.update_user_profile();
            }
        }
    }
}

pub trait MemberEvent {
    fn sender(&self) -> &UserId;
    fn content(&self) -> &RoomMemberEventContent;

    fn avatar_url(&self) -> Option<OwnedMxcUri> {
        self.content().avatar_url.to_owned()
    }

    fn display_name(&self) -> Option<String> {
        match &self.content().displayname {
            Some(display_name) => Some(display_name.clone()),
            None => self
                .content()
                .third_party_invite
                .as_ref()
                .map(|i| i.display_name.clone()),
        }
    }
}

impl MemberEvent for OriginalSyncStateEvent<RoomMemberEventContent> {
    fn sender(&self) -> &UserId {
        &self.sender
    }
    fn content(&self) -> &RoomMemberEventContent {
        &self.content
    }
}
impl MemberEvent for StrippedStateEvent<RoomMemberEventContent> {
    fn sender(&self) -> &UserId {
        &self.sender
    }
    fn content(&self) -> &RoomMemberEventContent {
        &self.content
    }
}
