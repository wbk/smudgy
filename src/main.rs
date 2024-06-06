#![feature(exact_size_is_empty)]
#![feature(duration_millis_float)]

use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, LazyLock, Mutex},
};

use i_slint_backend_winit::{winit::{dpi::{PhysicalPosition, PhysicalSize}, window::{Fullscreen, Icon}}, WinitWindowAccessor};

#[cfg(target_os = "windows")]
use i_slint_backend_winit::winit::platform::windows::WindowExtWindows;

use i_slint_core::lengths::LogicalRect;
use session::{Profile, Session};
use slint::{VecModel};
use tokio::runtime::Builder;

slint::include_modules!();

pub static TOKIO: std::sync::LazyLock<tokio::runtime::Runtime> =
    std::sync::LazyLock::new(|| Builder::new_multi_thread().enable_all().build().unwrap());

mod hotkey;
mod script_runtime;
pub mod session;
mod trigger;

#[cfg(target_os = "windows")]
fn set_taskbar_icon(window: &i_slint_backend_winit::winit::window::Window, icon: Option<Icon>) {
    window.set_taskbar_icon(icon);
}

#[cfg(not(target_os = "windows"))]
fn set_taskbar_icon(_window: &i_slint_backend_winit::winit::window::Window, _icon: Option<Icon>) {}

fn main() {
    deno_core::JsRuntime::init_platform(None);
    LazyLock::force(&TOKIO);

    slint::platform::set_platform(Box::new(
        i_slint_backend_winit::Backend::new_with_renderer_by_name(Some("skia-opengl")).unwrap(),
    ))
    .unwrap();

    let ui = MainWindow::new().unwrap();
    let sessions: Rc<RefCell<Vec<Arc<Mutex<Session>>>>> = Rc::new(RefCell::new(Vec::new()));
    let sessions_model = Rc::new(VecModel::default());

    ui.set_sessions(sessions_model.clone().into());

    // ui.window().with_winit_window(|window| {
    //     let pixmap =
    //         tiny_skia::Pixmap::decode_png(include_bytes!("../assets/icon256.png")).unwrap();
    //     let icon = Icon::from_rgba(pixmap.data().into(), 256, 256).unwrap();

    //     set_taskbar_icon(&window, Some(icon.clone()));

    //     window.set_window_icon(Some(icon));
    // });
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
        weak_window.upgrade().unwrap().window().with_winit_window(|window| {
            window.drag_window().unwrap();
        });
    });

    let ui_sessions = sessions.clone();
    let ui_sessions_model = sessions_model.clone();
    let weak_window = ui.as_weak();
    ui.on_toolbar_create_session_clicked(move || {
        let mut sessions = ui_sessions.borrow_mut();

        let new_session_id = sessions.len() as i32;

        let session = Arc::new(Mutex::new(Session::new(
            new_session_id,
            weak_window.clone(),
            Profile {
                host: "mud.arctic.org".to_string(),
                port: 2700,
                name: "Arctic".to_string(),
            },
        )));

        sessions.push(session.clone());

        let mut session_guard = session.lock().unwrap();

        let ui_session_state = SessionState {
            name: "Arctic".into(),
            buffer: session_guard.view().into(),
            scrollback_size: session_guard.view().row_count_model().into()
        };
        ui_sessions_model.push(ui_session_state);

        session_guard.connect();
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

    let ui_sessions = sessions.clone();
    let weak_window = ui.as_weak();

    ui.window()
        .set_rendering_notifier(move |state, _graphics_api| match state {
            slint::RenderingState::BeforeRendering => {
                let window = weak_window.upgrade().unwrap();
                let sessions = ui_sessions.borrow();

                if !sessions.is_empty() {
                    let size_hints = window.invoke_get_physical_terminal_area_dimensions();
                    let ui = weak_window.upgrade().unwrap();
                    
                    window.window().with_winit_window(|window| {
                        let window_size = window.inner_size();
                        
                        let terminal_height = window_size.height - (size_hints.terminal_padding * 2.0 + size_hints.terminal_spacing + size_hints.editor_area_height) as u32;
                        let terminal_width = window_size.width - (size_hints.terminal_padding * 2.0 + size_hints.terminal_spacing * (sessions.len() as f32) - size_hints.terminal_spacing) as u32 - size_hints.terminal_scrollbar_width as u32;

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
        let sessions = ui_sessions.borrow_mut();
        let to_refresh = sessions[session_index as usize].clone();
        let guard = to_refresh.lock().unwrap();
        guard.view().handle_incoming_lines();
    });

    ui.show().unwrap();
    slint::run_event_loop().unwrap();
    ui.hide().unwrap();
}
