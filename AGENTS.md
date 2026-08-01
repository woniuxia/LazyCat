# AGENTS.md

本文件是 LazyCat 的高频执行入口。用户明确指令优先；领域实现与排障细节查 [process.md](process.md) 和 [docs/experience/](docs/experience/README.md)。

## 1. 核心规则

### 权限与方案

- 先读当前任务相关文件和真实上下文，再做最小闭环改动；不顺手重构、不加无关功能。
- 用户只要求调研、规划、评审、检查、解释或复盘时，默认保持只读并交付分析，不进入实现、提交、打包或其他写操作；同一请求同时要求规划和执行时，先确认会改变结果的方案与边界，再实现。
- 规划或实现存在会影响用户行为、验收口径、数据模型、架构边界、范围、兼容性、依赖或 UI 方向的歧义时，不自行补全。先说明已知事实和具体歧义，再给出 2～3 个方案、关键取舍和明确推荐；仅不影响产品行为和方向的机械细节可按现有惯例处理，无法判断时一律先确认。
- 对“优化体验、改善交互、美化界面”等范围不明确的请求，先简要说明拟改范围；涉及视觉方向、快捷键或明显改变用户行为时，先确认后再修改。明确的 bug 修复和具体小改动直接执行。
- 已有用户确认的规格或计划时，核对最新指令、当前代码和仓库状态；没有冲突就按其执行，不重复规划、不扩大范围。“继续”或“确定”只延续最近确认范围；跨对话先以规格、提交、`git status`、相关 diff 和验证结果重建进度。发现矛盾、缺项或前提失效时，列明差异后确认。
- 专用功能升级为通用架构，或新增持久化、后台任务、并发、恢复等复杂能力时，暂停并列出当前 MVP、后续阶段、非目标、影响范围和最低验证。确认只对已明确范围有效；实现中出现新歧义、关键前提失效或重要边界变化时再次确认，未进入验收的历史、恢复、并行和自动触发等增强不顺带实现。

### 实现与安全

- 涉及并发、后台执行、事件、轮询或事务时，编码前明确唯一事实源、状态所有权、生命周期、事务边界、失败释放路径和恢复语义。
- 同一边界连续暴露两个并发、事务或状态一致性问题时，停止叠加局部修复，回到不变量和时序设计检查根因。
- 错误必须显式返回并保留必要上下文；不得吞异常、伪造成功、用静默兜底或隐藏默认值掩盖问题。仅明确标注为非关键的 best-effort 行为允许降级，并必须留下可诊断信息。
- 运行时不得依赖公网 CDN；字体、Monaco 和其他外部资源必须本地打包。
- UI 默认干净的浅色/白色风格；整体视觉方向改变先确认用户。
- 源码统一 UTF-8。PowerShell 写文件显式使用 UTF-8；含中文文件优先精确补丁，不整文件盲替换。
- 保留 dirty worktree 中与当前任务无关的改动。目标文件已有未提交改动时先读 diff；直接冲突则停下确认。
- 不自动启动产品 UI 或 `pnpm dev`；只有用户明确要求才启动。
- 默认在当前工作区和分支修改，尽可能不使用 worktree。只有当前工作区无法安全完成、并行修改存在明确冲突或流程强制隔离时，才可说明必要原因、影响范围、替代方案和清理方式并向用户申请；未经明确同意不得创建。
- 破坏性操作（删除文件、覆盖数据、迁移数据库）、批量修改、大范围资源变更或外部副作用前，先确认目标、影响和回退方式。
- 复杂任务开始前查相关经验。每完成关键步骤就判断核心请求是否已有足够证据完成，满足后立即停止；出现新的跨模块不变量、重复根因或可复用决策时再评估经验沉淀。

## 2. 项目速览

- LazyCat 是 Windows 优先的离线桌面开发者工具箱，技术栈为 Tauri 2 + Vue 3 + TypeScript + Rust。
- 新功能优先解决个人离线开发中的高频问题并控制长期维护成本。若功能明显重复成熟专业工具，或需要持续跟进复杂协议和生态，规划前先说明使用场景、差异价值、维护边界和退出条件，向用户确认是否继续。
- 使用 PowerShell；命令串联用 `;`，不要用 `&&`。
- 核心目录：`apps/desktop/src`（Vue）、`apps/desktop/src-tauri`（Rust）、`apps/desktop/src/bridge/tauri.ts`（IPC）、`apps/desktop/src/tool-registry.ts`（工具注册）、`resources/`（离线资源）、`scripts/`（构建发布）。
- 常见类型、纯函数和对应测试优先放在 `src/types/`、`src/utils/`；组件只负责状态编排和 UI 绑定。

## 3. 开工闸门

1. 判断授权边界：只读分析、实施修改，还是提交、打包、发布等外部操作；未获授权不越界。
2. 判断任务领域：普通功能、UI/样式、文档/规范，还是高风险操作。
3. 读取当前文件、相关测试和对应经验；新增工具先查架构经验。
4. 检查需求、验收标准、方案方向和实现边界是否明确；存在会影响结果的歧义时，先列出选项和取舍并向用户确认，不进入编码。
5. 检查联动：前端/bridge/Rust/类型/测试、Element Plus 双主题文件和 Agent 规则入口。
6. 明确最低验证：定向测试优先，其后类型检查、构建或最小冒烟。
7. 需要删除、迁移、批量覆盖或明显扩大视觉方向时，先向用户确认。

## 4. 常用命令与打包决策

| 命令                                       | 用途                                             |
| ------------------------------------------ | ------------------------------------------------ |
| `pnpm test`                                | 单元测试                                         |
| `pnpm typecheck`                           | 全工作区类型检查                                 |
| `pnpm --filter @lazycat/desktop build:web` | 渲染层构建                                       |
| `pnpm build`                               | 全量构建                                         |
| `pnpm package:win`                         | 默认本地打包：lite portable zip + SHA256，不上传 |
| `pnpm build:win`                           | 明确要求 NSIS 安装包                             |
| `pnpm release:win -- -Tag vX.Y.Z`          | 正式 GitHub Release，默认 lite portable          |
| `pnpm release:all:win -- -Tag vX.Y.Z`      | 正式 GitHub Release，完整四包                    |

- 用户只说“打包”或“本地打包”时，必须执行 `pnpm package:win`。
- 便携包优先交付 zip。需要完整四包才用 `release:all:win`。
- 桌面应用交付必须通过 Tauri build 嵌入前端资源，不能用裸 `cargo build --release` 代替。

## 5. 经验索引

| 场景                                                   | 经验文件                                                                     |
| ------------------------------------------------------ | ---------------------------------------------------------------------------- |
| 产品边界、架构、IPC、Tauri、SQLite 迁移、结构治理      | [architecture.md](docs/experience/architecture.md)                           |
| UI、响应式、滚动、scoped CSS、Element Plus、Teleport   | [ui-and-styling.md](docs/experience/ui-and-styling.md)                       |
| 数据字典                                               | [data-dictionary.md](docs/experience/data-dictionary.md)                     |
| Todo、提醒、日期时间                                   | [todo.md](docs/experience/todo.md)                                           |
| PM、甘特、思源                                         | [pm.md](docs/experience/pm.md)                                               |
| Spotlight、快捷键、浏览器身份                          | [spotlight-and-launcher.md](docs/experience/spotlight-and-launcher.md)       |
| API Mock、访问链路、Cron                               | [api-and-network-tools.md](docs/experience/api-and-network-tools.md)         |
| 请求转发                                               | [request-forward.md](docs/experience/request-forward.md)                     |
| 上线包                                                 | [release-package.md](docs/experience/release-package.md)                     |
| Windows 构建与发布                                     | [windows-build-and-release.md](docs/experience/windows-build-and-release.md) |
| Vault、Inbox                                           | [vault-and-inbox.md](docs/experience/vault-and-inbox.md)                     |
| 离线手册、资源、本地预览                               | [manuals-and-resources.md](docs/experience/manuals-and-resources.md)         |
| Agent 协作、只读边界、续作、交接、主动提交、规范、验证 | [agent-workflow.md](docs/experience/agent-workflow.md)                       |
| JSON 树、Base64、片段、番茄钟等                        | [other-tools.md](docs/experience/other-tools.md)                             |

完整索引和经验维护规则见 [process.md](process.md)。

## 6. 验证与提交

- 验证必须基于实际运行证据；未运行的命令、静态阅读和构建通过不得表述为行为已验证。
- 文档改动：检查链接、索引、Agent 规则入口和关键规则完整性。
- 功能改动：优先相关测试，再 `pnpm typecheck`；影响渲染层时补 `pnpm --filter @lazycat/desktop build:web`。
- Bug 修复必须先复现根因，并在最低稳定层补回归测试；无法自动化时说明原因并执行可复现的最小冒烟。用户反馈问题仍存在后，旧诊断和验证结论立即失效。
- 功能验收按任务覆盖适用的用户路径，包括创建、编辑、保存、重开、删除、失败反馈、持久化恢复和旧数据升级；单元测试、类型检查和构建不能替代行为验收。
- UI 改动：额外检查浅色主题、空态、交互态、弹窗态、Teleport 作用域、常用与窄窗口、内容溢出与滚动区，以及条件切换时的布局稳定性。需要启动产品 UI 才能验证时先向用户申请；未做运行时视觉验证时必须明确说明，不得宣称视觉验收完成。
- 数据字典改动：按 [data-dictionary.md](docs/experience/data-dictionary.md) 的定向测试执行。
- 实施任务完成一个可独立验收、可回退的阶段且相关验证通过后，或进入高风险后续工作前需要保存稳定基线时，主动提交当前任务改动，无需等待用户再次要求。失败、未验证或不可运行的中间态不得提交。
- 提交前运行 `git diff --check`，默认只暂存当前任务文件。用户明确要求“提交所有改动”时，先检查全部状态和差异，排除秘密、临时文件和无关生成物后再整体暂存；提交不自动包含推送、合并或打包。提交信息使用 `feat:`、`fix:`、`docs:`、`chore:`、`test:` 等约定式中文前缀。
- 主动提交只创建新的本地 commit；未经用户明确要求，不执行 amend、rebase、squash、force push 或其他历史改写操作。提交失败不得跳过钩子或降低校验。
