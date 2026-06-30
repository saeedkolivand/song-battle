use crate::domain::snapshot::Settings;
use crate::error::AppResult;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> AppResult<Settings> {
    Ok(state.settings())
}

#[tauri::command]
pub async fn set_anonymous(anonymous: bool, state: State<'_, AppState>) -> AppResult<()> {
    state.set_anonymous(anonymous).await
}

#[tauri::command]
pub async fn set_default_timer(sec: u32, state: State<'_, AppState>) -> AppResult<()> {
    // Clamp to a sane match length so the tick logic can't be fed 0 / absurd values.
    state.set_default_timer(sec.clamp(5, 600)).await
}
