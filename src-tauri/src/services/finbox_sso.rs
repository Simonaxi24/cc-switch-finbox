//! Finbox SSO login via embedded WebView.
//!
//! Opens a Tauri WebView window to finbox.jd.com/coverage for SSO authentication.
//! The user logs in via JD SSO.  Cookie extraction is handled by the frontend:
//! after the user finishes logging in, the frontend calls `extract_finbox_cookies`
//! which evaluates `document.cookie` in the SSO window context, saves the cookies
//! to the FinboxMarketplaceService, and closes the window.

use crate::services::finbox_marketplace::FinboxMarketplaceService;
use log;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

/// Open a Tauri WebView window to finbox.jd.com/coverage for SSO login.
///
/// Returns nothing — the window stays open until the user closes it or the
/// frontend calls `close_finbox_sso_window`.
pub fn open_finbox_sso_window(app: &AppHandle) -> Result<(), String> {
    if let Some(existing) = app.get_webview_window("finbox-sso") {
        existing.show().map_err(|e| format!("{e}"))?;
        existing.set_focus().map_err(|e| format!("{e}"))?;
        log::info!("Finbox SSO: reused existing window");
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

    log::info!("Finbox SSO 窗口已打开");
    Ok(())
}

/// Close the Finbox SSO window.
pub fn close_finbox_sso_window(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("finbox-sso") {
        window.close().map_err(|e| format!("{e}"))?;
        log::info!("Finbox SSO window closed");
    }
    Ok(())
}