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
    dir.push("smudgy");

    fs::create_dir_all(&dir).context(format!(
        "Failed to create smudgy directory at {}",
        dir.to_string_lossy()
    ))?;

    Ok(dir)
}

/// Initialize logging configuration.
/// 
/// In debug builds, uses pretty_env_logger for colorized console output.
/// In release builds, logs to a file named "smudgy.log" in the smudgy home directory
/// with timestamp information.
///
/// # Errors
///
/// Returns an error if logging initialization fails or if the log file cannot be created
/// in release builds.
fn init_logging() -> Result<()> {
    // Set default log level if not specified
    if std::env::var("SMUDGY_LOG").is_err() {
        // This only needs to be wrapped with unsafe because it isn't thread-safe; 
        // this is ok because we're only going to use this once, on the current thread
        unsafe {
            std::env::set_var("SMUDGY_LOG", "debug");
        }
    }

    #[cfg(debug_assertions)]
    {
        // Debug build: use pretty console logger
        pretty_env_logger::try_init_timed_custom_env("SMUDGY_LOG")
            .context("Failed to initialize pretty logger")?;
    }

    #[cfg(not(debug_assertions))]
    {
        // Release build: use file logger
        use simplelog::*;
        use std::fs::File;

        let log_level = match std::env::var("SMUDGY_LOG").unwrap_or_else(|_| "debug".to_string()).to_lowercase().as_str() {
            "trace" => LevelFilter::Trace,
            "debug" => LevelFilter::Debug,
            "info" => LevelFilter::Info,
            "warn" => LevelFilter::Warn,
            "error" => LevelFilter::Error,
            _ => LevelFilter::Debug,
        };

        let smudgy_home = get_smudgy_home()
            .context("Failed to get smudgy home directory for logging")?;
        let log_file_path = smudgy_home.join("smudgy.log");
        
        let log_file = File::create(&log_file_path)
            .context(format!("Failed to create log file at {}", log_file_path.display()))?;

        WriteLogger::init(
            log_level,
            Config::default(),
            log_file,
        ).context("Failed to initialize file logger")?;
    }

    Ok(())
}

pub fn init() {
    // Initialize logging
    if let Err(e) = init_logging() {
        eprintln!("Failed to initialize logging: {}", e);
        // Continue execution even if logging fails
    }

    info!(
        "smudgy started; version {} ({}, built on {})",
        env!("SMUDGY_BUILD_NAME"),
        env!("CARGO_PKG_VERSION"),
        build_time::build_time_local!("%Y-%m-%d %H:%M:%S")
    );

    deno_core::JsRuntime::init_platform(None, false);
    trace!(
        "deno initialized, v8 version {}",
        deno_core::v8::VERSION_STRING
    );
}

pub mod models;
pub mod session;
pub mod terminal_buffer;
