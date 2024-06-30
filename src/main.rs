#![feature(exact_size_is_empty)]
#![feature(duration_millis_float)]
//#![windows_subsystem = "windows"]

use log::{debug, error, info, log_enabled, Level};
use models::Profile;
use raw_window_handle::{
    HasRawDisplayHandle, HasRawWindowHandle, HasWindowHandle, RawWindowHandle,
};
use ui::ConnectWindowBuilder;

use std::{
 cell::RefCell, panic, process, rc::Rc, sync::{Arc, LazyLock, Mutex, Weak}
};

use i_slint_backend_winit::{
    winit::{
        dpi::{PhysicalPosition, PhysicalSize},
        window::{Fullscreen, Icon},
    },
    WinitWindowAccessor,
};

use i_slint_core::lengths::LogicalRect;
use session::Session;
use slint::{platform::WindowEvent, ComponentHandle, LogicalPosition, VecModel};
use tokio::runtime::Builder;

#[macro_use]
extern crate log;

slint::include_modules!();

pub static TOKIO: std::sync::LazyLock<tokio::runtime::Runtime> =
    std::sync::LazyLock::new(|| Builder::new_multi_thread().enable_all().build().unwrap());

mod hotkey;
pub mod models;
mod script_runtime;
pub mod session;
mod trigger;
mod ui;

use smudgy_connect_window::ConnectWindow;

fn main() {
    if let Err(_) = std::env::var("SMUDGY_LOG") {
        // This is only unsafe because it isn't thread-safe; no other threads have spawned yet.
        unsafe { std::env::set_var("SMUDGY_LOG", "debug,smudgy=trace"); }
    }

    pretty_env_logger::init_custom_env("SMUDGY_LOG");

    info!(
        "smudgy started; version {} ({}, built on {})",
        env!("SMUDGY_BUILD_NAME"),
        env!("CARGO_PKG_VERSION"),
        build_time::build_time_local!("%Y-%m-%d %H:%M:%S")
    );

    deno_core::JsRuntime::init_platform(None);
    trace!("deno initialized, v8 version {}", deno_core::v8_version());

    LazyLock::force(&TOKIO);
    trace!(
        "tokio runtime started, {} task workers running",
        TOKIO.metrics().num_workers()
    );

    let platform = Box::new(
        i_slint_backend_winit::Backend::new_with_renderer_by_name(Some("skia-opengl")).unwrap(),
    );

    slint::platform::set_platform(platform).unwrap();

    let ui: MainWindow = MainWindow::new().unwrap();

    let sessions: Rc<RefCell<Vec<Arc<Mutex<Session>>>>> = Rc::new(RefCell::new(Vec::new()));
    let sessions_model = Rc::new(VecModel::default());

    let connect_window: ConnectWindow =
        ConnectWindowBuilder::build(ui.as_weak(), sessions.clone(), sessions_model.clone());

    ui.set_sessions(sessions_model.clone().into());

    let weak_window = ui.as_weak();
    ui.on_toolbar_fullscreen_clicked(move || {
        let ui = weak_window.upgrade().unwrap();
        ui.window().with_winit_window(|window| {
            if let Some(_) = window.fullscreen() {
                window.set_fullscreen(None);
                window.set_decorations(true);
                ui.set_is_full_screen(false);
            } else {
                window.set_fullscreen(Some(Fullscreen::Borderless(None)));
                ui.set_is_full_screen(true);
            }
        });
    });

    let weak_window = ui.as_weak();
    ui.on_drag_window(move || {
        weak_window
            .upgrade()
            .unwrap()
            .window()
            .with_winit_window(|window| {
                window.drag_window().unwrap();
            });
    });

    ui.on_toolbar_close_clicked(|| {
        process::exit(0);
    });

    let weak_window = ui.as_weak();
    let ui_connect = connect_window.as_weak();
    ui.on_toolbar_create_session_clicked(move || {
        let connect = ui_connect.upgrade().unwrap();
        let window = weak_window.upgrade().unwrap();

        let connect_size = connect.window().size();
        let main_pos = window.window().position();
        let main_size = window.window().size();
        let connect_pos_phx = slint::PhysicalPosition::new(
            std::cmp::max(
                0,
                main_pos.x + (main_size.width as i32 / 2 - connect_size.width as i32 / 2),
            ),
            std::cmp::max(
                0,
                main_pos.y + (main_size.height as i32 / 2 - 400),
            ),
        );

        connect.window().set_position(connect_pos_phx);
        connect.invoke_refresh_profiles();
        connect.show().unwrap();
    });

    let ui_sessions = sessions.clone();
    ui.on_session_accepted(move |session_index: i32, line| {
        let sessions = ui_sessions.borrow_mut();
        let to_invoke = sessions[session_index as usize].clone();
        let mut guard = to_invoke.lock().unwrap();
        guard.on_session_accepted(line.as_str());
    });

    let ui_sessions = Rc::clone(&sessions);
    ui.on_request_autocomplete(
        move |session_index, line, continue_from_last_request| -> AutocompleteResult {
            let sessions = ui_sessions.borrow_mut();
            let to_invoke = sessions[session_index as usize].clone();
            let mut guard = to_invoke.lock().unwrap();
            guard.on_request_autocomplete(line.as_str(), continue_from_last_request)
        },
    );

    let ui_sessions = Rc::clone(&sessions);

    ui.on_session_key_pressed(
        move |session_index, ev, input_line| -> SessionKeyPressResponse {
            let sessions = ui_sessions.borrow_mut();
            let to_invoke = sessions[session_index as usize].clone();
            let mut guard = to_invoke.lock().unwrap();
            guard.on_key_pressed(ev, input_line.as_str())
        },
    );

    let ui_sessions = Rc::clone(&sessions);
    ui.on_session_scrollbar_value_changed(move |session_index, value| {
        let sessions = ui_sessions.borrow_mut();
        let to_invoke = sessions[session_index as usize].clone();
        let guard = to_invoke.lock().unwrap();
        guard.view().set_scroll_position(value);
    });

    let ui_sessions = sessions.clone();
    let weak_window = ui.as_weak();

    ui.window()
        .set_rendering_notifier(move |state, _graphics_api| match state {
            slint::RenderingState::BeforeRendering => {
                let window = weak_window.upgrade().unwrap();

                if !window.window().is_visible() {
                    return;
                }

                let sessions = ui_sessions.borrow();

                if !sessions.is_empty() {
                    let size_hints = window.invoke_get_physical_terminal_area_dimensions();
                    window.window().with_winit_window(|window| {
                        let window_size = window.inner_size();

                        let terminal_height = std::cmp::max(0, window_size.height
                            - (size_hints.terminal_padding * 3.5
                                + size_hints.terminal_spacing
                                + size_hints.editor_area_height)
                                as u32);
                        let terminal_width = (window_size.width
                            - (size_hints.terminal_padding * 2.0) as u32) / sessions.len() as u32 - size_hints.terminal_spacing as u32 - size_hints.terminal_scrollbar_width as u32;

                        for session in sessions.iter() {
                            let session_guard = session.lock().unwrap();
                            session_guard.prepare_render(terminal_width, terminal_height);
                        }
                    });
                }
            }
            slint::RenderingState::AfterRendering => {}
            _ => {}
        })
        .unwrap();

        let ui_sessions = Rc::clone(&sessions);
        ui.on_refresh_terminal(move |session_index: i32| {
            let sessions = ui_sessions.borrow();
            let to_refresh = sessions[session_index as usize].clone();
            let guard = to_refresh.lock().unwrap();
            guard.view().handle_incoming_lines();
        });
    

    let ui_sessions = Rc::clone(&sessions);
    let ui_sessions_model = Rc::clone(&sessions_model);
    ui.on_session_close_clicked(move |session_index: i32| {
        let mut sessions = RefCell::borrow_mut(&ui_sessions);
        let session = sessions.remove(session_index as usize);
        session.lock().unwrap().close();
        ui_sessions_model.remove(session_index as usize);
    });

    let ui_sessions = Rc::clone(&sessions);
    ui.on_session_reconnect_clicked(move |session_index: i32| {
        let sessions = ui_sessions.borrow();
        let session = sessions[session_index as usize].clone();
        let mut guard = session.lock().unwrap();
        guard.connect();
});
    
    ui.show().unwrap();
    trace!("Starting ui event loop...");
    slint::run_event_loop().unwrap();
    ui.hide().unwrap();
}
