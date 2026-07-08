use tauri::State;
use crate::services::finbox_sso;

#[tauri::command]
pub fn open_finbox_sso_window(app: tauri::AppHandle) -> Result<bool, String> {
    finbox_sso::open_finbox_sso_window(&app)?;
    Ok(true)
}

#[tauri::command]
pub fn close_finbox_sso_window(app: tauri::AppHandle) -> Result<bool, String> {
    finbox_sso::close_finbox_sso_window(&app)?;
    Ok(true)
}