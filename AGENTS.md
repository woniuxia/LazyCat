# AGENTS.md

本文件定义在本仓库内工作的编码代理（Codex/Claude/其他）统一执行规范。
如与用户明确指令冲突，以用户指令为准。

> **双文件同步约束**：本文件（`AGENTS.md`）与 `CLAUDE.md` 共同维护项目规范。
> 更新本文件的任何章节时，必须同步检查并更新 `CLAUDE.md` 中的对应内容，保持两者一致。

## 1. 核心规则

- 当用户要求实现功能时，立即开始写代码。不要花整个会话探索代码库，除非用户明确要求制定计划。
- 当用户提出问题，直接回答。不要为信息类问题启动代码库探索或进入计划模式。
- 所有 CDN 依赖和外部资源必须本地打包。运行时绝不能依赖 CDN 加载。
- 构建便携包时使用 portable/zip 目标，除非用户明确要求 NSIS。
- UI 默认浅色/白色主题，除非用户明确要求其他风格。
- Windows 平台注意：控制台 GBK 编码，运行中 .exe 持有文件锁须先终止才能重新构建。

## 2. 项目概览

- 项目：Lazycat（懒猫）-- 离线桌面开发者工具箱
- 平台：Windows 优先
- 技术栈：Tauri 2 + Vue 3 + TypeScript + Rust
- 终端：PowerShell（命令串联使用 `;`，不支持 `&&`）
- 本机脚本：已安装 Python 与 Node.js

## 3. 仓库结构

```
apps/desktop/                    Tauri 桌面应用
  src-tauri/src/tools/           30 个 Rust 工具域模块 + mod.rs + helpers.rs
  src/components/                57 个 Vue 面板组件
  src/composables/               状态管理 composables
  src/bridge/tauri.ts            IPC 通道映射（157 条通道，27 个域）
  src/tool-registry.ts           工具 ID -> 异步组件注册
packages/formatters/             Prettier standalone（唯一实际使用的 package）
resources/manuals/               离线手册（Vue 3、Element Plus、MDN JavaScript）
resources/regex-library/         内置正则模板
resources/hotkey-library/        快捷键库资源
scripts/                         构建脚本（build-tauri-win.ps1、release-all-win.ps1、scrape-mdn-js.mjs）
```

## 4. 命令与质量门槛

| 命令 | 说明 |
|------|------|
| `pnpm install` | 安装依赖 |
| `pnpm dev` | 开发模式 |
| `pnpm typecheck` | 类型检查 |
| `pnpm --filter @lazycat/desktop build:web` | 渲染层构建 |
| `pnpm build` | 全量构建 |
| `pnpm test` | 单元测试 |
| `pnpm test:e2e` | E2E 测试 |
| `pnpm build:win:precheck` | Windows 构建预检 |
| `pnpm build:win` / `pnpm build:portable` | Windows 打包（NSIS） |

### 提交规范

- 约定式前缀：`feat:`、`fix:`、`docs:`、`chore:`、`test:`
- 提交信息使用中文描述，例如：`feat(launcher): 添加分组管理和使用次数排序`

### 推送前检查

1. `pnpm typecheck`
2. `pnpm --filter @lazycat/desktop build:web`
3. `pnpm test`
4. `pnpm test:e2e`

## 5. 架构要点

### IPC 链路

- 前端入口：`bridge/tauri.ts` 的 `invokeToolByChannel`
- 通道映射：`tool:<domain>:<action>` -> `{ domain, action }`（157 条通道，27 个域）
- Rust 分发：`tool_execute` -> `tools/mod.rs` 各域 `execute`

### 前端

- 未使用 vue-router；`App.vue` 通过 `activeTool` + `tool-registry.ts` 的 `defineAsyncComponent` 动态加载面板
- 工具面板通过 `<component :is="currentComponent">` 渲染

| Composable | 说明 |
|------------|------|
| `useToolInvoke` | IPC 调用包装（loading/error 状态管理） |
| `useTabs` | 标签页管理（打开/切换/关闭/批量关闭，Ctrl+数字切换） |
| `useSettings` | 设置读写持久化 |
| `useFavorites` | 收藏夹与点击历史（近 30 天高频推荐） |
| `useMenuVisibility` | 侧边栏显隐（deny-list，分组剩 1 项自动提升，0 项整组隐藏） |
| `useClipboardSuggestion` | 剪贴板智能检测与工具推荐 |

### 后端

30 个 Rust 工具域模块位于 `src-tauri/src/tools/`，由 `mod.rs` 统一分发。辅助模块 `helpers.rs` 提供路径/DB/schema migration。

格式化：Rust 端 XML/HTML/Java/SQL 为直通模式，质量取决于 `@lazycat/formatters`（Prettier standalone + 显式解析器插件）。

所有状态存储在 SQLite `user_settings` 表。

## 6. 添加新工具

1. **`App.vue`** -- `sidebarItems` 注册 tool/group entry
2. **`tool-registry.ts`** -- 注册异步组件
3. **`components/XxxPanel.vue`** -- 新建面板
4. **`bridge/tauri.ts`** -- `CHANNEL_MAP` 添加通道（如需后端）
5. **`src-tauri/src/tools/`** -- Rust 模块 + `mod.rs` 注册（如需后端）

纯前端工具仅需步骤 1-3。工具分组见 `App.vue` 的 `sidebarItems`。

## 7. 编码与中文

- 源码文件统一 UTF-8，禁止 ANSI/GBK/UTF-16
- PowerShell 写文件显式指定 UTF-8（`Set-Content -Encoding UTF8`）
- 含中文文件避免整文件级大替换，优先按块精确修改
- 若补丁工具报 `stream did not contain valid UTF-8`，先转 UTF-8 再修改
- 文案默认中文，技术术语（JSON/SQL/JWT 等）可保留英文
- Prettier 必须使用 `prettier/standalone` + 显式解析器插件
- 乱码修复顺序：语法结构 -> 显示文本 -> 构建验证（`typecheck` + `build:web`）

## 8. 数据路径

- 指针配置: `%USERPROFILE%\.lazycat\config.json`（固定位置）
- 数据库: `<数据目录>\lazycat.sqlite`（默认 `%USERPROFILE%\.lazycat\`）
- Hosts 备份: `<数据目录>\hosts-backups`
- 自定义数据目录不可达时静默回退默认目录，不崩溃
- 迁移复制 `lazycat.sqlite` + `hosts-backups/`，旧目录保留；目标已存在 db 文件则拒绝

## 9. 离线手册

- 每个手册独立本地 HTTP 端口（VitePress 绝对路径资源需要独立根目录）
- 前端 `ManualPanel.vue` 用 `<iframe>` 内嵌展示

| 手册 ID | 名称 | 来源 |
|---------|------|------|
| `vue3` | Vue 3 开发手册 | 源码构建 |
| `element-plus` | Element Plus 组件库 | Puppeteer 抓取 |
| `mdn-js` | MDN JavaScript 手册 | Puppeteer 抓取 |

### 新增手册

1. 获取中文静态产物（优先源码构建，兜底 Puppeteer，参考 `scripts/scrape-mdn-js.mjs`）
2. 复制到 `resources/manuals/<id>/`
3. **两处必改**：`manuals.rs` 的 `known` 数组 + `App.vue` 的 `sidebarItems`（前端 id 格式 `manual-<id>`）
4. 噪音元素：在 `main.rs` 的 `INJECT` CSS 选择器追加即可
5. 路径解析：打包用 `resource_dir()` + `manuals/`，开发态 fallback 到项目根 `resources/manuals`

## 10. 构建与打包

| 方式 | 含 WebView2 | 产物 | 离线可用 | 适用场景 |
|------|:-----------:|------|:--------:|----------|
| NSIS（轻量） | 否 | `.exe` ~19MB | 否 | 目标机有 WebView2 或可联网 |
| NSIS（离线） | 是 | `.exe` ~218MB | 是 | 离线 Win10 部署 |
| 绿色包（轻量） | 否 | `.zip` ~30MB | 否 | 目标机有 WebView2 |
| 绿色包（离线） | 是 | `.zip` ~290MB | 是 | 离线环境解压即用 |

**WebView2 离线原理**：`main.rs` 启动时扫描 exe 同级 `Microsoft.WebView2.FixedVersionRuntime.*` 目录，设置环境变量使用本地运行时。`tauri.conf.json` 无需修改。

**构建命令**：`pnpm build:portable`。必须用 `tauri build` 而非 `cargo build --release`（后者不嵌入前端资源会导致白屏）。

**绿色包**：构建后手动 `7z a -tzip` 打包 `target/release/` 下的二进制 + 资源目录。

**关键警告**：
- Git `usr/bin/link.exe` 可能遮蔽 MSVC 链接器，`release-all-win.ps1` 已处理
- `build:web` 出现 `spawn EPERM` 先重试，仍失败提升权限重试
- 构建需要 Rust 工具链 + Perl（OpenSSL）+ 7z

## 11. 代理规则与 Gotcha

### 执行规则

- 不自动启动 UI 或 dev server，仅用户要求时执行 `pnpm dev`
- 优先小步、可验证改动，避免无关重构
- 较大变动验证通过后及时提交
- 未经用户要求不做破坏性命令
- 网页抓取 `WebFetch` 失败时 fallback 到 Playwright
- 涉及 `resources/manuals/**` 大量变更（>100 文件）时，提交前与用户确认范围

### process.md

- 开始复杂任务前查看是否有同类经验
- 复杂任务（3+ 文件）完成后记录到 `process.md`
- 使用次数 >= 3 的经验固化到规范文件

### 工具域 Gotcha

- **Cron**: Spring 6 字段默认，5 字段补秒，7 字段拒绝，优先 v2 通道
- **Hosts**: 需管理员权限，覆写前自动备份
- **capture**: 需 `capture` feature 条件编译，默认不启用
- **Prettier**: 必须用 `prettier/standalone` + 显式解析器插件
