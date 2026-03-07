# Process Log

本文件记录 LazyCat 项目中重要/复杂操作的处理流程与踩坑经验。

**使用次数规则**：每条记录有 `使用次数` 字段，初始为 0。后续会话遇到相同问题并参考该记录时 +1，并追加引用日期。当使用次数 >= 3 时，固化到 `CLAUDE.md` 对应章节。

---

<!-- 新记录添加在此处，最新的在最上面 -->

## 2026-03-07: 本地待办多提醒与逐条稍后提醒改造

**场景**: 用户要求调整本地待办新增/编辑弹窗的字段顺序与创建态文案，同时把提醒从单选升级为多选，新增默认勾选的“准时提醒”，并让“稍后10分钟”只影响当前触发的那一条提醒。

**问题**:
1. 现有待办模型从前端类型到 Rust 调度都建立在“单事项仅一个提醒时间”的前提上，`todo_tasks.remind_at` 和 `todo_templates.reminder_offset_minutes` 都只能表达单提醒。
2. 提醒中心事件与系统通知只携带 `taskId`，没有具体提醒记录标识，多提醒下无法精确实现“逐条稍后”。
3. 前端 `TodoPanel.vue` 同时承担表单、列表展示和历史兼容解析，若不先统一提醒数组语义，`typecheck` 虽可能通过，但运行时容易出现“默认值/无提醒/旧数据回填”不一致。

**解决**:
1. 前端把 `reminderPreset` 全量切为 `reminderPresets`，增加 `0m` 准时提醒与互斥的 `none` 哨兵值；创建态默认 `['0m']`，编辑已有无提醒事项时显示为 `['none']`。
2. Rust 端在 `helpers.rs` 增加 migration 18，新增 `todo_task_reminders` 与 `todo_template_reminders` 两张子表，把旧单提醒数据迁移成单元素提醒集合，并为 `todo_reminder_events` 补 `task_reminder_id` 与 `reminder_preset`。
3. `todo.rs` 的任务创建、任务更新、周期模板创建、周期模板更新、周期实例生成、提醒派发与提醒中心列表统一改为围绕提醒子表工作；旧列保留但不再作为主真源。
4. “稍后10分钟”改为优先吃 `taskReminderId`；若列表按钮未显式传入，则后端自动选择该事项最近一条仍可触发的提醒。
5. 前端弹窗字段顺序调整为“提醒 → 事项类型 → 周期规则 → 描述”，创建态标题固定“新增事项”，提交按钮固定“创建事项”。

**关键点**:
1. `none` 只作为前端互斥选项存在，提交到后端时必须转为空数组，不能和真实提醒预设一起落库。
2. 多提醒的 `snooze_until` / `last_notified_at` 必须下沉到提醒子表；若继续复用任务表字段，会导致一条提醒被稍后后误伤同事项的其它提醒。
3. 周期实例生成时不要直接复制旧 `remind_at`，而是基于实例 `event_at` 和模板提醒偏移重新计算，才能保证多个提醒时间都正确。

**涉及文件**:
- `apps/desktop/src/components/TodoPanel.vue`
- `apps/desktop/src/types/todo.ts`
- `apps/desktop/src/App.vue`
- `apps/desktop/src-tauri/src/tools/todo.rs`
- `apps/desktop/src-tauri/src/tools/helpers.rs`

**验证**:
- `cargo test todo:: -- --nocapture`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`
- `pnpm test`

**使用次数**: 0

## 2026-03-07: 本地待办调度区重构为日期/时间/提醒/重复

**场景**: 用户要求把本地待办的新增/编辑弹窗重构为更接近日历应用的调度体验，核心围绕“日期、时间、提醒、重复”，并为周期事项补上显式开始日期。

**问题**:
1. 前端 `TodoPanel.vue` 已经拆出新草稿字段，但模板、调度工具和保存逻辑存在两套接口并存，容易出现导出名不一致与提交流程断链。
2. 后端周期模板此前没有 `start_at`，`next_occurrence_at` 默认从保存当下开始推算，无法表达“从指定日期开始重复”。
3. 工具函数与测试一度处于新旧命名混用状态，若不先收敛为单一真源，`typecheck` 与 `build:web` 很容易反复失败。

**解决**:
1. 前端把 `TodoPanel.vue` 的调度区统一为单次事项 `singleDate/singleTime` 与周期事项 `recurrenceStartDate/recurrenceTime/repeatPreset` 两套草稿，并在 `saveItem` 中映射为 `eventAt` 与 `recurrence.startAt`。
2. 新增 `src/utils/todoSchedule.ts` 作为调度规则单一真源，同时兼容旧测试接口和新面板接口，统一提供重复预设、日期时间拆装、规则摘要与结束条件格式化。
3. Rust 端在 `helpers.rs` 增加 migration 17，为 `todo_templates` 增加 `start_at` 并回填历史数据；`todo.rs` 的模板创建、更新、启停、实例生成全部改为尊重 `start_at`。
4. 周期规则继续沿用 simple/Cron 双轨，但简单月规则允许 31 号；前端对“每周自定义且间隔大于 1”直接提示改用高级 Cron，避免后端 silent ignore。

**关键点**:
1. `start_at` 是周期系列的生效下界，不等于 `next_occurrence_at`；创建时要按 `start_at` 首次计算，更新/启用时则按 `max(now, start_at)` 重算，避免重复补历史实例。
2. 若工具文件已演进过多轮，优先收敛为一个稳定导出面，再回头补 `TodoPanel.vue` 与测试，成本低于在两套接口之间硬凑兼容。
3. 图片里的“时间段”这轮不落数据模型，只保留具体时间；“不重复”在现有双模型里仍由单次事项承担，因此周期编辑态需要阻止直接改成不重复。

**涉及文件**:
- `apps/desktop/src/components/TodoPanel.vue`
- `apps/desktop/src/utils/todoSchedule.ts`
- `apps/desktop/src/utils/todoSchedule.test.ts`
- `apps/desktop/src-tauri/src/tools/todo.rs`
- `apps/desktop/src-tauri/src/tools/helpers.rs`

**验证**:
- `pnpm typecheck`
- `pnpm test src/utils/todoSchedule.test.ts`
- `pnpm --filter @lazycat/desktop build:web`
- `cargo test todo:: -- --nocapture`

**使用次数**: 0

## 2026-03-07: 本地待办工具（任务+周期+提醒）一体化落地

**场景**: 新增本地待办能力，要求支持任务类型、优先级、执行人、提醒、周期提醒与周期事件，并同时提供系统提醒与应用内提醒中心。

**问题**:
1. 现有仓库没有 `todo` 工具域，前后端通道、数据表、面板均为空白。
2. 需要兼顾单次任务与周期实例，且应用退出后重启要补偿错过提醒。
3. 系统提醒需与应用内提醒状态同步，避免重复提醒与丢提醒。

**解决**:
1. Rust 新增 `tools/todo.rs`，实现类型/执行人/任务/周期模板/提醒中心 action，及 `scheduler_tick` 调度入口。
2. `helpers.rs` 增加 migration 13，创建 `todo_*` 系列表并注入内置类型（待报事项、工作任务、会议安排、个人事项）。
3. `main.rs` 增加调度线程：每 30 秒执行周期实例生成 + 到期提醒派发；同时发送系统通知并 `emit(\"todo-reminder-fired\")` 给前端。
4. 前端新增 `TodoPanel.vue`，提供任务管理、周期管理、提醒中心与基础数据管理；`App.vue` 全局监听提醒事件并弹通知。
5. 通道映射与类型体系扩展：`bridge/tauri.ts` 新增 `tool:todo:*`，`types/todo.ts` 与 `types/index.ts` 新增导出，`tool-registry.ts` 注册 `todo` 面板。

**关键点**:
1. 周期模板统一存储 Cron 表达式，简单规则在保存时转换为 Cron，降低调度复杂度。
2. 提醒触发条件采用 `COALESCE(snooze_until, remind_at)` + `last_notified_at` 去重，支持“稍后提醒”复触发。
3. 为防止离线过久导致单轮阻塞，周期补偿每轮每模板最多生成 500 条实例，后续轮次继续补齐。

**涉及文件**:
- `apps/desktop/src-tauri/src/tools/todo.rs`
- `apps/desktop/src-tauri/src/tools/helpers.rs`
- `apps/desktop/src-tauri/src/main.rs`
- `apps/desktop/src/bridge/tauri.ts`
- `apps/desktop/src/components/TodoPanel.vue`
- `apps/desktop/src/App.vue`
- `apps/desktop/src/types/todo.ts`
- `apps/desktop/src/types/index.ts`

**使用次数**: 0

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

## 2026-03-07: 本地待办统一为事项实例 + 周期系列

**场景**: 用户希望把原本分开的“任务”和“周期事件”整合成统一模型与统一维护入口，主列表以当前可执行事项为中心。

**问题**:
1. 旧实现虽然在同一个 `todo` 工具内，但前端仍按 `task/template` 两套对象分栏维护。
2. 后端缺少“单次事项也属于系列”的统一语义，周期规则与实例操作边界不清晰。
3. 前端编辑器没有统一承载“单次事项 / 周期事项 / 当前实例 / 后续系列”四种编辑上下文。

**解决**:
1. `helpers.rs` 新增 migration 14：为 `todo_templates` 增加 `series_kind`，并把历史孤立任务回填为 `one_off` 系列。
2. `todo.rs` 新增 unified actions：`item_*` 与 `series_*`，同时保留 `task_*` / `template_*` 兼容别名。
3. 主列表统一走 `item_list`，补充 `seriesId`、`seriesKind`、`isRecurring`、`canEditFuture`、`displayAt` 等字段。
4. `TodoPanel.vue` 重构为“事项 / 系列 / 提醒中心 / 基础数据”四视图，并用单一弹窗统一创建与编辑。
5. 周期实例编辑支持 `this_instance` 与 `future_instances` 两种作用域；后者由后端转为系列更新。

**关键点**:
1. 单次事项创建时也自动创建 `one_off` 系列，避免后续逻辑继续依赖空的 `source_template_id`。
2. 周期系列继续保留“生成实例”语义，调度器只处理 `recurring` 且启用中的系列。
3. 系列删除不会删除历史实例；已生成实例会退化为独立事项继续保留。

**涉及文件**:
- `apps/desktop/src-tauri/src/tools/helpers.rs`
- `apps/desktop/src-tauri/src/tools/todo.rs`
- `apps/desktop/src/bridge/tauri.ts`
- `apps/desktop/src/components/TodoPanel.vue`
- `apps/desktop/src/types/todo.ts`
- `apps/desktop/src/types/index.ts`

**验证**:
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`
- `cargo check`

**使用次数**: 0

## 2026-03-07: 本地待办自动收藏与命名快捷键接入

**场景**: 用户希望“本地待办”默认出现在首页常用工具中，并像代码片段/密码管理/快捷启动一样支持单独的全局快捷键呼出。
**问题**:
1. 首页“常用工具”当前完全依赖 `favorites` 与近 30 天点击历史，没有“默认收藏一次性补种”机制。
2. 现有命名快捷键链路已支持任意目标工具，但前端只暴露了 `snippets`、`vault`、`launcher` 三个配置入口。
3. 如果直接每次启动都强行把 `todo` 加回收藏，会覆盖用户手动取消收藏的意图。
**解决**:
1. 在 `useFavorites.ts` 中抽出 `normalizeFavoriteToolIds` 与 `bootstrapFavoriteToolIds`，统一做收藏去重、过滤与待办一次性补种。
2. 新增 `favorites_todo_seeded` 标记：首次启动时若收藏中没有 `todo`，自动插入收藏首位；一旦补种完成或用户原本已收藏，即写入标记，后续不再重复干预。
3. 在 `SettingsPanel.vue` 增加“本地待办”快捷键录入项，并纳入现有冲突检测、保存与清空流程。
4. 在 `App.vue` 启动阶段读取 `hotkey_todo`，通过现有 `registerNamedHotkey("todo", ...)` 注册；继续复用 `hotkey-navigate` 的显隐/跳转逻辑。
5. 新增 `useFavorites.test.ts` 覆盖补种规则，并补充 `hotkeyNavigate.test.ts` 的 `todo` 场景回归。
**关键点**:
1. “固定到常用工具”在本需求里等价于“走现有收藏模型的一次性自动收藏”，不是新增永久固定入口。
2. 对已手动收藏 `todo` 的用户也要写入补种完成标记，避免日后取消收藏后又被系统重新加回。
3. 复用现有命名快捷键协议即可，前后端无需新增 Tauri command 或事件结构。
**涉及文件**:
- `apps/desktop/src/composables/useFavorites.ts`
- `apps/desktop/src/composables/useFavorites.test.ts`
- `apps/desktop/src/App.vue`
- `apps/desktop/src/components/SettingsPanel.vue`
- `apps/desktop/src/utils/hotkeyNavigate.test.ts`

**验证**:
- `pnpm --filter @lazycat/desktop test src/utils/hotkeyNavigate.test.ts src/composables/useFavorites.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-03-07: 本地待办事件时间与提醒预设重构

**场景**: 用户要求把本地待办里的“截止时间 + 提醒时间”重构为“事件时间 + 提醒预设”，事件时间最小刻度统一为 5 分钟，并把周期系列从独立页签合并到事项页下方折叠区。
**问题**:
1. 旧模型同时维护 `due_at` / `remind_at`，单次事项与周期实例的含义不一致，前端也需要维护两个绝对时间输入。
2. 周期系列没有单独的提醒偏移字段，生成实例时只能把提醒时间直接写成触发时刻。
3. 现有文件里有历史乱码文案，重构过程中很容易把 Rust 字符串语法一并带坏，必须依赖编译器逐轮清理。
**解决**:
1. 前端类型与表单统一切到 `eventAt + reminderPreset`，提醒预设固定为 `none/5m/10m/30m/1h/1d/2d`，并在表单提交前校验 5 分钟刻度。
2. Rust 端新增 `event_at` 与 `reminder_offset_minutes` 模型：任务对外只暴露事件时间与提醒预设，内部继续复用 `remind_at + snooze_until` 做提醒调度。
3. migration 15 为历史数据回填 `event_at`，只保留能精确映射到新预设的旧提醒，其余旧 `remind_at` 直接清空；周期模板提醒偏移统一置空。
4. 事项页合并“系列”页签，在列表下方折叠展示周期系列，周期事项的规则区块移动到弹窗下半部分。
5. Rust 单测补充 5 分钟刻度与提醒预设换算，并用 `cargo test todo:: -- --nocapture` 做定向回归；前端用 `pnpm typecheck` 与 `pnpm --filter @lazycat/desktop build:web` 验证联调。
**关键点**:
1. 对任务编辑来说，`eventAt` 或 `reminderPreset` 任一变更都要重新计算 `remind_at`，同时清空 `snooze_until` 与 `last_notified_at`，避免旧稍后提醒污染新计划。
2. 对周期模板来说，只存提醒偏移，不存绝对提醒时间；实例生成时再根据发生时间反推 `remind_at`。
3. 处理历史乱码文件时，不要盲目整文件替换；先跑编译，再按报错行定点修复，成本最低、风险也最小。
**涉及文件**:
- `apps/desktop/src/components/TodoPanel.vue`
- `apps/desktop/src/types/todo.ts`
- `apps/desktop/src/types/index.ts`
- `apps/desktop/src-tauri/src/tools/todo.rs`
- `apps/desktop/src-tauri/src/tools/helpers.rs`

**验证**:
- `cargo test todo:: -- --nocapture`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0
