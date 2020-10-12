use gettextrs::{bindtextdomain, setlocale, textdomain, LocaleCategory};
use gio::prelude::*;
use glib::clone;
use gtk::prelude::*;
use lazy_static::lazy_static;
use libhandy::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime as TokioRuntime;

use log::error;

use crate::appop::AppOp;

use crate::actions;
use crate::config;
use crate::uibuilder;
use crate::widgets;

mod connect;
mod windowstate;

use windowstate::WindowState;

type GlobalAppOp = Arc<Mutex<AppOp>>;
pub type UpdateApp = Box<dyn FnOnce(&mut AppOp)>;

static mut APP_TX: Option<glib::Sender<UpdateApp>> = None;
// TODO: Deprecated. It should be removed
static mut OP: Option<GlobalAppOp> = None;

lazy_static! {
    pub static ref RUNTIME: TokioRuntime = TokioRuntime::new().unwrap();
}

#[macro_export]
macro_rules! APPOP {
    ($fn: ident, ($($x:ident),*) ) => {{
        $( let $x = $x.clone(); )*
        let _ = crate::app::get_app_tx().send(Box::new(move |op| {
            crate::appop::AppOp::$fn(op, $($x),*);
        }));
    }};
    ($fn: ident) => {{
        APPOP!($fn, ( ) );
    }}
}

// Our application struct for containing all the state we have to carry around.
// TODO: subclass gtk::Application once possible
pub struct App {
    main_window: libhandy::ApplicationWindow,
    /* Add widget directly here in place of uibuilder::UI*/
    ui: uibuilder::UI,
}

pub type AppRef = Rc<App>;

impl App {
    pub fn new(gtk_app: &gtk::Application) -> AppRef {
        // Set up the textdomain for gettext
        setlocale(LocaleCategory::LcAll, "");
        bindtextdomain("fractal", config::LOCALEDIR);
        textdomain("fractal");

        glib::set_application_name("fractal");
        glib::set_prgname(Some("fractal"));

        // Add style provider
        let provider = gtk::CssProvider::new();
        provider.load_from_resource("/org/gnome/Fractal/app.css");
        gtk::StyleContext::add_provider_for_screen(
            &gdk::Screen::get_default().expect("Error initializing gtk css provider."),
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        let (app_tx, app_rx) = glib::MainContext::channel(Default::default());
        let ui = uibuilder::UI::new();
        let op = AppOp::new(ui.clone(), app_tx.clone());

        unsafe {
            OP = Some(Arc::new(Mutex::new(op)));
            APP_TX = Some(app_tx);
        }

        let op = get_op();
        app_rx.attach(None, move |update_op: UpdateApp| {
            update_op(&mut op.lock().unwrap());

            glib::Continue(true)
        });

        let window: libhandy::ApplicationWindow = ui
            .builder
            .get_object("main_window")
            .expect("Couldn't find main_window in ui file.");
        window.set_application(Some(gtk_app));

        window.set_title("Fractal");

        let settings: gio::Settings = gio::Settings::new("org.gnome.Fractal");
        let window_state = WindowState::load_from_gsettings(&settings);
        window.set_default_size(window_state.width, window_state.height);
        if window_state.is_maximized {
            window.maximize();
        } else if window_state.x > 0 && window_state.y > 0 {
            window.move_(window_state.x, window_state.y);
        }
        window.show_all();

        if gtk_app
            .get_application_id()
            .map_or(false, |s| s.ends_with("Devel"))
        {
            window.get_style_context().add_class("devel");
        }

        let leaflet = ui
            .builder
            .get_object::<libhandy::Leaflet>("chat_page")
            .expect("Can't find chat_page in ui file.");
        let container = ui
            .builder
            .get_object::<gtk::Box>("history_container")
            .expect("Can't find history_container in ui file.");
        let popover = ui
            .builder
            .get_object::<gtk::Popover>("autocomplete_popover")
            .expect("Can't find autocomplete_popover in ui file.");

        if leaflet.get_folded() {
            container.get_style_context().add_class("folded-history");
            popover.get_style_context().add_class("narrow");
        }

        leaflet.connect_property_folded_notify(clone!(@weak container => move |leaflet| {
            if leaflet.get_folded() {
                container.get_style_context().add_class("folded-history");
                popover.get_style_context().add_class("narrow");
            } else {
                container.get_style_context().remove_class("folded-history");
                popover.get_style_context().remove_class("narrow");
            }
        }));

        let view_stack = ui
            .builder
            .get_object::<gtk::Stack>("subview_stack")
            .expect("Can't find subview_stack in ui file.");

        /* Add account settings view to the view stack */
        let child = ui
            .builder
            .get_object::<gtk::Box>("account_settings_box")
            .expect("Can't find account_settings_box in ui file.");
        view_stack.add_named(&child, "account-settings");

        let main_stack = ui
            .builder
            .get_object::<gtk::Stack>("main_content_stack")
            .expect("Can't find main_content_stack in ui file.");

        // Add login view to the main stack
        let login = widgets::LoginWidget::new(get_op());
        main_stack.add_named(&login.container, "login");

        gtk_app.set_accels_for_action("login.back", &["Escape"]);

        actions::Global::new(gtk_app, get_app_tx().clone(), get_op());

        let app = AppRef::new(Self {
            main_window: window,
            ui,
        });

        app.connect_gtk();

        app
    }

    pub fn on_startup(gtk_app: &gtk::Application) {
        // Create application.
        let app = App::new(gtk_app);

        // Initialize libhandy
        libhandy::init();

        gtk_app.connect_activate(clone!(@weak app => move |_| {
            app.on_activate();
        }));

        app.main_window
            .connect_property_has_toplevel_focus_notify(clone!(@weak app => move |_| {
                get_op().lock().unwrap().mark_active_room_messages();
            }));

        app.main_window.connect_delete_event(move |window, _| {
            let settings: gio::Settings = gio::Settings::new("org.gnome.Fractal");
            let w = window.upcast_ref();
            let window_state = WindowState::from_window(w);
            if let Err(err) = window_state.save_in_gsettings(&settings) {
                error!("Can't save the window settings: {:?}", err);
            }
            Inhibit(false)
        });

        get_op().lock().unwrap().init();

        // When the application is shut down we drop our app struct
        let app_container = RefCell::new(Some(app));
        gtk_app.connect_shutdown(move |_| {
            let app = app_container
                .borrow_mut()
                .take()
                .expect("Shutdown called multiple times");
            app.on_shutdown();
        });
    }

    fn on_activate(&self) {
        self.main_window.show();
        self.main_window.present()
    }

    fn on_shutdown(self: AppRef) {
        get_op().lock().unwrap().quit();
    }
}

// TODO: Deprecated. It should be removed
pub(self) fn get_op() -> &'static GlobalAppOp {
    unsafe { OP.as_ref().expect("Fatal: AppOp has not been initialized") }
}

pub fn get_app_tx() -> &'static glib::Sender<UpdateApp> {
    unsafe {
        APP_TX
            .as_ref()
            .expect("Fatal: AppRuntime has not been initialized")
    }
}
