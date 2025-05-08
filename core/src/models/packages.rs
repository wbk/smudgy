use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::{fs, io};

/// Helper function for serde to default boolean fields to true.
fn default_true() -> bool {
    true
}

/// Represents a node in the package hierarchy.
///
/// Each node corresponds to a package name (like "combat" or "defense").
/// It stores its own enabled status and any child packages.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct PackageNode {
    /// Whether this specific package is enabled.
    /// If false, all items and sub-packages within it are implicitly disabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// A map of child package names to their corresponding `PackageNode` definitions.
    #[serde(default)] // Default to an empty map if missing
    pub children: HashMap<String, PackageNode>,
}

/// Represents the entire package hierarchy for a server, loaded from `packages.json`.
///
/// This is a map from top-level package names to their `PackageNode` definitions.
pub type PackageTree = HashMap<String, PackageNode>;

use crate::get_smudgy_home;
use anyhow::{Context, Result};

/// Loads the package hierarchy definition from `packages.json` for a given server.
///
/// If `packages.json` does not exist, returns an empty `PackageTree` successfully.
///
/// # Arguments
///
/// * `server_name` - The name of the server whose package tree should be loaded.
///
/// # Errors
///
/// Returns an error if the server directory cannot be accessed, or if `packages.json`
/// exists but cannot be read or parsed.
pub fn load_packages(server_name: &str) -> Result<PackageTree> {
    let smudgy_dir = get_smudgy_home()?;
    let server_path = smudgy_dir.join(server_name);
    let packages_path = server_path.join("packages.json");

    match fs::read_to_string(&packages_path) {
        Ok(content) => {
            let tree: PackageTree = serde_json::from_str(&content)
                .context(format!("Failed to parse packages.json for server '{server_name}'"))?;
            Ok(tree)
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            // File not found is okay, just return an empty tree
            Ok(PackageTree::new())
        }
        Err(e) => {
            // Other read errors are propagated
            Err(e).context(format!("Failed to read packages.json for server '{server_name}'"))
        }
    }
}

/// Saves the package hierarchy definition to `packages.json` for a given server.
///
/// This will overwrite the existing file if it exists.
///
/// # Arguments
///
/// * `server_name` - The name of the server whose package tree should be saved.
/// * `tree` - The `PackageTree` data structure to save.
///
/// # Errors
///
/// Returns an error if the server directory cannot be accessed, or if `packages.json`
/// cannot be written.
pub fn save_packages(server_name: &str, tree: &PackageTree) -> Result<()> {
    let smudgy_dir = get_smudgy_home()?;
    let server_path = smudgy_dir.join(server_name);

    // Ensure the server directory exists (optional, but good practice)
    if !server_path.is_dir() {
        return Err(anyhow::anyhow!(
            "Server directory not found or not a directory: {:?}",
            server_path
        ));
    }

    let packages_path = server_path.join("packages.json");

    let json_content = serde_json::to_string_pretty(tree)
        .context(format!("Failed to serialize package tree for server '{server_name}'"))?;

    fs::write(&packages_path, json_content).context(format!(
        "Failed to write packages.json for server '{server_name}' at {packages_path:?}"
    ))?;

    Ok(())
}

/// Checks if a given package path is effectively enabled within the hierarchy.
///
/// A package path is effectively enabled if all package nodes along the path
/// (from the root down to the final component) have their `enabled` flag set to true.
/// An empty path or a path resolving to the root is considered enabled.
///
/// # Arguments
///
/// * `path_str` - The package path string (e.g., "combat/defense" or "utility").
///                An empty string signifies the root.
/// * `tree` - The loaded `PackageTree`.
///
/// # Returns
///
/// Returns `true` if the package path is effectively enabled, `false` otherwise.
/// Returns `false` if the path is invalid or doesn't exist in the tree.
#[must_use] pub fn is_package_effectively_enabled(path_str: &str, tree: &PackageTree) -> bool {
    let components: Vec<&str> = if path_str.is_empty() {
        vec![]
    } else {
        path_str.split('/').collect()
    };

    let mut current_level: &HashMap<String, PackageNode> = tree;

    for component in components {
        if component.is_empty() { // Handle accidental double slashes like "combat//defense"
            continue;
        }
        match current_level.get(component) {
            Some(node) => {
                if !node.enabled {
                    return false; // Found a disabled node along the path
                }
                // Move down to the children map for the next component
                current_level = &node.children;
            }
            None => {
                // Path component doesn't exist in the tree
                return false;
            }
        }
    }

    // If we traversed the whole path without finding a disabled node
    true
}

// Functions for loading, saving, and querying the package tree will go here. 