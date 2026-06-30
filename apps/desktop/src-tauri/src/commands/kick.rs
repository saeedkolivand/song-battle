use crate::domain::snapshot::ConnectionState;
use crate::domain::vote::parse_vote;
use crate::error::AppResult;
use crate::providers::kick::{validate_channel, KickProvider};
use crate::providers::{now_ms, ChatProvider, ProviderEvent};
use crate::state::AppState;
use tauri::State;

/// Bounded so a chat flood can't grow memory; the provider drops events on full.
const EVENT_CHANNEL_CAP: usize = 1024;

#[tauri::command]
pub async fn connect_kick(channel: String, state: State<'_, AppState>) -> AppResult<()> {
    validate_channel(&channel)?; // reject bad slugs before spawning anything
    state.stop_kick(); // drop any previous connection
    state.set_kick(ConnectionState::Connecting, Some(channel.clone()));
    state.mark_dirty();

    let (tx, mut rx) = tokio::sync::mpsc::channel::<ProviderEvent>(EVENT_CHANNEL_CAP);
    let provider = KickProvider::new(channel);
    let run = tokio::spawn(async move {
        let _ = provider.run(tx).await;
    });

    let app = state.inner().clone();
    let consume = tokio::spawn(async move {
        while let Some(ev) = rx.recv().await {
            match ev {
                ProviderEvent::Connection(s) => {
                    app.set_kick_state(s);
                    app.mark_dirty();
                }
                ProviderEvent::Message(m) => {
                    if let Some(choice) = parse_vote(&m.text) {
                        if app.cast_vote(m.user.user_id, choice, now_ms()) {
                            app.mark_dirty();
                        }
                    }
                }
            }
        }
    });

    state.set_kick_tasks(vec![run, consume]);
    Ok(())
}

#[tauri::command]
pub async fn disconnect_kick(state: State<'_, AppState>) -> AppResult<()> {
    state.stop_kick();
    state.set_kick(ConnectionState::Disconnected, None);
    state.mark_dirty();
    Ok(())
}
