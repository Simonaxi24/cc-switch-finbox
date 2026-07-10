//! Finbox SSO login via embedded WebView with automatic cookie extraction.

use crate::database::Database;
use crate::services::finbox_marketplace::FinboxMarketplaceService;
use log;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

/// Open a Tauri WebView window to finbox.jd.com/coverage for SSO login.
/// After the user logs in, the frontend monitors the window and calls
/// `extract_finbox_cookies` to extract cookies via JS evaluation.
pub fn open_finbox_sso_window(
    app: &AppHandle,
) -> Result<(), String> {
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

/// Extract cookies from the SSO window and save them.
/// Called from the frontend after user logs in.
pub fn extract_and_save_cookies(
    app: &AppHandle,
    finbox_service: Arc<FinboxMarketplaceService>,
    db: Arc<Database>,
) -> Result<String, String> {
    if let Some(sso_window) = app.get_webview_window("finbox-sso") {
        // Use JS eval to get document.cookie
        let js_result = sso_window
            .eval("document.cookie")
            .map_err(|e| format!("eval 失败: {e}"))?;

        // In Tauri v2, eval() is fire-and-forget, we can't get the result.
        // Instead, we inject a script that calls back via invoke
        let inject = r#"
            (function() {
                var cookie = document.cookie;
                if (cookie && cookie.length > 0) {
                    window.__TAURI__?.core?.invoke?.('set_finbox_sso_cookie', { cookie: cookie })
                        .then(function() {
                            console.log('Finbox: cookie saved via invoke');
                        })
                        .catch(function(e) {
                            console.error('Finbox: invoke failed:', e);
                        });
                } else {
                    console.log('Finbox: no cookies found');
                }
            })();
        "#;
        sso_window.eval(inject).map_err(|e| format!("JS 注入失败: {e}"))?;

        // Wait briefly for the invoke to complete
        std::thread::sleep(std::time::Duration::from_millis(500));

        // Emit success event for the frontend to pick up
        let _ = app.emit("finbox-sso-success", true);

        // Close the SSO window
        let _ = sso_window.close();
    }

    Ok(())
}

/// Close the Finbox SSO window.
pub fn close_finbox_sso_window(app: &AppHandle) -> Result<(), String> {
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