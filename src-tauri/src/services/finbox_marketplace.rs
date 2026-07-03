use crate::app_config::{AppType, InstalledSkill};
use crate::database::Database;
use crate::error::AppError;
use crate::services::skill::{DiscoverableSkill, SkillService};
use anyhow::{anyhow, Context, Result};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::str::FromStr;
use std::sync::Arc;

const FINBOX_URL: &str = "https://finbox.jd.com/coverage";
const CACHE_TTL_SECONDS: i64 = 3600;

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

pub struct FinboxMarketplaceService {
    client: reqwest::Client,
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
            .build()
            .unwrap_or_default();
        Self { client }
    }

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
                        .map(|description| description.to_lowercase().contains(&query))
                        .unwrap_or(false)
                    || skill
                        .category
                        .as_deref()
                        .map(|category| category.to_lowercase().contains(&query))
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
    ///
    /// Archive URLs are downloaded and installed via `SkillService::install_from_zip`.
    /// Other URLs are treated as GitHub repository URLs and installed through the existing
    /// `SkillService::install` flow.
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

    /// Force-refresh the Finbox marketplace cache from finbox.jd.com/coverage.
    pub async fn refresh_cache(&self, db: &Arc<Database>) -> Result<()> {
        let html = self.fetch_page().await?;
        let skills = self.parse_skills(&html)?;
        self.save_to_cache(db, &skills, &html)?;
        Ok(())
    }

    async fn fetch_page(&self) -> Result<String> {
        let response = self
            .client
            .get(FINBOX_URL)
            .send()
            .await?
            .error_for_status()?;
        Ok(response.text().await?)
    }

    fn parse_skills(&self, html: &str) -> Result<Vec<FinboxSkill>> {
        let document = Html::parse_document(html);

        // PLACEHOLDER SELECTORS: finbox.jd.com/coverage's actual DOM structure is unknown.
        // These generic selectors are intentionally broad and MUST be updated once the real
        // page markup is inspected.
        let item_selector = Self::selector(
            "div.skill-item, tr.skill-row, li.skill-entry, div[class*='skill'], div[class*='coverage']",
        )?;
        let name_selector = Self::selector("h3, h2, .name, .title, a")?;
        let description_selector = Self::selector("p, .desc, .description, .summary")?;
        let category_selector = Self::selector(".category, .tag, [data-category]")?;
        let link_selector = Self::selector("a[href]")?;

        let mut skills = Vec::new();
        for element in document.select(&item_selector) {
            let name = element
                .select(&name_selector)
                .next()
                .map(Self::element_text)
                .unwrap_or_default();

            if name.is_empty() {
                continue;
            }

            let description = element
                .select(&description_selector)
                .next()
                .map(Self::element_text)
                .filter(|value| !value.is_empty());

            let category = element
                .select(&category_selector)
                .next()
                .map(|category| {
                    category
                        .value()
                        .attr("data-category")
                        .map(str::to_string)
                        .unwrap_or_else(|| Self::element_text(category))
                })
                .filter(|value| !value.is_empty());

            let download_url = element
                .select(&link_selector)
                .find_map(|link| link.value().attr("href"))
                .map(Self::normalize_finbox_url);

            let key = element
                .value()
                .attr("data-key")
                .or_else(|| element.value().attr("data-id"))
                .map(str::to_string)
                .or_else(|| download_url.as_deref().map(Self::key_from_url))
                .unwrap_or_else(|| Self::slugify(&name));

            skills.push(FinboxSkill {
                key,
                name,
                description,
                download_url,
                category,
            });
        }

        Ok(skills)
    }

    fn save_to_cache(&self, db: &Arc<Database>, skills: &[FinboxSkill], html: &str) -> Result<()> {
        let html_hash = format!("{:x}", Sha256::digest(html.as_bytes()));
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

    fn is_archive_url(url: &str) -> bool {
        let lower = url.to_lowercase();
        lower.ends_with(".zip") || lower.ends_with(".tar.gz") || lower.ends_with(".tgz")
    }

    fn normalize_finbox_url(url: &str) -> String {
        if url.starts_with("http://") || url.starts_with("https://") {
            return url.to_string();
        }

        if url.starts_with("//") {
            return format!("https:{url}");
        }

        if url.starts_with('/') {
            return format!("https://finbox.jd.com{url}");
        }

        url.to_string()
    }

    fn key_from_url(url: &str) -> String {
        url.trim_end_matches('/')
            .rsplit('/')
            .next()
            .map(Self::slugify)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| Self::slugify(url))
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

    fn selector(css: &str) -> Result<Selector> {
        Selector::parse(css).map_err(|e| anyhow!("Invalid Finbox parser selector '{}': {:?}", css, e))
    }

    fn element_text(element: scraper::ElementRef<'_>) -> String {
        element.text().collect::<Vec<_>>().join(" ").trim().to_string()
    }
}
