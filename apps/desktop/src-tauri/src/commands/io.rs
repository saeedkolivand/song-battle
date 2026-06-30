use crate::domain::snapshot::Snapshot;
use crate::error::AppResult;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub async fn get_snapshot(state: State<'_, AppState>) -> AppResult<Snapshot> {
    Ok(state.current_snapshot())
}

#[tauri::command]
pub async fn export_json(state: State<'_, AppState>) -> AppResult<String> {
    state.export_json()
}

#[tauri::command]
pub async fn import_json(json: String, state: State<'_, AppState>) -> AppResult<()> {
    state.import_json(&json)?;
    state.persist().await;
    Ok(())
}
