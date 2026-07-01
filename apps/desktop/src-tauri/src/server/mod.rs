//! axum HTTP + WebSocket server. Serves the embedded overlay bundle to OBS and
//! relays the coalesced `Snapshot` stream produced by the broadcaster. New WS
//! clients get a full snapshot immediately on connect.

use crate::error::AppResult;
use crate::providers::kick_official;
use crate::state::AppState;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use rust_embed::RustEmbed;
use serde::Deserialize;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio::sync::broadcast::error::RecvError;

pub const PORT: u16 = 31337;

/// The overlay's built bundle, embedded at compile time → single self-contained
/// binary. `apps/overlay` must be built before the crate compiles.
#[derive(RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/../../overlay/dist"]
struct OverlayAssets;

/// Bind `127.0.0.1:port` and serve.
pub async fn run_server(port: u16, state: AppState) -> AppResult<()> {
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], port))).await?;
    serve_on(listener, state).await
}

/// Serve on an already-bound listener (lets tests inject an ephemeral port).
// ponytail: bound to loopback only. A `/ws` Origin/Host allowlist is deferred to
// a later phase (overlay is local-only for now).
pub async fn serve_on(listener: TcpListener, state: AppState) -> AppResult<()> {
    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/oauth/callback", get(oauth_callback))
        .fallback(static_handler)
        .with_state(state);
    tracing::info!("overlay server on http://{}/", listener.local_addr()?);
    axum::serve(listener, app).await?;
    Ok(())
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    // Subscribe BEFORE sending the hello frame so no update can slip through the
    // gap between connect and subscribe.
    let mut rx = state.subscribe();
    if socket
        .send(Message::Text(state.current_snapshot_json().into()))
        .await
        .is_err()
    {
        return;
    }
    loop {
        match rx.recv().await {
            Ok(msg) => {
                if socket.send(Message::Text(msg.into())).await.is_err() {
                    break; // client gone
                }
            }
            // A slow client fell behind the broadcast buffer: drop it (its own
            // task), which never stalls the broadcaster or other clients.
            Err(RecvError::Lagged(n)) => {
                tracing::warn!("overlay ws client lagged {n} frames; dropping");
                break;
            }
            Err(RecvError::Closed) => break,
        }
    }
}

#[derive(Debug, Deserialize)]
struct OauthCallbackParams {
    code: Option<String>,
    state: Option<String>,
    /// Kick sends this instead of `code` when the user denies consent.
    error: Option<String>,
}

/// The official Kick OAuth 2.1 + PKCE loopback redirect target (K1). Exchanges
/// the code for tokens, persists them, and notifies the UI via a `kick-auth`
/// Tauri event. Never panics — always renders small HTML so the popup tab
/// shows something sane even on failure.
async fn oauth_callback(
    Query(params): Query<OauthCallbackParams>,
    State(state): State<AppState>,
) -> Response {
    if let Some(err) = params.error {
        tracing::warn!("kick oauth: provider returned error={err}");
        return oauth_html("Kick login failed (denied or errored) — you can close this tab.")
            .into_response();
    }
    let (Some(code), Some(csrf_state)) = (params.code, params.state) else {
        return oauth_html("Kick login failed: missing code/state — you can close this tab.")
            .into_response();
    };
    let Some(verifier) = state.take_oauth(&csrf_state) else {
        tracing::warn!("kick oauth: callback with an unknown or expired state");
        return (
            axum::http::StatusCode::BAD_REQUEST,
            oauth_html("Kick login failed: expired session — please try logging in again."),
        )
            .into_response();
    };

    let creds = match state.get_kick_auth().await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("kick oauth: failed to read stored credentials: {e}");
            return oauth_html("Kick login failed: internal error — you can close this tab.")
                .into_response();
        }
    };
    let (Some(client_id), Some(client_secret)) = (creds.client_id, creds.client_secret) else {
        tracing::error!("kick oauth: no client credentials stored before callback");
        return oauth_html(
            "Kick login failed: no credentials configured — you can close this tab.",
        )
        .into_response();
    };

    let redirect_uri = kick_official::redirect_uri();
    let tokens = match kick_official::exchange_code(
        &client_id,
        &client_secret,
        &code,
        &redirect_uri,
        &verifier,
    )
    .await
    {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("kick oauth: token exchange failed: {e}");
            return oauth_html("Kick login failed: token exchange error — you can close this tab.")
                .into_response();
        }
    };

    let expires_at = kick_official::expiry_from(tokens.expires_in);
    if let Err(e) = state
        .set_kick_tokens(tokens.access_token, tokens.refresh_token, expires_at)
        .await
    {
        tracing::error!("kick oauth: failed to persist tokens: {e}");
        return oauth_html("Kick login failed: could not save tokens — you can close this tab.")
            .into_response();
    }

    // K2: create the events:subscribe webhook subscription here and persist
    // its id via state.set_kick_subscription(...).

    state.emit_event("kick-auth");
    tracing::info!("kick oauth: connected");
    oauth_html("Kick connected — you can close this tab.").into_response()
}

fn oauth_html(msg: &str) -> Html<String> {
    Html(format!(
        "<html><body style=\"font-family:sans-serif;text-align:center;margin-top:3rem\"><p>{msg}</p></body></html>"
    ))
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
    use super::*;
    use futures_util::StreamExt;
    use tokio_tungstenite::tungstenite::Message as TMessage;

    #[test]
    fn overlay_bundle_is_embedded() {
        assert!(
            OverlayAssets::get("index.html").is_some(),
            "overlay index.html must be embedded — run `pnpm --filter @sb/overlay build` first"
        );
    }

    fn seq_of(frame: &str) -> u64 {
        serde_json::from_str::<serde_json::Value>(frame)
            .unwrap()
            .get("seq")
            .and_then(serde_json::Value::as_u64)
            .unwrap()
    }

    #[tokio::test]
    async fn ws_streams_snapshots_with_increasing_seq() {
        let state = AppState::test();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let srv = state.clone();
        tokio::spawn(async move {
            let _ = serve_on(listener, srv).await;
        });

        let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/ws"))
            .await
            .unwrap();

        // hello frame proves we're subscribed; broadcasts after it can't be lost.
        let hello = next_text(&mut ws).await;
        let s0 = seq_of(&hello);

        state.broadcast();
        let f1 = next_text(&mut ws).await;
        let s1 = seq_of(&f1);

        state.broadcast();
        let f2 = next_text(&mut ws).await;
        let s2 = seq_of(&f2);

        assert!(s1 > s0, "{s1} > {s0}");
        assert!(s2 > s1, "{s2} > {s1}");
    }

    #[tokio::test]
    async fn slow_client_is_dropped_without_stalling_broadcaster() {
        use std::time::Duration;
        use tokio::time::timeout;

        let state = AppState::test();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let srv = state.clone();
        tokio::spawn(async move {
            let _ = serve_on(listener, srv).await;
        });

        // Slow client: reads the hello (proving it subscribed), then never reads.
        let (mut slow, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/ws"))
            .await
            .unwrap();
        let _ = next_text(&mut slow).await;

        // Flood far past the broadcast capacity. These are synchronous — if the
        // broadcaster could stall on a slow client, this loop (and test) hangs.
        for _ in 0..500 {
            state.broadcast();
        }

        // The lagging client's WS is dropped, so reading it terminates promptly
        // instead of hanging.
        let closed = timeout(Duration::from_secs(5), async {
            loop {
                match slow.next().await {
                    None | Some(Err(_)) | Some(Ok(TMessage::Close(_))) => return true,
                    Some(Ok(_)) => continue, // drain any buffered frames first
                }
            }
        })
        .await
        .expect("slow client should be dropped, not block the broadcaster");
        assert!(closed);

        // A fresh client still receives new broadcasts with increasing seq —
        // proving the broadcaster kept running and serves other clients.
        let (mut fast, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/ws"))
            .await
            .unwrap();
        let s0 = seq_of(&next_text(&mut fast).await);
        state.broadcast();
        let next = timeout(Duration::from_secs(5), next_text(&mut fast))
            .await
            .expect("fast client keeps receiving");
        assert!(seq_of(&next) > s0);
    }

    async fn next_text(
        ws: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    ) -> String {
        loop {
            match ws.next().await.expect("a frame").expect("ok frame") {
                TMessage::Text(t) => return t.to_string(),
                _ => continue,
            }
        }
    }

    // ── /oauth/callback (K1) ─────────────────────────────────────────────────
    // No real token exchange is exercised here (that needs Kick's live token
    // endpoint) — these cover the request-validation branches that run before
    // any network call, which is where a malformed/replayed/expired callback
    // must fail safely instead of panicking.

    #[tokio::test]
    async fn oauth_callback_rejects_unknown_or_expired_state_with_400() {
        let state = AppState::test();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = serve_on(listener, state).await;
        });

        // No matching start_oauth() was ever called — any state is "unknown".
        let resp = crate::net::shared()
            .get(format!(
                "http://{addr}/oauth/callback?code=abc123&state=not-pending"
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), reqwest::StatusCode::BAD_REQUEST);
        let body = resp.text().await.unwrap();
        assert!(body.contains("expired session"), "{body}");
    }

    #[tokio::test]
    async fn oauth_callback_handles_missing_params_without_panicking() {
        let state = AppState::test();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = serve_on(listener, state).await;
        });

        let resp = crate::net::shared()
            .get(format!("http://{addr}/oauth/callback"))
            .send()
            .await
            .unwrap();
        assert!(resp.status().is_success());
        let body = resp.text().await.unwrap();
        assert!(body.contains("missing code/state"), "{body}");
    }

    #[tokio::test]
    async fn oauth_callback_surfaces_provider_denial() {
        let state = AppState::test();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = serve_on(listener, state).await;
        });

        let resp = crate::net::shared()
            .get(format!("http://{addr}/oauth/callback?error=access_denied"))
            .send()
            .await
            .unwrap();
        assert!(resp.status().is_success());
        let body = resp.text().await.unwrap();
        assert!(body.contains("denied or errored"), "{body}");
    }

    #[tokio::test]
    async fn oauth_callback_rejects_valid_state_with_no_stored_credentials() {
        // A matching state passes the CSRF gate, but nothing ever stored a
        // client_id/secret → the exchange must be refused, not attempted.
        let state = AppState::test();
        state.start_oauth("verifier".into(), "state-ok".into());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let srv = state.clone();
        tokio::spawn(async move {
            let _ = serve_on(listener, srv).await;
        });

        let resp = crate::net::shared()
            .get(format!(
                "http://{addr}/oauth/callback?code=abc&state=state-ok"
            ))
            .send()
            .await
            .unwrap();
        assert!(resp.status().is_success());
        let body = resp.text().await.unwrap();
        assert!(body.contains("no credentials configured"), "{body}");
    }

    #[tokio::test]
    async fn oauth_callback_exchanges_code_and_persists_tokens() {
        use axum::{routing::post, Json};

        // Mock Kick token endpoint — returns a canned token response.
        let token_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let token_addr = token_listener.local_addr().unwrap();
        tokio::spawn(async move {
            let app = Router::new().route(
                "/oauth/token",
                post(|| async {
                    Json(serde_json::json!({
                        "access_token": "AT-live",
                        "refresh_token": "RT-live",
                        "expires_in": 3600
                    }))
                }),
            );
            let _ = axum::serve(token_listener, app).await;
        });
        kick_official::set_token_url_for_test(format!("http://{token_addr}/oauth/token"));

        // App has creds stored and a pending login matching the callback state.
        let state = AppState::test();
        state
            .set_kick_creds("cid".into(), "csecret".into())
            .await
            .unwrap();
        state.start_oauth("verifier-xyz".into(), "state-xyz".into());

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let srv = state.clone();
        tokio::spawn(async move {
            let _ = serve_on(listener, srv).await;
        });

        let resp = crate::net::shared()
            .get(format!(
                "http://{addr}/oauth/callback?code=the-code&state=state-xyz"
            ))
            .send()
            .await
            .unwrap();
        assert!(resp.status().is_success());
        assert!(resp.text().await.unwrap().contains("Kick connected"));

        // The exchanged tokens landed in the DB.
        let auth = state.get_kick_auth().await.unwrap();
        assert_eq!(auth.access_token.as_deref(), Some("AT-live"));
        assert_eq!(auth.refresh_token.as_deref(), Some("RT-live"));
    }
}
