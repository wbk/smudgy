use crate::get_smudgy_home;
use anyhow::{Context, Result};
use std::{fs, io, path::PathBuf};

/// Represents a discovered module file within a server's `modules` directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleFile {
    /// The name of the module file (e.g., "`auto_healer.ts`").
    pub name: String,
    /// The full path to the module file.
    pub path: PathBuf,
    // TODO: Add language detection (e.g., based on extension) if needed?
}

/// Lists all files found directly within a server's `modules` directory.
///
/// This function does not recurse into subdirectories within `modules`.
/// It simply lists the files present at the top level of the `modules` folder.
///
/// # Arguments
///
/// * `server_name` - The name of the server whose modules should be listed.
///
/// # Errors
///
/// Returns an error if the server or modules directory cannot be accessed.
/// If the `modules` directory doesn't exist, an empty list is returned successfully.
pub fn list_modules(server_name: &str) -> Result<Vec<ModuleFile>> {
    let smudgy_dir = get_smudgy_home()?;
    let modules_dir = smudgy_dir.join(server_name).join("modules");

    let mut module_files = Vec::new();

    match fs::read_dir(&modules_dir) {
        Ok(entries) => {
            for entry_result in entries {
                match entry_result {
                    Ok(entry) => {
                        let path = entry.path();
                        // Check if it's a file and get its name
                        if path.is_file() {
                            if let Some(name_osstr) = path.file_name() {
                                if let Some(name_str) = name_osstr.to_str() {
                                    module_files.push(ModuleFile {
                                        name: name_str.to_string(),
                                        path: path.clone(),
                                    });
                                } else {
                                    // Log or handle non-UTF8 filenames if necessary
                                    eprintln!("Warning: Skipping module file with non-UTF8 name in server '{server_name}': {path:?}");
                                }
                            } else {
                                // Path terminates in .. or similar? Should be rare for files.
                                eprintln!("Warning: Skipping module file with no filename component in server '{server_name}': {path:?}");
                            }
                        }
                        // Ignore directories within the modules folder for now
                    }
                    Err(e) => {
                        // Log warning: Failed to read directory entry
                        eprintln!("Warning: Failed to read directory entry in modules for server '{server_name}': {e}");
                    }
                }
            }
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            // If the modules dir doesn't exist for this server, return empty list.
            // This is not an error condition.
        }
        Err(e) => {
            // Other errors reading the modules dir are propagated.
            return Err(e).context(format!(
                "Failed to read modules directory for server '{}' at {}",
                server_name,
                modules_dir.to_string_lossy()
            ));
        }
    }

    Ok(module_files)
} 