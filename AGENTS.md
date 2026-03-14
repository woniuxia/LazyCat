# AGENTS.md

本文件为在本仓库内工作的编码代理（Codex / Claude / 其他）提供统一项目上下文与协作规范。
如与用户明确指令冲突，以用户指令为准。

> **双文件同步约束**：本文件（`AGENTS.md`）与 `CLAUDE.md` 共同维护项目规范。
> 更新本文件的任何章节时，必须同步检查并更新另一份文件对应内容，保持两者一致。

## 核心规则

- 当用户要求实现功能时，先做最小必要上下文确认，然后立即进入实现；除非用户明确要求，否则不要长时间停留在计划阶段。
- 当用户提出信息类问题时，直接回答，不要无意义进入代码改动流程。
- 所有 CDN 依赖和外部资源（字体、Monaco 等）必须本地打包；运行时不得依赖公网 CDN。
- UI 默认保持干净浅色/白色风格，除非用户明确要求其他视觉方向。
- Windows 环境注意：控制台可能为 GBK 编码；运行中的 `.exe` 会持有文件锁，重建前需先结束进程。
- 便携包/绿色包交付优先 `zip` 形态；当前 `pnpm build:portable` 底层仍执行 NSIS 构建（历史命名），需要再走脚本或手动封装为 zip。

## 项目概览

- 名称：Lazycat（懒猫）
- 类型：离线桌面开发者工具箱
- 平台：Windows 优先
- 技术栈：Tauri 2 + Vue 3 + TypeScript + Rust
- 终端：PowerShell（命令串联使用 `;`，不要使用 `&&`）
- 本机能力：已安装 Python 与 Node.js

## 仓库结构

```text
apps/desktop/                    Tauri 桌面应用
  src-tauri/                     Rust 后端
    src/tools/                   28 个工具域模块（含 capture）+ mod.rs + helpers.rs
  src/components/                57 个 Vue 面板组件
  src/composables/               状态管理 composables
  src/bridge/tauri.ts            IPC 通道映射（157 条通道，27 个域）
  src/tool-registry.ts           工具 ID -> 异步组件注册
packages/formatters/             Prettier standalone（唯一实际使用 package）
resources/manuals/               离线手册（Vue 3、Element Plus、MDN JavaScript）
resources/regex-library/         内置正则模板
resources/hotkey-library/        快捷键库资源
scripts/                         构建脚本（build-tauri-win.ps1、release-all-win.ps1、scrape-mdn-js.mjs）
```

## 本地命令

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
| `pnpm build:portable` | 当前与 `build:win` 同底层（NSIS）；便携 zip 需额外封装 |

## 架构说明

### 前后端调用链路（常规工具）

- 前端入口：`bridge/tauri.ts` 的 `invokeToolByChannel`
- 通道映射：`tool:<domain>:<action>` -> `{ domain, action }`（157 条通道，27 个域）
- Rust 分发：`tool_execute` -> `tools/mod.rs` 的 `execute_tool`（27 域）

### 特殊链路（capture）

- `capture` 模块存在于 `src-tauri/src/tools/capture.rs`，但不走 `CHANNEL_MAP` / `tool_execute`。
- 抓包相关能力通过 Tauri 独立 command 暴露（如 `start_capture`、`stop_capture`、`export_pcap`），入口位于 `src-tauri/src/main.rs`。

### 前端组织

- 未使用 `vue-router`；`App.vue` 通过 `activeTool` + `tool-registry.ts` 动态加载面板。
- 工具面板通过 `<component :is="currentComponent">` 渲染。

### 关键 Composables

| Composable | 说明 |
|------------|------|
| `useToolInvoke` | IPC 调用包装（loading/error 状态管理） |
| `useTabs` | 标签页管理（打开/切换/关闭/左右批量关闭，Ctrl+数字切换） |
| `useSettings` | 设置读写持久化 |
| `useFavorites` | 收藏夹与点击历史（近 30 天高频推荐） |
| `useMenuVisibility` | 侧边栏显隐（deny-list，分组剩 1 项自动提升，0 项整组隐藏） |
| `useClipboardSuggestion` | 剪贴板智能检测与工具推荐 |

### 持久化与格式化

- XML/HTML/Java/SQL 格式化在 Rust 端为直通模式，核心依赖 `@lazycat/formatters`（Prettier standalone + 显式解析器插件）。
- `user_settings` 主要存储用户偏好与配置项。
- 业务数据按域存储在独立表中（如 `hosts_profiles`、`snippet_*`、`vault_*`、`launcher_entries`）。

## 添加新工具的标准流程

### 常规工具（走 channel 分发）

1. `apps/desktop/src/App.vue`：在 `sidebarItems` 注册入口
2. `apps/desktop/src/tool-registry.ts`：注册异步组件
3. `apps/desktop/src/components/XxxPanel.vue`：新增面板
4. `apps/desktop/src/bridge/tauri.ts`：`CHANNEL_MAP` 增加通道（如需后端）
5. `apps/desktop/src-tauri/src/tools/`：新增 Rust 模块，并在 `mod.rs` 注册（如需后端）

### 非 channel 工具（如 capture）

- 除上述前端注册外，需要在 `src-tauri/src/main.rs` 增加/注册 Tauri command。
- 前端通过 `@tauri-apps/api/core` 的 `invoke` 直接调用 command。

## Element Plus 样式覆盖注意事项

项目对 Element Plus 的样式覆盖分布在多个文件中，存在层叠优先级陷阱：

- **加载顺序**：`element-overrides.css`（第 9 行）先于 `theme-light.css`（第 11 行），见 `src/styles/index.css`。
- **`theme-light.css` 使用 `html[data-theme="light"]` 前缀**，比 `element-overrides.css` 中同类选择器特异度更高。
- 因此，修改 Element Plus 组件样式变量时，**必须同时检查并更新两个文件**：
  1. `element-overrides.css` — 暗色主题 / 基础覆盖
  2. `theme-light.css` — 浅色主题覆盖（带 `html[data-theme="light"]` 前缀，特异度更高）
- 典型案例：为 `.el-button--primary` 添加 `.is-text` 变体时，仅在 `element-overrides.css` 添加会被 `theme-light.css` 的高特异度规则覆盖，导致浅色主题下样式不生效。

## 编码与中文规范

- 源码统一 UTF-8，禁止 ANSI/GBK/UTF-16。
- PowerShell 写文件必须显式 `-Encoding UTF8`。
- 含中文文件避免整文件替换，优先按块精确修改。
- 如补丁报 `stream did not contain valid UTF-8`，先转 UTF-8 再改。
- 文案默认中文，JSON/SQL/JWT 等术语可保留英文。
- 乱码修复顺序：语法结构 -> 显示文本 -> 构建验证（`typecheck` + `build:web`）。

## 数据路径与目录策略

- 指针配置：`%USERPROFILE%\\.lazycat\\config.json`（固定位置）
- 数据库：`<数据目录>\\lazycat.sqlite`（默认 `%USERPROFILE%\\.lazycat\\lazycat.sqlite`）
- Hosts 备份：`<数据目录>\\hosts-backups`
- 自定义数据目录不可达时静默回退默认目录，不崩溃。
- 迁移时复制 `lazycat.sqlite` 与 `hosts-backups/`；目标目录若已存在 db 文件则拒绝覆盖。

## 离线手册

- 每个手册启动独立本地 HTTP 端口（VitePress 绝对路径资源要求独立根路径）。
- 前端 `ManualPanel.vue` 通过 `<iframe>` 内嵌展示。

| 手册 ID | 名称 | 来源 |
|---------|------|------|
| `vue3` | Vue 3 开发手册 | 源码构建 |
| `element-plus` | Element Plus 组件库 | Puppeteer 抓取 |
| `mdn-js` | MDN JavaScript 手册 | Puppeteer 抓取 |

### 新增手册

1. 获取中文静态产物（优先源码构建，兜底 Puppeteer，参考 `scripts/scrape-mdn-js.mjs`）
2. 复制到 `resources/manuals/<id>/`
3. 同时修改：`manuals.rs` 的 `known` + `App.vue` 的 `sidebarItems`（前端 id：`manual-<id>`）
4. 噪音元素可在 `main.rs` 的 `INJECT` CSS 选择器补充
5. 路径解析：打包用 `resource_dir()/manuals`，开发态 fallback 到项目根 `resources/manuals`

## Windows 构建与打包

### 产物类型

| 方式 | 产物 | 离线可用 | 适用场景 |
|------|------|:--------:|----------|
| NSIS 轻量安装包 | `.exe` | 否 | 目标机已有 WebView2 或可联网 |
| NSIS 离线安装包 | `.exe` | 是 | 离线 Win10 部署 |
| 绿色轻量包 | `.zip` | 否 | 目标机已有 WebView2 |
| 绿色离线包 | `.zip` | 是 | 离线环境解压即用 |

### 关键说明

- 必须使用 `tauri build`，不要用 `cargo build --release`（后者不嵌入前端资源，会白屏）。
- `pnpm build:portable` 当前等价于 NSIS 构建流程；绿色 zip 需通过 `release-all-win.ps1` 或手动 `7z` 封装。
- `main.rs` 启动时会扫描 exe 同级 `Microsoft.WebView2.FixedVersionRuntime.*`，存在则自动切换到本地 WebView2。
- `release-all-win.ps1` 已处理 Git `usr/bin/link.exe` 遮蔽 MSVC 链接器问题。
- `build:web` 出现 `spawn EPERM` 时先重试，仍失败再提升权限重试。

## 代理协作规则

- 不自动启动 UI 或 dev server，仅在用户明确要求时执行 `pnpm dev`。
- 优先小步、可验证改动，避免无关重构。
- 较大改动验证通过后及时提交，避免改动堆积。
- 未经用户明确要求不执行破坏性命令。
- 网页抓取 `WebFetch` 失败时 fallback 到 Playwright。
- 涉及 `resources/manuals/**` 大量变更（>100 文件）时，提交前必须与用户确认范围。

## process.md 经验沉淀

- 开始复杂任务前先查 `process.md` 是否有同类经验。
- 复杂任务（3+ 文件）完成后记录到 `process.md`。
- 使用次数 >= 3 的经验固化到规范文件。

## 提交规范与推送前检查

### 提交规范

- 约定式前缀：`feat:`、`fix:`、`docs:`、`chore:`、`test:`
- 提交信息使用中文，例如：`feat(launcher): 添加分组管理和使用次数排序`

### 推送前检查

1. `pnpm typecheck`
2. `pnpm --filter @lazycat/desktop build:web`
3. `pnpm test`
4. `pnpm test:e2e`
