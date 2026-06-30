use crate::domain::battle::Battle;
use crate::domain::snapshot::ConnectionState;
use crate::domain::vote::{classify_chat, ChatAction};
use crate::error::AppResult;
use crate::providers::kick::{validate_channel, KickProvider};
use crate::providers::{now_ms, ChatProvider, ProviderEvent};
use crate::state::AppState;
use std::sync::Arc;
use tauri::State;
use tokio::sync::Semaphore;

/// Bounded so a chat flood can't grow memory; the provider drops events on full.
const EVENT_CHANNEL_CAP: usize = 1024;
/// Cap concurrent oEmbed fetches spawned off the chat loop.
const SUBMIT_FETCH_CONCURRENCY: usize = 6;

#[tauri::command]
pub async fn connect_kick(
    channel: String,
    chatroom_id: Option<i64>,
    state: State<'_, AppState>,
) -> AppResult<()> {
    validate_channel(&channel)?; // reject bad slugs before spawning anything
    state.stop_kick(); // drop any previous connection
    state.clear_submit_ledger(); // fresh per-user submit quotas this session
    state.set_kick(ConnectionState::Connecting, Some(channel.clone()));
    state.mark_dirty();

    let (tx, mut rx) = tokio::sync::mpsc::channel::<ProviderEvent>(EVENT_CHANNEL_CAP);
    let provider = KickProvider::new(channel, chatroom_id);
    let run = tokio::spawn(async move {
        let _ = provider.run(tx).await;
    });

    let app = state.inner().clone();
    let fetch_sem = Arc::new(Semaphore::new(SUBMIT_FETCH_CONCURRENCY));
    let consume = tokio::spawn(async move {
        while let Some(ev) = rx.recv().await {
            match ev {
                ProviderEvent::Connection(s) => {
                    app.set_kick_state(s);
                    app.mark_dirty();
                }
                ProviderEvent::Message(m) => {
                    let who = m.user.username.clone();
                    tracing::info!("kick chat: {who} -> '{}'", m.text);
                    match classify_chat(m.user.is_mod, &m.text) {
                    ChatAction::Vote(choice) => {
                        // counted=false means no match is Active / its timer isn't running.
                        let counted = app.cast_vote(m.user.user_id, choice, now_ms());
                        tracing::info!("kick vote from {who}: counted={counted}");
                        if counted {
                            app.mark_dirty();
                        }
                    }
                    // Anyone may submit a lobby song. Gate synchronously (cooldown /
                    // caps / dedup), then resolve oEmbed OFF the loop — never await
                    // the fetch here or it head-of-line-blocks vote ingestion.
                    ChatAction::Submit(raw_url) => {
                        if let Some((source, url)) =
                            app.gate_submission(&m.user.user_id, &raw_url, now_ms())
                        {
                            let app = app.clone();
                            let sem = fetch_sem.clone();
                            let submitter = m.user.username.clone();
                            tokio::spawn(async move {
                                let Ok(_permit) = sem.acquire().await else { return };
                                match crate::media::fetch(source, &url).await {
                                    // Drop on failure — a raw URL as a title would
                                    // render on the overlay (no placeholder).
                                    Ok(meta) => app.add_submitted_song(meta, submitter).await,
                                    Err(e) => tracing::warn!("submit fetch failed ({url}): {e}"),
                                }
                            });
                        }
                    }
                    // Mod-only: reset the current match's votes (not persisted state).
                    ChatAction::ResetVotes => {
                        if app.with_battle(Battle::reset_votes).is_ok() {
                            app.mark_dirty();
                        }
                    }
                    // Mod-only: skip resolves the match → persist the bracket change.
                    ChatAction::Skip => {
                        if app.with_battle(Battle::skip_match).is_ok() {
                            app.persist().await;
                        }
                    }
                    ChatAction::Ignore => {}
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
    state.clear_submit_ledger();
    state.set_kick(ConnectionState::Disconnected, None);
    state.mark_dirty();
    Ok(())
}
