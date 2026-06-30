//! Tauri command handlers — thin wrappers: lock state, mutate, persist, fan out.
//! One module per area. All return `AppResult<_>`.

pub mod battle;
pub mod io;
pub mod kick;
pub mod settings;
pub mod song;
pub mod tournaments;
