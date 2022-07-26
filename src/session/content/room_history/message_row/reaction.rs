use adw::subclass::prelude::*;
use gtk::{glib, prelude::*, subclass::prelude::*, CompositeTemplate};

use crate::session::room::ReactionGroup;

mod imp {
    use glib::subclass::InitializingObject;
    use once_cell::{sync::Lazy, unsync::OnceCell};

    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/org/gnome/Fractal/content-message-reaction.ui")]
    pub struct MessageReaction {
        /// The reaction group to display.
        pub group: OnceCell<ReactionGroup>,
        #[template_child]
        pub button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub reaction_key: TemplateChild<gtk::Label>,
        #[template_child]
        pub reaction_count: TemplateChild<gtk::Label>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MessageReaction {
        const NAME: &'static str = "ContentMessageReaction";
        type Type = super::MessageReaction;
        type ParentType = gtk::FlowBoxChild;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for MessageReaction {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpecObject::new(
                    "group",
                    "Group",
                    "The reaction group to display",
                    ReactionGroup::static_type(),
                    glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY,
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
                "group" => {
                    obj.set_group(value.get().unwrap());
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, _obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "group" => self.group.get().to_value(),
                _ => unimplemented!(),
            }
        }
    }

    impl WidgetImpl for MessageReaction {}

    impl FlowBoxChildImpl for MessageReaction {}
}

glib::wrapper! {
    /// A widget displaying the reactions of a message.
    pub struct MessageReaction(ObjectSubclass<imp::MessageReaction>)
        @extends gtk::Widget, gtk::FlowBoxChild, @implements gtk::Accessible;
}

impl MessageReaction {
    pub fn new(reaction_group: ReactionGroup) -> Self {
        glib::Object::new(&[("group", &reaction_group)]).expect("Failed to create MessageReaction")
    }

    fn set_group(&self, group: ReactionGroup) {
        let priv_ = self.imp();
        let key = group.key();
        priv_.reaction_key.set_label(key);
        priv_
            .button
            .set_action_target_value(Some(&key.to_variant()));
        group
            .bind_property("has-user", &*priv_.button, "active")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();
        group
            .bind_property("count", &*priv_.reaction_count, "label")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        priv_.group.set(group).unwrap();
    }
}
