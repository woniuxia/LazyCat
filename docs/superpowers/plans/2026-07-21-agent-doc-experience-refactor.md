# Agent 规范与经验库精简重构实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将根规范精简为高频执行入口，把 `process.md` 的有效经验按领域迁入 `docs/experience/`，并删除或合并已失效、重复和冲突内容。

**Architecture:** 以当前工作区中的 `process.md` 为迁移输入，先建立逐条审计清单，再生成领域经验文件；`process.md` 退化为总索引，`AGENTS.md` 与 `CLAUDE.md` 保持同构并只引用经验库。当前代码、脚本和配置是有效性判断的最高证据，迁移审计确保每条原始经验都有 `kept`、`merged` 或 `removed` 结果。

**Tech Stack:** Markdown、PowerShell、ripgrep、Git

---

## 文件结构

- Create: `docs/experience/README.md` — 经验库说明、领域索引和逐条迁移审计。
- Create: `docs/experience/architecture.md` — 通用架构、IPC、Tauri 与结构治理经验。
- Create: `docs/experience/ui-and-styling.md` — UI、Element Plus、弹层与交互经验。
- Create: `docs/experience/data-dictionary.md` — 数据字典经验。
- Create: `docs/experience/todo.md` — Todo 与提醒经验。
- Create: `docs/experience/pm.md` — PM 与思源集成经验。
- Create: `docs/experience/spotlight-and-launcher.md` — Spotlight、快捷键与浏览器身份经验。
- Create: `docs/experience/api-and-network-tools.md` — API Mock、访问链路、Cron 与网络工具经验。
- Create: `docs/experience/request-forward.md` — 请求转发经验。
- Create: `docs/experience/release-package.md` — 上线包经验。
- Create: `docs/experience/windows-build-and-release.md` — Windows 构建与 GitHub Release 经验。
- Create: `docs/experience/vault-and-inbox.md` — Vault 与 Inbox 经验。
- Create: `docs/experience/manuals-and-resources.md` — 离线手册、资源与本地预览经验。
- Create: `docs/experience/agent-workflow.md` — Agent 文档与协作经验。
- Create: `docs/experience/other-tools.md` — 其他工具经验。
- Modify: `process.md` — 改为经验总索引、维护规则和新增经验模板。
- Modify: `AGENTS.md` — 精简为高频规则、开工闸门、命令、验证和经验索引。
- Modify: `CLAUDE.md` — 与 `AGENTS.md` 同步。
- Modify if still current: `docs/prompts/2026-07-17-access-path-diagnostics-planning-prompt.md` — 将经验入口改为领域索引。

### Task 1：冻结迁移输入并建立完整清单

**Files:**
- Modify: `docs/experience/README.md`
- Read: `process.md`

- [ ] **Step 1: 确认工作区边界**

运行：

```powershell
git status --short
git diff -- process.md
```

预期：业务代码的三个既有改动保持不动；`process.md` 顶部新增“上线包归档终态日志与目录快捷入口”，该记录必须进入本次迁移输入。

- [ ] **Step 2: 生成原始标题清单**

运行：

```powershell
$entries = Select-String -Path process.md -Pattern '^## \d{4}-\d{2}-\d{2}: '
$entries.Count
$entries.Line
```

预期：输出当前工作区的全部经验标题；数量应为 119（已提交的 118 条加当前未提交的 1 条）。若数量变化，以执行时的实际工作区数量为基线并在审计说明中注明。

- [ ] **Step 3: 创建迁移审计表**

在 `docs/experience/README.md` 中为每个标题建立一行：

```markdown
| 原日期 | 原标题 | 处理 | 目标 | 依据 |
|--------|--------|------|------|------|
| 2026-07-21 | 上线包归档终态日志与目录快捷入口 | kept | `release-package.md#...` | 当前功能仍存在 |
```

`处理` 只能是 `kept`、`merged`、`removed`；`merged` 必须指向保留条目，`removed` 必须写明失效、重复或缺少复用价值的具体原因。

- [ ] **Step 4: 校验审计表无漏项**

运行标题提取与表格标题提取，对比后预期没有只存在于一侧的标题。审计表完成前不修改 `process.md`。

### Task 2：核验过期、冲突与演进链路

**Files:**
- Modify: `docs/experience/README.md`
- Read: `package.json`
- Read: `apps/desktop/src/**`
- Read: `apps/desktop/src-tauri/**`
- Read: `scripts/**`

- [ ] **Step 1: 核验已明确可能过期的功能和命令**

使用 `rg` 检查抓包、提醒中心、旧 release 行为、旧 PM 筛选、旧 Todo 模型、旧 Spotlight 能力和旧构建命令是否仍存在。最低检查：

```powershell
rg -n "capture|抓包|reminder center|提醒中心|release:win|release:all:win|package:win|build:portable|pmViewRegistry|todo_items|spotlight" AGENTS.md CLAUDE.md package.json apps scripts
```

预期：以当前代码和 `package.json` 确认事实；不得仅凭旧经验标题判断。

- [ ] **Step 2: 合并清晰的演进链路**

至少审查以下主题组，保留当前结论并合并被替代方案：

- Windows 发布与默认 lite portable。
- Todo 的事项模型、提醒、日期时间和列表分区。
- PM 状态筛选从甘特专用到共享工具栏的演进。
- API Mock 运行态、Content-Type、格式化与 CORS。
- 访问链路诊断从适配器、契约到探测和报告的演进。
- 请求转发从三栏工作台到实时筛选、恢复动作和布局偏好的演进。
- 上线包从两阶段提交到并行目标、归档覆盖和终态日志的演进。

合并后的条目标题使用最新记录的日期和当前有效结论；旧标题在审计表中标为 `merged`。

- [ ] **Step 3: 删除明确失效或无复用价值的内容**

满足以下条件之一时标为 `removed`：

- 对应功能已完整移除，且没有可抽象的通用根因。
- 只记录一次性颜色、间距或文案调整，没有可复用边界。
- 与较新条目完全重复，没有额外根因或验证价值。
- 引用的文件、命令和行为均已不存在，并被新机制整体替代。

- [ ] **Step 4: 处理无法确认的冲突**

先扩大只读搜索范围；仍无法由当前仓库证明且会改变硬规则时停止该条迁移并向用户报告。其余条目继续处理，不因单条不确定阻塞清单核验。

### Task 3：创建领域经验文件并迁移有效内容

**Files:**
- Create: `docs/experience/*.md`
- Modify: `docs/experience/README.md`
- Read: `process.md`

- [ ] **Step 1: 创建统一文件头**

每个领域文件使用：

```markdown
# <领域>经验

适用范围：<一句话范围>。

关键词：`keyword-1`、`keyword-2`、`keyword-3`

## 目录

- [YYYY-MM-DD：经验标题](#生成后的锚点)

---
```

- [ ] **Step 2: 按审计表迁移 `kept` 条目**

条目按日期倒序排列，统一全角冒号标题：

```markdown
## YYYY-MM-DD：标题

**场景**：...

**问题**：...

**解决**：...

**关键点**：...

**涉及文件**：
- `path`

**验证**：
- `command`

**使用次数**：0
```

允许删减重复背景和失效路径；不得伪造未执行的验证结果。

- [ ] **Step 3: 写入合并条目**

合并条目只保留当前有效方案。必要的历史信息放在“问题”或“关键点”中，用“旧行为已废弃”明确隔离，不把旧命令继续写成可执行建议。

- [ ] **Step 4: 完成各文件目录**

每个保留条目在同文件目录中出现一次，目录链接能跳转到标题。

### Task 4：将 `process.md` 改为总索引

**Files:**
- Modify: `process.md`
- Read: `docs/experience/README.md`
- Read: `docs/experience/*.md`

- [ ] **Step 1: 替换经验正文**

`process.md` 只保留：用途、读取顺序、按领域索引、关键词索引、使用次数规则、固化规则和新经验模板。

- [ ] **Step 2: 明确新增经验位置**

新增经验直接写入对应 `docs/experience/<domain>.md` 顶部，并同步更新该文件目录；没有合适领域时先写入 `other-tools.md`，不要重新把正文追加到 `process.md`。

- [ ] **Step 3: 保留固化门槛**

保留“初始使用次数为 0；复用后 +1 并追加日期；达到 3 次后评估固化到根规范”的语义，同时明确只有高频、稳定、会影响 agent 决策的内容才固化。

### Task 5：精简并同步根规范

**Files:**
- Modify: `AGENTS.md`
- Modify: `CLAUDE.md`
- Read: `docs/experience/*.md`
- Read: `package.json`

- [ ] **Step 1: 重写 `AGENTS.md` 骨架**

使用以下一级结构：

```markdown
# AGENTS.md

## 1. 核心规则
## 2. 项目速览
## 3. 开工闸门
## 4. 常用命令与打包决策
## 5. 经验索引
## 6. 验证与提交
```

- [ ] **Step 2: 保留高频硬规则**

必须保留：用户指令优先、双文件同步、运行时不得依赖公网 CDN、默认浅色 UI、UTF-8 与 PowerShell 编码、dirty worktree 保护、不自动启动 dev server、默认直接在 `main` 修改、破坏性和数据迁移先确认、Windows 文件锁、复杂任务读取经验、3+ 文件后评估沉淀。

- [ ] **Step 3: 保留当前打包决策**

必须与 `package.json` 和当前脚本一致：用户仅说“打包”或“本地打包”时执行 `pnpm package:win`；安装包、正式 Release、完整四包分别使用明确命令；`tauri build` 负责可运行应用构建，不用裸 `cargo build --release` 交付桌面应用。

- [ ] **Step 4: 用索引替代专题正文**

PM、Todo、数据字典、富文本、Element Plus、离线手册、Windows 发版等专题细节只保留一行入口，指向对应经验文件。

- [ ] **Step 5: 同步生成 `CLAUDE.md`**

除标题、当前文件名和互指对象外，两份文档完全相同。

### Task 6：修复当前有效引用

**Files:**
- Modify if applicable: `docs/prompts/2026-07-17-access-path-diagnostics-planning-prompt.md`
- Read: non-historical repository documentation

- [ ] **Step 1: 搜索旧入口引用**

运行：

```powershell
rg -n "process\.md|AGENTS\.md|CLAUDE\.md" --glob "!docs/superpowers/specs/**" --glob "!docs/superpowers/plans/**" --glob "!docs/plans/**" --glob "!process.md" --glob "!AGENTS.md" --glob "!CLAUDE.md" .
```

- [ ] **Step 2: 只修复仍作为当前操作入口的文档**

历史 specs、plans、reviews 保留原始上下文，不批量改写。当前提示词或 README 若要求直接从旧 `process.md` 正文查经验，改为先查 `process.md` 总索引，再进入领域文件。

### Task 7：执行完整验证

**Files:**
- Verify: `AGENTS.md`
- Verify: `CLAUDE.md`
- Verify: `process.md`
- Verify: `docs/experience/*.md`

- [ ] **Step 1: 对账原始经验**

从修改前 `process.md` 的基线标题清单与 `docs/experience/README.md` 审计表比较。预期每条恰好一个处理结果，且：

```text
kept + merged + removed = baseline
```

- [ ] **Step 2: 验证领域文件结构**

检查每个 `kept` 目标存在，每个 `merged` 目标存在，每个保留条目包含日期、标题和使用次数；无空的 `TBD` / `TODO` / “稍后补充”。

- [ ] **Step 3: 验证链接**

逐一检查 `AGENTS.md`、`CLAUDE.md`、`process.md`、`docs/experience/README.md` 中的相对 Markdown 文件链接，预期目标均存在。

- [ ] **Step 4: 验证双文件同构**

将 `AGENTS.md` / `CLAUDE.md` 中的文件名归一化后执行 `Compare-Object`，预期输出 `IDENTICAL`。

- [ ] **Step 5: 验证关键规则**

使用 `rg` 确认以下内容仍存在于两份根规范：

```text
package:win
release:win
release:all:win
UTF-8
CDN
dirty worktree
破坏性
process.md
docs/experience
```

- [ ] **Step 6: 验证格式和改动边界**

运行：

```powershell
git diff --check
git status --short
git diff --stat
```

预期：无空白错误；三个既有业务文件改动保持原样；本任务只新增或修改设计、计划、根规范、经验索引和经验文档。

- [ ] **Step 7: 提交文档重构**

仅暂存本任务文件：

```powershell
git add AGENTS.md CLAUDE.md process.md docs/experience docs/superpowers/plans/2026-07-21-agent-doc-experience-refactor.md
git commit -m "docs(agent): 精简规范并按领域整理经验"
```

不得暂存当前工作区的三个业务代码改动。
