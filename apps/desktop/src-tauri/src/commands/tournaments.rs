use crate::domain::snapshot::SavedBattle;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub async fn list_battles(state: State<'_, AppState>) -> AppResult<Vec<SavedBattle>> {
    state.list_battles().await
}

/// Load a saved tournament and make it the active battle.
#[tauri::command]
pub async fn load_battle(id: String, state: State<'_, AppState>) -> AppResult<()> {
    if state.load_saved_battle(id.clone()).await? {
        Ok(())
    } else {
        Err(AppError::NotFound(format!("battle {id}")))
    }
}

#[tauri::command]
pub async fn delete_battle(id: String, state: State<'_, AppState>) -> AppResult<()> {
    state.delete_battle(id).await
}
