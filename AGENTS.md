# AGENTS.md

本文件定义在本仓库内工作的编码代理（Codex/Claude/其他）统一执行规范。
如与用户明确指令冲突，以用户指令为准。

## 1. 项目概览

- 项目：Lazycat（懒猫）
- 类型：离线桌面开发者工具箱
- 平台：Windows 优先
- 技术栈：Tauri 2 + Vue 3 + TypeScript + Rust
- 终端环境：PowerShell（命令串联使用 `;`，不要依赖 `&&`）

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
- Windows 打包：`pnpm build:win`
- 便携版打包：`pnpm build:portable`

## 4. 代理执行规则

- 不要自动启动 UI 或 dev server。
- 仅在用户明确要求时执行 `pnpm dev` 或任何会拉起界面的命令。
- 优先小步、可验证改动，避免无关重构。
- 涉及 UI 文案或模板结构修改后，必须做构建验证。
- 未经用户要求，不做破坏性命令（例如强制回滚/重置）。
- 较大变动（跨多文件、新增功能、架构调整）在验证通过后应及时提交，避免改动长期堆积导致回滚困难或提交范围模糊。

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

- 离线手册采用“每个手册独立本地 HTTP 端口”方式，避免 VitePress 绝对路径资源冲突。
- 手册列表来源于 Rust 端 `manuals:list` 的已注册项，新增手册需同步更新注册表。
- 新增手册优先“源码构建产物复制”方案；无法源码构建（如需外部 token）时可使用 Puppeteer 抓取作为兜底。
- 修改手册加载逻辑后，必须同时验证开发态与打包态路径解析。

新增手册标准步骤：

1. 准备手册静态产物（优先构建产物，兜底 Puppeteer 抓取）。
2. 复制到 `resources/manuals/<id>/`。
3. 在 Rust 端 `manuals:list` 中注册 `<id>/<name>/<entry>`。
4. 构建并人工验证 iframe 可加载、站内链接可跳转。

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

推送前建议检查：

1. `pnpm typecheck`
2. `pnpm --filter @lazycat/desktop build:web`
3. `pnpm test`
4. `pnpm test:e2e`
