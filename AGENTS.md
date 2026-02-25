# AGENTS.md

本文件定义在本仓库内工作的编码代理（Codex/Claude/其他）统一执行规范。
如与用户明确指令冲突，以用户指令为准。

> **双文件同步约束**：本文件（`AGENTS.md`）与 `CLAUDE.md` 共同维护项目规范。
> 更新本文件的任何章节时，必须同步检查并更新 `CLAUDE.md` 中的对应内容，保持两者一致。

## 1. 项目概览

- 项目：Lazycat（懒猫）
- 类型：离线桌面开发者工具箱
- 平台：Windows 优先
- 技术栈：Tauri 2 + Vue 3 + TypeScript + Rust
- 终端环境：PowerShell（命令串联使用 `;`，不要依赖 `&&`）
- 本机脚本能力：已安装 `Python` 与 `Node.js`，可用于处理小型脚本、数据修复与临时排障

## 2. 仓库结构

- `apps/desktop`：桌面应用（Vue 渲染层 + Tauri Rust）
- `apps/desktop/src-tauri`：Rust 工具执行与 IPC 入口
- `packages/*`：工具包封装（部分为薄封装/桩）
- `resources/manuals`：离线文档资源
- `scripts`：构建脚本
- `process.md`：复杂任务经验沉淀

## 3. 常用命令

- 安装依赖：`pnpm install`
- 开发：`pnpm dev`
- 类型检查：`pnpm typecheck`
- 渲染层构建：`pnpm --filter @lazycat/desktop build:web`
- 全量构建：`pnpm build`
- 单测：`pnpm test`
- E2E：`pnpm test:e2e`
- Windows 预检：`pnpm build:win:precheck`
- Windows 打包（NSIS 安装包）：`pnpm build:win`
- Windows 打包（NSIS，同上）：`pnpm build:portable`
- 绿色免安装包：详见 CLAUDE.md "Windows 打包方式总览" 章节

### Windows 打包方式总览

根据是否内嵌 WebView2 运行时，共四种打包方式：

| 方式 | 含 WebView2 | 产物 | 离线可用 | 适用场景 |
|------|:-----------:|------|:--------:|----------|
| NSIS（轻量） | 否 | `.exe` ~19MB | 否 | 目标机有 WebView2 或可联网 |
| NSIS（离线） | 是 | `.exe` ~218MB | 是 | 离线 Win10 部署（需 fixedRuntime 配置） |
| 绿色包（轻量） | 否 | `.zip` ~30MB | 否 | 目标机有 WebView2 |
| 绿色包（离线） | 是 | `.zip` ~290MB | 是 | 离线环境解压即用 |

**WebView2 离线原理**：`tauri.conf.json` 无需修改。`main.rs` 启动时扫描 exe 同级目录的 `Microsoft.WebView2.FixedVersionRuntime.*` 目录，设置 `WEBVIEW2_BROWSER_EXECUTABLE_FOLDER` 环境变量，`WebView2Loader.dll` 据此使用本地运行时。

**离线模式前置**：需在 `src-tauri/WebView2/` 下放置解压后的 WebView2 Fixed Runtime（从微软 CAB 文件用 7z 提取，~604MB，已 gitignore）。

**构建命令**：`pnpm build:portable`（等价于 `pnpm build:web && tauri build --bundles nsis`）。必须用 `tauri build` 而非 `cargo build --release`，后者不嵌入前端资源会导致白屏。

**绿色包**：构建后手动 `7z a -tzip` 打包 `target/release/` 下的 `lazycat-desktop.exe`、`lazycat_lib.dll`、`manuals/`、`regex-library/`、`hotkey-library/`（离线模式还需包含 WebView2 运行时目录）。

**devtools**：`Cargo.toml` 中 tauri 已启用 `devtools` feature，release 模式右键可打开开发者工具。

**Monaco Editor**：已从 CDN 改为 Vite ESM 本地打包（`src/utils/monaco-setup.ts`），离线环境可用。

**注意**：`offlineInstaller` 模式在某些环境下安装失败（错误码 `-2147219700`），不推荐。完整操作步骤见 CLAUDE.md。

## 4. 代理执行规则

- 不要自动启动 UI 或 dev server。
- 仅在用户明确要求时执行 `pnpm dev` 或任何会拉起界面的命令。
- 优先小步、可验证改动，避免无关重构。
- 涉及 UI 文案或模板结构修改后，必须做构建验证。
- 未经用户要求，不做破坏性命令（例如强制回滚/重置）。
- 较大变动（跨多文件、新增功能、架构调整）在验证通过后应及时提交，避免改动长期堆积导致回滚困难或提交范围模糊。
- 网页抓取策略：`WebFetch` 失败时（网络限制、域名验证不通过等），应主动 fallback 到 Playwright（`browser_navigate` + `browser_snapshot`）。Playwright 作为真实浏览器，对国内网站和 SPA 页面的兼容性更好。

## 5. 复杂任务流程

- 开始前先查看 `process.md` 是否已有同类经验。
- 复杂任务（跨 3+ 文件、功能新增、架构调整）完成后，将过程记录到 `process.md`。
- 当 `process.md` 某经验使用次数 >= 3，应提炼并固化到规范文件（`CLAUDE.md` / `AGENTS.md`）。

建议记录模板：

```md
## YYYY-MM-DD: 标题
**场景**:
**问题**:
**解决**:
**关键点**:
**涉及文件**:
**使用次数**: 0
```

## 6. 编码与字符集（高优先级）

- 源码文件（`*.vue`、`*.ts`、`*.css`、`*.md`）必须为 UTF-8（BOM 可有可无）。
- PowerShell 写文件显式指定 UTF-8：
  - `Set-Content -Encoding UTF8 ...`
  - `Out-File -Encoding utf8 ...`
- 含中文文件避免整文件级大替换；优先按块精确修改。
- 若出现乱码，先修复语法结构，再修复显示文本。
- 文案默认中文，技术术语（JSON/SQL/JWT 等）可保留英文。
- 若补丁工具报错 `stream did not contain valid UTF-8`，先将目标文件转为 UTF-8，再继续修改。

UI 文本改动后至少执行：

1. `pnpm --filter @lazycat/desktop typecheck`
2. `pnpm --filter @lazycat/desktop build:web`

## 7. 前后端调用链路

- 前端入口：`apps/desktop/src/bridge/tauri.ts` 的 `invokeToolByChannel`
- 通道映射：`tool:<domain>:<action>` -> `{ domain, action }`
- Rust 分发：`tool_execute` -> `apps/desktop/src-tauri/src/tools/mod.rs` 各域 `execute`

新增工具能力时：

1. Rust 端实现对应 domain/action
2. 前端 `CHANNEL_MAP` 增加映射
3. 面板组件接入并做错误态/加载态处理

## 8. 数据与路径约定

- 默认数据目录：`%USERPROFILE%\.lazycat`
- 指针文件：`%USERPROFILE%\.lazycat\config.json`（固定位置）
- 数据库：`<数据目录>\lazycat.sqlite`
- Hosts 备份：`<数据目录>\hosts-backups`
- 自定义数据目录不可达时，需回退默认目录而非崩溃。
- 数据目录迁移时复制 `lazycat.sqlite` 与 `hosts-backups`，旧目录保留不自动删除。
- 目标目录若已存在 `lazycat.sqlite`，迁移应拒绝，避免覆盖用户数据。

## 9. 手册架构与扩展（新增）

- 离线手册采用"每个手册独立本地 HTTP 端口"方式，避免 VitePress 绝对路径资源冲突。
- 手册列表来源于 Rust 端 `manuals:list` 的已注册项，新增手册需同步更新注册表。
- 新增手册优先"源码构建产物复制"方案；无法源码构建（如需外部 token）时可使用 Puppeteer 抓取作为兜底。
- 修改手册加载逻辑后，必须同时验证开发态与打包态路径解析。

新增手册**必须同时修改两处**（缺一不可）：

1. 准备手册静态产物（优先构建产物，兜底 Puppeteer 抓取）。
2. 复制到 `resources/manuals/<id>/`。
3. **`apps/desktop/src-tauri/src/tools/manuals.rs`** 的 `known` 数组中注册 `(id, name, entry_path)`。
4. **`apps/desktop/src/App.vue`** 的 `sidebarItems` 离线手册分组中增加 `{ id: "manual-<id>", name, desc }`。
   - 前端 id 格式必须为 `manual-<id>`，`ManualPanel.vue` 会自动去掉前缀与后端匹配。
5. 构建并人工验证 iframe 可加载、站内链接可跳转。

**Puppeteer 抓取要点（MDN 类型）**：
- SPA 路由无扩展名的路径（如 `/zh-CN/docs/Web/JavaScript/Reference/Array`）必须保存为 `<path>/index.html`，直接保存为同名文件会导致子路径 `ENOTDIR`。
- 使用系统 Edge：`C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe`。
- ESM 脚本中用 `createRequire(import.meta.url)` 导入 CJS 的 puppeteer。
- Windows 不支持路径含 `*` 的文件名（约 5 个 MDN 特殊页面），可忽略。
- 抓取脚本模板：`scripts/scrape-mdn-js.mjs`（可参考复用）。

**HTML 注入机制（离线噪音清除）**：
- HTTP 服务器在 `handle_manual_request` 中对所有 HTML 响应在 `</head>` 前注入 CSS + JS。
- 位置：`apps/desktop/src-tauri/src/main.rs` → `INJECT` 常量。
- CSS 隐藏已知噪音元素，JS 兜底扫描 `fixed`/`sticky` 定位弹窗（DOMContentLoaded + 1s + 3s 三次）。
- **新增手册有离线噪音时**，只需在 `INJECT` 的 CSS 选择器列表末尾追加对应 class/id，无需改其他文件。

## 10. settings 通道约定（新增）

- settings 相关改动优先复用现有通道：
  - `tool:settings:get` / `set` / `get-all`
  - `tool:settings:export` / `import`
  - `tool:settings:export-to-file` / `import-from-file`
  - `tool:settings:get-data-dir` / `set-data-dir` / `reset-data-dir`
- 导出/导入优先使用 Tauri 原生文件对话框能力，不使用浏览器下载或 `<input type="file">` 的替代实现。

## 11. 已知边界

- `pnpm build` 依赖 Rust 工具链（`cargo`、`rustc`）及系统依赖。
- Windows 上 vendored OpenSSL 需要 Perl 环境。
- 若修改打包、资源路径或手册加载逻辑，需同时验证 dev/production 路径解析。

## 12. Cron 工具约定（新增）

- 默认语法标准：Spring 6 字段（`秒 分 时 日 月 周`）。
- 输入兼容：允许 5 字段输入，解析时自动补前导秒 `0`。
- 不支持 7 字段 year（遇到 7 字段应明确报错，不做隐式兼容）。
- 前端优先调用：
  - `tool:cron:normalize`
  - `tool:cron:describe`
  - `tool:cron:preview-v2`
- 保持旧通道可用（`preview` / `parse`），仅作为兼容保留，新增功能优先走 v2。

## 13. 大体量资源变更约定（新增）

- 涉及 `resources/manuals/**` 大量新增/更新（>100 文件）时，提交前必须与用户确认提交范围：
  1. 仅功能代码
  2. 功能代码 + 单个资源目录
  3. 全量改动
- 这类提交建议使用明确提交信息，如：
  - `feat: ... and sync manuals resources`
- 提交前执行一次 `git status --short`，确认不会误带无关目录。

## 14. Windows 构建异常处理（新增）

- 若 `pnpm --filter @lazycat/desktop build:web` 出现 `spawn EPERM`：
  - 先重试一次同命令。
  - 若仍失败，可申请提升权限后重试；不得跳过构建验证。

## 15. 提交与质量门槛

提交建议使用约定式前缀：

- `feat:`
- `fix:`
- `docs:`
- `chore:`
- `test:`

提交信息使用中文描述，例如：`feat(launcher): 添加分组管理和使用次数排序`

推送前建议检查：

1. `pnpm typecheck`
2. `pnpm --filter @lazycat/desktop build:web`
3. `pnpm test`
4. `pnpm test:e2e`

## 16. 功能地图（项目现状）

以下为当前前端工具分组（以 `apps/desktop/src/App.vue` 为准）：

- 常驻一级：`formatter`、`snippets`、`calc-draft`、`regex`、`diff`、`markdown`
- 编解码：`base64`、`url`、`md5`、`hash`、`qr`
- 加密与安全：`rsa`、`aes`、`jwt`、`uuid`、`password`、`bcrypt`
- 数据转换：`json-process`、`json-schema`、`csv-json`、`java-bean-js`、`mybatis-helper`、`base-converter`、`color`、`escape-unescape`、`text-process`、`naming-case`、`config-convert`、`sql-entity`
- 网络与系统：`network`、`dns`、`hosts`、`ports`、`env`、`nginx-helper`、`hotkey`、`http-status`、`chmod-calc`
- 文件与媒体：`split-merge`、`pdf`、`image`
- 时间工具：`timestamp`、`cron`、`date-calc`
- 离线手册：`manual-vue3`、`manual-element-plus`、`manual-mdn-js`

新增/调整工具时，必须同步以下三个来源：

1. `apps/desktop/src/App.vue`（侧边栏分组与文案）
2. `apps/desktop/src/tool-registry.ts`（工具 ID -> 面板组件）
3. `apps/desktop/src/bridge/tauri.ts`（`CHANNEL_MAP`）

## 17. 关键状态流（前端）

- 主界面与代码片段工作区双视图：`viewMode = main | snippet-workspace`。
- 标签页体系：`useTabs` 管理打开、切换、关闭、左右批量关闭。
- 收藏与首页热度：`useFavorites` 管理收藏、点击历史、近 30 天高频工具。
- 菜单显隐：`useMenuVisibility` 维护 deny-list；分组在过滤后可能被自动提升为一级菜单。
- 设置入口：`SettingsPanel` 负责主题、快捷键、菜单显隐等，配置通过 settings IPC 持久化。

涉及以上能力改动时，优先复用现有 composables，避免在组件内重复造状态逻辑。

## 18. Rust 工具域（后端现状）

`apps/desktop/src-tauri/src/tools/mod.rs` 当前域包括：

- `encode`、`convert`、`text`、`time`、`gen`、`regex`、`cron`
- `crypto`、`format`、`network`、`dns`、`env`、`port`
- `file`、`image`、`hosts`、`manuals`、`settings`
- `hotkey`、`jwt`、`schema`、`mybatis`、`nginx`、`snippets`、`pdf`

新增域时至少完成：

1. 在 `tools/mod.rs` 声明模块并接入 `execute_tool` 分发
2. 前端 `CHANNEL_MAP` 增加映射
3. 对应面板增加错误态和加载态

## 19. 添加新工具的标准流程

每个新工具需改动以下文件（如需后端 Rust 支持）：

1. **`apps/desktop/src/App.vue`** -- `sidebarItems` 数组中注册 tool/group entry
2. **`apps/desktop/src/tool-registry.ts`** -- `toolRegistry` 注册异步组件
3. **`apps/desktop/src/components/XxxPanel.vue`** -- 新建面板组件
4. **`apps/desktop/src/bridge/tauri.ts`** -- `CHANNEL_MAP` 添加 IPC 通道（如需后端）
5. **`apps/desktop/src-tauri/src/tools/`** -- 新建 Rust 模块 + 在 `mod.rs` 注册（如需后端）

纯前端工具仅需步骤 1-3。

## 20. 近期新增工具架构备注

### PDF 工具（`pdf` 域）

- Rust 后端模块：`tools/pdf.rs`，使用 `lopdf` crate（`Cargo.toml` 中 `lopdf = "0.34"`）
- 三个 action：
  - `info` -- 提取元数据（页数、文件大小、PDF 版本、页面尺寸、纸张类型、标题/作者/创建日期等）
  - `split` -- 按页码范围拆分为多个独立 PDF 文件，输出到指定目录；页码范围留空时逐页拆分
  - `merge` -- 合并多个 PDF 为一个文件
- IPC 通道：`tool:pdf:info` / `tool:pdf:split` / `tool:pdf:merge`
- 前端面板：`PdfPanel.vue`，三个 Tab（信息/拆分/合并）
- lopdf API 注意事项：
  - `doc.version` 是 `String` 类型，不是 tuple
  - `Object::as_reference()` 返回 `Result`，需 `.ok()` 转为 `Option` 再链式调用

### SQL 转实体类（`convert` 域 `sql_to_entity` action）

- 在现有 `tools/convert.rs` 中新增 action，无需新增 Rust 模块
- 支持 6 种语言：Java（Lombok @Data）、TypeScript、Go、Python、Kotlin、C#
- Java 实体使用 `@Data` 注解 + 自动收集 import（`lombok.Data`、`java.time.*`、`java.math.BigDecimal`）
- IPC 通道：`tool:convert:sql-to-entity`
- 前端面板：`SqlEntityPanel.vue`

### 正则可视化（纯前端）

- 无需后端，集成在现有 `RegexPanel.vue` 中
- 前端依赖：`regexp-tree`（正则 AST 解析）+ `railroad-diagrams`（铁路图 SVG 渲染）
- 转换逻辑：`apps/desktop/src/utils/regex-to-railroad.ts`
- 类型声明：`apps/desktop/src/types/railroad-diagrams.d.ts`
- 500ms 防抖自动渲染，支持缩放（0.25x - 3x）

## 21. 手册与资源维护清单

- 新增手册后，必须在 `manuals.rs` 的 `known` 列表注册，否则前端不会显示入口。
- 必须同时在 `App.vue` 的 `sidebarItems` 中添加 `manual-<id>` 条目，否则侧边栏不显示。
- 手册首页路径必须可直接被本地 HTTP 服务访问（建议验证 200 + 资源加载）。
- 若新增手册文件量大，提交前先与用户确认是否合并到同一提交。
- 变更 `resources/manuals` 后建议验证：
  1. `manuals:list` 是否返回目标手册
  2. 前端 `manual-<id>` 是否能打开
  3. 站内跳转和静态资源是否正常
- **HTML 注入选择器维护**：`apps/desktop/src-tauri/src/main.rs` → `handle_manual_request` → `INJECT` 常量。
  当前隐藏的元素（截至 2026-02-21）：
  - MDN：`.navigation__popup`、`.menu__panel` 系列、`.page-layout__footer`、`.page-layout__banner`、`mdn-placement-*`、`.bb-banner`、`#bb-banner`、`.spsr-container`、`.content-section.article-footer`
  - Vue 3：`.preference-tooltip`
  - 通用：`.notification-bar`、`.mdn-cta`、`.top-banner`、`.pong-box`、`.article-actions-container`

## 22. Windows 构建脚本约定（补充）

`scripts/build-tauri-win.ps1` 目前会执行：

1. 校验 `cargo`、`perl`
2. 检测 `VsDevCmd.bat` 与 `kernel32.lib`
3. 先构建渲染层：`pnpm --filter @lazycat/desktop build:web`
4. 再在 VS 开发者环境执行：`pnpm --filter @lazycat/desktop build:tauri`

涉及 Windows 打包问题时，优先使用 `pnpm build:win:precheck` 复现与排查。
