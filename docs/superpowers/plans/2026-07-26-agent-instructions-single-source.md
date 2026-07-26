# Agent Instructions Single Source Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 `AGENTS.md` 收敛为项目规则唯一事实源，并让 Claude Code 通过薄 `CLAUDE.md` 入口导入同一份规则。

**Architecture:** `AGENTS.md` 保留完整公共规则，`CLAUDE.md` 只包含标题和 `@AGENTS.md` 导入。现行流程文档统一描述这一加载关系，测试分别守护业务规则事实源和 Claude Code 适配契约。

**Tech Stack:** Markdown、Claude Code memory imports、Vitest、TypeScript、pnpm

---

### Task 1: 锁定 Claude Code 薄适配契约

**Files:**
- Modify: `apps/desktop/src/utils/windowsPackagingCommand.test.ts`
- Test: `apps/desktop/src/utils/windowsPackagingCommand.test.ts`

- [ ] **Step 1: 将业务规则检查收敛到唯一事实源**

把现有测试中的：

```ts
expect(agentsSource).toContain(agentRule);
expect(claudeSource).toContain(agentRule);
```

改为：

```ts
expect(agentsSource).toContain(agentRule);
```

- [ ] **Step 2: 增加 Claude Code 适配入口契约测试**

在同一 `describe` 中增加：

```ts
it("loads shared agent rules through the Claude adapter", () => {
  expect(claudeSource.replaceAll("\r\n", "\n").trim()).toBe(
    "# CLAUDE.md\n\n@AGENTS.md",
  );
});
```

- [ ] **Step 3: 运行定向测试并确认新契约失败**

Run: `pnpm --filter @lazycat/desktop test -- windowsPackagingCommand.test.ts`

Expected: FAIL，`CLAUDE.md` 当前仍包含完整重复规则，不等于薄适配内容。

### Task 2: 迁移项目规则入口

**Files:**
- Modify: `AGENTS.md`
- Modify: `CLAUDE.md`

- [ ] **Step 1: 删除 `AGENTS.md` 的双文件维护要求**

删除文首“双文件同步”引用；将开工闸门第 3 条改为：

```markdown
3. 检查联动：前端/bridge/Rust/类型/测试、Element Plus 双主题文件和 Agent 规则入口。
```

将文档验证规则改为：

```markdown
- 文档改动：检查链接、索引、Agent 规则入口和关键规则完整性。
```

- [ ] **Step 2: 将 `CLAUDE.md` 替换为薄适配入口**

完整内容为：

```markdown
# CLAUDE.md

@AGENTS.md
```

- [ ] **Step 3: 运行定向测试并确认通过**

Run: `pnpm --filter @lazycat/desktop test -- windowsPackagingCommand.test.ts`

Expected: PASS，5 个测试全部通过。

### Task 3: 更新现行流程与经验说明

**Files:**
- Modify: `process.md`
- Modify: `docs/experience/README.md`
- Modify: `docs/experience/agent-workflow.md`

- [ ] **Step 1: 统一执行入口描述**

在 `process.md` 和 `docs/experience/README.md` 中将根入口描述改为：

```markdown
先读根目录 `AGENTS.md` 的核心规则和开工闸门；Claude Code 通过 `CLAUDE.md` 自动导入同一份规则。
```

- [ ] **Step 2: 用单一事实源经验替换双文件同步经验**

将 `docs/experience/agent-workflow.md` 对应章节改为：

```markdown
## Agent 规则使用单一事实源

跨 Agent 的项目规则只维护在 `AGENTS.md`。`CLAUDE.md` 仅通过 `@AGENTS.md` 提供 Claude Code 加载适配；修改规则时检查适配入口仍有效，不复制规则正文。
```

- [ ] **Step 3: 检查现行文档没有遗留维护要求**

Run: `rg -n "双文件同步|归一化比较|保持同构|同步另一份" AGENTS.md CLAUDE.md process.md docs/experience/README.md docs/experience/agent-workflow.md`

Expected: 无输出。

- [ ] **Step 4: 检查入口引用和业务规则**

Run: `rg -n "@AGENTS.md|唯一事实源|pnpm package:win" AGENTS.md CLAUDE.md process.md docs/experience/README.md docs/experience/agent-workflow.md`

Expected: `CLAUDE.md` 命中 `@AGENTS.md`，经验文件命中“唯一事实源”，`AGENTS.md` 命中打包规则。

### Task 4: 完整验证与提交

**Files:**
- Verify: `AGENTS.md`
- Verify: `CLAUDE.md`
- Verify: `process.md`
- Verify: `docs/experience/README.md`
- Verify: `docs/experience/agent-workflow.md`
- Verify: `apps/desktop/src/utils/windowsPackagingCommand.test.ts`

- [ ] **Step 1: 运行定向测试**

Run: `pnpm --filter @lazycat/desktop test -- windowsPackagingCommand.test.ts`

Expected: PASS，5 个测试全部通过。

- [ ] **Step 2: 运行格式与变更边界检查**

Run: `git diff --check`

Expected: 无输出，退出码为 0。

Run: `git status --short`

Expected: 只包含本计划列出的当前任务文件和计划文件。

- [ ] **Step 3: 提交实现**

```powershell
git add AGENTS.md CLAUDE.md process.md docs/experience/README.md docs/experience/agent-workflow.md apps/desktop/src/utils/windowsPackagingCommand.test.ts docs/superpowers/plans/2026-07-26-agent-instructions-single-source.md
git commit -m "docs(agent): 收敛项目规则单一事实源"
```
