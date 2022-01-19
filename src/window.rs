use crate::components::InAppNotification;
use crate::config::{APP_ID, PROFILE};
use crate::secret;
use crate::Application;
use crate::Login;
use crate::Session;
use crate::{Error, UserFacingError};
use adw::subclass::prelude::AdwApplicationWindowImpl;
use gettextrs::gettext;
use glib::signal::Inhibit;
use gtk::subclass::prelude::*;
use gtk::{self, prelude::*};
use gtk::{gio, glib, glib::clone, CompositeTemplate};
use log::warn;

mod imp {
    use super::*;
    use glib::subclass::InitializingObject;

    #[derive(Debug, CompositeTemplate, Default)]
    #[template(resource = "/org/gnome/FractalNext/window.ui")]
    pub struct Window {
        #[template_child]
        pub main_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub login: TemplateChild<Login>,
        #[template_child]
        pub sessions: TemplateChild<gtk::Stack>,
        #[template_child]
        pub error_list: TemplateChild<gio::ListStore>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Window {
        const NAME: &'static str = "Window";
        type Type = super::Window;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            Error::static_type();
            InAppNotification::static_type();
            Self::bind_template(klass);
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Window {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            let builder = gtk::Builder::from_resource("/org/gnome/FractalNext/shortcuts.ui");
            let shortcuts = builder.object("shortcuts").unwrap();
            obj.set_help_overlay(Some(&shortcuts));

            // Devel Profile
            if PROFILE == "Devel" {
                obj.add_css_class("devel");
            }

            obj.load_window_size();
            obj.restore_sessions();

            self.login
                .connect_new_session(clone!(@weak obj => move |_login, session| {
                    obj.add_session(&session);
                    obj.switch_to_sessions_page();
                }));

            self.main_stack.connect_visible_child_notify(
                clone!(@weak obj => move |_| obj.set_default_by_child()),
            );

            obj.set_default_by_child();

            // Ask for the toggle fullscreen state
            let fullscreen = gio::SimpleAction::new("toggle-fullscreen", None);
            fullscreen.connect_activate(clone!(@weak obj as window => move |_, _| {
                if window.is_fullscreened() {
                    window.unfullscreen();
                } else {
                    window.fullscreen();
                }
            }));
            obj.add_action(&fullscreen);
        }
    }

    impl WindowImpl for Window {
        // save window state on delete event
        fn close_request(&self, obj: &Self::Type) -> Inhibit {
            if let Err(err) = obj.save_window_size() {
                warn!("Failed to save window state, {}", &err);
            }
            Inhibit(false)
        }
    }

    impl WidgetImpl for Window {}
    impl ApplicationWindowImpl for Window {}
    impl AdwApplicationWindowImpl for Window {}
}

glib::wrapper! {
    pub struct Window(ObjectSubclass<imp::Window>)
        @extends gtk::Widget, gtk::Window, gtk::Root, gtk::ApplicationWindow, adw::ApplicationWindow, @implements gio::ActionMap, gio::ActionGroup;
}

impl Window {
    pub fn new(app: &Application) -> Self {
        glib::Object::new(&[("application", &Some(app)), ("icon-name", &Some(APP_ID))])
            .expect("Failed to create Window")
    }

    fn add_session(&self, session: &Session) {
        let priv_ = &imp::Window::from_instance(self);
        session.set_logged_in_users(&priv_.sessions.pages());
        priv_.sessions.add_child(session);
        priv_.sessions.set_visible_child(session);
        // We need to grab the focus so that keyboard shortcuts work
        session.grab_focus();

        session.connect_logged_out(clone!(@weak self as obj => move |session| {
            obj.remove_session(session)
        }));
    }

    fn remove_session(&self, session: &Session) {
        let priv_ = imp::Window::from_instance(self);

        priv_.sessions.remove(session);

        if let Some(child) = priv_.sessions.first_child() {
            priv_.sessions.set_visible_child(&child);
        } else {
            self.switch_to_login_page(false);
        }
    }

    fn restore_sessions(&self) {
        match secret::restore_sessions() {
            Ok(sessions) => {
                for stored_session in sessions {
                    let session = Session::new();
                    session.login_with_previous_session(stored_session);
                    self.add_session(&session);
                    self.switch_to_sessions_page();
                }
            }
            Err(error) => {
                warn!("Failed to restore previous sessions: {:?}", error);
                let error_string = error.to_user_facing();
                self.append_error(&Error::new(move |_| {
                    let error_label = gtk::Label::builder()
                        .label(
                            &(gettext("Unable to restore previous sessions")
                                + ": "
                                + &error_string),
                        )
                        .wrap(true)
                        .build();
                    Some(error_label.upcast())
                }));
            }
        }
    }

    pub fn save_window_size(&self) -> Result<(), glib::BoolError> {
        let settings = Application::default().settings();

        let size = self.default_size();

        settings.set_int("window-width", size.0)?;
        settings.set_int("window-height", size.1)?;

        settings.set_boolean("is-maximized", self.is_maximized())?;

        Ok(())
    }

    fn load_window_size(&self) {
        let settings = Application::default().settings();

        let width = settings.int("window-width");
        let height = settings.int("window-height");
        let is_maximized = settings.boolean("is-maximized");

        self.set_default_size(width, height);
        self.set_property("maximized", &is_maximized);
    }

    /// Change the default widget of the window based on the visible child
    /// If the login screen is visible, its login button becomes the default widget
    fn set_default_by_child(&self) {
        let priv_ = imp::Window::from_instance(self);
        if priv_.main_stack.visible_child() == Some(priv_.login.get().upcast()) {
            self.set_default_widget(Some(&priv_.login.default_widget()));
        } else {
            self.set_default_widget(gtk::Widget::NONE);
        }
    }

    pub fn switch_to_sessions_page(&self) {
        let priv_ = imp::Window::from_instance(self);
        priv_.main_stack.set_visible_child(&priv_.sessions.get());
    }

    pub fn switch_to_login_page(&self, clean: bool) {
        let priv_ = imp::Window::from_instance(self);
        if clean {
            priv_.login.clean();
        }
        priv_
            .login
            .show_back_to_session_button(priv_.sessions.get().pages().n_items() > 0);
        priv_.main_stack.set_visible_child(&*priv_.login);
    }

    /// This appends a new error to the list of errors
    pub fn append_error(&self, error: &Error) {
        let priv_ = imp::Window::from_instance(self);
        priv_.error_list.append(error);
    }
}
