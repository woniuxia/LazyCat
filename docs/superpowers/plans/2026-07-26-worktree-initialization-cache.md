# Worktree Initialization Cache Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 缩短 LazyCat 必要 worktree 的初始化时间，避免初始化阶段完整构建，并启用本机已有的 Rust 编译缓存。

**Architecture:** 使用用户级 `RUSTC_WRAPPER=sccache` 为不同 worktree 提供跨目录 Rust 编译缓存，同时保留各 worktree 独立的 Cargo `target`。项目侧只同步更新 `AGENTS.md` 与 `CLAUDE.md`，将轻量初始化命令和禁止完整构建的边界设为高频规则，不增加脚本或仓库级工具依赖。

**Tech Stack:** PowerShell、pnpm 9、Cargo、sccache、Markdown、Git

---

### Task 1: 启用用户级 Rust 编译缓存

**Files:**
- Modify external user environment: `RUSTC_WRAPPER`

- [ ] **Step 1: 验证 sccache 可用且用户变量尚未配置**

Run:

```powershell
(Get-Command sccache -ErrorAction Stop).Source
[Environment]::GetEnvironmentVariable("RUSTC_WRAPPER", "User")
```

Expected: 第一条输出 `sccache.exe` 路径；第二条当前为空。若第一条失败，停止且不设置变量。

- [ ] **Step 2: 设置用户级 RUSTC_WRAPPER**

Run:

```powershell
[Environment]::SetEnvironmentVariable("RUSTC_WRAPPER", "sccache", "User")
```

Expected: 命令成功，无输出。该设置只影响之后启动的终端和进程，不覆盖当前运行中的 shell。

- [ ] **Step 3: 验证持久化值和 sccache 状态**

Run:

```powershell
[Environment]::GetEnvironmentVariable("RUSTC_WRAPPER", "User")
sccache --show-stats
```

Expected: 第一条输出 `sccache`；第二条正常输出缓存统计和缓存目录。

### Task 2: 固化 worktree 轻量初始化规则

**Files:**
- Modify: `AGENTS.md:15-16,37-43`
- Modify: `CLAUDE.md:15-16,37-43`

- [ ] **Step 1: 运行规则契约检查并确认失败**

Run:

```powershell
$files = @("AGENTS.md", "CLAUDE.md")
$rule = "worktree 初始化只运行 ``pnpm install --frozen-lockfile --prefer-offline``"
$command = "| ``pnpm install --frozen-lockfile --prefer-offline`` | worktree 依赖初始化"
foreach ($file in $files) {
  if (-not (Select-String -LiteralPath $file -SimpleMatch $rule -Quiet)) { throw "$file 缺少 worktree 初始化规则" }
  if (-not (Select-String -LiteralPath $file -SimpleMatch $command -Quiet)) { throw "$file 缺少 worktree 初始化命令" }
}
```

Expected: FAIL，指出 `AGENTS.md` 缺少 worktree 初始化规则。

- [ ] **Step 2: 同步增加核心规则**

在 `AGENTS.md` 和 `CLAUDE.md` 的“默认直接在 `main` 修改”之后同步加入：

```markdown
- 必要 worktree 初始化只运行 `pnpm install --frozen-lockfile --prefer-offline`；不得运行 `pnpm build`、Tauri build 或安装包构建。按任务范围执行定向验证。
- 本机已安装 `sccache` 时，通过用户级 `RUSTC_WRAPPER=sccache` 复用 Rust 编译缓存；各 worktree 保持独立 `target`，不把 `sccache` 设为项目硬依赖。
```

- [ ] **Step 3: 同步增加常用命令**

在两份文件的常用命令表中，将以下行加在 `pnpm test` 之前：

```markdown
| `pnpm install --frozen-lockfile --prefer-offline` | worktree 依赖初始化，不执行构建 |
```

- [ ] **Step 4: 重新运行规则契约检查**

Run:

```powershell
$files = @("AGENTS.md", "CLAUDE.md")
$rule = "worktree 初始化只运行 ``pnpm install --frozen-lockfile --prefer-offline``"
$command = "| ``pnpm install --frozen-lockfile --prefer-offline`` | worktree 依赖初始化"
foreach ($file in $files) {
  if (-not (Select-String -LiteralPath $file -SimpleMatch $rule -Quiet)) { throw "$file 缺少 worktree 初始化规则" }
  if (-not (Select-String -LiteralPath $file -SimpleMatch $command -Quiet)) { throw "$file 缺少 worktree 初始化命令" }
}
```

Expected: PASS，无输出。

- [ ] **Step 5: 验证双文件同构**

Run:

```powershell
$agents = (Get-Content -Raw AGENTS.md).Replace("# AGENTS.md", "# ROOT_AGENT_FILE").Replace("更新 ``CLAUDE.md``", "更新 ``PEER_AGENT_FILE``")
$claude = (Get-Content -Raw CLAUDE.md).Replace("# CLAUDE.md", "# ROOT_AGENT_FILE").Replace("更新 ``AGENTS.md``", "更新 ``PEER_AGENT_FILE``")
if ($agents -cne $claude) { throw "AGENTS.md 与 CLAUDE.md 不同构" }
```

Expected: PASS，无输出。

- [ ] **Step 6: 检查并提交规范改动**

Run:

```powershell
git diff --check
git diff -- AGENTS.md CLAUDE.md
git add AGENTS.md CLAUDE.md
git commit -m "docs(agent): 优化 worktree 初始化流程"
```

Expected: 两份文件新增完全相同的两条规则和一条命令；提交仅包含 `AGENTS.md` 与 `CLAUDE.md`。

### Task 3: 最终验证

**Files:**
- Verify: `AGENTS.md`
- Verify: `CLAUDE.md`
- Verify external user environment: `RUSTC_WRAPPER`

- [ ] **Step 1: 验证环境、规则和 Git 边界**

Run:

```powershell
$wrapper = [Environment]::GetEnvironmentVariable("RUSTC_WRAPPER", "User")
if ($wrapper -cne "sccache") { throw "用户级 RUSTC_WRAPPER 未配置为 sccache" }
sccache --show-stats
Select-String -Path AGENTS.md,CLAUDE.md -Pattern "worktree 初始化只运行|RUSTC_WRAPPER=sccache|worktree 依赖初始化"
git diff --check
git status --short
```

Expected: 用户变量为 `sccache`；缓存统计可读取；两个文件分别命中三项规则；`git diff --check` 无输出；工作树无本任务未提交改动。

本计划不运行 `pnpm build`、Tauri build 或安装包构建，因为改动仅涉及用户环境与 Markdown 规范，且完整构建是本次明确要移出 worktree 初始化阶段的高成本操作。
