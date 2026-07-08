# CC-Switch 增强改造设计文档

**日期**: 2026-07-25
**版本**: 3.17.0
**方案**: 渐进式扩展（方案 A）

## 概述

对 cc-switch 进行三大改造：集成 Finbox Skill 商场、实现全局+项目级 Skill 管理、添加 Joybuilder 供应商预设。最终产出 macOS + Windows 安装包。

## 1. Finbox Skill 商场

### 1.1 架构

新增 `FinboxMarketplaceService`（Rust），负责爬取 finbox.jd.com/coverage 页面，解析 skill 列表和详情，复用现有 `SkillService::install_skill_unified()` 完成安装。

### 1.2 数据模型

新增 `finbox_skill_cache` 表，缓存爬取结果，TTL 默认 1 小时：

```sql
CREATE TABLE finbox_skill_cache (
    key TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    download_url TEXT,
    category TEXT,
    raw_html_hash TEXT,
    cached_at DATETIME NOT NULL,
    expires_at DATETIME NOT NULL
);
```

数据库迁移版本：v12。

### 1.3 后端新增

**新文件：**
- `src-tauri/src/services/finbox_marketplace.rs` — 爬取 + 解析 + 缓存逻辑
- `src-tauri/src/commands/finbox_marketplace.rs` — Tauri 命令层

**Tauri 命令：**
- `search_finbox_skills(query: String)` — 搜索 finbox skill
- `get_finbox_skill_detail(key: String)` — 获取单个 skill 详情
- `install_from_finbox(key: String)` — 一键安装
- `refresh_finbox_cache()` — 强制刷新缓存

**爬取策略：**
- 使用 `reqwest` 获取页面 HTML
- 使用 `scraper` crate（Rust HTML 解析库）提取 skill 信息
- 解析失败时返回友好错误，不影响现有功能
- 爬取在后台线程执行，UI 显示加载状态

### 1.4 前端新增

**新组件：**
- `src/components/skills/FinboxMarketplacePanel.tsx` — 商场 UI（浏览、搜索、安装按钮）

**改动：**
- `src/components/skills/UnifiedSkillsPanel.tsx` — 添加 "Finbox 商场" Tab

### 1.5 依赖

Rust 新增依赖：`scraper`（HTML 解析），已安装的 `reqwest` 用于 HTTP 请求。

## 2. 全局 + 项目级 Skill 管理

### 2.1 核心概念

- **全局 skill**：安装到 `~/.cc-switch/skills/`，对所有项目生效（现有行为，不变）
- **项目级 skill**：安装到当前项目的 `.claude/skills/`（或对应工具的项目级目录），仅当前项目可见

### 2.2 数据模型变更

扩展 `skills` 表（迁移 v12）：

```sql
ALTER TABLE skills ADD COLUMN scope TEXT NOT NULL DEFAULT 'global';
-- 'global' | 'project'
ALTER TABLE skills ADD COLUMN project_path TEXT;
-- 项目级 skill 记录所属项目路径，全局 skill 为 NULL
```

### 2.3 后端改动

**`src-tauri/src/services/skill.rs`：**
- `install_skill_unified` 新增 `scope` 和 `project_path` 参数
- 项目级安装时，目标目录切换到 `project_path/.claude/skills/`
- 查询 skill 时按 scope 过滤：展示所有全局 skill + 当前项目的项目级 skill

**`src-tauri/src/commands/skill.rs`：**
- 现有命令添加 `scope` 参数
- 新增 `get_project_skills(project_path: String)` — 获取项目级 skill 列表
- 新增 `set_current_project(project_path: String)` — 设置当前工作项目

**一键操作行为：**
- **安装**：选择作用域 → 下载 → 安装到对应目录 → 写 DB
- **更新**：检测内容哈希差异 → 提示更新 → 重新下载替换
- **卸载**：删除文件 + 清理 DB 记录 + 可选备份

### 2.4 前端改动

**`src/components/skills/UnifiedSkillsPanel.tsx`：**
- 添加作用域切换 Tab：「全局 Skills」/ 「项目 Skills」
- 项目 Tab 下拉选择当前项目路径
- 每个 skill 卡片增加「作用域」标签

**新组件：**
- `src/components/skills/SkillActionButtons.tsx` — 统一的安装/更新/卸载按钮组件，根据 scope 自动选择目标路径

### 2.5 同步策略

项目级 skill 的同步与全局 skill 一致：
- 优先 symlink，fallback 到 copy
- 只同步到当前项目对应的工具配置目录

## 3. Joybuilder 供应商

### 3.1 核心思路

Joybuilder 是标准 API 供应商，只需在各 app 的预设文件中添加预设条目，无需改动 Rust 层。

### 3.2 前端预设文件改动

以下文件各添加一条 Joybuilder 预设：

- `src/config/claudeProviderPresets.ts`
- `src/config/codexProviderPresets.ts`
- `src/config/geminiProviderPresets.ts`
- `src/config/universalProviderPresets.ts`

预设字段（占位符，后续填入实际值）：

```typescript
{
  id: 'joybuilder',
  name: 'Joybuilder',
  category: 'official',
  icon: 'joybuilder',
  iconColor: '#PLACEHOLDER',
  websiteUrl: 'https://PLACEHOLDER',
  settingsConfig: {
    apiKey: '',
    apiUrl: 'https://PLACEHOLDER/v1',
    model: 'PLACEHOLDER',
  }
}
```

### 3.3 图标

新增 `src/assets/providers/joybuilder.svg` — 供应商 logo（占位符，后续替换）。

### 3.4 无需改动 Rust 层

预设是前端渲染的快捷添加模板，用户点击后通过现有 `add_provider` 命令写入数据库。

## 4. 安装包构建

### 4.1 版本号

升级到 `3.17.0`，修改以下文件：
- `src-tauri/tauri.conf.json`
- `package.json`

### 4.2 macOS 安装包

```bash
# Apple Silicon
pnpm tauri build --target aarch64-apple-darwin

# Intel Mac
pnpm tauri build --target x86_64-apple-darwin

# 通用二进制
pnpm tauri build --target universal-apple-darwin
```

产出：`.dmg` 安装包 + `.app` 应用。

### 4.3 Windows 安装包

在 Windows 机器或 CI 上构建：

```bash
pnpm tauri build
```

产出：`.msi` 安装包 + `.exe` 安装程序。

### 4.4 CI/CD（推荐）

利用 GitHub Actions 自动构建双平台安装包：
- `macos-latest` runner 构建 macOS .dmg
- `windows-latest` runner 构建 Windows .msi/.exe
- 构建产物上传到 GitHub Releases

## 5. 数据库迁移

所有数据库变更统一在迁移 v12 中处理：

1. 创建 `finbox_skill_cache` 表
2. `skills` 表新增 `scope` 和 `project_path` 列

迁移文件：`src-tauri/src/database/migration.rs`，在 `migrate_v11_to_v12` 函数中实现。

## 6. 改动文件清单

### 新增文件

| 文件 | 说明 |
|------|------|
| `src-tauri/src/services/finbox_marketplace.rs` | Finbox 商场爬取服务 |
| `src-tauri/src/commands/finbox_marketplace.rs` | Finbox 商场 Tauri 命令 |
| `src/components/skills/FinboxMarketplacePanel.tsx` | Finbox 商场 UI |
| `src/components/skills/SkillActionButtons.tsx` | 统一 skill 操作按钮 |
| `src/assets/providers/joybuilder.svg` | Joybuilder logo |

### 修改文件

| 文件 | 说明 |
|------|------|
| `src-tauri/src/database/migration.rs` | 新增 v12 迁移 |
| `src-tauri/src/database/schema.rs` | 新增 finbox cache 表结构和 skill scope 字段 |
| `src-tauri/src/services/skill.rs` | 支持 scope/project_path 参数 |
| `src-tauri/src/commands/skill.rs` | 命令添加 scope 参数 |
| `src-tauri/src/lib.rs` | 注册新命令和初始化 FinboxMarketplaceService |
| `src/components/skills/UnifiedSkillsPanel.tsx` | 添加 Finbox Tab 和作用域切换 |
| `src/config/claudeProviderPresets.ts` | 添加 Joybuilder 预设 |
| `src/config/codexProviderPresets.ts` | 添加 Joybuilder 预设 |
| `src/config/geminiProviderPresets.ts` | 添加 Joybuilder 预设 |
| `src/config/universalProviderPresets.ts` | 添加 Joybuilder 预设 |
| `src-tauri/tauri.conf.json` | 版本号升级 |
| `package.json` | 版本号升级 |
| `src-tauri/Cargo.toml` | 新增 scraper 依赖 |

## 7. 风险与缓解

| 风险 | 缓解措施 |
|------|----------|
| finbox 页面结构变化导致爬取失败 | 解析失败返回友好错误，不影响现有功能；TTL 缓存减少请求频率 |
| 项目级 skill 路径冲突 | 安装前检查路径是否已存在，提供覆盖提示 |
| 数据库迁移兼容性 | 新增列均有默认值，不影响现有数据 |
| scraper crate 增大二进制体积 | scraper 是编译时依赖，对运行时体积影响极小 |
