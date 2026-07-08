use crate::app_config::{AppType, InstalledSkill};
use crate::database::Database;
use crate::error::AppError;
use crate::services::skill::{DiscoverableSkill, SkillService};
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::str::FromStr;
use std::sync::Arc;

const FINBOX_BASE_URL: &str = "https://finbox.jd.com";
const CACHE_TTL_SECONDS: i64 = 3600;

// ── API response types ──

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinboxSkill {
    pub key: String,
    pub name: String,
    pub description: Option<String>,
    pub download_url: Option<String>,
    pub category: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinboxSkillDetail {
    pub key: String,
    pub name: String,
    pub description: Option<String>,
    pub download_url: Option<String>,
    pub category: Option<String>,
    pub readme: Option<String>,
}

/// Finbox `/finbox/tools` API response item
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FinboxTool {
    id: i64,
    name: String,
    description: Option<String>,
    owner: Option<String>,
    // attachments are in versions, not directly on the tool
}

/// Finbox `/finbox/tools/{id}/versions` API response item
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FinboxToolVersion {
    id: i64,
    title: Option<String>,
    #[serde(default)]
    attachments: Vec<FinboxAttachment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FinboxAttachment {
    id: i64,
    file_name: Option<String>,
    object_key: Option<String>,
    object_url: Option<String>,
}

/// Finbox `/finbox/coverage/assets` API response item
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FinboxCoverageAsset {
    id: i64,
    asset_type: Option<String>,
    asset_ref: Option<String>,
    name: Option<String>,
    description: Option<String>,
    status: Option<String>,
}

/// Generic Finbox API wrapper: `{"success": true, "data": ...}`
#[derive(Debug, Deserialize)]
struct FinboxApiResponse<T> {
    success: bool,
    #[serde(default)]
    data: Option<T>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    detail: Option<String>,
}

// ── Service ──

pub struct FinboxMarketplaceService {
    client: reqwest::Client,
    /// JD SSO cookie（用户配置后使用）
    sso_cookie: std::sync::Mutex<Option<String>>,
}

impl Default for FinboxMarketplaceService {
    fn default() -> Self {
        Self::new()
    }
}

impl FinboxMarketplaceService {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("CC-Switch/3.17.0")
            .timeout(std::time::Duration::from_secs(30))
            .cookie_store(true)
            .build()
            .unwrap_or_default();
        Self {
            client,
            sso_cookie: std::sync::Mutex::new(None),
        }
    }

    /// Set the JD SSO cookie for authenticated API requests.
    pub fn set_sso_cookie(&self, cookie: String) {
        if let Ok(mut guard) = self.sso_cookie.lock() {
            *guard = Some(cookie);
        }
    }

    /// Get the current SSO cookie (if configured).
    pub fn get_sso_cookie(&self) -> Option<String> {
        self.sso_cookie.lock().ok().and_then(|g| g.clone())
    }

    /// Check if SSO cookie is configured.
    pub fn has_sso_cookie(&self) -> bool {
        self.sso_cookie.lock().map(|g| g.is_some()).unwrap_or(false)
    }

    // ── Public API ──

    /// Search Finbox marketplace skills, using the local SQLite cache first.
    pub async fn search_skills(
        &self,
        db: &Arc<Database>,
        query: Option<&str>,
    ) -> Result<Vec<FinboxSkill>> {
        let mut cached = self.get_cached_skills(db)?;

        if cached.is_empty() {
            self.refresh_cache(db).await?;
            cached = self.get_cached_skills(db)?;
        }

        let Some(query) = query.map(str::trim).filter(|q| !q.is_empty()) else {
            return Ok(cached);
        };

        let query = query.to_lowercase();
        Ok(cached
            .into_iter()
            .filter(|skill| {
                skill.key.to_lowercase().contains(&query)
                    || skill.name.to_lowercase().contains(&query)
                    || skill
                        .description
                        .as_deref()
                        .map(|d| d.to_lowercase().contains(&query))
                        .unwrap_or(false)
                    || skill
                        .category
                        .as_deref()
                        .map(|c| c.to_lowercase().contains(&query))
                        .unwrap_or(false)
            })
            .collect())
    }

    /// Get a single Finbox skill detail, refreshing the cache on miss.
    pub async fn get_skill_detail(
        &self,
        db: &Arc<Database>,
        key: &str,
    ) -> Result<FinboxSkillDetail> {
        if let Some(skill) = self.get_cached_skill(db, key)? {
            return Ok(Self::to_detail(skill));
        }

        self.refresh_cache(db).await?;
        let skill = self
            .get_cached_skill(db, key)?
            .ok_or_else(|| anyhow!("Skill '{}' not found on Finbox", key))?;
        Ok(Self::to_detail(skill))
    }

    /// Install a Finbox skill into CC Switch.
    pub async fn install_skill(
        &self,
        db: &Arc<Database>,
        key: &str,
        skill_service: &SkillService,
        current_app: &str,
        scope: Option<&str>,
        project_path: Option<&str>,
    ) -> Result<InstalledSkill> {
        let detail = self.get_skill_detail(db, key).await?;
        let download_url = detail
            .download_url
            .as_deref()
            .ok_or_else(|| anyhow!("Skill '{}' has no download URL", key))?;
        let app_type = AppType::from_str(current_app)
            .map_err(|e| anyhow!("Invalid app type '{}': {}", current_app, e))?;
        let scope = scope.unwrap_or("global");

        if Self::is_archive_url(download_url) {
            return self
                .install_archive(db, key, download_url, &app_type)
                .await;
        }

        let skill = Self::discoverable_from_detail(&detail, download_url)?;
        skill_service
            .install(db, &skill, &app_type, scope, project_path)
            .await
    }

    /// Force-refresh the Finbox marketplace cache via API.
    pub async fn refresh_cache(&self, db: &Arc<Database>) -> Result<()> {
        let skills = self.fetch_skills_from_api().await?;
        self.save_to_cache(db, &skills)?;
        Ok(())
    }

    // ── API calls ──

    /// Build an authenticated request builder with SSO cookie.
    fn authed_request(&self, method: reqwest::Method, url: &str) -> Result<reqwest::RequestBuilder> {
        let cookie = self.sso_cookie.lock()
            .map_err(|e| anyhow!("Mutex lock failed: {}", e))?
            .clone()
            .ok_or_else(|| anyhow!("未配置京东 SSO Cookie，请在设置中配置后再使用 Finbox 商场"))?;

        Ok(self.client
            .request(method, url)
            .header("Cookie", &cookie)
            .header("Cache-Control", "no-store, no-cache, must-revalidate")
            .header("Pragma", "no-cache"))
    }

    /// Fetch skills from the Finbox API.
    ///
    /// Tries two endpoints in order:
    /// 1. `/finbox/tools` — the tool/skill registry
    /// 2. `/finbox/coverage/assets` — coverage assets (fallback)
    async fn fetch_skills_from_api(&self) -> Result<Vec<FinboxSkill>> {
        // Try /finbox/tools first
        match self.fetch_tools().await {
            Ok(skills) if !skills.is_empty() => return Ok(skills),
            Ok(_) => { /* empty result, try fallback */ }
            Err(e) => {
                log::warn!("Finbox /finbox/tools failed: {}, trying fallback", e);
            }
        }

        // Fallback: /finbox/coverage/assets
        match self.fetch_coverage_assets().await {
            Ok(skills) if !skills.is_empty() => return Ok(skills),
            Ok(_) => Err(anyhow!("Finbox 返回了空的 skill 列表")),
            Err(e) => Err(anyhow!("无法从 Finbox 获取 skill 数据: {}", e)),
        }
    }

    /// Fetch from /finbox/tools endpoint.
    async fn fetch_tools(&self) -> Result<Vec<FinboxSkill>> {
        let url = format!("{}/finbox/tools", FINBOX_BASE_URL);
        let resp = self.authed_request(reqwest::Method::GET, &url)?
            .send()
            .await?;

        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(anyhow!("SSO 登录已过期，请重新配置 Cookie"));
        }

        let api_resp: FinboxApiResponse<Vec<FinboxTool>> = resp.json().await
            .map_err(|e| anyhow!("解析 Finbox API 响应失败: {}", e))?;

        if !api_resp.success {
            let msg = api_resp.error.as_deref().or(api_resp.detail.as_deref()).unwrap_or("未知错误");
            return Err(anyhow!("Finbox API 错误: {}", msg));
        }

        let tools = api_resp.data.unwrap_or_default();
        let mut skills = Vec::with_capacity(tools.len());

        for tool in tools {
            // Try to get download URL from the latest version
            let download_url = self.fetch_tool_download_url(tool.id).await.ok();

            skills.push(FinboxSkill {
                key: format!("finbox:tool:{}", tool.id),
                name: tool.name,
                description: tool.description,
                download_url,
                category: tool.owner,
            });
        }

        Ok(skills)
    }

    /// Get the download URL for a tool's latest version.
    async fn fetch_tool_download_url(&self, tool_id: i64) -> Result<String> {
        let url = format!("{}/finbox/tools/{}/versions", FINBOX_BASE_URL, tool_id);
        let resp = self.authed_request(reqwest::Method::GET, &url)?
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("获取工具版本失败: HTTP {}", resp.status()));
        }

        let api_resp: FinboxApiResponse<Vec<FinboxToolVersion>> = resp.json().await
            .map_err(|e| anyhow!("解析版本响应失败: {}", e))?;

        let versions = api_resp.data.unwrap_or_default();
        let latest = versions.into_iter().next();

        if let Some(version) = latest {
            // Look for an attachment with a downloadable URL
            if let Some(attachment) = version.attachments.into_iter().next() {
                if let Some(obj_url) = attachment.object_url {
                    return Ok(obj_url);
                }
                if let Some(obj_key) = attachment.object_key {
                    return Ok(format!("{}/finbox/uploads/tool-attachments/{}/download", FINBOX_BASE_URL, obj_key));
                }
            }
        }

        Err(anyhow!("未找到可下载的版本"))
    }

    /// Fetch from /finbox/coverage/assets endpoint (fallback).
    async fn fetch_coverage_assets(&self) -> Result<Vec<FinboxSkill>> {
        let url = format!("{}/finbox/coverage/assets", FINBOX_BASE_URL);
        let resp = self.authed_request(reqwest::Method::GET, &url)?
            .send()
            .await?;

        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(anyhow!("SSO 登录已过期，请重新配置 Cookie"));
        }

        let api_resp: FinboxApiResponse<Vec<FinboxCoverageAsset>> = resp.json().await
            .map_err(|e| anyhow!("解析 Finbox Coverage Assets 响应失败: {}", e))?;

        if !api_resp.success {
            let msg = api_resp.error.as_deref().or(api_resp.detail.as_deref()).unwrap_or("未知错误");
            return Err(anyhow!("Finbox Coverage API 错误: {}", msg));
        }

        let assets = api_resp.data.unwrap_or_default();

        Ok(assets
            .into_iter()
            .filter(|a| a.status.as_deref() == Some("active"))
            .filter(|a| a.asset_type.as_deref() == Some("tool"))
            .map(|asset| FinboxSkill {
                key: format!("finbox:asset:{}", asset.id),
                name: asset.name.unwrap_or_default(),
                description: asset.description,
                download_url: None,
                category: asset.asset_ref,
            })
            .collect())
    }

    // ── Cache ──

    fn save_to_cache(&self, db: &Arc<Database>, skills: &[FinboxSkill]) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        let expires_at = now + CACHE_TTL_SECONDS;

        let mut conn = db
            .conn
            .lock()
            .map_err(|e| AppError::Database(format!("Mutex lock failed: {}", e)))?;
        let tx = conn
            .transaction()
            .map_err(|e| AppError::Database(e.to_string()))?;
        tx.execute("DELETE FROM finbox_skill_cache", [])
            .map_err(|e| AppError::Database(e.to_string()))?;

        for skill in skills {
            let html_hash = format!("api-{}", now);
            tx.execute(
                "INSERT OR REPLACE INTO finbox_skill_cache \
                 (key, name, description, download_url, category, raw_html_hash, cached_at, expires_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![
                    skill.key,
                    skill.name,
                    skill.description,
                    skill.download_url,
                    skill.category,
                    html_hash,
                    now,
                    expires_at,
                ],
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        }

        tx.commit()
            .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    fn get_cached_skills(&self, db: &Arc<Database>) -> Result<Vec<FinboxSkill>> {
        let conn = db
            .conn
            .lock()
            .map_err(|e| AppError::Database(format!("Mutex lock failed: {}", e)))?;
        let now = chrono::Utc::now().timestamp();

        let mut stmt = conn
            .prepare(
                "SELECT key, name, description, download_url, category \
                 FROM finbox_skill_cache WHERE expires_at > ?1 ORDER BY name COLLATE NOCASE",
            )
            .map_err(|e| AppError::Database(e.to_string()))?;

        let skills = stmt
            .query_map(rusqlite::params![now], |row| {
                Ok(FinboxSkill {
                    key: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    download_url: row.get(3)?,
                    category: row.get(4)?,
                })
            })
            .map_err(|e| AppError::Database(e.to_string()))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(skills)
    }

    fn get_cached_skill(&self, db: &Arc<Database>, key: &str) -> Result<Option<FinboxSkill>> {
        let conn = db
            .conn
            .lock()
            .map_err(|e| AppError::Database(format!("Mutex lock failed: {}", e)))?;
        let now = chrono::Utc::now().timestamp();

        let mut stmt = conn
            .prepare(
                "SELECT key, name, description, download_url, category \
                 FROM finbox_skill_cache WHERE key = ?1 AND expires_at > ?2",
            )
            .map_err(|e| AppError::Database(e.to_string()))?;

        let mut rows = stmt
            .query_map(rusqlite::params![key, now], |row| {
                Ok(FinboxSkill {
                    key: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    download_url: row.get(3)?,
                    category: row.get(4)?,
                })
            })
            .map_err(|e| AppError::Database(e.to_string()))?;

        let skill = rows
            .next()
            .transpose()
            .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(skill)
    }

    // ── Install helpers ──

    async fn install_archive(
        &self,
        db: &Arc<Database>,
        key: &str,
        download_url: &str,
        app_type: &AppType,
    ) -> Result<InstalledSkill> {
        let temp_dir = tempfile::tempdir()?;
        let zip_path = temp_dir.path().join(format!("{}.zip", Self::slugify(key)));
        let bytes = self
            .client
            .get(download_url)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;
        std::fs::write(&zip_path, &bytes)
            .with_context(|| format!("Failed to write Finbox skill archive: {}", zip_path.display()))?;

        SkillService::install_from_zip(db, &zip_path, app_type)?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("No skill installed from archive"))
    }

    fn discoverable_from_detail(detail: &FinboxSkillDetail, download_url: &str) -> Result<DiscoverableSkill> {
        let (repo_owner, repo_name, repo_branch, directory) = Self::parse_github_skill_url(download_url)
            .ok_or_else(|| anyhow!("Finbox skill '{}' download URL is not a supported archive or GitHub repository URL: {}", detail.key, download_url))?;
        let directory = if directory.is_empty() {
            Self::slugify(&detail.name)
        } else {
            directory
        };

        Ok(DiscoverableSkill {
            key: detail.key.clone(),
            name: detail.name.clone(),
            description: detail.description.clone().unwrap_or_default(),
            directory,
            readme_url: None,
            repo_owner,
            repo_name,
            repo_branch,
        })
    }

    // ── URL helpers ──

    fn parse_github_skill_url(url: &str) -> Option<(String, String, String, String)> {
        let parsed = url::Url::parse(url).ok()?;
        if parsed.host_str()? != "github.com" {
            return None;
        }

        let segments = parsed
            .path_segments()?
            .filter(|segment| !segment.is_empty())
            .collect::<Vec<_>>();
        if segments.len() < 2 {
            return None;
        }

        let owner = segments[0].to_string();
        let repo = segments[1].trim_end_matches(".git").to_string();
        if segments.len() >= 5 && matches!(segments[2], "tree" | "blob") {
            let branch = segments[3].to_string();
            let directory = if segments[2] == "blob" && segments.last() == Some(&"SKILL.md") {
                segments[4..segments.len() - 1].join("/")
            } else {
                segments[4..].join("/")
            };
            return Some((owner, repo, branch, directory));
        }

        Some((owner, repo, "main".to_string(), String::new()))
    }

    fn is_archive_url(url: &str) -> bool {
        let lower = url.to_lowercase();
        lower.ends_with(".zip") || lower.ends_with(".tar.gz") || lower.ends_with(".tgz")
    }

    fn to_detail(skill: FinboxSkill) -> FinboxSkillDetail {
        FinboxSkillDetail {
            key: skill.key,
            name: skill.name,
            description: skill.description,
            download_url: skill.download_url,
            category: skill.category,
            readme: None,
        }
    }

    fn slugify(value: &str) -> String {
        let slug = value
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() {
                    ch.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect::<String>()
            .split('-')
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("-");

        if slug.is_empty() {
            "finbox-skill".to_string()
        } else {
            slug
        }
    }
}
