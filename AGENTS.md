# AGENTS.md

本文件是 LazyCat 的高频执行入口。用户明确指令优先；领域实现与排障细节查 [process.md](process.md) 和 [docs/experience/](docs/experience/README.md)。

## 1. 核心规则

- 先读当前任务相关文件和真实上下文，再做最小闭环改动；不顺手重构、不加无关功能。
- 规划或实现存在模糊空间时，不自行补全需求或替用户决定方向。只要不同理解会影响用户行为、验收口径、数据模型、架构边界、实现范围、兼容性、依赖选择或 UI 方向，就先暂停并向用户确认。
- 发起方案确认时，先说明当前已知事实和具体歧义，再给出 2～3 个可选方案、关键取舍和明确推荐；只集中询问会改变方案或结果的问题，确认后再继续实现。
- 仅限不影响产品行为和方案方向的低风险机械细节，可依据现有代码惯例自行决定；无法判断是否属于低风险时，一律先向用户确认，不自作主张。
- 当专用功能升级为通用架构，或任务新增持久化、后台任务、并发、恢复等复杂能力时，暂停实现并重新列出当前 MVP、后续阶段、非目标、影响范围和最低验证，只集中确认一次。
- 方案确认只对已明确的范围有效；实现中发现新歧义、关键前提不成立，或用户行为、数据模型、架构边界、破坏性操作、外部副作用发生变化时，必须再次暂停确认。
- 通用能力先交付当前验收所需的最小闭环；历史、恢复、并行、自动触发等增强能力未进入本阶段验收标准时，不顺带实现。
- 涉及并发、后台执行、事件、轮询或事务时，编码前明确唯一事实源、状态所有权、生命周期、事务边界、失败释放路径和恢复语义。
- 同一边界连续暴露两个并发、事务或状态一致性问题时，停止叠加局部修复，回到不变量和时序设计检查根因。
- 验证分层执行：局部改动跑定向验证，阶段完成跑相关测试和类型检查，最终交付或合并前跑完整验证；共享契约或构建配置变化时可提前补完整验证。
- 运行时不得依赖公网 CDN；字体、Monaco 和其他外部资源必须本地打包。
- UI 默认干净的浅色/白色风格；整体视觉方向改变先确认用户。
- 源码统一 UTF-8。PowerShell 写文件显式使用 UTF-8；含中文文件优先精确补丁，不整文件盲替换。
- 保留 dirty worktree 中与当前任务无关的改动。目标文件已有未提交改动时先读 diff；直接冲突则停下确认。
- 不自动启动产品 UI 或 `pnpm dev`；只有用户明确要求才启动。
- 默认直接在当前工作区和当前分支修改，尽可能不使用 worktree；不得因习惯、方便或一般性的隔离需求自行创建 worktree。
- 只有当前工作区无法安全完成任务、并行修改存在明确冲突风险，或任务流程强制要求隔离时，才可提出使用 worktree。创建前必须说明必要原因、影响范围、替代方案和后续清理方式，并获得用户明确同意；未经确认不得创建。
- 用户同意创建 worktree 后，初始化只运行 `pnpm install --frozen-lockfile --prefer-offline`；不得运行 `pnpm build`、Tauri build 或安装包构建。按任务范围执行定向验证。
- 本机已安装 `sccache` 时，通过用户级 `RUSTC_WRAPPER=sccache` 复用 Rust 编译缓存；各 worktree 保持独立 `target`，不把 `sccache` 设为项目硬依赖。
- 破坏性操作（删除文件、覆盖数据、迁移数据库）、批量修改、大范围资源变更或外部副作用前，先确认目标、影响和回退方式。
- Windows 下运行中的 `.exe` 会持有文件锁，重建前先结束对应进程。
- 复杂任务开始前查经验；改动涉及 3+ 文件后评估是否沉淀新经验。

## 2. 项目速览

- LazyCat 是 Windows 优先的离线桌面开发者工具箱，技术栈为 Tauri 2 + Vue 3 + TypeScript + Rust。
- 使用 PowerShell；命令串联用 `;`，不要用 `&&`。
- 核心目录：`apps/desktop/src`（Vue）、`apps/desktop/src-tauri`（Rust）、`apps/desktop/src/bridge/tauri.ts`（IPC）、`apps/desktop/src/tool-registry.ts`（工具注册）、`resources/`（离线资源）、`scripts/`（构建发布）。
- 常见类型、纯函数和对应测试优先放在 `src/types/`、`src/utils/`；组件只负责状态编排和 UI 绑定。

## 3. 开工闸门

1. 判断任务：普通功能、UI/样式、文档/规范，还是高风险操作。
2. 读取当前文件、相关测试和对应经验；新增工具先查架构经验。
3. 检查需求、验收标准、方案方向和实现边界是否明确；存在会影响结果的歧义时，先列出选项和取舍并向用户确认，不进入编码。
4. 检查联动：前端/bridge/Rust/类型/测试、Element Plus 双主题文件和 Agent 规则入口。
5. 明确最低验证：定向测试优先，其后类型检查、构建或最小冒烟。
6. 需要删除、迁移、批量覆盖或明显扩大视觉方向时，先向用户确认。

## 4. 常用命令与打包决策

| 命令 | 用途 |
|------|------|
| `pnpm install --frozen-lockfile --prefer-offline` | worktree 依赖初始化，不执行构建 |
| `pnpm test` | 单元测试 |
| `pnpm typecheck` | 全工作区类型检查 |
| `pnpm --filter @lazycat/desktop build:web` | 渲染层构建 |
| `pnpm build` | 全量构建 |
| `pnpm package:win` | 默认本地打包：lite portable zip + SHA256，不上传 |
| `pnpm build:win` | 明确要求 NSIS 安装包 |
| `pnpm release:win -- -Tag vX.Y.Z` | 正式 GitHub Release，默认 lite portable |
| `pnpm release:all:win -- -Tag vX.Y.Z` | 正式 GitHub Release，完整四包 |

- 用户只说“打包”或“本地打包”时，必须执行 `pnpm package:win`。
- 便携包优先交付 zip。需要完整四包才用 `release:all:win`。
- 桌面应用交付必须通过 Tauri build 嵌入前端资源，不能用裸 `cargo build --release` 代替。

## 5. 经验索引

| 场景 | 经验文件 |
|------|----------|
| 架构、IPC、Tauri、结构治理 | [architecture.md](docs/experience/architecture.md) |
| UI、Element Plus、Teleport | [ui-and-styling.md](docs/experience/ui-and-styling.md) |
| 数据字典 | [data-dictionary.md](docs/experience/data-dictionary.md) |
| Todo、提醒、日期时间 | [todo.md](docs/experience/todo.md) |
| PM、甘特、思源 | [pm.md](docs/experience/pm.md) |
| Spotlight、快捷键、浏览器身份 | [spotlight-and-launcher.md](docs/experience/spotlight-and-launcher.md) |
| API Mock、访问链路、Cron | [api-and-network-tools.md](docs/experience/api-and-network-tools.md) |
| 请求转发 | [request-forward.md](docs/experience/request-forward.md) |
| 上线包 | [release-package.md](docs/experience/release-package.md) |
| Windows 构建与发布 | [windows-build-and-release.md](docs/experience/windows-build-and-release.md) |
| Vault、Inbox | [vault-and-inbox.md](docs/experience/vault-and-inbox.md) |
| 离线手册、资源、本地预览 | [manuals-and-resources.md](docs/experience/manuals-and-resources.md) |
| Agent 协作、规范、验证 | [agent-workflow.md](docs/experience/agent-workflow.md) |
| JSON 树、Base64、片段、番茄钟等 | [other-tools.md](docs/experience/other-tools.md) |

完整索引和经验维护规则见 [process.md](process.md)。

## 6. 验证与提交

- 文档改动：检查链接、索引、Agent 规则入口和关键规则完整性。
- 功能改动：优先相关测试，再 `pnpm typecheck`；影响渲染层时补 `pnpm --filter @lazycat/desktop build:web`。
- UI 改动：额外检查浅色主题、空态、交互态、弹窗态与 Teleport 作用域。
- 数据字典改动：按 [data-dictionary.md](docs/experience/data-dictionary.md) 的定向测试执行。
- 提交前运行 `git diff --check`，只暂存当前任务文件。提交信息使用 `feat:`、`fix:`、`docs:`、`chore:`、`test:` 等约定式中文前缀。
