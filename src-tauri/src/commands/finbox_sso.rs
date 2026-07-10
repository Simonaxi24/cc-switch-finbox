use crate::commands::finbox_marketplace::FinboxServiceState;
use crate::services::finbox_sso;
use crate::store::AppState;
use tauri::State;

#[tauri::command]
pub fn open_finbox_sso_window(
    app: tauri::AppHandle,
    finbox_state: State<'_, FinboxServiceState>,
    app_state: State<'_, AppState>,
) -> Result<bool, String> {
    finbox_sso::open_finbox_sso_window(&app, finbox_state.0.clone(), app_state.db.clone())?;
    Ok(true)
}

#[tauri::command]
pub fn close_finbox_sso_window(app: tauri::AppHandle) -> Result<bool, String> {
    finbox_sso::close_finbox_sso_window(&app)?;
    Ok(true)
}