use crate::domain::song::{detect_source, Song};
use crate::error::{AppError, AppResult};
use crate::media;
use crate::state::AppState;
use tauri::State;
use uuid::Uuid;

#[tauri::command]
pub async fn import_song(
    url: String,
    submitter: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let source = detect_source(&url).ok_or_else(|| AppError::UnsupportedUrl(url.clone()))?;
    // Enrich via oEmbed; on failure still add the song (placeholder title).
    let meta = match media::fetch(source, &url).await {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!("oembed failed for {url}: {e}");
            media::placeholder(source, &url)
        }
    };
    let song = Song {
        id: Uuid::new_v4().to_string(),
        title: meta.title,
        artist: meta.artist,
        thumbnail: meta.thumbnail,
        duration_sec: meta.duration_sec,
        source,
        source_url: url,
        submitter,
        metadata: None,
    };
    state.with_battle(|b| b.add_song(song))?;
    state.persist().await;
    Ok(())
}

#[tauri::command]
pub async fn remove_song(id: String, state: State<'_, AppState>) -> AppResult<()> {
    state.with_battle(|b| b.remove_song(&id))?;
    state.persist().await;
    Ok(())
}

#[tauri::command]
pub async fn shuffle_songs(state: State<'_, AppState>) -> AppResult<()> {
    // Scope the non-Send rng so it drops before the await.
    state.with_battle(|b| b.shuffle(&mut rand::thread_rng()))?;
    state.persist().await;
    Ok(())
}

/// Drag-to-seed: set the song order. Only allowed in the lobby (no bracket yet),
/// since seeding is read from `songs` at generation; once a bracket exists it's a no-op.
#[tauri::command]
pub async fn reorder_songs(
    ordered_ids: Vec<String>,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let reordered = state.with_battle(|b| {
        if b.matches.is_empty() {
            b.reorder_songs(&ordered_ids);
            true
        } else {
            false // bracket already generated → seeding locked
        }
    })?;
    if reordered {
        state.persist().await;
    }
    Ok(())
}
