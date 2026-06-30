// AppError aggregates large library errors (reqwest/rusqlite/tungstenite) via
// `#[from]`; boxing each would cost the `?` ergonomics for a perf lint that
// doesn't matter on these cold error paths.
#![allow(clippy::result_large_err)]

// ponytail: supply-chain CI (deny.toml + cargo-audit / cargo-deny) is deferred to
// Phase 8 — add it when the dependency surface stabilizes.

mod commands;
mod db;
mod domain;
mod error;
mod media;
mod net;
mod observability;
mod platform;
mod providers;
mod server;
mod state;

use error::AppResult;
use state::AppState;
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

// IPC: dashboard → Rust. Phase 0 round-trip probes; the battle/song/match verbs
// live under `commands/`.
#[tauri::command]
fn ping() -> String {
    "pong from Rust".into()
}

#[tauri::command]
fn overlay_url() -> String {
    format!("http://localhost:{}/", server::PORT)
}

/// Open (or focus) a fullscreen window showing the overlay — local preview / the
/// F hotkey. `overlay_url` stays for OBS / external browsers.
#[tauri::command]
fn open_overlay_window(app: tauri::AppHandle) -> AppResult<()> {
    if let Some(w) = app.get_webview_window("overlay") {
        w.set_focus()?;
        return Ok(());
    }
    let url = url::Url::parse(&format!("http://localhost:{}/", server::PORT))?;
    WebviewWindowBuilder::new(&app, "overlay", WebviewUrl::External(url))
        .title("Song Battle Overlay")
        .fullscreen(true)
        .build()?;
    Ok(())
}

/// Open the on-disk DB, falling back to in-memory so the app still launches.
fn open_db() -> rusqlite::Connection {
    let opened = platform::app_data_dir().and_then(|dir| db::open(&dir.join("songbattle.db")));
    match opened {
        Ok(conn) => conn,
        Err(e) => {
            tracing::error!("db open failed ({e}); using in-memory (no persistence)");
            db::open_in_memory().expect("in-memory db")
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    observability::init();

    let conn = open_db();
    let restored: Option<domain::battle::Battle> = db::load_latest(&conn).unwrap_or_else(|e| {
        tracing::error!("failed to load battle: {e}");
        None
    });
    let state = AppState::new(conn);
    if let Some(b) = restored {
        state.set_battle(b);
    }

    let managed = state.clone();
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(managed)
        .setup(move |app| {
            state.set_app_handle(app.handle().clone());
            let server_state = state.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = server::run_server(server::PORT, server_state).await {
                    tracing::error!("overlay server failed: {e}");
                }
            });
            state::spawn_broadcaster(state.clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ping,
            overlay_url,
            open_overlay_window,
            commands::battle::create_battle,
            commands::battle::generate_bracket,
            commands::battle::start_match,
            commands::battle::reset_votes,
            commands::battle::skip_match,
            commands::battle::set_timer,
            commands::song::import_song,
            commands::song::remove_song,
            commands::song::shuffle_songs,
            commands::kick::connect_kick,
            commands::kick::disconnect_kick,
            commands::io::get_snapshot,
            commands::io::export_json,
            commands::io::import_json,
            commands::tournaments::list_battles,
            commands::tournaments::load_battle,
            commands::tournaments::delete_battle,
            commands::settings::get_settings,
            commands::settings::set_anonymous,
            commands::settings::set_default_timer,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
