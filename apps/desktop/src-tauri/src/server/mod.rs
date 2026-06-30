//! Phase 0: axum HTTP + WebSocket server. Serves the embedded overlay bundle to
//! OBS and broadcasts a heartbeat snapshot so the overlay can prove the pipe.
//!
//! ponytail: fixed port + 1Hz heartbeat for now. Port-scan-on-conflict, the full
//! battle Snapshot, and the coalesced 100ms vote broadcast arrive in Phase 1.

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use rust_embed::RustEmbed;
use std::{net::SocketAddr, time::Duration};
use tokio::sync::broadcast;

pub const PORT: u16 = 31337;

/// The overlay's built bundle, embedded at compile time → single self-contained binary.
/// `apps/overlay` must be built before the Rust crate compiles (the desktop `build`
/// script and `beforeBuildCommand` both run the overlay build first).
#[derive(RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/../../overlay/dist"]
struct OverlayAssets;

#[derive(Clone)]
struct ServerState {
    tx: broadcast::Sender<String>,
}

pub async fn serve(port: u16) -> anyhow::Result<()> {
    let (tx, _rx) = broadcast::channel::<String>(64);
    spawn_heartbeat(tx.clone());

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .fallback(static_handler)
        .with_state(ServerState { tx });

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("overlay server on http://{addr}/");
    axum::serve(listener, app).await?;
    Ok(())
}

// Phase 0 proof-of-pipe: emit an incrementing counter once a second.
fn spawn_heartbeat(tx: broadcast::Sender<String>) {
    tokio::spawn(async move {
        let mut seq: u64 = 0;
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        loop {
            tick.tick().await;
            seq += 1;
            let _ = tx.send(format!("{{\"seq\":{seq},\"counter\":{seq}}}"));
        }
    });
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<ServerState>) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: ServerState) {
    let mut rx = state.tx.subscribe();
    // Phase 1: send a full snapshot here on connect so a fresh OBS scene is instantly correct.
    while let Ok(msg) = rx.recv().await {
        if socket.send(Message::Text(msg.into())).await.is_err() {
            break;
        }
    }
}

async fn static_handler(uri: axum::http::Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    if let Some(content) = OverlayAssets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        return (
            [(axum::http::header::CONTENT_TYPE, mime.as_ref())],
            content.data.into_owned(),
        )
            .into_response();
    }

    // SPA fallback → index.html.
    match OverlayAssets::get("index.html") {
        Some(c) => (
            [(axum::http::header::CONTENT_TYPE, "text/html")],
            c.data.into_owned(),
        )
            .into_response(),
        None => (axum::http::StatusCode::NOT_FOUND, "overlay not built").into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::OverlayAssets;
    use rust_embed::RustEmbed;

    // Proves the Phase 0 pipe wiring: the overlay bundle is compiled into the binary,
    // so axum can serve it to OBS with no external files.
    #[test]
    fn overlay_bundle_is_embedded() {
        assert!(
            OverlayAssets::get("index.html").is_some(),
            "overlay index.html must be embedded — run `pnpm --filter @sb/overlay build` first"
        );
    }
}
