# 上线包目标归档覆盖 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在目标归档目录已存在时提示用户取消或直接覆盖，并以可回滚的完整替换方式提交新上线包。

**Architecture:** 新增只读 `target_check` action，在启动前由前端查询目标路径状态；确认覆盖后通过 `overwriteExisting` 将授权显式传到运行时。Rust `ArchiveSession` 继续先构建 staging，提交时把旧目录同卷重命名为 runId 备份，再切换新目录，提交失败则恢复旧目录。

**Tech Stack:** Vue 3、TypeScript、Element Plus、Tauri IPC、Rust、rusqlite、Vitest、Cargo test

---

## 文件结构

- `apps/desktop/src-tauri/src/tools/release_package.rs`：目标检查 action、覆盖参数解析与启动前输入校验。
- `apps/desktop/src-tauri/src/tools/release_package_archive.rs`：staging、备份、完整替换和回滚。
- `apps/desktop/src-tauri/src/tools/release_package_runtime.rs`：把覆盖授权传入归档会话。
- `apps/desktop/src/bridge/tauri.ts`：注册 `tool:release-package:target-check` 通道。
- `apps/desktop/src/types/release-package.ts`：声明目标检查结果。
- `apps/desktop/src/components/ReleasePackagePanel.vue`：启动前检查和 Element Plus 二次确认。
- `apps/desktop/src/components/ReleasePackagePanel.test.ts`：锁定确认文案、按钮和请求参数。
- `process.md`：记录事务式完整替换经验。

### Task 1: 目标路径检查

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/release_package.rs`
- Modify: `apps/desktop/src/bridge/tauri.ts`
- Modify: `apps/desktop/src/types/release-package.ts`

- [ ] **Step 1: 写目标检查失败测试**

在 `release_package.rs` 测试模块创建临时 output root 和项目，断言目标不存在时 `exists=false`、创建目标目录后 `exists=true`，并断言文件类型目标返回错误：

```rust
#[test]
fn target_check_reports_existing_directory_and_rejects_file() {
    let root = std::env::temp_dir().join(format!(
        "lazycat-release-target-check-test-{}",
        uuid::Uuid::new_v4()
    ));
    let output = root.join("output");
    fs::create_dir_all(&output).unwrap();
    let conn = test_conn();
    let mut project = payload();
    project["outputRoot"] = json!(output.to_string_lossy());
    let id = project_create_with_conn(&conn, &project).unwrap()["id"]
        .as_i64()
        .unwrap();

    let missing = target_check_with_conn(&conn, id, "release").unwrap();
    assert_eq!(missing["exists"], false);

    fs::create_dir(output.join("release")).unwrap();
    let existing = target_check_with_conn(&conn, id, "release").unwrap();
    assert_eq!(existing["exists"], true);

    fs::remove_dir(output.join("release")).unwrap();
    fs::write(output.join("release"), "file").unwrap();
    assert!(target_check_with_conn(&conn, id, "release").is_err());
    fs::remove_dir_all(root).unwrap();
}
```

- [ ] **Step 2: 运行 Rust 测试并确认失败**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package::tests::target_check_reports_existing_directory_and_rejects_file -- --nocapture`

Expected: FAIL，`target_check_with_conn` 尚不存在。

- [ ] **Step 3: 实现目标检查 action**

新增 `target_check` 到 `ACTIONS`，校验 `projectId`、`folderName`、项目归档根目录和目标类型，返回：

```rust
fn target_check_with_conn(conn: &Connection, project_id: i64, folder_name: &str) -> Result<Value, String> {
    validate_folder_name(folder_name)?;
    let project = load_project(conn, project_id)?;
    let output_root = PathBuf::from(&project.output_root);
    if !output_root.is_dir() {
        return Err("归档根目录不存在或不是文件夹".into());
    }
    let archive_path = output_root.join(folder_name);
    if archive_path.exists() && !archive_path.is_dir() {
        return Err("目标归档路径已存在且不是文件夹".into());
    }
    Ok(json!({
        "archivePath": archive_path.to_string_lossy(),
        "exists": archive_path.is_dir(),
    }))
}
```

同步在 bridge 注册 `target-check`，并新增：

```ts
export interface ReleasePackageTargetCheckResult {
  archivePath: string;
  exists: boolean;
}
```

- [ ] **Step 4: 运行目标检查测试并确认通过**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package::tests::target_check_reports_existing_directory_and_rejects_file -- --nocapture`

Expected: PASS。

### Task 2: 归档完整替换与回滚

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/release_package_archive.rs`

- [ ] **Step 1: 写覆盖和回滚失败测试**

为 `ArchiveSession::create` 增加 `overwrite_existing` 参数后，先写调用新签名的测试：

```rust
#[test]
fn overwrite_replaces_existing_directory_without_stale_files() {
    let root = TestDir::new();
    let output = root.0.join("output");
    let final_path = output.join("release");
    fs::create_dir_all(&final_path).unwrap();
    fs::write(final_path.join("stale.txt"), "old").unwrap();
    let cancelled = AtomicBool::new(false);
    let mut session = ArchiveSession::create(&output, "release", "run-overwrite", true, &cancelled).unwrap();
    fs::write(session.staging_path().join("new.txt"), "new").unwrap();

    session.commit(&cancelled).unwrap();

    assert!(!final_path.join("stale.txt").exists());
    assert_eq!(fs::read_to_string(final_path.join("new.txt")).unwrap(), "new");
    assert!(!output.join(".lazycat-release-package-run-overwrite.backup").exists());
}

#[test]
fn failed_overwrite_commit_restores_existing_directory() {
    let root = TestDir::new();
    let output = root.0.join("output");
    let final_path = output.join("release");
    fs::create_dir_all(&final_path).unwrap();
    fs::write(final_path.join("old.txt"), "old").unwrap();
    let cancelled = AtomicBool::new(false);
    let mut session = ArchiveSession::create(&output, "release", "run-rollback", true, &cancelled).unwrap();
    fs::remove_dir_all(session.staging_path()).unwrap();

    assert!(session.commit(&cancelled).is_err());
    assert_eq!(fs::read_to_string(final_path.join("old.txt")).unwrap(), "old");
}
```

- [ ] **Step 2: 运行归档测试并确认失败**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package_archive -- --nocapture`

Expected: FAIL，新签名和覆盖行为尚未实现。

- [ ] **Step 3: 实现备份切换和回滚**

让 `ArchiveSession` 保存 `backup_path` 和 `overwrite_existing`。`create` 对已有文件拒绝，对已有目录仅在明确覆盖时放行，并拒绝已存在的 runId 备份。`commit` 执行：

```rust
if self.final_path.exists() {
    if !self.overwrite_existing {
        return Err(ArchiveError::Failed("目标归档目录在执行期间被创建".into()));
    }
    if !self.final_path.is_dir() {
        return Err(ArchiveError::Failed("目标归档路径已存在且不是文件夹".into()));
    }
    fs::rename(&self.final_path, &self.backup_path)
        .map_err(|error| io_error("备份已有归档目录", &self.final_path, &self.backup_path, error))?;
}
if let Err(error) = fs::rename(&self.staging_path, &self.final_path) {
    let rollback_error = self.backup_path.exists()
        .then(|| fs::rename(&self.backup_path, &self.final_path).err())
        .flatten();
    return Err(commit_error_with_rollback(error, rollback_error));
}
self.committed = true;
if self.backup_path.exists() {
    fs::remove_dir_all(&self.backup_path)
        .map_err(|error| io_error("清理旧归档备份", &self.backup_path, &self.final_path, error))?;
}
```

所有现有 `ArchiveSession::create` 调用补 `false`，保证默认仍拒绝覆盖。

- [ ] **Step 4: 运行归档测试并确认通过**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package_archive -- --nocapture`

Expected: PASS，覆盖后无旧文件和备份残留，提交失败恢复旧目录。

### Task 3: 覆盖授权贯穿启动与运行时

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/release_package.rs`
- Modify: `apps/desktop/src-tauri/src/tools/release_package_runtime.rs`

- [ ] **Step 1: 写覆盖参数校验测试**

把 `validate_run_inputs` 签名改为接收 `overwrite_existing`，测试已有目录在 `false` 时失败、`true` 时通过，并增加严格布尔解析测试：

```rust
assert!(validate_run_inputs(&project, "release", &[ReleaseTarget::Backend], false).is_err());
assert!(validate_run_inputs(&project, "release", &[ReleaseTarget::Backend], true).is_ok());
assert_eq!(parse_overwrite_existing(&json!({})).unwrap(), false);
assert_eq!(parse_overwrite_existing(&json!({ "overwriteExisting": true })).unwrap(), true);
assert!(parse_overwrite_existing(&json!({ "overwriteExisting": "true" })).is_err());
```

- [ ] **Step 2: 运行 Rust 测试并确认失败**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package -- --nocapture`

Expected: FAIL，新参数尚未贯穿。

- [ ] **Step 3: 实现参数解析和运行时传递**

实现缺失默认 `false` 的严格解析：

```rust
fn parse_overwrite_existing(payload: &Value) -> Result<bool, String> {
    match payload.get("overwriteExisting") {
        None => Ok(false),
        Some(Value::Bool(value)) => Ok(*value),
        Some(_) => Err("overwriteExisting must be a boolean".into()),
    }
}
```

将该值依次传入 `validate_run_inputs`、`release_package_runtime::start`、`run_pipeline` 和 `ArchiveSession::create`。

- [ ] **Step 4: 运行所有上线包 Rust 测试**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package -- --nocapture`

Expected: PASS。

### Task 4: 前端二次确认

**Files:**

- Modify: `apps/desktop/src/components/ReleasePackagePanel.test.ts`
- Modify: `apps/desktop/src/components/ReleasePackagePanel.vue`

- [ ] **Step 1: 写前端失败测试**

将原“不得出现 overwrite”断言改为明确行为断言：

```ts
it("checks an existing target before start and requires explicit overwrite confirmation", () => {
  expect(source).toContain("tool:release-package:target-check");
  expect(source).toContain(
    "目标归档目录已存在。直接覆盖将完整替换其中的所有文件，此操作无法撤销。",
  );
  expect(source).toContain('confirmButtonText: "直接覆盖"');
  expect(source).toContain('cancelButtonText: "取消"');
  expect(source).toContain("overwriteExisting");
  expect(source).not.toContain('el-checkbox v-model="overwrite');
});
```

- [ ] **Step 2: 运行前端测试并确认失败**

Run: `pnpm --filter @lazycat/desktop exec vitest run src/components/ReleasePackagePanel.test.ts`

Expected: FAIL，目标检查和确认文案尚不存在。

- [ ] **Step 3: 实现启动前检查和确认**

在 `confirmStart` 中先获取：

```ts
const target = (await invokeToolByChannel("tool:release-package:target-check", {
  projectId,
  folderName: folderName.value,
})) as ReleasePackageTargetCheckResult;
let overwriteExisting = false;
if (target.exists) {
  try {
    await ElMessageBox.confirm(
      "目标归档目录已存在。直接覆盖将完整替换其中的所有文件，此操作无法撤销。",
      "目标归档目录已存在",
      {
        type: "warning",
        confirmButtonText: "直接覆盖",
        cancelButtonText: "取消",
      },
    );
  } catch {
    return;
  }
  overwriteExisting = true;
}
```

调用 `start` 时传递 `overwriteExisting`。用户取消二次确认时保持确认打包弹窗开启，并且不调用 `runtime.beginStart` 或 `start`。

- [ ] **Step 4: 运行前端上线包测试**

Run: `pnpm --filter @lazycat/desktop exec vitest run src/utils/releasePackage.test.ts src/components/ReleasePackagePanel.test.ts`

Expected: PASS。

### Task 5: 收口验证与经验记录

**Files:**

- Modify: `process.md`

- [ ] **Step 1: 记录覆盖提交经验**

在 `process.md` 增加本次场景、目标检查只负责交互、后端最终校验、staging/backup/final 三阶段切换及回滚边界。

- [ ] **Step 2: 运行类型检查**

Run: `pnpm typecheck`

Expected: PASS。

- [ ] **Step 3: 运行渲染层构建**

Run: `pnpm --filter @lazycat/desktop build:web`

Expected: PASS；只允许现有 chunk 大小警告。

- [ ] **Step 4: 检查差异质量**

Run: `git diff --check`

Expected: PASS，无空白错误。

- [ ] **Step 5: 提交功能改动**

仅暂存本计划涉及文件，避免带入工作区中请求转发等无关改动：

```powershell
git add -- apps/desktop/src-tauri/src/tools/release_package.rs apps/desktop/src-tauri/src/tools/release_package_archive.rs apps/desktop/src-tauri/src/tools/release_package_runtime.rs apps/desktop/src/bridge/tauri.ts apps/desktop/src/types/release-package.ts apps/desktop/src/components/ReleasePackagePanel.vue apps/desktop/src/components/ReleasePackagePanel.test.ts process.md docs/superpowers/plans/2026-07-19-release-package-overwrite.md
git commit -m "feat(release-package): 支持完整覆盖已有归档"
```
