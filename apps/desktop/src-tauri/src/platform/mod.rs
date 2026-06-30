//! The ONLY place that reads env / resolves the OS data dir. `std::env` is
//! banned everywhere else.

use crate::error::{AppError, AppResult};
use std::path::PathBuf;

/// `<os-data-dir>/SongBattle`, created if missing.
pub fn app_data_dir() -> AppResult<PathBuf> {
    let dir = data_root()?.join("SongBattle");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

// ponytail: tiny per-OS mirrors instead of pulling the `dirs` crate. These are
// `#[cfg]`-gated so only the host's branch compiles — keep them trivial.
#[cfg(target_os = "windows")]
fn data_root() -> AppResult<PathBuf> {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .ok_or_else(|| AppError::Other("APPDATA not set".into()))
}

#[cfg(target_os = "macos")]
fn data_root() -> AppResult<PathBuf> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| AppError::Other("HOME not set".into()))?;
    Ok(home.join("Library/Application Support"))
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn data_root() -> AppResult<PathBuf> {
    if let Some(x) = std::env::var_os("XDG_DATA_HOME") {
        return Ok(PathBuf::from(x));
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| AppError::Other("HOME not set".into()))?;
    Ok(home.join(".local/share"))
}
