# Agent 交付效率规则实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 通过根规则和 Agent 协作经验减少范围失控、碎片化确认、状态型功能后期返工和重复全量验证。

**Architecture:** `AGENTS.md` 只增加六条会改变执行决策的高频规则；`docs/experience/agent-workflow.md` 保存范围重定界模板、状态型功能检查表、审查顺序和三级验证矩阵。`CLAUDE.md` 继续引用 `AGENTS.md`，`process.md` 继续使用现有经验索引，不复制规则正文。

**Tech Stack:** Markdown、PowerShell、Git

---

## 文件职责与改动边界

- Modify: `AGENTS.md`：加入范围重定界、批量确认、MVP 分期、状态型功能闸门、结构性返工止损和分层验证六条根规则。
- Modify: `docs/experience/agent-workflow.md`：加入四个执行细节章节，承载模板和检查表。
- Verify only: `CLAUDE.md`：保持只通过 `@AGENTS.md` 加载规则，不复制正文。
- Verify only: `process.md`：保持现有 `agent-workflow.md` 索引可达。

不修改 Superpowers 插件、测试脚本、CI、Git Hook 或产品代码。

### Task 1: 增加根级交付效率规则

**Files:**

- Modify: `AGENTS.md:5`
- Verify: `CLAUDE.md`

- [ ] **Step 1: 记录修改前边界**

Run:

```powershell
git status --short
git diff -- AGENTS.md CLAUDE.md
```

Expected: 工作树中没有本任务之外对 `AGENTS.md` 或 `CLAUDE.md` 的修改；如目标文件已有用户改动，先读取并保留这些改动。

- [ ] **Step 2: 在核心规则中加入六条决策规则**

在 `AGENTS.md` 的 `## 1. 核心规则` 中，紧接“先读当前任务相关文件”规则后加入：

```markdown
- 当专用功能升级为通用架构，或任务新增持久化、后台任务、并发、恢复等复杂能力时，暂停实现并重新列出当前 MVP、后续阶段、非目标、影响范围和最低验证，只集中确认一次。
- 架构方向明确后，低风险实现细节由 Agent 基于现有代码合理决策；仅当用户行为、数据模型、架构边界、破坏性操作或外部副作用发生变化时再次确认。
- 通用能力先交付当前验收所需的最小闭环；历史、恢复、并行、自动触发等增强能力未进入本阶段验收标准时，不顺带实现。
- 涉及并发、后台执行、事件、轮询或事务时，编码前明确唯一事实源、状态所有权、生命周期、事务边界、失败释放路径和恢复语义。
- 同一边界连续暴露两个并发、事务或状态一致性问题时，停止叠加局部修复，回到不变量和时序设计检查根因。
- 验证分层执行：局部改动跑定向验证，阶段完成跑相关测试和类型检查，最终交付或合并前跑完整验证；共享契约或构建配置变化时可提前补完整验证。
```

不要修改 `CLAUDE.md`。该文件当前内容必须继续为：

```markdown
# CLAUDE.md

@AGENTS.md
```

- [ ] **Step 3: 检查六项规则和单一事实源**

Run:

```powershell
rg -n "专用功能升级|低风险实现细节|当前验收所需|唯一事实源|连续暴露两个|验证分层执行" AGENTS.md
Get-Content -Raw CLAUDE.md
```

Expected:

- `rg` 输出六行，每个关键词命中一条新规则；
- `CLAUDE.md` 只包含标题和 `@AGENTS.md`，没有复制新规则。

- [ ] **Step 4: 检查根规则差异**

Run:

```powershell
git diff --check
git diff -- AGENTS.md CLAUDE.md
```

Expected: `git diff --check` 无输出；差异只在 `AGENTS.md` 增加六条规则，`CLAUDE.md` 无差异。

- [ ] **Step 5: 提交根规则**

```powershell
git add AGENTS.md
git commit -m "docs: 增加 Agent 交付效率规则"
```

### Task 2: 补充 Agent 协作执行模板

**Files:**

- Modify: `docs/experience/agent-workflow.md`
- Verify: `process.md`

- [ ] **Step 1: 在 Agent 协作经验中加入范围重定界章节**

在 `docs/experience/agent-workflow.md` 的“dirty worktree 先划定边界”之后加入：

````markdown
## 范围升级先重新定界

专用功能升级为通用架构，或新增持久化、后台执行、并发、恢复等能力时，先暂停实现并集中输出一次范围摘要：

```text
当前目标：本阶段必须交付的用户结果
MVP：实现该结果所需的最小能力
后续阶段：有价值但不阻塞当前结果的能力
非目标：明确不做的行为
影响范围：涉及的领域、持久化、IPC 和 UI 边界
最低验证：本阶段完成的证据
```

如果用户已经明确选择，不重复询问。用户回复“继续”或“确定”后，按已确认摘要执行到下一个会改变用户行为、数据模型、架构边界或外部副作用的决策点；低风险实现细节由 Agent 基于现有代码处理。
````

- [ ] **Step 2: 加入状态型功能设计检查表**

紧接范围重定界章节加入：

```markdown
## 状态型功能先明确不变量

涉及并发、后台执行、事件、轮询或事务时，编码前回答：

- 哪个存储或对象是唯一事实源，缓存和事件只承担什么角色；
- 谁创建、持有和释放运行状态，成功、失败、panic 和启动失败路径是否都能释放；
- 哪些读取与写入必须处于同一事务，是否存在校验后状态变化的 TOCTOU；
- 并行 worker 是否共享不可跨线程资源，单个失败是否与其他步骤隔离；
- 事件、轮询和请求晚响应如何避免旧状态覆盖新状态；
- 页面卸载、监听重建和应用重启后的状态如何收口。

同一边界连续出现两个并发、事务或状态一致性问题，说明不变量或时序仍不完整。此时停止叠加局部补丁，重新检查所有权、事务边界和交错路径。
```

- [ ] **Step 3: 加入风险前置审查顺序**

紧接状态型功能章节加入：

```markdown
## 复杂状态功能按风险前置审查

1. 数据模型、事务边界和状态机骨架完成后，先审查不变量及交错时序。
2. 领域执行与 UI 接通后，审查跨模块契约和用户反馈。
3. 最终交付或合并前，审查最终差异、遗漏测试和工作树边界。

修复审查问题后先验证受影响边界；除非修改触及共享契约或构建链，不重复执行未受影响的完整验证。
```

- [ ] **Step 4: 用三级验证矩阵替换原“验证基于证据”正文**

保留现有 `## 验证基于证据` 标题，将其正文替换为：

```markdown
验证分为三级：

| 级别     | 触发时机                           | 验证范围                                                |
| -------- | ---------------------------------- | ------------------------------------------------------- |
| 定向验证 | 每个局部实现或修复后               | 对应单元测试、契约测试或最小检查                        |
| 阶段验证 | 一个数据层、执行层或 UI 阶段完成后 | 相关测试集合，必要时增加类型检查                        |
| 最终验证 | 功能验收完成、提交或合并前         | 任务要求的完整测试、类型检查、构建和 `git diff --check` |

失败必须显式报告。分层验证只减少无影响范围的重复执行，不允许省略任务要求的最终验证，也不把静态阅读或未运行命令描述为“已验证”。文档迁移继续使用数量对账、链接检查、规则关键词检查和差异边界检查。
```

- [ ] **Step 5: 检查章节、索引和措辞一致性**

Run:

```powershell
rg -n "^## 范围升级先重新定界|^## 状态型功能先明确不变量|^## 复杂状态功能按风险前置审查|^## 验证基于证据" docs/experience/agent-workflow.md
rg -n "agent-workflow.md" process.md
rg -n "唯一事实源|TOCTOU|三级|最终验证" AGENTS.md docs/experience/agent-workflow.md
```

Expected:

- Agent 协作经验中的四个章节各命中一次；
- `process.md` 仍有一条指向 `docs/experience/agent-workflow.md` 的索引；
- 根规则与经验文档使用相同的事实源、事务和分层验证术语。

- [ ] **Step 6: 执行最终文档验证**

Run:

```powershell
git diff --check
git status --short
git diff -- docs/experience/agent-workflow.md process.md CLAUDE.md
```

Expected:

- `git diff --check` 无输出；
- 工作树只包含 `docs/experience/agent-workflow.md` 的本任务修改；
- `process.md` 和 `CLAUDE.md` 无差异。

- [ ] **Step 7: 提交执行细节**

```powershell
git add docs/experience/agent-workflow.md
git commit -m "docs: 补充 Agent 复杂任务执行准则"
```

### Task 3: 最终对账

**Files:**

- Verify: `AGENTS.md`
- Verify: `CLAUDE.md`
- Verify: `process.md`
- Verify: `docs/experience/agent-workflow.md`

- [ ] **Step 1: 对账设计覆盖**

Run:

```powershell
rg -n "专用功能升级|低风险实现细节|当前验收所需|唯一事实源|连续暴露两个|验证分层执行" AGENTS.md
rg -n "范围升级先重新定界|状态型功能先明确不变量|复杂状态功能按风险前置审查|验证基于证据" docs/experience/agent-workflow.md
Get-Content -Raw CLAUDE.md
rg -n "agent-workflow.md" process.md
```

Expected: 六项根规则、四个执行章节、`@AGENTS.md` 引用和经验索引全部存在。

- [ ] **Step 2: 检查最终差异与提交**

Run:

```powershell
git diff --check HEAD~2 HEAD
git status --short --branch
git log -2 --oneline
```

Expected:

- `git diff --check HEAD~2 HEAD` 无输出；
- 工作树干净；
- 最近两个提交分别为根规则和执行细节提交。

本任务只修改 Markdown，不运行产品测试、类型检查或构建。
