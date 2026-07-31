# Safe Default Windows Packaging Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 新增无歧义的 `pnpm package:win` 本地打包入口，并通过命令提示和代理规则确保未指定产物类型时默认生成 lite portable zip。

**Architecture:** 新建一个最小 PowerShell 包装脚本，自动从根 `package.json` 读取版本，并固定调用现有 `release-all-win.ps1 -SkipUpload`，不复制任何构建逻辑。保留全部旧命令行为，通过静态契约测试、NSIS 警告和同步文档形成三层防呆。

**Tech Stack:** PowerShell、pnpm、Node.js、Vitest、Markdown

---

## File Structure

- Create: `scripts/package-lite-win.ps1` - 自动读取版本并调用现有 lite portable 本地打包流程。
- Create: `apps/desktop/src/utils/windowsPackagingCommand.test.ts` - 验证命令、脚本参数、警告和文档规则。
- Modify: `package.json` - 注册 `package:win`。
- Modify: `scripts/build-tauri-win.ps1` - 在 NSIS 构建入口输出纠错提示。
- Modify: `AGENTS.md` - 固化代理的默认打包决策规则和命令说明。
- Modify: `CLAUDE.md` - 与 `AGENTS.md` 同步打包规则。
- Modify: `README.md` - 区分本地 lite 打包与正式发布命令。
- Modify: `process.md` - 记录误操作根因和防呆经验。

### Task 1: 写入打包命令契约测试

**Files:**

- Create: `apps/desktop/src/utils/windowsPackagingCommand.test.ts`

- [ ] **Step 1: 创建失败测试**

```ts
import { existsSync, readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const rootUrl = new URL("../../../../", import.meta.url);

function readRootFile(path: string): string {
  const url = new URL(path, rootUrl);
  return existsSync(url) ? readFileSync(url, "utf-8") : "";
}

const packageJson = JSON.parse(readRootFile("package.json")) as {
  scripts: Record<string, string>;
};
const wrapperSource = readRootFile("scripts/package-lite-win.ps1");
const nsisBuildSource = readRootFile("scripts/build-tauri-win.ps1");
const agentsSource = readRootFile("AGENTS.md");
const claudeSource = readRootFile("CLAUDE.md");
const readmeSource = readRootFile("README.md");

describe("Windows packaging command guardrails", () => {
  it("exposes one local lite portable packaging command", () => {
    expect(packageJson.scripts["package:win"]).toBe(
      "powershell -ExecutionPolicy Bypass -File scripts/package-lite-win.ps1",
    );
  });

  it("derives the tag from package.json and always skips upload", () => {
    expect(wrapperSource).toContain('$rootPackageJsonPath = Join-Path $repoRoot "package.json"');
    expect(wrapperSource).toContain("$version = [string]$rootPackage.version");
    expect(wrapperSource).toContain("[string]::IsNullOrWhiteSpace($version)");
    expect(wrapperSource).toContain('& $releaseScript -Tag "v$version" -SkipUpload');
    expect(wrapperSource).not.toContain("-AllPackages");
  });

  it("warns when the NSIS build entry is used", () => {
    expect(nsisBuildSource).toContain(
      'Write-Warning "This command builds a Windows NSIS installer. For the default lite portable zip, run: pnpm package:win"',
    );
  });

  it("documents the same default command for agents and users", () => {
    const agentRule = "只说“打包”或“本地打包”时，必须执行 `pnpm package:win`";
    expect(agentsSource).toContain(agentRule);
    expect(claudeSource).toContain(agentRule);
    expect(readmeSource).toContain("`pnpm package:win`");
    expect(readmeSource).toContain("本地构建 lite portable");
  });
});
```

- [ ] **Step 2: 运行测试并确认按预期失败**

Run:

```powershell
pnpm --filter @lazycat/desktop test -- src/utils/windowsPackagingCommand.test.ts
```

Expected: FAIL；失败应明确指出 `package:win` 为 `undefined`、包装脚本内容为空、NSIS 警告或文档规则缺失，而不是文件读取异常。

### Task 2: 实现兼容的双重防呆入口

**Files:**

- Create: `scripts/package-lite-win.ps1`
- Modify: `package.json`
- Modify: `scripts/build-tauri-win.ps1`

- [ ] **Step 1: 新增最小包装脚本**

创建 `scripts/package-lite-win.ps1`：

```powershell
$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$rootPackageJsonPath = Join-Path $repoRoot "package.json"
$rootPackage = Get-Content $rootPackageJsonPath -Raw | ConvertFrom-Json
$version = [string]$rootPackage.version

if ([string]::IsNullOrWhiteSpace($version)) {
  throw "Root package version is missing: $rootPackageJsonPath"
}

$releaseScript = Join-Path $PSScriptRoot "release-all-win.ps1"
Write-Host "Packaging Lazycat v$version as lite portable (local only, no upload)..."
& $releaseScript -Tag "v$version" -SkipUpload
```

- [ ] **Step 2: 注册根命令**

在根 `package.json` 的 `scripts` 中，将以下命令放在 `build:portable` 后：

```json
"package:win": "powershell -ExecutionPolicy Bypass -File scripts/package-lite-win.ps1"
```

- [ ] **Step 3: 给旧 NSIS 入口增加明确提示**

在 `scripts/build-tauri-win.ps1` 的 `$ErrorActionPreference = "Stop"` 后增加：

```powershell
Write-Warning "This command builds a Windows NSIS installer. For the default lite portable zip, run: pnpm package:win"
```

- [ ] **Step 4: 运行针对性测试**

Run:

```powershell
pnpm --filter @lazycat/desktop test -- src/utils/windowsPackagingCommand.test.ts
```

Expected: 前三个测试通过，文档测试仍因规则尚未同步而失败。

### Task 3: 同步代理规则和用户文档

**Files:**

- Modify: `AGENTS.md`
- Modify: `CLAUDE.md`
- Modify: `README.md`
- Modify: `process.md`

- [ ] **Step 1: 在双规范核心硬规则中增加默认决策**

在 `AGENTS.md` 和 `CLAUDE.md` 的便携包规则后同步增加：

```markdown
- 用户未指定产物类型、只说“打包”或“本地打包”时，必须执行 `pnpm package:win`；只有明确要求安装包、正式 GitHub Release 或完整四包时，才使用对应的 `build:win`、`release:win` 或 `release:all:win`。
```

- [ ] **Step 2: 在双规范和 README 命令表增加本地入口**

在三份命令表中增加：

```markdown
| `pnpm package:win` | 本地构建 lite portable zip 和 SHA256，不上传 GitHub |
```

并在 `AGENTS.md`、`CLAUDE.md` 的构建要点中增加：

```markdown
- `pnpm package:win` 是日常“打包”的唯一默认入口：自动读取当前版本并调用 `release:win` 的本地 lite portable 流程，不创建 tag、不推送、不上传。
```

- [ ] **Step 3: 记录 process.md 经验**

在 `process.md` 顶部的新记录区域加入：

```markdown
## 2026-07-21: Windows 本地打包使用唯一防呆入口

**场景**: 用户只说“打包”时，代理误把 Windows 打包理解为 NSIS 安装包，执行了名称含 `precheck` 但实际会完整构建 NSIS 的脚本。

**问题**:

1. `release:win` 虽已默认生成 lite portable，但本地打包仍需调用者手动传版本和 `-SkipUpload`，通用“打包”缺少唯一入口。
2. `build:win:precheck` 的名称容易被理解为轻量检查，实际会继续执行 `tauri build --bundles nsis`。

**解决**:

1. 新增 `pnpm package:win`，自动读取根版本并固定调用 `release-all-win.ps1 -SkipUpload`，默认只生成 lite portable zip 和 SHA256。
2. 保留旧命令行为，在 NSIS 构建脚本入口增加纠错提示。
3. 在 `AGENTS.md`、`CLAUDE.md` 和 README 中统一命令决策：未指定产物类型的“打包”必须走 `package:win`。

**关键点**:

- 通过唯一命令表达用户意图，比继续堆叠文档提醒更可靠。
- 本地打包与正式发布必须分开：前者禁止上传，后者继续显式要求 tag。

**涉及文件**:

- scripts/package-lite-win.ps1
- scripts/build-tauri-win.ps1
- package.json
- AGENTS.md
- CLAUDE.md
- README.md
- apps/desktop/src/utils/windowsPackagingCommand.test.ts

**验证**:

- pnpm --filter @lazycat/desktop test -- src/utils/windowsPackagingCommand.test.ts
- PowerShell AST 解析
- pnpm test
- pnpm typecheck

**使用次数**: 0
```

- [ ] **Step 4: 运行针对性测试并确认全部通过**

Run:

```powershell
pnpm --filter @lazycat/desktop test -- src/utils/windowsPackagingCommand.test.ts
```

Expected: PASS，4 个测试全部通过。

### Task 4: 验证与提交

**Files:**

- Verify: `scripts/package-lite-win.ps1`
- Verify: `scripts/build-tauri-win.ps1`
- Verify: all modified files

- [ ] **Step 1: 解析 PowerShell 语法**

Run:

```powershell
$parseErrors = @()
[System.Management.Automation.Language.Parser]::ParseFile((Resolve-Path "scripts/package-lite-win.ps1"), [ref]$null, [ref]$parseErrors) | Out-Null
[System.Management.Automation.Language.Parser]::ParseFile((Resolve-Path "scripts/build-tauri-win.ps1"), [ref]$null, [ref]$parseErrors) | Out-Null
if ($parseErrors.Count -gt 0) { $parseErrors | Format-List; exit 1 }
```

Expected: 退出码 0，无解析错误。

- [ ] **Step 2: 检查双规范同步和差异格式**

Run:

```powershell
$agentRule = '只说“打包”或“本地打包”时，必须执行 `pnpm package:win`'
if (-not (Select-String -Path "AGENTS.md" -SimpleMatch $agentRule)) { exit 1 }
if (-not (Select-String -Path "CLAUDE.md" -SimpleMatch $agentRule)) { exit 1 }
git diff --check
```

Expected: 退出码 0，无缺失规则或空白错误。

- [ ] **Step 3: 运行完整测试与类型检查**

Run:

```powershell
pnpm test
pnpm typecheck
```

Expected: 两条命令退出码均为 0；全部测试通过，类型检查无错误。

- [ ] **Step 4: 提交实现**

```powershell
git add "scripts/package-lite-win.ps1" "scripts/build-tauri-win.ps1" "package.json" "AGENTS.md" "CLAUDE.md" "README.md" "process.md" "apps/desktop/src/utils/windowsPackagingCommand.test.ts" "docs/superpowers/plans/2026-07-21-safe-default-windows-packaging.md"
git commit -m "feat(build): 增加默认 lite 打包防呆入口"
```
