use adw::{prelude::*, subclass::prelude::*};
use gettextrs::gettext;
use gtk::{gio, glib, glib::clone, subclass::prelude::*};
use matrix_sdk::ruma::events::AnySyncRoomEvent;

use crate::{
    components::{ContextMenuBin, ContextMenuBinExt, ContextMenuBinImpl, ReactionChooser},
    session::{
        content::room_history::{message_row::MessageRow, DividerRow, RoomHistory, StateRow},
        room::{
            Event, EventActions, TimelineDayDivider, TimelineItem, TimelineNewMessagesDivider,
            TimelineSpinner,
        },
    },
};

mod imp {
    use std::cell::RefCell;

    use glib::{signal::SignalHandlerId, WeakRef};
    use once_cell::unsync::OnceCell;

    use super::*;

    #[derive(Debug, Default)]
    pub struct ItemRow {
        pub room_history: OnceCell<WeakRef<RoomHistory>>,
        pub item: RefCell<Option<TimelineItem>>,
        pub action_group: RefCell<Option<gio::SimpleActionGroup>>,
        pub notify_handler: RefCell<Option<SignalHandlerId>>,
        pub binding: RefCell<Option<glib::Binding>>,
        pub reaction_chooser: RefCell<Option<ReactionChooser>>,
        pub emoji_chooser: RefCell<Option<gtk::EmojiChooser>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ItemRow {
        const NAME: &'static str = "ContentItemRow";
        type Type = super::ItemRow;
        type ParentType = ContextMenuBin;
    }

    impl ObjectImpl for ItemRow {
        fn properties() -> &'static [glib::ParamSpec] {
            use once_cell::sync::Lazy;
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecObject::new(
                        "item",
                        "Item",
                        "The timeline item represented by this row",
                        TimelineItem::static_type(),
                        glib::ParamFlags::READWRITE,
                    ),
                    glib::ParamSpecObject::new(
                        "room-history",
                        "room-history",
                        "The ancestor room history of this row",
                        RoomHistory::static_type(),
                        glib::ParamFlags::READWRITE | glib::ParamFlags::CONSTRUCT_ONLY,
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
                "item" => obj.set_item(value.get().unwrap()),
                "room-history" => self
                    .room_history
                    .set(value.get::<RoomHistory>().unwrap().downgrade())
                    .unwrap(),
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "item" => obj.item().to_value(),
                "room-history" => obj.room_history().to_value(),
                _ => unimplemented!(),
            }
        }

        fn dispose(&self, _obj: &Self::Type) {
            if let Some(event) = self
                .item
                .borrow()
                .as_ref()
                .and_then(|item| item.downcast_ref::<Event>())
            {
                if let Some(handler) = self.notify_handler.borrow_mut().take() {
                    event.disconnect(handler);
                }
            }
        }
    }

    impl WidgetImpl for ItemRow {}
    impl BinImpl for ItemRow {}

    impl ContextMenuBinImpl for ItemRow {
        fn menu_opened(&self, obj: &Self::Type) {
            if let Some(event) = obj.item().and_then(|item| item.downcast::<Event>().ok()) {
                let room_history = obj.room_history();
                let popover = room_history.item_context_menu().to_owned();

                if event.content().is_some() {
                    let menu_model = Self::Type::event_message_menu_model();
                    let reaction_chooser = room_history.item_reaction_chooser();
                    if popover.menu_model().as_ref() != Some(menu_model) {
                        popover.set_menu_model(Some(menu_model));
                        popover.add_child(reaction_chooser, "reaction-chooser");
                    }

                    reaction_chooser.set_reactions(Some(event.reactions().to_owned()));

                    // Open emoji chooser
                    let more_reactions = gio::SimpleAction::new("more-reactions", None);
                    more_reactions.connect_activate(
                        clone!(@weak obj, @weak popover => move |_, _| {
                            obj.show_emoji_chooser(&popover);
                        }),
                    );
                    obj.action_group().unwrap().add_action(&more_reactions);
                } else {
                    let menu_model = Self::Type::event_state_menu_model();
                    if popover.menu_model().as_ref() != Some(menu_model) {
                        popover.set_menu_model(Some(menu_model));
                    }
                }

                obj.set_popover(Some(popover));
            } else {
                obj.set_popover(None);
            }
        }
    }
}

glib::wrapper! {
    pub struct ItemRow(ObjectSubclass<imp::ItemRow>)
        @extends gtk::Widget, adw::Bin, ContextMenuBin, @implements gtk::Accessible;
}

impl ItemRow {
    pub fn new(room_history: &RoomHistory) -> Self {
        glib::Object::new(&[("room-history", room_history)]).expect("Failed to create ItemRow")
    }

    pub fn room_history(&self) -> RoomHistory {
        self.imp().room_history.get().unwrap().upgrade().unwrap()
    }

    pub fn action_group(&self) -> Option<gio::SimpleActionGroup> {
        self.imp().action_group.borrow().clone()
    }

    fn set_action_group(&self, action_group: Option<gio::SimpleActionGroup>) {
        if self.action_group() == action_group {
            return;
        }

        self.imp().action_group.replace(action_group);
    }

    /// Get the row's [`TimelineItem`].
    pub fn item(&self) -> Option<TimelineItem> {
        self.imp().item.borrow().clone()
    }

    /// This method sets this row to a new [`TimelineItem`].
    ///
    /// It tries to reuse the widget and only update the content whenever
    /// possible, but it will create a new widget and drop the old one if it
    /// has to.
    fn set_item(&self, item: Option<TimelineItem>) {
        let priv_ = self.imp();

        if let Some(event) = priv_
            .item
            .borrow()
            .as_ref()
            .and_then(|item| item.downcast_ref::<Event>())
        {
            if let Some(handler) = priv_.notify_handler.borrow_mut().take() {
                event.disconnect(handler);
            }
        } else if let Some(binding) = priv_.binding.borrow_mut().take() {
            binding.unbind()
        }

        if let Some(ref item) = item {
            if let Some(event) = item.downcast_ref::<Event>() {
                self.set_action_group(self.set_event_actions(Some(event)));

                let notify_handler = event.connect_notify_local(
                    Some("event"),
                    clone!(@weak self as obj => move |event, _| {
                        obj.set_event_widget(event);
                    }),
                );
                priv_.notify_handler.replace(Some(notify_handler));

                self.set_event_widget(event);
            } else if let Some(divider) = item.downcast_ref::<TimelineDayDivider>() {
                self.set_popover(None);
                self.set_action_group(None);
                self.set_event_actions(None);

                let child = if let Some(child) =
                    self.child().and_then(|w| w.downcast::<DividerRow>().ok())
                {
                    child
                } else {
                    let child = DividerRow::new();
                    self.set_child(Some(&child));
                    child
                };

                let binding = divider
                    .bind_property("formatted-date", &child, "label")
                    .flags(glib::BindingFlags::SYNC_CREATE)
                    .build();
                priv_.binding.replace(Some(binding));
            } else if item.downcast_ref::<TimelineSpinner>().is_some()
                && self
                    .child()
                    .filter(|widget| widget.is::<gtk::Spinner>())
                    .is_none()
            {
                self.set_popover(None);
                self.set_action_group(None);
                self.set_event_actions(None);

                let spinner = gtk::Spinner::builder()
                    .spinning(true)
                    .margin_top(12)
                    .margin_bottom(12)
                    .build();
                self.set_child(Some(&spinner));
            } else if item.downcast_ref::<TimelineNewMessagesDivider>().is_some() {
                self.set_popover(None);
                self.set_action_group(None);
                self.set_event_actions(None);

                let label = gettext("New Messages");

                if let Some(Ok(child)) = self.child().map(|w| w.downcast::<DividerRow>()) {
                    child.set_label(&label);
                } else {
                    let child = DividerRow::with_label(label);
                    self.set_child(Some(&child));
                };
            }
        }
        priv_.item.replace(item);
    }

    fn set_event_widget(&self, event: &Event) {
        match event.matrix_event() {
            Some(AnySyncRoomEvent::State(state)) => {
                let child = if let Some(Ok(child)) = self.child().map(|w| w.downcast::<StateRow>())
                {
                    child
                } else {
                    let child = StateRow::new();
                    self.set_child(Some(&child));
                    child
                };
                child.update(&state);
            }
            _ => {
                let child =
                    if let Some(Ok(child)) = self.child().map(|w| w.downcast::<MessageRow>()) {
                        child
                    } else {
                        let child = MessageRow::new();
                        self.set_child(Some(&child));
                        child
                    };
                child.set_event(event.clone());
            }
        }
    }

    fn show_emoji_chooser(&self, popover: &gtk::PopoverMenu) {
        let emoji_chooser = gtk::EmojiChooser::builder().has_arrow(false).build();
        emoji_chooser.connect_emoji_picked(clone!(@weak self as obj => move |_, emoji| {
            obj
                .activate_action("event.toggle-reaction", Some(&emoji.to_variant()))
                .unwrap();
        }));
        emoji_chooser.set_parent(self);
        emoji_chooser.connect_closed(|emoji_chooser| {
            emoji_chooser.unparent();
        });

        let (_, rectangle) = popover.pointing_to();
        emoji_chooser.set_pointing_to(Some(&rectangle));

        popover.popdown();
        emoji_chooser.popup();
    }
}

impl EventActions for ItemRow {}
