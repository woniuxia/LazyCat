# Default Main Worktree Policy Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在项目规范中明确非必要不使用 Git worktree，并默认在 `main` 分支修改。

**Architecture:** 同步修改 `AGENTS.md` 与 `CLAUDE.md` 的 `07.1 默认执行原则`，使用完全相同的单条规则。通过文本提取和文件比较验证双文件对应章节一致，不修改其他规则或 `process.md`。

**Tech Stack:** Markdown、Git

---

### Task 1: 同步新增默认 main 分支规则

**Files:**
- Modify: `AGENTS.md:360-370`
- Modify: `CLAUDE.md:360-370`

**Step 1: 记录修改前对应章节**

Run:

```powershell
Get-Content AGENTS.md | Select-Object -Skip 359 -First 12
Get-Content CLAUDE.md | Select-Object -Skip 359 -First 12
```

Expected: 两份文件的 `07.1 默认执行原则` 内容一致，尚无默认分支/worktree 约束。

**Step 2: 同步增加规则**

在两份文件的 `07.1 默认执行原则` 中，紧随“不自动启动 UI 或 dev server”规则之后加入完全相同的文案：

```markdown
- 非必要不创建或使用 Git worktree；默认直接在 `main` 分支修改。仅当用户明确要求隔离、存在并行修改冲突风险，或任务流程明确强制要求隔离时，才使用 worktree。
```

不修改 `02.2 Agent 决策闸门`、其他章节或 `process.md`。

**Step 3: 校验差异范围**

Run:

```powershell
git diff --check
git diff -- AGENTS.md CLAUDE.md
```

Expected: 两份文件各新增同一行，无其他差异。

**Step 4: 校验新增规则完全一致**

Run:

```powershell
Select-String -Path AGENTS.md,CLAUDE.md -Pattern "非必要不创建或使用 Git worktree"
```

Expected: 两份文件各命中一次，文本完全一致。

**Step 5: 提交**

```powershell
git add AGENTS.md CLAUDE.md
git commit -m "docs(agent): 默认在 main 分支执行修改"
```
