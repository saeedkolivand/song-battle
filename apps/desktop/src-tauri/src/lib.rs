mod server;

// IPC: dashboard → Rust. Phase 0 proves the round-trip; battle/song/match control
// verbs land in Phase 1 under commands/.
#[tauri::command]
fn ping() -> String {
    "pong from Rust".into()
}

#[tauri::command]
fn overlay_url() -> String {
    format!("http://localhost:{}/", server::PORT)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|_app| {
            // Spawn the axum overlay server on Tauri's tokio runtime.
            tauri::async_runtime::spawn(async {
                if let Err(e) = server::serve(server::PORT).await {
                    tracing::error!("overlay server failed: {e}");
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![ping, overlay_url])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
