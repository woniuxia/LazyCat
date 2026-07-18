# 上线包打包工具 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 LazyCat 中实现可维护多个项目、串行执行前后端 PowerShell 构建、实时显示日志并把产物可靠归档到“最近周四-项目名”目录的上线包打包工具。

**Architecture:** Vue 主从工作台负责配置与当前运行态，SQLite 表保存项目，全局归档根目录继续使用 `user_settings`。Rust 将数据库 CRUD、纯归档算法和长任务运行时拆成三个清晰单元；运行时通过带 `runId` 的 Tauri 事件发送日志和终态，先写同卷临时目录，全部成功后再重命名为最终目录。

**Tech Stack:** Vue 3、TypeScript、Element Plus、Vitest、Tauri 2、Rust、rusqlite、chrono、zip、walkdir、encoding_rs、PowerShell。

## Global Constraints

- 设计依据：`docs/superpowers/specs/2026-07-18-release-package-design.md`。
- Windows 优先；构建命令固定用 `powershell.exe -NoProfile -NonInteractive -Command`，不新增 Shell 插件。
- 项目信息保存到新表 `release_package_projects`；全局归档根目录保存到 `user_settings` 的 `release_package.output_root`。
- 周四当天取当天，其他日期取未来最近的周四；默认目录名固定为 `yyyyMMdd-项目名`，确认时允许修改。
- 前端、后端严格串行；任一失败或用户终止都不创建最终目录。
- 前端产物必须是目录，可保留目录本身或生成 ZIP；ZIP 内保留产物顶层目录。
- 后端产物可以是文件或目录，按源名称复制到归档根部；顶层名称冲突直接失败。
- 同名最终目录存在时直接失败，禁止覆盖、合并或自动追加序号。
- 同时只允许一个运行任务；不持久化运行历史或日志。
- 产物路径允许相对路径和绝对路径；相对路径分别以对应工程目录为基准解析。
- 仅使用现有 `zip`、`walkdir`、`encoding_rs`、`uuid` 等依赖，不新增第三方包。
- 不自动启动 `pnpm dev`；实现完成后执行针对性测试、类型检查、Web 构建和临时目录冒烟。
- 当前工作区可能包含用户未提交改动；每次提交只暂存任务列出的文件，禁止 `git add .`。

## File Map

- `apps/desktop/src/types/release-package.ts`：前端项目、表单、预检、事件和运行状态类型。
- `apps/desktop/src/utils/releasePackage.ts`：表单归一化、脏状态、事件过滤和有界日志纯函数。
- `apps/desktop/src/composables/useReleasePackageRuntime.ts`：跨面板挂载周期保留的当前运行态和 Tauri 监听器。
- `apps/desktop/src/components/ReleasePackagePanel.vue`：项目 CRUD、全局根目录、确认弹窗、执行日志 UI。
- `apps/desktop/src-tauri/src/tools/release_package.rs`：action 分发、项目 CRUD、`prepare` 和运行时入口。
- `apps/desktop/src-tauri/src/tools/release_package_archive.rs`：日期、路径、目录名、复制、ZIP 和临时目录提交。
- `apps/desktop/src-tauri/src/tools/release_package_runtime.rs`：唯一活动任务、PowerShell、日志解码、取消和事件。
- `apps/desktop/src-tauri/src/tools/helpers.rs`：初始化项目表。
- `apps/desktop/src-tauri/src/tools/mod.rs`：注册 domain、supported actions 和 AppHandle 分发。
- `apps/desktop/src/bridge/tauri.ts`、`apps/desktop/src/bridge/events.ts`、`apps/desktop/src-tauri/src/events.rs`：IPC 与事件契约。
- `apps/desktop/src/composables/toolCatalog.ts`、`apps/desktop/src/tool-registry.ts`：工具入口。
- `apps/desktop/src/composables/useSettings.ts`、`apps/desktop/src/composables/index.ts`：增加可等待落库的设置写入。

---

### Task 1: 前端领域类型与纯状态函数

**Files:**
- Create: `apps/desktop/src/types/release-package.ts`
- Modify: `apps/desktop/src/types/index.ts`
- Create: `apps/desktop/src/utils/releasePackage.ts`
- Test: `apps/desktop/src/utils/releasePackage.test.ts`

**Interfaces:**
- Produces: `ReleasePackageProject`、`ReleasePackageProjectDraft`、`ReleasePackagePrepareResult`、`ReleasePackageLogEvent`、`ReleasePackageStatusEvent`。
- Produces: `createEmptyReleasePackageDraft()`、`projectToReleasePackageDraft()`、`validateReleasePackageDraft()`、`isReleasePackageDraftDirty()`、`acceptReleasePackageEvent()`、`appendReleasePackageLog()`。
- Consumes: 无。

- [ ] **Step 1: 写纯函数失败测试**

创建 `apps/desktop/src/utils/releasePackage.test.ts`：

```ts
import { describe, expect, it } from "vitest";
import type { ReleasePackageLogEvent, ReleasePackageProject } from "../types/release-package";
import {
  acceptReleasePackageEvent,
  appendReleasePackageLog,
  createEmptyReleasePackageDraft,
  isReleasePackageDraftDirty,
  projectToReleasePackageDraft,
  validateReleasePackageDraft,
} from "./releasePackage";

const project: ReleasePackageProject = {
  id: 7,
  name: "客户门户",
  frontendProjectPath: "D:\\work\\portal-web",
  frontendBuildCommand: "pnpm build",
  frontendArtifactPath: "dist",
  frontendArtifactMode: "copy_directory",
  backendProjectPath: "D:\\work\\portal-server",
  backendBuildCommand: "mvn clean package -Pprod",
  backendArtifactPath: "target\\portal.jar",
  createdAt: "2026-07-18 10:00:00",
  updatedAt: "2026-07-18 10:00:00",
};

function log(runId: string, line: string): ReleasePackageLogEvent {
  return { runId, projectId: 7, phase: "frontend", stream: "stdout", line };
}

describe("release package view helpers", () => {
  it("creates a blank project draft with copy mode", () => {
    expect(createEmptyReleasePackageDraft()).toEqual({
      name: "",
      frontendProjectPath: "",
      frontendBuildCommand: "",
      frontendArtifactPath: "",
      frontendArtifactMode: "copy_directory",
      backendProjectPath: "",
      backendBuildCommand: "",
      backendArtifactPath: "",
    });
  });

  it("normalizes a project into an editable draft and detects dirty fields", () => {
    const draft = projectToReleasePackageDraft(project);
    expect(isReleasePackageDraftDirty(project, draft)).toBe(false);
    draft.frontendBuildCommand = "pnpm build:prod";
    expect(isReleasePackageDraftDirty(project, draft)).toBe(true);
    expect(isReleasePackageDraftDirty(null, createEmptyReleasePackageDraft())).toBe(false);
  });

  it("returns the first required field error", () => {
    const draft = createEmptyReleasePackageDraft();
    expect(validateReleasePackageDraft(draft)).toBe("请输入项目名");
    draft.name = "客户门户";
    expect(validateReleasePackageDraft(draft)).toBe("请选择前端工程目录");
  });

  it("accepts events only for the active run", () => {
    expect(acceptReleasePackageEvent("run-1", { runId: "run-1" })).toBe(true);
    expect(acceptReleasePackageEvent("run-1", { runId: "run-2" })).toBe(false);
    expect(acceptReleasePackageEvent(null, { runId: "run-1" })).toBe(false);
  });

  it("bounds logs without reordering accepted lines", () => {
    expect(appendReleasePackageLog([log("run-1", "a"), log("run-1", "b")], log("run-1", "c"), 2))
      .toEqual([log("run-1", "b"), log("run-1", "c")]);
  });
});
```

- [ ] **Step 2: 运行测试并确认失败**

Run:

```powershell
pnpm --filter @lazycat/desktop test -- src/utils/releasePackage.test.ts
```

Expected: FAIL，提示 `../types/release-package` 或 `./releasePackage` 不存在。

- [ ] **Step 3: 创建精确类型定义**

创建 `apps/desktop/src/types/release-package.ts`：

```ts
export type ReleasePackageArtifactMode = "copy_directory" | "zip_directory";
export type ReleasePackagePhase = "frontend" | "backend" | "archive";
export type ReleasePackageRunStatus = "idle" | "running" | "succeeded" | "failed" | "cancelled";

export interface ReleasePackageProjectDraft {
  name: string;
  frontendProjectPath: string;
  frontendBuildCommand: string;
  frontendArtifactPath: string;
  frontendArtifactMode: ReleasePackageArtifactMode;
  backendProjectPath: string;
  backendBuildCommand: string;
  backendArtifactPath: string;
}

export interface ReleasePackageProject extends ReleasePackageProjectDraft {
  id: number;
  createdAt: string;
  updatedAt: string;
}

export interface ReleasePackageProjectListResult { projects: ReleasePackageProject[] }

export interface ReleasePackagePrepareResult {
  defaultFolderName: string;
  outputRoot: string;
  archivePath: string;
  frontendArtifactMode: ReleasePackageArtifactMode;
}

export interface ReleasePackageStartResult { runId: string }
export interface ReleasePackageCancelResult { cancelRequested: boolean }

export interface ReleasePackageLogEvent {
  runId: string;
  projectId: number;
  phase: ReleasePackagePhase;
  stream: "stdout" | "stderr" | "system";
  line: string;
}

export interface ReleasePackageStatusEvent {
  runId: string;
  projectId: number;
  status: Exclude<ReleasePackageRunStatus, "idle">;
  phase: ReleasePackagePhase;
  archivePath?: string;
  error?: string;
}
```

在 `apps/desktop/src/types/index.ts` 末尾导出这些类型：

```ts
export type {
  ReleasePackageArtifactMode,
  ReleasePackagePhase,
  ReleasePackageRunStatus,
  ReleasePackageProjectDraft,
  ReleasePackageProject,
  ReleasePackageProjectListResult,
  ReleasePackagePrepareResult,
  ReleasePackageStartResult,
  ReleasePackageCancelResult,
  ReleasePackageLogEvent,
  ReleasePackageStatusEvent,
} from "./release-package";
```

- [ ] **Step 4: 实现最小纯函数**

创建 `apps/desktop/src/utils/releasePackage.ts`：

```ts
import type {
  ReleasePackageLogEvent,
  ReleasePackageProject,
  ReleasePackageProjectDraft,
} from "../types/release-package";

export function createEmptyReleasePackageDraft(): ReleasePackageProjectDraft {
  return {
    name: "",
    frontendProjectPath: "",
    frontendBuildCommand: "",
    frontendArtifactPath: "",
    frontendArtifactMode: "copy_directory",
    backendProjectPath: "",
    backendBuildCommand: "",
    backendArtifactPath: "",
  };
}

export function projectToReleasePackageDraft(project: ReleasePackageProject): ReleasePackageProjectDraft {
  return {
    name: project.name,
    frontendProjectPath: project.frontendProjectPath,
    frontendBuildCommand: project.frontendBuildCommand,
    frontendArtifactPath: project.frontendArtifactPath,
    frontendArtifactMode: project.frontendArtifactMode,
    backendProjectPath: project.backendProjectPath,
    backendBuildCommand: project.backendBuildCommand,
    backendArtifactPath: project.backendArtifactPath,
  };
}

export function normalizeReleasePackageDraft(draft: ReleasePackageProjectDraft): ReleasePackageProjectDraft {
  return Object.fromEntries(
    Object.entries(draft).map(([key, value]) => [key, typeof value === "string" ? value.trim() : value]),
  ) as unknown as ReleasePackageProjectDraft;
}

export function validateReleasePackageDraft(draft: ReleasePackageProjectDraft): string | null {
  const value = normalizeReleasePackageDraft(draft);
  if (!value.name) return "请输入项目名";
  if (!value.frontendProjectPath) return "请选择前端工程目录";
  if (!value.frontendBuildCommand) return "请输入前端构建命令";
  if (!value.frontendArtifactPath) return "请输入前端产物路径";
  if (!value.backendProjectPath) return "请选择后端工程目录";
  if (!value.backendBuildCommand) return "请输入后端构建命令";
  if (!value.backendArtifactPath) return "请输入后端产物路径";
  return null;
}

export function isReleasePackageDraftDirty(
  project: ReleasePackageProject | null,
  draft: ReleasePackageProjectDraft,
): boolean {
  if (!project) {
    return JSON.stringify(normalizeReleasePackageDraft(draft)) !== JSON.stringify(createEmptyReleasePackageDraft());
  }
  return JSON.stringify(projectToReleasePackageDraft(project)) !== JSON.stringify(normalizeReleasePackageDraft(draft));
}

export function acceptReleasePackageEvent(
  activeRunId: string | null,
  event: { runId: string },
): boolean {
  return activeRunId !== null && activeRunId === event.runId;
}

export function appendReleasePackageLog(
  current: ReleasePackageLogEvent[],
  event: ReleasePackageLogEvent,
  limit = 2_000,
): ReleasePackageLogEvent[] {
  const next = [...current, event];
  return next.length > limit ? next.slice(next.length - limit) : next;
}
```

- [ ] **Step 5: 运行测试并提交**

Run:

```powershell
pnpm --filter @lazycat/desktop test -- src/utils/releasePackage.test.ts
```

Expected: PASS，5 tests。

Commit:

```powershell
git add -- apps/desktop/src/types/release-package.ts apps/desktop/src/types/index.ts apps/desktop/src/utils/releasePackage.ts apps/desktop/src/utils/releasePackage.test.ts
git commit -m "feat(release-package): 定义项目与运行状态模型"
```

### Task 2: 数据库项目 CRUD 与打包预检

**Files:**
- Create: `apps/desktop/src-tauri/src/tools/release_package.rs`
- Create: `apps/desktop/src-tauri/src/tools/release_package_archive.rs`
- Modify: `apps/desktop/src-tauri/src/tools/helpers.rs`
- Modify: `apps/desktop/src-tauri/src/tools/mod.rs`
- Modify: `apps/desktop/src/bridge/tauri.ts`
- Test: inline Rust tests in both new modules

**Interfaces:**
- Consumes: `db_conn()`、`user_settings`。
- Produces: domain `release_package` actions `project_list/project_create/project_update/project_delete/prepare`。
- Produces: `ReleasePackageProjectConfig` and `PrepareResult` for the runtime task.
- Produces: `validate_folder_name()`、`default_folder_name()`、`resolve_artifact_path()` for later archive/runtime tasks.

- [ ] **Step 1: 注册模块骨架并写失败测试**

在 `tools/mod.rs` 声明 `pub mod release_package;` 与 `pub mod release_package_archive;`，但先不加入 dispatch。创建两个模块，先写以下测试：

```rust
// release_package_archive.rs
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use std::path::Path;

    #[test]
    fn thursday_is_inclusive_and_other_days_advance() {
        assert_eq!(default_folder_name(NaiveDate::from_ymd_opt(2026, 7, 23).unwrap(), "客户门户"), "20260723-客户门户");
        assert_eq!(default_folder_name(NaiveDate::from_ymd_opt(2026, 7, 24).unwrap(), "客户门户"), "20260730-客户门户");
    }

    #[test]
    fn folder_name_rejects_paths_and_windows_reserved_names() {
        for value in ["", ".", "..", "a/b", "a\\b", "CON", "LPT1.txt", "name.", "name "] {
            assert!(validate_folder_name(value).is_err(), "must reject {value:?}");
        }
        assert!(validate_folder_name("20260723-客户门户").is_ok());
    }

    #[test]
    fn artifact_paths_resolve_relative_to_project() {
        assert_eq!(resolve_artifact_path(Path::new(r"D:\work\web"), "dist"), Path::new(r"D:\work\web").join("dist"));
        assert_eq!(resolve_artifact_path(Path::new(r"D:\work\web"), r"E:\shared\dist"), Path::new(r"E:\shared\dist"));
    }
}

// release_package.rs
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use rusqlite::Connection;
    use serde_json::{json, Value};

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(RELEASE_PACKAGE_SCHEMA_SQL).unwrap();
        conn.execute_batch("CREATE TABLE user_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL, updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);").unwrap();
        conn
    }

    fn payload() -> Value {
        json!({
            "name": "客户门户",
            "frontendProjectPath": r"D:\work\web",
            "frontendBuildCommand": "pnpm build",
            "frontendArtifactPath": "dist",
            "frontendArtifactMode": "copy_directory",
            "backendProjectPath": r"D:\work\server",
            "backendBuildCommand": "mvn clean package -Pprod",
            "backendArtifactPath": r"target\portal.jar"
        })
    }

    #[test]
    fn project_crud_round_trip() {
        let conn = test_conn();
        let created = project_create_with_conn(&conn, &payload()).unwrap();
        let id = created["id"].as_i64().unwrap();
        let listed = project_list_with_conn(&conn).unwrap();
        assert_eq!(listed["projects"][0]["name"], "客户门户");
        let mut update = payload();
        update["id"] = json!(id);
        update["name"] = json!("客户门户 Pro");
        project_update_with_conn(&conn, &update).unwrap();
        assert_eq!(load_project(&conn, id).unwrap().name, "客户门户 Pro");
        project_delete_with_conn(&conn, &json!({ "id": id })).unwrap();
        assert!(load_project(&conn, id).is_err());
    }

    #[test]
    fn prepare_uses_global_output_root_and_inclusive_thursday() {
        let conn = test_conn();
        let id = project_create_with_conn(&conn, &payload()).unwrap()["id"].as_i64().unwrap();
        conn.execute("INSERT INTO user_settings(key, value) VALUES (?1, ?2)", [OUTPUT_ROOT_KEY, r"D:\releases"]).unwrap();
        let out = prepare_with_conn(&conn, id, NaiveDate::from_ymd_opt(2026, 7, 23).unwrap()).unwrap();
        assert_eq!(out["defaultFolderName"], "20260723-客户门户");
        assert_eq!(out["archivePath"], r"D:\releases\20260723-客户门户");
    }
}
```

- [ ] **Step 2: 运行 Rust 测试并确认失败**

Run from `apps/desktop/src-tauri`:

```powershell
cargo test release_package -- --nocapture
```

Expected: FAIL，缺少 schema、CRUD、日期和路径函数。

- [ ] **Step 3: 实现日期、目录名和产物路径基础函数**

在 `release_package_archive.rs` 实现以下接口；保留设备名判断必须取第一个 `.` 前的 basename 并做 ASCII 大写比较：

```rust
use chrono::{Datelike, Days, NaiveDate, Weekday};
use std::path::{Path, PathBuf};

pub fn validate_folder_name(raw: &str) -> Result<(), String> {
    if raw.is_empty() || raw.trim() != raw || matches!(raw, "." | "..") {
        return Err("归档目录名不能为空，且不能包含首尾空格、`.` 或 `..`".into());
    }
    if raw.chars().any(|ch| ch < '\u{20}' || "<>:\"/\\|?*".contains(ch)) {
        return Err("归档目录名包含 Windows 不允许的字符".into());
    }
    if raw.ends_with('.') || raw.ends_with(' ') || Path::new(raw).components().count() != 1 {
        return Err("归档目录名必须是单级 Windows 文件夹名".into());
    }
    let stem = raw.split('.').next().unwrap_or("").to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4 && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'));
    if reserved { return Err("归档目录名不能使用 Windows 保留设备名".into()); }
    Ok(())
}

pub fn default_folder_name(today: NaiveDate, project_name: &str) -> String {
    let today_num = today.weekday().num_days_from_monday();
    let thursday_num = Weekday::Thu.num_days_from_monday();
    let offset = (thursday_num + 7 - today_num) % 7;
    let date = today.checked_add_days(Days::new(offset.into())).expect("valid next Thursday");
    format!("{}-{project_name}", date.format("%Y%m%d"))
}

pub fn resolve_artifact_path(project_path: &Path, artifact_path: &str) -> PathBuf {
    let artifact = PathBuf::from(artifact_path);
    if artifact.is_absolute() { artifact } else { project_path.join(artifact) }
}
```

- [ ] **Step 4: 实现 schema、项目映射和 CRUD**

在 `release_package.rs` 定义 schema、action 和配置结构：

```rust
use chrono::{Local, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Value};

use super::helpers::db_conn;
use super::release_package_archive::{default_folder_name, validate_folder_name};

pub const OUTPUT_ROOT_KEY: &str = "release_package.output_root";
pub const RELEASE_PACKAGE_SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS release_package_projects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    frontend_project_path TEXT NOT NULL,
    frontend_build_command TEXT NOT NULL,
    frontend_artifact_path TEXT NOT NULL,
    frontend_artifact_mode TEXT NOT NULL CHECK (frontend_artifact_mode IN ('copy_directory', 'zip_directory')),
    backend_project_path TEXT NOT NULL,
    backend_build_command TEXT NOT NULL,
    backend_artifact_path TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
"#;

const ACTIONS: &[&str] = &["project_list", "project_create", "project_update", "project_delete", "prepare"];

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleasePackageProjectConfig {
    pub id: i64,
    pub name: String,
    pub frontend_project_path: String,
    pub frontend_build_command: String,
    pub frontend_artifact_path: String,
    pub frontend_artifact_mode: String,
    pub backend_project_path: String,
    pub backend_build_command: String,
    pub backend_artifact_path: String,
    pub created_at: String,
    pub updated_at: String,
}
```

实现 `parse_project_payload`，对八个字符串字段执行 `trim()` 和非空校验，对项目名调用 `validate_folder_name`，对 mode 只接受两个枚举值。实现：

```rust
fn project_list_with_conn(conn: &Connection) -> Result<Value, String>;
fn project_create_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String>;
fn project_update_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String>;
fn project_delete_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String>;
pub(crate) fn load_project(conn: &Connection, id: i64) -> Result<ReleasePackageProjectConfig, String>;
fn prepare_with_conn(conn: &Connection, project_id: i64, today: NaiveDate) -> Result<Value, String>;
```

CRUD SQL 固定为：

```sql
SELECT id, name, frontend_project_path, frontend_build_command, frontend_artifact_path,
       frontend_artifact_mode, backend_project_path, backend_build_command,
       backend_artifact_path, created_at, updated_at
FROM release_package_projects
ORDER BY name COLLATE NOCASE ASC, id ASC;

INSERT INTO release_package_projects(
    name, frontend_project_path, frontend_build_command, frontend_artifact_path,
    frontend_artifact_mode, backend_project_path, backend_build_command, backend_artifact_path
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8);

UPDATE release_package_projects SET
    name=?1, frontend_project_path=?2, frontend_build_command=?3,
    frontend_artifact_path=?4, frontend_artifact_mode=?5,
    backend_project_path=?6, backend_build_command=?7, backend_artifact_path=?8,
    updated_at=CURRENT_TIMESTAMP
WHERE id=?9;

DELETE FROM release_package_projects WHERE id=?1;
```

`prepare_with_conn` 必须读取 `OUTPUT_ROOT_KEY`，调用 `default_folder_name(today, &project.name)` 和 `validate_folder_name`，返回：

```rust
Ok(json!({
    "defaultFolderName": folder_name,
    "outputRoot": output_root,
    "archivePath": PathBuf::from(&output_root).join(&folder_name).to_string_lossy(),
    "frontendArtifactMode": project.frontend_artifact_mode,
}))
```

- [ ] **Step 5: 接入 schema、dispatch 和前端 channel**

在 `helpers.rs` 的两个独立 domain schema 初始化之后增加：

```rust
conn.execute_batch(super::release_package::RELEASE_PACKAGE_SCHEMA_SQL)
    .map_err(|e| format!("create release package schema failed: {e}"))?;
```

在 `release_package.rs` 暴露：

```rust
#[cfg(test)]
pub(crate) fn supported_actions() -> &'static [&'static str] { ACTIONS }

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    if !ACTIONS.contains(&action) { return Err(format!("unsupported release_package action: {action}")); }
    let conn = db_conn()?;
    match action {
        "project_list" => project_list_with_conn(&conn),
        "project_create" => project_create_with_conn(&conn, payload),
        "project_update" => project_update_with_conn(&conn, payload),
        "project_delete" => project_delete_with_conn(&conn, payload),
        "prepare" => {
            let id = payload["projectId"].as_i64().ok_or("projectId is required")?;
            prepare_with_conn(&conn, id, Local::now().date_naive())
        }
        _ => unreachable!(),
    }
}
```

同步 `tools/mod.rs` 的 `dispatch_tool`、`supported_actions` 和 `contract_tests.rs` 的 `DOMAINS`。在 `bridge/tauri.ts` 增加五条一行式映射，action 使用 snake_case：

```ts
"tool:release-package:project-list": { domain: "release_package", action: "project_list" },
"tool:release-package:project-create": { domain: "release_package", action: "project_create" },
"tool:release-package:project-update": { domain: "release_package", action: "project_update" },
"tool:release-package:project-delete": { domain: "release_package", action: "project_delete" },
"tool:release-package:prepare": { domain: "release_package", action: "prepare" },
```

- [ ] **Step 6: 运行测试并提交**

Run:

```powershell
cargo test release_package -- --nocapture
cargo test tools::contract_tests:: -- --nocapture
```

Expected: PASS；CRUD、周四规则、目录名、路径解析和 channel/action 契约全部通过。

Commit:

```powershell
git add -- apps/desktop/src-tauri/src/tools/release_package.rs apps/desktop/src-tauri/src/tools/release_package_archive.rs apps/desktop/src-tauri/src/tools/helpers.rs apps/desktop/src-tauri/src/tools/mod.rs apps/desktop/src-tauri/src/tools/contract_tests.rs apps/desktop/src/bridge/tauri.ts
git commit -m "feat(release-package): 添加项目配置与打包预检"
```

### Task 3: 事务式产物归档引擎

**Files:**
- Modify: `apps/desktop/src-tauri/src/tools/release_package_archive.rs`
- Test: inline Rust tests in `release_package_archive.rs`

**Interfaces:**
- Consumes: 已解析的前后端产物路径。
- Produces: `ArchiveRequest`、`ArchiveError`、`archive_artifacts()`。
- Guarantee: 只有最终归档成功时才出现最终目录；错误或取消自动删除本次临时目录。

- [ ] **Step 1: 写归档失败测试**

在 archive 模块新增 `TestDir`（`std::env::temp_dir()` + `Uuid::new_v4()`，`Drop` 只删除自己的目录），并加入：

```rust
#[test]
fn copy_mode_keeps_source_directory_and_backend_file() {
    let root = TestDir::new();
    let frontend = root.0.join("dist");
    let backend = root.0.join("portal.jar");
    let output = root.0.join("output");
    fs::create_dir_all(&frontend).unwrap();
    fs::create_dir_all(&output).unwrap();
    fs::write(frontend.join("index.html"), "ok").unwrap();
    fs::write(&backend, "jar").unwrap();
    let result = archive_artifacts(&ArchiveRequest {
        frontend_artifact: frontend, frontend_mode: "copy_directory".into(),
        backend_artifact: backend, output_root: output,
        folder_name: "20260723-客户门户".into(), run_id: "run-copy".into(),
    }, &AtomicBool::new(false), |_| {}).unwrap();
    assert!(result.join("dist/index.html").is_file());
    assert!(result.join("portal.jar").is_file());
}

#[test]
fn zip_mode_keeps_frontend_directory_as_zip_root() {
    let root = TestDir::new();
    let frontend = root.0.join("dist");
    let backend = root.0.join("server.jar");
    let output = root.0.join("output");
    fs::create_dir_all(frontend.join("assets")).unwrap();
    fs::create_dir_all(&output).unwrap();
    fs::write(frontend.join("assets/app.js"), "js").unwrap();
    fs::write(&backend, "jar").unwrap();
    let result = archive_artifacts(&ArchiveRequest {
        frontend_artifact: frontend, frontend_mode: "zip_directory".into(),
        backend_artifact: backend, output_root: output,
        folder_name: "20260723-客户门户".into(), run_id: "run-zip".into(),
    }, &AtomicBool::new(false), |_| {}).unwrap();
    let mut zip = ZipArchive::new(fs::File::open(result.join("dist.zip")).unwrap()).unwrap();
    assert!(zip.by_name("dist/assets/app.js").is_ok());
}

#[test]
fn collision_and_cancel_never_create_final_directory() {
    let root = TestDir::new();
    let frontend = root.0.join("dist");
    let backend = root.0.join("backend/DIST");
    let output = root.0.join("output");
    fs::create_dir_all(&frontend).unwrap();
    fs::create_dir_all(&backend).unwrap();
    fs::create_dir_all(&output).unwrap();
    let error = archive_artifacts(&ArchiveRequest {
        frontend_artifact: frontend.clone(), frontend_mode: "copy_directory".into(),
        backend_artifact: backend, output_root: output.clone(),
        folder_name: "20260723-客户门户".into(), run_id: "run-collision".into(),
    }, &AtomicBool::new(false), |_| {}).unwrap_err();
    assert!(matches!(error, ArchiveError::Failed(message) if message.contains("名称冲突")));
    let cancelled = AtomicBool::new(true);
    let error = archive_artifacts(&ArchiveRequest {
        frontend_artifact: frontend, frontend_mode: "copy_directory".into(),
        backend_artifact: root.0.join("missing.jar"), output_root: output.clone(),
        folder_name: "20260723-另一个项目".into(), run_id: "run-cancel".into(),
    }, &cancelled, |_| {}).unwrap_err();
    assert!(matches!(error, ArchiveError::Cancelled));
    assert!(!output.join("20260723-客户门户").exists());
    assert!(!output.join("20260723-另一个项目").exists());
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test release_package_archive -- --nocapture`

Expected: FAIL，缺少 `ArchiveRequest`、`ArchiveError` 和 `archive_artifacts`。

- [ ] **Step 3: 实现受控临时目录和归档入口**

```rust
#[derive(Debug)]
pub enum ArchiveError { Cancelled, Failed(String) }

pub struct ArchiveRequest {
    pub frontend_artifact: PathBuf,
    pub frontend_mode: String,
    pub backend_artifact: PathBuf,
    pub output_root: PathBuf,
    pub folder_name: String,
    pub run_id: String,
}

struct StagingGuard { path: PathBuf, committed: bool }
impl Drop for StagingGuard {
    fn drop(&mut self) { if !self.committed { let _ = fs::remove_dir_all(&self.path); } }
}

fn check_cancel(cancelled: &AtomicBool) -> Result<(), ArchiveError> {
    if cancelled.load(Ordering::Acquire) { Err(ArchiveError::Cancelled) } else { Ok(()) }
}

pub fn archive_artifacts(
    request: &ArchiveRequest,
    cancelled: &AtomicBool,
    mut emit: impl FnMut(&str),
) -> Result<PathBuf, ArchiveError> {
    validate_folder_name(&request.folder_name).map_err(ArchiveError::Failed)?;
    check_cancel(cancelled)?;
    if !request.output_root.is_dir() { return Err(ArchiveError::Failed("全局归档根目录不存在".into())); }
    if !request.frontend_artifact.is_dir() { return Err(ArchiveError::Failed("前端产物必须是文件夹".into())); }
    if !request.backend_artifact.exists() { return Err(ArchiveError::Failed("后端产物不存在".into())); }
    let frontend_name = source_name(&request.frontend_artifact)?;
    let frontend_target = match request.frontend_mode.as_str() {
        "copy_directory" => frontend_name.clone(),
        "zip_directory" => format!("{frontend_name}.zip"),
        _ => return Err(ArchiveError::Failed("未知的前端产物处理模式".into())),
    };
    let backend_target = source_name(&request.backend_artifact)?;
    if frontend_target.eq_ignore_ascii_case(&backend_target) {
        return Err(ArchiveError::Failed(format!("前后端归档名称冲突：{frontend_target}")));
    }
    let final_path = request.output_root.join(&request.folder_name);
    if final_path.exists() { return Err(ArchiveError::Failed("目标归档目录已存在".into())); }
    let staging_path = request.output_root.join(format!(".lazycat-release-package-{}.tmp", request.run_id));
    if staging_path.exists() { return Err(ArchiveError::Failed("本次运行临时目录已存在".into())); }
    fs::create_dir(&staging_path).map_err(|e| ArchiveError::Failed(format!("创建归档临时目录失败：{e}")))?;
    let mut guard = StagingGuard { path: staging_path.clone(), committed: false };
    emit("正在归档前端产物");
    if request.frontend_mode == "zip_directory" {
        zip_directory_with_root(&request.frontend_artifact, &staging_path.join(frontend_target), cancelled)?;
    } else { copy_path_with_root(&request.frontend_artifact, &staging_path, cancelled)?; }
    emit("正在归档后端产物");
    copy_path_with_root(&request.backend_artifact, &staging_path, cancelled)?;
    check_cancel(cancelled)?;
    if final_path.exists() { return Err(ArchiveError::Failed("目标归档目录在执行期间被创建".into())); }
    fs::rename(&staging_path, &final_path).map_err(|e| ArchiveError::Failed(format!("提交最终归档目录失败：{e}")))?;
    guard.committed = true;
    Ok(final_path)
}
```

- [ ] **Step 4: 实现复制和 ZIP 细节**

实现 `source_name`、`copy_path_with_root`、`zip_directory_with_root`。文件复制到 `destination_root/<source basename>`；目录复制保留 basename；每个 WalkDir entry 和 64 KiB 文件读写循环前检查取消。ZIP entry 使用 `/` 分隔并保留源目录第一层，使用现有 `zip::CompressionMethod::Deflated`，不设置密码。所有 IO 错误必须包含源和目标路径。

```rust
let file = File::create(destination_zip)
    .map_err(|e| ArchiveError::Failed(format!("创建 ZIP 失败：{e}")))?;
let mut writer = zip::ZipWriter::new(file);
let options = FileOptions::default()
    .compression_method(zip::CompressionMethod::Deflated)
    .unix_permissions(0o644);
let root_name = source_name(source)?;
let mut buffer = [0_u8; 64 * 1024];
for entry in WalkDir::new(source) {
    check_cancel(cancelled)?;
    let entry = entry.map_err(|e| ArchiveError::Failed(format!("遍历 ZIP 源目录失败：{e}")))?;
    let relative = entry.path().strip_prefix(source)
        .map_err(|e| ArchiveError::Failed(format!("计算 ZIP 相对路径失败：{e}")))?;
    let suffix = relative.to_string_lossy().replace('\\', "/");
    let name = if suffix.is_empty() { root_name.clone() } else { format!("{root_name}/{suffix}") };
    if entry.file_type().is_dir() {
        writer.add_directory(format!("{name}/"), options)
            .map_err(|e| ArchiveError::Failed(format!("写入 ZIP 目录失败：{e}")))?;
        continue;
    }
    writer.start_file(name, options)
        .map_err(|e| ArchiveError::Failed(format!("写入 ZIP 文件头失败：{e}")))?;
    let mut reader = BufReader::new(File::open(entry.path())
        .map_err(|e| ArchiveError::Failed(format!("读取 ZIP 源文件失败：{e}")))?);
    loop {
        check_cancel(cancelled)?;
        let size = reader.read(&mut buffer)
            .map_err(|e| ArchiveError::Failed(format!("读取 ZIP 源文件失败：{e}")))?;
        if size == 0 { break; }
        writer.write_all(&buffer[..size])
            .map_err(|e| ArchiveError::Failed(format!("写入 ZIP 失败：{e}")))?;
    }
}
writer.finish().map_err(|e| ArchiveError::Failed(format!("完成 ZIP 失败：{e}")))?;
```

- [ ] **Step 5: 运行测试并提交**

Run: `cargo test release_package_archive -- --nocapture`

Expected: PASS，复制、ZIP 顶层、名称冲突和取消清理全部通过。

```powershell
git add -- apps/desktop/src-tauri/src/tools/release_package_archive.rs
git commit -m "feat(release-package): 实现事务式产物归档"
```

### Task 4: PowerShell 运行时、实时事件与终止

**Files:**
- Create: `apps/desktop/src-tauri/src/tools/release_package_runtime.rs`
- Modify: `apps/desktop/src-tauri/src/tools/release_package.rs`
- Modify: `apps/desktop/src-tauri/src/tools/mod.rs`
- Modify: `apps/desktop/src-tauri/src/events.rs`
- Modify: `apps/desktop/src/bridge/events.ts`
- Modify: `apps/desktop/src/bridge/tauri.ts`
- Modify: `apps/desktop/src-tauri/src/main.rs`
- Test: inline Rust tests in `release_package_runtime.rs`

**Interfaces:**
- Consumes: `ReleasePackageProjectConfig`、`resolve_artifact_path()`、`archive_artifacts()`、`tauri::AppHandle`。
- Produces: actions `start/cancel`，events `release-package://log`、`release-package://status`，以及 `on_app_exit()`。
- Guarantee: 全应用最多一个 `ActiveRun`；start 返回 `{ runId }`，cancel 返回 `{ cancelRequested }`。

- [ ] **Step 1: 写命令、编码和取消失败测试**

```rust
#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    #[test]
    fn powershell_reports_both_streams_and_nonzero_exit() {
        let logs = Arc::new(Mutex::new(Vec::new()));
        let sink = logs.clone();
        let result = run_powershell(
            &std::env::temp_dir(),
            "Write-Output 'front-ok'; [Console]::Error.WriteLine('front-err'); exit 7",
            Arc::new(AtomicBool::new(false)),
            Arc::new(Mutex::new(None)),
            Arc::new(move |stream, line| sink.lock().unwrap().push((stream.to_string(), line))),
        );
        assert!(matches!(result, Err(CommandError::ExitCode(7))));
        let lines = logs.lock().unwrap();
        assert!(lines.iter().any(|(_, line)| line.contains("front-ok")));
        assert!(lines.iter().any(|(_, line)| line.contains("front-err")));
    }

    #[test]
    fn powershell_process_tree_can_be_cancelled() {
        let cancel = Arc::new(AtomicBool::new(false));
        let pid = Arc::new(Mutex::new(None));
        let handle = {
            let cancel = cancel.clone();
            let pid = pid.clone();
            thread::spawn(move || run_powershell(
                &std::env::temp_dir(), "Start-Sleep -Seconds 30", cancel, pid, Arc::new(|_, _| {}),
            ))
        };
        thread::sleep(Duration::from_millis(500));
        cancel.store(true, Ordering::Release);
        if let Some(value) = *pid.lock().unwrap() { terminate_process_tree(value).unwrap(); }
        assert!(matches!(handle.join().unwrap(), Err(CommandError::Cancelled)));
    }

    #[test]
    fn decoder_prefers_utf8_then_falls_back_to_gbk() {
        assert_eq!(decode_console_line("构建成功\r\n".as_bytes()), "构建成功");
        let (gbk, _, _) = encoding_rs::GBK.encode("构建成功\r\n");
        assert_eq!(decode_console_line(&gbk), "构建成功");
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test release_package_runtime -- --nocapture`

Expected: FAIL，缺少 runner、decoder 和 process-tree 终止函数。

- [ ] **Step 3: 实现可测试的 PowerShell runner**

```rust
#[derive(Debug)]
enum CommandError { Cancelled, ExitCode(i32), Spawn(String), Wait(String) }

fn decode_console_line(bytes: &[u8]) -> String {
    let bytes = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    let bytes = bytes.strip_suffix(b"\r").unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(value) => value.to_string(),
        Err(_) => encoding_rs::GBK.decode(bytes).0.into_owned(),
    }
}

fn run_powershell(
    cwd: &Path,
    command: &str,
    cancelled: Arc<AtomicBool>,
    pid_slot: Arc<Mutex<Option<u32>>>,
    emit: Arc<dyn Fn(&'static str, String) + Send + Sync>,
) -> Result<(), CommandError>;
```

实现要求：

- `Command::new("powershell.exe")`，参数依次为 `-NoProfile`、`-NonInteractive`、`-Command`、完整命令文本，`current_dir(cwd)`。
- stdout/stderr 使用 `Stdio::piped()`，Windows 使用 `CREATE_NO_WINDOW`；spawn 后立刻保存 `child.id()`。
- 两个 reader thread 分别用 `BufReader::read_until(b'\n', ...)`，经 `decode_console_line` 后标记 `stdout`/`stderr`。
- 主循环每 100ms `try_wait()`；取消标记为 true 时终止进程树、等待 child 并返回 `Cancelled`。
- child 退出后 join reader，清空 `pid_slot`；非零返回 `ExitCode(code.unwrap_or(-1))`。
- 非 Windows 版本返回“当前仅支持 Windows PowerShell 打包”，但必须保持跨平台编译通过。

Windows 终止函数：

```rust
fn terminate_process_tree(pid: u32) -> Result<(), String> {
    let output = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("终止构建进程失败：{e}"))?;
    if output.status.success() { Ok(()) } else {
        Err(format!("终止构建进程失败：{}", decode_console_line(&output.stderr)))
    }
}
```

- [ ] **Step 4: 实现唯一活动任务和事件 payload**

在 `events.rs` 增加并加入 `ALL`：

```rust
pub const EVENT_RELEASE_PACKAGE_LOG: &str = "release-package://log";
pub const EVENT_RELEASE_PACKAGE_STATUS: &str = "release-package://status";
```

在 `bridge/events.ts` 增加：

```ts
RELEASE_PACKAGE_LOG: "release-package://log",
RELEASE_PACKAGE_STATUS: "release-package://status",
```

runtime 定义：

```rust
struct ActiveRun {
    run_id: String,
    cancelled: Arc<AtomicBool>,
    pid: Arc<Mutex<Option<u32>>>,
}
static ACTIVE_RUN: OnceLock<Mutex<Option<ActiveRun>>> = OnceLock::new();

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LogEvent { run_id: String, project_id: i64, phase: String, stream: String, line: String }

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusEvent {
    run_id: String, project_id: i64, status: String, phase: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    archive_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

trait EventSink: Send + Sync {
    fn log(&self, event: LogEvent);
    fn status(&self, event: StatusEvent);
}

struct TauriEventSink { app: tauri::AppHandle }

#[derive(Debug)]
enum PipelineError {
    Cancelled,
    Failed { phase: &'static str, message: String },
}

fn run_pipeline(
    run_id: &str,
    project: ReleasePackageProjectConfig,
    output_root: PathBuf,
    folder_name: String,
    cancelled: Arc<AtomicBool>,
    pid: Arc<Mutex<Option<u32>>>,
    sink: Arc<dyn EventSink>,
) -> Result<PathBuf, PipelineError>;

pub fn start(app: &tauri::AppHandle, project: ReleasePackageProjectConfig, output_root: PathBuf, folder_name: String) -> Result<Value, String>;
pub fn cancel(run_id: &str) -> Result<Value, String>;
pub fn on_app_exit();
```

`start` 在锁内拒绝已有任务，生成 UUID 并登记 cancel/pid，随后 spawn thread。线程必须严格执行前端 PowerShell、后端 PowerShell、`archive_artifacts`；每阶段先发 `running` 状态和 system log。终态映射固定为：非零退出或归档错误发 `failed`，任意取消发 `cancelled`，成功发 `succeeded` 并携带最终 `archivePath`。线程结束时只在 `run_id` 仍匹配时清空 `ACTIVE_RUN`。

`cancel` 先核对 runId，再设置取消标记并对当前 pid 执行一次 best-effort `taskkill`；runner 观察到标记后即使进程已被 cancel action 结束，也必须稳定返回 `CommandError::Cancelled`，不能把第二次 taskkill 的“进程不存在”转成失败。

- [ ] **Step 5: 接入 start/cancel 和应用退出**

把 `ACTIONS` 扩展为：

```rust
const ACTIONS: &[&str] = &[
    "project_list", "project_create", "project_update", "project_delete",
    "prepare", "start", "cancel",
];
```

扩展 `ACTIONS` 后同步修改无 AppHandle 的 `execute`：`start/cancel` 分支返回 `release_package action requires app context`，不得继续落入 `unreachable!()`。

在 `release_package.rs` 增加：

```rust
pub fn execute_with_app(action: &str, payload: &Value, app: &tauri::AppHandle) -> Result<Value, String> {
    match action {
        "start" => {
            let project_id = payload["projectId"].as_i64().ok_or("projectId is required")?;
            let folder_name = payload["folderName"].as_str().ok_or("folderName is required")?.to_string();
            validate_folder_name(&folder_name)?;
            let conn = db_conn()?;
            let project = load_project(&conn, project_id)?;
            let output_root = load_output_root(&conn)?;
            validate_run_inputs(&project, &output_root, &folder_name)?;
            super::release_package_runtime::start(app, project, output_root.into(), folder_name)
        }
        "cancel" => {
            let run_id = payload["runId"].as_str().ok_or("runId is required")?;
            super::release_package_runtime::cancel(run_id)
        }
        _ => execute(action, payload),
    }
}
```

`validate_run_inputs` 要求 output root、前端工程、后端工程均为目录，并在构建前检查最终目录不存在；产物留到各构建成功后检查。`tools/mod.rs` 增加 runtime module，并在 `execute_tool_with_app` 将 `release_package` 转发到 `execute_with_app`。bridge 增加：

```ts
"tool:release-package:start": { domain: "release_package", action: "start" },
"tool:release-package:cancel": { domain: "release_package", action: "cancel" },
```

在 `main.rs` 的 `RunEvent::ExitRequested` 增加：

```rust
tools::release_package_runtime::on_app_exit();
```

- [ ] **Step 6: 运行测试并提交**

```powershell
cargo test release_package_runtime -- --nocapture
cargo test release_package -- --nocapture
cargo test tools::contract_tests:: -- --nocapture
```

Expected: PASS，且测试结束后没有残留 PowerShell 子进程。

```powershell
git add -- apps/desktop/src-tauri/src/tools/release_package_runtime.rs apps/desktop/src-tauri/src/tools/release_package.rs apps/desktop/src-tauri/src/tools/mod.rs apps/desktop/src-tauri/src/events.rs apps/desktop/src/bridge/events.ts apps/desktop/src/bridge/tauri.ts apps/desktop/src-tauri/src/main.rs
git commit -m "feat(release-package): 添加构建运行时与终止控制"
```

### Task 5: 可等待设置写入与前端单例运行态

**Files:**
- Modify: `apps/desktop/src/composables/useSettings.ts`
- Modify: `apps/desktop/src/composables/index.ts`
- Test: `apps/desktop/src/composables/useSettings.test.ts`
- Create: `apps/desktop/src/composables/useReleasePackageRuntime.ts`
- Test: `apps/desktop/src/composables/useReleasePackageRuntime.test.ts`

**Interfaces:**
- Produces: `setSettingAndWait(key, value)`，保证内存值更新且 SQLite 写入完成后才 resolve。
- Consumes: Task 1 的 event filtering/log helpers，Task 4 的事件和 cancel action。
- Produces: renderer 生命周期内唯一的 `useReleasePackageRuntime()` 状态，切换工具面板不会丢失当前日志。

- [ ] **Step 1: 写设置和运行态失败测试**

`useSettings.test.ts`：

```ts
import { beforeEach, describe, expect, it, vi } from "vitest";
const invokeToolByChannel = vi.fn();
vi.mock("../bridge/tauri", () => ({ invokeToolByChannel }));
import { getSetting, setSettingAndWait } from "./useSettings";

describe("setSettingAndWait", () => {
  beforeEach(() => invokeToolByChannel.mockReset().mockResolvedValue({ ok: true }));
  it("updates memory and waits for SQLite persistence", async () => {
    await setSettingAndWait("release_package.output_root", "D:\\releases");
    expect(getSetting("release_package.output_root")).toBe("D:\\releases");
    expect(invokeToolByChannel).toHaveBeenCalledWith("tool:settings:set", {
      key: "release_package.output_root", value: "D:\\releases",
    });
  });
  it("restores the previous in-memory value when persistence fails", async () => {
    await setSettingAndWait("release_package.output_root", "D:\\old");
    invokeToolByChannel.mockRejectedValueOnce(new Error("write failed"));
    await expect(setSettingAndWait("release_package.output_root", "D:\\new")).rejects.toThrow("write failed");
    expect(getSetting("release_package.output_root")).toBe("D:\\old");
  });
});
```

`useReleasePackageRuntime.test.ts` 测试导出的纯状态工厂，不调用真实 Tauri：

```ts
import { describe, expect, it } from "vitest";
import { createReleasePackageRuntimeState, reduceReleasePackageStatus } from "./useReleasePackageRuntime";

describe("release package runtime state", () => {
  it("binds the first event while start is pending and rejects stale runs", () => {
    const state = createReleasePackageRuntimeState();
    state.pendingProjectId = 7;
    reduceReleasePackageStatus(state, {
      runId: "run-1", projectId: 7, status: "running", phase: "frontend",
    });
    expect(state.activeRunId).toBe("run-1");
    reduceReleasePackageStatus(state, {
      runId: "old-run", projectId: 7, status: "failed", phase: "backend", error: "old",
    });
    expect(state.status).toBe("running");
  });

  it("keeps the final archive path on success", () => {
    const state = createReleasePackageRuntimeState();
    state.activeRunId = "run-1";
    reduceReleasePackageStatus(state, {
      runId: "run-1", projectId: 7, status: "succeeded", phase: "archive",
      archivePath: "D:\\releases\\20260723-客户门户",
    });
    expect(state.status).toBe("succeeded");
    expect(state.archivePath).toContain("20260723-客户门户");
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

Run:

```powershell
pnpm --filter @lazycat/desktop test -- src/composables/useSettings.test.ts src/composables/useReleasePackageRuntime.test.ts
```

Expected: FAIL，缺少 awaited setter 和 runtime state。

- [ ] **Step 3: 实现可等待设置写入**

在 `useSettings.ts` 增加并让原 `setSetting` 复用同一底层逻辑：

```ts
export async function setSettingAndWait(key: string, value: string): Promise<void> {
  const previous = settings[key];
  settings[key] = value;
  try {
    await invokeToolByChannel("tool:settings:set", { key, value });
  } catch (error) {
    if (previous === undefined) delete settings[key];
    else settings[key] = previous;
    throw error;
  }
}

export function setSetting(key: string, value: string): void {
  void setSettingAndWait(key, value).catch(() => {
    // 保持既有 fire-and-forget 语义；需要确认落库的调用方使用 setSettingAndWait。
  });
}
```

从 `composables/index.ts` 导出 `setSettingAndWait`。不要在面板中先 fire-and-forget 再立即 `prepare`，否则 Rust 可能读取到旧归档根目录。

- [ ] **Step 4: 实现跨面板挂载周期的单例运行态**

`useReleasePackageRuntime.ts` 的 module-level refs 和公开接口固定为：

```ts
import { computed, reactive, ref, toRefs } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { APP_EVENTS } from "../bridge/events";
import { invokeToolByChannel } from "../bridge/tauri";
import type {
  ReleasePackageLogEvent, ReleasePackagePhase, ReleasePackageRunStatus,
  ReleasePackageStatusEvent,
} from "../types/release-package";
import { acceptReleasePackageEvent, appendReleasePackageLog } from "../utils/releasePackage";

export interface ReleasePackageRuntimeState {
  activeRunId: string | null;
  activeProjectId: number | null;
  pendingProjectId: number | null;
  status: ReleasePackageRunStatus;
  phase: ReleasePackagePhase | null;
  archivePath: string;
  error: string;
}

export function createReleasePackageRuntimeState(): ReleasePackageRuntimeState {
  return { activeRunId: null, activeProjectId: null, pendingProjectId: null,
    status: "idle", phase: null, archivePath: "", error: "" };
}

export function reduceReleasePackageStatus(
  state: ReleasePackageRuntimeState,
  event: ReleasePackageStatusEvent,
): void {
  if (!state.activeRunId && state.pendingProjectId === event.projectId) state.activeRunId = event.runId;
  if (!acceptReleasePackageEvent(state.activeRunId, event)) return;
  state.activeProjectId = event.projectId;
  state.status = event.status;
  state.phase = event.phase;
  state.archivePath = event.archivePath ?? state.archivePath;
  state.error = event.error ?? "";
  if (event.status !== "running") state.pendingProjectId = null;
}
```

单例 composable 必须提供：

```ts
ensureListeners(): Promise<void>
beginStart(projectId: number): void
bindStartedRun(runId: string, projectId: number): void
abortStart(message: string): void
cancel(): Promise<void>
reset(): void
```

`ensureListeners` 只注册一次两个 APP_EVENTS listener。log listener 在 pending project 首个事件到达时也允许绑定 `activeRunId`，之后只接受相同 runId，并用 `appendReleasePackageLog(..., 2_000)`。终态不清空 runId/log，直到下一次 `beginStart` 或显式 reset；这样切换到其他工具再回来仍能看到本次运行结果。

module scope 中创建 `const state = reactive(createReleasePackageRuntimeState())` 与 `const logs = ref<ReleasePackageLogEvent[]>([])`。`beginStart` 必须清空旧日志/路径/错误，设置 pending project 和 `status="running"`；`abortStart` 清掉 pending/active run 并设为 `failed`；`cancel` 只在 activeRunId 存在时调用后端。composable 返回 ref，保证面板中的 `.value` 用法一致：

```ts
export function useReleasePackageRuntime() {
  return {
    ...toRefs(state),
    logs,
    isRunning: computed(() => state.status === "running"),
    ensureListeners,
    beginStart,
    bindStartedRun,
    abortStart,
    cancel,
    reset,
  };
}
```

- [ ] **Step 5: 运行测试并提交**

```powershell
pnpm --filter @lazycat/desktop test -- src/composables/useSettings.test.ts src/composables/useReleasePackageRuntime.test.ts src/utils/releasePackage.test.ts
```

Expected: PASS。

```powershell
git add -- apps/desktop/src/composables/useSettings.ts apps/desktop/src/composables/index.ts apps/desktop/src/composables/useSettings.test.ts apps/desktop/src/composables/useReleasePackageRuntime.ts apps/desktop/src/composables/useReleasePackageRuntime.test.ts
git commit -m "feat(release-package): 管理前端打包运行状态"
```

### Task 6: 主从工作台 UI 与工具入口

**Files:**
- Create: `apps/desktop/src/components/ReleasePackagePanel.vue`
- Test: `apps/desktop/src/components/ReleasePackagePanel.test.ts`
- Modify: `apps/desktop/src/composables/toolCatalog.ts`
- Modify: `apps/desktop/src/composables/toolCatalog.test.ts`
- Modify: `apps/desktop/src/tool-registry.ts`

**Interfaces:**
- Consumes: Task 1 类型/纯函数、Task 2 CRUD/prepare、Task 4 start/cancel、Task 5 awaited setting/runtime。
- Produces: tool ID `release-package`，左项目列表、右编辑表单、确认 Dialog、当前日志区。

- [ ] **Step 1: 写入口和组件结构失败测试**

```ts
// ReleasePackagePanel.test.ts
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
const source = readFileSync(new URL("./ReleasePackagePanel.vue", import.meta.url), "utf8");

describe("ReleasePackagePanel", () => {
  it("uses a master-detail workspace and explicit run confirmation", () => {
    expect(source).toContain('class="release-package-projects"');
    expect(source).toContain('class="release-package-editor"');
    expect(source).toContain('class="release-package-log"');
    expect(source).toContain("确认打包");
    expect(source).toContain("终止打包");
  });
  it("uses all release-package actions and awaited global setting persistence", () => {
    for (const channel of ["project-list", "project-create", "project-update", "project-delete", "prepare", "start"]) {
      expect(source).toContain(`tool:release-package:${channel}`);
    }
    expect(source).toContain("setSettingAndWait");
    expect(source).toContain("useReleasePackageRuntime");
  });
  it("does not persist logs or silently overwrite archives", () => {
    expect(source).not.toContain("localStorage");
    expect(source).not.toContain("overwrite");
  });
});
```

在 `toolCatalog.test.ts` 新增：

```ts
it("registers the release package tool", () => {
  expect(getAllTools()).toContainEqual(expect.objectContaining({ id: "release-package", name: "上线包打包" }));
  expect(isRealToolId("release-package")).toBe(true);
});
```

- [ ] **Step 2: 运行测试确认失败**

Run:

```powershell
pnpm --filter @lazycat/desktop test -- src/components/ReleasePackagePanel.test.ts src/composables/toolCatalog.test.ts
```

Expected: FAIL，组件和工具入口不存在。

- [ ] **Step 3: 注册工具入口**

在 `toolCatalog.ts` 的“更多工具”中加入：

```ts
{ id: "release-package", name: "上线包打包", desc: "按项目构建前后端并归档上线产物" },
```

在 `tool-registry.ts` 加入：

```ts
"release-package": defineAsyncComponent(() => import("./components/ReleasePackagePanel.vue")),
```

- [ ] **Step 4: 实现页面状态和 CRUD**

`ReleasePackagePanel.vue` 使用 `script setup`，状态固定为：

```ts
const projects = ref<ReleasePackageProject[]>([]);
const selectedId = ref<number | null>(null);
const draft = reactive<ReleasePackageProjectDraft>(createEmptyReleasePackageDraft());
const outputRoot = ref("");
const loading = ref(false);
const saving = ref(false);
const confirmVisible = ref(false);
const prepareResult = ref<ReleasePackagePrepareResult | null>(null);
const folderName = ref("");
const selectedProject = computed(() => projects.value.find((item) => item.id === selectedId.value) ?? null);
const dirty = computed(() => isReleasePackageDraftDirty(selectedProject.value, draft));
const runtime = useReleasePackageRuntime();
const running = computed(() => runtime.status.value === "running");
```

`onMounted` 必须先 `await initSettings()`，再把 `getSetting("release_package.output_root") ?? ""` 写入 `outputRoot`，随后 `await runtime.ensureListeners()` 和 `await loadProjects()`；加载失败显式显示错误。组件卸载时不取消 Rust 任务，也不注销 singleton listener，保证切换工具后当前运行态仍继续。

实现以下行为：

- `loadProjects()` 调 `project-list`；保持仍存在的 selectedId，否则选择首项；空列表进入新建态。
- `selectProject(project)` 在脏表单时先用 `ElMessageBox.confirm` 确认放弃，再 `Object.assign(draft, projectToReleasePackageDraft(project))`。
- `newProject()` 复用同一脏状态确认并加载空 draft。
- `saveProject()` 先用 `validateReleasePackageDraft`，再按 selectedId 调 create/update，成功后 reload 并选中新 id。
- `deleteProject()` 二次确认，调用 delete 后 reload；文案明确“只删除配置，不删除工程或归档文件”。
- 三个目录选择使用 `@tauri-apps/plugin-dialog`：全局根目录、前端工程、后端工程均 `directory: true, multiple: false`；产物路径保留文本输入以支持相对路径、文件和目录。
- 全局根目录选择成功后必须 `await setSettingAndWait("release_package.output_root", path)`，成功后才更新成功提示。

- [ ] **Step 5: 实现确认、执行、取消和日志 UI**

开始流程必须是：

```ts
async function prepareStart() {
  if (!selectedProject.value || dirty.value) {
    ElMessage.warning(dirty.value ? "请先保存项目配置" : "请先选择项目");
    return;
  }
  prepareResult.value = await invokeToolByChannel("tool:release-package:prepare", {
    projectId: selectedProject.value.id,
  }) as ReleasePackagePrepareResult;
  folderName.value = prepareResult.value.defaultFolderName;
  confirmVisible.value = true;
}

async function confirmStart() {
  const projectId = selectedProject.value?.id;
  if (!projectId) return;
  await runtime.ensureListeners();
  runtime.beginStart(projectId);
  try {
    const result = await invokeToolByChannel("tool:release-package:start", {
      projectId, folderName: folderName.value,
    }) as ReleasePackageStartResult;
    runtime.bindStartedRun(result.runId, projectId);
    confirmVisible.value = false;
  } catch (error) {
    runtime.abortStart(error instanceof Error ? error.message : String(error));
    ElMessage.error(runtime.error.value);
  }
}
```

确认 Dialog 必须实时显示 `outputRoot + folderName` 的完整路径，不允许输入路径分隔符；最终合法性仍由 Rust 校验。执行中锁定项目列表、表单、全局路径、保存和开始按钮，只保留“终止打包”。日志每行显示阶段、stream 和正文，stderr 使用危险色，容器用 `aria-live="polite"`；新日志到达时仅在用户已接近底部时自动滚动，避免阅读历史时被抢走位置。成功态提供：

```ts
await invokeToolByChannel("tool:system:open-local-path", { path: runtime.archivePath.value });
```

- [ ] **Step 6: 实现已确认的紧凑浅色布局**

模板层级固定为：顶部 `.release-package-toolbar`；主体两列 `.release-package-workspace`；左侧 `.release-package-projects` 宽 220px；右侧 `.release-package-editor`；表单按“基本信息/前端工程/后端工程”用 divider 分组；日志 `.release-package-log` 固定最小高度 180px。使用 `--lc-*` 与 Element Plus 变量，不修改全局 Element Plus 覆盖文件；在 `max-width: 960px` 时改为单列，项目列表横向滚动。

核心 CSS：

```css
.release-package-panel { display: flex; flex-direction: column; gap: 12px; min-height: 0; }
.release-package-toolbar { display: flex; align-items: center; gap: 8px; }
.release-package-root { flex: 1; min-width: 0; }
.release-package-workspace { display: grid; grid-template-columns: 220px minmax(0, 1fr); min-height: 0; border-top: 1px solid var(--lc-border); }
.release-package-projects { padding: 12px 12px 12px 0; border-right: 1px solid var(--lc-border); }
.release-package-editor { min-width: 0; padding: 12px 0 0 16px; }
.release-package-log { min-height: 180px; max-height: 320px; overflow: auto; padding: 12px; color: #d7dae0; background: #1f2329; font: 12px/1.6 var(--lc-font-mono); }
@media (max-width: 960px) {
  .release-package-workspace { grid-template-columns: 1fr; }
  .release-package-projects { display: flex; gap: 8px; overflow-x: auto; border-right: 0; border-bottom: 1px solid var(--lc-border); }
  .release-package-editor { padding-left: 0; }
}
```

- [ ] **Step 7: 运行前端测试、类型检查并提交**

```powershell
pnpm --filter @lazycat/desktop test -- src/components/ReleasePackagePanel.test.ts src/composables/toolCatalog.test.ts src/composables/useReleasePackageRuntime.test.ts src/utils/releasePackage.test.ts
pnpm typecheck
```

Expected: PASS，无新增 TypeScript 错误。

```powershell
git add -- apps/desktop/src/components/ReleasePackagePanel.vue apps/desktop/src/components/ReleasePackagePanel.test.ts apps/desktop/src/composables/toolCatalog.ts apps/desktop/src/composables/toolCatalog.test.ts apps/desktop/src/tool-registry.ts
git commit -m "feat(release-package): 添加上线包打包工作台"
```

### Task 7: 全链路回归、最小冒烟与经验沉淀

**Files:**
- Modify: `apps/desktop/src-tauri/src/tools/release_package_runtime.rs`（仅增加 pipeline 集成测试）
- Modify: `process.md`
- Inspect only: `apps/desktop/components.d.ts`（构建可能自动补全新组件；不得覆盖现有用户改动）

**Interfaces:**
- Consumes: Tasks 1-6 全部产物。
- Produces: 可重复执行的成功/失败 pipeline 测试、完整验证证据、`process.md` 经验记录。

- [ ] **Step 1: 增加不依赖真实项目的 pipeline 冒烟测试**

复用 Task 4 已定义的 `EventSink`，生产实现 `TauriEventSink` 调 `app.emit`，测试实现只收集状态：

```rust
struct TestDir(PathBuf);
impl TestDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("lazycat-release-runtime-{}", Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for TestDir { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }

#[derive(Default)]
struct CollectingSink { statuses: Mutex<Vec<StatusEvent>> }
impl EventSink for CollectingSink {
    fn log(&self, _event: LogEvent) {}
    fn status(&self, event: StatusEvent) { self.statuses.lock().unwrap().push(event); }
}
impl CollectingSink {
    fn phases(&self) -> Vec<String> {
        self.statuses.lock().unwrap().iter()
            .filter(|event| event.status == "running")
            .map(|event| event.phase.clone())
            .fold(Vec::new(), |mut phases, phase| {
                if phases.last() != Some(&phase) { phases.push(phase); }
                phases
            })
    }
}

#[test]
fn pipeline_builds_frontend_then_backend_and_archives_both() {
    let root = TestDir::new();
    let frontend_project = root.0.join("web");
    let backend_project = root.0.join("server");
    let output_root = root.0.join("output");
    fs::create_dir_all(&frontend_project).unwrap();
    fs::create_dir_all(&backend_project).unwrap();
    fs::create_dir_all(&output_root).unwrap();
    let project = ReleasePackageProjectConfig {
        id: 1,
        name: "冒烟项目".into(),
        frontend_project_path: frontend_project.to_string_lossy().into_owned(),
        frontend_build_command: "New-Item -ItemType Directory -Force dist | Out-Null; Set-Content dist/index.html web".into(),
        frontend_artifact_path: "dist".into(),
        frontend_artifact_mode: "copy_directory".into(),
        backend_project_path: backend_project.to_string_lossy().into_owned(),
        backend_build_command: "New-Item -ItemType Directory -Force target | Out-Null; Set-Content target/app.jar jar".into(),
        backend_artifact_path: "target/app.jar".into(),
        created_at: String::new(),
        updated_at: String::new(),
    };
    let sink = Arc::new(CollectingSink::default());
    let result = run_pipeline(
        "smoke-run", project, output_root.clone(), "20260723-冒烟项目".into(),
        Arc::new(AtomicBool::new(false)), Arc::new(Mutex::new(None)), sink.clone(),
    ).unwrap();
    assert!(result.join("dist/index.html").is_file());
    assert!(result.join("app.jar").is_file());
    assert_eq!(sink.phases(), vec!["frontend", "backend", "archive"]);
}

#[test]
fn failed_frontend_never_runs_backend_or_creates_final_directory() {
    let root = TestDir::new();
    let frontend_project = root.0.join("web");
    let backend_project = root.0.join("server");
    let output_root = root.0.join("output");
    fs::create_dir_all(&frontend_project).unwrap();
    fs::create_dir_all(&backend_project).unwrap();
    fs::create_dir_all(&output_root).unwrap();
    let project = ReleasePackageProjectConfig {
        id: 2,
        name: "冒烟项目".into(),
        frontend_project_path: frontend_project.to_string_lossy().into_owned(),
        frontend_build_command: "exit 9".into(),
        frontend_artifact_path: "dist".into(),
        frontend_artifact_mode: "copy_directory".into(),
        backend_project_path: backend_project.to_string_lossy().into_owned(),
        backend_build_command: "Set-Content marker.txt should-not-run".into(),
        backend_artifact_path: "marker.txt".into(),
        created_at: String::new(),
        updated_at: String::new(),
    };
    let error = run_pipeline(
        "failed-run", project, output_root.clone(), "20260723-冒烟项目".into(),
        Arc::new(AtomicBool::new(false)), Arc::new(Mutex::new(None)),
        Arc::new(CollectingSink::default()),
    ).unwrap_err();
    assert!(matches!(error, PipelineError::Failed { phase, .. } if phase == "frontend"));
    assert!(!backend_project.join("marker.txt").exists());
    assert!(!output_root.join("20260723-冒烟项目").exists());
}
```

- [ ] **Step 2: 运行所有针对性测试**

Run from repository root：

```powershell
cargo test release_package --manifest-path apps/desktop/src-tauri/Cargo.toml -- --nocapture
cargo test tools::contract_tests:: --manifest-path apps/desktop/src-tauri/Cargo.toml -- --nocapture
pnpm --filter @lazycat/desktop test -- src/utils/releasePackage.test.ts src/composables/useSettings.test.ts src/composables/useReleasePackageRuntime.test.ts src/components/ReleasePackagePanel.test.ts src/composables/toolCatalog.test.ts
```

Expected: 全部 PASS；PowerShell 取消测试执行后用 `Get-Process powershell` 只检查测试前后新增 PID，不能误杀用户原有 PowerShell。

- [ ] **Step 3: 执行格式、类型、Rust 和 Web 构建验证**

Run:

```powershell
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
git diff --check
```

Expected: 全部 exit 0。若 `build:web` 首次出现已知 `spawn EPERM`，按项目规则先重试一次；仍失败再申请提升权限重试。不要启动 dev server。

- [ ] **Step 4: 检查生成文件和工作区边界**

Run:

```powershell
git status --short
git diff -- apps/desktop/components.d.ts
git diff -- process.md
```

Expected:

- `components.d.ts` 如新增 `ReleasePackagePanel` 属于自动生成预期；若文件还含任务开始前已有改动，不得整体暂存或还原，最终报告说明。
- 逐项核对没有修改 `resources/manuals/**`、版本号、release 脚本或其他无关模块。
- 目标目录冲突、失败、取消三个路径均没有残留 `.lazycat-release-package-*.tmp`。

- [ ] **Step 5: 按项目规则记录 process.md**

先读现有 `git diff -- process.md`，再用 `apply_patch` 在文件末尾追加以下结构，不改写已有段落：

```markdown
## 2026-07-18: 上线包打包采用构建与归档两阶段提交

**场景**: 外部项目需要在桌面工具中串行执行前后端构建，并把不同形态的产物合并归档到可编辑的周四日期目录。

**问题**:
1. 构建命令、产物复制和 UI 日志如果分散到前端，会形成多份运行状态并难以可靠终止子进程。
2. 直接向最终目录复制会在失败或取消后留下看似可用的不完整上线包。
3. 面板切换会卸载 Vue 组件，组件局部状态无法持续接收长任务事件。

**方案**:
1. Rust 统一编排 PowerShell、stdout/stderr、进程树取消和产物归档；所有事件绑定唯一 runId。
2. 前后端均成功后才写同卷临时目录，复制/ZIP 全部完成后原子重命名；失败由 staging guard 清理。
3. 前端运行态放在 module-level composable，日志有界保留，旧 runId 事件不能覆盖当前任务。

**涉及文件**:
- `apps/desktop/src-tauri/src/tools/release_package*.rs`
- `apps/desktop/src/components/ReleasePackagePanel.vue`
- `apps/desktop/src/composables/useReleasePackageRuntime.ts`

**验证**:
- `cargo test release_package --manifest-path apps/desktop/src-tauri/Cargo.toml -- --nocapture`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`
```

如果 `process.md` 仍包含任务开始前的未提交改动，本任务不得把整份文件加入提交；保留追加内容并在交付说明中明确。若文件已干净，可单独提交：

```powershell
git add -- process.md
git commit -m "docs(process): 记录上线包打包编排实践"
```

- [ ] **Step 6: 提交 pipeline 测试并执行完成前复核**

```powershell
git add -- apps/desktop/src-tauri/src/tools/release_package_runtime.rs
git commit -m "test(release-package): 覆盖完整构建归档流程"
git status --short
git log -6 --oneline
```

Expected: 功能提交完整；剩余 dirty 文件只能是任务开始前的用户改动、未单独提交的 `process.md` 追加或预期生成文件。完成声明前必须使用 `superpowers:verification-before-completion` 复核最新命令输出；重大偏差先修正并重新验证，不把静态阅读当成运行验证。
