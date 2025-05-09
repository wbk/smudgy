// Models related to profile configurations

use crate::get_smudgy_home;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::{fs, io};
use validator::Validate;

/// Represents the configuration for a single profile within a server.
/// This struct is serialized to/from `profile.json` within the profile's directory.
#[derive(Serialize, Deserialize, Debug, Validate, Clone, PartialEq, Eq)]
pub struct ProfileConfig {
    pub caption: String,
    pub send_on_connect: String,
}

/// Represents a profile, including its configuration and associated directory path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    /// The unique name of the profile, derived from its directory name.
    pub name: String,
    /// The path to the profile's directory within the server's profiles directory.
    pub path: PathBuf,
    /// The profile's configuration details loaded from `profile.json`.
    pub config: ProfileConfig,
}

/// Helper function to load and deserialize `ProfileConfig` from a file.
///
/// # Errors
///
/// Returns an error if the file cannot be opened, read, or if the contents
/// cannot be deserialized into a `ProfileConfig` or fail validation.
fn load_profile_config(path: &PathBuf) -> Result<ProfileConfig> {
    let file_content = fs::read_to_string(path)
        .context(format!("Failed to read profile config file: {path:?}"))?;
    let config: ProfileConfig = serde_json::from_str(&file_content)
        .context(format!("Failed to parse profile config file: {path:?}"))?;
    config
        .validate()
        .context(format!("Profile config validation failed: {path:?}"))?;
    Ok(config)
}

/// Lists all valid profiles found within a specific server's profile directory.
///
/// A profile is considered valid if it's a directory within the server's `profiles` subfolder
/// and contains a readable and valid `profile.json` file.
///
/// # Arguments
///
/// * `server_name` - The name of the server whose profiles should be listed.
///
/// # Errors
///
/// Returns an error if the smudgy home or the server directory cannot be accessed.
/// If the server's `profiles` directory doesn't exist, an empty list is returned.
/// Errors reading individual profile directories or parsing `profile.json` files
/// are logged as warnings, and those profiles are skipped.
pub fn list_profiles(server_name: &str) -> Result<Vec<Profile>> {
    let smudgy_dir = get_smudgy_home()?;
    let server_path = smudgy_dir.join(server_name);
    let profiles_dir = server_path.join("profiles");

    let mut profiles = Vec::new();

    match fs::read_dir(&profiles_dir) {
        Ok(entries) => {
            for entry_result in entries {
                match entry_result {
                    Ok(entry) => {
                        let path = entry.path();
                        if path.is_dir() {
                            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                                let config_path = path.join("profile.json");
                                match load_profile_config(&config_path) {
                                    Ok(config) => {
                                        profiles.push(Profile {
                                            name: name.to_string(),
                                            path: path.clone(),
                                            config,
                                        });
                                    }
                                    Err(e) => {
                                        // Log warning: Failed to load profile config
                                        eprintln!(
                                            "Warning: Skipping profile '{name}' in server '{server_name}'. Failed to load config: {e}"
                                        );
                                    }
                                }
                            } else {
                                // Log warning: Invalid directory name (not UTF-8)
                                eprintln!(
                                    "Warning: Skipping profile directory with non-UTF8 name in server '{server_name}': {path:?}"
                                );
                            }
                        }
                    }
                    Err(e) => {
                        // Log warning: Failed to read profile directory entry
                        eprintln!(
                            "Warning: Failed to read profile directory entry in server '{server_name}': {e}"
                        );
                    }
                }
            }
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            // If the profiles dir doesn't exist for this server, return empty list.
            // This is not an error condition.
        }
        Err(e) => {
            // Other errors reading the profiles dir are propagated.
            return Err(e).context(format!(
                "Failed to read profiles directory for server '{}' at {}",
                server_name,
                profiles_dir.to_string_lossy()
            ));
        }
    }

    Ok(profiles)
}

/// Creates a new profile directory and configuration file within a server.
///
/// # Arguments
///
/// * `server_name` - The name of the server to add the profile to.
/// * `profile_name` - The name for the new profile. Must be a valid directory name.
/// * `config` - The initial `ProfileConfig` for the profile.
///
/// # Errors
///
/// Returns an error if:
/// * The profile name is invalid.
/// * The provided `config` is invalid.
/// * The smudgy home or server directory cannot be accessed.
/// * The server's `profiles` directory doesn't exist.
/// * A profile with the same name already exists within that server.
/// * There are filesystem errors during directory or file creation.
pub fn create_profile(
    server_name: &str,
    profile_name: &str,
    config: ProfileConfig,
) -> Result<Profile> {
    // Validate profile name
    if profile_name.is_empty()
        || profile_name.contains(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
    {
        return Err(anyhow::anyhow!(
            "Invalid profile name: '{}'. Use only alphanumeric, underscore, or hyphen.",
            profile_name
        ));
    }

    // Validate the provided configuration
    config.validate().context(format!(
        "Invalid configuration for profile '{profile_name}' in server '{server_name}'"
    ))?;

    let smudgy_dir = get_smudgy_home()?;
    let server_path = smudgy_dir.join(server_name);
    let profiles_dir = server_path.join("profiles");
    let profile_path = profiles_dir.join(profile_name);

    // Pre-flight check: Ensure server and profiles directories exist
    if !server_path.is_dir() {
        return Err(anyhow::anyhow!(
            "Server directory not found or not a directory: {:?}",
            server_path
        ));
    }
    if !profiles_dir.is_dir() {
        // This shouldn't happen if ensure_server_subdirs was called, but check defensively.
        return Err(anyhow::anyhow!(
            "Profiles directory not found within server '{}': {:?}",
            server_name,
            profiles_dir
        ));
    }

    // Check if profile directory already exists
    if profile_path.exists() {
        return Err(anyhow::anyhow!(
            "Profile '{}' already exists in server '{}' at {:?}",
            profile_name,
            server_name,
            profile_path
        ));
    }

    // Create the profile directory
    fs::create_dir(&profile_path).context(format!(
        "Failed to create directory for profile '{profile_name}' in server '{server_name}' at {profile_path:?}"
    ))?;

    // Write the profile.json file
    let config_path = profile_path.join("profile.json");
    let config_json = serde_json::to_string_pretty(&config).context(format!(
        "Failed to serialize config for profile '{profile_name}' in server '{server_name}'"
    ))?;

    fs::write(&config_path, config_json).context(format!(
        "Failed to write profile.json for profile '{profile_name}' in server '{server_name}' at {config_path:?}"
    ))?;

    Ok(Profile {
        name: profile_name.to_string(),
        path: profile_path,
        config,
    })
}

/// Loads a specific profile by its name within a given server.
///
/// # Arguments
///
/// * `server_name` - The name of the server containing the profile.
/// * `profile_name` - The name of the profile to load.
///
/// # Errors
///
/// Returns an error if:
/// * The smudgy home, server, or profiles directory cannot be accessed.
/// * No directory with the given `profile_name` exists within the server's profiles directory.
/// * The found path is not a directory.
/// * The `profile.json` file is missing, cannot be read, or is invalid.
pub fn load_profile(server_name: &str, profile_name: &str) -> Result<Profile> {
    let smudgy_dir = get_smudgy_home()?;
    let server_path = smudgy_dir.join(server_name);
    let profiles_dir = server_path.join("profiles");
    let profile_path = profiles_dir.join(profile_name);

    if !profile_path.exists() {
        return Err(anyhow::anyhow!(
            "Profile '{}' not found in server '{}'",
            profile_name,
            server_name
        ))
        .with_context(|| format!("Looked in directory: {profile_path:?}"));
    }

    if !profile_path.is_dir() {
        return Err(anyhow::anyhow!(
            "Path for profile '{}' in server '{}' exists but is not a directory: {:?}",
            profile_name,
            server_name,
            profile_path
        ));
    }

    // Load the configuration
    let config_path = profile_path.join("profile.json");
    let config = load_profile_config(&config_path).context(format!(
        "Failed to load config for profile '{profile_name}' in server '{server_name}'"
    ))?;

    Ok(Profile {
        name: profile_name.to_string(),
        path: profile_path,
        config,
    })
}

/// Updates the configuration of an existing profile within a server.
///
/// Finds the profile by name, validates the new configuration, and overwrites
/// the existing `profile.json` file.
///
/// # Arguments
///
/// * `server_name` - The name of the server containing the profile.
/// * `profile_name` - The name of the profile to update.
/// * `new_config` - The `ProfileConfig` containing the updated settings.
///
/// # Errors
///
/// Returns an error if:
/// * The profile with the given `name` cannot be found within the server.
/// * The path found is not a directory.
/// * The `new_config` fails validation.
/// * The `profile.json` file cannot be written.
pub fn update_profile(
    server_name: &str,
    profile_name: &str,
    new_config: ProfileConfig,
) -> Result<Profile> {
    // Validate the new configuration first
    new_config.validate().context(format!(
        "Invalid new configuration provided for profile '{profile_name}' in server '{server_name}'"
    ))?;

    let smudgy_dir = get_smudgy_home()?;
    let server_path = smudgy_dir.join(server_name);
    let profiles_dir = server_path.join("profiles");
    let profile_path = profiles_dir.join(profile_name);

    // Ensure the profile directory exists and is a directory
    if !profile_path.exists() {
        return Err(anyhow::anyhow!(
            "Profile '{}' not found in server '{}' for update",
            profile_name,
            server_name
        ))
        .with_context(|| format!("Looked for directory: {profile_path:?}"));
    }
    if !profile_path.is_dir() {
        return Err(anyhow::anyhow!(
            "Path for profile '{}' in server '{}' exists but is not a directory: {:?}",
            profile_name,
            server_name,
            profile_path
        ));
    }

    // Construct path to profile.json
    let config_path = profile_path.join("profile.json");

    // Serialize the new config
    let config_json = serde_json::to_string_pretty(&new_config).context(format!(
        "Failed to serialize updated config for profile '{profile_name}' in server '{server_name}'"
    ))?;

    // Write the new config, overwriting the old one
    fs::write(&config_path, config_json).context(format!(
        "Failed to write updated profile.json for profile '{profile_name}' in server '{server_name}' at {config_path:?}"
    ))?;

    // Return the profile representation with the new config
    Ok(Profile {
        name: profile_name.to_string(),
        path: profile_path,
        config: new_config, // Use the validated new_config
    })
}

/// Deletes a profile and all its associated data from a server.
///
/// Finds the profile directory by name within the specified server's `profiles`
/// directory and removes it recursively.
/// If the profile directory does not exist, the function succeeds silently.
///
/// # Arguments
///
/// * `server_name` - The name of the server containing the profile.
/// * `profile_name` - The name of the profile to delete.
///
/// # Errors
///
/// Returns an error if:
/// * The smudgy home or server directory cannot be accessed.
/// * A file exists with the profile name (instead of a directory).
/// * The directory or its contents cannot be removed due to permissions or other I/O issues.
pub fn delete_profile(server_name: &str, profile_name: &str) -> Result<()> {
    let smudgy_dir = get_smudgy_home()?;
    let server_path = smudgy_dir.join(server_name);
    let profiles_dir = server_path.join("profiles");
    let profile_path = profiles_dir.join(profile_name);

    if profile_path.exists() {
        // Check if it's actually a directory before attempting recursive delete
        if !profile_path.is_dir() {
            return Err(anyhow::anyhow!(
                "Cannot delete profile '{}' in server '{}': Path exists but is not a directory: {:?}",
                profile_name,
                server_name,
                profile_path
            ));
        }

        // Recursively remove the directory
        fs::remove_dir_all(&profile_path).context(format!(
            "Failed to delete directory for profile '{profile_name}' in server '{server_name}' at {profile_path:?}"
        ))?;
    } else {
        // Optionally log that the profile didn't exist? For now, silent success.
        println!(
            "Info: Profile '{profile_name}' not found in server '{server_name}' for deletion."
        );
    }

    Ok(())
}
