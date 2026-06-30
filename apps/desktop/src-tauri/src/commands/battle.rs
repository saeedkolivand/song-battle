use crate::domain::battle::Battle;
use crate::error::AppResult;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub async fn create_battle(
    title: String,
    description: String,
    theme: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    // No explicit per-battle timer yet → seed from the persisted default. A later
    // `set_timer` overrides it; `start_match` then uses the battle's duration.
    let mut battle = Battle::new(title, description, theme);
    battle.set_timer(state.default_timer_sec());
    state.set_battle(battle);
    state.persist().await;
    Ok(())
}

#[tauri::command]
pub async fn generate_bracket(state: State<'_, AppState>) -> AppResult<()> {
    state.with_battle(Battle::generate_bracket)??;
    state.persist().await;
    Ok(())
}

#[tauri::command]
pub async fn start_match(state: State<'_, AppState>) -> AppResult<()> {
    state.with_battle(Battle::start_match)??;
    state.persist().await;
    Ok(())
}

#[tauri::command]
pub async fn reset_votes(state: State<'_, AppState>) -> AppResult<()> {
    state.with_battle(Battle::reset_votes)?;
    state.mark_dirty();
    Ok(())
}

#[tauri::command]
pub async fn skip_match(state: State<'_, AppState>) -> AppResult<()> {
    state.with_battle(Battle::skip_match)??;
    state.persist().await;
    Ok(())
}

#[tauri::command]
pub async fn set_timer(duration_sec: u32, state: State<'_, AppState>) -> AppResult<()> {
    state.with_battle(|b| b.set_timer(duration_sec))?;
    state.persist().await;
    Ok(())
}
