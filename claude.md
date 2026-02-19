# CLAUDE.md

This file provides project context and collaboration conventions for Claude or other coding agents.

## Project

- Name: Lazycat (懒猫)
- Type: Offline desktop developer toolbox
- Primary platform: Windows
- Runtime: Tauri 2 + Vue 3 + TypeScript

## Monorepo Layout

- `apps/desktop`: Tauri desktop app (Rust commands + Vue renderer)
- `packages/core`: codec, text tools, conversion, regex, cron, generators
- `packages/crypto`: RSA/AES/DES crypto wrappers
- `packages/formatters`: JSON/XML/HTML/Java/SQL formatters
- `packages/network`: connectivity/runtime/port checks
- `packages/file-tools`: file split/merge utilities
- `packages/image-tools`: image conversion/resize/crop/compress
- `packages/db`: sqlite persistence
- `packages/ipc-contracts`: request/response contracts
- `resources/manuals`: offline manual snapshots
- `resources/regex-library`: built-in regex templates

## Local Commands

- Install: `pnpm install`
- Dev: `pnpm dev`
- Typecheck: `pnpm typecheck`
- Build: `pnpm build`
- Build (with Windows precheck): `pnpm build:win:precheck`
- Unit tests: `pnpm test`
- E2E tests: `pnpm test:e2e`
- Windows package (NSIS): `pnpm build:win`
- Windows package (portable alias): `pnpm build:portable`

## Agent Collaboration Rules

- Do not start the app/dev server automatically.
- Only run `pnpm dev` (or any command that launches the desktop UI) when the user explicitly asks to start it.

## Encoding & Garbled Text (Important)

- Recent issue: garbled Chinese text introduced template/script corruption in `apps/desktop/src/App.vue`:
  - broken quoted attributes (missing closing `"`),
  - broken button closing tags (e.g. `?/el-button>`),
  - broken string literals in `<script>` (unterminated quotes).
- Typical symptoms:
  - Vite/Vue parse errors such as:
    - `Attribute name cannot contain ...`
    - `Unquoted attribute value cannot contain ...`
    - `Unterminated string constant`
    - `Error parsing JavaScript expression`
- Required handling when touching UI text:
  - Preserve valid UTF-8 and avoid bulk replacements that may alter punctuation/quotes.
  - Prefer small, targeted edits.
  - If garbling is detected, fix structural correctness first (quotes/tags), then fix display text.
- Required verification after text edits in Vue files:
  1. `pnpm --filter @lazycat/desktop typecheck`
  2. `pnpm --filter @lazycat/desktop build:web`
- Extra note for formatter feature:
  - Prettier in renderer path must use standalone + explicit plugins (`prettier/standalone`, plus parser plugins), otherwise parser resolution may fail at runtime.

## Architecture Notes

- Frontend ↔ backend call path:
  - Vue calls `invokeToolByChannel` in `apps/desktop/src/bridge/tauri.ts`
  - Channel strings (e.g. `tool:encode:base64-encode`) are mapped to `{domain, action}` via `CHANNEL_MAP`
  - Tauri command `tool_execute` dispatches in Rust via a single `match (domain, action)` in `main.rs`
- Frontend routing: no vue-router; single `App.vue` uses `v-else-if` chains on `activeTool` ref to switch panels
- Extracted sub-components in `apps/desktop/src/components/`: `HomePanel`, `CalcDraftPanel`, `FormatterPanel`, `RegexPanel`, `HostsPanel`, `PortsPanel`, `MonacoPane`, `ManualPanel`, `EncodePanel`
- Formatter architecture: XML/HTML/Java/SQL formatting is **passthrough in Rust**; actual formatting is done by `@lazycat/formatters` (Prettier standalone) in the renderer
- Cron preview (`cron.preview`) is currently a **stub** — returns placeholder strings, not real next-fire times
- Hosts activate requires **administrator privileges** to write `C:\Windows\System32\drivers\etc\hosts`; auto-backs up original before overwrite
- Runtime data:
  - App data under `%USERPROFILE%\\.lazycat`
  - Hosts profiles in sqlite
  - Hosts backup directory managed by Rust side
- State persistence: favorites, tool click history, calc draft history stored in `localStorage`; hosts profiles in SQLite

## Important Runtime Paths

- DB file: `%USERPROFILE%\\.lazycat\\lazycat.sqlite`
- Hosts backups: `%USERPROFILE%\\.lazycat\\hosts-backups`

## Current Known Constraints

- `pnpm build` requires Rust toolchain (`cargo`, `rustc`) and platform prerequisites.
- On Windows, vendored OpenSSL requires `perl` (e.g. Strawberry Perl).
- All Rust tool logic lives in a single `apps/desktop/src-tauri/src/main.rs` (~990 lines); no module splitting yet.
- `packages/core`, `packages/crypto`, `packages/db`, `packages/file-tools`, `packages/image-tools`, `packages/network`, `packages/ipc-contracts` are currently stubs or thin wrappers — actual logic runs in Rust. Only `packages/formatters` is actively used by the renderer (Prettier standalone).
- Cron preview is a stub (returns placeholder strings). Real next-fire-time calculation not yet implemented.
- XML/HTML/Java/SQL formatters in Rust are passthrough; formatting quality depends on `@lazycat/formatters` (Prettier).
- Offline manuals are placeholder snapshots and can be replaced by full static docs.
- Hosts activate requires running the app as Administrator on Windows.

## Offline Manuals Architecture

### How It Works

- Rust 在 `setup` 阶段扫描 `resources/manuals/` 下的子目录，为每个手册启动独立的本地 HTTP 文件服务器（`TcpListener::bind("127.0.0.1:0")` 自动分配端口）
- 端口存储在全局 `MANUAL_SERVERS: OnceLock<HashMap<String, u16>>`
- `manuals:list` IPC 从全局 map 读取端口，返回 `http://127.0.0.1:{port}/guide/introduction.html` 格式的 URL
- 前端 `ManualPanel.vue` 用 `<iframe :src="url">` 内嵌展示，文档自带的搜索和导航在 iframe 内直接可用

### Why Local HTTP Server (Not Custom URI Scheme)

- Tauri 2 的 `register_uri_scheme_protocol` 在 iframe 中加载 HTML 有已知 bug（[tauri#12767](https://github.com/tauri-apps/tauri/issues/12767)），CSS/JS 资源无法正确加载
- 本地 HTTP 服务器方案最稳定，完全兼容 VitePress 文档的绝对路径资源引用（`/assets/...`）

### Why Per-Manual Separate Port

- VitePress 构建产物中所有资源路径是绝对路径（`/assets/style.xxx.css`），从 server 根目录解析
- 如果多个手册共享一个端口（如 `http://127.0.0.1:{port}/vue3/index.html`），`/assets/...` 会跳过 `vue3/` 前缀导致 404
- 每个手册独立端口，其目录即为 HTTP 根路径，绝对路径天然正确

### Dev vs Production Path Resolution

- 打包后：`app.path().resource_dir()` + `manuals/` （由 `tauri.conf.json` 的 `bundle.resources` 配置打包）
- 开发模式：`resource_dir()` 指向 `target/debug/`，文件不存在，fallback 到 `CARGO_MANIFEST_DIR/../../../resources/manuals`（项目根目录）
- `tauri.conf.json` 中 `bundle.resources` 路径相对于 `src-tauri/`，当前值 `"../../../resources/manuals/**/*"`

### Adding a New Manual (Step by Step)

1. **获取中文文档源码**（以 VitePress 文档为例）：
   ```bash
   git clone --depth=1 https://github.com/<org>/<docs-repo> /tmp/docs
   cd /tmp/docs && pnpm install && pnpm build
   ```
   - Vue 3 中文：`vuejs-translations/docs-zh-cn`，产物在 `.vitepress/dist/`
   - 注意：务必使用中文翻译仓库，而非英文原版

2. **复制构建产物**到 `resources/manuals/<id>/`：
   ```bash
   cp -r .vitepress/dist resources/manuals/<id>
   ```

3. **注册手册**（`main.rs` 的 `manuals:list` 分支）：
   ```rust
   let known = [
       ("vue3", "Vue 3 开发手册"),
       ("<id>", "<名称>"),  // 新增
   ];
   ```

4. **清理临时目录**，验证 `pnpm dev` 能正确加载

### Pitfalls to Avoid

- **不要用 `website-scraper` / `wget --mirror` 抓取 VitePress 站点** — VitePress 是 SPA，抓取到的是空壳 HTML，JS 渲染的内容不会被保存
- **必须从源码构建** — `git clone` + `pnpm build` 得到的才是完整的静态产物
- **注意 `bundle.resources` 路径** — 相对于 `apps/desktop/src-tauri/`，不是项目根目录

## Commit Conventions

- Conventional commits preferred:
  - `feat: ...`
  - `fix: ...`
  - `docs: ...`
  - `chore: ...`
  - `test: ...`
- Keep each commit focused by domain (ui/core/build/test).

## Pre-commit Gate

Run these before pushing:

1. `pnpm typecheck`
2. `pnpm --filter @lazycat/desktop build:web` (renderer)
3. `pnpm test`
4. `pnpm test:e2e`

If packaging is required:

5. `pnpm build:win`
