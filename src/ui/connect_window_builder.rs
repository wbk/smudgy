use std::{
    borrow::Borrow,
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex},
};

use anyhow::Context;
use i_slint_backend_winit::winit::event;
use slint::{ComponentHandle, Model};
use slint::{VecModel, Weak};
use smudgy_connect_window::{ConnectWindow, UiResult};

use crate::{
    models::{Character, Profile, ProfileData},
    session::Session,
    MainWindow, SessionState,
};

pub struct ConnectWindowBuilder {}

impl ConnectWindowBuilder {
    pub fn build(
        main_window: Weak<MainWindow>,
        sessions: Rc<RefCell<Vec<Arc<Mutex<Session>>>>>,
        sessions_model: Rc<VecModel<SessionState>>,
    ) -> ConnectWindow {
        let window = ConnectWindow::new().unwrap();

        let event_connect_window = window.as_weak();
        window.on_refresh_profiles(move || {
            let profiles: Rc<VecModel<_>> = Rc::new(
                Profile::iter_all()
                    .map(|profile| profile.into())
                    .collect::<Vec<_>>()
                    .into(),
            );
            event_connect_window
                .upgrade()
                .unwrap()
                .set_profiles(profiles.into());
        });

        let event_connect_window = window.as_weak();
        window.on_create_profile(move |params| {
            match Profile::new(ProfileData::from(params.clone())).map(|profile| profile.save()) {
                Ok(Ok(_)) => {
                    event_connect_window.upgrade().map(|window| {
                        window.invoke_refresh_profiles();

                        let profiles: Rc<VecModel<_>> = Rc::new(
                            Profile::iter_all()
                                .map(|profile| profile.into())
                                .collect::<Vec<smudgy_connect_window::Profile>>()
                                .into(),
                        );

                        profiles
                            .iter()
                            .enumerate()
                            .find(|(_, profile)| profile.name == params.name)
                            .map(|(index, _)| {
                                window.set_profiles(profiles.into());
                                window.invoke_set_selected_profile_idx(index as i32);
                            });
                    });
                    smudgy_connect_window::UiResult {
                        success: true,
                        message: "".into(),
                    }
                }
                Ok(Err(e)) | Err(e) => smudgy_connect_window::UiResult {
                    success: false,
                    message: e.to_string().into(),
                },
            }
        });

        let event_sessions = sessions.clone();
        let event_sessions_model = sessions_model.clone();
        let event_main_window = main_window.clone();
        let event_connect_window = window.as_weak();
        window.on_connect_clicked(move |profile, character| {
            let mut sessions = event_sessions.borrow_mut();
            let new_session_id = sessions.len() as i32;

            let session_name = format!("{} - {}", character.name, character.name);

            let profile = Rc::new(Profile::try_from(ProfileData::from(profile)).unwrap());
            let character = Character::load(character.name.as_str(), Rc::downgrade(&profile))
                .context("Error loading character from file")
                .unwrap();
            character.touch();

            let session = Arc::new(Mutex::new(Session::new(
                new_session_id,
                event_main_window.clone(),
                Rc::into_inner(profile).unwrap(),
            )));

            sessions.push(session.clone());

            let mut session_guard = session.lock().unwrap();

            let session_state = SessionState {
                name: session_name.into(),
                buffer: session_guard.view().into(),
                scrollback_size: session_guard.view().row_count_model().into(),
            };
            event_sessions_model.push(session_state);

            session_guard.connect();

            event_main_window
                .upgrade()
                .unwrap()
                .invoke_set_toolbar_show(false);
            event_connect_window.upgrade().unwrap().hide().unwrap();
        });

        window.on_save_character(move |_profile, _character| {
            UiResult {
                success: false,
                message: "unimplemented".into(),
            }
        });
        window
    }
}


fn on_save_character(_profile: Profile, character: Character) -> UiResult {
    UiResult {
        success: false,
        message: "unimplemented".into(),
    }
}