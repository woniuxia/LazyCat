# CLAUDE.md

本文件为 Claude 或其他编码代理提供项目上下文和协作规范。

## 项目信息

- 名称: Lazycat (懒猫)
- 类型: 离线桌面开发者工具箱
- 主要平台: Windows
- 开发环境: Windows（命令行使用 PowerShell，不支持 `&&` 链接命令，使用 `;` 分隔）
- 运行时: Tauri 2 + Vue 3 + TypeScript

## 仓库结构

- `apps/desktop`: Tauri 桌面应用（Rust 命令 + Vue 渲染层）
- `packages/core`: 编解码、文本工具、转换、正则、Cron、生成器
- `packages/crypto`: RSA/AES/DES 加密封装
- `packages/formatters`: JSON/XML/HTML/Java/SQL 格式化
- `packages/network`: 网络连通性/运行时/端口检查
- `packages/file-tools`: 文件拆分/合并工具
- `packages/image-tools`: 图片转换/缩放/裁剪/压缩
- `packages/db`: SQLite 持久化
- `packages/ipc-contracts`: 请求/响应契约定义
- `resources/manuals`: 离线手册（Vue 3、Element Plus）
- `resources/regex-library`: 内置正则模板
- `scripts`: 构建脚本（`build-tauri-win.ps1`）

## 本地命令

- 安装依赖: `pnpm install`
- 开发模式: `pnpm dev`
- 类型检查: `pnpm typecheck`
- 构建: `pnpm build`
- 构建（Windows 预检）: `pnpm build:win:precheck`
- 单元测试: `pnpm test`
- E2E 测试: `pnpm test:e2e`
- Windows 打包（NSIS）: `pnpm build:win`
- Windows 打包（便携版）: `pnpm build:portable`

## 代理协作规则

- 不要自动启动应用/开发服务器。
- 仅在用户明确要求时才运行 `pnpm dev`（或任何启动桌面 UI 的命令）。
- 完成复杂任务（3+ 文件、非简单调试、架构变更）后，将流程总结写入 `process.md`。
- 开始复杂任务前，先检查 `process.md` 是否有相关经验。
- 当 `process.md` 中某条经验使用次数 >= 3 时，固化到 `CLAUDE.md`。
- 较大变动（跨多文件、新增功能、架构调整等）在确认更改有效后，应及时提交一次代码，避免大量改动堆积导致回滚困难或提交信息模糊。

## 编码与乱码问题（重要）

- 已知问题：中文乱码曾导致 `apps/desktop/src/App.vue` 模板/脚本损坏：
  - 引号属性缺失闭合 `"`
  - 按钮闭合标签损坏（如 `?/el-button>`）
  - `<script>` 中字符串字面量未终止
- 典型报错症状：
  - Vite/Vue 解析错误：
    - `Attribute name cannot contain ...`
    - `Unquoted attribute value cannot contain ...`
    - `Unterminated string constant`
    - `Error parsing JavaScript expression`
- 修改 UI 文本时的必要措施：
  - 保持有效 UTF-8，避免批量替换导致标点/引号变异
  - 优先小范围、精确编辑
  - 发现乱码时，先修复结构正确性（引号/标签），再修复显示文本
- 修改 Vue 文件中文本后的必要验证：
  1. `pnpm --filter @lazycat/desktop typecheck`
  2. `pnpm --filter @lazycat/desktop build:web`
- 格式化功能补充说明：
  - 渲染层中 Prettier 必须使用 standalone + 显式插件（`prettier/standalone` + 解析器插件），否则运行时解析器解析会失败

## 架构说明

- 前后端调用链路：
  - Vue 调用 `apps/desktop/src/bridge/tauri.ts` 中的 `invokeToolByChannel`
  - 通道字符串（如 `tool:encode:base64-encode`）通过 `CHANNEL_MAP` 映射为 `{domain, action}`
  - Tauri 命令 `tool_execute` 在 Rust 端通过 `main.rs` 中的 `match (domain, action)` 分发
- 前端路由：未使用 vue-router；`App.vue` 通过 `v-else-if` 链式判断 `activeTool` ref 来切换面板
- 已提取的子组件位于 `apps/desktop/src/components/`：`HomePanel`、`CalcDraftPanel`、`FormatterPanel`、`RegexPanel`、`HostsPanel`、`PortsPanel`、`NetworkPanel`、`MonacoPane`、`ManualPanel`、`EncodePanel`、`CsvJsonPanel`、`SidebarNav`
- 格式化架构：XML/HTML/Java/SQL 格式化在 Rust 端为**直通模式**；实际格式化由渲染层的 `@lazycat/formatters`（Prettier standalone）完成
- Cron 预览（`cron.preview`）当前为**桩实现** -- 返回占位字符串，非真实的下次触发时间
- Hosts 激活需要**管理员权限**写入 `C:\Windows\System32\drivers\etc\hosts`；覆写前自动备份原文件
- 运行时数据：
  - 默认数据目录：`%USERPROFILE%\\.lazycat`（可通过设置面板自定义）
  - 指针文件 `%USERPROFILE%\\.lazycat\\config.json` 记录自定义数据目录路径，该文件位置固定不变
  - Hosts 配置、用户设置均存储在 SQLite
  - Hosts 备份目录由 Rust 端管理
- 菜单显隐：`useMenuVisibility` composable 管理侧边栏工具的显示/隐藏
  - 数据模型：deny-list（被隐藏的工具 ID 数组），SQLite key `menu_visibility`，新增工具默认可见
  - 数据流管线：`sidebarItems`（静态定义）→ `sortedSidebarItems`（热度排序）→ `visibleSidebarItems`（过滤隐藏+自动提升）→ `SidebarNav :items`
  - 分组只剩 1 个可见子项时自动提升为一级菜单，0 个则整组消失
  - `allTools`/`allToolMap` 始终基于未过滤的 `sidebarItems`，隐藏工具仍可通过标签页、收藏、首页访问
  - 设置入口：`SettingsPanel` → `MenuVisibilityDialog`（el-tree 树形复选框）
- 状态持久化：收藏夹、工具点击历史、计算草稿历史、外观主题、快捷键、菜单显隐等全部存储在 SQLite（`user_settings` 表）；旧版 localStorage 数据在首次启动时自动迁移

## 重要运行时路径

- 指针配置: `%USERPROFILE%\\.lazycat\\config.json`（固定位置，记录自定义数据目录）
- 数据库文件: `<数据目录>\\lazycat.sqlite`（默认 `%USERPROFILE%\\.lazycat\\lazycat.sqlite`）
- Hosts 备份: `<数据目录>\\hosts-backups`

## 数据管理

### 可配置数据目录

- `helpers.rs` 提供三层路径函数：
  - `get_base_dir()` -- 固定返回 `~/.lazycat`，创建目录
  - `get_config_path()` -- 固定返回 `~/.lazycat/config.json`
  - `get_data_dir()` -- 读 `config.json` 中的 `data_dir` 字段，若存在且路径可达则使用，否则回退 `get_base_dir()`
- `config.json` 格式：`{"data_dir": "D:\\MyData\\lazycat"}`，该文件永远在 `~/.lazycat/` 下，不随数据目录迁移
- 容错：自定义路径不可达（外置硬盘拔出等）时静默回退默认目录，不崩溃
- 迁移策略：复制 `lazycat.sqlite` + `hosts-backups/` 到新目录，旧目录数据保留不删除
- 安全检查：目标目录已存在 `lazycat.sqlite` 时拒绝迁移，避免覆盖

### 导出/导入

- 导出/导入使用 Tauri 原生文件对话框（`@tauri-apps/plugin-dialog` 的 `save()`/`open()`），不使用浏览器 blob 下载或 `<input type="file">`
- Rust 端 `export_to_file` 直接调用 `settings_export()` 获取数据后写入指定路径
- Rust 端 `import_from_file` 读取文件后复用 `settings_import()` 逻辑
- 导出格式 JSON，包含 `version`、`exportedAt`、`settings`（全部用户设置）、`hosts_profiles`
- 导入支持 `merge`（合并）和 `overwrite`（覆盖）两种模式
- 所需权限：`capabilities/default.json` 中需要 `dialog:allow-save` 和 `dialog:allow-open`

### settings 域 IPC 通道一览

| 通道 | Rust action | 说明 |
|------|-------------|------|
| `tool:settings:get` | `get` | 读取单个设置项 |
| `tool:settings:set` | `set` | 写入单个设置项 |
| `tool:settings:get-all` | `get_all` | 读取全部设置 |
| `tool:settings:export` | `export` | 导出数据（返回 JSON 值） |
| `tool:settings:import` | `import` | 导入数据（传入 JSON 字符串） |
| `tool:settings:export-to-file` | `export_to_file` | 导出数据到指定文件路径 |
| `tool:settings:import-from-file` | `import_from_file` | 从指定文件路径导入数据 |
| `tool:settings:get-data-dir` | `get_data_dir` | 获取当前数据目录信息 |
| `tool:settings:set-data-dir` | `set_data_dir` | 设置自定义数据目录（含迁移） |
| `tool:settings:reset-data-dir` | `reset_data_dir` | 恢复默认数据目录 |

## 当前已知限制

- `pnpm build` 需要 Rust 工具链（`cargo`、`rustc`）及平台依赖。
- Windows 上 vendored OpenSSL 需要 `perl`（如 Strawberry Perl）。
- 所有 Rust 工具逻辑集中在 `apps/desktop/src-tauri/src/main.rs`（约 1340 行），尚未拆分模块。
- `packages/core`、`packages/crypto`、`packages/db`、`packages/file-tools`、`packages/image-tools`、`packages/network`、`packages/ipc-contracts` 当前为桩或薄封装 -- 实际逻辑在 Rust 端。仅 `packages/formatters` 被渲染层实际使用（Prettier standalone）。
- Cron 预览为桩实现（返回占位字符串），真实的下次触发时间计算尚未实现。
- Rust 端的 XML/HTML/Java/SQL 格式化为直通模式；格式化质量取决于 `@lazycat/formatters`（Prettier）。
- 离线手册已集成 Vue 3 和 Element Plus，可替换或新增更多完整静态文档。
- Hosts 激活需要以管理员身份运行应用。

## 离线手册架构

### 工作原理

- Rust 在 `setup` 阶段扫描 `resources/manuals/` 下的子目录，为每个手册启动独立的本地 HTTP 文件服务器（`TcpListener::bind("127.0.0.1:0")` 自动分配端口）
- 端口存储在全局 `MANUAL_SERVERS: OnceLock<HashMap<String, u16>>`
- `manuals:list` IPC 从全局 map 读取端口，返回 `http://127.0.0.1:{port}/guide/introduction.html` 格式的 URL
- 前端 `ManualPanel.vue` 用 `<iframe :src="url">` 内嵌展示，文档自带的搜索和导航在 iframe 内直接可用

### 为什么用本地 HTTP 服务器（而非自定义 URI Scheme）

- Tauri 2 的 `register_uri_scheme_protocol` 在 iframe 中加载 HTML 有已知 bug（[tauri#12767](https://github.com/tauri-apps/tauri/issues/12767)），CSS/JS 资源无法正确加载
- 本地 HTTP 服务器方案最稳定，完全兼容 VitePress 文档的绝对路径资源引用（`/assets/...`）

### 为什么每个手册独立端口

- VitePress 构建产物中所有资源路径是绝对路径（`/assets/style.xxx.css`），从 server 根目录解析
- 如果多个手册共享一个端口（如 `http://127.0.0.1:{port}/vue3/index.html`），`/assets/...` 会跳过 `vue3/` 前缀导致 404
- 每个手册独立端口，其目录即为 HTTP 根路径，绝对路径天然正确

### 开发模式 vs 生产模式路径解析

- 打包后：`app.path().resource_dir()` + `manuals/` （由 `tauri.conf.json` 的 `bundle.resources` 配置打包）
- 开发模式：`resource_dir()` 指向 `target/debug/`，文件不存在，fallback 到 `CARGO_MANIFEST_DIR/../../../resources/manuals`（项目根目录）
- `tauri.conf.json` 中 `bundle.resources` 路径相对于 `src-tauri/`，当前值 `"../../../resources/manuals/**/*"`

### 添加新手册（步骤）

1. **获取中文文档源码**（以 VitePress 文档为例）：
   ```bash
   git clone --depth=1 https://github.com/<org>/<docs-repo> /tmp/docs
   cd /tmp/docs && pnpm install && pnpm build
   ```
   - Vue 3 中文：`vuejs-translations/docs-zh-cn`，产物在 `.vitepress/dist/`
   - 注意：务必使用中文翻译仓库，而非英文原版
   - **Element Plus 例外**：中文翻译由 Crowdin 管理，源码构建需要 Crowdin API token 才能生成中文版。替代方案是用 Puppeteer 抓取线上 SPA 渲染后的 HTML（见下方）

2. **复制构建产物**到 `resources/manuals/<id>/`：
   ```bash
   cp -r .vitepress/dist resources/manuals/<id>
   ```

3. **注册手册**（`main.rs` 的 `manuals:list` 分支）：
   ```rust
   let known = [
       ("vue3",         "Vue 3 开发手册",       "/guide/introduction.html"),
       ("element-plus", "Element Plus 组件库",  "/zh-CN/component/overview"),
       ("<id>",         "<名称>",               "/<首页路径>"),  // 新增
   ];
   ```

4. **清理临时目录**，验证 `pnpm dev` 能正确加载

### Puppeteer SPA 抓取方案（Element Plus 适用）

当文档无法从源码构建中文版时（如 Element Plus 需要 Crowdin API token），用 Puppeteer 抓取线上 SPA：

1. 从 sitemap 获取所有 `/zh-CN/` 页面 URL
2. 用 Puppeteer (headless Edge/Chrome) 逐页打开，等待 `networkidle0` + `#app .VPContent` 渲染
3. `page.content()` 获取完整 DOM HTML，将绝对 URL 替换为相对路径后保存
4. 收集页面中引用的 CSS/JS/字体/图片 URL，用 `fetch` 批量下载

注意事项：
- SPA 路由的 URL 没有 `.html` 扩展名（如 `/zh-CN/guide/design`），保存为同名文件
- HTTP 服务器需要处理无扩展名文件：先尝试加 `.html`，再尝试作为目录找 `index.html`，并通过 body 内容检测 MIME 类型
- Puppeteer 可用系统已装的 Edge：`executablePath: "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe"`
- 100 个页面 + 200 个静态资源，约需 5-10 分钟

### 常见坑点

- **不要用 `website-scraper` / `wget --mirror` 抓取 VitePress 站点** — VitePress 是 SPA，抓取到的是空壳 HTML，JS 渲染的内容不会被保存
- **优先从源码构建** — `git clone` + `pnpm build` 得到的才是完整的 SSR 静态产物；Puppeteer 抓取是 fallback 方案
- **注意 `bundle.resources` 路径** — 相对于 `apps/desktop/src-tauri/`，不是项目根目录
- **Element Plus 源码构建中文版需要 Crowdin API token** — 没有 token 只能构建英文版

## 流程记录 (process.md)

### 用途

`process.md` 是项目级的流程记录文件，用于记录每次重要/复杂操作的处理流程、踩坑经验和决策依据。

### 何时写入 process.md

- 跨多文件的复杂修改（涉及 3+ 文件）
- 调试过程中发现的非显而易见的问题及解决方案
- 架构决策及其理由
- 新功能集成的完整步骤（如添加新手册、新工具面板）
- 构建/打包/部署过程中遇到的环境问题

### 记录格式

```markdown
## YYYY-MM-DD: [简短标题]

**场景**: 做了什么
**问题**: 遇到了什么
**解决**: 怎么解决的
**关键点**: 需要记住的核心经验（1-3 条）
**涉及文件**: 改动的关键文件列表
**使用次数**: 0
```

### 使用次数规则

- 每次新建记录时 `使用次数` 初始为 `0`
- 后续会话中遇到相同/相似问题并参考了该条记录时，`使用次数 + 1`
- 更新使用次数时同时追加引用日期：`**使用次数**: 3 (2026-01-15, 2026-02-01, 2026-02-19)`

### 固化规则

当 process.md 中某条经验的 **使用次数 >= 3** 时：
1. 将该经验提炼为通用规则
2. 写入 CLAUDE.md 对应章节（如"架构说明"、"编码与乱码问题"等）
3. 在 process.md 原条目中标注 `[已固化到 CLAUDE.md - YYYY-MM-DD]`，保留记录但不再计数

### 维护原则

- 每条记录保持简洁，重点是"坑"和"解法"，不记流水账
- 不记录简单操作（单文件、< 20 行的修改）
- 定期清理已固化条目（保留最近 3 个月）

## 提交规范

- 使用约定式提交格式：
  - `feat: ...`
  - `fix: ...`
  - `docs: ...`
  - `chore: ...`
  - `test: ...`
- 每次提交按领域聚焦（ui/core/build/test）。

## 提交前检查

推送前执行以下检查：

1. `pnpm typecheck`
2. `pnpm --filter @lazycat/desktop build:web`（渲染层）
3. `pnpm test`
4. `pnpm test:e2e`

如需打包：

5. `pnpm build:win`
