use std::{fs, path::{Path, PathBuf}, sync::LazyLock};

use anyhow::Context;

static SMUDGY_HOME: LazyLock<PathBuf> = LazyLock::new(|| {
    let mut dir = dirs::document_dir().unwrap();
    dir.push("smudgy");
    fs::create_dir_all(dir.clone()).context(format!("Failed to create {}, bailing", dir.to_string_lossy())).unwrap();
    dir
});
