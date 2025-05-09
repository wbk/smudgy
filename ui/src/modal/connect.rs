use iced::font::Family;

use iced::widget::{
    Column, Row, TextInput, button, column, container, horizontal_space, scrollable, text,
    text_editor,
};
use iced::{Alignment, Font, Length, Pixels, Task};
use log::warn;
use validator::Validate;

use crate::theme::Element;

// Keep core model imports
use smudgy_core::models::{
    profile::{Profile, ProfileConfig},
    server::{Server, ServerConfig},
};
use std::collections::HashMap;

// --- Module-specific types ---

pub type ServerName = String;
pub type ProfileName = String;

// Events emitted by this modal back to the main application
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    CloseModalRequested, // Renamed from Close for clarity
    Connect(ServerName, ProfileName),
}

// Messages handled internally by this modal's update logic
#[derive(Debug, Clone)]
pub enum Message {
    // Data Loading
    ServersLoaded(Result<Vec<Server>, String>),
    ProfilesLoaded(ServerName, Result<Vec<Profile>, String>),
    // UI Interaction
    SelectServer(ServerName),
    CloseRequested, // E.g., from Esc key or background click mapped by parent
    ConnectProfile(ServerName, ProfileName),
    // Server CRUD UI Actions
    RequestCreateServer,
    RequestEditServer(ServerName),
    RequestConfirmDeleteServer(ServerName), // User clicks delete in details view
    ConfirmDeleteServer(ServerName),        // User confirms deletion
    // Server Form Interaction
    UpdateServerFormField(ServerFormField, String),
    SubmitServerForm,
    CancelServerForm,
    // Server CRUD Async Results
    ServerCreated(Result<Server, String>),
    ServerUpdated(Result<Server, String>),
    ServerDeleted(Result<ServerName, String>), // Pass back name on success
    // --- Profile CRUD ---
    // UI Actions (act on selected_server)
    RequestCreateProfile,
    RequestEditProfile(ProfileName),
    RequestConfirmDeleteProfile(ProfileName),
    ConfirmDeleteProfile(ProfileName),
    // Form Interaction
    UpdateProfileFormField(ProfileFormField, String),
    UpdateProfileFormSendOnConnect(text_editor::Action),
    SubmitProfileForm,
    CancelProfileForm,
    // Async Results
    ProfileCreated(Result<smudgy_core::models::profile::Profile, String>),
    ProfileUpdated(Result<smudgy_core::models::profile::Profile, String>),
    ProfileDeleted(Result<(ServerName, ProfileName), String>), // Need both names for state update
}

/// Fields in the server create/edit form.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ServerFormField {
    Name, // Only for Create, maybe disable editing name later?
    Host,
    Port,
}

/// Fields in the profile create/edit form.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ProfileFormField {
    Name,
    Caption,
}

/// Temporary storage for server form input.
#[derive(Debug, Default)]
pub struct ServerConfigFormData {
    pub name: String,
    pub host: String,
    pub port: String,
}

/// Temporary storage for profile form input.
#[derive(Debug, Default)]
pub struct ProfileConfigFormData {
    pub name: String,
    pub caption: String,
}

/// Represents the current server-related action being performed (if any).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerCrudAction {
    Create,
    Edit(ServerName),          // Stores the original name for the update operation
    ConfirmDelete(ServerName), // Added for confirmation step
}

/// Represents the current profile-related action being performed (if any).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileCrudAction {
    Create,                     // Assumes context of state.selected_server
    Edit(ProfileName),          // Assumes context of state.selected_server
    ConfirmDelete(ProfileName), // Added for confirmation step
}

// State managed by this modal
#[derive(Debug)]
pub struct State {
    servers: Vec<Server>,
    profiles: HashMap<ServerName, Vec<Profile>>,
    selected_server: Option<ServerName>,
    is_loading_servers: bool,
    is_loading_profiles: Option<ServerName>,
    // --- Server CRUD State ---
    /// Tracks if we are currently creating or editing a server.
    server_action: Option<ServerCrudAction>,
    /// Holds the temporary data entered into the server form.
    server_form_data: ServerConfigFormData, // Use Default::default()
    /// Holds any error message related to server CRUD operations.
    server_crud_error: Option<String>,
    // TODO: Profile CRUD State
    // --- Profile CRUD State ---
    /// Tracks if we are currently creating or editing a profile.
    profile_action: Option<ProfileCrudAction>,
    /// Holds the temporary data entered into the profile form.
    profile_form_data: ProfileConfigFormData,
    profile_form_send_on_connect_content: text_editor::Content,
    /// Holds any error message related to profile CRUD operations.
    profile_crud_error: Option<String>,
}

impl Default for State {
    fn default() -> Self {
        State {
            servers: Vec::new(),
            profiles: HashMap::new(),
            selected_server: None,
            is_loading_servers: false, // Load triggered by update
            is_loading_profiles: None,
            server_action: None,
            server_form_data: ServerConfigFormData::default(),
            server_crud_error: None,
            profile_action: None,
            profile_form_data: ProfileConfigFormData::default(),
            profile_form_send_on_connect_content: text_editor::Content::with_text(""),
            profile_crud_error: None,
        }
    }
}

// --- Async Loaders ---

// --- Server CRUD Async Wrappers ---

async fn create_server_async(name: String, config: ServerConfig) -> Result<Server, String> {
    smudgy_core::models::server::create_server(&name, config) // Pass name explicitly
        .map_err(|e| e.to_string())
}

async fn update_server_async(name: String, config: ServerConfig) -> Result<Server, String> {
    smudgy_core::models::server::update_server(&name, config).map_err(|e| e.to_string())
}

async fn delete_server_async(name: String) -> Result<String, String> {
    smudgy_core::models::server::delete_server(&name)
        .map(|_| name) // Return the name on success for state update
        .map_err(|e| e.to_string())
}

// --- Profile CRUD Async Wrappers ---

async fn create_profile_async(
    server_name: String,
    profile_name: String,
    config: smudgy_core::models::profile::ProfileConfig,
) -> Result<smudgy_core::models::profile::Profile, String> {
    smudgy_core::models::profile::create_profile(&server_name, &profile_name, config)
        .map_err(|e| e.to_string())
}

async fn update_profile_async(
    server_name: String,
    profile_name: String,
    config: smudgy_core::models::profile::ProfileConfig,
) -> Result<smudgy_core::models::profile::Profile, String> {
    smudgy_core::models::profile::update_profile(&server_name, &profile_name, config)
        .map_err(|e| e.to_string())
}

async fn delete_profile_async(
    server_name: String,
    profile_name: String,
) -> Result<(ServerName, ProfileName), String> {
    smudgy_core::models::profile::delete_profile(&server_name, &profile_name)
        .map(|_| (server_name, profile_name)) // Return tuple on success
        .map_err(|e| e.to_string())
}

// --- Profile Loaders ---

pub(super) async fn load_servers_async() -> Result<Vec<Server>, String> {
    smudgy_core::models::server::list_servers().map_err(|e| e.to_string())
}

async fn load_profiles_async(server_name: String) -> Result<Vec<Profile>, String> {
    smudgy_core::models::profile::list_profiles(&server_name).map_err(|e| e.to_string())
}

// --- Update Logic ---

/// Helper function to handle server form submission.
fn handle_submit_server_form(state: &mut State) -> Task<Message> {
    state.server_crud_error = None; // Clear previous error

    match state.server_action.clone() {
        // Clone needed for async task
        Some(ServerCrudAction::Create) => {
            let port = match state.server_form_data.port.trim().parse::<u16>() {
                Ok(p) => p,
                Err(_) => {
                    state.server_crud_error =
                        Some("Invalid port number. Must be between 1 and 65535.".to_string());
                    return Task::none();
                }
            };
            let config = ServerConfig {
                host: state.server_form_data.host.trim().to_string(),
                port,
            };
            if let Err(e) = config.validate() {
                state.server_crud_error = Some(format!("Configuration error: {e}"));
                return Task::none();
            }
            let name = state.server_form_data.name.trim().to_string();
            if name.is_empty() {
                state.server_crud_error = Some("Server name cannot be empty.".to_string());
                return Task::none();
            }
            Task::perform(create_server_async(name, config), Message::ServerCreated)
        }
        Some(ServerCrudAction::Edit(original_name)) => {
            let port = match state.server_form_data.port.trim().parse::<u16>() {
                Ok(p) => p,
                Err(_) => {
                    state.server_crud_error =
                        Some("Invalid port number. Must be between 1 and 65535.".to_string());
                    return Task::none();
                }
            };
            let config = ServerConfig {
                host: state.server_form_data.host.trim().to_string(),
                port,
            };
            if let Err(e) = config.validate() {
                state.server_crud_error = Some(format!("Configuration error: {e}"));
                return Task::none();
            }
            Task::perform(
                update_server_async(original_name.clone(), config),
                Message::ServerUpdated,
            )
        }
        Some(ServerCrudAction::ConfirmDelete(_)) => {
            warn!("Error: SubmitServerForm called during ConfirmDelete state.");
            state.server_crud_error =
                Some("Unexpected error: Cannot submit while confirming delete.".to_string());
            Task::none()
        }
        None => {
            warn!("Error: SubmitServerForm called without a ServerCrudAction set.");
            state.server_crud_error = Some("Unexpected error: No action in progress.".to_string());
            Task::none()
        }
    }
}

/// Helper function to handle profile form submission.
fn handle_submit_profile_form(state: &mut State) -> Task<Message> {
    state.profile_crud_error = None; // Clear previous error

    let server_name = if let Some(name) = state.selected_server.clone() {
        name
    } else {
        warn!("Error: SubmitProfileForm called without a server selected.");
        state.profile_crud_error = Some("Error: No server selected.".to_string());
        return Task::none();
    };

    match state.profile_action.clone() {
        Some(ProfileCrudAction::Create) => {
            let config = ProfileConfig {
                caption: state.profile_form_data.caption.trim().to_string(),
                send_on_connect: state.profile_form_send_on_connect_content.text(),
            };
            if let Err(e) = config.validate() {
                state.profile_crud_error = Some(format!("Configuration error: {e}"));
                return Task::none();
            }
            let profile_name = state.profile_form_data.name.trim().to_string();
            if profile_name.is_empty() {
                state.profile_crud_error = Some("Profile name cannot be empty.".to_string());
                return Task::none();
            }
            Task::perform(
                create_profile_async(server_name, profile_name, config),
                Message::ProfileCreated,
            )
        }
        Some(ProfileCrudAction::Edit(original_profile_name)) => {
            let config = ProfileConfig {
                caption: state.profile_form_data.caption.trim().to_string(),
                send_on_connect: state.profile_form_send_on_connect_content.text(),
            };
            if let Err(e) = config.validate() {
                state.profile_crud_error = Some(format!("Configuration error: {e}"));
                return Task::none();
            }
            Task::perform(
                update_profile_async(server_name, original_profile_name, config),
                Message::ProfileUpdated,
            )
        }
        Some(ProfileCrudAction::ConfirmDelete(_)) => {
            warn!("Error: SubmitProfileForm called during ConfirmDelete state.");
            state.profile_crud_error =
                Some("Unexpected error: Cannot submit while confirming delete.".to_string());
            Task::none()
        }
        None => {
            warn!("Error: SubmitProfileForm called without a profile action set.");
            state.profile_crud_error = Some("Unexpected error: No action in progress.".to_string());
            Task::none()
        }
    }
}

/// Handles messages specific to the Connect Modal logic.
pub fn update(state: &mut State, message: Message) -> (Task<Message>, Option<Event>) {
    let mut task = Task::none();
    let mut event = None;

    // Clear server CRUD error on most actions unless explicitly set
    if !matches!(
        message,
        Message::SubmitServerForm
            | Message::ServerCreated(_)
            | Message::ServerUpdated(_)
            | Message::ServerDeleted(_)
    ) {
        state.server_crud_error = None;
    }

    match message {
        Message::ServersLoaded(Ok(servers)) => {
            state.is_loading_servers = false;
            state.servers = servers;
            state.profiles.clear();
            // If a server was being edited/created, cancel that action
            state.server_action = None;
            state.server_form_data = ServerConfigFormData::default();
            state.server_crud_error = None;

            if let Some(first_server) = state.servers.first() {
                let server_name_clone = first_server.name.clone();
                state.selected_server = Some(server_name_clone.clone());
                state.is_loading_profiles = Some(server_name_clone.clone());
                task = Task::perform(
                    load_profiles_async(server_name_clone.clone()),
                    move |result| {
                        let name = server_name_clone.clone();
                        Message::ProfilesLoaded(name, result)
                    },
                );
            } else {
                state.selected_server = None;
                state.is_loading_profiles = None; // Ensure profiles aren't loading if no server selected
            }
        }
        Message::ServersLoaded(Err(e)) => {
            state.is_loading_servers = false;
            let err_msg = format!("Error loading servers: {e}");
            warn!("{err_msg}");
            state.server_crud_error = Some(err_msg); // Display error to user
        }
        Message::ProfilesLoaded(server_name, Ok(mut profiles)) => {
            // Add mut for sorting
            if state.is_loading_profiles.as_ref() == Some(&server_name) {
                state.is_loading_profiles = None;
            }
            // Sort profiles by name for consistent display
            profiles.sort_by(|a, b| a.name.cmp(&b.name));
            state.profiles.insert(server_name, profiles);
        }
        Message::ProfilesLoaded(server_name, Err(e)) => {
            if state.is_loading_profiles.as_ref() == Some(&server_name) {
                state.is_loading_profiles = None;
            }
            let err_msg = format!("Error loading profiles for '{server_name}': {e}");
            warn!("{err_msg}");
            state.profile_crud_error = Some(err_msg); // Display error to user
        }
        Message::SelectServer(server_name) => {
            if state.selected_server.as_ref() != Some(&server_name) {
                let server_name_clone = server_name.clone();
                state.selected_server = Some(server_name_clone.clone());
                if !state.profiles.contains_key(&server_name_clone) {
                    state.is_loading_profiles = Some(server_name_clone.clone());
                    task = Task::perform(
                        load_profiles_async(server_name_clone.clone()),
                        move |result| {
                            let name = server_name_clone.clone();
                            Message::ProfilesLoaded(name, result)
                        },
                    );
                }
            }
        }
        Message::CloseRequested => {
            event = Some(Event::CloseModalRequested);
        }
        Message::ConnectProfile(server_name, profile_name) => {
            // Cancel any ongoing server CRUD action if user connects
            state.server_action = None;
            state.server_form_data = ServerConfigFormData::default();
            state.server_crud_error = None;
            event = Some(Event::Connect(server_name, profile_name));
        }
        Message::RequestCreateServer => {
            state.server_action = Some(ServerCrudAction::Create);
            state.server_form_data = ServerConfigFormData::default(); // Clear form
            state.server_crud_error = None;
            state.selected_server = None; // De-select server when opening create form
            state.is_loading_profiles = None; // Cancel profile load
        }
        Message::RequestEditServer(server_name) => {
            if let Some(server_to_edit) = state.servers.iter().find(|s| s.name == server_name) {
                state.server_action = Some(ServerCrudAction::Edit(server_name.clone()));
                state.server_form_data = ServerConfigFormData {
                    name: server_to_edit.name.clone(), // Pre-fill name (though not directly editable usually)
                    host: server_to_edit.config.host.clone(),
                    port: server_to_edit.config.port.to_string(),
                };
                state.server_crud_error = None;
                state.selected_server = Some(server_name); // Ensure server remains selected
                state.is_loading_profiles = None; // Cancel profile load
            } else {
                warn!("Error: Requested to edit non-existent server '{server_name}'");
            }
        }
        Message::RequestConfirmDeleteServer(server_name) => {
            state.server_action = Some(ServerCrudAction::ConfirmDelete(server_name));
            state.server_crud_error = None;
            state.profile_action = None; // Ensure profile form is hidden
        }
        Message::ConfirmDeleteServer(server_name) => {
            state.server_crud_error = None;
            task = Task::perform(delete_server_async(server_name), Message::ServerDeleted);
            // The state.server_action remains ConfirmDelete until ServerDeleted result arrives.
        }
        Message::UpdateServerFormField(field, value) => {
            // Only update if in Create or Edit mode
            if matches!(
                state.server_action,
                Some(ServerCrudAction::Create) | Some(ServerCrudAction::Edit(_))
            ) {
                match field {
                    ServerFormField::Name => state.server_form_data.name = value,
                    ServerFormField::Host => state.server_form_data.host = value,
                    ServerFormField::Port => state.server_form_data.port = value,
                }
                state.server_crud_error = None; // Clear error when user types
            }
        }
        Message::SubmitServerForm => {
            task = handle_submit_server_form(state);
        }
        Message::CancelServerForm => {
            // Clear action, form data, and error regardless of previous state
            state.server_action = None;
            state.server_form_data = ServerConfigFormData::default();
            state.server_crud_error = None;
            // If a server was selected before opening the form (e.g., for Edit or ConfirmDelete),
            // we don't explicitly re-select it here. The user can click it again in the list.
            // This keeps the cancellation logic simple.
        }
        Message::ServerCreated(result) => {
            match result {
                Ok(new_server) => {
                    state.server_action = None;
                    state.server_form_data = ServerConfigFormData::default();
                    state.server_crud_error = None;

                    // Add to list and sort (optional, but good for UI)
                    state.servers.push(new_server.clone());
                    state.servers.sort_by(|a, b| a.name.cmp(&b.name));

                    // Select the new server and trigger profile load
                    let server_name_clone = new_server.name.clone();
                    state.selected_server = Some(server_name_clone.clone());
                    state.is_loading_profiles = Some(server_name_clone.clone());
                    task =
                        Task::perform(load_profiles_async(server_name_clone.clone()), move |res| {
                            let name = server_name_clone.clone();
                            Message::ProfilesLoaded(name, res)
                        });
                }
                Err(e) => {
                    state.server_crud_error = Some(format!("Failed to create server: {e}"));
                }
            }
        }
        Message::ServerUpdated(result) => {
            match result {
                Ok(updated_server) => {
                    state.server_action = None;
                    state.server_form_data = ServerConfigFormData::default();
                    state.server_crud_error = None;

                    // Find and update in the list
                    if let Some(server_in_list) = state
                        .servers
                        .iter_mut()
                        .find(|s| s.name == updated_server.name)
                    {
                        *server_in_list = updated_server.clone();
                    } else {
                        warn!(
                            "Error: Updated server '{}' not found in list after update.",
                            updated_server.name
                        );
                    }
                    state.selected_server = Some(updated_server.name);
                }
                Err(e) => {
                    state.server_crud_error = Some(format!("Failed to update server: {e}"));
                }
            }
        }
        Message::ServerDeleted(result) => {
            match result {
                Ok(deleted_name) => {
                    state.server_crud_error = None; // Clear any previous error
                    state.server_action = None; // Ensure action is cleared after successful delete

                    // Remove from server list
                    state.servers.retain(|s| s.name != deleted_name);
                    // Remove from profiles map
                    state.profiles.remove(&deleted_name);

                    // If the deleted server was selected, select the first one or none
                    if state.selected_server.as_ref() == Some(&deleted_name) {
                        if let Some(first_server) = state.servers.first() {
                            let server_name_clone = first_server.name.clone();
                            state.selected_server = Some(server_name_clone.clone());
                            state.is_loading_profiles = Some(server_name_clone.clone());
                            task = Task::perform(
                                load_profiles_async(server_name_clone.clone()),
                                move |res| {
                                    let name = server_name_clone.clone();
                                    Message::ProfilesLoaded(name, res)
                                },
                            );
                        } else {
                            state.selected_server = None;
                            state.is_loading_profiles = None;
                        }
                    }
                    // No need to clear server_action etc. as delete happens outside the form flow
                }
                Err(e) => {
                    // Show error, maybe associate with the server if possible?
                    state.server_crud_error = Some(format!("Failed to delete server: {e}"));
                    warn!("Failed to delete server: {e}");
                    // If deletion failed while confirming, reset state back to None
                    // (or maybe back to Edit if that was the origin? Simpler to just reset)
                    if matches!(
                        state.server_action,
                        Some(ServerCrudAction::ConfirmDelete(_))
                    ) {
                        state.server_action = None;
                    }
                }
            }
        }
        Message::RequestCreateProfile => {
            if state.selected_server.is_some() {
                state.profile_action = Some(ProfileCrudAction::Create);
                state.profile_form_data = ProfileConfigFormData::default();
                state.profile_crud_error = None;
                state.server_action = None; // Hide server form
            } else {
                warn!("Error: Cannot create profile, no server selected.");
            }
        }
        Message::RequestEditProfile(profile_name) => {
            // Ensure a server is selected first
            if let Some(server_name) = &state.selected_server {
                // Find the profile within the selected server's profile list
                if let Some(profile_vec) = state.profiles.get(server_name) {
                    if let Some(profile_to_edit) =
                        profile_vec.iter().find(|p| p.name == profile_name)
                    {
                        state.profile_action = Some(ProfileCrudAction::Edit(profile_name.clone()));
                        state.profile_form_data = ProfileConfigFormData {
                            name: profile_to_edit.name.clone(), // Pre-fill name for context (won't be editable in form)
                            caption: profile_to_edit.config.caption.clone(),
                        };
                        state.profile_form_send_on_connect_content =
                            text_editor::Content::with_text(
                                profile_to_edit.config.send_on_connect.as_str(),
                            );
                        state.profile_crud_error = None;
                        state.server_action = None; // Hide server form if it was open
                    } else {
                        warn!(
                            "Error: Requested to edit non-existent profile '{profile_name}' in server '{server_name}'"
                        );
                    }
                } else {
                    warn!(
                        "Error: Profile list not available for server '{server_name}' when trying to edit profile '{profile_name}'"
                    );
                }
            } else {
                warn!("Error: Cannot edit profile, no server selected.");
            }
        }
        Message::RequestConfirmDeleteProfile(profile_name) => {
            state.profile_action = Some(ProfileCrudAction::ConfirmDelete(profile_name));
            state.profile_crud_error = None;
        }
        Message::ConfirmDeleteProfile(profile_name) => {
            state.profile_crud_error = None;
            // Let's try calling the async task directly for simplicity.
            if let Some(server_name) = state.selected_server.clone() {
                // Use if let for safety
                state.profile_crud_error = None;
                task = Task::perform(
                    delete_profile_async(server_name, profile_name),
                    Message::ProfileDeleted,
                );
            } else {
                warn!("Error: Cannot delete profile, no server selected during confirmation.");
                state.profile_crud_error =
                    Some("Error: No server selected for deletion confirmation.".to_string());
                state.profile_action = None;
            }
        }
        Message::UpdateProfileFormField(field, value) => {
            match field {
                ProfileFormField::Name => state.profile_form_data.name = value,
                ProfileFormField::Caption => state.profile_form_data.caption = value,
            }
            state.profile_crud_error = None;
        }
        Message::UpdateProfileFormSendOnConnect(action) => {
            state.profile_form_send_on_connect_content.perform(action);
        }
        Message::SubmitProfileForm => {
            task = handle_submit_profile_form(state);
        }
        Message::CancelProfileForm => {
            state.profile_action = None;
            state.profile_form_data = ProfileConfigFormData::default();
            state.profile_form_send_on_connect_content = text_editor::Content::new(); // Reset editor content
            state.profile_crud_error = None;
        }
        Message::ProfileCreated(result) => {
            match result {
                Ok(new_profile) => {
                    state.profile_action = None;
                    state.profile_form_data = ProfileConfigFormData::default();
                    state.profile_crud_error = None;

                    // Need to find the server name this profile belongs to.
                    // This relies on the create action having been initiated with a selected server.
                    // A more robust approach might involve the async task returning the server name.
                    // For now, assume state.selected_server holds the relevant server.
                    if let Some(server_name) = &state.selected_server {
                        if let Some(server_profiles) = state.profiles.get_mut(server_name) {
                            server_profiles.push(new_profile.clone());
                            server_profiles.sort_by(|a, b| a.name.cmp(&b.name)); // Sort by name
                        } else {
                            warn!(
                                "Error: Server '{}' not found in profile map after creating profile '{}'",
                                server_name, new_profile.name
                            );
                        }
                    } else {
                        warn!("Error: No server selected after profile creation finished.")
                    }
                    // Keep the current server selected
                }
                Err(e) => {
                    state.profile_crud_error = Some(format!("Failed to create profile: {e}"));
                }
            }
        }
        Message::ProfileUpdated(result) => {
            match result {
                Ok(updated_profile) => {
                    state.profile_action = None;
                    state.profile_form_data = ProfileConfigFormData::default();
                    state.profile_crud_error = None;

                    // Assume state.selected_server holds the relevant server context
                    if let Some(server_name) = &state.selected_server {
                        if let Some(server_profiles) = state.profiles.get_mut(server_name) {
                            if let Some(profile_in_list) = server_profiles
                                .iter_mut()
                                .find(|p| p.name == updated_profile.name)
                            {
                                *profile_in_list = updated_profile.clone();
                                server_profiles.sort_by(|a, b| a.name.cmp(&b.name)); // Sort by name
                            } else {
                                warn!(
                                    "Error: Updated profile '{}' not found in list for server '{}'",
                                    updated_profile.name, server_name
                                );
                            }
                        } else {
                            warn!(
                                "Error: Server '{}' not found in profile map after updating profile '{}'",
                                server_name, updated_profile.name
                            );
                        }
                    } else {
                        warn!("Error: No server selected after profile update finished.")
                    }
                    // Keep the current server selected
                }
                Err(e) => {
                    state.profile_crud_error = Some(format!("Failed to update profile: {e}"));
                }
            }
        }
        Message::ProfileDeleted(result) => {
            match result {
                Ok((server_name, deleted_profile_name)) => {
                    state.profile_crud_error = None;
                    state.profile_action = None; // Ensure action is cleared after successful delete

                    // Remove from the map
                    if let Some(server_profiles) = state.profiles.get_mut(&server_name) {
                        server_profiles.retain(|p| p.name != deleted_profile_name);
                    } else {
                        warn!(
                            "Warning: Server '{server_name}' not found in profile map when handling deletion of profile '{deleted_profile_name}'"
                        );
                    }
                    // If the current server is the one affected, we might want to refresh
                    // its view, but no need to change selection unless the server itself was deleted.
                }
                Err(e) => {
                    // Show error, maybe associate with the server if possible?
                    state.profile_crud_error = Some(format!("Failed to delete profile: {e}"));
                    warn!("Failed to delete profile: {e}");
                    // Keep the confirmation state active so the user sees the error
                    // Or maybe reset to Edit state? Let's reset to Edit.
                    if let Some(ProfileCrudAction::ConfirmDelete(name)) = &state.profile_action {
                        state.profile_action = Some(ProfileCrudAction::Edit(name.clone()));
                    }
                }
            }
        }
    }
    (task, event)
}

// --- View Logic ---

/// Renders the server list pane.
fn view_server_list(state: &State) -> Element<Message> {
    let server_list_content: Element<Message> = if state.servers.is_empty() {
        if state.is_loading_servers {
            column![text("Loading servers...")]
        } else {
            column![text("No servers.")]
        }
        .into()
    } else {
        state
            .servers
            .iter()
            // Fold produces a Column<'_, Message>
            .fold(Column::new().spacing(5), |col, server| {
                let is_selected = state.selected_server.as_ref() == Some(&server.name);

                // Start building the button
                let mut server_button = button(text(&server.name)).width(Length::Fill);

                // Conditionally add the on_press handler
                if state.profile_action.is_none() {
                    server_button =
                        server_button.on_press(Message::SelectServer(server.name.clone()));
                }
                // If profile_action is Some, button remains without on_press

                // Push the button (which emits Message)
                col.push(server_button)
            })
            .into() // Converts Column<'_, Message> into Element<'_, Message>
    };

    // No mapping needed anymore
    // let mapped_server_list = server_list_content.map(std::convert::identity);

    let mut final_column = column![
        text("Connect").size(Pixels(24.0)),
        scrollable(server_list_content).height(Length::Fill), // Use original list
                                                              // "New Server" button added conditionally below
    ]
    .width(Length::Fixed(200.0))
    .spacing(10)
    .padding(15);

    // Conditionally add the "New Server" button
    if state.profile_action.is_none() {
        let new_server_button = button("New Server")
            .width(Length::Fill)
            .on_press(Message::RequestCreateServer);
        final_column = final_column.push(new_server_button);
    }

    final_column.into()
}

/// Renders the server create/edit form.
fn view_server_form<'a>(state: &'a State, action: &'a ServerCrudAction) -> Element<'a, Message> {
    match action {
        ServerCrudAction::Create => {
            // --- Create Form ---
            let form_title = "Create New Server";
            let name_input =
                TextInput::new("Server Name (e.g., 'MyMUD')", &state.server_form_data.name)
                    .on_input(|val| Message::UpdateServerFormField(ServerFormField::Name, val));
            let host_input = TextInput::new(
                "Host (e.g., 'mud.example.com')",
                &state.server_form_data.host,
            )
            .on_input(|val| Message::UpdateServerFormField(ServerFormField::Host, val));
            let port_input = TextInput::new("Port (e.g., '4000')", &state.server_form_data.port)
                .on_input(|val| Message::UpdateServerFormField(ServerFormField::Port, val));

            let error_display: Element<Message> = match &state.server_crud_error {
                Some(error) => text(error).into(),
                None => horizontal_space().into(),
            };

            let save_button = button("Save Server").on_press(Message::SubmitServerForm);
            let cancel_button = button("Cancel").on_press(Message::CancelServerForm);

            Column::new()
                .push(text(form_title).size(Pixels(24.0)))
                .push(name_input)
                .push(host_input)
                .push(port_input)
                .push(error_display)
                .push(Row::new().push(save_button).push(cancel_button).spacing(10))
                .spacing(15)
                .into()
        }
        ServerCrudAction::Edit(name) => {
            // --- Edit Form ---
            let form_title = "Edit Server";
            let name_display = text(format!("Editing: {name}")).size(Pixels(20.0));
            let host_input = TextInput::new(
                "Host (e.g., 'mud.example.com')",
                &state.server_form_data.host,
            )
            .on_input(|val| Message::UpdateServerFormField(ServerFormField::Host, val));
            let port_input = TextInput::new("Port (e.g., '4000')", &state.server_form_data.port)
                .on_input(|val| Message::UpdateServerFormField(ServerFormField::Port, val));

            let error_display: Element<Message> = match &state.server_crud_error {
                Some(error) => text(error).into(),
                None => horizontal_space().into(),
            };

            let save_button = button("Save Changes").on_press(Message::SubmitServerForm);
            let cancel_button = button("Cancel").on_press(Message::CancelServerForm);
            let delete_button =
                button("Delete Server") // Add the delete button
                    .on_press(Message::RequestConfirmDeleteServer(name.clone()));

            Column::new()
                .push(text(form_title).size(Pixels(24.0)))
                .push(name_display) // Show non-editable name
                .push(host_input)
                .push(port_input)
                .push(error_display)
                .push(Row::new().push(save_button).push(cancel_button).spacing(10))
                .push(horizontal_space().height(Pixels(20.0))) // Add some space before delete
                .push(delete_button) // Place delete button here
                .spacing(15)
                .into()
        }
        ServerCrudAction::ConfirmDelete(name) => {
            // --- Delete Confirmation ---
            let form_title = "Delete Server";
            let confirmation_text = text(format!(
                "Are you sure you want to delete the server '{name}'? This cannot be undone."
            ))
            .size(Pixels(18.0));

            let confirm_delete_button = button("Yes, Delete This Server")
                .on_press(Message::ConfirmDeleteServer(name.clone()));

            // CancelServerForm resets the action to None, hiding the form.
            let cancel_delete_button = button("Cancel").on_press(Message::CancelServerForm);

            let error_display: Element<Message> = match &state.server_crud_error {
                Some(error) => text(error).into(), // Show error here if delete fails
                None => horizontal_space().into(),
            };

            Column::new()
                .push(text(form_title).size(Pixels(24.0)))
                .push(confirmation_text)
                .push(error_display) // Display potential errors from failed delete attempt
                .push(
                    Row::new()
                        .push(confirm_delete_button)
                        .push(cancel_delete_button)
                        .spacing(10),
                )
                .spacing(15)
                .into()
        }
    }
}

/// Renders the profile create/edit form, or the delete confirmation.
fn view_profile_form<'a>(state: &'a State, action: &'a ProfileCrudAction) -> Element<'a, Message> {
    match action {
        ProfileCrudAction::Create => {
            // --- Create Form ---
            let name_input = TextInput::new(
                "Profile Name (e.g., 'MyChar')",
                &state.profile_form_data.name,
            )
            .on_input(|val| Message::UpdateProfileFormField(ProfileFormField::Name, val));

            let caption_input = TextInput::new(
                "Caption (e.g., 'My Cool Character')",
                &state.profile_form_data.caption,
            )
            .on_input(|val| Message::UpdateProfileFormField(ProfileFormField::Caption, val));

            let send_on_connect_text_editor =
                text_editor(&state.profile_form_send_on_connect_content)
                    .placeholder("Send on Connect (optional, e.g., 'connect player password')")
                    .height(Length::Fixed(60.0))
                    .font(Font {
                        family: Family::Monospace,
                        ..Font::default()
                    })
                    .on_action(Message::UpdateProfileFormSendOnConnect);

            let error_display: Element<Message> = match &state.profile_crud_error {
                Some(error) => text(error).into(),
                None => horizontal_space().into(),
            };

            let save_button = button("Create Profile").on_press(Message::SubmitProfileForm);
            let cancel_button = button("Cancel").on_press(Message::CancelProfileForm);

            Column::new()
                .push(text("Create New Profile").size(Pixels(24.0)))
                .push(name_input)
                .push(caption_input)
                .push(send_on_connect_text_editor)
                .push(error_display)
                .push(Row::new().push(save_button).push(cancel_button).spacing(10))
                .spacing(15)
                .into()
        }
        ProfileCrudAction::Edit(name) => {
            // --- Edit Form ---
            let name_display = text(format!("Editing Profile: {name}")).size(Pixels(20.0));

            let caption_input = TextInput::new("Caption", &state.profile_form_data.caption)
                .on_input(|val| Message::UpdateProfileFormField(ProfileFormField::Caption, val));

            let send_on_connect_text_editor =
                text_editor(&state.profile_form_send_on_connect_content)
                    .placeholder("Send on Connect (optional)")
                    .height(Length::Fixed(60.0))
                    .font(Font {
                        family: Family::Monospace,
                        ..Font::default()
                    })
                    .on_action(Message::UpdateProfileFormSendOnConnect);

            let error_display: Element<Message> = match &state.profile_crud_error {
                Some(error) => text(error).into(),
                None => horizontal_space().into(),
            };

            let save_button = button("Save Changes").on_press(Message::SubmitProfileForm);
            let cancel_button = button("Cancel").on_press(Message::CancelProfileForm);
            let request_delete_button = button("Delete Profile")
                .on_press(Message::RequestConfirmDeleteProfile(name.clone()));

            Column::new()
                .push(text("Edit Profile").size(Pixels(24.0)))
                .push(name_display)
                .push(caption_input)
                .push(send_on_connect_text_editor)
                .push(error_display)
                .push(Row::new().push(save_button).push(cancel_button).spacing(10))
                .push(horizontal_space().height(Pixels(20.0))) // Add some space
                .push(request_delete_button) // Delete button at the bottom
                .spacing(15)
                .into()
        }
        ProfileCrudAction::ConfirmDelete(name) => {
            // --- Delete Confirmation ---
            let confirmation_text =
                text(format!("Are you sure you want to delete profile '{name}'?"))
                    .size(Pixels(18.0)); // Slightly larger text for confirmation

            let confirm_delete_button = button("Yes, Delete This Profile")
                .on_press(Message::ConfirmDeleteProfile(name.clone()));

            // Re-use CancelProfileForm which resets the state appropriately
            let cancel_delete_button = button("Cancel").on_press(Message::CancelProfileForm);

            let error_display: Element<Message> = match &state.profile_crud_error {
                Some(error) => text(error).into(),
                None => horizontal_space().into(),
            };

            Column::new()
                .push(text("Confirm Deletion").size(Pixels(24.0)))
                .push(confirmation_text)
                .push(error_display) // Display potential errors from failed delete attempt
                .push(
                    Row::new()
                        .push(confirm_delete_button)
                        .push(cancel_delete_button)
                        .spacing(10),
                )
                .spacing(15)
                .into()
        }
    }
}

/// Renders the server details and profile list for the selected server.
fn view_server_details_and_profiles<'a>(
    state: &'a State,
    server_name: &'a ServerName,
) -> Element<'a, Message> {
    // Find the actual server details
    let server_details = state.servers.iter().find(|s| s.name == *server_name);

    // --- Profile List ---
    let profiles = state.profiles.get(server_name);
    let is_loading_p = state.is_loading_profiles.as_ref() == Some(server_name);

    let profile_list_content: Element<Message> = match (profiles, is_loading_p) {
        (_, true) => column![text("Loading profiles...")].into(),
        (Some(profiles), false) if profiles.is_empty() => container(
            text("Create a character to continue")
                .size(Pixels(24.0))
                .center(),
        )
        .padding(15)
        .width(Length::Fill)
        .into(),
        (Some(profiles), false) => {
            profiles
                .iter()
                .fold(Column::new().spacing(5), |col, profile| {
                    let connect_button = button("Connect")
                        .width(Length::FillPortion(3)) // Give connect button more space
                        .on_press(Message::ConnectProfile(
                            server_name.clone(),
                            profile.name.clone(),
                        ));

                    let edit_button = button(text("Edit").size(Pixels(12.0))) // Changed label to "Edit"
                        .width(Length::Fixed(40.0)) // Adjusted width
                        .on_press(Message::RequestEditProfile(profile.name.clone()));

                    // Delete button ('X') removed from this row

                    col.push(
                        Row::new()
                            .push(text(&profile.name))
                            .push(horizontal_space())
                            .push(edit_button) // Only show Edit button here
                            .push(connect_button)
                            .spacing(5)
                            .align_y(Alignment::Center),
                    )
                    .push(text(&profile.config.caption).size(Pixels(12.0)))
                })
                .into()
        }
        (None, false) => column![text("Error loading profiles?")].into(),
    };

    // --- Top Section: Server Info + Edit/Delete Buttons ---
    let server_info_row = if let Some(server) = server_details {
        Row::new()
            .push(text(format!("Host: {}", server.config.host)).size(Pixels(16.0)))
            .push(horizontal_space())
            .push(text(format!("Port: {}", server.config.port)).size(Pixels(16.0)))
            .spacing(10)
    } else {
        Row::new().push(text("Server details not found?").size(Pixels(16.0)))
    };

    let edit_server_button =
        button("Edit Server").on_press(Message::RequestEditServer(server_name.clone()));

    // Row for server name and edit button
    let title_edit_row = Row::new()
        .push(text(server_name).size(Pixels(28.0))) // Server Name Title
        .push(horizontal_space()) // Push button to the right
        .push(edit_server_button) // Add edit button
        .align_y(Alignment::Center); // Vertically align items

    // --- Combine Details/Profiles View ---
    let mut content_col = Column::new()
        .push(title_edit_row) // Use the new combined row
        .push(server_info_row) // Host & Port Row
        .push(text("Characters").size(Pixels(20.0))) // Characters Title
        .push(scrollable(profile_list_content).height(Length::FillPortion(1))) // Profile List
        .spacing(15);

    // Add New Profile button at the bottom
    let new_profile_button = button("New Character")
        .width(Length::Fill)
        .on_press(Message::RequestCreateProfile);
    content_col = content_col.push(new_profile_button);

    content_col.into()
}

/// Renders the placeholder content when no server is selected or loading.
fn view_placeholder(state: &State) -> Element<Message> {
    if state.is_loading_servers {
        column![text("Loading servers...").size(Pixels(20.0))].into()
    } else {
        column![text("Select or create a server").size(Pixels(20.0))].into()
    }
}

/// The main view function for the connect modal.
pub fn view(state: &State) -> Element<Message> {
    let server_pane = view_server_list(state);

    // Determine the content for the main pane based on the state
    let main_pane_content = if let Some(action) = &state.server_action {
        // Show server form if a server action is active
        view_server_form(state, action)
    } else if let Some(action) = &state.profile_action {
        // Show profile form if a profile action is active (Create, Edit, or ConfirmDelete)
        view_profile_form(state, action)
    } else if let Some(server_name) = &state.selected_server {
        // Show server details and profiles if a server is selected
        view_server_details_and_profiles(state, server_name)
    } else {
        // Show placeholder if no server is selected and no form is active
        view_placeholder(state)
    };

    let main_pane = container(main_pane_content)
        .width(Length::Fill)
        .padding(15)
        .into();

    // Combine panes into the modal body
    Row::with_children(vec![server_pane, main_pane]).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    // use iced::Command; // Ensure this line is removed or commented if present from previous edits
    use smudgy_core::models::profile::ProfileConfig;
    use smudgy_core::models::server::ServerConfig;

    // Helper to create a default state
    fn initial_state() -> State {
        State::default()
    }

    #[test]
    fn test_initial_state_is_correct() {
        let state = initial_state();
        assert!(state.servers.is_empty());
        assert!(state.profiles.is_empty());
        assert!(state.selected_server.is_none());
        assert!(!state.is_loading_servers); // Should be false until a load is triggered
        assert!(state.is_loading_profiles.is_none());
        assert!(state.server_action.is_none());
        assert_eq!(state.server_form_data.name, "");
        assert_eq!(state.server_form_data.host, "");
        assert_eq!(state.server_form_data.port, "");
        assert!(state.server_crud_error.is_none());
        assert!(state.profile_action.is_none());
        assert_eq!(state.profile_form_data.name, "");
        assert_eq!(state.profile_form_data.caption, "");
        assert_eq!(state.profile_form_send_on_connect_content.text(), "\n");
        assert!(state.profile_crud_error.is_none());
    }

    #[test]
    fn test_request_create_server_updates_state() {
        let mut state = initial_state();
        let (_task, event) = update(&mut state, Message::RequestCreateServer);

        assert!(event.is_none());
        assert_eq!(state.server_action, Some(ServerCrudAction::Create));
        assert_eq!(state.server_form_data.name, "");
        assert!(state.server_crud_error.is_none());
        assert!(state.selected_server.is_none());
        assert!(state.is_loading_profiles.is_none());
    }

    #[test]
    fn test_cancel_server_form_resets_state() {
        let mut state = initial_state();
        state.server_action = Some(ServerCrudAction::Create);
        state.server_form_data.name = "Test".to_string();
        state.server_crud_error = Some("Error".to_string());

        let (_task, event) = update(&mut state, Message::CancelServerForm);

        assert!(event.is_none());
        assert!(state.server_action.is_none());
        assert_eq!(state.server_form_data.name, "");
        assert!(state.server_crud_error.is_none());
    }

    #[test]
    fn test_submit_server_form_create_valid() {
        let mut state = initial_state();
        state.server_action = Some(ServerCrudAction::Create);
        state.server_form_data = ServerConfigFormData {
            name: "MyMUD".to_string(),
            host: "mud.example.com".to_string(),
            port: "4000".to_string(),
        };

        // The task is not asserted directly. Its effect is tested via Message::ServerCreated.
        let (_task, event) = update(&mut state, Message::SubmitServerForm);

        assert!(event.is_none());
        assert!(state.server_crud_error.is_none());
    }

    #[test]
    fn test_submit_server_form_create_invalid_port() {
        let mut state = initial_state();
        state.server_action = Some(ServerCrudAction::Create);
        state.server_form_data = ServerConfigFormData {
            name: "MyMUD".to_string(),
            host: "mud.example.com".to_string(),
            port: "invalid_port".to_string(),
        };

        // The task is not asserted directly. No task should be spawned.
        let (_task, event) = update(&mut state, Message::SubmitServerForm);
        // Ensure user's assert!(task) is removed if it was here.
        assert!(event.is_none());
        assert!(state.server_crud_error.is_some());
        assert_eq!(
            state.server_crud_error.as_ref().unwrap(),
            "Invalid port number. Must be between 1 and 65535."
        );
    }

    #[test]
    fn test_submit_server_form_create_empty_name() {
        let mut state = initial_state();
        state.server_action = Some(ServerCrudAction::Create);
        state.server_form_data = ServerConfigFormData {
            name: "".to_string(),
            host: "mud.example.com".to_string(),
            port: "4000".to_string(),
        };

        let (_task, event) = update(&mut state, Message::SubmitServerForm);

        assert!(event.is_none());
        assert!(state.server_crud_error.is_some());
        assert_eq!(
            state.server_crud_error.as_ref().unwrap(),
            "Server name cannot be empty."
        );
    }

    #[test]
    fn test_select_server_loads_profiles_if_not_present() {
        let mut state = initial_state();
        let server_name = "TestServer".to_string();

        state.servers.push(Server {
            name: server_name.clone(),
            config: ServerConfig {
                host: "test.com".to_string(),
                port: 1234,
            },
            path: std::path::PathBuf::new(),
        });

        let (_task, event) = update(&mut state, Message::SelectServer(server_name.clone()));

        assert!(event.is_none());
        assert_eq!(state.selected_server, Some(server_name.clone()));
        assert_eq!(state.is_loading_profiles, Some(server_name.clone()));
    }

    #[test]
    fn test_select_server_does_not_load_profiles_if_present() {
        let mut state = initial_state();
        let server_name = "TestServer".to_string();

        state.servers.push(Server {
            name: server_name.clone(),
            config: ServerConfig {
                host: "test.com".to_string(),
                port: 1234,
            },
            path: std::path::PathBuf::new(),
        });
        state.profiles.insert(
            server_name.clone(),
            vec![Profile {
                name: "TestProfile".to_string(),
                config: ProfileConfig {
                    caption: "Caption".to_string(),
                    send_on_connect: "".to_string(),
                },
                path: std::path::PathBuf::new(),
            }],
        );

        let (_task, event) = update(&mut state, Message::SelectServer(server_name.clone()));

        assert!(event.is_none());
        assert_eq!(state.selected_server, Some(server_name.clone()));
        assert!(state.is_loading_profiles.is_none());
    }

    #[test]
    fn test_servers_loaded_selects_first_and_loads_profiles() {
        let mut state = initial_state();
        let server1 = Server {
            name: "AlphaServer".to_string(),
            config: ServerConfig {
                host: "".to_string(),
                port: 0,
            },
            path: std::path::PathBuf::new(),
        };
        let server2 = Server {
            name: "BetaServer".to_string(),
            config: ServerConfig {
                host: "".to_string(),
                port: 0,
            },
            path: std::path::PathBuf::new(),
        };
        let servers_to_load = vec![server1.clone(), server2.clone()];

        let (_task, event) = update(&mut state, Message::ServersLoaded(Ok(servers_to_load)));

        assert!(event.is_none());
        assert_eq!(state.servers.len(), 2);
        assert_eq!(state.selected_server, Some(server1.name.clone()));
        assert_eq!(state.is_loading_profiles, Some(server1.name.clone()));
    }

    #[test]
    fn test_servers_loaded_empty_sets_no_selection() {
        let mut state = initial_state();
        state.selected_server = Some("OldServer".to_string());
        state.is_loading_profiles = Some("OldServer".to_string());
        state.server_action = Some(ServerCrudAction::Create);

        let (_task, event) = update(&mut state, Message::ServersLoaded(Ok(vec![])));

        assert!(event.is_none());
        assert!(state.servers.is_empty());
        assert!(state.selected_server.is_none());
        assert!(state.is_loading_profiles.is_none());
        assert!(state.server_action.is_none());
    }

    #[test]
    fn test_servers_loaded_error() {
        let mut state = initial_state();
        state.is_loading_servers = true;
        let error_msg = "Failed to load".to_string();

        let (_task, event) = update(&mut state, Message::ServersLoaded(Err(error_msg.clone())));

        assert!(event.is_none());
        assert!(!state.is_loading_servers);
    }

    #[test]
    fn test_profiles_loaded_success() {
        let mut state = initial_state();
        let server_name = "MyServer".to_string();
        let profile1 = Profile {
            name: "Char1".to_string(),
            config: ProfileConfig {
                caption: "".to_string(),
                send_on_connect: "".to_string(),
            },
            path: std::path::PathBuf::new(),
        };
        state.selected_server = Some(server_name.clone());
        state.is_loading_profiles = Some(server_name.clone());

        let (_task, event) = update(
            &mut state,
            Message::ProfilesLoaded(server_name.clone(), Ok(vec![profile1.clone()])),
        );

        assert!(event.is_none());
        assert!(state.is_loading_profiles.is_none());
        assert!(state.profiles.contains_key(&server_name));
        assert_eq!(state.profiles.get(&server_name).unwrap().len(), 1);
        assert_eq!(
            state.profiles.get(&server_name).unwrap()[0].name,
            profile1.name
        );
    }

    #[test]
    fn test_profiles_loaded_success_for_non_current_loading_server() {
        let mut state = initial_state();
        let server_name_loaded = "ServerLoaded".to_string();
        let server_name_currently_loading = "ServerCurrentlyLoading".to_string();
        let profile1 = Profile {
            name: "Char1".to_string(),
            config: ProfileConfig {
                caption: "".to_string(),
                send_on_connect: "".to_string(),
            },
            path: std::path::PathBuf::new(),
        };

        state.selected_server = Some(server_name_currently_loading.clone());
        state.is_loading_profiles = Some(server_name_currently_loading.clone());

        let (_task, event) = update(
            &mut state,
            Message::ProfilesLoaded(server_name_loaded.clone(), Ok(vec![profile1.clone()])),
        );

        assert!(event.is_none());
        assert_eq!(
            state.is_loading_profiles,
            Some(server_name_currently_loading)
        );
        assert!(state.profiles.contains_key(&server_name_loaded));
        assert_eq!(state.profiles.get(&server_name_loaded).unwrap().len(), 1);
    }

    #[test]
    fn test_profiles_loaded_error() {
        let mut state = initial_state();
        let server_name = "MyServer".to_string();
        state.selected_server = Some(server_name.clone());
        state.is_loading_profiles = Some(server_name.clone());
        let error_msg = "Failed to load profiles".to_string();

        let (_task, event) = update(
            &mut state,
            Message::ProfilesLoaded(server_name.clone(), Err(error_msg.clone())),
        );

        assert!(event.is_none());
        assert!(state.is_loading_profiles.is_none());
        assert!(!state.profiles.contains_key(&server_name));
    }
}
