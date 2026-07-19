# 上线包按目标并行打包 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让上线包工具支持运行级前后端目标选择、双工作线程独立打包、部分成功归档、产物路径选择和项目级双列日志。

**Architecture:** 保留唯一活动 run，由现有后台线程充当轻量协调器，派生前端和后端工作线程；工作线程只写共享 staging 下互不冲突的目标，协调器汇总后只提交一次最终目录。Vue 运行态按 `projectId` 保存会话内最近一次结果，后端事件按 `frontend | backend | overall` 分发。

**Tech Stack:** Vue 3、TypeScript、Vitest、Element Plus、Tauri 2、Rust 标准线程/同步原语、rusqlite。

---

## 文件职责

- `apps/desktop/src/types/release-package.ts`：运行目标、任务状态、部分成功和事件协议类型。
- `apps/desktop/src/utils/releasePackage.ts`：目标选择校验、状态标签和有界日志纯函数。
- `apps/desktop/src/composables/useReleasePackageRuntime.ts`：按项目隔离最近一次运行态，并绑定唯一活动 run。
- `apps/desktop/src/components/ReleasePackagePanel.vue`：路径选择、确认弹窗目标勾选和项目内双列日志。
- `apps/desktop/src-tauri/src/tools/release_package.rs`：解析/校验 `targets`，只校验所选工程目录。
- `apps/desktop/src-tauri/src/tools/release_package_archive.rs`：管理 staging 生命周期、单目标归档和一次性提交。
- `apps/desktop/src-tauri/src/tools/release_package_runtime.rs`：两个工作线程、双 PID 取消、结果汇总与事件发送。
- 对应 `*.test.ts` 与 Rust 内联测试：先锁定行为，再写实现。

### Task 1: 扩展前端协议和纯函数

**Files:**
- Modify: `apps/desktop/src/types/release-package.ts`
- Modify: `apps/desktop/src/utils/releasePackage.test.ts`
- Modify: `apps/desktop/src/utils/releasePackage.ts`

- [ ] **Step 1: 写失败测试**

在 `releasePackage.test.ts` 增加默认目标、空选择校验、部分成功标签和按 phase 追加日志：

```ts
expect(createDefaultReleasePackageTargets()).toEqual(["frontend", "backend"]);
expect(validateReleasePackageTargets([])).toBe("请至少选择前端包或后端包");
expect(validateReleasePackageTargets(["backend"])).toBeNull();
expect(releasePackageRunStatusLabel("partially_succeeded")).toBe("部分成功");
expect(appendReleasePackageLog([], log("run-1", "a"), 1)).toEqual([log("run-1", "a")]);
```

- [ ] **Step 2: 验证测试按预期失败**

Run: `pnpm test src/utils/releasePackage.test.ts`

Expected: FAIL，提示新 helper 未导出或 `partially_succeeded` 不属于状态联合类型。

- [ ] **Step 3: 添加最小类型与 helper**

在类型文件中定义：

```ts
export type ReleasePackageTarget = "frontend" | "backend";
export type ReleasePackagePhase = ReleasePackageTarget | "overall";
export type ReleasePackageRunStatus =
  | "idle" | "running" | "succeeded" | "partially_succeeded" | "failed" | "cancelled";
export type ReleasePackageTargetStatus =
  | "idle" | "pending" | "running" | "succeeded" | "failed" | "cancelled" | "skipped";
```

在纯函数文件中实现：

```ts
export const createDefaultReleasePackageTargets = (): ReleasePackageTarget[] => ["frontend", "backend"];

export function validateReleasePackageTargets(targets: readonly ReleasePackageTarget[]): string | null {
  return targets.length === 0 ? "请至少选择前端包或后端包" : null;
}
```

并给状态标签表增加 `partially_succeeded: "部分成功"`。

- [ ] **Step 4: 验证测试通过**

Run: `pnpm test src/utils/releasePackage.test.ts`

Expected: PASS。

- [ ] **Step 5: 提交**

```powershell
git add apps/desktop/src/types/release-package.ts apps/desktop/src/utils/releasePackage.ts apps/desktop/src/utils/releasePackage.test.ts
git commit -m "feat(release-package): 扩展按目标运行协议"
```

### Task 2: 将前端运行态改为项目级隔离

**Files:**
- Modify: `apps/desktop/src/composables/useReleasePackageRuntime.test.ts`
- Modify: `apps/desktop/src/composables/useReleasePackageRuntime.ts`

- [ ] **Step 1: 写项目隔离失败测试**

测试两个项目的日志和状态互不覆盖、迟到 run 被拒绝、每列独立限长：

```ts
runtime.beginStart(7, ["frontend", "backend"]);
runtime.bindStartedRun("run-1", 7);
emit("release-package://log", frontendLog("run-1", 7, "web"));
emit("release-package://status", overallStatus("run-1", 7, "partially_succeeded"));

runtime.beginStart(8, ["backend"]);
runtime.bindStartedRun("run-2", 8);
emit("release-package://log", backendLog("run-2", 8, "server"));
emit("release-package://log", frontendLog("run-1", 7, "late"));

expect(runtime.getProjectRuntime(7).frontendLogs.map((item) => item.line)).toEqual(["web"]);
expect(runtime.getProjectRuntime(8).backendLogs.map((item) => item.line)).toEqual(["server"]);
```

- [ ] **Step 2: 验证测试按预期失败**

Run: `pnpm test src/composables/useReleasePackageRuntime.test.ts`

Expected: FAIL，提示 `beginStart` 参数或 `getProjectRuntime` 不存在。

- [ ] **Step 3: 实现项目运行态**

使用响应式 `Map<number, ReleasePackageProjectRuntime>`；每个项目保存：

```ts
interface ReleasePackageProjectRuntime {
  runId: string | null;
  status: ReleasePackageRunStatus;
  archivePath: string;
  error: string;
  targetStatus: Record<ReleasePackageTarget, ReleasePackageTargetStatus>;
  targetErrors: Partial<Record<ReleasePackageTarget, string>>;
  frontendLogs: ReleasePackageLogEvent[];
  backendLogs: ReleasePackageLogEvent[];
}
```

`beginStart(projectId, targets)` 只重置该项目；事件必须同时匹配当前活动 `runId` 和 `projectId`。`frontend`/`backend` 状态更新对应目标，`overall` 更新整体状态并在终态释放 pending 状态。每列最多保留 1,000 行。

- [ ] **Step 4: 验证 composable 测试通过**

Run: `pnpm test src/composables/useReleasePackageRuntime.test.ts`

Expected: PASS。

- [ ] **Step 5: 提交**

```powershell
git add apps/desktop/src/composables/useReleasePackageRuntime.ts apps/desktop/src/composables/useReleasePackageRuntime.test.ts
git commit -m "feat(release-package): 按项目隔离运行日志"
```

### Task 3: 解析运行目标并按选择校验

**Files:**
- Modify: `apps/desktop/src-tauri/src/tools/release_package.rs`

- [ ] **Step 1: 写 Rust 失败测试**

增加 `parse_targets` 和 `validate_run_inputs` 测试：

```rust
assert_eq!(parse_targets(&json!(["frontend", "backend"])).unwrap().len(), 2);
assert!(parse_targets(&json!([])).unwrap_err().contains("至少选择"));
assert!(parse_targets(&json!(["frontend", "frontend"])).is_err());
assert!(parse_targets(&json!(["mobile"])).is_err());

let project = project_with_missing_frontend_and_valid_backend();
assert!(validate_run_inputs(&project, output.path(), "release", &[ReleaseTarget::Backend]).is_ok());
```

- [ ] **Step 2: 验证 Rust 测试失败**

Run: `cargo test release_package::tests --manifest-path apps/desktop/src-tauri/Cargo.toml -- --nocapture`

Expected: FAIL，提示 `ReleaseTarget`、`parse_targets` 或新签名不存在。

- [ ] **Step 3: 实现目标解析与选择性校验**

定义可复制目标枚举：

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReleaseTarget { Frontend, Backend }
```

`parse_targets` 只接受非空、无重复的字符串数组。`execute_with_app("start")` 读取 `targets` 并传给 runtime；`validate_run_inputs` 只检查所选目标的工程目录，仍统一检查归档根目录和最终目录。

- [ ] **Step 4: 验证 Rust 输入测试通过**

Run: `cargo test release_package::tests --manifest-path apps/desktop/src-tauri/Cargo.toml -- --nocapture`

Expected: PASS。

- [ ] **Step 5: 提交**

```powershell
git add apps/desktop/src-tauri/src/tools/release_package.rs
git commit -m "feat(release-package): 校验本次打包目标"
```

### Task 4: 将归档拆为独立目标和统一提交

**Files:**
- Modify: `apps/desktop/src-tauri/src/tools/release_package_archive.rs`

- [ ] **Step 1: 写单目标归档失败测试**

覆盖前端单独提交、后端单独提交、失败目标清理且成功目标保留、顶层名称冲突：

```rust
let mut session = ArchiveSession::create(&output, "release", "run-partial", &cancelled).unwrap();
archive_frontend_artifact(&frontend, "copy_directory", session.staging_path(), &cancelled, |_| {}).unwrap();
let backend_error = archive_backend_artifact(&missing_backend, session.staging_path(), &cancelled, |_| {}).unwrap_err();
assert!(matches!(backend_error, ArchiveError::Failed(_)));
let final_path = session.commit(&cancelled).unwrap();
assert!(final_path.join("dist/index.html").is_file());
assert!(!final_path.join("server.jar").exists());
```

- [ ] **Step 2: 验证归档测试失败**

Run: `cargo test release_package_archive --manifest-path apps/desktop/src-tauri/Cargo.toml -- --nocapture`

Expected: FAIL，提示 `ArchiveSession` 和单目标函数不存在。

- [ ] **Step 3: 实现 staging 会话和目标 guard**

将现有统一 `archive_artifacts` 拆为：

```rust
pub struct ArchiveSession { staging_path: PathBuf, final_path: PathBuf, committed: bool }
impl ArchiveSession {
    pub fn create(output_root: &Path, folder_name: &str, run_id: &str, cancelled: &AtomicBool) -> Result<Self, ArchiveError>;
    pub fn staging_path(&self) -> &Path;
    pub fn commit(&mut self, cancelled: &AtomicBool) -> Result<PathBuf, ArchiveError>;
}

pub fn archive_frontend_artifact(
    source: &Path,
    mode: &str,
    staging_path: &Path,
    cancelled: &AtomicBool,
    emit: impl FnMut(&str),
) -> Result<(), ArchiveError>;
pub fn archive_backend_artifact(
    source: &Path,
    staging_path: &Path,
    cancelled: &AtomicBool,
    emit: impl FnMut(&str),
) -> Result<(), ArchiveError>;
pub fn validate_artifact_target_collision(
    frontend_source: &Path,
    frontend_mode: &str,
    backend_source: &Path,
) -> Result<(), ArchiveError>;
```

每个单目标函数使用只清理自己目标路径的 guard；成功后解除 guard。`ArchiveSession::Drop` 仅在未提交时清理本 run staging，`commit` 只执行一次同卷重命名。

- [ ] **Step 4: 验证归档测试通过**

Run: `cargo test release_package_archive --manifest-path apps/desktop/src-tauri/Cargo.toml -- --nocapture`

Expected: PASS。

- [ ] **Step 5: 提交**

```powershell
git add apps/desktop/src-tauri/src/tools/release_package_archive.rs
git commit -m "refactor(release-package): 拆分独立产物归档"
```

### Task 5: 实现双工作线程和轻量协调器

**Files:**
- Modify: `apps/desktop/src-tauri/src/tools/release_package_runtime.rs`

- [ ] **Step 1: 写并发和部分成功失败测试**

替换旧“前端失败后端不运行”断言，新增同步屏障或标记文件测试，证明两个命令可同时进入运行态；再覆盖一端 `exit 9` 时另一端仍产出并提交：

```rust
let result = run_pipeline(
    "partial-run", project, output_root.clone(), "release".into(),
    vec![ReleaseTarget::Frontend, ReleaseTarget::Backend],
    Arc::new(AtomicBool::new(false)), ProcessSlots::new(), sink,
).unwrap();
assert_eq!(result.status, "partially_succeeded");
assert!(result.archive_path.unwrap().join("app.jar").is_file());
assert!(backend_project.join("marker.txt").is_file());
```

同时增加两个 PID 槽位均会收到取消请求的单元测试。

- [ ] **Step 2: 验证 runtime 测试失败**

Run: `cargo test release_package_runtime --manifest-path apps/desktop/src-tauri/Cargo.toml -- --nocapture`

Expected: FAIL，旧 pipeline 串行短路或新参数/状态不存在。

- [ ] **Step 3: 实现协调器与工作线程**

新增：

```rust
#[derive(Clone)]
struct ProcessSlots {
    frontend: Arc<Mutex<Option<u32>>>,
    backend: Arc<Mutex<Option<u32>>>,
}

struct PipelineSummary {
    status: &'static str,
    archive_path: Option<PathBuf>,
    error: Option<String>,
}
```

协调器先建 `ArchiveSession`，再为已选目标各启动一个 `thread::spawn`。worker 执行命令、校验产物、归档自己的目标并发送目标终态；协调器 `join` 全部 worker，在共享取消标记未设置时按成功数决定提交与 `succeeded | partially_succeeded | failed`，最后只发送一次 `overall` 终态。

`ActiveRun` 保存 `ProcessSlots`；取消先设置共享标记，再分别终止两个非空 PID。工作失败不能设置共享取消标记。

- [ ] **Step 4: 验证 runtime 测试通过**

Run: `cargo test release_package_runtime --manifest-path apps/desktop/src-tauri/Cargo.toml -- --nocapture`

Expected: PASS；并发测试证明后端不会因前端失败而跳过。

- [ ] **Step 5: 提交**

```powershell
git add apps/desktop/src-tauri/src/tools/release_package_runtime.rs
git commit -m "feat(release-package): 并行执行前后端打包"
```

### Task 6: 更新确认弹窗、路径选择和双列日志

**Files:**
- Modify: `apps/desktop/src/components/ReleasePackagePanel.test.ts`
- Modify: `apps/desktop/src/components/ReleasePackagePanel.vue`

- [ ] **Step 1: 写组件结构失败测试**

锁定用户确认的结构和调用：

```ts
expect(source).toContain("chooseFrontendArtifact");
expect(source).toContain("chooseBackendArtifact");
expect(source).toContain('open({ directory: true, multiple: false');
expect(source).toContain('open({ directory: false, multiple: false');
expect(source.indexOf("归档目录名")).toBeLessThan(source.indexOf("本次打包内容"));
expect(source).toContain('class="release-package-project-log"');
expect(source).toContain('class="release-package-log-columns"');
expect(source).toContain('targets: selectedTargets.value');
```

- [ ] **Step 2: 验证组件测试失败**

Run: `pnpm test src/components/ReleasePackagePanel.test.ts`

Expected: FAIL，缺少路径选择函数、目标勾选和项目内双列日志。

- [ ] **Step 3: 实现确认和路径选择**

增加 `selectedTargets`，每次 `prepareStart` 重置为 `["frontend", "backend"]`。确认时先调用 `validateReleasePackageTargets`，再发送：

```ts
await invokeToolByChannel("tool:release-package:start", {
  projectId,
  folderName: folderName.value,
  targets: [...selectedTargets.value],
});
```

前端产物按钮使用目录对话框，后端产物按钮使用文件对话框；输入框不设 `readonly`。

- [ ] **Step 4: 实现项目内日志布局**

把日志卡片移入 `.release-package-editor`，以当前 `selectedId` 获取项目 runtime。使用两个日志容器分别渲染 `frontendLogs`、`backendLogs` 与目标状态；`.release-package-log-columns` 在宽屏为两列，在现有移动断点变为一列。整体状态包含 `partially_succeeded` 的 warning 标签，成功或部分成功且有路径时均显示“打开归档目录”。

- [ ] **Step 5: 验证前端相关测试通过**

Run: `pnpm test src/components/ReleasePackagePanel.test.ts src/composables/useReleasePackageRuntime.test.ts src/utils/releasePackage.test.ts`

Expected: PASS。

- [ ] **Step 6: 提交**

```powershell
git add apps/desktop/src/components/ReleasePackagePanel.vue apps/desktop/src/components/ReleasePackagePanel.test.ts
git commit -m "feat(release-package): 添加目标选择和双列日志"
```

### Task 7: 回归验证与经验沉淀

**Files:**
- Modify: `process.md`

- [ ] **Step 1: 运行 Rust 上线包测试**

Run: `cargo test release_package --manifest-path apps/desktop/src-tauri/Cargo.toml -- --nocapture`

Expected: PASS，所有上线包归档、运行和输入测试通过。

- [ ] **Step 2: 运行前端上线包测试**

Run: `pnpm test src/components/ReleasePackagePanel.test.ts src/composables/useReleasePackageRuntime.test.ts src/utils/releasePackage.test.ts`

Expected: PASS。

- [ ] **Step 3: 运行类型检查和渲染层构建**

Run: `pnpm typecheck`

Expected: exit 0。

Run: `pnpm --filter @lazycat/desktop build:web`

Expected: exit 0；允许已有 chunk size warning，不允许新增编译错误。

- [ ] **Step 4: 检查差异和工作区边界**

Run: `git diff --check`

Expected: exit 0。确认未修改现有 `request_forward` dirty files，且没有数据库迁移或无关重构。

- [ ] **Step 5: 记录 process.md**

新增一条经验：共享 staging 的并行任务由轻量协调器统一提交；工作线程只写独立目标；业务失败不传播取消，用户取消才传播共享取消标记；前端长任务状态按业务实体 id 隔离。

- [ ] **Step 6: 提交收尾文档**

```powershell
git add process.md
git commit -m "docs: 记录并行上线包打包经验"
```
