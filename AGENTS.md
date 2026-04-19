# AGENTS.md

本文件为在本仓库内工作的编码代理（Codex / Claude / 其他）提供统一项目上下文与协作规范。
如与用户明确指令冲突，以用户指令为准。

> **双文件同步约束**：本文件（`AGENTS.md`）与 `CLAUDE.md` 共同维护项目规范。
> 更新本文件的任何章节时，必须同步检查并更新另一份文件对应内容，保持两者一致。

## 01. 文档定位

适用场景：

- 初次进入仓库，需要先明确执行边界与核心约束。
- 不确定当前任务该直接实现、还是先确认范围时，先看本节。
- 修改规范类文档前，先确认双文件同步约束。

### 01.1 文档作用

- 本文件用于统一项目级代理协作规则、项目上下文和高频执行约束。
- 用户明确指令优先级高于本文件；若两者冲突，以用户要求为准。
- 本文件偏“长期稳定规则”，具体任务经验按 `process.md` 沉淀。

### 01.2 核心硬规则

- 当用户要求实现功能时，先做最小必要上下文确认，然后立即进入实现；除非用户明确要求，否则不要长时间停留在计划阶段。
- 当用户提出信息类问题时，直接回答，不要无意义进入代码改动流程。
- 所有 CDN 依赖和外部资源（字体、Monaco 等）必须本地打包；运行时不得依赖公网 CDN。
- UI 默认保持干净浅色/白色风格，除非用户明确要求其他视觉方向。
- Windows 环境注意：控制台可能为 GBK 编码；运行中的 `.exe` 会持有文件锁，重建前需先结束进程。
- 便携包/绿色包交付优先 `zip` 形态；当前 `pnpm build:portable` 底层仍执行 NSIS 构建（历史命名），需要再走脚本或手动封装为 `zip`。

## 02. 快速检索

适用场景：

- 已经知道当前问题类型，只想快速定位到对应章节。
- 需要在正式动手前做一次防错扫描。
- 需要判断当前任务该走哪条执行路径。

### 02.1 问题导向索引

- 要看项目背景、目录或命令：看 `03`
- 要理解调用链路、前后端边界或新增工具入口：看 `04`
- 要改 Element Plus 样式：看 `05.1`
- 要处理中文乱码、编码或写文件问题：看 `05.2`
- 要查数据目录、迁移与备份策略：看 `05.3`
- 要新增离线手册：看 `05.4`
- 要看 Windows 构建、便携包、WebView2 或正式发版：看 `06`
- 要看协作纪律、可视化辅助和 `process.md` 规则：看 `07`
- 要按场景执行防错检查：看 `07.4` 到 `07.7`
- 要看提交规范和推送前检查：看 `08`

### 02.2 Agent 决策闸门

开工前按顺序检查以下 5 件事：

1. 当前任务属于哪个场景。
   - 文档规范改动：进入 `07.4`
   - 普通功能开发：进入 `07.5`
   - UI / 样式改动：进入 `07.6`
   - 高风险改动：进入 `07.7`
2. 当前任务是否有同步约束。
   - 修改 `AGENTS.md` 时，必须同步 `CLAUDE.md`，反之亦然
   - 修改 Element Plus 覆盖时，必须同步检查 `05.1` 中提到的两个样式文件
   - 复杂任务涉及 `3+` 文件时，完成后评估是否要记录 `process.md`
3. 当前任务是否需要先确认用户。
   - 破坏性操作、批量改动、大范围资源变更、数据迁移或外部副作用，先进入 `07.7`
4. 当前任务的最低验证要求是什么。
   - 文档改动：至少校验结构一致性和关键规则完整性
   - 功能改动：按影响面执行相关测试、`pnpm typecheck`，必要时执行 `pnpm --filter @lazycat/desktop build:web`
   - UI / 样式改动：除上述验证外，额外检查主题覆盖、空态、弹窗态和边界态
5. 当前任务完成后是否需要经验沉淀。
   - 先看 `07.3`
   - 若经验使用次数达到 `3` 次以上，再固化回规范文件

## 03. 项目速览

适用场景：

- 需要快速了解项目是什么、跑在什么环境、主要目录在哪里。
- 需要确认常用命令，而不是从 `package.json` 或脚本中现翻。
- 需要先建立仓库地图，再进入具体实现。

### 03.1 项目概览

- 名称：Lazycat（懒猫）
- 类型：离线桌面开发者工具箱
- 平台：Windows 优先
- 技术栈：Tauri 2 + Vue 3 + TypeScript + Rust
- 终端：PowerShell（命令串联使用 `;`，不要使用 `&&`）
- 本机能力：已安装 Python 与 Node.js

### 03.2 仓库结构

```text
apps/desktop/                    Tauri 桌面应用
  src-tauri/                     Rust 后端
    src/tools/                   工具域模块、mod.rs、helpers.rs
  src/components/                Vue 面板组件
  src/composables/               状态管理 composables
  src/bridge/tauri.ts            IPC 通道映射
  src/tool-registry.ts           工具 ID -> 异步组件注册
packages/formatters/             Prettier standalone（唯一实际使用 package）
resources/manuals/               离线手册（Vue 3、Element Plus、MDN JavaScript）
resources/regex-library/         内置正则模板
resources/hotkey-library/        快捷键库资源
scripts/                         构建脚本（build-tauri-win.ps1、release-all-win.ps1、scrape-mdn-js.mjs）
```

### 03.3 本地命令

| 命令 | 说明 |
|------|------|
| `pnpm install` | 安装依赖 |
| `pnpm dev` | 开发模式 |
| `pnpm typecheck` | 全工作区类型检查 |
| `pnpm --filter @lazycat/desktop build:web` | 渲染层构建 |
| `pnpm build` | 全量构建 |
| `pnpm test` | 单元测试 |
| `pnpm test:e2e` | E2E 测试 |
| `pnpm build:win:precheck` | Windows 构建预检 |
| `pnpm build:win` | Windows NSIS 打包 |
| `pnpm build:portable` | 当前与 `build:win` 同底层（NSIS）；便携 `zip` 需额外封装 |
| `pnpm release:all:win -- -Tag vX.Y.Z` | 构建安装包/绿色包、生成 SHA256、推送 tag 并上传 GitHub Release |

## 04. 开发与架构要点

适用场景：

- 需要确认功能调用链路、前后端边界和接入方式。
- 需要新增工具或扩展现有工具。
- 需要判断当前改动会联动哪些层。

### 04.1 常规工具调用链路

- 前端入口：`bridge/tauri.ts` 的 `invokeToolByChannel`
- 通道映射：`tool:<domain>:<action>` -> `{ domain, action }`
- Rust 分发：`tool_execute` -> `src-tauri/src/tools/mod.rs` 的 `execute_tool`

### 04.2 capture 特殊链路

- `capture` 模块位于 `src-tauri/src/tools/capture.rs`，但不走 `CHANNEL_MAP` / `tool_execute`。
- 抓包能力通过 Tauri 独立 command 暴露，例如 `start_capture`、`stop_capture`、`export_pcap`。
- 入口位于 `src-tauri/src/main.rs`。

### 04.3 前端组织

- 未使用 `vue-router`；`App.vue` 通过 `activeTool` + `tool-registry.ts` 动态加载面板。
- 工具面板通过 `<component :is="currentComponent">` 渲染。
- 新增前端工具入口时，通常会同时改 `App.vue`、`tool-registry.ts` 和对应面板组件。
- 面板内部多视图切换走相同模式：PM 面板的 `composables/pmViewRegistry.ts` 注册 `kanban/gantt/today/list/calendar/matrix` 6 个视图，`PmPanel.vue` 通过 `<component :is="currentView.component">` 渲染；视图选择按上下文记忆，由 `composables/usePmViewMemory.ts` 读写 `user_settings`（key 规则 `pm:view:overview` 或 `pm:view:project-<id>`）。

### 04.4 关键 Composables

- `useToolInvoke`：IPC 调用包装，管理 loading / error 状态。
- `useTabs`：标签页管理，支持打开、切换、关闭和快捷键切换。
- `useSettings`：设置读写持久化。
- `useFavorites`：收藏夹与点击历史，驱动高频推荐。
- `useMenuVisibility`：侧边栏显隐管理。
- `useClipboardSuggestion`：剪贴板智能检测与工具推荐。

### 04.5 持久化与格式化

- XML / HTML / Java / SQL 格式化在 Rust 端为直通模式，核心依赖 `@lazycat/formatters`（Prettier standalone + 显式解析器插件）。
- `user_settings` 主要存储用户偏好与配置项。
- 业务数据按域存储在独立表中，例如 `hosts_profiles`、`snippet_*`、`vault_*`、`launcher_entries`、`todo_items`。

### 04.6 新增工具标准流程

常规工具（走 channel 分发）：

1. 在 `apps/desktop/src/App.vue` 的 `sidebarItems` 注册入口。
2. 在 `apps/desktop/src/tool-registry.ts` 注册异步组件。
3. 新增 `apps/desktop/src/components/XxxPanel.vue` 面板。
4. 如需后端，在 `apps/desktop/src/bridge/tauri.ts` 的 `CHANNEL_MAP` 增加通道。
5. 如需后端，在 `apps/desktop/src-tauri/src/tools/` 新增 Rust 模块，并在 `mod.rs` 注册。

非 channel 工具（如 `capture`）：

1. 先完成前端入口、组件和工具注册。
2. 再在 `src-tauri/src/main.rs` 增加并注册 Tauri command。
3. 前端通过 `@tauri-apps/api/core` 的 `invoke` 直接调用 command。

### 04.7 PM 域视图扩展

- PM 后端按视图拆分到独立模块：`pm.rs`（CRUD 主干）、`pm_today.rs`（今日视图）、`pm_calendar.rs`（日历视图）、`pm_matrix.rs`（四象限视图）、`pm_siyuan.rs`（思源集成）、`pm_todo_link.rs`（Todo 打通）。
- 5 个扩展 action（均走常规通道分发）：
  - `item_today_list` / `item_today_counts`：今日视图分区数据与侧栏 badge 计数，参数含 `todayDate` 客户端本地日期。
  - `item_calendar_range`：日历视图区间查询，参数 `startDate`、`endDate`。
  - `item_matrix_bucket`：四象限分桶，参数 `urgentThresholdDays`、`hideCompleted`、`todayDate`。
  - `item_batch_update`：列表视图批量改 `status/priority/project/pinned`，事务中一次写入。
- 性能索引已在 `helpers.rs` 建好：`idx_pm_items_project_status`、`idx_pm_items_end_at`、`idx_pm_items_status`、`idx_pm_items_updated_at`、`idx_pm_items_completed_at`。跨项目查询（`project_id IS NULL`）依赖 `end_at`/`status`/`completed_at` 索引避免全表扫描。
- 列表视图 `PmListView.vue` 在无分组且数据 > 500 行时启用渐进式渲染（初始 200 行，滚动底部追加 200），避免大数据量初次渲染卡顿。

## 05. 高频注意事项

适用场景：

- 当前问题不是“怎么做功能”，而是“为什么这里容易踩坑”。
- 改样式、编码、数据目录、离线手册时，先看本节。
- 需要快速定位常见高频陷阱，而不是重读整份文档。

### 05.1 Element Plus 样式覆盖

- 规则：修改 Element Plus 组件样式变量时，必须同时检查 `element-overrides.css` 和 `theme-light.css`。
- 原因：`theme-light.css` 使用 `html[data-theme="light"]` 前缀，特异度更高；只改前者会被浅色主题覆盖。
- 加载顺序：`src/styles/index.css` 中，`element-overrides.css` 先于 `theme-light.css` 加载。
- 典型案例：给 `.el-button--primary` 增加 `.is-text` 变体时，如果只在 `element-overrides.css` 中追加，浅色主题下会被 `theme-light.css` 覆盖。
- `ElMessageBox` 宽度控制：`.el-message-box` 使用 `max-width: var(--el-messagebox-width)`；正确做法是通过 `customClass` 覆盖 CSS 变量，例如 `--el-messagebox-width: 580px`，而不是直接改 `width`。

### 05.2 编码与中文规范

- 源码统一 UTF-8，禁止 ANSI / GBK / UTF-16。
- PowerShell 写文件必须显式 `-Encoding UTF8`。
- 含中文文件避免整文件替换，优先按块精确修改。
- 如补丁报 `stream did not contain valid UTF-8`，先转 UTF-8 再改。
- 文案默认中文，JSON / SQL / JWT 等术语可保留英文。
- 乱码修复顺序：语法结构 -> 显示文本 -> 构建验证（`typecheck` + `build:web`）。

### 05.3 数据路径与目录策略

- 指针配置：`%USERPROFILE%\.lazycat\config.json`（固定位置）
- 数据库：`<数据目录>\lazycat.sqlite`（默认 `%USERPROFILE%\.lazycat\lazycat.sqlite`）
- Hosts 备份：`<数据目录>\hosts-backups`
- 自定义数据目录不可达时静默回退默认目录，不崩溃。
- 迁移时复制 `lazycat.sqlite` 与 `hosts-backups/`；目标目录若已存在数据库文件则拒绝覆盖。

### 05.4 离线手册

- 每个手册启动独立本地 HTTP 端口；VitePress 绝对路径资源要求独立根路径。
- 前端 `ManualPanel.vue` 通过 `<iframe>` 内嵌展示。
- 当前内置手册：
  - `vue3`：Vue 3 开发手册，源码构建
  - `element-plus`：Element Plus 组件库，Puppeteer 抓取
  - `mdn-js`：MDN JavaScript 手册，Puppeteer 抓取

新增手册：

1. 获取中文静态产物，优先源码构建，兜底 Puppeteer，参考 `scripts/scrape-mdn-js.mjs`。
2. 复制到 `resources/manuals/<id>/`。
3. 同时修改 `manuals.rs` 的 `known` 和 `App.vue` 的 `sidebarItems`（前端 id：`manual-<id>`）。
4. 如需清理噪音元素，在 `main.rs` 的 `INJECT` CSS 选择器中补充。
5. 路径解析规则：打包时用 `resource_dir()/manuals`，开发态 fallback 到项目根 `resources/manuals`。

## 06. 构建、打包与发布

适用场景：

- 需要本地构建、打包、排查 Windows 环境问题。
- 需要判断安装包与绿色包的差异。
- 需要正式发布 GitHub Release。

### 06.1 构建与打包要点

- 规则：必须使用 `tauri build`，不要用 `cargo build --release`。
- 原因：后者不会嵌入前端资源，最终会白屏。
- `pnpm build:portable` 当前等价于 NSIS 构建流程；绿色 `zip` 仍需通过 `release-all-win.ps1` 或手动 `7z` 封装。
- `main.rs` 启动时会扫描 exe 同级 `Microsoft.WebView2.FixedVersionRuntime.*`；若存在则自动切换到本地 WebView2。
- `release-all-win.ps1` 已处理 Git `usr/bin/link.exe` 遮蔽 MSVC 链接器、便携包 DLL 输出路径变化、旧 PowerShell 缺少 `Get-FileHash` 的兼容问题。
- `build:web` 出现 `spawn EPERM` 时先重试，仍失败再提升权限重试。

### 06.2 产物类型

| 方式 | 产物 | 离线可用 | 适用场景 |
|------|------|:--------:|----------|
| NSIS 轻量安装包 | `.exe` | 否 | 目标机已有 WebView2 或可联网 |
| NSIS 离线安装包 | `.exe` | 是 | 离线 Win10 部署 |
| 绿色轻量包 | `.zip` | 否 | 目标机已有 WebView2 |
| 绿色离线包 | `.zip` | 是 | 离线环境解压即用 |

### 06.3 GitHub Release 正式流程

1. 先统一版本号，至少同步根 `package.json`、`apps/desktop/package.json`、`apps/desktop/src-tauri/Cargo.toml`、`apps/desktop/src-tauri/tauri.conf.json`；发布 tag 固定为 `v<version>`。
2. 正式发版只从 `main` 的干净工作区执行；版本变更、脚本修复和 Release 说明都要先提交到 Git，并先推送 `origin/main`。
3. 发版前至少执行 `pnpm typecheck`、`pnpm --filter @lazycat/desktop build:web`、`pnpm test`；需要完整发布检查时再补 `pnpm test:e2e`。
4. 正式发 GitHub Release 使用 `pnpm release:all:win -- -Tag vX.Y.Z`。脚本会校验版本一致性、tag 与版本匹配、当前分支是否为 `main`、工作区是否干净，并在发 tag 前先推送当前 `main`。
5. 若构建已完成，但哈希生成或 GitHub 上传阶段中断，使用 `pnpm release:all:win -- -Tag vX.Y.Z -SkipBuild` 继续，不要重复完整构建。
6. 若只需要本地出包、不上传 GitHub Release，使用 `pnpm release:all:win -- -Tag vX.Y.Z -SkipUpload`；该模式只生成产物与 `SHA256SUMS.txt`。

## 07. 协作与变更纪律

适用场景：

- 当前问题不在“架构怎么接”，而在“执行时有哪些边界不能漏”。
- 需要判断是否该启用可视化、是否该记 `process.md`、是否属于高风险操作。
- 需要按场景走一遍 agent 防错清单。

### 07.1 默认执行原则

- 不自动启动 UI 或 dev server；仅在用户明确要求时执行 `pnpm dev`。
- 优先小步、可验证改动，避免无关重构。
- 较大改动验证通过后及时提交，避免改动堆积。
- 未经用户明确要求不执行破坏性命令。
- 网页抓取 `WebFetch` 失败时 fallback 到 Playwright。
- 涉及 `resources/manuals/**` 大量变更（`>100` 文件）时，提交前必须与用户确认范围。

### 07.2 可视化辅助

- 可视化辅助默认开启：涉及布局、交互、结构对比、流程图、信息架构或其他明显更适合视觉表达的内容时，默认优先使用本地预览并在浏览器中展示。
- 该规则不等于自动启动产品 UI 或 `pnpm dev`；优先使用仓库现成的本地预览脚本。
- Windows 优先使用 `scripts/start-server.ps1` / `scripts/stop-server.ps1`；兼容入口为 `scripts/start-server.sh` / `scripts/stop-server.sh`。
- 展示内容可以是本地预览页、HTML 原型页、对比页面、流程图 / 线框图页面或浏览器内说明页。
- 仅在用户明确要求纯文本，或当前内容明显更适合终端文本时，回退到文字沟通。

### 07.3 process.md 经验沉淀

- 开始复杂任务前先查 `process.md` 是否有同类经验。
- 复杂任务（`3+` 文件）完成后记录到 `process.md`。
- 使用次数 `>= 3` 的经验固化到规范文件。

### 07.4 文档规范改动检查清单

**开始前**

1. 确认是否涉及 `AGENTS.md` / `CLAUDE.md` 双文件同步。
2. 判断本次是结构优化、措辞优化，还是规则变更；若涉及规则变更，先与用户确认。
3. 先检查 `process.md` 是否已有同类经验可复用。

**实施中**

1. 修改一份规范文件时，同步检查另一份对应章节。
2. 避免无意新增规则语义、删除仍有效约束，或改变已有风险边界。
3. 让文档更利于 agent 检索和执行，不要只增加篇幅。

**完成前**

1. 检查两份文件的章节结构、关键规则和交叉引用是否一致。
2. 检查 `02.2 Agent 决策闸门` 与 `07.4-07.7` 是否都能落到具体章节。
3. 若本次已构成复杂任务，评估是否需要记录 `process.md`。

**必须停下并确认用户**

1. 需要改变现有规则语义。
2. 需要扩大高风险操作边界。
3. 需要调整用户已经确认过的文档方向。

### 07.5 普通功能开发检查清单

**开始前**

1. 确认当前功能主要落点：前端组件、bridge、Rust 工具、类型、数据库、测试。
2. 先看 `04` 对应调用链路和 `process.md` 同类经验。
3. 若属于新增工具，优先按 `04.6` 规划接入点；若涉及高风险或数据迁移，转入 `07.7`。

**实施中**

1. 优先复用现有 helper、composable 和既有接入模式，不临时另起一套。
2. 同步检查前端、bridge、后端、类型、测试 fixture 是否联动。
3. 在 dirty worktree 下避免误改无关文件。
4. 能抽成纯函数并做单测的逻辑，不要全部堆进组件。

**完成前**

1. 按影响面执行最小必要验证；常见基线是相关测试、`pnpm typecheck`，必要时执行 `pnpm --filter @lazycat/desktop build:web`。
2. 检查是否遗漏通道注册、类型导出、测试桩、迁移脚本或持久化变更。
3. 若任务涉及 `3+` 文件或沉淀出稳定经验，记录 `process.md`。

**必须停下并确认用户**

1. 需要做破坏性操作。
2. 需要修改数据库结构或迁移用户数据。
3. 与用户当前未提交改动直接冲突。

### 07.6 UI / 样式改动检查清单

**开始前**

1. 确认本次是局部视觉修正，还是整体视觉方向变更；后者先与用户确认。
2. 先看 `05.1` 的样式覆盖与主题联动说明。
3. 若任务更适合视觉表达，按 `07.2` 准备本地预览。

**实施中**

1. 修改 Element Plus 变量时，同步检查 `element-overrides.css` 和 `theme-light.css`。
2. 确认样式放在正确作用域：`scoped`、全局样式或 Teleport 样式。
3. 只做当前任务相关的视觉调整，避免顺手扩散成大面积重构。
4. 同时留意浅色主题、空态、交互态、弹窗态和移动端表现。

**完成前**

1. 检查浅色主题是否被高特异度规则覆盖。
2. 视影响面执行 `pnpm typecheck`，必要时执行 `pnpm --filter @lazycat/desktop build:web`。
3. 若本次改动形成稳定经验，记录 `process.md`。

**必须停下并确认用户**

1. 需要明显改变整体视觉方向。
2. 需要大面积重写既有设计系统。
3. 需要同时触及大量视觉资源或手册资源。

### 07.7 高风险改动检查清单

**开始前**

1. 判断是否涉及破坏性操作、数据库结构调整、数据迁移、大范围文件改动或外部副作用。
2. 明确影响范围、目标对象和回退方式。
3. 若涉及 `resources/manuals/**` 大量变更（`>100` 文件），提交前先与用户确认范围。

**实施中**

1. 严格限制改动范围，不顺手清理无关内容。
2. 对数据或文件操作，先确认目标路径、目标对象和影响面。
3. 尽量提供兼容或迁移路径，避免直接破坏用户数据。

**完成前**

1. 明确说明影响范围、验证结果和剩余风险。
2. 完成必要验证，再结束任务。
3. 若形成稳定经验，记录 `process.md`。

**必须停下并确认用户**

1. 删除文件或目录、执行其他破坏性命令前。
2. 需要批量修改或迁移大量数据 / 文件时。
3. 需要修改数据库结构、覆盖用户数据或影响外部环境时。

## 08. 提交与验证

适用场景：

- 准备提交代码或向用户汇报验证结果时。
- 需要确认提交格式和推送前检查项时。
- 需要把“已完成修改”收束成可交付状态时。

### 08.1 提交规范

- 约定式前缀：`feat:`、`fix:`、`docs:`、`chore:`、`test:`
- 提交信息使用中文，例如：`feat(launcher): 添加分组管理和使用次数排序`

### 08.2 推送前检查

1. `pnpm typecheck`
2. `pnpm --filter @lazycat/desktop build:web`
3. `pnpm test`
4. `pnpm test:e2e`
