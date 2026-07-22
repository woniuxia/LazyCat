# Release Package Types Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将上线包重构为互斥的本地归档与服务器上传两种类型，使上传直接读取构建产物且完全不依赖本地归档目录。

**Architecture:** 在项目配置中以 `package_type` 作为唯一行为真值；运行时先并行构建并产出 `BuiltTarget`，再分别进入本地 `ArchiveSession` 或远端 `ArtifactManifest`/SFTP 交付分支。上传失败重试保存源产物清单并在新预检后重建部署目标，不保存或依赖 `archivePath`。

**Tech Stack:** Vue 3、TypeScript、Vitest、Element Plus、Tauri 2、Rust、rusqlite、ssh2/SFTP。

---

## 文件边界

- `apps/desktop/src-tauri/src/tools/release_package.rs`：项目配置、SQLite 迁移、IPC 参数解析和类型专属门禁。
- `apps/desktop/src-tauri/src/tools/release_package_runtime.rs`：共用构建、归档/上传分支、终态聚合和上传重试。
- `apps/desktop/src-tauri/src/tools/release_package_deploy.rs`：产物清单校验和远端事务；只补足直接源清单测试，不改变 SFTP 事务。
- `apps/desktop/src/types/release-package.ts`：前端判别类型和 IPC 数据契约。
- `apps/desktop/src/utils/releasePackage.ts`：草稿默认值、归一化、按类型校验和状态文案。
- `apps/desktop/src/components/ReleasePackagePanel.vue`：类型控件、条件表单和类型专属启动流程。
- `apps/desktop/src/composables/useReleasePackageRuntime.ts`：无归档路径终态的前端状态归并。
- `apps/desktop/src-tauri/src/global_notification.rs`：按类型输出本地归档或服务器上传终态文案。
- `apps/desktop/src/types/global-notification.ts`：全局通知携带打包类型。
- `apps/desktop/src/utils/globalNotification.ts`：校验通知中的打包类型并生成类型化文案。
- `apps/desktop/src/components/GlobalNotificationPopup.vue`：展示类型化终态文案。
- `docs/experience/release-package.md`：替换已经失效的“先归档再上传”经验。

### Task 1: 项目打包类型与数据库迁移

**Files:**
- Modify: `apps/desktop/src-tauri/src/tools/release_package.rs`

- [ ] **Step 1: 写失败测试，锁定迁移和类型校验**

在 `release_package.rs` 测试模块新增：

```rust
#[test]
fn migration_maps_legacy_upload_flag_to_package_type_once() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(LEGACY_SCHEMA_WITH_UPLOAD_FLAG).unwrap();
    conn.execute(
        "INSERT INTO release_package_projects(
            name, output_root, frontend_project_path, frontend_build_command,
            frontend_artifact_path, frontend_artifact_mode, backend_project_path,
            backend_build_command, backend_artifact_path, upload_enabled
         ) VALUES ('local', 'D:\\release', 'D:\\web', 'pnpm build', 'dist',
                   'copy_directory', 'D:\\server', 'mvn package', 'target/app.jar', 0)",
        [],
    ).unwrap();
    let local_id = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO release_package_projects(
            name, output_root, frontend_project_path, frontend_build_command,
            frontend_artifact_path, frontend_artifact_mode, backend_project_path,
            backend_build_command, backend_artifact_path, upload_enabled
         ) VALUES ('upload', '', 'D:\\web', 'pnpm build', 'dist',
                   'copy_directory', 'D:\\server', 'mvn package', 'target/app.jar', 1)",
        [],
    ).unwrap();
    let upload_id = conn.last_insert_rowid();

    ensure_schema(&conn).unwrap();

    assert_eq!(load_project(&conn, local_id).unwrap().package_type, ReleasePackageType::LocalArchive);
    assert_eq!(load_project(&conn, upload_id).unwrap().package_type, ReleasePackageType::ServerUpload);
    conn.execute(
        "UPDATE release_package_projects SET package_type='local_archive' WHERE id=?1",
        [upload_id],
    ).unwrap();
    ensure_schema(&conn).unwrap();
    assert_eq!(load_project(&conn, upload_id).unwrap().package_type, ReleasePackageType::LocalArchive);
}

#[test]
fn server_upload_allows_empty_output_root_but_requires_remote_config() {
    let mut payload = payload();
    payload["packageType"] = json!("server_upload");
    payload["outputRoot"] = json!("");
    assert!(parse_project_payload(&payload).is_ok());

    payload["sshHost"] = json!("");
    assert_eq!(parse_project_payload(&payload).unwrap_err(), "sshHost is required for server_upload");
}

#[test]
fn local_archive_requires_output_root_without_remote_config() {
    let mut payload = payload();
    payload["packageType"] = json!("local_archive");
    payload["outputRoot"] = json!("");
    assert_eq!(parse_project_payload(&payload).err().unwrap(), "outputRoot is required for local_archive");
    payload["outputRoot"] = json!(r"D:\releases");
    assert!(parse_project_payload(&payload).is_ok());
}
```

- [ ] **Step 2: 运行测试并确认按预期失败**

Run: `cargo test release_package::tests::migration_maps_legacy_upload_flag_to_package_type_once -- --nocapture`

Expected: FAIL，原因是 `ReleasePackageType` / `package_type` 尚不存在。

- [ ] **Step 3: 实现唯一类型真值和幂等迁移**

核心类型和迁移形态：

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleasePackageType {
    LocalArchive,
    ServerUpload,
}

impl ReleasePackageType {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "local_archive" => Ok(Self::LocalArchive),
            "server_upload" => Ok(Self::ServerUpload),
            _ => Err("packageType must be local_archive or server_upload".into()),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::LocalArchive => "local_archive",
            Self::ServerUpload => "server_upload",
        }
    }
}
```

`ensure_schema` 先读取列集合；仅当 `package_type` 不存在时执行 `ALTER TABLE`，随后一次性执行：

```sql
UPDATE release_package_projects
SET package_type = CASE
  WHEN upload_enabled = 1 THEN 'server_upload'
  ELSE 'local_archive'
END
```

测试模块新增完整的 `LEGACY_SCHEMA_WITH_UPLOAD_FLAG` 常量，字段与当前旧表一致但不含 `package_type`，避免依赖不存在的测试 helper。

将 `ReleasePackageProjectConfig` / `ProjectPayload`、SELECT、INSERT、UPDATE 和行映射改用 `package_type`。保留旧列但不再读写 `upload_enabled`。`parse_project_payload` 使用 `optional_string` 读取 `outputRoot`，再按类型执行专属校验。

- [ ] **Step 4: 运行 release_package 配置测试**

Run: `cargo test release_package::tests -- --nocapture`

Expected: PASS，迁移第二次运行不会覆盖已经修改的 `package_type`。

- [ ] **Step 5: 提交配置迁移**

```text
git add apps/desktop/src-tauri/src/tools/release_package.rs
git commit -m "refactor(release-package): 增加打包类型配置"
```

### Task 2: 前端类型与按类型校验

**Files:**
- Modify: `apps/desktop/src/types/release-package.ts`
- Modify: `apps/desktop/src/utils/releasePackage.ts`
- Modify: `apps/desktop/src/utils/releasePackage.test.ts`

- [ ] **Step 1: 写失败测试，表达两种草稿行为**

把测试项目的 `uploadEnabled` 改为 `packageType: "local_archive"`，并新增：

```ts
it("defaults new projects to local archive", () => {
  expect(createEmptyReleasePackageDraft().packageType).toBe("local_archive");
});

it("validates only fields required by the selected package type", () => {
  const draft = createCompleteDraft();
  draft.packageType = "server_upload";
  draft.outputRoot = "";
  expect(validateReleasePackageDraft(draft)).toBeNull();

  draft.sshHost = "";
  expect(validateReleasePackageDraft(draft)).toBe("请输入服务器地址");

  draft.packageType = "local_archive";
  expect(validateReleasePackageDraft(draft)).toBe("请选择归档根目录");
});

it("labels upload failure as build succeeded and upload failed", () => {
  expect(releasePackageRunStatusLabel("package_succeeded_upload_failed"))
    .toBe("构建完成，上传失败");
});
```

- [ ] **Step 2: 运行前端 helper 测试并确认失败**

Run: `pnpm --filter @lazycat/desktop test -- src/utils/releasePackage.test.ts`

Expected: FAIL，缺少 `packageType` 且旧校验仍强制 `outputRoot`。

- [ ] **Step 3: 实现判别类型和校验**

在类型文件中定义：

```ts
export type ReleasePackageType = "local_archive" | "server_upload";

export interface ReleasePackageUploadConfig {
  sshHost: string;
  sshPort: number;
  sshUsername: string;
  sshAuthType: ReleasePackageSshAuthType;
  sshPrivateKeyPath: string;
  frontendRemoteDir: string;
  backendRemotePath: string;
}

export interface ReleasePackageProjectDraft extends ReleasePackageUploadConfig {
  packageType: ReleasePackageType;
  outputRoot: string;
  // 保留现有构建字段
}

export type ReleasePackagePrepareResult =
  | { packageType: "local_archive"; defaultFolderName: string; outputRoot: string; archivePath: string }
  | { packageType: "server_upload" };
```

删除 `ReleasePackageStartMode` 和 `uploadEnabled`。`validateReleasePackageUpload` 不再自行判断开关，由 `validateReleasePackageDraft` 仅在 `packageType === "server_upload"` 时调用；`outputRoot` 只在本地归档时要求。

- [ ] **Step 4: 运行 helper 测试与类型检查**

Run: `pnpm --filter @lazycat/desktop test -- src/utils/releasePackage.test.ts`

Expected: PASS。

Run: `pnpm --filter @lazycat/desktop typecheck`

Expected: FAIL 仅来自尚未迁移的 `ReleasePackagePanel.vue` 旧字段，记录错误位置后进入 Task 3。

- [ ] **Step 5: 提交前端契约**

```text
git add apps/desktop/src/types/release-package.ts apps/desktop/src/utils/releasePackage.ts apps/desktop/src/utils/releasePackage.test.ts
git commit -m "refactor(release-package): 按打包类型校验项目"
```

### Task 3: 面板类型控件与专属启动流程

**Files:**
- Modify: `apps/desktop/src/components/ReleasePackagePanel.vue`
- Modify: `apps/desktop/src/components/ReleasePackagePanel.test.ts`

- [ ] **Step 1: 写失败的组件契约测试**

将旧 `uploadEnabled` / `startMode` 断言替换为：

```ts
it("renders mutually exclusive package types and type-specific fields", () => {
  expect(source).toContain('v-model="draft.packageType"');
  expect(source).toContain('value="local_archive"');
  expect(source).toContain('value="server_upload"');
  expect(source).toContain('v-if="draft.packageType === \'local_archive\'"');
  expect(source).toContain('v-if="draft.packageType === \'server_upload\'"');
  expect(source).not.toContain("draft.uploadEnabled");
  expect(source).not.toContain("startMode");
});

it("checks only the delivery target for the selected type", () => {
  const start = source.slice(source.indexOf("async function confirmStart"));
  expect(start).toContain('prepareResult.value?.packageType === "local_archive"');
  expect(start).toContain("confirmArchiveOverwrite");
  expect(start).toContain("runUploadPreflight");
  expect(start.indexOf("confirmArchiveOverwrite")).not.toBe(start.indexOf("runUploadPreflight"));
  expect(source).not.toContain("mode: startMode.value");
});
```

- [ ] **Step 2: 运行组件测试并确认失败**

Run: `pnpm --filter @lazycat/desktop test -- src/components/ReleasePackagePanel.test.ts`

Expected: FAIL，页面仍使用上传开关和单次启动模式。

- [ ] **Step 3: 实现项目类型 UI 和确认框分支**

在基础配置使用 `el-radio-group` 分段控件：

```vue
<el-form-item label="打包类型" required>
  <el-radio-group v-model="draft.packageType" :disabled="running" class="package-type-group">
    <el-radio-button value="local_archive">本地归档</el-radio-button>
    <el-radio-button value="server_upload">上传服务器</el-radio-button>
  </el-radio-group>
</el-form-item>
```

归档根目录外层增加 `v-if="draft.packageType === 'local_archive'"`；服务器配置只在 `server_upload` 显示。删除默认上传开关和启动模式切换。

`prepareStart` 始终调用 `prepare` 并使用判别结果。本地类型设置 `folderName`；上传类型不读取归档字段。`confirmStart` 明确分支：

```ts
const packageType = prepareResult.value?.packageType;
if (packageType === "local_archive") {
  const folderNameError = validateArchiveFolderName(folderName.value);
  if (folderNameError) throw new Error(folderNameError);
  const decision = await confirmArchiveOverwrite(projectId);
  if (decision === null) return;
  overwriteExisting = decision;
} else if (packageType === "server_upload") {
  if (!(await runUploadPreflight(projectId, selectedTargets.value))) return;
} else {
  throw new Error("打包类型无效，请重新打开确认窗口");
}
```

启动 payload 只为本地类型发送 `folderName` / `overwriteExisting`，只为上传类型发送 `preflightToken` / `overwriteRemoteTargets`。打开归档入口继续以运行时真实 `archivePath` 为条件。

- [ ] **Step 4: 运行面板测试和类型检查**

Run: `pnpm --filter @lazycat/desktop test -- src/components/ReleasePackagePanel.test.ts src/utils/releasePackage.test.ts`

Expected: PASS。

Run: `pnpm --filter @lazycat/desktop typecheck`

Expected: PASS。

- [ ] **Step 5: 提交面板改造**

```text
git add apps/desktop/src/components/ReleasePackagePanel.vue apps/desktop/src/components/ReleasePackagePanel.test.ts
git commit -m "refactor(release-package): 分离两种打包交互"
```

### Task 4: Rust 类型专属 IPC 门禁

**Files:**
- Modify: `apps/desktop/src-tauri/src/tools/release_package.rs`

- [ ] **Step 1: 写失败测试覆盖 prepare/start/action 门禁**

新增测试：

```rust
#[test]
fn prepare_returns_a_discriminated_result_without_archive_for_upload() {
    let conn = seeded_conn_with_type(ReleasePackageType::ServerUpload, "");
    let out = prepare_with_conn(&conn, 1, NaiveDate::from_ymd_opt(2026, 7, 23).unwrap()).unwrap();
    assert_eq!(out, json!({ "packageType": "server_upload" }));
}

#[test]
fn start_input_rejects_parameters_from_the_other_package_type() {
    assert!(parse_start_input(
        ReleasePackageType::LocalArchive,
        &json!({ "folderName": "release", "preflightToken": "token" }),
    ).is_err());
    assert!(parse_start_input(
        ReleasePackageType::ServerUpload,
        &json!({ "folderName": "release", "preflightToken": "token" }),
    ).is_err());
}
```

- [ ] **Step 2: 运行测试并确认失败**

Run: `cargo test release_package::tests::prepare_returns_a_discriminated_result_without_archive_for_upload -- --nocapture`

Expected: FAIL，`prepare` 仍强制归档根目录。

- [ ] **Step 3: 实现判别 prepare 和 StartInput**

定义内部输入：

```rust
enum ReleaseStartInput {
    LocalArchive { folder_name: String, overwrite_existing: bool },
    ServerUpload {
        preflight_token: String,
        overwrite_remote_targets: Vec<ReleaseTarget>,
    },
}
```

`prepare_with_conn` 在上传类型只返回 `{ packageType: "server_upload" }`。`target_check`、远端 probe/preflight、`start` 和 `upload_retry` 在加载项目后调用类型断言。`start` 删除 `mode` 解析，消费 `ReleaseStartInput` 后只校验对应目录或预检绑定。

- [ ] **Step 4: 运行 Rust 配置与 action 测试**

Run: `cargo test release_package::tests -- --nocapture`

Expected: PASS。

- [ ] **Step 5: 提交 IPC 门禁**

```text
git add apps/desktop/src-tauri/src/tools/release_package.rs
git commit -m "refactor(release-package): 按类型解析启动参数"
```

### Task 5: 共用构建与本地归档分支

**Files:**
- Modify: `apps/desktop/src-tauri/src/tools/release_package_runtime.rs`

- [ ] **Step 1: 写失败测试锁定 BuiltTarget 和本地归档回归**

新增 Windows 测试，令构建命令生成前端目录和后端文件，然后调用共用构建：

```rust
#[test]
fn build_targets_returns_sources_without_archiving() {
    let fixture = build_fixture();
    let summary = run_build_pipeline(
        "run-build",
        fixture.project.clone(),
        vec![ReleaseTarget::Frontend, ReleaseTarget::Backend],
        Arc::new(AtomicBool::new(false)),
        ProcessSlots::new(),
        Arc::new(RecordingSink::default()),
    ).unwrap();

    assert_eq!(summary.built_targets.len(), 2);
    assert!(summary.built_targets.iter().any(|item| item.source_path.ends_with("dist")));
    assert!(summary.built_targets.iter().any(|item| item.source_path.ends_with("app.jar")));
}
```

保留现有 `pipeline_commits_frontend_and_backend` 等归档事务测试，改为调用 `run_local_archive_pipeline`。

- [ ] **Step 2: 运行测试并确认失败**

Run: `cargo test release_package_runtime::tests::build_targets_returns_sources_without_archiving -- --nocapture`

Expected: FAIL，缺少 `BuiltTarget` / `run_build_pipeline`。

- [ ] **Step 3: 拆出共用构建结果**

```rust
#[derive(Clone, Debug)]
struct BuiltTarget {
    target: ReleaseTarget,
    source_path: PathBuf,
    artifact_mode: String,
}

#[derive(Debug)]
struct BuildSummary {
    status: &'static str,
    built_targets: Vec<BuiltTarget>,
    selected_count: usize,
    error: Option<String>,
}
```

`run_target` 改为只运行命令、解析源路径并校验：前端必须是目录，后端必须是普通文件；返回 `BuiltTarget`。`run_build_pipeline` 保留原有并行线程、目标状态和取消逻辑。

`run_local_archive_pipeline` 创建 `ArchiveSession`，把成功的 `BuiltTarget` 复制/压缩到 stage，形成 `ArchivedTarget`，最后 commit。保持部分成功、覆盖、取消和错误聚合行为。

- [ ] **Step 4: 运行本地归档运行时测试**

Run: `cargo test release_package_runtime::tests -- --nocapture`

Expected: 本地归档、取消和并行目标相关测试 PASS；旧上传重试测试可因尚未迁移而失败，失败清单必须只涉及 Task 6 的重试描述符。

- [ ] **Step 5: 提交运行时拆分**

```text
git add apps/desktop/src-tauri/src/tools/release_package_runtime.rs
git commit -m "refactor(release-package): 拆分构建与本地归档"
```

### Task 6: 直接产物上传与无归档重试

**Files:**
- Modify: `apps/desktop/src-tauri/src/tools/release_package_runtime.rs`
- Modify: `apps/desktop/src-tauri/src/tools/release_package_deploy.rs`

- [ ] **Step 1: 写失败测试覆盖直接目录内容和重试一致性**

新增：

```rust
#[test]
fn upload_summary_uses_built_sources_without_archive_path() {
    let built = vec![BuiltTarget {
        target: ReleaseTarget::Frontend,
        source_path: frontend_fixture(),
        artifact_mode: "zip_directory".into(),
    }];
    let summary = build_upload_summary(built).unwrap();
    assert!(summary.archive_path.is_none());
    assert_eq!(summary.manifests[0].source_path.file_name().unwrap(), "dist");
    assert_eq!(summary.manifests[0].entries[0].relative_path, "index.html");
}

#[test]
fn retry_rejects_changed_live_artifacts() {
    let source = frontend_fixture();
    let manifest = ArtifactManifest::from_directory(ReleaseTarget::Frontend, &source).unwrap();
    fs::write(source.join("index.html"), "changed-size").unwrap();
    let retry = RetryJob::from_manifests(7, vec![manifest]);
    let error = build_retry_deployment_request("retry", &retry, &consumed_preflight()).unwrap_err();
    assert!(error.message.contains("部署产物在打包后发生变化"));
}
```

在 deploy 测试中明确断言 `/srv/web` 的上传条目是 `index.html` / `assets/app.js`，不包含 `dist/` 前缀。

- [ ] **Step 2: 运行测试并确认失败**

Run: `cargo test release_package_runtime::tests::upload_summary_uses_built_sources_without_archive_path -- --nocapture`

Expected: FAIL，上传摘要仍要求已归档目标和 `archive_path`。

- [ ] **Step 3: 实现源清单上传和重试描述符**

把摘要和重试改为：

```rust
struct PipelineSummary {
    status: &'static str,
    archive_path: Option<PathBuf>,
    manifests: Vec<ArtifactManifest>,
    error: Option<String>,
    retry_descriptor: Option<RetryDescriptor>,
    remote_committed: bool,
}

#[derive(Clone, Debug)]
struct RetryDescriptor {
    manifests: Vec<ArtifactManifest>,
}
```

`build_upload_summary` 由 `BuiltTarget.source_path` 创建清单，不调用 `ArchiveSession`。`package_can_upload` 改为要求构建全部成功且 `manifests` 非空，不再要求 `archive_path`。

`build_deployment_request` 使用当前预检的远端路径和存在状态把 manifests 映射为 `DeploymentTarget`。`combine_package_and_deploy` 上传失败时克隆 manifests 生成 retry descriptor。`build_retry_deployment_request` 先逐个 `verify_source`，再使用新预检重建部署目标；删除 ZIP 解压和归档路径重试分支。

- [ ] **Step 4: 串联 start 的两种运行分支**

将 runtime `start` 接收类型专属请求：本地调用 `run_build_pipeline` 后 `run_local_archive_pipeline`；上传调用同一构建函数后 `build_upload_summary` 和 `run_deployment_phase`。上传终态 `archive_path` 必须保持 `None`。

- [ ] **Step 5: 运行全部上线包 Rust 测试**

Run: `cargo test release_package -- --nocapture`

Expected: PASS；上传测试不创建本地归档目录，原本依赖归档重试的测试已替换为源清单重试测试。

- [ ] **Step 6: 提交直接上传实现**

```text
git add apps/desktop/src-tauri/src/tools/release_package_runtime.rs apps/desktop/src-tauri/src/tools/release_package_deploy.rs
git commit -m "refactor(release-package): 直接上传构建产物"
```

### Task 7: 终态通知、经验和完整验证

**Files:**
- Modify: `apps/desktop/src-tauri/src/global_notification.rs`
- Modify: `apps/desktop/src/composables/useReleasePackageRuntime.ts`
- Modify: `apps/desktop/src/composables/useReleasePackageRuntime.test.ts`
- Modify: `apps/desktop/src/types/global-notification.ts`
- Modify: `apps/desktop/src/utils/globalNotification.ts`
- Modify: `apps/desktop/src/utils/globalNotification.test.ts`
- Modify: `apps/desktop/src/components/GlobalNotificationPopup.vue`
- Modify: `apps/desktop/src/components/GlobalNotificationPopup.test.ts`
- Modify: `docs/experience/release-package.md`

- [ ] **Step 1: 写失败测试锁定类型化终态文案**

Rust 通知测试新增 `package_type` 参数并断言序列化契约：

```rust
let notification = build_release_package_notification(
    "run", 1, "门户", ReleasePackageType::ServerUpload,
    "overall", "succeeded", None, None,
).unwrap();
let payload = serde_json::to_value(notification).unwrap();
assert_eq!(payload["packageType"], "server_upload");
assert!(payload.get("archivePath").is_none());
```

`globalNotification.test.ts` 新增：

```ts
it("uses the delivery type in release notification copy", () => {
  expect(releasePackageNotificationCopy("succeeded", "local_archive").detail)
    .toContain("本地归档完成");
  expect(releasePackageNotificationCopy("succeeded", "server_upload").detail)
    .toContain("服务器上传完成");
  expect(releasePackageNotificationCopy("package_succeeded_upload_failed", "server_upload").detail)
    .toContain("构建成功、上传失败");
});
```

运行时测试先写入一个旧归档路径，再开始新的上传运行并接收无 `archivePath` 的成功事件，断言全局和项目 runtime 的路径均为空。

- [ ] **Step 2: 运行定向测试并确认失败**

Run: `cargo test global_notification::tests -- --nocapture`

Expected: FAIL，通知函数尚不知道打包类型。

Run: `pnpm --filter @lazycat/desktop test -- src/components/GlobalNotificationPopup.test.ts src/composables/useReleasePackageRuntime.test.ts`

Expected: FAIL 或缺少无归档路径用例。

- [ ] **Step 3: 实现终态清理和文案**

在 Rust `GlobalNotification::ReleasePackage` 增加 `package_type: ReleasePackageType`，并由 `build_release_package_notification` 接收、序列化该类型。前端 `ReleasePackageNotification` 增加 `packageType`；`normalizeGlobalNotificationPayload` 只接受两个合法值；`releasePackageNotificationCopy(status, packageType)` 对本地成功返回“本地归档完成”，对上传成功返回“服务器上传完成”，对上传失败返回“构建成功、上传失败”。弹窗把 `currentPackage.packageType` 传给 helper。

在 `beginStart` 时清空全局和项目 runtime 的 `archivePath`，overall 成功事件缺少路径时保持为空，避免跨运行残留。

- [ ] **Step 4: 更新经验文档**

将 `docs/experience/release-package.md` 的“本地归档与远端上传分别表达”改为两种互斥打包类型：上传不依赖归档目录、直接读取生成物目录内容、失败重试校验源清单；保留 SSH 预检和远端事务经验。使用次数加 1 并记录本次日期 `2026-07-22`。

- [ ] **Step 5: 运行完整验证**

Run: `cargo test release_package -- --nocapture`

Expected: PASS，0 failed。

Run: `pnpm --filter @lazycat/desktop test -- src/utils/releasePackage.test.ts src/components/ReleasePackagePanel.test.ts src/composables/useReleasePackageRuntime.test.ts src/components/GlobalNotificationPopup.test.ts`

Expected: PASS，0 failed。

Run: `pnpm typecheck`

Expected: PASS。

Run: `pnpm --filter @lazycat/desktop build:web`

Expected: PASS。

Run: `git diff --check`

Expected: 无输出，退出码 0。

- [ ] **Step 6: 检查需求清单**

逐项确认：上传项目可空 `outputRoot`；上传启动不调用 `target-check`；远端路径接收目录内容而非源目录本身；本地归档仍支持 ZIP、覆盖和部分成功；上传重试不读取归档；旧项目迁移只执行一次；通知不混淆归档和上传。

- [ ] **Step 7: 提交终态与文档**

```text
git add apps/desktop/src-tauri/src/global_notification.rs apps/desktop/src/composables/useReleasePackageRuntime.ts apps/desktop/src/composables/useReleasePackageRuntime.test.ts apps/desktop/src/types/global-notification.ts apps/desktop/src/utils/globalNotification.ts apps/desktop/src/utils/globalNotification.test.ts apps/desktop/src/components/GlobalNotificationPopup.vue apps/desktop/src/components/GlobalNotificationPopup.test.ts docs/experience/release-package.md
git commit -m "fix(release-package): 区分归档与上传终态"
```

## 计划自检

- 规格中的数据迁移、UI、IPC、运行时、直接目录内容上传、重试、状态、错误与验证均有对应任务。
- `packageType`、`ReleasePackageType`、`BuiltTarget`、`ArtifactManifest` 和 `RetryDescriptor` 在所有任务中命名一致。
- 没有第三种组合模式，没有本地上传临时副本，没有增加哈希或新 IPC channel。
- 实现按失败测试 -> 最小实现 -> 定向验证 -> 提交顺序执行。
