# Process Log

本文件记录 LazyCat 项目中重要/复杂操作的处理流程与踩坑经验。

**使用次数规则**：每条记录有 `使用次数` 字段，初始为 0。后续会话遇到相同问题并参考该记录时 +1，并追加引用日期。当使用次数 >= 3 时，固化到 `CLAUDE.md` 对应章节。

---

<!-- 新记录添加在此处，最新的在最上面 -->

## 2026-02-21: 添加 MDN JavaScript 中文手册（Puppeteer 抓取方案）

**场景**: 将 MDN JS 中文手册（https://developer.mozilla.org/zh-CN/docs/Web/JavaScript）添加为离线手册

**问题**:
1. MDN 是 React SSR + 客户端水合的 SPA，没有静态构建产物可直接使用
2. Yari（MDN 官方构建系统）整站产物数 GB，不现实
3. 页面路径无 `.html` 扩展名（如 `/zh-CN/docs/Web/JavaScript/Reference/Array`）
4. Windows 文件系统不支持 `*` 字符，5 个路径含星号的页面（如 `async_function*`）无法保存

**解决**:
1. 用 Puppeteer + 系统 Edge（`C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe`）抓取
2. 抓取脚本：`scripts/scrape-mdn-js.mjs`，使用 `createRequire` 导入 pnpm 本地 puppeteer
3. 无扩展名 URL 路径一律保存为 `<path>/index.html`（避免同名文件与目录冲突，ENOTDIR 错误）
4. 注册到 `manuals.rs`：`("mdn-js", "MDN JavaScript 手册", "/zh-CN/docs/Web/JavaScript/")`
5. `tauri.conf.json` 的 `bundle.resources` 已有 `**/*` 通配符，自动覆盖新手册目录

**关键点**:
1. `createRequire(import.meta.url)` 以脚本所在目录为基准解析相对路径，ESM 脚本中导入 CJS 模块的正确方式
2. SPA 路由的无扩展名路径必须保存为目录下 `index.html`，否则子路径写入时报 ENOTDIR
3. HTTP 服务器已处理无扩展名路径（`file_path.extension().is_none()` → 尝试加 `.html` 或 `index.html`），MDN 内链接直接可用
4. 含 `*` 字符的页面在 Windows 下无法保存，属于不可绕过的 OS 限制，影响 5 个页面，可忽略

**涉及文件**:
- `scripts/scrape-mdn-js.mjs`（新建，抓取脚本）
- `apps/desktop/src-tauri/src/tools/manuals.rs`（注册新手册）
- `resources/manuals/mdn-js/`（新建，872 个文件，72.3 MB）

**使用次数**: 0

## 2026-02-20: 六方案全量重构（类型集中化 + Composables + App.vue 拆分 + Rust 模块化 + 构建优化 + CSS 分层）

**场景**: 项目存在巨型 App.vue (1538行)、巨型 main.rs (1341行)、重复接口定义、Element Plus 全量导入、CSS 单文件、Monaco 主题不联动等6个架构问题

**问题**:
1. App.vue 1538行 60+ ref 21个 v-else-if，不可维护
2. Rust main.rs 59分支 match，1341行单文件
3. 9处接口重复定义
4. Element Plus 全量导入导致 index.js 999KB
5. styles.css 1447行单文件
6. Monaco 编辑器硬编码 `theme: "vs"`，不跟随 Dark/Light 切换

**解决**:
1. **类型集中化**: 新建 `src/types/` (tools.ts, hosts.ts, ports.ts, calc.ts, index.ts)，所有组件 import from `../types`
2. **Composables**: 新建 `src/composables/` (useToolInvoke.ts, useLocalStorage.ts, useFavorites.ts)
3. **App.vue 拆分**:
   - 新建 `tool-registry.ts`，用 `defineAsyncComponent` 映射工具ID到组件
   - 模板用 `<component :is="currentComponent" :key="activeTool" v-bind="currentComponentProps" />` 替代 21 个 v-else-if
   - 新建 12 个胖组件: RsaPanel, AesPanel, JsonXmlPanel, JsonYamlPanel, TextProcessPanel, EnvPanel, SplitMergePanel, ImagePanel, TimestampPanel, UuidPanel, CronPanel, SettingsPanel
   - 重写已有薄壳组件 (FormatterPanel, RegexPanel, HostsPanel, PortsPanel, CalcDraftPanel) 为胖组件，内化状态和 IPC 调用
   - App.vue: 1538行 -> 190行
4. **Rust 模块化**: 新建 `src-tauri/src/tools/` (18个文件: mod.rs, helpers.rs, encode.rs, convert.rs 等)
   - main.rs: 1341行 -> 311行
5. **构建优化**: 安装 `unplugin-vue-components` + `unplugin-auto-import`，配置 ElementPlusResolver 按需导入；配置 `manualChunks` 拆分 element-plus 和 monaco-editor
   - index.js: 999KB -> 20KB (element-plus 独立 415KB chunk)
6. **CSS 分层**: 拆分 styles.css 为 9 个文件 (tokens, reset, layout, sidebar, home, panels, element-overrides, responsive, theme-light)
   - MonacoPane: MutationObserver 监听 `data-theme` 切换 `vs`/`vs-dark`
   - 修复硬编码 `#dce3ef` -> `var(--lc-border)`

**关键点**:
1. Vue SFC 中不能对普通对象使用 v-model（SettingsPanel 的 isDarkMode），需要用 `:model-value` + `@update:model-value` 模式
2. `<component :is>` 的 v-bind 中可以传递 `onUpdate:xxx` 事件处理器实现双向绑定
3. Rust 模块化后编译器自动捕获所有错误，风险极低

**涉及文件**: App.vue, main.ts, vite.config.ts, styles.css, MonacoPane.vue, tool-registry.ts, src/types/*, src/composables/*, src/components/*Panel.vue (12新建+5重写), src/styles/* (10文件), src-tauri/src/tools/* (18文件), src-tauri/src/main.rs

**使用次数**: 0

## 2026-02-21: 代码片段页三栏拥挤治理与检索管理迭代（批量能力）
**场景**: 代码片段页在三栏结构下信息密度过高，检索与管理动作分散，缺乏批量处理能力，导致日常整理效率低。
**问题**:
1. 中栏仅有搜索和排序，缺少结果反馈与快速筛选。
2. 列表无法多选，无法批量收藏/移动/打标签/删除。
3. 前后端缺少批量操作接口，管理动作需要逐条执行。
4. 布局拥挤，列表与管理动作缺乏分层。

**解决**:
1. 前端中栏改造：
   - 增加“无标签/最近7天”快速筛选。
   - 增加结果计数与“清空筛选”。
   - 列表支持多选（checkbox）并保留单项点击编辑。
   - 增加底部批量操作条（收藏/取消收藏/移动到当前文件夹/添加标签/删除/清空选择）。
2. 前端状态逻辑增强：
   - 增加 `selectedIds` 多选状态与派生计数。
   - 增加 `quickFilter` 快速筛选状态。
   - 在 `loadSnippets` 中统一应用快速筛选，并同步清理不可见选中项。
3. 后端新增批量接口（事务）：
   - `batch_update`: 支持批量收藏、移动文件夹、添加/移除标签。
   - `batch_delete`: 支持批量删除片段。
4. IPC 通道映射新增：
   - `tool:snippets:batch-update` -> `batch_update`
   - `tool:snippets:batch-delete` -> `batch_delete`

**关键点**:
1. 批量更新必须校验 `ids` 非空且去重，且至少包含一个操作字段。
2. 批量写入使用数据库事务，避免部分成功导致状态不一致。
3. 列表筛选后要同步修正多选状态，避免“不可见项仍被批量操作”。
4. 批量移动采用“移动到当前选中文件夹”，无目标文件夹时提示用户先选择。

**涉及文件**:
- apps/desktop/src/components/SnippetPanel.vue
- apps/desktop/src/bridge/tauri.ts
- apps/desktop/src-tauri/src/tools/snippets.rs

**使用次数**: 0

## 2026-02-21: 代码片段专属工作区 V2 重构（右键入口 + 新模型 + FTS 检索）
**场景**:
需要将左上角 Lazycat 的交互改为左键回首页、右键进入专属代码片段工作区，并对代码片段页面做结构级重构。

**问题**:
1. 现有 snippets 页面挂在通用工具壳层中，无法形成专注工作区。
2. 旧 snippets 数据模型和查询逻辑偏旧，缺少“最近使用优先”和结构化初始化流程。
3. 首次进入需要执行“清空旧数据并重建”的强制流程。

**解决**:
1. App 壳层增加 `viewMode`，支持 `main` 与 `snippet-workspace` 双模式切换。
2. `SidebarNav` 品牌按钮增加右键事件，右键进入专属工作区，左键行为保持回首页。
3. `SnippetPanel.vue` 重写为标签优先三栏布局，接入 `tool:snippets:v2:*` 通道。
4. Rust `snippets.rs` 重写 V2 逻辑，新增：
   - `v2_init`（首次确认后清空并重建）
   - `v2_list` / `v2_search` / `v2_get` / `v2_create` / `v2_update` / `v2_delete`
   - `v2_mark_used` / `v2_tag_stats` / `v2_folder_list` / `v2_folder_create` / `v2_folder_update` / `v2_folder_delete`
5. `helpers.rs` 增加 schema migration 8，创建 snippets v2 表结构与索引；FTS5 建表降级为可选，避免不支持 FTS 的环境直接失败。

**关键点**:
1. 首次初始化采用强确认输入 `DELETE`，降低误触导致的数据清空风险。
2. 排序默认切到 `last_used_at + use_count`，并在打开/复制时调用 `mark_used`。
3. FTS 不可用时自动退化到 LIKE 查询，不阻断可用性。

**涉及文件**:
- apps/desktop/src/App.vue
- apps/desktop/src/components/SidebarNav.vue
- apps/desktop/src/components/SnippetPanel.vue
- apps/desktop/src/bridge/tauri.ts
- apps/desktop/src/styles/layout.css
- apps/desktop/src/styles/responsive.css
- apps/desktop/src-tauri/src/tools/snippets.rs
- apps/desktop/src-tauri/src/tools/helpers.rs

**使用次数**: 0

## 2026-02-21: Cron 工具易用性 V2（Spring 6 字段标准 + 5 字段兼容 + 时区预览）
**场景**:
Cron 工具原先仅提供基础 6 字段输入与简单预览，缺少规范化、模板、规则描述与时区切换，易用性不足。

**问题**:
1. 用户输入 5 字段表达式时无兼容策略，容易报错。
2. 缺少“表达式含义”反馈，用户难以快速确认规则。
3. 预览结果固定本地时间，跨环境排查不便。
4. 前端与后端接口粒度较粗，不利于扩展。

**解决**:
1. Rust `cron` 工具新增 action：`normalize`、`preview_v2`、`describe`。
2. 标准化策略固定为 Spring 6 字段；兼容 5 字段时自动补秒 `0` 并返回 warnings。
3. 预览支持时区参数（local / UTC / IANA 时区），并返回结构化时间项（display/iso/epochMs）。
4. Cron 面板重构为四段式：表达式规范化、字段构建、模板应用、预览表格。
5. 新增前端 `types/cron.ts`，统一响应类型定义。
6. 增加 Rust 单元测试覆盖 normalize、时区回退、常见描述规则。

**关键点**:
1. 保留旧 `tool:cron:preview/parse`，新增 v2 能力，降低回归风险。
2. 7 字段（含 year）明确拒绝，避免隐式不兼容。
3. 时区解析失败回退 local 并给 warning，不中断主流程。

**涉及文件**:
- apps/desktop/src/components/CronPanel.vue
- apps/desktop/src/bridge/tauri.ts
- apps/desktop/src/types/cron.ts
- apps/desktop/src/types/index.ts
- apps/desktop/src-tauri/src/tools/cron.rs
- apps/desktop/src-tauri/Cargo.toml

**使用次数**: 0
