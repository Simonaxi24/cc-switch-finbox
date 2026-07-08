# CC-Switch 增强改造实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 cc-switch 添加 Finbox Skill 商场、全局+项目级 Skill 管理、Joybuilder 供应商预设，最终构建 macOS + Windows 安装包。

**Architecture:** 在现有 Tauri 2 (Rust + React) 分层架构上渐进扩展。后端遵循 Commands → Services → DAO → Database 模式，前端遵循 Components → Hooks → TanStack Query → Tauri IPC 模式。数据库迁移统一在 v12 中完成。

**Tech Stack:** Rust (Tauri 2, reqwest, scraper, rusqlite, tokio), TypeScript (React 18, TanStack Query v5, shadcn/ui, TailwindCSS)

## Global Constraints

- 数据库当前版本 v11，新增迁移为 v12，SCHEMA_VERSION 从 11 升到 12
- 所有 Rust Tauri 命令返回 `Result<T, String>`，错误通过 `.map_err(|e| e.to_string())` 转换
- 前端通过 `invoke()` 调用 Tauri 命令，类型定义在 `src/lib/api/skills.ts`
- 版本号三处同步更新：`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`、`package.json`
- 新增列必须有默认值，确保迁移兼容现有数据
- i18n key 前缀：skills 相关为 `skills.`，providers 相关为 `providerForm.`
- Joybuilder 预设中 API URL 和模型名暂用占位符

---

## File Structure

| Operation | Path | Responsibility |
|-----------|------|----------------|
| Create | `src-tauri/src/services/finbox_marketplace.rs` | Finbox 爬取+解析+缓存 |
| Create | `src-tauri/src/commands/finbox_marketplace.rs` | Finbox Tauri 命令层 |
| Create | `src/components/skills/FinboxMarketplacePanel.tsx` | Finbox 商场浏览/搜索/安装 UI |
| Create | `src/components/skills/SkillActionButtons.tsx` | 统一安装/更新/卸载按钮组件 |
| Create | `src/assets/providers/joybuilder.svg` | Joybuilder logo 占位符 |
| Create | `src/lib/api/finbox.ts` | Finbox 前端 API 封装 |
| Create | `src/hooks/useFinbox.ts` | Finbox React Query hooks |
| Modify | `src-tauri/src/database/mod.rs` | SCHEMA_VERSION 11 → 12 |
| Modify | `src-tauri/src/database/schema.rs` | v12 迁移函数 |
| Modify | `src-tauri/src/app_config.rs` | InstalledSkill 添加 scope/project_path 字段 |
| Modify | `src-tauri/src/services/skill.rs` | install/get 方法支持 scope |
| Modify | `src-tauri/src/commands/skill.rs` | 命令添加 scope 参数 |
| Modify | `src-tauri/src/services/mod.rs` | 导出 FinboxMarketplaceService |
| Modify | `src-tauri/src/commands/mod.rs` | 导出 finbox_marketplace 模块 |
| Modify | `src-tauri/src/lib.rs` | 注册新命令和新 service 状态 |
| Modify | `src-tauri/Cargo.toml` | 添加 scraper 依赖 |
| Modify | `src/lib/api/skills.ts` | InstalledSkill 类型添加 scope/projectPath |
| Modify | `src/components/skills/UnifiedSkillsPanel.tsx` | 添加 Finbox Tab + scope 切换 |
| Modify | `src/config/claudeProviderPresets.ts` | Joybuilder 预设 |
| Modify | `src/config/codexProviderPresets.ts` | Joybuilder 预设 |
| Modify | `src/config/geminiProviderPresets.ts` | Joybuilder 预设 |
| Modify | `src/config/universalProviderPresets.ts` | Joybuilder 预设 |
| Modify | `src-tauri/tauri.conf.json` | 版本号 3.16.5 → 3.17.0 |
| Modify | `package.json` | 版本号 3.16.5 → 3.17.0 |

---

### Task 1: 数据库迁移 v12

**Files:**
- Modify: `src-tauri/src/database/mod.rs:52` (SCHEMA_VERSION)
- Modify: `src-tauri/src/database/schema.rs` (添加 migrate_v11_to_v12)

**Interfaces:**
- Produces: `skills` 表新增 `scope TEXT NOT NULL DEFAULT 'global'` 和 `project_path TEXT` 列；新增 `finbox_skill_cache` 表

- [ ] **Step 1: 更新 SCHEMA_VERSION 常量**

在 `src-tauri/src/database/mod.rs` 第 52 行，将：
```rust
pub(crate) const SCHEMA_VERSION: i32 = 11;
```
改为：
```rust
pub(crate) const SCHEMA_VERSION: i32 = 12;
```

- [ ] **Step 2: 在 schema.rs 的 apply_schema_migrations_on_conn 中添加 v11 → v12 match arm**

在 `src-tauri/src/database/schema.rs` 的 `apply_schema_migrations_on_conn` 函数中，在 `10 =>` 分支之后添加：

```rust
11 => {
    log::info!("迁移数据库从 v11 到 v12（Finbox Skill 商场缓存 + Skill 作用域支持）");
    Self::migrate_v11_to_v12(conn)?;
    Self::set_user_version(conn, 12)?;
}
```

- [ ] **Step 3: 实现 migrate_v11_to_v12 函数**

在 `src-tauri/src/database/schema.rs` 末尾（`migrate_v10_to_v11` 之后）添加：

```rust
/// v11 -> v12：添加 Finbox Skill 缓存表 + Skills 作用域支持
fn migrate_v11_to_v12(conn: &Connection) -> Result<(), AppError> {
    // 创建 Finbox Skill 缓存表
    conn.execute(
        "CREATE TABLE IF NOT EXISTS finbox_skill_cache (
            key TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            download_url TEXT,
            category TEXT,
            raw_html_hash TEXT,
            cached_at INTEGER NOT NULL,
            expires_at INTEGER NOT NULL
        )",
        [],
    )
    .map_err(|e| AppError::Database(format!("创建 finbox_skill_cache 表失败: {e}")))?;

    // Skills 表添加作用域字段
    if Self::table_exists(conn, "skills")? {
        Self::add_column_if_missing(
            conn,
            "skills",
            "scope",
            "TEXT NOT NULL DEFAULT 'global'",
        )?;
        Self::add_column_if_missing(
            conn,
            "skills",
            "project_path",
            "TEXT",
        )?;
    }

    log::info!("v11 -> v12 迁移完成：Finbox Skill 缓存表 + Skills 作用域");
    Ok(())
}
```

- [ ] **Step 4: 在 create_tables_on_conn 中添加 finbox_skill_cache 建表和 skills 表新列**

在 `create_tables_on_conn` 中，在 skill_repos 表创建之后、settings 表创建之前，添加：

```rust
// 6.5 Finbox Skill 缓存表
conn.execute(
    "CREATE TABLE IF NOT EXISTS finbox_skill_cache (
        key TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        description TEXT,
        download_url TEXT,
        category TEXT,
        raw_html_hash TEXT,
        cached_at INTEGER NOT NULL,
        expires_at INTEGER NOT NULL
    )",
    [],
)
.map_err(|e| AppError::Database(e.to_string()))?;
```

在 skills 表的 CREATE TABLE 语句中，在 `updated_at INTEGER NOT NULL DEFAULT 0` 之后添加两列：

```sql
scope TEXT NOT NULL DEFAULT 'global',
project_path TEXT
```

即 skills 的 CREATE TABLE 变为：
```sql
CREATE TABLE IF NOT EXISTS skills (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    directory TEXT NOT NULL,
    repo_owner TEXT,
    repo_name TEXT,
    repo_branch TEXT DEFAULT 'main',
    readme_url TEXT,
    enabled_claude BOOLEAN NOT NULL DEFAULT 0,
    enabled_codex BOOLEAN NOT NULL DEFAULT 0,
    enabled_gemini BOOLEAN NOT NULL DEFAULT 0,
    enabled_opencode BOOLEAN NOT NULL DEFAULT 0,
    enabled_hermes BOOLEAN NOT NULL DEFAULT 0,
    installed_at INTEGER NOT NULL DEFAULT 0,
    content_hash TEXT,
    updated_at INTEGER NOT NULL DEFAULT 0,
    scope TEXT NOT NULL DEFAULT 'global',
    project_path TEXT
)
```

- [ ] **Step 5: 编译验证**

Run: `cd /Users/xixinyu.simona/projects/体系培训/hw3/cc-switch/src-tauri && cargo check 2>&1 | tail -5`
Expected: 编译成功（可能有 warning 但无 error）

- [ ] **Step 6: Commit**

```bash
cd /Users/xixinyu.simona/projects/体系培训/hw3/cc-switch
git add src-tauri/src/database/mod.rs src-tauri/src/database/schema.rs
git commit -m "feat(db): add v12 migration for finbox cache and skill scope"
```

---

### Task 2: InstalledSkill 数据结构扩展

**Files:**
- Modify: `src-tauri/src/app_config.rs:169` (InstalledSkill struct)
- Modify: `src/lib/api/skills.ts:26` (前端 InstalledSkill 接口)

**Interfaces:**
- Consumes: Task 1 的数据库列定义
- Produces: `InstalledSkill.scope: String`（默认 `"global"`），`InstalledSkill.project_path: Option<String>`

- [ ] **Step 1: 修改 Rust InstalledSkill 结构体**

在 `src-tauri/src/app_config.rs` 的 `InstalledSkill` 结构体中，在 `updated_at` 字段之后添加：

```rust
    /// 作用域：'global'（全局）或 'project'（项目级）
    #[serde(default = "default_scope")]
    pub scope: String,
    /// 项目级 skill 所属的项目路径（全局 skill 为 None）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_path: Option<String>,
```

在同一文件的 `InstalledSkill` impl 块之前添加辅助函数：

```rust
fn default_scope() -> String {
    "global".to_string()
}
```

- [ ] **Step 2: 更新 skills 表的读取逻辑**

在 `src-tauri/src/services/skill.rs` 中，找到 `get_all_installed` 和其他从数据库读取 `InstalledSkill` 的函数，在 SELECT 语句中添加 `scope, project_path` 列，在 row mapping 中添加：

```rust
let scope: String = row.get("scope").unwrap_or_else(|_| "global".to_string());
let project_path: Option<String> = row.get("project_path").ok();
```

并在构建 `InstalledSkill` 时添加这两个字段。

需要搜索所有 `InstalledSkill` 的构造点（`InstalledSkill { ... }`），每个都要添加：
```rust
scope: "global".to_string(),  // 或从参数传入
project_path: None,            // 或从参数传入
```

- [ ] **Step 3: 修改前端 InstalledSkill 接口**

在 `src/lib/api/skills.ts` 的 `InstalledSkill` 接口中，在 `updatedAt` 之后添加：

```typescript
  /** 作用域：'global' 或 'project' */
  scope: "global" | "project";
  /** 项目级 skill 所属的项目路径 */
  projectPath?: string;
```

- [ ] **Step 4: 编译验证**

Run: `cd /Users/xixinyu.simona/projects/体系培训/hw3/cc-switch && pnpm typecheck 2>&1 | tail -10`
Expected: 类型检查可能有新字段相关错误（后续 Task 修复），Rust 侧需 `cargo check` 通过

Run: `cd /Users/xixinyu.simona/projects/体系培训/hw3/cc-switch/src-tauri && cargo check 2>&1 | tail -5`
Expected: 编译成功

- [ ] **Step 5: Commit**

```bash
cd /Users/xixinyu.simona/projects/体系培训/hw3/cc-switch
git add src-tauri/src/app_config.rs src-tauri/src/services/skill.rs src/lib/api/skills.ts
git commit -m "feat(skills): add scope and project_path fields to InstalledSkill"
```

---

### Task 3: Skill Service 支持 scope 参数

**Files:**
- Modify: `src-tauri/src/services/skill.rs` (install_skill_unified 支持 scope)
- Modify: `src-tauri/src/commands/skill.rs` (命令添加 scope 参数)

**Interfaces:**
- Consumes: Task 2 的 InstalledSkill 新字段
- Produces: `install_skill_unified(scope, project_path)` 支持；`get_installed_skills` 返回带 scope 的数据；`get_project_skills(project_path)` 新命令

- [ ] **Step 1: 修改 SkillService::install 方法签名**

在 `src-tauri/src/services/skill.rs` 中，修改 `install` 方法签名，添加 scope 参数：

```rust
pub async fn install(&self, db: &Arc<Database>, skill: &DiscoverableSkill, current_app: &AppType, scope: &str, project_path: Option<&str>) -> Result<InstalledSkill>
```

在函数体中，根据 scope 决定安装目标目录：

```rust
let install_base_dir = if scope == "project" {
    let pp = project_path.ok_or_else(|| anyhow::anyhow!("项目级 skill 必须指定 project_path"))?;
    PathBuf::from(pp).join(".claude").join("skills")
} else {
    Self::get_ssot_dir()?
};
```

在写入数据库时，INSERT 语句添加 `scope` 和 `project_path` 列。

- [ ] **Step 2: 修改 get_all_installed 支持按 scope 过滤**

在 `src-tauri/src/services/skill.rs` 中，添加新方法：

```rust
/// 获取全局 skill + 指定项目的项目级 skill
pub fn get_skills_by_scope(db: &Arc<Database>, project_path: Option<&str>) -> Result<Vec<InstalledSkill>> {
    let conn = lock_conn!(db.conn);
    let skills = if let Some(pp) = project_path {
        // 全局 + 当前项目
        let mut stmt = conn.prepare(
            "SELECT * FROM skills WHERE scope = 'global' OR (scope = 'project' AND project_path = ?1) ORDER BY name"
        )?;
        stmt.query_map(rusqlite::params![pp], |row| {
            Self::row_to_installed_skill(row)
        })?.collect::<Result<Vec<_>, _>>()
    } else {
        // 仅全局
        let mut stmt = conn.prepare(
            "SELECT * FROM skills WHERE scope = 'global' ORDER BY name"
        )?;
        stmt.query_map([], |row| {
            Self::row_to_installed_skill(row)
        })?.collect::<Result<Vec<_>, _>>()
    }.map_err(|e| AppError::Database(e.to_string()))?;
    Ok(skills)
}
```

同时添加 `row_to_installed_skill` 辅助函数（将重复的 row mapping 逻辑抽取出来）。

- [ ] **Step 3: 修改 Tauri 命令添加 scope 参数**

在 `src-tauri/src/commands/skill.rs` 中，修改 `install_skill_unified` 命令：

```rust
#[tauri::command]
pub async fn install_skill_unified(
    skill: DiscoverableSkill,
    current_app: String,
    scope: Option<String>,
    project_path: Option<String>,
    service: State<'_, SkillServiceState>,
    app_state: State<'_, AppState>,
) -> Result<InstalledSkill, String> {
    let app = parse_app_type(&current_app)?;
    let s = scope.unwrap_or_else(|| "global".to_string());
    service.0.install(&app_state.db, &skill, &app, &s, project_path.as_deref())
        .await
        .map_err(|e| e.to_string())
}
```

修改 `get_installed_skills` 命令：

```rust
#[tauri::command]
pub fn get_installed_skills(
    app_state: State<'_, AppState>,
    project_path: Option<String>,
) -> Result<Vec<InstalledSkill>, String> {
    SkillService::get_skills_by_scope(&app_state.db, project_path.as_deref())
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 4: 编译验证**

Run: `cd /Users/xixinyu.simona/projects/体系培训/hw3/cc-switch/src-tauri && cargo check 2>&1 | tail -5`
Expected: 编译成功

- [ ] **Step 5: Commit**

```bash
cd /Users/xixinyu.simona/projects/体系培训/hw3/cc-switch
git add src-tauri/src/services/skill.rs src-tauri/src/commands/skill.rs
git commit -m "feat(skills): support scope parameter in install and query"
```

---

### Task 4: Finbox Marketplace Service（后端）

**Files:**
- Modify: `src-tauri/Cargo.toml` (添加 scraper 依赖)
- Create: `src-tauri/src/services/finbox_marketplace.rs`
- Modify: `src-tauri/src/services/mod.rs` (导出新模块)

**Interfaces:**
- Consumes: Task 1 的 finbox_skill_cache 表
- Produces: `FinboxMarketplaceService` 及其公共方法：`search_skills`, `get_skill_detail`, `install_skill`, `refresh_cache`

- [ ] **Step 1: 添加 scraper 依赖**

在 `src-tauri/Cargo.toml` 的 `[dependencies]` 中添加：

```toml
scraper = "0.22"
```

- [ ] **Step 2: 创建 FinboxMarketplaceService**

创建 `src-tauri/src/services/finbox_marketplace.rs`：

```rust
use crate::database::Database;
use crate::error::AppError;
use crate::services::skill::{DiscoverableSkill, SkillService};
use anyhow::Result;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

const FINBOX_URL: &str = "https://finbox.jd.com/coverage";
const CACHE_TTL_SECONDS: i64 = 3600; // 1 小时

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

impl FinboxMarketplaceService {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("CC-Switch/3.17.0")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_default();
        Self { client }
    }

    /// 搜索 Finbox 商场中的 skill（优先从缓存读取）
    pub async fn search_skills(&self, db: &Arc<Database>, query: Option<&str>) -> Result<Vec<FinboxSkill>> {
        let cached = self.get_cached_skills(db)?;
        let filtered = if let Some(q) = query {
            let q_lower = q.to_lowercase();
            cached.into_iter()
                .filter(|s| s.name.to_lowercase().contains(&q_lower)
                    || s.description.as_ref().map_or(false, |d| d.to_lowercase().contains(&q_lower)))
                .collect()
        } else {
            cached
        };
        Ok(filtered)
    }

    /// 获取单个 skill 详情（优先从缓存读取）
    pub async fn get_skill_detail(&self, db: &Arc<Database>, key: &str) -> Result<FinboxSkillDetail> {
        let cached = self.get_cached_skill(db, key)?;
        if let Some(skill) = cached {
            return Ok(FinboxSkillDetail {
                key: skill.key,
                name: skill.name,
                description: skill.description,
                download_url: skill.download_url,
                category: skill.category,
                readme: None,
            });
        }
        // 缓存未命中，刷新后重试
        self.refresh_cache(db).await?;
        let skill = self.get_cached_skill(db, key)?
            .ok_or_else(|| anyhow::anyhow!("Skill '{}' not found on Finbox", key))?;
        Ok(FinboxSkillDetail {
            key: skill.key,
            name: skill.name,
            description: skill.description,
            download_url: skill.download_url,
            category: skill.category,
            readme: None,
        })
    }

    /// 从 Finbox 安装 skill 到 CC Switch
    pub async fn install_skill(&self, db: &Arc<Database>, key: &str, skill_service: &SkillService, current_app: &str) -> Result<crate::app_config::InstalledSkill> {
        let detail = self.get_skill_detail(db, key).await?;
        let download_url = detail.download_url
            .ok_or_else(|| anyhow::anyhow!("Skill '{}' has no download URL", key))?;

        // 判断下载类型：ZIP 还是 Git repo
        if download_url.ends_with(".zip") || download_url.ends_with(".tar.gz") {
            // 下载 ZIP 并通过 install_from_zip 安装
            let temp_dir = tempfile::tempdir()?;
            let zip_path = temp_dir.path().join(format!("{}.zip", key.replace('/', '_')));
            let response = self.client.get(&download_url).send().await?;
            let bytes = response.bytes().await?;
            std::fs::write(&zip_path, &bytes)?;
            let app_type = crate::app_config::AppType::from_str(current_app)
                .map_err(|_| anyhow::anyhow!("Invalid app type: {}", current_app))?;
            SkillService::install_from_zip(db, &zip_path, &app_type)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?
                .into_iter()
                .next()
                .ok_or_else(|| anyhow::anyhow!("No skill installed from ZIP"))
        } else {
            // Git repo URL：构造 DiscoverableSkill 并安装
            let skill = DiscoverableSkill {
                key: key.to_string(),
                name: detail.name.clone(),
                description: detail.description.clone().unwrap_or_default(),
                directory: key.replace('/', "-"),
                readme_url: None,
                repo_owner: "".to_string(),
                repo_name: "".to_string(),
                repo_branch: "main".to_string(),
            };
            let app_type = crate::app_config::AppType::from_str(current_app)
                .map_err(|_| anyhow::anyhow!("Invalid app type: {}", current_app))?;
            skill_service.install(db, &skill, &app_type, "global", None).await
        }
    }

    /// 强制刷新缓存
    pub async fn refresh_cache(&self, db: &Arc<Database>) -> Result<()> {
        let html = self.fetch_page().await?;
        let skills = self.parse_skills(&html)?;
        self.save_to_cache(db, &skills, &html)?;
        Ok(())
    }

    async fn fetch_page(&self) -> Result<String> {
        let response = self.client.get(FINBOX_URL).send().await?;
        let html = response.text().await?;
        Ok(html)
    }

    fn parse_skills(&self, html: &str) -> Result<Vec<FinboxSkill>> {
        let document = Html::parse_document(html);

        // 选择器需根据 finbox.jd.com/coverage 实际页面结构调整
        // 以下为通用模板，首次运行后需根据实际 HTML 修正
        let item_selector = Selector::parse("div.skill-item, tr.skill-row, li.skill-entry, div[class*='skill'], div[class*='coverage']").unwrap();

        let mut skills = Vec::new();
        for element in document.select(&item_selector) {
            let name = element.select(&Selector::parse("h3, h2, .name, .title, a").unwrap())
                .next()
                .map(|e| e.text().collect::<String>().trim().to_string())
                .unwrap_or_default();

            if name.is_empty() {
                continue;
            }

            let description = element.select(&Selector::parse("p, .desc, .description, .summary").unwrap())
                .next()
                .map(|e| e.text().collect::<String>().trim().to_string());

            let link = element.select(&Selector::parse("a[href]").unwrap())
                .next()
                .and_then(|e| e.value().attr("href").map(String::from));

            let key = link.clone().unwrap_or_else(|| name.replace(' ', "-").to_lowercase());

            skills.push(FinboxSkill {
                key,
                name,
                description,
                download_url: link,
                category: None,
            });
        }

        Ok(skills)
    }

    fn save_to_cache(&self, db: &Arc<Database>, skills: &[FinboxSkill], html: &str) -> Result<()> {
        use sha2::{Sha256, Digest};
        let html_hash = format!("{:x}", Sha256::digest(html.as_bytes()));
        let now = chrono::Utc::now().timestamp();
        let expires_at = now + CACHE_TTL_SECONDS;

        let conn = Database::lock_conn(&db.conn);
        conn.execute("DELETE FROM finbox_skill_cache", [])
            .map_err(|e| AppError::Database(e.to_string()))?;

        for skill in skills {
            conn.execute(
                "INSERT OR REPLACE INTO finbox_skill_cache (key, name, description, download_url, category, raw_html_hash, cached_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
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
            ).map_err(|e| AppError::Database(e.to_string()))?;
        }
        Ok(())
    }

    fn get_cached_skills(&self, db: &Arc<Database>) -> Result<Vec<FinboxSkill>> {
        let conn = Database::lock_conn(&db.conn);
        let now = chrono::Utc::now().timestamp();

        let mut stmt = conn.prepare(
            "SELECT key, name, description, download_url, category FROM finbox_skill_cache WHERE expires_at > ?1"
        ).map_err(|e| AppError::Database(e.to_string()))?;

        let skills = stmt.query_map(rusqlite::params![now], |row| {
            Ok(FinboxSkill {
                key: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                download_url: row.get(3)?,
                category: row.get(4)?,
            })
        }).map_err(|e| AppError::Database(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::Database(e.to_string()))?;

        // 如果缓存过期或为空，在后台刷新（非阻塞）
        if skills.is_empty() {
            drop(stmt);
            drop(conn);
            // 返回空列表，前端会触发 refresh
        }

        Ok(skills)
    }

    fn get_cached_skill(&self, db: &Arc<Database>, key: &str) -> Result<Option<FinboxSkill>> {
        let skills = self.get_cached_skills(db)?;
        Ok(skills.into_iter().find(|s| s.key == key))
    }
}
```

注意：`parse_skills` 中的 CSS 选择器是占位符，需要实际访问 finbox.jd.com/coverage 后根据真实 HTML 结构修正。

- [ ] **Step 3: 在 services/mod.rs 中导出**

在 `src-tauri/src/services/mod.rs` 中添加：

```rust
pub mod finbox_marketplace;
pub use finbox_marketplace::FinboxMarketplaceService;
```

- [ ] **Step 4: 编译验证**

Run: `cd /Users/xixinyu.simona/projects/体系培训/hw3/cc-switch/src-tauri && cargo check 2>&1 | tail -10`
Expected: 可能需要修复一些编译错误（如 `AppType::from_str` 是否存在、`Database::lock_conn` 的调用方式等），逐一修复直到编译通过

- [ ] **Step 5: Commit**

```bash
cd /Users/xixinyu.simona/projects/体系培训/hw3/cc-switch
git add src-tauri/Cargo.toml src-tauri/src/services/finbox_marketplace.rs src-tauri/src/services/mod.rs
git commit -m "feat(finbox): add FinboxMarketplaceService for web scraping"
```

---

### Task 5: Finbox Marketplace 命令层

**Files:**
- Create: `src-tauri/src/commands/finbox_marketplace.rs`
- Modify: `src-tauri/src/commands/mod.rs` (导出新模块)
- Modify: `src-tauri/src/lib.rs` (注册命令和状态)

**Interfaces:**
- Consumes: Task 4 的 `FinboxMarketplaceService`
- Produces: Tauri 命令 `search_finbox_skills`, `get_finbox_skill_detail`, `install_from_finbox`, `refresh_finbox_cache`

- [ ] **Step 1: 创建命令文件**

创建 `src-tauri/src/commands/finbox_marketplace.rs`：

```rust
use crate::app_config::InstalledSkill;
use crate::services::finbox_marketplace::{FinboxMarketplaceService, FinboxSkill, FinboxSkillDetail};
use crate::services::skill::SkillService;
use std::sync::Arc;
use tauri::State;

pub struct FinboxServiceState(pub Arc<FinboxMarketplaceService>);

#[tauri::command]
pub async fn search_finbox_skills(
    query: Option<String>,
    service: State<'_, FinboxServiceState>,
    app_state: State<'_, crate::commands::AppState>,
) -> Result<Vec<FinboxSkill>, String> {
    service.0.search_skills(&app_state.db, query.as_deref())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_finbox_skill_detail(
    key: String,
    service: State<'_, FinboxServiceState>,
    app_state: State<'_, crate::commands::AppState>,
) -> Result<FinboxSkillDetail, String> {
    service.0.get_skill_detail(&app_state.db, &key)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn install_from_finbox(
    key: String,
    current_app: String,
    finbox_service: State<'_, FinboxServiceState>,
    skill_service: State<'_, crate::commands::skill::SkillServiceState>,
    app_state: State<'_, crate::commands::AppState>,
) -> Result<InstalledSkill, String> {
    finbox_service.0.install_skill(&app_state.db, &key, &skill_service.0, &current_app)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn refresh_finbox_cache(
    service: State<'_, FinboxServiceState>,
    app_state: State<'_, crate::commands::AppState>,
) -> Result<bool, String> {
    service.0.refresh_cache(&app_state.db)
        .await
        .map_err(|e| e.to_string())?;
    Ok(true)
}
```

- [ ] **Step 2: 在 commands/mod.rs 中导出**

在 `src-tauri/src/commands/mod.rs` 中添加：

```rust
pub mod finbox_marketplace;
```

- [ ] **Step 3: 在 lib.rs 中注册状态和命令**

在 `src-tauri/src/lib.rs` 的 `setup` 闭包中（`SkillService` 初始化之后）添加：

```rust
let finbox_service = FinboxMarketplaceService::new();
app.manage(commands::finbox_marketplace::FinboxServiceState(Arc::new(finbox_service)));
```

在 `generate_handler![]` 宏中添加：

```rust
commands::finbox_marketplace::search_finbox_skills,
commands::finbox_marketplace::get_finbox_skill_detail,
commands::finbox_marketplace::install_from_finbox,
commands::finbox_marketplace::refresh_finbox_cache,
```

在 `lib.rs` 顶部的 `pub use services::` 中添加 `FinboxMarketplaceService`。

- [ ] **Step 4: 编译验证**

Run: `cd /Users/xixinyu.simona/projects/体系培训/hw3/cc-switch/src-tauri && cargo check 2>&1 | tail -10`
Expected: 编译成功

- [ ] **Step 5: Commit**

```bash
cd /Users/xixinyu.simona/projects/体系培训/hw3/cc-switch
git add src-tauri/src/commands/finbox_marketplace.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat(finbox): add Tauri commands for Finbox marketplace"
```

---

### Task 6: Finbox 前端 API + Hooks

**Files:**
- Create: `src/lib/api/finbox.ts`
- Create: `src/hooks/useFinbox.ts`

**Interfaces:**
- Consumes: Task 5 的 Tauri 命令
- Produces: `finboxApi` 对象和 `useFinboxSkills`, `useFinboxSkillDetail`, `useInstallFromFinbox`, `useRefreshFinboxCache` hooks

- [ ] **Step 1: 创建前端 API 封装**

创建 `src/lib/api/finbox.ts`：

```typescript
import { invoke } from "@tauri-apps/api/core";

export interface FinboxSkill {
  key: string;
  name: string;
  description?: string;
  downloadUrl?: string;
  category?: string;
}

export interface FinboxSkillDetail {
  key: string;
  name: string;
  description?: string;
  downloadUrl?: string;
  category?: string;
  readme?: string;
}

export const finboxApi = {
  async searchSkills(query?: string): Promise<FinboxSkill[]> {
    return await invoke("search_finbox_skills", { query: query ?? null });
  },

  async getSkillDetail(key: string): Promise<FinboxSkillDetail> {
    return await invoke("get_finbox_skill_detail", { key });
  },

  async installFromFinbox(
    key: string,
    currentApp: string,
  ): Promise<import("./skills").InstalledSkill> {
    return await invoke("install_from_finbox", { key, currentApp });
  },

  async refreshCache(): Promise<boolean> {
    return await invoke("refresh_finbox_cache");
  },
};
```

- [ ] **Step 2: 创建 React Query hooks**

创建 `src/hooks/useFinbox.ts`：

```typescript
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { finboxApi } from "@/lib/api/finbox";

export function useFinboxSkills(query?: string) {
  return useQuery({
    queryKey: ["finbox-skills", query],
    queryFn: () => finboxApi.searchSkills(query),
    staleTime: 30 * 60 * 1000, // 30 分钟
  });
}

export function useFinboxSkillDetail(key: string | null) {
  return useQuery({
    queryKey: ["finbox-skill-detail", key],
    queryFn: () => finboxApi.getSkillDetail(key!),
    enabled: !!key,
  });
}

export function useInstallFromFinbox() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ key, currentApp }: { key: string; currentApp: string }) =>
      finboxApi.installFromFinbox(key, currentApp),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["installed-skills"] });
    },
  });
}

export function useRefreshFinboxCache() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () => finboxApi.refreshCache(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["finbox-skills"] });
    },
  });
}
```

- [ ] **Step 3: Commit**

```bash
cd /Users/xixinyu.simona/projects/体系培训/hw3/cc-switch
git add src/lib/api/finbox.ts src/hooks/useFinbox.ts
git commit -m "feat(finbox): add frontend API and React Query hooks"
```

---

### Task 7: Finbox 商场 UI

**Files:**
- Create: `src/components/skills/FinboxMarketplacePanel.tsx`
- Modify: `src/components/skills/UnifiedSkillsPanel.tsx` (添加 Finbox Tab)

**Interfaces:**
- Consumes: Task 6 的 hooks
- Produces: Finbox 商场浏览/搜索/安装界面

- [ ] **Step 1: 创建 FinboxMarketplacePanel 组件**

创建 `src/components/skills/FinboxMarketplacePanel.tsx`：

```tsx
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Search, RefreshCw, Download, ExternalLink } from "lucide-react";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { toast } from "sonner";
import { useFinboxSkills, useInstallFromFinbox, useRefreshFinboxCache } from "@/hooks/useFinbox";
import type { AppId } from "@/lib/api/types";

interface FinboxMarketplacePanelProps {
  currentApp: AppId;
}

export function FinboxMarketplacePanel({ currentApp }: FinboxMarketplacePanelProps) {
  const { t } = useTranslation();
  const [searchQuery, setSearchQuery] = useState("");
  const { data: skills, isLoading, error } = useFinboxSkills(searchQuery || undefined);
  const installMutation = useInstallFromFinbox();
  const refreshMutation = useRefreshFinboxCache();

  const handleInstall = (key: string) => {
    installMutation.mutate(
      { key, currentApp },
      {
        onSuccess: () => toast.success(t("skills.installSuccess")),
        onError: (err) => toast.error(t("skills.installFailed") + ": " + err.message),
      },
    );
  };

  const handleRefresh = () => {
    refreshMutation.mutate(undefined, {
      onSuccess: () => toast.success(t("skills.cacheRefreshed")),
      onError: (err) => toast.error(t("skills.cacheRefreshFailed") + ": " + err.message),
    });
  };

  if (error) {
    return (
      <div className="flex flex-col items-center justify-center py-12 text-muted-foreground">
        <p>{t("skills.finboxLoadError")}</p>
        <Button variant="outline" onClick={handleRefresh} className="mt-4">
          <RefreshCw className="mr-2 h-4 w-4" />
          {t("skills.retry")}
        </Button>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center gap-2">
        <div className="relative flex-1">
          <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            placeholder={t("skills.searchFinbox")}
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="pl-9"
          />
        </div>
        <Button variant="outline" size="icon" onClick={handleRefresh} disabled={refreshMutation.isPending}>
          <RefreshCw className={`h-4 w-4 ${refreshMutation.isPending ? "animate-spin" : ""}`} />
        </Button>
      </div>

      {isLoading ? (
        <div className="flex items-center justify-center py-12 text-muted-foreground">
          <RefreshCw className="mr-2 h-4 w-4 animate-spin" />
          {t("skills.loading")}
        </div>
      ) : !skills?.length ? (
        <div className="flex flex-col items-center justify-center py-12 text-muted-foreground">
          <p>{t("skills.noFinboxSkills")}</p>
        </div>
      ) : (
        <div className="space-y-2">
          {skills.map((skill) => (
            <div
              key={skill.key}
              className="flex items-center justify-between rounded-lg border p-3"
            >
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2">
                  <span className="font-medium truncate">{skill.name}</span>
                  {skill.category && (
                    <Badge variant="secondary" className="text-xs">
                      {skill.category}
                    </Badge>
                  )}
                </div>
                {skill.description && (
                  <p className="text-sm text-muted-foreground mt-1 line-clamp-2">
                    {skill.description}
                  </p>
                )}
              </div>
              <div className="flex items-center gap-2 ml-4">
                {skill.downloadUrl && (
                  <a href={skill.downloadUrl} target="_blank" rel="noopener noreferrer">
                    <Button variant="ghost" size="icon">
                      <ExternalLink className="h-4 w-4" />
                    </Button>
                  </a>
                )}
                <Button
                  size="sm"
                  onClick={() => handleInstall(skill.key)}
                  disabled={installMutation.isPending}
                >
                  <Download className="mr-1 h-3 w-3" />
                  {t("skills.install")}
                </Button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 2: 在 UnifiedSkillsPanel 中添加 Finbox Tab**

在 `src/components/skills/UnifiedSkillsPanel.tsx` 中：

1. 导入组件：
```typescript
import { FinboxMarketplacePanel } from "./FinboxMarketplacePanel";
```

2. 在现有 Tab 列表中添加 "Finbox" Tab（找到现有的 tabs 定义，添加一个新的 tab item）。

3. 在 Tab 内容区域添加渲染逻辑：
```tsx
{activeTab === "finbox" && (
  <FinboxMarketplacePanel currentApp={currentApp} />
)}
```

- [ ] **Step 3: 添加 i18n key**

在 `src/locales/zh.json` 的 `skills` 部分添加：
```json
"searchFinbox": "搜索 Finbox Skill...",
"finboxLoadError": "无法加载 Finbox 商场数据",
"cacheRefreshed": "缓存已刷新",
"cacheRefreshFailed": "缓存刷新失败",
"noFinboxSkills": "暂无 Finbox Skill",
"retry": "重试",
"installSuccess": "安装成功",
"installFailed": "安装失败"
```

在 `src/locales/en.json` 对应添加英文翻译。

- [ ] **Step 4: Commit**

```bash
cd /Users/xixinyu.simona/projects/体系培训/hw3/cc-switch
git add src/components/skills/FinboxMarketplacePanel.tsx src/components/skills/UnifiedSkillsPanel.tsx src/locales/
git commit -m "feat(finbox): add Finbox marketplace UI panel"
```

---

### Task 8: 全局+项目级 Skill 管理 UI

**Files:**
- Create: `src/components/skills/SkillActionButtons.tsx`
- Modify: `src/components/skills/UnifiedSkillsPanel.tsx` (添加 scope Tab + 项目选择)

**Interfaces:**
- Consumes: Task 3 的 scope 参数支持
- Produces: 全局/项目级 skill 切换界面，统一操作按钮

- [ ] **Step 1: 创建 SkillActionButtons 组件**

创建 `src/components/skills/SkillActionButtons.tsx`：

```tsx
import { useTranslation } from "react-i18next";
import { Download, RefreshCw, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";

interface SkillActionButtonsProps {
  scope: "global" | "project";
  skillId: string;
  skillName: string;
  onUpdate?: () => void;
  onUninstall?: () => void;
  isUpdating?: boolean;
  isUninstalling?: boolean;
}

export function SkillActionButtons({
  scope,
  skillId,
  skillName,
  onUpdate,
  onUninstall,
  isUpdating,
  isUninstalling,
}: SkillActionButtonsProps) {
  const { t } = useTranslation();

  return (
    <div className="flex items-center gap-1">
      {onUpdate && (
        <Button
          variant="ghost"
          size="icon"
          onClick={onUpdate}
          disabled={isUpdating}
          title={t("skills.update")}
        >
          <RefreshCw className={`h-4 w-4 ${isUpdating ? "animate-spin" : ""}`} />
        </Button>
      )}
      {onUninstall && (
        <Button
          variant="ghost"
          size="icon"
          onClick={onUninstall}
          disabled={isUninstalling}
          title={t("skills.uninstall")}
        >
          <Trash2 className="h-4 w-4" />
        </Button>
      )}
    </div>
  );
}
```

- [ ] **Step 2: 在 UnifiedSkillsPanel 中添加 scope 切换 Tab**

在 `src/components/skills/UnifiedSkillsPanel.tsx` 中：

1. 添加 state：
```typescript
const [skillScope, setSkillScope] = useState<"global" | "project">("global");
const [currentProjectPath, setCurrentProjectPath] = useState<string>("");
```

2. 在已安装 skill 列表上方添加 scope 切换 UI：
```tsx
<div className="flex items-center gap-2 mb-4">
  <Button
    variant={skillScope === "global" ? "default" : "outline"}
    size="sm"
    onClick={() => setSkillScope("global")}
  >
    {t("skills.globalSkills")}
  </Button>
  <Button
    variant={skillScope === "project" ? "default" : "outline"}
    size="sm"
    onClick={() => setSkillScope("project")}
  >
    {t("skills.projectSkills")}
  </Button>
</div>
```

3. 当 `skillScope === "project"` 时，显示项目路径选择器：
```tsx
{skillScope === "project" && (
  <div className="mb-4">
    <Input
      placeholder={t("skills.projectPathPlaceholder")}
      value={currentProjectPath}
      onChange={(e) => setCurrentProjectPath(e.target.value)}
    />
  </div>
)}
```

4. 修改 `useInstalledSkills` 调用，传入 `projectPath` 参数：
```typescript
const { data: installedSkills } = useQuery({
  queryKey: ["installed-skills", skillScope, currentProjectPath],
  queryFn: () => skillsApi.getInstalled(
    skillScope === "project" ? currentProjectPath : undefined
  ),
});
```

5. 在每个 skill 卡片中添加 scope 标签：
```tsx
<Badge variant={skill.scope === "project" ? "secondary" : "outline"} className="text-xs">
  {skill.scope === "project" ? t("skills.projectScope") : t("skills.globalScope")}
</Badge>
```

- [ ] **Step 3: 更新 install 调用以传递 scope**

修改安装 skill 时的调用，添加 `scope` 和 `projectPath` 参数：

```typescript
await skillsApi.installUnified(skill, currentApp, skillScope, skillScope === "project" ? currentProjectPath : undefined);
```

更新 `src/lib/api/skills.ts` 中的 `installUnified` 方法：
```typescript
async installUnified(
  skill: DiscoverableSkill,
  currentApp: AppId,
  scope?: "global" | "project",
  projectPath?: string,
): Promise<InstalledSkill> {
  return await invoke("install_skill_unified", {
    skill,
    currentApp,
    scope: scope ?? "global",
    projectPath: projectPath ?? null,
  });
},
```

更新 `getInstalled` 方法：
```typescript
async getInstalled(projectPath?: string): Promise<InstalledSkill[]> {
  return await invoke("get_installed_skills", { projectPath: projectPath ?? null });
},
```

- [ ] **Step 4: 添加 i18n key**

在 `src/locales/zh.json` 的 `skills` 部分添加：
```json
"globalSkills": "全局 Skills",
"projectSkills": "项目 Skills",
"globalScope": "全局",
"projectScope": "项目级",
"projectPathPlaceholder": "输入项目路径，如 /path/to/project"
```

在 `src/locales/en.json` 对应添加。

- [ ] **Step 5: Commit**

```bash
cd /Users/xixinyu.simona/projects/体系培训/hw3/cc-switch
git add src/components/skills/SkillActionButtons.tsx src/components/skills/UnifiedSkillsPanel.tsx src/lib/api/skills.ts src/locales/
git commit -m "feat(skills): add global/project scope toggle and action buttons"
```

---

### Task 9: Joybuilder 供应商预设

**Files:**
- Modify: `src/config/claudeProviderPresets.ts`
- Modify: `src/config/codexProviderPresets.ts`
- Modify: `src/config/geminiProviderPresets.ts`
- Modify: `src/config/universalProviderPresets.ts`
- Create: `src/assets/providers/joybuilder.svg`

**Interfaces:**
- Consumes: 现有 ProviderPreset / UniversalProviderPreset 接口
- Produces: Joybuilder 预设条目（占位符 URL/模型名）

- [ ] **Step 1: 创建 Joybuilder SVG 图标占位符**

创建 `src/assets/providers/joybuilder.svg`：

```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
  <rect x="3" y="3" width="18" height="18" rx="2"/>
  <path d="M9 12h6M12 9v6"/>
</svg>
```

- [ ] **Step 2: 在 Claude 预设中添加 Joybuilder**

在 `src/config/claudeProviderPresets.ts` 的 `providerPresets` 数组末尾添加：

```typescript
{
  name: "Joybuilder",
  websiteUrl: "https://PLACEHOLDER",
  apiKeyUrl: "https://PLACEHOLDER",
  settingsConfig: {
    env: {
      ANTHROPIC_BASE_URL: "https://PLACEHOLDER/v1",
      ANTHROPIC_AUTH_TOKEN: "",
      ANTHROPIC_MODEL: "PLACEHOLDER",
      ANTHROPIC_DEFAULT_HAIKU_MODEL: "PLACEHOLDER",
      ANTHROPIC_DEFAULT_SONNET_MODEL: "PLACEHOLDER",
      ANTHROPIC_DEFAULT_OPUS_MODEL: "PLACEHOLDER",
    },
  },
  category: "third_party",
  icon: "joybuilder",
  iconColor: "#6366F1",
  apiFormat: "openai_chat",
},
```

- [ ] **Step 3: 在 Codex 预设中添加 Joybuilder**

在 `src/config/codexProviderPresets.ts` 的预设数组中添加 Joybuilder 条目，`settingsConfig` 改为 Codex 的格式（使用 `OPENAI_API_KEY`, `OPENAI_BASE_URL` 等字段）。

- [ ] **Step 4: 在 Gemini 预设中添加 Joybuilder**

在 `src/config/geminiProviderPresets.ts` 的预设数组中添加 Joybuilder 条目，`settingsConfig` 改为 Gemini CLI 的格式（使用 `.env` 字段）。

- [ ] **Step 5: 在 Universal 预设中添加 Joybuilder**

在 `src/config/universalProviderPresets.ts` 的 `universalProviderPresets` 数组中添加：

```typescript
{
  name: "Joybuilder",
  providerType: "joybuilder",
  defaultApps: {
    claude: true,
    codex: true,
    gemini: true,
  },
  defaultModels: {
    claude: {
      model: "PLACEHOLDER",
      haikuModel: "PLACEHOLDER",
      sonnetModel: "PLACEHOLDER",
      opusModel: "PLACEHOLDER",
    },
    codex: {
      model: "PLACEHOLDER",
      reasoningEffort: "high",
    },
    gemini: {
      model: "PLACEHOLDER",
    },
  },
  websiteUrl: "https://PLACEHOLDER",
  icon: "joybuilder",
  iconColor: "#6366F1",
  description: "Joybuilder API 供应商",
},
```

- [ ] **Step 6: Commit**

```bash
cd /Users/xixinyu.simona/projects/体系培训/hw3/cc-switch
git add src/config/claudeProviderPresets.ts src/config/codexProviderPresets.ts src/config/geminiProviderPresets.ts src/config/universalProviderPresets.ts src/assets/providers/joybuilder.svg
git commit -m "feat(providers): add Joybuilder provider presets"
```

---

### Task 10: 版本号升级 + 安装包构建

**Files:**
- Modify: `src-tauri/Cargo.toml` (version)
- Modify: `src-tauri/tauri.conf.json` (version)
- Modify: `package.json` (version)

**Interfaces:**
- Consumes: 所有前置 Task 完成
- Produces: macOS .dmg + Windows .msi 安装包

- [ ] **Step 1: 升级 Cargo.toml 版本号**

在 `src-tauri/Cargo.toml` 中，将 `version = "3.16.5"` 改为 `version = "3.17.0"`。

- [ ] **Step 2: 升级 tauri.conf.json 版本号**

在 `src-tauri/tauri.conf.json` 中，将 `"version": "3.16.5"` 改为 `"version": "3.17.0"`。

- [ ] **Step 3: 升级 package.json 版本号**

在 `package.json` 中，将 `"version": "3.16.5"` 改为 `"version": "3.17.0"`。

- [ ] **Step 4: 构建 macOS 安装包**

Run: `cd /Users/xixinyu.simona/projects/体系培训/hw3/cc-switch && pnpm tauri build --target aarch64-apple-darwin 2>&1 | tail -20`
Expected: 构建成功，产出 `src-tauri/target/release/bundle/dmg/cc-switch_3.17.0_aarch64.dmg`

- [ ] **Step 5: 验证安装包**

Run: `ls -la /Users/xixinyu.simona/projects/体系培训/hw3/cc-switch/src-tauri/target/release/bundle/dmg/`
Expected: 看到 `cc-switch_3.17.0_aarch64.dmg` 文件

- [ ] **Step 6: Commit 版本号变更**

```bash
cd /Users/xixinyu.simona/projects/体系培训/hw3/cc-switch
git add src-tauri/Cargo.toml src-tauri/tauri.conf.json package.json
git commit -m "chore: bump version to 3.17.0"
```

- [ ] **Step 7: Windows 安装包说明**

Windows 安装包需要在 Windows 机器上构建，或通过 GitHub Actions CI：

```yaml
# .github/workflows/build.yml 示例
jobs:
  build-windows:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: pnpm/action-setup@v4
      - uses: actions/setup-node@v4
        with: { node-version-file: '.node-version' }
      - run: pnpm install
      - run: pnpm tauri build
      - uses: actions/upload-artifact@v4
        with:
          name: cc-switch-windows
          path: src-tauri/target/release/bundle/msi/*.msi
```

产出文件：`src-tauri/target/release/bundle/msi/cc-switch_3.17.0_x64_en-US.msi`

---

## Self-Review

**1. Spec coverage check:**
- Finbox Skill 商场（爬取+缓存+安装）：Tasks 1, 4, 5, 6, 7 ✓
- 全局+项目级 Skill 管理（scope 字段+UI+命令）：Tasks 1, 2, 3, 8 ✓
- Joybuilder 供应商预设：Task 9 ✓
- 安装包构建：Task 10 ✓
- 数据库迁移 v12：Task 1 ✓

**2. Placeholder scan:**
- `parse_skills` 中的 CSS 选择器标注为需根据实际页面修正 — 这是预期的，finbox 页面结构未知
- Joybuilder 的 `PLACEHOLDER` URL/模型名 — 用户要求先留占位符

**3. Type consistency check:**
- Rust `InstalledSkill.scope: String` ↔ TypeScript `scope: "global" | "project"` — Rust 侧用 String 是因为 serde 默认，前端用联合类型做约束
- Rust `InstalledSkill.project_path: Option<String>` ↔ TypeScript `projectPath?: string` — 一致
- `install_skill_unified` 命令参数：Rust `(skill, current_app, scope, project_path, service, app_state)` ↔ TypeScript `invoke("install_skill_unified", { skill, currentApp, scope, projectPath })` — Tauri 会自动做 camelCase → snake_case 转换，需确认 `scope` 和 `projectPath` 的 Tauri 序列化名。可能需要在 Rust 命令参数中加 `#[serde(rename_all = "camelCase")]` 或用 `scope: Option<String>` 确保匹配。
