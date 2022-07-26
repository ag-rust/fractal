use gtk::{glib, glib::clone, prelude::*, subclass::prelude::*};

use super::Event;
use crate::session::UserExt;

mod imp {
    use std::cell::RefCell;

    use indexmap::IndexSet;
    use once_cell::{sync::Lazy, unsync::OnceCell};

    use super::*;

    #[derive(Debug, Default)]
    pub struct ReactionGroup {
        /// The key of the group.
        pub key: OnceCell<String>,
        /// The reactions in the group.
        pub reactions: RefCell<IndexSet<Event>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ReactionGroup {
        const NAME: &'static str = "ReactionGroup";
        type Type = super::ReactionGroup;
    }

    impl ObjectImpl for ReactionGroup {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecString::new(
                        "key",
                        "Key",
                        "The key of the group",
                        None,
                        glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY,
                    ),
                    glib::ParamSpecUInt::new(
                        "count",
                        "Count",
                        "The number of reactions in this group",
                        u32::MIN,
                        u32::MAX,
                        0,
                        glib::ParamFlags::READABLE,
                    ),
                    glib::ParamSpecBoolean::new(
                        "has-user",
                        "Has User",
                        "Whether this group has a reaction from this user",
                        false,
                        glib::ParamFlags::READABLE,
                    ),
                ]
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
                "key" => {
                    self.key.set(value.get::<String>().unwrap()).unwrap();
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "key" => obj.key().to_value(),
                "count" => obj.count().to_value(),
                "has-user" => obj.has_user().to_value(),
                _ => unimplemented!(),
            }
        }
    }
}

glib::wrapper! {
    /// Reactions grouped by a given key.
    pub struct ReactionGroup(ObjectSubclass<imp::ReactionGroup>);
}

impl ReactionGroup {
    pub fn new(key: &str) -> Self {
        glib::Object::new(&[("key", &key)]).expect("Failed to create ReactionGroup")
    }

    pub fn key(&self) -> &str {
        self.imp().key.get().unwrap()
    }

    pub fn count(&self) -> u32 {
        self.imp()
            .reactions
            .borrow()
            .iter()
            .filter(|event| !event.redacted())
            .count() as u32
    }

    /// The reaction in this group sent by this user, if any.
    pub fn user_reaction(&self) -> Option<Event> {
        let reactions = self.imp().reactions.borrow();
        if let Some(user) = reactions
            .first()
            .and_then(|event| event.room().session().user().cloned())
        {
            for reaction in reactions.iter().filter(|event| !event.redacted()) {
                if reaction.matrix_sender() == user.user_id() {
                    return Some(reaction.clone());
                }
            }
        }
        None
    }

    /// Whether this group has a reaction from this user.
    pub fn has_user(&self) -> bool {
        self.user_reaction().is_some()
    }

    /// Add new reactions to this group.
    pub fn add_reactions(&self, new_reactions: Vec<Event>) {
        let prev_has_user = self.has_user();
        let mut added_reactions = Vec::with_capacity(new_reactions.len());

        {
            let mut reactions = self.imp().reactions.borrow_mut();

            reactions.reserve(new_reactions.len());

            for reaction in new_reactions {
                if reactions.insert(reaction.clone()) {
                    added_reactions.push(reaction);
                }
            }
        }

        for reaction in added_reactions.iter() {
            // Reaction's source should only change when it is redacted.
            reaction.connect_notify_local(
                Some("source"),
                clone!(@weak self as obj => move |_, _| {
                    obj.notify("count");
                    obj.notify("has-user");
                }),
            );
        }

        if !added_reactions.is_empty() {
            self.notify("count");
        }

        if self.has_user() != prev_has_user {
            self.notify("has-user");
        }
    }
}
