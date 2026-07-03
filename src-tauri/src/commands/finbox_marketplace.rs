use crate::app_config::InstalledSkill;
use crate::services::finbox_marketplace::{
    FinboxMarketplaceService, FinboxSkill, FinboxSkillDetail,
};
use std::sync::Arc;
use tauri::State;

pub struct FinboxServiceState(pub Arc<FinboxMarketplaceService>);

#[tauri::command]
pub async fn search_finbox_skills(
    query: Option<String>,
    service: State<'_, FinboxServiceState>,
    app_state: State<'_, crate::commands::AppState>,
) -> Result<Vec<FinboxSkill>, String> {
    service
        .0
        .search_skills(&app_state.db, query.as_deref())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_finbox_skill_detail(
    key: String,
    service: State<'_, FinboxServiceState>,
    app_state: State<'_, crate::commands::AppState>,
) -> Result<FinboxSkillDetail, String> {
    service
        .0
        .get_skill_detail(&app_state.db, &key)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn install_from_finbox(
    key: String,
    current_app: String,
    scope: Option<String>,
    project_path: Option<String>,
    finbox_service: State<'_, FinboxServiceState>,
    skill_service: State<'_, crate::commands::skill::SkillServiceState>,
    app_state: State<'_, crate::commands::AppState>,
) -> Result<InstalledSkill, String> {
    finbox_service
        .0
        .install_skill(
            &app_state.db,
            &key,
            &skill_service.0,
            &current_app,
            scope.as_deref(),
            project_path.as_deref(),
        )
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn refresh_finbox_cache(
    service: State<'_, FinboxServiceState>,
    app_state: State<'_, crate::commands::AppState>,
) -> Result<bool, String> {
    service
        .0
        .refresh_cache(&app_state.db)
        .await
        .map_err(|e| e.to_string())?;
    Ok(true)
}
