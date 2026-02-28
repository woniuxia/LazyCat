# CLAUDE.md

本文件为 Claude 或其他编码代理提供项目上下文和协作规范。

> **双文件同步约束**：本文件（`CLAUDE.md`）与 `AGENTS.md` 共同维护项目规范。
> 更新本文件的任何章节时，必须同步检查并更新 `AGENTS.md` 中的对应内容，保持两者一致。

## 核心规则

- 当用户要求实现功能时，立即开始写代码。不要花整个会话探索代码库和写计划，除非用户明确要求制定计划。如果需要上下文，快速收集后立即进入实现。
- 当用户提出问题（如"怎么安装 X？"、"Y 可行吗？"），直接回答问题。不要为信息类问题启动代码库探索或进入计划模式。
- 所有 CDN 依赖和外部资源（字体、Monaco Editor 等）必须本地打包。这是桌面应用，运行时绝不能依赖 CDN 加载。
- 构建便携包时，始终使用 portable/zip 目标，除非用户明确要求，否则不要生成 NSIS 安装包。
- UI 设计默认使用干净的浅色/白色主题，与现有页面风格保持一致。除非用户明确要求，否则不要使用深色、赛博朋克或花哨的设计风格。

## 项目概览

- 名称: Lazycat (懒猫)
- 类型: 离线桌面开发者工具箱
- 主要平台: Windows
- 技术栈: Tauri 2 + Vue 3 + TypeScript + Rust
- 终端: PowerShell（不支持 `&&`，使用 `;` 分隔命令）
- 本机脚本: 已安装 Python 与 Node.js，可用于小型脚本和临时排障
- Windows 特性: 控制台输出 GBK 编码（非 UTF-8）；运行中的 .exe 持有文件锁须先终止才能重新构建

## 仓库结构

```
apps/desktop/                    Tauri 桌面应用
  src-tauri/                     Rust 工具执行与 IPC 入口
    src/tools/                   30 个 Rust 工具域模块 + mod.rs 分发器 + helpers.rs
  src/components/                57 个 Vue 面板组件
  src/composables/               状态管理 composables
  src/bridge/tauri.ts            前后端 IPC 通道映射（157 条通道，27 个域）
  src/tool-registry.ts           工具 ID -> 异步组件注册
packages/formatters/             JSON/XML/HTML/Java/SQL 格式化（Prettier standalone，唯一实际使用的 package）
resources/manuals/               离线手册（Vue 3、Element Plus、MDN JavaScript）
resources/regex-library/         内置正则模板
resources/hotkey-library/        快捷键库资源
scripts/                         构建与工具脚本
  build-tauri-win.ps1            Windows NSIS 打包脚本
  release-all-win.ps1            四类包一键构建与 GitHub 发布脚本
  scrape-mdn-js.mjs             MDN JavaScript 抓取脚本
```

## 本地命令

| 命令 | 说明 |
|------|------|
| `pnpm install` | 安装依赖 |
| `pnpm dev` | 开发模式 |
| `pnpm typecheck` | 类型检查 |
| `pnpm build` | 全量构建 |
| `pnpm build:win:precheck` | Windows 构建预检 |
| `pnpm test` | 单元测试 |
| `pnpm test:e2e` | E2E 测试 |
| `pnpm build:win` | Windows 打包（NSIS 安装包） |
| `pnpm build:portable` | Windows 打包（NSIS，同上） |

## 架构说明

### 前后端调用链路

- Vue 调用 `bridge/tauri.ts` 的 `invokeToolByChannel`
- 通道字符串（如 `tool:encode:base64-encode`）通过 `CHANNEL_MAP`（157 条通道，27 个域）映射为 `{domain, action}`
- Tauri 命令 `tool_execute` 在 Rust 端通过 `tools/mod.rs` 的 `execute_tool` 分发到各域模块

### 前端动态组件

- 未使用 vue-router；`App.vue` 通过 `activeTool` 决定当前面板
- 工具面板通过 `tool-registry.ts` 的 `defineAsyncComponent` 动态加载，`<component :is="currentComponent">` 渲染

### 前端 Composables

| Composable | 说明 |
|------------|------|
| `useToolInvoke` | IPC 调用包装（loading/error 状态管理） |
| `useTabs` | 标签页管理（打开/切换/关闭/左右批量关闭，Ctrl+数字切换） |
| `useSettings` | 设置读写持久化 |
| `useFavorites` | 收藏夹与点击历史（近 30 天高频工具推荐） |
| `useMenuVisibility` | 侧边栏工具显隐（deny-list 模型，分组剩 1 项自动提升为一级，0 项整组隐藏） |
| `useClipboardSuggestion` | 剪贴板内容智能检测与工具推荐 |

### 格式化与持久化

- XML/HTML/Java/SQL 格式化在 Rust 端为**直通模式**；实际格式化由 `@lazycat/formatters`（Prettier standalone）完成
- Prettier 必须使用 `prettier/standalone` + 显式解析器插件，否则运行时会失败
- 所有状态（收藏、历史、草稿、主题、快捷键、菜单显隐）存储在 SQLite `user_settings` 表

### Rust 后端

30 个工具域模块位于 `apps/desktop/src-tauri/src/tools/`，由 `mod.rs` 统一分发。辅助模块 `helpers.rs` 提供路径/DB/schema migration。

关键 gotcha:
- **Cron**: 默认 Spring 6 字段（`秒 分 时 日 月 周`），5 字段自动补前导秒 `0`，7 字段拒绝。优先使用 `normalize`/`describe`/`preview-v2` 通道
- **Hosts**: 激活需要管理员权限写入 `C:\Windows\System32\drivers\etc\hosts`，覆写前自动备份
- **capture**: 条件编译，需 `capture` feature（依赖 `pcap`/`etherparse`/`libc`），默认不启用
- **Monaco Editor**: 已从 CDN 改为 Vite ESM 本地打包（`src/utils/monaco-setup.ts`），离线环境可用

## 添加新工具的标准流程

每个新工具需改动以下文件（如需后端 Rust 支持）：

1. **`apps/desktop/src/App.vue`** -- `sidebarItems` 数组中注册 tool/group entry
2. **`apps/desktop/src/tool-registry.ts`** -- `toolRegistry` 注册异步组件
3. **`apps/desktop/src/components/XxxPanel.vue`** -- 新建面板组件
4. **`apps/desktop/src/bridge/tauri.ts`** -- `CHANNEL_MAP` 添加 IPC 通道（如需后端）
5. **`apps/desktop/src-tauri/src/tools/`** -- 新建 Rust 模块 + 在 `mod.rs` 注册（如需后端）

纯前端工具（如正则可视化）仅需步骤 1-3。

工具分组见 `App.vue` 的 `sidebarItems`。新增/调整工具时，必须同步以上三个前端来源（App.vue、tool-registry.ts、tauri.ts）。

## 编码与中文安全

### 硬性要求

- 前端源码文件（`*.vue`、`*.ts`、`*.css`、`*.md`）统一使用 **UTF-8**，禁止 ANSI/GBK/UTF-16
- PowerShell 写文件必须显式指定 UTF-8：`Set-Content -Encoding UTF8` / `Out-File -Encoding utf8`
- 含中文文件禁止整文件级大替换；优先小范围、可定位的精确修改
- 如遇 `apply_patch` 报错 `stream did not contain valid UTF-8`，先将目标文件转为 UTF-8 再继续修改
- 菜单/导航/按钮等用户可见文案默认使用中文（通用技术词如 JSON/SQL/JWT 除外）

### 乱码修复优先级

1. 先修复语法结构（引号闭合、标签闭合、字符串闭合）
2. 再修复显示文本
3. 最后执行构建验证：`pnpm --filter @lazycat/desktop typecheck` + `build:web`

## 数据管理

### 运行时路径

- 指针配置: `%USERPROFILE%\.lazycat\config.json`（固定位置，记录自定义数据目录）
- 数据库文件: `<数据目录>\lazycat.sqlite`（默认 `%USERPROFILE%\.lazycat\lazycat.sqlite`）
- Hosts 备份: `<数据目录>\hosts-backups`

### 数据目录

`helpers.rs` 提供 `get_base_dir()`（固定 `~/.lazycat`）、`get_config_path()`、`get_data_dir()`（读 `config.json` 中 `data_dir` 字段，不可达时回退默认）三层路径函数。

- 容错：自定义路径不可达时静默回退默认目录，不崩溃
- 迁移：复制 `lazycat.sqlite` + `hosts-backups/` 到新目录，旧目录保留不删除
- 安全：目标目录已存在 `lazycat.sqlite` 时拒绝迁移，避免覆盖
- 导出/导入：使用 Tauri 原生文件对话框（`@tauri-apps/plugin-dialog`），JSON 格式，支持 merge/overwrite 两种模式
- settings 域提供 10 个 IPC 通道（get/set/get-all/export/import/export-to-file/import-from-file/get-data-dir/set-data-dir/reset-data-dir）

## 离线手册架构

### 工作原理

- Rust 在 `setup` 阶段为每个手册启动独立本地 HTTP 文件服务器（`TcpListener::bind("127.0.0.1:0")` 自动分配端口）
- 每个手册独立端口的原因：VitePress 构建产物使用绝对路径资源引用（`/assets/...`），共享端口会导致 404
- 前端 `ManualPanel.vue` 用 `<iframe>` 内嵌展示

### 已集成的手册

| 手册 ID | 名称 | 来源 |
|---------|------|------|
| `vue3` | Vue 3 开发手册 | 源码构建（`vuejs-translations/docs-zh-cn`） |
| `element-plus` | Element Plus 组件库 | Puppeteer 抓取 |
| `mdn-js` | MDN JavaScript 手册 | Puppeteer 抓取（872 文件，~72MB） |

### 路径解析

- 打包后：`resource_dir()` + `manuals/`（由 `tauri.conf.json` 的 `bundle.resources` 配置，路径相对于 `src-tauri/`）
- 开发模式：fallback 到 `CARGO_MANIFEST_DIR/../../../resources/manuals`（项目根目录）

### 添加新手册

1. 获取中文文档静态产物（优先源码构建，无法构建时用 Puppeteer 抓取，参考 `scripts/scrape-mdn-js.mjs`）
2. 复制到 `resources/manuals/<id>/`
3. **两处必改**：
   - `manuals.rs` 的 `known` 数组注册 `(id, name, entry_path)`
   - `App.vue` 的 `sidebarItems` 离线手册分组增加 `{ id: "manual-<id>", name, desc }`
   - 前端 id 格式为 `manual-<id>`，`ManualPanel.vue` 自动去掉前缀与后端匹配
4. 验证 `pnpm dev` 能正确加载

### HTML 注入机制

HTTP 服务器在返回 HTML 时，于 `</head>` 前注入 CSS + JS 隐藏离线噪音元素（位置：`main.rs` 的 `INJECT` 常量）。新增手册需隐藏元素时，直接在 CSS 选择器列表中追加即可。

### 关键注意事项

- 不要用 `website-scraper`/`wget --mirror` 抓取 VitePress 站点（SPA 空壳）；优先源码构建
- SPA 路由无扩展名路径必须保存为 `<path>/index.html`，否则子路径 `ENOTDIR`
- 注册新手册必须同时改 `manuals.rs` + `App.vue`，缺一不可

## Windows 构建与打包

### 四类打包方式

LazyCat 依赖 WebView2 运行时（Win11 自带，Win10 不一定有）：

| 方式 | 含 WebView2 | 产物 | 体积 | 离线可用 | 适用场景 |
|------|:-----------:|------|------|:--------:|----------|
| NSIS 安装包（轻量） | 否 | `.exe` 安装包 | ~19 MB | 否 | 目标机已有 WebView2 或可联网 |
| NSIS 安装包（离线） | 是 | `.exe` 安装包 | ~218 MB | 是 | 离线 Win10 部署 |
| 绿色免安装包（轻量） | 否 | `.zip` | ~30 MB | 否 | 目标机已有 WebView2 |
| 绿色免安装包（离线） | 是 | `.zip` | ~290 MB | 是 | 离线环境解压即用 |

### WebView2 离线原理

`tauri.conf.json` 无需修改。`main.rs` 启动时扫描 exe 同级 `Microsoft.WebView2.FixedVersionRuntime.*` 目录，设置 `WEBVIEW2_BROWSER_EXECUTABLE_FOLDER` 环境变量，必须在 `tauri::Builder::default()` 之前执行。同一二进制文件有运行时目录则用本地版本，没有则用系统安装版本。

离线模式需预先在 `src-tauri/WebView2/` 下放置解压后的 WebView2 Fixed Runtime（微软 CAB 文件用 7z 提取，~604MB，已 gitignore）。

### 构建命令与产物

```bash
pnpm build:portable   # 等价于 pnpm build:web && tauri build --bundles nsis
```

- NSIS 安装包：`target/release/bundle/nsis/Lazycat_0.1.0_x64-setup.exe`
- Release 二进制：`target/release/lazycat-desktop.exe` + `lazycat_lib.dll`

### 绿色免安装包

构建后手动 7z 打包 `target/release/` 下的文件：

```bash
cd apps/desktop/src-tauri/target/release
7z a -tzip Lazycat_0.1.0_x64_portable.zip \
  lazycat-desktop.exe lazycat_lib.dll manuals/ regex-library/ hotkey-library/
# 离线版额外加入：../../WebView2/Microsoft.WebView2.FixedVersionRuntime.*.x64/
```

### 关键警告

- **必须用 `tauri build`**，不能用 `cargo build --release`（后者不嵌入前端资源，运行白屏）
- 构建时若 exe 被占用（os error 5），需先关闭运行中的程序
- Git 的 `usr/bin/link.exe` 可能遮蔽 MSVC 链接器，`release-all-win.ps1` 已处理（构建前过滤 PATH）
- 构建机器需要 Rust 工具链 + Perl（OpenSSL 编译）+ 7z（绿色包打包）
- `offlineInstaller` 模式在某些环境下安装失败（错误码 `-2147219700`），不推荐
- `Cargo.toml` 中 tauri 已启用 `devtools` feature，release 模式右键可打开开发者工具

## 代理协作规则

- 不要自动启动应用/开发服务器。仅在用户明确要求时才运行 `pnpm dev`
- 完成复杂任务（3+ 文件、非简单调试、架构变更）后，将流程总结写入 `process.md`
- 开始复杂任务前，先检查 `process.md` 是否有相关经验
- 当 `process.md` 中某条经验使用次数 >= 3 时，固化到 `CLAUDE.md`
- 较大变动在确认有效后应及时提交，避免改动堆积
- 网页抓取策略：`WebFetch` 失败时应 fallback 到 Playwright（`browser_navigate` + `browser_snapshot`）
- process.md 记录格式见该文件头部说明
- 涉及 `resources/manuals/**` 大量变更（>100 文件）时，提交前与用户确认范围

## 提交规范与构建检查

### 提交格式

- 约定式提交：`feat:`、`fix:`、`docs:`、`chore:`、`test:`
- 提交信息使用中文描述，例如：`feat(launcher): 添加分组管理和使用次数排序`

### 推送前检查

1. `pnpm typecheck`
2. `pnpm --filter @lazycat/desktop build:web`
3. `pnpm test`
4. `pnpm test:e2e`
5. 如需打包：`pnpm build:win`

### 构建异常处理

- `build:web` 出现 `spawn EPERM`：先重试，仍失败则提升权限重试，不得跳过构建验证
- Rust 链接 `link.exe` 报错：检查 PATH 中 `Git\usr\bin`（含 GNU link.exe），过滤后重试
