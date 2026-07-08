# CC-Switch Finbox Skill 管理操作文档

## 概述

CC-Switch Finbox 版本在原有 Skill 管理基础上新增了三大功能：

1. **Finbox Skill 商场** — 从 finbox.jd.com/coverage 浏览和安装 Skill
2. **全局/项目级 Skill 管理** — 按作用域管理 Skill，支持一键安装、更新、卸载
3. **Joybuilder 供应商** — 新增 Joybuilder API 供应商预设

---

## 页面入口

打开 CC-Switch 应用，在左侧导航栏点击 **Skills** 进入 Skill 管理页面。

页面顶部有两个 Tab：

| Tab | 功能 |
|-----|------|
| **Installed** | 管理已安装的 Skill（全局 + 项目级） |
| **Finbox** | 浏览和安装 Finbox 商场中的 Skill |

---

## 一、Installed Tab — 已安装 Skill 管理

### 1.1 全局/项目级切换

在 Installed Tab 内，顶部有两个按钮：

- **全局 Skills** — 管理安装到 `~/.cc-switch/skills/` 的全局 Skill，对所有项目生效
- **项目 Skills** — 管理安装到指定项目 `.claude/skills/` 目录的项目级 Skill，仅当前项目可见

选择「项目 Skills」后，会出现项目路径输入框，输入目标项目的绝对路径（如 `/Users/xxx/projects/my-app`），即可查看该项目专属的 Skill 列表。

### 1.2 Skill 列表

每个 Skill 条目显示：

| 元素 | 说明 |
|------|------|
| 名称 | Skill 名称 |
| 来源 | GitHub 仓库（`owner/repo`）或 `本地` |
| 作用域标签 | `全局` 或 `项目级`，标识当前 Skill 的作用范围 |
| 更新徽章 | 黄色 `可更新` 标签，表示有新版本 |
| 描述 | Skill 功能简介 |
| 应用开关 | 一组 Toggle，为 Claude / Codex / Gemini / OpenCode / Hermes 等工具单独启用/禁用 |
| 操作按钮 | 悬停时出现：更新（有更新时）/ 卸载 |

### 1.3 一键操作

#### 安装 Skill

点击页面右上角操作区域的安装按钮，从以下来源安装：

- **从仓库发现** — 浏览 GitHub 仓库中的 Skill
- **从 ZIP 安装** — 选择本地 ZIP 文件
- **从应用导入** — 扫描已安装但未被 CC-Switch 管理的 Skill

安装时可选择作用域（全局/项目级），项目级需指定项目路径。

#### 更新 Skill

1. 点击顶部「检查更新」按钮
2. 有可用更新时，对应 Skill 会出现黄色「可更新」徽章
3. 悬停 Skill 条目，点击更新按钮
4. 如有多个更新，点击「全部更新」一键更新

#### 卸载 Skill

1. 悬停 Skill 条目，点击卸载按钮
2. 确认卸载对话框
3. 卸载后自动创建备份，可从备份恢复

### 1.4 备份与恢复

- **恢复备份** — 点击页面右上角操作区域，可查看卸载历史并恢复
- **删除备份** — 在备份列表中删除不再需要的备份

---

## 二、Finbox Tab — Finbox Skill 商场

### 2.1 浏览 Skill

切换到「Finbox」Tab，自动从 finbox.jd.com/coverage 加载 Skill 列表。每个 Skill 显示：

| 元素 | 说明 |
|------|------|
| 名称 | Skill 名称 |
| 分类 | Skill 类别标签 |
| 描述 | Skill 功能简介 |
| 外链按钮 | 在浏览器中打开 Skill 详情页 |
| 安装按钮 | 一键安装到 CC-Switch |

### 2.2 搜索 Skill

在 Finbox Tab 顶部的搜索框中输入关键词，实时过滤 Skill 列表（按名称和描述匹配）。

### 2.3 安装 Skill

1. 在列表中找到目标 Skill
2. 点击「安装」按钮
3. Skill 将安装为全局 Skill，并自动启用当前应用

### 2.4 刷新缓存

Finbox 数据缓存 1 小时。如需获取最新数据，点击搜索框旁的刷新按钮强制更新缓存。

### 2.5 错误处理

如页面加载失败，会显示错误提示和「重试」按钮。点击重试重新加载数据。

---

## 三、Joybuilder 供应商

### 3.1 添加 Joybuilder 供应商

1. 进入 **Providers** 页面
2. 点击「添加供应商」
3. 在预设列表中找到 **Joybuilder**
4. 填写 API Key 和其他配置
5. 保存

Joybuilder 作为标准 API 供应商预设，同时出现在：
- Claude Code 供应商预设
- Codex 供应商预设
- Gemini CLI 供应商预设
- 统一供应商预设（一键同步三个工具）

---

## 四、应用开关说明

每个 Skill 条目右侧有一组应用开关，对应以下工具：

| 开关 | 对应工具 | 配置文件路径 |
|------|----------|-------------|
| Claude | Claude Code | `~/.claude/settings.json` |
| Codex | OpenAI Codex CLI | `~/.codex/auth.json` |
| Gemini | Gemini CLI | `.env` |
| OpenCode | OpenCode | OpenCode 配置 |
| Hermes | Hermes Agent | Hermes 配置 |

开启开关后，CC-Switch 会自动将 Skill 同步到对应工具的配置目录。

---

## 五、常见问题

### Q: 全局 Skill 和项目级 Skill 有什么区别？

- **全局 Skill**：安装到 `~/.cc-switch/skills/`，通过符号链接/复制同步到各工具的全局目录，所有项目共享
- **项目级 Skill**：安装到项目的 `.claude/skills/` 目录，仅在该项目内生效，适合项目专属的定制化 Skill

### Q: Finbox 商场加载很慢或失败怎么办？

Finbox 数据从 finbox.jd.com/coverage 实时爬取，网络延迟或页面结构变更可能导致加载失败。点击刷新按钮重试，或检查网络连接。

### Q: Skill 安装后没有出现在列表中？

检查是否切换到了正确的作用域（全局/项目级）。项目级 Skill 需要输入正确的项目路径才能看到。

### Q: 如何更新所有 Skill？

在 Installed Tab 顶部点击「检查更新」，有可用更新时会出现「全部更新」按钮，一键更新所有。
