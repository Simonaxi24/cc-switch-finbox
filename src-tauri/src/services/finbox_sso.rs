//! Finbox SSO login via embedded WebView with automatic cookie extraction.

use crate::database::Database;
use crate::services::finbox_marketplace::FinboxMarketplaceService;
use log;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

const FINBOX_SSO_COOKIE_KEY: &str = "finbox_sso_cookie";

/// Open a Tauri WebView window to finbox.jd.com/coverage for SSO login.
/// After the user logs in, automatically extract cookies and save them.
pub fn open_finbox_sso_window(
    app: &AppHandle,
    finbox_service: Arc<FinboxMarketplaceService>,
    db: Arc<Database>,
) -> Result<(), String> {
    if let Some(existing) = app.get_webview_window("finbox-sso") {
        existing.show().map_err(|e| format!("{e}"))?;
        existing.set_focus().map_err(|e| format!("{e}"))?;
        return Ok(());
    }

    let app_handle = app.clone();
    let svc = finbox_service.clone();
    let db_clone = db.clone();

    let builder = tauri::WebviewWindowBuilder::new(
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
    .on_navigation(move |url| {
        let url_str = url.to_string();

        if url_str.starts_with("https://finbox.jd.com/")
            && !url_str.contains("/sso/")
            && !url_str.contains("ssa.jd.com")
        {
            log::info!("Finbox SSO: 登录成功，提取 cookie");

            let ah = app_handle.clone();
            let svc2 = svc.clone();
            let db2 = db_clone.clone();

            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(1500));

                if let Some(sso_window) = ah.get_webview_window("finbox-sso") {
                    let extract_js = r#"
                        (function() {
                            var cookies = document.cookie;
                            if (cookies && cookies.length > 0) {
                                window.__TAURI__.core.invoke('set_finbox_sso_cookie', { cookie: cookies });
                            }
                        })();
                    "#;
                    if let Err(e) = sso_window.eval(extract_js) {
                        log::error!("Finbox SSO: JS 注入失败: {}", e);
                        return;
                    }

                    std::thread::sleep(std::time::Duration::from_millis(500));

                    if let Some(cookie) = svc2.get_sso_cookie() {
                        persist_sso_cookie(&db2, &cookie);
                        let _ = ah.emit("finbox-sso-success", true);
                    }

                    let _ = sso_window.close();
                }
            });
        }
        true
    });

    builder
        .build()
        .map_err(|e| format!("创建 Finbox SSO 窗口失败: {e}"))?;

    Ok(())
}

/// Close the Finbox SSO window.
pub fn close_finbox_sso_window(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("finbox-sso") {
        window.close().map_err(|e| format!("{e}"))?;
    }
    Ok(())
}

/// Persist SSO cookie to the settings table.
pub fn persist_sso_cookie(db: &Arc<Database>, cookie: &str) {
    if let Ok(conn) = db.conn.lock() {
        let _ = conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            rusqlite::params![FINBOX_SSO_COOKIE_KEY, cookie],
        );
    }
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
            stmt.query_row(rusqlite::params![FINBOX_SSO_COOKIE_KEY], |row| {
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
