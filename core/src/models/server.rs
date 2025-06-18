// Models related to server configurations

use crate::get_smudgy_home;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::{fs, io};
use validator::Validate;

/// Represents the configuration for a single server connection.
/// This struct is serialized to/from `server.json` within the server's directory.
#[derive(Serialize, Deserialize, Debug, Validate, Clone, PartialEq, Eq)]
pub struct ServerConfig {
    /// The hostname or IP address of the server.
    #[validate(length(min = 1, message = "Host cannot be empty"))]
    pub host: String,
    /// The port number of the server.
    #[validate(range(min = 1, max = 65535, message = "Port must be between 1 and 65535"))]
    pub port: u16,
}

/// Represents a server, including its configuration and associated directory path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Server {
    /// The unique name of the server, derived from its directory name.
    pub name: String,
    /// The path to the server's directory within the smudgy home.
    pub path: PathBuf,
    /// The server's configuration details loaded from `server.json`.
    pub config: ServerConfig,
}

/// Lists all valid servers found within the smudgy home directory.
///
/// A server is considered valid if it's a directory within the smudgy home
/// and contains a readable and valid `server.json` file.
///
/// # Errors
///
/// Returns an error if the smudgy home directory cannot be accessed or read.
/// Errors reading individual server directories or parsing `server.json` files
/// are logged as warnings, and those servers are skipped.
pub fn list_servers() -> Result<Vec<Server>> {
    let smudgy_dir = get_smudgy_home()?;
    let mut servers = Vec::new();

    match fs::read_dir(&smudgy_dir) {
        Ok(entries) => {
            for entry_result in entries {
                match entry_result {
                    Ok(entry) => {
                        let path = entry.path();
                        if path.is_dir() {
                            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                                let config_path = path.join("server.json");
                                match load_server_config(&config_path) {
                                    Ok(config) => {
                                        servers.push(Server {
                                            name: name.to_string(),
                                            path: path.clone(),
                                            config,
                                        });
                                    }
                                    Err(e) => {
                                        // Log warning: Failed to load server config
                                        eprintln!(
                                            "Warning: Skipping server '{name}'. Failed to load config: {e}"
                                        );
                                    }
                                }
                            } else {
                                // Log warning: Invalid directory name (not UTF-8)
                                eprintln!(
                                    "Warning: Skipping directory with non-UTF8 name: {path:?}"
                                );
                            }
                        }
                    }
                    Err(e) => {
                        // Log warning: Failed to read directory entry
                        eprintln!("Warning: Failed to read directory entry: {e}");
                    }
                }
            }
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            // If the smudgy dir itself doesn't exist yet (first run?), return empty list.
            // get_smudgy_home() already created it, but read_dir might race?
            // Or maybe permissions issue. Log warning.
            eprintln!("Warning: Smudgy home directory not found or accessible during scan: {e}");
            // Returning empty list is fine here.
        }
        Err(e) => {
            // Other errors reading the main smudgy dir are propagated.
            return Err(e).context(format!(
                "Failed to read smudgy directory entries at {}",
                smudgy_dir.to_string_lossy()
            ));
        }
    }

    Ok(servers)
}

/// Helper function to load and deserialize `ServerConfig` from a file.
///
/// # Errors
///
/// Returns an error if the file cannot be opened, read, or if the contents
/// cannot be deserialized into a `ServerConfig`.
fn load_server_config(path: &PathBuf) -> Result<ServerConfig> {
    let file_content =
        fs::read_to_string(path).context(format!("Failed to read server config file: {path:?}"))?;
    let config: ServerConfig = serde_json::from_str(&file_content)
        .context(format!("Failed to parse server config file: {path:?}"))?;
    config
        .validate()
        .context(format!("Server config validation failed: {path:?}"))?;
    Ok(config)
}

/// Ensures the standard subdirectories exist within a given server directory.
///
/// Creates `profiles`, `aliases`, `hotkeys`, `triggers`, `modules`, and `maps`
/// directories if they don't already exist.
///
/// # Arguments
///
/// * `server_path` - The path to the server's root directory.
///
/// # Errors
///
/// Returns an error if any of the directories cannot be created.
pub fn ensure_server_subdirs(server_path: &PathBuf) -> Result<()> {
    let subdirs = [
        "profiles", "aliases", "hotkeys", "triggers", "modules", "maps", "logs",
    ];

    for subdir in &subdirs {
        let dir_path = server_path.join(subdir);
        fs::create_dir_all(&dir_path).context(format!(
            "Failed to create subdirectory '{subdir}' in {server_path:?}"
        ))?;
    }

    Ok(())
}

/// Creates a new server directory structure and configuration file.
///
/// # Arguments
///
/// * `name` - The name for the new server. Must be a valid directory name.
/// * `config` - The initial `ServerConfig` for the server.
///
/// # Errors
///
/// Returns an error if:
/// * The server name is invalid.
/// * The provided `config` is invalid.
/// * The smudgy home directory cannot be accessed.
/// * A server with the same name already exists.
/// * There are filesystem errors during directory or file creation.
pub fn create_server(name: &str, config: ServerConfig) -> Result<Server> {
    // Validate server name (basic validation for now)
    if name.is_empty() || name.contains(|c: char| !c.is_alphanumeric() && c != '_' && c != '-') {
        return Err(anyhow::anyhow!(
            "Invalid server name: '{}'. Use only alphanumeric, underscore, or hyphen.",
            name
        ));
    }

    // Validate the provided configuration
    config
        .validate()
        .context(format!("Invalid configuration for server '{name}'"))?;

    let smudgy_dir = get_smudgy_home()?;
    let server_path = smudgy_dir.join(name);

    // Check if server directory already exists
    if server_path.exists() {
        return Err(anyhow::anyhow!(
            "Server '{}' already exists at {:?}",
            name,
            server_path
        ));
    }

    // Create the main server directory
    fs::create_dir(&server_path).context(format!(
        "Failed to create main directory for server '{name}' at {server_path:?}"
    ))?;

    // Ensure standard subdirectories are created
    ensure_server_subdirs(&server_path)?;

    // Write the server.json file
    let config_path = server_path.join("server.json");
    let config_json = serde_json::to_string_pretty(&config)
        .context(format!("Failed to serialize config for server '{name}'"))?;

    fs::write(&config_path, config_json).context(format!(
        "Failed to write server.json for server '{name}' at {config_path:?}"
    ))?;

    Ok(Server {
        name: name.to_string(),
        path: server_path,
        config,
    })
}

/// Loads a specific server by its name.
///
/// This function finds the server directory, ensures the standard subdirectories
/// exist (creating them if necessary), loads the `server.json` configuration,
/// and returns the `Server` struct.
///
/// # Arguments
///
/// * `name` - The name of the server to load.
///
/// # Errors
///
/// Returns an error if:
/// * The smudgy home directory cannot be accessed.
/// * No directory with the given `name` exists within the smudgy home.
/// * The found path is not a directory.
/// * The `server.json` file is missing, cannot be read, or is invalid.
/// * Any required subdirectories cannot be created.
pub fn load_server(name: &str) -> Result<Server> {
    let smudgy_dir = get_smudgy_home()?;
    let server_path = smudgy_dir.join(name);

    if !server_path.exists() {
        return Err(anyhow::anyhow!("Server '{}' not found", name))
            .with_context(|| format!("Looked in directory: {server_path:?}"));
    }

    if !server_path.is_dir() {
        return Err(anyhow::anyhow!(
            "Path for server '{}' exists but is not a directory: {:?}",
            name,
            server_path
        ));
    }

    // Ensure standard subdirectories exist
    ensure_server_subdirs(&server_path).context(format!(
        "Failed to ensure subdirectories for server '{name}'"
    ))?;

    // Load the configuration
    let config_path = server_path.join("server.json");
    let config = load_server_config(&config_path)
        .context(format!("Failed to load config for server '{name}'"))?;

    Ok(Server {
        name: name.to_string(),
        path: server_path,
        config,
    })
}

/// Updates the configuration of an existing server.
///
/// Finds the server by name, validates the new configuration, and overwrites
/// the existing `server.json` file.
///
/// # Arguments
///
/// * `name` - The name of the server to update.
/// * `new_config` - The `ServerConfig` containing the updated settings.
///
/// # Errors
///
/// Returns an error if:
/// * The server with the given `name` cannot be found.
/// * The path found is not a directory.
/// * The `new_config` fails validation.
/// * The `server.json` file cannot be written.
pub fn update_server(name: &str, new_config: ServerConfig) -> Result<Server> {
    // Validate the new configuration first
    new_config.validate().context(format!(
        "Invalid new configuration provided for server '{name}'"
    ))?;

    let smudgy_dir = get_smudgy_home()?;
    let server_path = smudgy_dir.join(name);

    // Ensure the server directory exists and is a directory
    if !server_path.exists() {
        return Err(anyhow::anyhow!("Server '{}' not found for update", name))
            .with_context(|| format!("Looked for directory: {server_path:?}"));
    }
    if !server_path.is_dir() {
        return Err(anyhow::anyhow!(
            "Path for server '{}' exists but is not a directory: {:?}",
            name,
            server_path
        ));
    }

    // Construct path to server.json
    let config_path = server_path.join("server.json");

    // Serialize the new config
    let config_json = serde_json::to_string_pretty(&new_config).context(format!(
        "Failed to serialize updated config for server '{name}'"
    ))?;

    // Write the new config, overwriting the old one
    fs::write(&config_path, config_json).context(format!(
        "Failed to write updated server.json for server '{name}' at {config_path:?}"
    ))?;

    // Return the server representation with the new config
    Ok(Server {
        name: name.to_string(),
        path: server_path,
        config: new_config, // Use the validated new_config
    })
}

/// Deletes a server and all its associated data.
///
/// Finds the server directory by name and removes it recursively.
/// If the server directory does not exist, the function succeeds silently.
///
/// # Arguments
///
/// * `name` - The name of the server to delete.
///
/// # Errors
///
/// Returns an error if:
/// * The smudgy home directory cannot be accessed.
/// * A file exists with the server name (instead of a directory).
/// * The directory or its contents cannot be removed due to permissions or other I/O issues.
pub fn delete_server(name: &str) -> Result<()> {
    let smudgy_dir = get_smudgy_home()?;
    let server_path = smudgy_dir.join(name);

    if server_path.exists() {
        // Check if it's actually a directory before attempting recursive delete
        if !server_path.is_dir() {
            return Err(anyhow::anyhow!(
                "Cannot delete server '{}': Path exists but is not a directory: {:?}",
                name,
                server_path
            ));
        }

        // Recursively remove the directory
        fs::remove_dir_all(&server_path).context(format!(
            "Failed to delete directory for server '{name}' at {server_path:?}"
        ))?;
    } else {
        // Optionally log that the server didn't exist? For now, silent success.
        println!("Info: Server '{name}' not found for deletion.");
    }

    Ok(())
}
