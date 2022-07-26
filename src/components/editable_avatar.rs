use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::{
    gdk, gio, glib,
    glib::{clone, closure_local},
    prelude::*,
    subclass::prelude::*,
    CompositeTemplate,
};
use log::error;

use super::{ActionButton, ActionState};
use crate::{session::Avatar, spawn, toast};

mod imp {
    use std::cell::{Cell, RefCell};

    use glib::subclass::{InitializingObject, Signal};
    use once_cell::sync::Lazy;

    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/org/gnome/Fractal/components-editable-avatar.ui")]
    pub struct EditableAvatar {
        /// The avatar to display.
        pub avatar: RefCell<Option<Avatar>>,
        /// Whether this avatar is changeable.
        pub editable: Cell<bool>,
        /// The state of the avatar edit.
        pub edit_state: Cell<ActionState>,
        /// Whether the edit button is sensitive.
        pub edit_sensitive: Cell<bool>,
        /// Whether this avatar is removable.
        pub removable: Cell<bool>,
        /// The state of the avatar removal.
        pub remove_state: Cell<ActionState>,
        /// Whether the remove button is sensitive.
        pub remove_sensitive: Cell<bool>,
        /// A temporary image to show instead of the avatar.
        pub temp_image: RefCell<Option<gdk::Paintable>>,
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub button_remove: TemplateChild<ActionButton>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for EditableAvatar {
        const NAME: &'static str = "ComponentsEditableAvatar";
        type Type = super::EditableAvatar;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            ActionButton::static_type();
            Self::bind_template(klass);

            klass.install_action("editable-avatar.edit-avatar", None, |obj, _, _| {
                spawn!(clone!(@weak obj => async move {
                    obj.choose_avatar().await;
                }));
            });
            klass.install_action("editable-avatar.remove-avatar", None, |obj, _, _| {
                obj.emit_by_name::<()>("remove-avatar", &[]);
            });
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for EditableAvatar {
        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![
                    Signal::builder(
                        "edit-avatar",
                        &[gio::File::static_type().into()],
                        <()>::static_type().into(),
                    )
                    .build(),
                    Signal::builder("remove-avatar", &[], <()>::static_type().into()).build(),
                ]
            });
            SIGNALS.as_ref()
        }

        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![
                    glib::ParamSpecObject::new(
                        "avatar",
                        "Avatar",
                        "The Avatar to display",
                        Avatar::static_type(),
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpecBoolean::new(
                        "editable",
                        "Editable",
                        "Whether this avatar is editable",
                        false,
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpecEnum::new(
                        "edit-state",
                        "Edit State",
                        "The state of the avatar edit",
                        ActionState::static_type(),
                        ActionState::default() as i32,
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpecBoolean::new(
                        "edit-sensitive",
                        "Edit Sensitive",
                        "Whether the edit button is sensitive",
                        true,
                        glib::ParamFlags::READWRITE
                            | glib::ParamFlags::EXPLICIT_NOTIFY
                            | glib::ParamFlags::CONSTRUCT,
                    ),
                    glib::ParamSpecBoolean::new(
                        "removable",
                        "Removable",
                        "Whether this avatar is removable",
                        false,
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpecEnum::new(
                        "remove-state",
                        "Remove State",
                        "The state of the avatar removal",
                        ActionState::static_type(),
                        ActionState::default() as i32,
                        glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                    ),
                    glib::ParamSpecBoolean::new(
                        "remove-sensitive",
                        "Remove Sensitive",
                        "Whether the remove button is sensitive",
                        true,
                        glib::ParamFlags::READWRITE
                            | glib::ParamFlags::EXPLICIT_NOTIFY
                            | glib::ParamFlags::CONSTRUCT,
                    ),
                    glib::ParamSpecObject::new(
                        "temp-image",
                        "Temp Image",
                        "A temporary image to show instead of the avatar",
                        gdk::Paintable::static_type(),
                        glib::ParamFlags::READABLE,
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
                "avatar" => obj.set_avatar(value.get().unwrap()),
                "editable" => obj.set_editable(value.get().unwrap()),
                "edit-state" => obj.set_edit_state(value.get().unwrap()),
                "edit-sensitive" => obj.set_edit_sensitive(value.get().unwrap()),
                "removable" => obj.set_removable(value.get().unwrap()),
                "remove-state" => obj.set_remove_state(value.get().unwrap()),
                "remove-sensitive" => obj.set_remove_sensitive(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "avatar" => obj.avatar().to_value(),
                "editable" => obj.editable().to_value(),
                "edit-state" => obj.edit_state().to_value(),
                "edit-sensitive" => obj.edit_sensitive().to_value(),
                "removable" => obj.removable().to_value(),
                "remove-state" => obj.remove_state().to_value(),
                "remove-sensitive" => obj.remove_sensitive().to_value(),
                "temp-image" => obj.temp_image().to_value(),
                _ => unimplemented!(),
            }
        }

        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            self.button_remove.set_extra_classes(&["error"]);
        }
    }

    impl WidgetImpl for EditableAvatar {}

    impl BinImpl for EditableAvatar {}
}

glib::wrapper! {
    /// An `Avatar` that can be edited.
    pub struct EditableAvatar(ObjectSubclass<imp::EditableAvatar>)
        @extends gtk::Widget, adw::Bin, @implements gtk::Accessible;
}

impl EditableAvatar {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create EditableAvatar")
    }

    pub fn avatar(&self) -> Option<Avatar> {
        self.imp().avatar.borrow().to_owned()
    }

    pub fn set_avatar(&self, avatar: Option<Avatar>) {
        if self.avatar() == avatar {
            return;
        }

        self.imp().avatar.replace(avatar);
        self.notify("avatar");
    }

    pub fn editable(&self) -> bool {
        self.imp().editable.get()
    }

    pub fn set_editable(&self, editable: bool) {
        if self.editable() == editable {
            return;
        }

        self.imp().editable.set(editable);
        self.notify("editable");
    }

    pub fn edit_state(&self) -> ActionState {
        self.imp().edit_state.get()
    }

    pub fn set_edit_state(&self, state: ActionState) {
        if self.edit_state() == state {
            return;
        }

        self.imp().edit_state.set(state);
        self.notify("edit-state");
    }

    pub fn edit_sensitive(&self) -> bool {
        self.imp().edit_sensitive.get()
    }

    pub fn set_edit_sensitive(&self, sensitive: bool) {
        if self.edit_sensitive() == sensitive {
            return;
        }

        self.imp().edit_sensitive.set(sensitive);
        self.notify("edit-sensitive");
    }

    pub fn removable(&self) -> bool {
        self.imp().removable.get()
    }

    pub fn set_removable(&self, removable: bool) {
        if self.removable() == removable {
            return;
        }

        self.imp().removable.set(removable);
        self.notify("removable");
    }

    pub fn remove_state(&self) -> ActionState {
        self.imp().remove_state.get()
    }

    pub fn set_remove_state(&self, state: ActionState) {
        if self.remove_state() == state {
            return;
        }

        self.imp().remove_state.set(state);
        self.notify("remove-state");
    }

    pub fn remove_sensitive(&self) -> bool {
        self.imp().remove_sensitive.get()
    }

    pub fn set_remove_sensitive(&self, sensitive: bool) {
        if self.remove_sensitive() == sensitive {
            return;
        }

        self.imp().remove_sensitive.set(sensitive);
        self.notify("remove-sensitive");
    }

    pub fn temp_image(&self) -> Option<gdk::Paintable> {
        self.imp().temp_image.borrow().clone()
    }

    pub fn set_temp_image_from_file(&self, file: Option<&gio::File>) {
        self.imp().temp_image.replace(
            file.and_then(|file| gdk::Texture::from_file(file).ok())
                .map(|texture| texture.upcast()),
        );
        self.notify("temp-image");
    }

    /// Show an avatar with `temp_image` instead of `avatar`.
    pub fn show_temp_image(&self, show_temp: bool) {
        let stack = &self.imp().stack;
        if show_temp {
            stack.set_visible_child_name("temp");
        } else {
            stack.set_visible_child_name("default");
        }
    }

    async fn choose_avatar(&self) {
        let image_filter = gtk::FileFilter::new();
        image_filter.add_mime_type("image/*");

        let dialog = gtk::FileChooserNative::builder()
            .title(&gettext("Choose Avatar"))
            .modal(true)
            .transient_for(
                self.root()
                    .as_ref()
                    .and_then(|root| root.downcast_ref::<gtk::Window>())
                    .unwrap(),
            )
            .action(gtk::FileChooserAction::Open)
            .accept_label(&gettext("Choose"))
            .cancel_label(&gettext("Cancel"))
            .filter(&image_filter)
            .build();

        if dialog.run_future().await == gtk::ResponseType::Accept {
            if let Some(file) = dialog.file() {
                if let Some(content_type) = file
                    .query_info_future(
                        &gio::FILE_ATTRIBUTE_STANDARD_CONTENT_TYPE,
                        gio::FileQueryInfoFlags::NONE,
                        glib::PRIORITY_LOW,
                    )
                    .await
                    .ok()
                    .and_then(|info| info.content_type())
                {
                    if gio::content_type_is_a(&content_type, "image/*") {
                        self.emit_by_name::<()>("edit-avatar", &[&file]);
                    } else {
                        error!("The chosen file is not an image");
                        toast!(self, gettext("The chosen file is not an image"));
                    }
                } else {
                    error!("Could not get the content type of the file");
                    toast!(
                        self,
                        gettext("Could not determine the type of the chosen file")
                    );
                }
            } else {
                error!("No file chosen");
                toast!(self, gettext("No file was chosen"));
            }
        }
    }

    pub fn connect_edit_avatar<F: Fn(&Self, gio::File) + 'static>(
        &self,
        f: F,
    ) -> glib::SignalHandlerId {
        self.connect_closure(
            "edit-avatar",
            true,
            closure_local!(|obj: Self, file: gio::File| {
                f(&obj, file);
            }),
        )
    }

    pub fn connect_remove_avatar<F: Fn(&Self) + 'static>(&self, f: F) -> glib::SignalHandlerId {
        self.connect_closure(
            "remove-avatar",
            true,
            closure_local!(|obj: Self| {
                f(&obj);
            }),
        )
    }
}

impl Default for EditableAvatar {
    fn default() -> Self {
        Self::new()
    }
}
