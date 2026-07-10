use crate::app_config::{AppType, InstalledSkill};
use crate::database::Database;
use crate::error::AppError;
use crate::services::skill::{DiscoverableSkill, SkillService};
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::Arc;

const FINBOX_BASE_URL: &str = "https://finbox.jd.com";
const CACHE_TTL_SECONDS: i64 = 3600;

// ── Public types (returned to frontend) ──

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinboxSkill {
    pub key: String,
    pub name: String,
    pub description: Option<String>,
    pub download_url: Option<String>,
    pub category: Option<String>,
    pub owner: Option<String>,
    pub version: Option<String>,
    pub status: Option<String>,
    pub download_count: Option<i64>,
    pub monthly_usage: Option<i64>,
    pub star_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinboxSkillDetail {
    pub key: String,
    pub name: String,
    pub description: Option<String>,
    pub download_url: Option<String>,
    pub category: Option<String>,
    pub version: Option<String>,
    pub readme: Option<String>,
}

// ── API response types (match actual finbox.jd.com responses) ──

/// GET /api/finbox/tools returns {"tools": [...]}
#[derive(Debug, Deserialize)]
struct FinboxToolsResponse {
    #[serde(default)]
    tools: Vec<FinboxToolRaw>,
}

/// Each tool in /api/finbox/tools
#[derive(Debug, Deserialize)]
struct FinboxToolRaw {
    id: String,
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    owner: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    download_count: Option<i64>,
    #[serde(default)]
    monthly_usage: Option<i64>,
    #[serde(default)]
    star_count: Option<i64>,
}

// ── Service ──

pub struct FinboxMarketplaceService {
    client: reqwest::Client,
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
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap_or_default();
        Self {
            client,
            sso_cookie: std::sync::Mutex::new(None),
        }
    }

    pub fn set_sso_cookie(&self, cookie: String) {
        if let Ok(mut guard) = self.sso_cookie.lock() {
            *guard = Some(cookie);
        }
    }

    pub fn get_sso_cookie(&self) -> Option<String> {
        self.sso_cookie.lock().ok().and_then(|g| g.clone())
    }

    pub fn has_sso_cookie(&self) -> bool {
        self.sso_cookie.lock().map(|g| g.is_some()).unwrap_or(false)
    }

    // ── Public API ──

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
                skill.name.to_lowercase().contains(&query)
                    || skill.description.as_deref().map_or(false, |d| d.to_lowercase().contains(&query))
                    || skill.category.as_deref().map_or(false, |c| c.to_lowercase().contains(&query))
                    || skill.owner.as_deref().map_or(false, |o| o.to_lowercase().contains(&query))
            })
            .collect())
    }

    pub async fn get_skill_detail(
        &self,
        db: &Arc<Database>,
        key: &str,
    ) -> Result<FinboxSkillDetail> {
        if let Some(skill) = self.get_cached_skill(db, key)? {
            return Ok(FinboxSkillDetail {
                key: skill.key,
                name: skill.name,
                description: skill.description,
                download_url: skill.download_url,
                category: skill.category,
                version: skill.version,
                readme: None,
            });
        }
        Err(anyhow!("Skill '{}' not found", key))
    }

    pub async fn install_skill(
        &self,
        db: &Arc<Database>,
        key: &str,
        skill_service: &SkillService,
        current_app: &str,
    ) -> Result<InstalledSkill> {
        let detail = self.get_skill_detail(db, key).await?;
        let download_url = detail
            .download_url
            .as_deref()
            .ok_or_else(|| anyhow!("Skill '{}' has no download URL", key))?;
        let app_type = AppType::from_str(current_app)
            .map_err(|e| anyhow!("Invalid app type '{}': {}", current_app, e))?;

        if Self::is_archive_url(download_url) {
            return self.install_archive(db, key, download_url, &app_type).await;
        }

        let skill = Self::discoverable_from_detail(&detail, download_url)?;
        skill_service.install(db, &skill, &app_type, "global", None).await
    }

    pub async fn refresh_cache(&self, db: &Arc<Database>) -> Result<()> {
        let skills = self.fetch_tools().await?;
        self.save_to_cache(db, &skills)?;
        Ok(())
    }

    // ── API call ──

    fn authed_request(&self, url: &str) -> Result<reqwest::RequestBuilder> {
        let cookie = self.sso_cookie.lock()
            .map_err(|e| anyhow!("Mutex lock failed: {}", e))?
            .clone()
            .ok_or_else(|| anyhow!("未配置京东 SSO Cookie，请先登录 Finbox"))?;

        Ok(self.client
            .get(url)
            .header("Cookie", &cookie)
            .header("Accept", "application/json, text/plain, */*")
            .header("X-Requested-With", "XMLHttpRequest")
            .header("Referer", "https://finbox.jd.com/coverage/departments/3"))
    }

    async fn fetch_tools(&self) -> Result<Vec<FinboxSkill>> {
        let url = format!("{}/api/finbox/tools", FINBOX_BASE_URL);
        let resp = self.authed_request(&url)?.send().await?;

        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(anyhow!("SSO 登录已过期，请重新登录"));
        }

        let body = resp.text().await?;
        let parsed: FinboxToolsResponse = serde_json::from_str(&body)
            .map_err(|e| anyhow!("解析 Finbox tools 响应失败: {} — body prefix: {}", e, &body[..body.len().min(200)]))?;

        Ok(parsed.tools.into_iter().map(|t| FinboxSkill {
            key: t.id.clone(),
            name: t.name,
            description: t.description,
            download_url: None,
            category: t.category,
            owner: t.owner,
            version: t.version,
            status: t.status,
            download_count: t.download_count,
            monthly_usage: t.monthly_usage,
            star_count: t.star_count,
        }).collect())
    }

    // ── Cache ──

    fn save_to_cache(&self, db: &Arc<Database>, skills: &[FinboxSkill]) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        let expires_at = now + CACHE_TTL_SECONDS;

        let mut conn = db.conn.lock()
            .map_err(|e| AppError::Database(format!("Mutex lock failed: {}", e)))?;
        let tx = conn.transaction()
            .map_err(|e| AppError::Database(e.to_string()))?;
        tx.execute("DELETE FROM finbox_skill_cache", [])
            .map_err(|e| AppError::Database(e.to_string()))?;

        for skill in skills {
            tx.execute(
                "INSERT OR REPLACE INTO finbox_skill_cache \
                 (key, name, description, download_url, category, raw_html_hash, cached_at, expires_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![
                    skill.key, skill.name, skill.description, skill.download_url,
                    skill.category, format!("api-{}", now), now, expires_at,
                ],
            ).map_err(|e| AppError::Database(e.to_string()))?;
        }

        tx.commit().map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    fn get_cached_skills(&self, db: &Arc<Database>) -> Result<Vec<FinboxSkill>> {
        let conn = db.conn.lock()
            .map_err(|e| AppError::Database(format!("Mutex lock failed: {}", e)))?;
        let now = chrono::Utc::now().timestamp();

        let mut stmt = conn.prepare(
            "SELECT key, name, description, download_url, category \
             FROM finbox_skill_cache WHERE expires_at > ?1 ORDER BY name COLLATE NOCASE",
        ).map_err(|e| AppError::Database(e.to_string()))?;

        let skills = stmt.query_map(rusqlite::params![now], |row| {
            Ok(FinboxSkill {
                key: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                download_url: row.get(3)?,
                category: row.get(4)?,
                owner: None, version: None, status: None,
                download_count: None, monthly_usage: None, star_count: None,
            })
        }).map_err(|e| AppError::Database(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(skills)
    }

    fn get_cached_skill(&self, db: &Arc<Database>, key: &str) -> Result<Option<FinboxSkill>> {
        let conn = db.conn.lock()
            .map_err(|e| AppError::Database(format!("Mutex lock failed: {}", e)))?;
        let now = chrono::Utc::now().timestamp();

        let mut stmt = conn.prepare(
            "SELECT key, name, description, download_url, category \
             FROM finbox_skill_cache WHERE key = ?1 AND expires_at > ?2",
        ).map_err(|e| AppError::Database(e.to_string()))?;

        let skill = stmt.query_map(rusqlite::params![key, now], |row| {
            Ok(FinboxSkill {
                key: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                download_url: row.get(3)?,
                category: row.get(4)?,
                owner: None, version: None, status: None,
                download_count: None, monthly_usage: None, star_count: None,
            })
        }).map_err(|e| AppError::Database(e.to_string()))?
        .next()
        .transpose()
        .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(skill)
    }

    // ── Install helpers ──

    async fn install_archive(&self, db: &Arc<Database>, key: &str, download_url: &str, app_type: &AppType) -> Result<InstalledSkill> {
        let temp_dir = tempfile::tempdir()?;
        let zip_path = temp_dir.path().join(format!("{}.zip", Self::slugify(key)));
        let bytes = self.client.get(download_url).send().await?.error_for_status()?.bytes().await?;
        std::fs::write(&zip_path, &bytes)
            .with_context(|| format!("Failed to write archive: {}", zip_path.display()))?;
        SkillService::install_from_zip(db, &zip_path, app_type)?
            .into_iter().next()
            .ok_or_else(|| anyhow!("No skill installed from archive"))
    }

    fn discoverable_from_detail(detail: &FinboxSkillDetail, download_url: &str) -> Result<DiscoverableSkill> {
        let (owner, repo, branch, dir) = Self::parse_github_url(download_url)
            .ok_or_else(|| anyhow!("Not a supported URL: {}", download_url))?;
        let dir = if dir.is_empty() { Self::slugify(&detail.name) } else { dir };
        Ok(DiscoverableSkill {
            key: detail.key.clone(), name: detail.name.clone(),
            description: detail.description.clone().unwrap_or_default(),
            directory: dir, readme_url: None,
            repo_owner: owner, repo_name: repo, repo_branch: branch,
        })
    }

    fn parse_github_url(url: &str) -> Option<(String, String, String, String)> {
        let p = url::Url::parse(url).ok()?;
        if p.host_str()? != "github.com" { return None; }
        let s: Vec<_> = p.path_segments()?.filter(|s| !s.is_empty()).collect();
        if s.len() < 2 { return None; }
        let owner = s[0].to_string();
        let repo = s[1].trim_end_matches(".git").to_string();
        if s.len() >= 5 && matches!(s[2], "tree" | "blob") {
            let branch = s[3].to_string();
            let dir = s[4..].join("/");
            return Some((owner, repo, branch, dir));
        }
        Some((owner, repo, "main".to_string(), String::new()))
    }

    fn is_archive_url(url: &str) -> bool {
        let l = url.to_lowercase();
        l.ends_with(".zip") || l.ends_with(".tar.gz") || l.ends_with(".tgz")
    }

    fn slugify(value: &str) -> String {
        let s: String = value.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
        let s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
        if s.is_empty() { "finbox-skill".to_string() } else { s }
    }
}
