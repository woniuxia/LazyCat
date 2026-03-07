# Process Log

本文件记录 LazyCat 项目中重要/复杂操作的处理流程与踩坑经验。

**使用次数规则**：每条记录有 `使用次数` 字段，初始为 0。后续会话遇到相同问题并参考该记录时 +1，并追加引用日期。当使用次数 >= 3 时，固化到 `CLAUDE.md` 对应章节。

---

<!-- 新记录添加在此处，最新的在最上面 -->

## 2026-03-07: 密码库移除软锁并改为失焦仅隐藏敏感信息

**场景**: 将密码库从“敏感信息隐藏 → 软锁 → 硬锁”收敛为“敏感信息隐藏 → 硬锁”，同时保留失焦时的安全保护体验。

**问题**:
1. 软锁引入了额外状态、IPC 和元数据列表链路，前后端实现复杂度偏高。
2. 失焦锁定会打断当前上下文，用户更需要的是立即恢复掩码显示，而不是直接改变会话状态。
3. `show-password` 输入框的显隐状态由组件内部维护，仅清理外层状态无法在失焦时自动恢复掩码。

**解决**:
1. 后端移除 `soft_lock`、`list_metadata`、`vault_soft_locked` 和 `SoftLocked` 状态，统一只保留 unlocked / locked 两态。
2. 前端空闲计时器改为“到期隐藏敏感信息 + 到期直接硬锁”，失焦事件只执行敏感信息隐藏，不再触发锁定。
3. 为 `VaultPanel`、`VaultEntryDialog`、`VaultLockScreen` 的密码输入引入 `maskVersion` 重挂载机制，失焦时可恢复掩码显示且不清空已输入内容。

**关键点**:
1. “隐藏敏感信息”与“锁定会话”需要明确分层：前者只影响 UI 展示，后者才影响后端解锁态。
2. 失焦隐藏要覆盖列表明文、复制反馈和 `show-password` 组件内部显隐状态，否则体验会出现保护不一致。
3. 锁定预设继续复用 `vault_lock_profile`，仅保留隐藏时长和硬锁时长，避免再引入新的配置分支。

**涉及文件**:
- `apps/desktop/src/components/VaultPanel.vue`
- `apps/desktop/src/components/VaultEntryDialog.vue`
- `apps/desktop/src/components/VaultLockScreen.vue`
- `apps/desktop/src/components/SettingsPanel.vue`
- `apps/desktop/src/composables/useSettings.ts`
- `apps/desktop/src/utils/vaultLock.ts`
- `apps/desktop/src-tauri/src/tools/vault.rs`
- `apps/desktop/src/bridge/tauri.ts`

**使用次数**: 0

## 2026-03-07: 密码库分级锁定优先复用现有会话与设置通道
**场景**: 为密码管理增加“敏感信息隐藏 → 软锁 → 硬锁”的平衡方案，同时保留主密码为唯一解锁凭据。
**问题**:
1. 原实现只有固定 5 分钟硬锁，前端只有布尔锁定态，缺少软锁与预设配置。
2. `vault` 已经具备通用设置持久化、状态查询和会话内存密钥，不适合再造一套存储模型。
3. 软锁需要保留列表上下文，但现有 `list` 接口会解密并返回账号/摘要，不能直接复用到软锁态。
**解决**:
1. 设置层继续走 `user_settings`，新增 `vault_lock_profile`，前端通过 `useSettings` 提供统一读取与策略换算。
2. 后端会话保持“内存密钥 + 状态枚举”，新增 `soft_lock` / `touch` / `list_metadata`，并让 `status` 返回 `lockState`。
3. 前端在 `VaultPanel` 本地做空闲计时与失焦软锁，后端负责硬锁兜底；软锁时改走 `list_metadata` 仅返回非敏感字段。
4. 关闭到托盘时在 `main.rs` 直接调用 `tools::vault::force_lock()`，避免窗口隐藏后仍保留解锁态。
**关键点**:
1. 分级锁定里，“软锁保留上下文”与“硬锁清空会话密钥”要明确分工：前端保留视图，后端控制密钥生命周期。
2. 锁定预设尽量收敛为 `strict / balanced / convenient`，不要把秒数配置直接暴露给用户。
3. 若前端测试在沙箱内出现 `spawn EPERM`，按规范提权重跑即可，不要因为单次 EPERM 放弃验证。
**涉及文件**:
- `apps/desktop/src/components/VaultPanel.vue`
- `apps/desktop/src/components/SettingsPanel.vue`
- `apps/desktop/src/composables/useSettings.ts`
- `apps/desktop/src/utils/vaultLock.ts`
- `apps/desktop/src/bridge/tauri.ts`
- `apps/desktop/src-tauri/src/tools/vault.rs`
- `apps/desktop/src-tauri/src/main.rs`

**使用次数**: 0

## 2026-03-07: 命名快捷键二次触发隐藏失败根因为缺少 `core:window:allow-hide`
**场景**: `snippets`、`launcher`、`vault` 通过命名快捷键呼出后，再次按下同一快捷键没有隐藏主窗口。
**问题**:
1. 前端热键监听已经命中隐藏分支，但 `appWindow.hide()` 在 Tauri 权限层被拒绝。
2. 日志报错明确提示缺少 `core:window:allow-hide`，导致看起来像“逻辑无效”，实际是权限不足。
**解决**:
1. 在 `apps/desktop/src-tauri/capabilities/default.json` 为主窗口补充 `core:window:allow-hide`。
2. 保留命名快捷键使用结构化 payload 的隐藏判定逻辑，清理仅用于排查的调试日志和设置项。
**关键点**:
1. Tauri 2 的窗口 API 即使前端调用命中分支，也可能因 capability 缺失而在运行时失败。
2. 这类问题应先看权限报错，再决定是否继续扩大逻辑排查范围。
**涉及文件**:
- `apps/desktop/src-tauri/capabilities/default.json`
- `apps/desktop/src/App.vue`
- `apps/desktop/src-tauri/src/main.rs`

**使用次数**: 0

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

## 2026-02-21: 文本处理工具重做（清洗 + 提取 + 双栏统计）
**场景**:
将“文本处理”从仅按行去重/排序升级为可配置的文本清洗与提取管线，并增强结果展示。

**问题**:
1. 旧能力过窄，仅 2 个后端 action，难以覆盖日志/配置清洗场景。
2. 前端缺少操作编排、统计反馈、差异预览，用户难以判断处理效果。
3. 文本面板存在文案乱码风险，影响可读性和可维护性。

**解决**:
1. Rust `text` 域替换为统一 `process` action，支持 trim/remove-empty/dedupe/sort/filter/replace/prefix/suffix/extract-column。
2. 新增 `presets` action，返回日志清洗、配置键提取、错误日志提取等预设。
3. 前端 `TextProcessPanel` 重写为双栏对照，新增操作区、统计卡片、变更样本表、自动执行与预设套用。
4. 通道映射改为 `tool:text:process` + `tool:text:presets`，移除旧 `unique-lines/sort-lines`。
5. 新增 `types/text.ts` 并统一导出，明确请求/响应与操作类型。

**关键点**:
1. 采用“前后端协同”：Rust 提供稳定算子，前端负责编排与展示。
2. 变更样本做数量上限控制（`previewLimit`），避免大文本导致前端卡顿。
3. 直接替换旧通道前先全仓检索调用点，确认仅单点使用后再切换。

**涉及文件**:
- apps/desktop/src/components/TextProcessPanel.vue
- apps/desktop/src/bridge/tauri.ts
- apps/desktop/src/types/text.ts
- apps/desktop/src/types/index.ts
- apps/desktop/src-tauri/src/tools/text.rs
- apps/desktop/src/App.vue

**使用次数**: 0

## 2026-02-21: Backend Unit Test Expansion for Critical Tool Domains
**场景**: 为 Rust 后端 tools 域补充单元测试，重点覆盖编码转换、加解密、模板渲染与高风险输入分支。
**问题**:
1. 现有测试主要集中在 cron/text，核心安全与转换能力覆盖不足。
2. 多个 action 缺少错误分支验证，回归时容易出现静默偏差。
3. 系统能力（network/dns/file/image/env/port 等）缺少稳定 smoke 测试。
**解决**:
1. 为 `encode/crypto/convert/jwt/schema/mybatis/nginx` 增加核心单测与错误分支。
2. 为 `network/dns/file/image/env/port/format/gen/time/regex/manuals/mod` 增加稳定测试。
3. 调整易波动断言（如 OpenSSL DES 可用性、resize 等比行为、url_decode 容错行为）以避免假阳性失败。
4. 统一执行 `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml`，最终 76/76 通过。
**关键点**:
1. 避免新增测试依赖导致环境下载失败；优先使用标准库与现有依赖。
2. 对系统相关能力尽量使用本地回环与临时文件，避免依赖外网和真实系统状态。
3. 对第三方库行为差异（OpenSSL provider、urlencoding 容错）采用兼容断言。
**涉及文件**:
- apps/desktop/src-tauri/Cargo.toml
- apps/desktop/src-tauri/src/tools/encode.rs
- apps/desktop/src-tauri/src/tools/crypto.rs
- apps/desktop/src-tauri/src/tools/convert.rs
- apps/desktop/src-tauri/src/tools/jwt.rs
- apps/desktop/src-tauri/src/tools/schema.rs
- apps/desktop/src-tauri/src/tools/mybatis.rs
- apps/desktop/src-tauri/src/tools/nginx.rs
- apps/desktop/src-tauri/src/tools/network.rs
- apps/desktop/src-tauri/src/tools/dns.rs
- apps/desktop/src-tauri/src/tools/file.rs
- apps/desktop/src-tauri/src/tools/image.rs
- apps/desktop/src-tauri/src/tools/env.rs
- apps/desktop/src-tauri/src/tools/port.rs
- apps/desktop/src-tauri/src/tools/regex.rs
- apps/desktop/src-tauri/src/tools/manuals.rs
- apps/desktop/src-tauri/src/tools/settings.rs
- apps/desktop/src-tauri/src/tools/snippets.rs
- apps/desktop/src-tauri/src/tools/hotkey.rs
- apps/desktop/src-tauri/src/tools/format.rs
- apps/desktop/src-tauri/src/tools/gen.rs
- apps/desktop/src-tauri/src/tools/time.rs
- apps/desktop/src-tauri/src/tools/mod.rs

## 2026-02-27: release 脚本 Git link.exe 遮蔽 MSVC 链接器

**场景**: 执行 `release-all-win.ps1` 打包脚本，Rust 编译链接阶段失败
**问题**: `C:\Program Files\Git\usr\bin\link.exe`（GNU coreutils link）在 PATH 中优先于 MSVC 的 `link.exe`，导致 `linking with link.exe failed: exit code: 1`。即使 VsDevCmd.bat 已执行，Git 的 usr/bin 仍在 PATH 前面
**解决**: 在 `Invoke-InVsDevEnv` 函数中，调用 cmd /c 前在 PowerShell 层面过滤 PATH：`$env:Path = ($env:Path -split ';' | Where-Object { $_ -notmatch 'Git\\usr\\bin' }) -join ';'`，并在 finally 块中恢复原始 PATH
**关键点**:
1. cmd.exe 内的 `set "PATH=%PATH:old=new%"` 字符串替换对含空格路径不可靠，应在 PowerShell 层面处理
2. VsDevCmd.bat 虽然设置了 MSVC 工具路径，但不会移除已有的 Git 路径
**涉及文件**: scripts/release-all-win.ps1
**使用次数**: 0
**使用次数**: 0
