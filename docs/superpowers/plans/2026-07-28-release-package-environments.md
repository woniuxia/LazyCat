# Release Package Environments Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为每个上线包项目提供固定的测试、生产环境配置，并让配置保存、打包、上传、重试、通知和 Todo 动作绑定都以环境 ID 为稳定引用。

**Architecture:** 保留 `release_package_projects` 作为项目公共信息表，新建 `release_package_environments` 保存两套环境专属配置，并用自增环境 ID 贯穿 IPC 与运行链路。运行时在启动前按环境 ID 加载不可变快照，生产环境增加显式确认，既有项目和动作绑定在单事务迁移中归入生产环境。

**Tech Stack:** Tauri 2、Rust、rusqlite、serde、Vue 3、TypeScript、Element Plus、Vitest、pnpm。

---

## 实施边界

- 直接在当前 `main` 工作区执行，不创建 worktree；开始前确认工作树只包含本计划允许的改动。
- 严格保持全局单运行槽，不增加环境并发、队列或持久化恢复。
- 环境固定为 `test`、`production`，不增加环境管理入口。
- 测试环境不得继承生产配置；现有配置只迁移到生产环境。
- 继续使用既有 SSH 信任、预检、上传事务、取消、上传重试和命令重试实现。
- 每个任务先写失败测试，再做最小实现，再提交；不得把后续任务的功能提前混入当前提交。

## 文件职责映射

| 文件                                                                                            | 本计划职责                                            |
| ----------------------------------------------------------------------------------------------- | ----------------------------------------------------- |
| `apps/desktop/src/types/release-package.ts`                                                     | 项目、环境、草稿、事件和 IPC 类型                     |
| `apps/desktop/src/utils/releasePackage.ts`                                                      | 公共草稿/环境草稿转换、校验、dirty 判断、启动 payload |
| `apps/desktop/src-tauri/src/tools/release_package.rs`                                           | schema 迁移、项目/环境 CRUD、按环境 ID 解析所有 IPC   |
| `apps/desktop/src-tauri/src/tools/release_package_runtime.rs`                                   | 环境运行身份、生产确认、运行快照、令牌绑定和事件      |
| `apps/desktop/src-tauri/src/tools/release_package_remote.rs`                                    | 预检绑定增加环境 ID 和配置指纹                        |
| `apps/desktop/src-tauri/src/tools/action_center/definitions.rs`                                 | 动作目标从项目改为环境                                |
| `apps/desktop/src-tauri/src/tools/action_center/bindings.rs`                                    | 环境目标校验与展示                                    |
| `apps/desktop/src-tauri/src/tools/action_center/dispatches.rs`                                  | 派发与环境运行关联                                    |
| `apps/desktop/src-tauri/src/global_notification.rs`                                             | 通知携带项目与环境快照                                |
| `apps/desktop/src/composables/useReleasePackageRuntime.ts`                                      | 按环境 ID 隔离运行态与事件                            |
| `apps/desktop/src/composables/useReleasePackageUploadPreflight.ts`                              | 远程预检统一传环境 ID                                 |
| `apps/desktop/src/composables/useReleasePackageCommandRetry.ts`                                 | 命令重试统一传环境 ID                                 |
| `apps/desktop/src/components/ReleasePackagePanel.vue`                                           | 环境切换、公共/环境表单、生产确认、动作意图           |
| `apps/desktop/src/types/global-notification.ts`、`apps/desktop/src/utils/globalNotification.ts` | 前端通知环境契约与校验                                |
| 对应 `*.test.ts` 和 Rust `#[cfg(test)]` 模块                                                    | TDD 与回归覆盖                                        |
| `docs/experience/release-package.md`                                                            | 沉淀固定环境、环境 ID 和生产保护经验                  |

### Task 1: 建立前端项目/环境类型与纯函数边界

**Files:**

- Modify: `apps/desktop/src/types/release-package.ts`
- Modify: `apps/desktop/src/types/index.ts`
- Modify: `apps/desktop/src/utils/releasePackage.ts`
- Test: `apps/desktop/src/utils/releasePackage.test.ts`

- [ ] **Step 1: 写项目公共草稿、环境草稿和启动参数的失败测试**

在 `releasePackage.test.ts` 增加明确覆盖，并在同一测试文件定义完整 fixture：

```ts
function releaseProjectFixture(): ReleasePackageProject {
  const environmentBase: ReleasePackageEnvironmentDraft = {
    packageType: "server_upload",
    outputRoot: "",
    frontendBuildCommand: "pnpm build",
    frontendSuccessKeyword: "built",
    frontendPostUploadCommand: "nginx -s reload",
    frontendArtifactPath: "dist",
    frontendArtifactMode: "copy_directory",
    backendBuildCommand: "mvn clean package",
    backendSuccessKeyword: "BUILD SUCCESS",
    backendPostUploadCommand: "systemctl restart portal",
    backendArtifactPath: "target/app.jar",
    sshHost: "10.0.0.8",
    sshPort: 22,
    sshUsername: "deploy",
    sshAuthType: "private_key",
    vaultEntryId: null,
    sshPrivateKeyPath: "C:\\Keys\\deploy",
    frontendRemoteDir: "/srv/portal/web",
    backendRemotePath: "/srv/portal/app.jar",
  };
  return {
    id: 7,
    name: "客户门户",
    frontendProjectPath: "D:\\portal\\web",
    backendProjectPath: "D:\\portal\\server",
    createdAt: "2026-07-28T00:00:00Z",
    updatedAt: "2026-07-28T00:00:00Z",
    environments: [
      {
        ...environmentBase,
        id: 41,
        projectId: 7,
        environment: "test",
        configured: true,
        frontendBuildCommand: "pnpm build:test",
        frontendRemoteDir: "/srv/portal-test/web",
        backendRemotePath: "/srv/portal-test/app.jar",
        createdAt: "2026-07-28T00:00:00Z",
        updatedAt: "2026-07-28T00:00:00Z",
      },
      {
        ...environmentBase,
        id: 42,
        projectId: 7,
        environment: "production",
        configured: true,
        frontendBuildCommand: "pnpm build:prod",
        createdAt: "2026-07-28T00:00:00Z",
        updatedAt: "2026-07-28T00:00:00Z",
      },
    ],
  };
}

it("separates shared project fields from fixed environment fields", () => {
  const projectDraft = createEmptyReleasePackageProjectDraft();
  const environmentDraft = createEmptyReleasePackageEnvironmentDraft();

  expect(projectDraft).toEqual({
    name: "",
    frontendProjectPath: "",
    backendProjectPath: "",
  });
  expect(environmentDraft.packageType).toBe("local_archive");
  expect(environmentDraft.outputRoot).toBe("");
  expect(environmentDraft.frontendBuildCommand).toBe("");
  expect(environmentDraft.backendBuildCommand).toBe("");
  expect(environmentDraft.sshPort).toBe(22);
});

it("builds starts from an environment id and explicit production confirmation", () => {
  expect(
    createReleasePackageStartPayload("local_archive", {
      environmentId: 42,
      targets: ["frontend"],
      folderName: "2026-07-28-portal",
      overwriteExisting: false,
      preflightToken: "",
      overwriteRemoteTargets: [],
      productionConfirmed: true,
    }),
  ).toEqual({
    environmentId: 42,
    targets: ["frontend"],
    folderName: "2026-07-28-portal",
    overwriteExisting: false,
    productionConfirmed: true,
  });
});

it("detects shared and environment changes independently", () => {
  const project = releaseProjectFixture();
  const production = project.environments.find((item) => item.environment === "production")!;
  const projectDraft = projectToReleasePackageProjectDraft(project);
  const environmentDraft = environmentToReleasePackageDraft(production);

  expect(isReleasePackageDraftDirty(project, production, projectDraft, environmentDraft)).toBe(
    false,
  );
  environmentDraft.backendBuildCommand = "mvn clean package -Pprod";
  expect(isReleasePackageDraftDirty(project, production, projectDraft, environmentDraft)).toBe(
    true,
  );
});
```

同时把测试 fixture 改成一个项目包含 `test`、`production` 两条环境，二者使用不同命令和远程路径。

- [ ] **Step 2: 运行定向测试确认因类型和函数缺失而失败**

Run:

```powershell
pnpm --filter @lazycat/desktop test -- src/utils/releasePackage.test.ts
```

Expected: FAIL，错误包含 `createEmptyReleasePackageProjectDraft is not a function` 或新类型导出缺失。

- [ ] **Step 3: 实现新的前端类型和纯函数**

在 `types/release-package.ts` 定义并导出：

```ts
export type ReleasePackageEnvironmentKind = "test" | "production";

export interface ReleasePackageProjectDraft {
  name: string;
  frontendProjectPath: string;
  backendProjectPath: string;
}

export interface ReleasePackageEnvironmentDraft extends ReleasePackageUploadConfig {
  packageType: ReleasePackageType;
  outputRoot: string;
  frontendBuildCommand: string;
  frontendSuccessKeyword: string;
  frontendPostUploadCommand: string;
  frontendArtifactPath: string;
  frontendArtifactMode: ReleasePackageArtifactMode;
  backendBuildCommand: string;
  backendSuccessKeyword: string;
  backendPostUploadCommand: string;
  backendArtifactPath: string;
}

export interface ReleasePackageEnvironmentConfig extends ReleasePackageEnvironmentDraft {
  id: number;
  projectId: number;
  environment: ReleasePackageEnvironmentKind;
  configured: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface ReleasePackageProject extends ReleasePackageProjectDraft {
  id: number;
  environments: ReleasePackageEnvironmentConfig[];
  createdAt: string;
  updatedAt: string;
}
```

给 `ReleasePackageLogEvent`、`ReleasePackageStatusEvent` 增加：

```ts
environmentId: number;
projectId: number;
environment: ReleasePackageEnvironmentKind;
```

在 `utils/releasePackage.ts` 用两份草稿替代旧混合草稿，并保留既有字段校验：

```ts
export function createEmptyReleasePackageProjectDraft(): ReleasePackageProjectDraft {
  return { name: "", frontendProjectPath: "", backendProjectPath: "" };
}

export function createEmptyReleasePackageEnvironmentDraft(): ReleasePackageEnvironmentDraft {
  return {
    packageType: "local_archive",
    outputRoot: "",
    frontendBuildCommand: "",
    frontendSuccessKeyword: "",
    frontendPostUploadCommand: "",
    frontendArtifactPath: "",
    frontendArtifactMode: "copy_directory",
    backendBuildCommand: "",
    backendSuccessKeyword: "",
    backendPostUploadCommand: "",
    backendArtifactPath: "",
    sshHost: "",
    sshPort: 22,
    sshUsername: "",
    sshAuthType: "password",
    vaultEntryId: null,
    sshPrivateKeyPath: "",
    frontendRemoteDir: "",
    backendRemotePath: "",
  };
}
```

`validateReleasePackageProjectDraft` 只校验项目名和两个工程目录；`validateReleasePackageEnvironmentDraft` 校验构建命令、产物及类型专属字段。`createReleasePackageStartPayload` 公共字段固定包含 `environmentId`，仅在输入值为 `true` 时增加 `productionConfirmed: true`。同步更新 `types/index.ts` 导出新类型。

同时实现并导出后续面板直接使用的规范化函数，字符串只去掉首尾空白，多行命令不得重排内部内容：

```ts
export function normalizeReleasePackageProjectDraft(
  draft: ReleasePackageProjectDraft,
): ReleasePackageProjectDraft {
  return {
    name: draft.name.trim(),
    frontendProjectPath: draft.frontendProjectPath.trim(),
    backendProjectPath: draft.backendProjectPath.trim(),
  };
}

export function normalizeReleasePackageEnvironmentDraft(
  draft: ReleasePackageEnvironmentDraft,
): ReleasePackageEnvironmentDraft {
  return Object.fromEntries(
    Object.entries(draft).map(([key, value]) => [
      key,
      typeof value === "string" ? value.trim() : value,
    ]),
  ) as unknown as ReleasePackageEnvironmentDraft;
}
```

草稿恢复和 dirty 判断使用明确字段映射，不把 `id`、`configured` 或时间字段混入可编辑数据：

```ts
export function projectToReleasePackageProjectDraft(
  project: ReleasePackageProject,
): ReleasePackageProjectDraft {
  return normalizeReleasePackageProjectDraft({
    name: project.name,
    frontendProjectPath: project.frontendProjectPath,
    backendProjectPath: project.backendProjectPath,
  });
}

export function environmentToReleasePackageDraft(
  environment: ReleasePackageEnvironmentConfig,
): ReleasePackageEnvironmentDraft {
  return normalizeReleasePackageEnvironmentDraft({
    packageType: environment.packageType,
    outputRoot: environment.outputRoot,
    frontendBuildCommand: environment.frontendBuildCommand,
    frontendSuccessKeyword: environment.frontendSuccessKeyword,
    frontendPostUploadCommand: environment.frontendPostUploadCommand,
    frontendArtifactPath: environment.frontendArtifactPath,
    frontendArtifactMode: environment.frontendArtifactMode,
    backendBuildCommand: environment.backendBuildCommand,
    backendSuccessKeyword: environment.backendSuccessKeyword,
    backendPostUploadCommand: environment.backendPostUploadCommand,
    backendArtifactPath: environment.backendArtifactPath,
    sshHost: environment.sshHost,
    sshPort: environment.sshPort,
    sshUsername: environment.sshUsername,
    sshAuthType: environment.sshAuthType,
    vaultEntryId: environment.vaultEntryId,
    sshPrivateKeyPath: environment.sshPrivateKeyPath,
    frontendRemoteDir: environment.frontendRemoteDir,
    backendRemotePath: environment.backendRemotePath,
  });
}

export function isReleasePackageDraftDirty(
  project: ReleasePackageProject | null,
  environment: ReleasePackageEnvironmentConfig | null,
  projectDraft: ReleasePackageProjectDraft,
  environmentDraft: ReleasePackageEnvironmentDraft,
): boolean {
  if (!project || !environment) return true;
  return (
    JSON.stringify(projectToReleasePackageProjectDraft(project)) !==
      JSON.stringify(normalizeReleasePackageProjectDraft(projectDraft)) ||
    JSON.stringify(environmentToReleasePackageDraft(environment)) !==
      JSON.stringify(normalizeReleasePackageEnvironmentDraft(environmentDraft))
  );
}

export interface ReleasePackageStartPayloadInput {
  environmentId: number;
  targets: readonly ReleasePackageTarget[];
  folderName: string;
  overwriteExisting: boolean;
  preflightToken: string;
  overwriteRemoteTargets: readonly ReleasePackageTarget[];
  productionConfirmed: boolean;
  actionDispatchId?: string;
}
```

- [ ] **Step 4: 运行纯函数测试确认通过**

Run:

```powershell
pnpm --filter @lazycat/desktop test -- src/utils/releasePackage.test.ts
```

Expected: PASS，且旧的命令示例、Linux 路径、Vault 端口和打包类型测试仍通过。

- [ ] **Step 5: 提交前端领域模型**

```powershell
git add apps/desktop/src/types/release-package.ts apps/desktop/src/types/index.ts apps/desktop/src/utils/releasePackage.ts apps/desktop/src/utils/releasePackage.test.ts
git commit -m "feat(release-package): 拆分项目与环境配置模型"
```

### Task 2: 新增环境表并原子迁移既有项目

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/release_package.rs`
- Test: `apps/desktop/src-tauri/src/tools/release_package.rs`

- [ ] **Step 1: 写 schema 与迁移失败测试**

在 Rust 测试模块增加：

```rust
fn seed_legacy_release_package_schema(conn: &Connection) {
    conn.execute_batch(
        "CREATE TABLE release_package_projects (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            output_root TEXT NOT NULL,
            package_type TEXT NOT NULL,
            frontend_project_path TEXT NOT NULL,
            frontend_build_command TEXT NOT NULL,
            frontend_success_keyword TEXT NOT NULL DEFAULT '',
            frontend_post_upload_command TEXT NOT NULL DEFAULT '',
            frontend_artifact_path TEXT NOT NULL,
            frontend_artifact_mode TEXT NOT NULL,
            backend_project_path TEXT NOT NULL,
            backend_build_command TEXT NOT NULL,
            backend_success_keyword TEXT NOT NULL DEFAULT '',
            backend_post_upload_command TEXT NOT NULL DEFAULT '',
            backend_artifact_path TEXT NOT NULL,
            upload_enabled INTEGER NOT NULL DEFAULT 0,
            ssh_host TEXT NOT NULL DEFAULT '',
            ssh_port INTEGER NOT NULL DEFAULT 22,
            ssh_username TEXT NOT NULL DEFAULT '',
            ssh_auth_type TEXT NOT NULL DEFAULT 'password',
            vault_entry_id INTEGER NULL,
            ssh_private_key_path TEXT NOT NULL DEFAULT '',
            frontend_remote_dir TEXT NOT NULL DEFAULT '',
            backend_remote_path TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );"
    ).unwrap();
}

fn seed_legacy_release_project(conn: &Connection, id: i64, name: &str, remote_dir: &str) {
    conn.execute(
        "INSERT INTO release_package_projects(
            id, name, output_root, package_type, frontend_project_path,
            frontend_build_command, frontend_artifact_path, frontend_artifact_mode,
            backend_project_path, backend_build_command, backend_artifact_path,
            ssh_auth_type, vault_entry_id, frontend_remote_dir, backend_remote_path
         ) VALUES(?1, ?2, '', 'server_upload', 'D:\\portal\\web',
                  'pnpm build:prod', 'dist', 'copy_directory',
                  'D:\\portal\\server', 'mvn package -Pprod', 'target/app.jar',
                  'password', 3, ?3, '/srv/portal/app.jar')",
        params![id, name, remote_dir],
    ).unwrap();
}

#[test]
fn schema_migrates_each_legacy_project_to_production_and_blank_test() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    seed_legacy_release_package_schema(&conn);
    seed_legacy_release_project(&conn, 7, "客户门户", "/srv/portal/web");

    ensure_schema(&conn).unwrap();

    let project: (i64, String, String, String) = conn.query_row(
        "SELECT id, name, frontend_project_path, backend_project_path
         FROM release_package_projects WHERE id=7",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    ).unwrap();
    assert_eq!(project.0, 7);
    assert_eq!(project.1, "客户门户");

    let environments = conn.prepare(
        "SELECT environment, frontend_build_command, frontend_remote_dir, vault_entry_id
         FROM release_package_environments WHERE project_id=7 ORDER BY environment"
    ).unwrap().query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, Option<i64>>(3)?))
    }).unwrap().collect::<Result<Vec<_>, _>>().unwrap();

    assert_eq!(environments.len(), 2);
    assert_eq!(environments[0].0, "production");
    assert_eq!(environments[0].2, "/srv/portal/web");
    assert_eq!(environments[1], ("test".into(), "".into(), "".into(), None));

    ensure_schema(&conn).unwrap();
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM release_package_environments WHERE project_id=7",
        [],
        |row| row.get(0),
    ).unwrap();
    assert_eq!(count, 2);
}
```

再加约束测试：重复 `(project_id, environment)` 插入失败，删除项目级联删除两个环境。

- [ ] **Step 2: 运行 Rust 定向测试确认旧 schema 不满足断言**

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml schema_migrates_each_legacy_project_to_production_and_blank_test -- --nocapture
```

Expected: FAIL，错误表明 `release_package_environments` 不存在。

- [ ] **Step 3: 实现环境 enum、DDL 和幂等事务迁移**

在 `release_package.rs` 增加：

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleasePackageEnvironmentKind {
    Test,
    Production,
}

impl ReleasePackageEnvironmentKind {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "test" => Ok(Self::Test),
            "production" => Ok(Self::Production),
            _ => Err("上线包环境无效".into()),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Test => "test",
            Self::Production => "production",
        }
    }
}
```

将新建库 DDL 改为公共项目表，并创建环境表及索引：

```sql
CREATE TABLE IF NOT EXISTS release_package_projects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    frontend_project_path TEXT NOT NULL,
    backend_project_path TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS release_package_environments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL REFERENCES release_package_projects(id) ON DELETE CASCADE,
    environment TEXT NOT NULL CHECK(environment IN ('test','production')),
    output_root TEXT NOT NULL DEFAULT '',
    package_type TEXT NOT NULL DEFAULT 'local_archive' CHECK(package_type IN ('local_archive','server_upload')),
    frontend_build_command TEXT NOT NULL DEFAULT '',
    frontend_success_keyword TEXT NOT NULL DEFAULT '',
    frontend_post_upload_command TEXT NOT NULL DEFAULT '',
    frontend_artifact_path TEXT NOT NULL DEFAULT '',
    frontend_artifact_mode TEXT NOT NULL DEFAULT 'copy_directory' CHECK(frontend_artifact_mode IN ('copy_directory','zip_directory')),
    backend_build_command TEXT NOT NULL DEFAULT '',
    backend_success_keyword TEXT NOT NULL DEFAULT '',
    backend_post_upload_command TEXT NOT NULL DEFAULT '',
    backend_artifact_path TEXT NOT NULL DEFAULT '',
    ssh_host TEXT NOT NULL DEFAULT '',
    ssh_port INTEGER NOT NULL DEFAULT 22,
    ssh_username TEXT NOT NULL DEFAULT '',
    ssh_auth_type TEXT NOT NULL DEFAULT 'password' CHECK(ssh_auth_type IN ('password','private_key')),
    vault_entry_id INTEGER NULL,
    ssh_private_key_path TEXT NOT NULL DEFAULT '',
    frontend_remote_dir TEXT NOT NULL DEFAULT '',
    backend_remote_path TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(project_id, environment)
);
```

迁移 helper 使用 `rusqlite::Transaction::new_unchecked(conn, TransactionBehavior::Immediate)`，按以下固定顺序执行：旧表改名、创建新公共表、保留原 ID 复制公共字段、创建环境表、插入生产配置、插入空测试配置、验证每个项目环境计数为 2、更新活动动作目标、删除旧表、提交。用 `PRAGMA table_info(release_package_projects)` 检测旧字段 `frontend_build_command`，作为是否需要迁移的唯一结构判断。

旧数据复制使用明确列映射，不使用 `SELECT *`：

```sql
ALTER TABLE release_package_projects RENAME TO release_package_projects_legacy;

CREATE TABLE release_package_projects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    frontend_project_path TEXT NOT NULL,
    backend_project_path TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- 紧接着执行本任务前述 release_package_environments 完整 DDL。

INSERT INTO release_package_projects(
    id, name, frontend_project_path, backend_project_path, created_at, updated_at
)
SELECT id, name, frontend_project_path, backend_project_path, created_at, updated_at
FROM release_package_projects_legacy;

INSERT INTO release_package_environments(
    project_id, environment, output_root, package_type,
    frontend_build_command, frontend_success_keyword, frontend_post_upload_command,
    frontend_artifact_path, frontend_artifact_mode,
    backend_build_command, backend_success_keyword, backend_post_upload_command,
    backend_artifact_path, ssh_host, ssh_port, ssh_username, ssh_auth_type,
    vault_entry_id, ssh_private_key_path, frontend_remote_dir, backend_remote_path,
    created_at, updated_at
)
SELECT id, 'production', output_root, package_type,
       frontend_build_command, frontend_success_keyword, frontend_post_upload_command,
       frontend_artifact_path, frontend_artifact_mode,
       backend_build_command, backend_success_keyword, backend_post_upload_command,
       backend_artifact_path, ssh_host, ssh_port, ssh_username, ssh_auth_type,
       vault_entry_id, ssh_private_key_path, frontend_remote_dir, backend_remote_path,
       created_at, updated_at
FROM release_package_projects_legacy;

INSERT INTO release_package_environments(project_id, environment)
SELECT id, 'test' FROM release_package_projects;
```

完成环境计数、动作目标和外键验证后执行 `DROP TABLE release_package_projects_legacy`。`release_package_known_hosts` 不依赖项目或环境 ID，保持原表和数据不变。

迁移生产环境 ID 时先建立临时映射表：

```sql
CREATE TEMP TABLE release_package_environment_migration_map (
    project_id INTEGER PRIMARY KEY,
    production_environment_id INTEGER NOT NULL
);
INSERT INTO release_package_environment_migration_map(project_id, production_environment_id)
SELECT project_id, id FROM release_package_environments WHERE environment='production';
```

只有 `sqlite_master` 确认对应动作表已存在时才执行目标迁移，保证 release package 独立 schema 测试和完整应用初始化都可运行。表存在时执行：

```sql
UPDATE action_bindings
SET target_id = (
    SELECT CAST(production_environment_id AS TEXT)
    FROM release_package_environment_migration_map
    WHERE project_id = CAST(action_bindings.target_id AS INTEGER)
)
WHERE action_type='release_package.run'
  AND target_id <> ''
  AND target_id NOT GLOB '*[^0-9]*';

UPDATE action_dispatches
SET target_id = (
    SELECT CAST(production_environment_id AS TEXT)
    FROM release_package_environment_migration_map
    WHERE project_id = CAST(action_dispatches.target_id AS INTEGER)
)
WHERE action_type='release_package.run'
  AND status IN ('pending_confirmation','running')
  AND target_id <> ''
  AND target_id NOT GLOB '*[^0-9]*';
```

动作表存在时，迁移后查询所有上线包活动绑定及活动派发，任何空 `target_id` 或无法 JOIN 到环境表的记录都返回迁移错误。

事务提交前删除临时映射表。错误路径依赖 `Transaction` drop 回滚，不捕获后伪装成功。

- [ ] **Step 4: 运行 schema 相关测试**

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package::tests::schema_ -- --nocapture
```

Expected: PASS，旧的 Vault、端口和 `package_type` 迁移测试改写到环境表后继续通过。

- [ ] **Step 5: 提交 schema 迁移**

```powershell
git add apps/desktop/src-tauri/src/tools/release_package.rs
git commit -m "feat(release-package): 新增固定环境配置表"
```

### Task 3: 将项目 CRUD 改为公共配置加当前环境事务

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/release_package.rs`
- Test: `apps/desktop/src-tauri/src/tools/release_package.rs`

- [ ] **Step 1: 写聚合列表、创建和更新失败测试**

增加测试，要求创建项目时生成两条环境、列表返回聚合结构、更新不能跨项目引用环境 ID：

```rust
fn environment_project_payload(environment: &str) -> Value {
    json!({
        "project": {
            "name": "客户门户",
            "frontendProjectPath": "D:\\portal\\web",
            "backendProjectPath": "D:\\portal\\server"
        },
        "environment": environment,
        "environmentConfig": {
            "packageType": "local_archive",
            "outputRoot": "D:\\releases",
            "frontendBuildCommand": "pnpm build",
            "frontendSuccessKeyword": "built",
            "frontendPostUploadCommand": "",
            "frontendArtifactPath": "dist",
            "frontendArtifactMode": "copy_directory",
            "backendBuildCommand": "mvn clean package",
            "backendSuccessKeyword": "BUILD SUCCESS",
            "backendPostUploadCommand": "",
            "backendArtifactPath": "target/app.jar",
            "sshHost": "",
            "sshPort": 22,
            "sshUsername": "",
            "sshAuthType": "password",
            "vaultEntryId": null,
            "sshPrivateKeyPath": "",
            "frontendRemoteDir": "",
            "backendRemotePath": ""
        }
    })
}

#[test]
fn project_crud_returns_two_environments_and_rejects_cross_project_update() {
    let conn = test_conn();
    let created = project_create_with_conn(&conn, &environment_project_payload("test")).unwrap();
    let project_id = created["id"].as_i64().unwrap();
    let test_environment_id = created["environmentId"].as_i64().unwrap();

    let listed = project_list_with_conn(&conn).unwrap();
    let project = &listed["projects"][0];
    assert_eq!(project["id"], project_id);
    assert_eq!(project["environments"].as_array().unwrap().len(), 2);
    assert_eq!(project["environments"][0]["environment"], "test");
    assert_eq!(project["environments"][0]["configured"], true);
    assert_eq!(project["environments"][1]["environment"], "production");
    assert_eq!(project["environments"][1]["configured"], false);

    let other = project_create_with_conn(&conn, &environment_project_payload("test")).unwrap();
    let mut update = environment_project_payload("test");
    update["id"] = project_id.into();
    update["environmentId"] = other["environmentId"].clone();
    assert_eq!(
        project_update_with_conn(&conn, &update).unwrap_err(),
        "上线包环境不属于当前项目"
    );
    assert!(load_environment(&conn, test_environment_id).is_ok());
}

#[test]
fn project_list_rejects_a_missing_fixed_environment() {
    let conn = test_conn();
    let created = project_create_with_conn(&conn, &environment_project_payload("test")).unwrap();
    conn.execute(
        "DELETE FROM release_package_environments
         WHERE project_id=?1 AND environment='production'",
        [created["id"].as_i64().unwrap()],
    ).unwrap();
    assert_eq!(
        project_list_with_conn(&conn).unwrap_err(),
        "上线包项目环境配置不完整"
    );
}
```

- [ ] **Step 2: 运行 CRUD 测试确认失败**

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml project_crud_returns_two_environments_and_rejects_cross_project_update -- --nocapture
```

Expected: FAIL，旧创建结果没有 `environmentId`，列表没有 `environments`。

- [ ] **Step 3: 实现配置结构、聚合查询和事务 CRUD**

把旧 `ReleasePackageProjectConfig` 拆成公共项目与可运行环境快照：

```rust
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleasePackageProjectConfig {
    pub id: i64,
    pub name: String,
    pub frontend_project_path: String,
    pub backend_project_path: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleasePackageEnvironmentConfig {
    pub id: i64,
    pub project_id: i64,
    #[serde(skip_serializing)]
    pub project_name: String,
    pub environment: ReleasePackageEnvironmentKind,
    pub configured: bool,
    pub package_type: ReleasePackageType,
    pub output_root: String,
    #[serde(skip_serializing)]
    pub frontend_project_path: String,
    pub frontend_build_command: String,
    pub frontend_success_keyword: String,
    pub frontend_post_upload_command: String,
    pub frontend_artifact_path: String,
    pub frontend_artifact_mode: String,
    #[serde(skip_serializing)]
    pub backend_project_path: String,
    pub backend_build_command: String,
    pub backend_success_keyword: String,
    pub backend_post_upload_command: String,
    pub backend_artifact_path: String,
    pub ssh_host: String,
    pub ssh_port: u16,
    pub ssh_username: String,
    pub ssh_auth_type: String,
    pub vault_entry_id: Option<i64>,
    pub ssh_private_key_path: String,
    pub frontend_remote_dir: String,
    pub backend_remote_path: String,
    pub created_at: String,
    pub updated_at: String,
}
```

`load_environment(conn, environment_id)` 用一次 JOIN 加载公共工程目录和环境字段。`project_list_with_conn` 先查项目，再一次查出全部环境并按 `project_id` 分组，固定按 `test`、`production` 排序；`configured` 由与运行时相同的校验函数计算。若任一项目不是恰好两条环境，或环境类型不是一条测试加一条生产，列表直接返回“上线包项目环境配置不完整”，不在读取路径补写数据库。

`project_create_with_conn` 在事务内：验证公共草稿和当前环境草稿、插入项目、插入当前环境、插入另一空环境、返回 `id + environmentId`。`project_update_with_conn` 先验证环境归属，再在同一事务更新公共表与当前环境表。`project_delete_with_conn` 保持项目 ID 输入并依赖级联删除。

- [ ] **Step 4: 运行全部 release_package 配置测试**

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package::tests -- --nocapture
```

Expected: PASS，CRUD、Vault、类型专属校验、目录校验和 schema 测试全部通过。

- [ ] **Step 5: 提交环境 CRUD**

```powershell
git add apps/desktop/src-tauri/src/tools/release_package.rs
git commit -m "feat(release-package): 按环境保存项目配置"
```

### Task 4: 将动作中心目标和未完成派发迁移为环境 ID

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/release_package.rs`
- Modify: `apps/desktop/src-tauri/src/tools/action_center/definitions.rs`
- Modify: `apps/desktop/src-tauri/src/tools/action_center/bindings.rs`
- Modify: `apps/desktop/src-tauri/src/tools/action_center/dispatches.rs`
- Test: the same Rust files

- [ ] **Step 1: 写动作目标、绑定展示和派发关联失败测试**

在 `definitions.rs` 测试要求返回两个环境且空环境不可用：

```rust
assert_eq!(definition("release_package.run").unwrap().target_kind, "release_package_environment");
assert_eq!(targets[0].label, "客户门户 · 测试环境");
assert!(!targets[0].available);
assert_eq!(targets[0].unavailable_reason.as_deref(), Some("环境配置不完整"));
assert_eq!(targets[1].label, "客户门户 · 生产环境");
assert!(targets[1].available);
```

在 `dispatches.rs` 把关联测试改为环境 ID，并增加错误断言：

```rust
assert_eq!(
    associate_release_package_run_with_conn(&mut conn, &dispatch.id, "run-1", test_environment_id)
        .unwrap_err(),
    "动作派发目标与上线包环境不匹配"
);
associate_release_package_run_with_conn(
    &mut conn,
    &dispatch.id,
    "run-1",
    production_environment_id,
).unwrap();
```

- [ ] **Step 2: 运行 action_center 定向测试确认失败**

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml action_center -- --nocapture
```

Expected: FAIL，目标仍显示项目名且关联函数仍比较项目 ID。

- [ ] **Step 3: 实现环境目标适配器和迁移更新**

将 release package action target row 定义为：

```rust
pub(crate) struct ReleasePackageActionTargetRow {
    pub id: i64,
    pub label: String,
    pub available: bool,
    pub unavailable_reason: Option<String>,
}
```

`list_action_target_rows` JOIN 项目与环境，标签按 `environment` 输出“测试环境”或“生产环境”，可用性复用环境完整校验。`load_action_target_label` 按环境 ID 查询并返回同一标签。

`definitions.rs` 直接映射 row 的 `available` 与 `unavailable_reason`，并把 `target_kind` 改为 `release_package_environment`。`bindings.rs` 将 `target_id` 解析为环境 ID并调用新 label loader。`dispatches.rs` 的 `associate_release_package_run_with_conn` 第四个参数命名为 `environment_id`，只比较派发 `target_id` 与环境 ID。

补充集成迁移测试；该测试验证 Task 2 已实现的原子映射，不在此任务重新写第二套迁移逻辑：

```rust
#[test]
fn legacy_action_targets_migrate_to_the_production_environment_id() {
    let conn = Connection::open_in_memory().unwrap();
    seed_legacy_release_package_schema(&conn);
    seed_legacy_release_project(&conn, 7, "客户门户", "/srv/portal/web");
    conn.execute_batch(
        "CREATE TABLE action_bindings (
            id INTEGER PRIMARY KEY,
            action_type TEXT NOT NULL,
            target_id TEXT NOT NULL
        );
        CREATE TABLE action_dispatches (
            id TEXT PRIMARY KEY,
            action_type TEXT NOT NULL,
            target_id TEXT NOT NULL,
            status TEXT NOT NULL
        );
        INSERT INTO action_bindings VALUES(1, 'release_package.run', '7');
        INSERT INTO action_dispatches VALUES('dispatch-1', 'release_package.run', '7', 'pending_confirmation');"
    ).unwrap();

    ensure_schema(&conn).unwrap();
    let production_id: i64 = conn.query_row(
        "SELECT id FROM release_package_environments
         WHERE project_id=7 AND environment='production'",
        [],
        |row| row.get(0),
    ).unwrap();
    let binding_target: String = conn.query_row(
        "SELECT target_id FROM action_bindings WHERE id=1",
        [],
        |row| row.get(0),
    ).unwrap();
    let dispatch_target: String = conn.query_row(
        "SELECT target_id FROM action_dispatches WHERE id='dispatch-1'",
        [],
        |row| row.get(0),
    ).unwrap();

    assert_eq!(binding_target, production_id.to_string());
    assert_eq!(dispatch_target, production_id.to_string());
}
```

- [ ] **Step 4: 运行动作中心和 Todo Rust 测试**

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml action_center -- --nocapture
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml todo -- --nocapture
```

Expected: PASS，既有派发终态、Todo 自动完成和启动恢复测试不回归。

- [ ] **Step 5: 提交动作目标迁移**

```powershell
git add apps/desktop/src-tauri/src/tools/release_package.rs apps/desktop/src-tauri/src/tools/action_center/definitions.rs apps/desktop/src-tauri/src/tools/action_center/bindings.rs apps/desktop/src-tauri/src/tools/action_center/dispatches.rs
git commit -m "feat(action-center): 绑定上线包环境目标"
```

### Task 5: 后端所有预检和启动入口改用环境 ID

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/release_package.rs`
- Modify: `apps/desktop/src-tauri/src/tools/release_package_remote.rs`
- Test: both Rust files

- [ ] **Step 1: 写类型专属 action、跨环境令牌和生产确认失败测试**

增加解析测试：

```rust
#[test]
fn production_start_requires_explicit_confirmation() {
    assert_eq!(
        parse_production_confirmation(&json!({}), ReleasePackageEnvironmentKind::Production)
            .unwrap_err(),
        "生产环境发布需要明确确认"
    );
    assert!(parse_production_confirmation(
        &json!({ "productionConfirmed": true }),
        ReleasePackageEnvironmentKind::Production,
    ).is_ok());
    assert!(parse_production_confirmation(
        &json!({}),
        ReleasePackageEnvironmentKind::Test,
    ).is_ok());
}
```

把 `prepare`、`target_check`、`remote_probe`、`host_trust`、`remote_preflight` 和命令重试测试 fixture 改为传 `environmentId`。在 remote store 测试中创建两个环境 ID，确认测试环境不能消费生产环境的 probe/preflight token。

- [ ] **Step 2: 运行 release_package 与 remote 定向测试确认失败**

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml production_start_requires_explicit_confirmation -- --nocapture
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package_remote -- --nocapture
```

Expected: FAIL，确认解析函数缺失，现有绑定仍使用 `project_id`。

- [ ] **Step 3: 实现环境入口和预检绑定**

所有运行 action 先严格读取正整数 `environmentId`，再调用 `load_environment`。删除运行入口对 `projectId` 的配置选择依赖。

给 `PreflightBinding` 增加：

```rust
pub struct PreflightBinding {
    pub environment_id: i64,
    pub project_id: i64,
    pub environment: ReleasePackageEnvironmentKind,
    pub endpoint: RemoteEndpoint,
    pub auth_type: String,
    pub vault_entry_id: Option<i64>,
    pub private_key_path: String,
    pub targets: Vec<RemoteTarget>,
}
```

probe snapshot、host trust 和 preflight 消费都比较 `environment_id`。`prepare_with_conn`、`target_check_with_conn`、`upload_endpoint_with_conn`、`remote_probe_with_conn` 和 `remote_preflight_with_conn` 参数改为环境 ID，并从环境快照读取类型专属配置。

实现严格生产确认：

```rust
fn parse_production_confirmation(
    payload: &Value,
    environment: ReleasePackageEnvironmentKind,
) -> Result<(), String> {
    let confirmed = payload.get("productionConfirmed");
    match environment {
        ReleasePackageEnvironmentKind::Test if confirmed.is_none() => Ok(()),
        ReleasePackageEnvironmentKind::Test => Err("测试环境启动不能携带生产确认参数".into()),
        ReleasePackageEnvironmentKind::Production if confirmed == Some(&Value::Bool(true)) => Ok(()),
        ReleasePackageEnvironmentKind::Production => Err("生产环境发布需要明确确认".into()),
    }
}
```

在占用运行槽、关联动作派发和启动线程之前调用该函数。上传重试与命令重试是已确认生产运行的恢复动作，但仍必须验证令牌绑定环境；重试入口不重新接收通用布尔确认。

- [ ] **Step 4: 运行 release_package Rust 测试**

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package -- --nocapture
```

Expected: PASS，类型不匹配、Vault、主机信任、预检消费和生产确认测试全部通过。

- [ ] **Step 5: 提交后端环境入口**

```powershell
git add apps/desktop/src-tauri/src/tools/release_package.rs apps/desktop/src-tauri/src/tools/release_package_remote.rs
git commit -m "feat(release-package): 按环境执行预检与启动"
```

### Task 6: 让运行事件、重试令牌和通知携带环境身份

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/release_package_runtime.rs`
- Modify: `apps/desktop/src-tauri/src/global_notification.rs`
- Modify: `apps/desktop/src/types/global-notification.ts`
- Modify: `apps/desktop/src/utils/globalNotification.ts`
- Test: corresponding Rust and TypeScript tests

- [ ] **Step 1: 写环境事件、令牌隔离和通知快照失败测试**

在 runtime 测试 sink 中断言：

```rust
assert_eq!(status.environment_id, 42);
assert_eq!(status.project_id, 7);
assert_eq!(status.environment, ReleasePackageEnvironmentKind::Production);
assert_eq!(log.environment_id, 42);
```

把令牌测试扩展为：

```rust
let descriptor = RetryDescriptor {
    manifests: vec![ArtifactManifest {
        target: ReleaseTarget::Frontend,
        source_path: PathBuf::from("D:\\portal\\dist"),
        entries: vec![ArtifactEntry {
            relative_path: "index.html".into(),
            size: 5,
        }],
        file_count: 1,
        total_bytes: 5,
    }],
    commands: Vec::new(),
};
let token = issue_retry(42, descriptor).unwrap();
assert_eq!(retry_targets(&token, 41).unwrap_err(), "上传重试令牌无效或与当前环境不匹配");
assert_eq!(retry_targets(&token, 42).unwrap(), vec![ReleaseTarget::Frontend]);
```

在 `global_notification.rs` 增加序列化断言：

```rust
assert_eq!(value["environmentId"], 42);
assert_eq!(value["environment"], "production");
assert_eq!(value["projectName"], "客户门户");
```

在 `globalNotification.test.ts` 加入环境字段缺失或非法时返回无效通知的用例。

- [ ] **Step 2: 运行 runtime 和通知测试确认失败**

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package_runtime -- --nocapture
pnpm --filter @lazycat/desktop test -- src/utils/globalNotification.test.ts
```

Expected: FAIL，事件和通知没有环境字段，重试仍按项目 ID 绑定。

- [ ] **Step 3: 实现稳定运行身份并贯穿所有发射点**

在 runtime 定义单一身份结构并传递引用，避免每个 helper 重复拼参数：

```rust
#[derive(Clone, Debug)]
struct RunIdentity {
    run_id: String,
    environment_id: i64,
    project_id: i64,
    project_name: String,
    environment: ReleasePackageEnvironmentKind,
}
```

`LogEvent`、`StatusEvent` 和 `GlobalNotification::ReleasePackage` 增加环境 ID 与类型。`emit_status`、`emit_upload_status`、`emit_command_status`、`emit_system_log`、`emit_terminal_result` 和上传进度 reporter 都接收 `&RunIdentity`。

`RetryJob`、`CommandRetryJob`、`CommandAuthBinding` 与对应 issue/prepare/consume 函数从 `project_id` 绑定改为 `environment_id` 绑定；保留 `project_id` 只用于展示和动作终态。错误文案统一使用“当前环境不匹配”。配置安全快照继续保存 endpoint、远程目标与命令，不从最新界面草稿重建。

前端通知类型增加：

```ts
environmentId: number;
environment: ReleasePackageEnvironmentKind;
```

`isReleasePackageNotification` 严格校验正整数环境 ID 和固定环境类型。

- [ ] **Step 4: 运行 runtime、通知和动作派发测试**

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package_runtime -- --nocapture
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml global_notification -- --nocapture
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml action_center -- --nocapture
pnpm --filter @lazycat/desktop test -- src/utils/globalNotification.test.ts src/components/GlobalNotificationPopup.test.ts
```

Expected: PASS，通知终态、重试一次性消费和 Todo run 关联不回归。

- [ ] **Step 5: 提交运行身份**

```powershell
git add apps/desktop/src-tauri/src/tools/release_package_runtime.rs apps/desktop/src-tauri/src/global_notification.rs apps/desktop/src/types/global-notification.ts apps/desktop/src/utils/globalNotification.ts apps/desktop/src/utils/globalNotification.test.ts
git commit -m "feat(release-package): 隔离环境运行身份"
```

### Task 7: 前端运行态、预检和命令重试统一使用环境 ID

**Files:**

- Modify: `apps/desktop/src/composables/useReleasePackageRuntime.ts`
- Modify: `apps/desktop/src/composables/useReleasePackageUploadPreflight.ts`
- Modify: `apps/desktop/src/composables/useReleasePackageCommandRetry.ts`
- Test: corresponding `*.test.ts`

- [ ] **Step 1: 写环境级运行隔离和 composable payload 失败测试**

在 `useReleasePackageRuntime.test.ts` 增加同项目两个环境不会互相覆盖：

```ts
runtime.beginStart(41, ["frontend"]);
runtime.bindStartedRun("run-test", 41);
emit("release-package://status", {
  runId: "run-test",
  environmentId: 41,
  projectId: 7,
  environment: "test",
  status: "succeeded",
  phase: "overall",
});

runtime.beginStart(42, ["backend"]);
runtime.bindStartedRun("run-prod", 42);
emit("release-package://status", {
  runId: "run-prod",
  environmentId: 42,
  projectId: 7,
  environment: "production",
  status: "failed",
  phase: "overall",
  error: "prod failed",
});

expect(runtime.getEnvironmentRuntime(41).status).toBe("succeeded");
expect(runtime.getEnvironmentRuntime(42).error).toBe("prod failed");
```

在 upload preflight 和 command retry 测试中精确断言每个 invoke payload 使用 `environmentId` 且不包含 `projectId`。

- [ ] **Step 2: 运行三个 composable 测试确认失败**

Run:

```powershell
pnpm --filter @lazycat/desktop test -- src/composables/useReleasePackageRuntime.test.ts src/composables/useReleasePackageUploadPreflight.test.ts src/composables/useReleasePackageCommandRetry.test.ts
```

Expected: FAIL，当前 Map 和 payload 仍使用项目 ID。

- [ ] **Step 3: 实现环境级前端状态**

`projectRuntimes` 改名为：

```ts
const environmentRuntimes = reactive(new Map<number, ReleasePackageEnvironmentRuntime>());
```

`getEnvironmentRuntime(environmentId)`、`beginStart(environmentId, targets)`、`bindStartedRun(runId, environmentId)` 统一按环境 ID 操作。事件必须同时满足 `runId` 和 `environmentId`；事件中的项目 ID 和环境类型写入 runtime 只读身份字段。

`useReleasePackageUploadPreflight` 的 `PreflightInput`、`probe`、`trustHost`、`check` 全部使用 `environmentId`。`useReleasePackageCommandRetry` 将本地 `projectId` ref 改为 `environmentId`，prepare/trust/preflight/start payload 只发送环境 ID。

保留现有 reset generation token、秘密清理和过期请求竞争处理，不因参数改名删除这些分支。

- [ ] **Step 4: 运行 composable 测试确认通过**

Run:

```powershell
pnpm --filter @lazycat/desktop test -- src/composables/useReleasePackageRuntime.test.ts src/composables/useReleasePackageUploadPreflight.test.ts src/composables/useReleasePackageCommandRetry.test.ts
```

Expected: PASS，旧的 token 清理、过期请求和互斥重试测试继续通过。

- [ ] **Step 5: 提交前端运行身份**

```powershell
git add apps/desktop/src/composables/useReleasePackageRuntime.ts apps/desktop/src/composables/useReleasePackageRuntime.test.ts apps/desktop/src/composables/useReleasePackageUploadPreflight.ts apps/desktop/src/composables/useReleasePackageUploadPreflight.test.ts apps/desktop/src/composables/useReleasePackageCommandRetry.ts apps/desktop/src/composables/useReleasePackageCommandRetry.test.ts
git commit -m "feat(release-package): 按环境隔离前端运行态"
```

### Task 8: 改造面板为公共配置加固定环境切换

**Files:**

- Modify: `apps/desktop/src/components/ReleasePackagePanel.vue`
- Modify: `apps/desktop/src/components/ReleasePackagePanel.test.ts`

- [ ] **Step 1: 写默认测试环境、待配置、dirty 切换和保存 payload 失败测试**

在面板测试加入源码契约与最小挂载行为：

```ts
it("renders fixed test and production environments and defaults to test", () => {
  expect(source).toContain('value="test"');
  expect(source).toContain('value="production"');
  expect(source).toContain(
    'const selectedEnvironmentKind = ref<ReleasePackageEnvironmentKind>("test")',
  );
  expect(source).toContain('environment.configured ? "已配置" : "待配置"');
});

it("guards dirty environment switches and saves shared plus selected environment", () => {
  expect(source).toContain("async function selectEnvironment");
  expect(source).toContain("await confirmDiscardChanges()");
  expect(source).toContain("selectedEnvironmentKind.value = environment");
  expect(source).toContain("async function saveProject");
  expect(source).toContain("projectDraft");
  expect(source).toContain("environmentDraft");
  expect(source).toContain("environmentId");
});
```

将 mounted fixture 返回一个项目、两条环境，并断言挂载后环境控件显示测试环境，测试草稿不含生产命令。

先从 `../utils/releasePackage` 导入 `createEmptyReleasePackageEnvironmentDraft`，再用以下 fixture 替换旧单层项目：

```ts
const mountedProject: ReleasePackageProject = {
  id: 7,
  name: "Portal",
  frontendProjectPath: "C:\\portal\\web",
  backendProjectPath: "C:\\portal\\api",
  createdAt: "2026-07-28T00:00:00Z",
  updatedAt: "2026-07-28T00:00:00Z",
  environments: [
    {
      ...createEmptyReleasePackageEnvironmentDraft(),
      id: 41,
      projectId: 7,
      environment: "test",
      configured: true,
      packageType: "local_archive",
      outputRoot: "D:\\releases-test",
      frontendBuildCommand: "pnpm build:test",
      frontendArtifactPath: "dist",
      backendBuildCommand: "mvn package -Ptest",
      backendArtifactPath: "target/portal.jar",
      createdAt: "2026-07-28T00:00:00Z",
      updatedAt: "2026-07-28T00:00:00Z",
    },
    {
      ...createEmptyReleasePackageEnvironmentDraft(),
      id: 42,
      projectId: 7,
      environment: "production",
      configured: true,
      packageType: "server_upload",
      frontendBuildCommand: "pnpm build:prod",
      frontendArtifactPath: "dist",
      backendBuildCommand: "mvn package -Pprod",
      backendArtifactPath: "target/portal.jar",
      sshAuthType: "private_key",
      sshHost: "deploy.internal",
      sshPort: 22,
      sshUsername: "deploy",
      sshPrivateKeyPath: "C:\\keys\\deploy",
      frontendRemoteDir: "/srv/portal/web",
      backendRemotePath: "/srv/portal/portal.jar",
      createdAt: "2026-07-28T00:00:00Z",
      updatedAt: "2026-07-28T00:00:00Z",
    },
  ],
};
```

- [ ] **Step 2: 运行面板测试确认失败**

Run:

```powershell
pnpm --filter @lazycat/desktop test -- src/components/ReleasePackagePanel.test.ts
```

Expected: FAIL，环境控件和拆分草稿不存在。

- [ ] **Step 3: 实现环境选择、草稿恢复和事务保存 UI**

把单一 `draft` 拆成：

```ts
const projectDraft = reactive<ReleasePackageProjectDraft>(createEmptyReleasePackageProjectDraft());
const environmentDraft = reactive<ReleasePackageEnvironmentDraft>(
  createEmptyReleasePackageEnvironmentDraft(),
);
const selectedEnvironmentKind = ref<ReleasePackageEnvironmentKind>("test");

const selectedEnvironment = computed(
  () =>
    selectedProject.value?.environments.find(
      (item) => item.environment === selectedEnvironmentKind.value,
    ) ?? null,
);
```

新增 `restoreSelectedDrafts()`，只从项目公共字段和当前环境复制草稿。`loadProjects`、`selectProject`、`newProject` 完成后都把环境重置为 `test`。`selectEnvironment` 先走现有 dirty 确认，再清理一次性预检/重试状态，最后恢复目标环境草稿。

模板在项目标题旁增加固定 `el-radio-group`。测试环境使用普通 tag；生产环境使用 `type="danger"` tag，但运行成功/失败标签继续由状态语义决定。公共工程目录表单绑定 `projectDraft`，其余构建与交付表单绑定 `environmentDraft`。

`saveProject` 先分别运行公共与环境校验，再提交：

```ts
{
  id: selectedProject.value?.id,
  environmentId: selectedEnvironment.value?.id,
  environment: selectedEnvironmentKind.value,
  project: normalizeReleasePackageProjectDraft(projectDraft),
  environmentConfig: normalizeReleasePackageEnvironmentDraft(environmentDraft),
}
```

新建项目保存成功后使用后端返回的项目 ID 和环境 ID 刷新列表并恢复测试环境。删除仍传项目 ID。

- [ ] **Step 4: 运行面板与纯函数测试**

Run:

```powershell
pnpm --filter @lazycat/desktop test -- src/components/ReleasePackagePanel.test.ts src/utils/releasePackage.test.ts
```

Expected: PASS，既有布局、Vault、打包类型、日志、目录选择和 dirty 防护测试继续通过。

- [ ] **Step 5: 提交环境配置界面**

```powershell
git add apps/desktop/src/components/ReleasePackagePanel.vue apps/desktop/src/components/ReleasePackagePanel.test.ts
git commit -m "feat(release-package): 增加测试生产环境切换"
```

### Task 9: 接通生产确认、动作意图和环境通知展示

**Files:**

- Modify: `apps/desktop/src/components/ReleasePackagePanel.vue`
- Modify: `apps/desktop/src/components/ReleasePackagePanel.test.ts`
- Modify: `apps/desktop/src/components/GlobalNotificationPopup.vue`
- Modify: `apps/desktop/src/components/GlobalNotificationPopup.test.ts`
- Modify: `apps/desktop/src/composables/useActionDispatchIntent.test.ts`
- Modify: `apps/desktop/src/composables/useTodoActionBinding.test.ts`

- [ ] **Step 1: 写生产确认和环境动作意图失败测试**

面板测试增加：

```ts
it("requires the final production confirmation and sends it only for production", () => {
  expect(source).toContain("确认生产发布");
  expect(source).toContain('selectedEnvironmentKind.value === "production"');
  expect(source).toContain("async function confirmStart");
  expect(source).toContain("productionConfirmed");
  expect(source).toContain("selectedEnvironment.value.id");
});

it("selects an action target by environment id without keeping the default test environment", () => {
  expect(source).toContain("async function applyActionDispatchIntent");
  expect(source).toContain("findEnvironmentById");
  expect(source).toContain("selectedEnvironmentKind.value = target.environment");
  expect(source).toContain("await prepareStart()");
});
```

通知组件与动作绑定测试增加以下精确断言，生产使用危险色 tag，通知标题仍由终态决定：

```ts
it("renders the release environment without replacing the terminal status style", () => {
  expect(source).toContain('currentPackage.value.environment === "production"');
  expect(source).toContain('type="danger"');
  expect(source).toContain("currentPackage.value.projectName");
  expect(source).toContain("生产环境");
  expect(source).toContain("测试环境");
  expect(source).toContain("releasePackageNotificationCopy");
});
```

把 `useTodoActionBinding.test.ts` 中上线包目标 fixture 改为：

```ts
{
  id: "42",
  label: "客户门户 · 生产环境",
  available: true,
}
```

并断言选择后 `draft.actionTargetId === "42"`；保留不可用目标测试，用 `{ id: "41", label: "客户门户 · 测试环境", available: false, unavailableReason: "环境配置不完整" }` 验证禁用原因。

- [ ] **Step 2: 运行面板、通知和动作 composable 测试确认失败**

Run:

```powershell
pnpm --filter @lazycat/desktop test -- src/components/ReleasePackagePanel.test.ts src/components/GlobalNotificationPopup.test.ts src/composables/useActionDispatchIntent.test.ts src/composables/useTodoActionBinding.test.ts
```

Expected: FAIL，动作 target 仍按项目 ID，生产最终确认和通知环境标签不存在。

- [ ] **Step 3: 实现环境动作定位与生产最终确认**

实现 `findEnvironmentById`，从项目聚合列表返回 `{ project, environment }`。`applyActionDispatchIntent` 先检查 dirty/running，再刷新项目列表，按 `intent.targetId` 查环境 ID，设置项目和环境草稿，最后复用 `prepareStart`；找不到时调用 `stopPendingActionDispatch("failed", "上线包环境配置不存在")`，配置不完整时调用 `stopPendingActionDispatch("failed", "上线包环境配置不完整")`。

`prepareStart`、主机信任、预检、重试和 `runtime.beginStart` 全部传 `selectedEnvironment.value.id`。生产最终确认状态使用独立 ref：

```ts
const productionConfirmed = ref(false);

function confirmProductionStart(): void {
  if (selectedEnvironmentKind.value !== "production") return;
  productionConfirmed.value = true;
}
```

只有生产摘要完整展示项目名、目标、打包类型、最终本地路径或 SSH 端点与远程路径后才显示“确认生产发布”。关闭、返回、预检失败、配置切换和启动完成都将其重置为 `false`。`createReleasePackageStartPayload` 接收 `productionConfirmed`，测试环境不发送该字段。

`GlobalNotificationPopup.vue` 使用通知中的 `environment` 渲染环境 tag，并组合显示 `projectName · 测试环境/生产环境`。不要把生产成功通知改成错误样式。

- [ ] **Step 4: 运行全部前端相关测试与类型检查**

Run:

```powershell
pnpm --filter @lazycat/desktop test -- src/components/ReleasePackagePanel.test.ts src/components/GlobalNotificationPopup.test.ts src/utils/releasePackage.test.ts src/utils/globalNotification.test.ts src/composables/useReleasePackageRuntime.test.ts src/composables/useReleasePackageUploadPreflight.test.ts src/composables/useReleasePackageCommandRetry.test.ts src/composables/useActionDispatchIntent.test.ts src/composables/useTodoActionBinding.test.ts
pnpm typecheck
```

Expected: 全部 PASS，typecheck 退出码 0。

- [ ] **Step 5: 提交生产保护与动作集成**

```powershell
git add apps/desktop/src/components/ReleasePackagePanel.vue apps/desktop/src/components/ReleasePackagePanel.test.ts apps/desktop/src/components/GlobalNotificationPopup.vue apps/desktop/src/components/GlobalNotificationPopup.test.ts apps/desktop/src/composables/useActionDispatchIntent.test.ts apps/desktop/src/composables/useTodoActionBinding.test.ts
git commit -m "feat(release-package): 增加生产发布保护"
```

### Task 10: 完整回归、经验文档与最终检查

**Files:**

- Modify: `docs/experience/release-package.md`
- Verify: all files changed by Tasks 1-9

- [ ] **Step 1: 更新上线包经验索引和不变量**

在 `docs/experience/release-package.md` 目录增加“固定环境使用稳定环境 ID”条目，正文写明：

```markdown
## 固定环境使用稳定环境 ID

同一上线包项目只保存一份项目名和前后端工程目录，测试、生产环境分别保存构建与交付配置。环境表使用自增 ID 作为 IPC、运行、令牌和动作绑定的稳定引用，同时以 `(project_id, environment)` 唯一约束保留业务语义。

运行入口只接收环境 ID，后端加载环境及所属项目形成不可变快照。日志、状态、通知、上传预检和重试令牌都携带或绑定环境身份，不能依赖前端当前选择，也不能跨环境复用。测试环境默认且不得继承生产配置；生产启动必须经过显式确认。
```

- [ ] **Step 2: 运行 Rust 全域定向验证**

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package -- --nocapture
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml action_center -- --nocapture
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml global_notification -- --nocapture
```

Expected: 全部 PASS，无新增 panic、dead code 或失败终态回归。

- [ ] **Step 3: 运行前端测试、类型检查和渲染层构建**

Run:

```powershell
pnpm test
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
```

Expected: 全部退出码 0；构建不访问公网 CDN。

- [ ] **Step 4: 检查差异边界和格式**

Run:

```powershell
git status --short
git diff --check
git diff --stat
```

Expected: 只有本计划文件和实现涉及文件发生变化；`git diff --check` 无输出。重点人工核对：旧 `projectId` 不再作为运行配置选择参数、环境 ID 不被误当项目 ID、生产确认在占用运行槽前完成、测试环境没有复制生产秘密。

- [ ] **Step 5: 提交经验与最终验证结果**

```powershell
git add docs/experience/release-package.md
git commit -m "docs: 记录上线包固定环境边界"
```

提交后运行：

```powershell
git status --short
git log -10 --oneline
```

Expected: 工作树为空；最近提交按 Tasks 1-10 顺序出现。

## 手工冒烟清单

仅在用户明确要求启动产品 UI 时执行，不自动运行 `pnpm dev`：

1. 新建项目，确认默认测试环境，保存后生产环境为“待配置”。
2. 为测试、生产设置不同命令和不同非生产远端目录，来回切换确认草稿不串线。
3. 有未保存修改时切换环境，验证保存、放弃、取消三条路径。
4. 测试环境执行本地归档，确认无生产确认步骤。
5. 生产环境进入最终确认，核对项目、服务器和远程路径，关闭弹窗后确认状态被清除。
6. Todo 分别绑定测试、生产环境，确认打开正确环境；生产绑定仍要求最终确认。
7. 制造上传失败后尝试跨环境重试，确认令牌被拒绝；原环境配置未变化时可正常重试。
8. 检查日志和全局通知均显示正确环境，且不输出 Vault 密码或私钥口令。
