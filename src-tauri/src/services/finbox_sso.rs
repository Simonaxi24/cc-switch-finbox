//! Finbox SSO login via embedded WebView with automatic cookie extraction.

use crate::database::Database;
use crate::services::finbox_marketplace::FinboxMarketplaceService;
use log;
use std::sync::Arc;
use tauri::Manager;

/// Open a Tauri WebView window to finbox.jd.com/coverage for SSO login.
/// After the user logs in, the frontend monitors the window and calls
/// `extract_finbox_cookies` to extract cookies via JS evaluation.
pub fn open_finbox_sso_window(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(existing) = app.get_webview_window("finbox-sso") {
        existing.show().map_err(|e| format!("{e}"))?;
        existing.set_focus().map_err(|e| format!("{e}"))?;
        return Ok(());
    }

    tauri::WebviewWindowBuilder::new(
        app,
        "finbox-sso",
        tauri::WebviewUrl::External(
            "https://finbox.jd.com/coverage"
                .parse()
                .map_err(|e| format!("URL parse failed: {e}"))?,
        ),
    )
    .title("Finbox 小财神 登录")
    .inner_size(800.0, 600.0)
    .resizable(true)
    .center()
    .visible(true)
    .build()
    .map_err(|e| format!("创建 Finbox SSO 窗口失败: {e}"))?;

    Ok(())
}

/// Close the Finbox SSO window.
pub fn close_finbox_sso_window(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("finbox-sso") {
        window.close().map_err(|e| format!("{e}"))?;
    }
    Ok(())
}

/// Load persisted SSO cookie from DB and inject into FinboxMarketplaceService.
pub fn load_persisted_sso_cookie(
    db: &Arc<Database>,
    finbox_service: &FinboxMarketplaceService,
) {
    let cookie = match db.conn.lock() {
        Ok(conn) => {
            let mut stmt = match conn.prepare("SELECT value FROM settings WHERE key = ?1") {
                Ok(s) => s,
                Err(_) => return,
            };
            stmt.query_row(rusqlite::params!["finbox_sso_cookie"], |row| {
                row.get::<_, String>(0)
            })
            .ok()
        }
        Err(_) => None,
    };

    if let Some(cookie) = cookie {
        finbox_service.set_sso_cookie(cookie);
        log::info!("Finbox SSO cookie 已从 DB 加载");
    }
}