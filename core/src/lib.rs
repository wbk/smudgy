// Compile-time check for regex backend features
#[cfg(all(feature = "regex-backend-regex", feature = "regex-backend-hyperscan"))]
compile_error!(
    "Features 'regex-backend-regex' and 'regex-backend-hyperscan' are mutually exclusive. Please enable only one."
);

#[cfg(not(any(feature = "regex-backend-regex", feature = "regex-backend-hyperscan")))]
compile_error!(
    "Exactly one regex backend feature ('regex-backend-regex' or 'regex-backend-hyperscan') must be enabled."
);

use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

#[macro_use]
extern crate log;

/// Returns the path to the smudgy home directory, creating it if it doesn't exist.
///
/// # Errors
///
/// Returns an error if the user's document directory cannot be determined or if the
/// smudgy directory cannot be created.
pub fn get_smudgy_home() -> Result<PathBuf> {
    let mut dir = dirs::document_dir().context("Failed to get user document directory")?;
    dir.push("iced-smudgy");

    fs::create_dir_all(&dir).context(format!(
        "Failed to create smudgy directory at {}",
        dir.to_string_lossy()
    ))?;

    Ok(dir)
}

pub fn init() {
    // Define settings for the application window
    if std::env::var("SMUDGY_LOG").is_err() {
        // This only needs to be wrapped with unsafe because it isn't thread-safe; this is ok because we're only going to use this once, on the current thread
        unsafe {
            std::env::set_var("SMUDGY_LOG", "debug");
        }
    }

    info!(
        "smudgy started; version {} ({}, built on {})",
        env!("SMUDGY_BUILD_NAME"),
        env!("CARGO_PKG_VERSION"),
        build_time::build_time_local!("%Y-%m-%d %H:%M:%S")
    );

    pretty_env_logger::init_custom_env("SMUDGY_LOG");

    deno_core::JsRuntime::init_platform(None, false);
    trace!(
        "deno initialized, v8 version {}",
        deno_core::v8::VERSION_STRING
    );
}

pub mod models;
pub mod session;
