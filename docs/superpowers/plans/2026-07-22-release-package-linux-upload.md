# 上线包 Linux 上传 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为上线包工具增加构建前 SSH 预检、密码/私钥认证、前端目录与后端文件 SFTP 上传、安全完整替换、取消和失败重试能力。

**Architecture:** 保留现有“并行构建 → 本地归档提交”闭环，在归档成功后增加独立上传阶段。Rust 使用 `ssh2` 提供 SSH/SFTP 基础能力，`release_package_remote.rs` 管理连接、信任和短期凭据，`release_package_deploy.rs` 管理产物清单与远端替换事务；现有 runtime 只负责编排和终态聚合，Vue 负责配置、预检确认与独立上传日志。

**Tech Stack:** Vue 3、TypeScript、Element Plus、Vitest、Tauri 2、Rust、rusqlite、ssh2/libssh2、OpenSSL、SFTP

---

## 实施边界与文件职责

- `apps/desktop/src-tauri/Cargo.toml`、`Cargo.lock`：引入并锁定 `ssh2`，先完成 Windows 原生依赖编译闸门。
- `apps/desktop/src-tauri/src/tools/release_package.rs`：项目上传配置 CRUD、known-host 数据、上传 actions 和启动参数校验。
- `apps/desktop/src-tauri/src/tools/helpers.rs`：调用上线包幂等 schema 迁移入口，不散落字段迁移 SQL。
- `apps/desktop/src-tauri/src/tools/release_package_remote.rs`：SSH 握手、指纹、认证、SFTP 适配、探测/预检令牌和敏感值生命周期。
- `apps/desktop/src-tauri/src/tools/release_package_deploy.rs`：本地产物清单、递归上传、进度、远端临时/备份/正式目标事务与回滚。
- `apps/desktop/src-tauri/src/tools/release_package_archive.rs`：返回归档目标名称，并提供 ZIP 重试源的安全解压能力。
- `apps/desktop/src-tauri/src/tools/release_package_runtime.rs`：消费预检令牌，编排构建、归档、上传、重试和取消。
- `apps/desktop/src-tauri/src/tools/mod.rs`：注册新增 Rust 模块。
- `apps/desktop/src/bridge/tauri.ts`：注册 `remote-probe`、`host-trust`、`remote-preflight`、`upload-retry` 通道。
- `apps/desktop/src/types/release-package.ts`：上传配置、预检、进度、重试和终态契约。
- `apps/desktop/src/utils/releasePackage.ts`：上传配置默认值、归一化、校验和状态文案。
- `apps/desktop/src/composables/useReleasePackageRuntime.ts`：项目级 upload lane、进度和整体运行态。
- `apps/desktop/src/composables/useReleasePackageUploadPreflight.ts`：无凭据持久状态的主机探测、信任和预检编排。
- `apps/desktop/src/components/ReleasePackagePanel.vue`：服务器配置区、启动模式、凭据输入、预检结果、覆盖确认、上传日志和重试入口。
- `apps/desktop/src-tauri/src/global_notification.rs`、`apps/desktop/src/types/global-notification.ts`、`apps/desktop/src/utils/globalNotification.ts`、`apps/desktop/src/components/GlobalNotificationPopup.vue`：上传终态通知。
- `docs/experience/release-package.md`：沉淀预检、主机信任、凭据和远端事务边界。

实现直接在 `main` 进行；开始 Task 1 前执行 `git status --short`，若上述目标文件出现本计划之外的新改动，先读 diff 并避开冲突。

### Task 1: 验证 ssh2 在 Windows Tauri 构建中可用

**Files:**

- Modify: `apps/desktop/src-tauri/Cargo.toml`
- Modify: `apps/desktop/src-tauri/Cargo.lock`
- Modify: `apps/desktop/src-tauri/src/tools/mod.rs`
- Create: `apps/desktop/src-tauri/src/tools/release_package_remote.rs`

- [ ] **Step 1: 添加最小编译测试**

先创建只验证 crate API 和线程模型的模块，不连接网络：

```rust
use ssh2::Session;

pub(crate) fn new_session() -> Result<Session, String> {
    Session::new().map_err(|error| format!("创建 SSH 会话失败：{error}"))
}

#[cfg(test)]
mod tests {
    use super::new_session;

    #[test]
    fn creates_an_ssh_session_without_network_access() {
        assert!(new_session().is_ok());
    }
}
```

在 `tools/mod.rs` 注册：

```rust
pub mod release_package_remote;
```

- [ ] **Step 2: 运行测试并确认缺少依赖**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package_remote::tests::creates_an_ssh_session_without_network_access -- --nocapture`

Expected: FAIL，提示无法解析 `ssh2` crate。

- [ ] **Step 3: 加入 ssh2 依赖**

在 `[dependencies]` 增加：

```toml
ssh2 = { version = "0.9", features = ["vendored-openssl"] }
```

让 Cargo 正常更新 `apps/desktop/src-tauri/Cargo.lock`。不得安装系统级 DLL，也不得把下载文件放入 `resources/`。

- [ ] **Step 4: 验证测试和 Windows 编译闸门**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package_remote::tests::creates_an_ssh_session_without_network_access -- --nocapture`

Expected: PASS。

Run: `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml`

Expected: PASS，且没有 libssh2/OpenSSL 链接错误。若依赖下载被网络沙箱拦截，按执行环境流程申请网络权限后重跑同一命令，不能换成系统 `scp` 绕过。

- [ ] **Step 5: 提交依赖闸门**

```powershell
git add -- apps/desktop/src-tauri/Cargo.toml apps/desktop/src-tauri/Cargo.lock apps/desktop/src-tauri/src/tools/mod.rs apps/desktop/src-tauri/src/tools/release_package_remote.rs
git commit -m "chore(release-package): 接入 ssh2 依赖"
```

### Task 2: 扩展上传项目配置与数据库迁移

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/release_package.rs`
- Modify: `apps/desktop/src-tauri/src/tools/helpers.rs`
- Modify: `apps/desktop/src/types/release-package.ts`
- Modify: `apps/desktop/src/utils/releasePackage.ts`
- Modify: `apps/desktop/src/utils/releasePackage.test.ts`

- [ ] **Step 1: 写 Rust 旧库迁移与 CRUD 失败测试**

在 `release_package.rs` 测试模块增加一个只有旧字段的表，调用新的 `ensure_schema` 后验证默认值；再验证保存/读取上传配置且 payload 不包含密码：

```rust
#[test]
fn schema_migrates_existing_projects_and_never_persists_passwords() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE release_package_projects (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            output_root TEXT NOT NULL,
            frontend_project_path TEXT NOT NULL,
            frontend_build_command TEXT NOT NULL,
            frontend_artifact_path TEXT NOT NULL,
            frontend_artifact_mode TEXT NOT NULL,
            backend_project_path TEXT NOT NULL,
            backend_build_command TEXT NOT NULL,
            backend_artifact_path TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        INSERT INTO release_package_projects(
            name, output_root, frontend_project_path, frontend_build_command,
            frontend_artifact_path, frontend_artifact_mode, backend_project_path,
            backend_build_command, backend_artifact_path
        ) VALUES ('portal', 'D:\\release', 'D:\\web', 'pnpm build', 'dist',
                  'copy_directory', 'D:\\server', 'mvn package', 'target/app.jar');"
    ).unwrap();

    ensure_schema(&conn).unwrap();
    let projects = project_list_with_conn(&conn).unwrap();
    let project = &projects["projects"][0];
    assert_eq!(project["uploadEnabled"], false);
    assert_eq!(project["sshPort"], 22);
    assert_eq!(project["sshAuthType"], "password");
    assert!(serde_json::to_string(project).unwrap().find("password").is_none());
}
```

扩展现有 `project_round_trip` fixture，加入：

```rust
json!({
    "uploadEnabled": true,
    "sshHost": "deploy.example.internal",
    "sshPort": 2222,
    "sshUsername": "deploy",
    "sshAuthType": "private_key",
    "sshPrivateKeyPath": r"C:\Users\tester\.ssh\lazycat",
    "frontendRemoteDir": "/srv/portal/web",
    "backendRemotePath": "/srv/portal/app.jar"
})
```

- [ ] **Step 2: 写前端草稿失败测试**

扩展 `createEmptyReleasePackageDraft` 精确断言：

```ts
expect(createEmptyReleasePackageDraft()).toMatchObject({
  uploadEnabled: false,
  sshHost: "",
  sshPort: 22,
  sshUsername: "",
  sshAuthType: "password",
  sshPrivateKeyPath: "",
  frontendRemoteDir: "",
  backendRemotePath: "",
});
```

新增校验断言：

```ts
const draft = createEmptyReleasePackageDraft();
draft.uploadEnabled = true;
expect(validateReleasePackageDraft(draft)).toBe("请输入服务器地址");
draft.sshHost = "10.0.0.8";
draft.sshUsername = "deploy";
draft.frontendRemoteDir = "/srv/app/web";
draft.backendRemotePath = "/srv/app/app.jar";
expect(validateReleasePackageUpload(draft)).toBeNull();
draft.sshPort = 0;
expect(validateReleasePackageUpload(draft)).toBe("SSH 端口必须在 1 到 65535 之间");
```

- [ ] **Step 3: 运行测试并确认失败**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package::tests::schema_migrates_existing_projects_and_never_persists_passwords -- --nocapture`

Expected: FAIL，`ensure_schema` 或新字段不存在。

Run: `pnpm --filter @lazycat/desktop exec vitest run src/utils/releasePackage.test.ts`

Expected: FAIL，草稿和校验尚无上传字段。

- [ ] **Step 4: 实现幂等 schema、配置读写和前端类型**

在 Rust 配置模型加入以下字段，端口使用 `u16`：

```rust
pub upload_enabled: bool,
pub ssh_host: String,
pub ssh_port: u16,
pub ssh_username: String,
pub ssh_auth_type: String,
pub ssh_private_key_path: String,
pub frontend_remote_dir: String,
pub backend_remote_path: String,
```

增加唯一 schema 入口：

```rust
pub fn ensure_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(RELEASE_PACKAGE_SCHEMA_SQL)
        .map_err(|error| format!("create release package schema failed: {error}"))?;
    for (column, statement) in [
        ("upload_enabled", "ALTER TABLE release_package_projects ADD COLUMN upload_enabled INTEGER NOT NULL DEFAULT 0"),
        ("ssh_host", "ALTER TABLE release_package_projects ADD COLUMN ssh_host TEXT NOT NULL DEFAULT ''"),
        ("ssh_port", "ALTER TABLE release_package_projects ADD COLUMN ssh_port INTEGER NOT NULL DEFAULT 22"),
        ("ssh_username", "ALTER TABLE release_package_projects ADD COLUMN ssh_username TEXT NOT NULL DEFAULT ''"),
        ("ssh_auth_type", "ALTER TABLE release_package_projects ADD COLUMN ssh_auth_type TEXT NOT NULL DEFAULT 'password'"),
        ("ssh_private_key_path", "ALTER TABLE release_package_projects ADD COLUMN ssh_private_key_path TEXT NOT NULL DEFAULT ''"),
        ("frontend_remote_dir", "ALTER TABLE release_package_projects ADD COLUMN frontend_remote_dir TEXT NOT NULL DEFAULT ''"),
        ("backend_remote_path", "ALTER TABLE release_package_projects ADD COLUMN backend_remote_path TEXT NOT NULL DEFAULT ''"),
    ] {
        let exists = conn.prepare("PRAGMA table_info(release_package_projects)")
            .and_then(|mut query| {
                let rows = query.query_map([], |row| row.get::<_, String>(1))?;
                Ok(rows.filter_map(Result::ok).any(|name| name == column))
            })
            .map_err(|error| format!("inspect release package schema failed: {error}"))?;
        if !exists {
            conn.execute_batch(statement)
                .map_err(|error| format!("migrate release package column {column} failed: {error}"))?;
        }
    }
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS release_package_known_hosts (
            host TEXT NOT NULL,
            port INTEGER NOT NULL,
            key_type TEXT NOT NULL,
            fingerprint_sha256 TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY(host, port)
        );"
    ).map_err(|error| format!("create release package known hosts failed: {error}"))
}
```

`helpers.rs` 将当前 `execute_batch(RELEASE_PACKAGE_SCHEMA_SQL)` 和单独的 `output_root` 迁移替换为：

```rust
super::release_package::ensure_schema(conn)?;
```

TypeScript 增加：

```ts
export type ReleasePackageSshAuthType = "password" | "private_key";

export interface ReleasePackageUploadConfig {
  uploadEnabled: boolean;
  sshHost: string;
  sshPort: number;
  sshUsername: string;
  sshAuthType: ReleasePackageSshAuthType;
  sshPrivateKeyPath: string;
  frontendRemoteDir: string;
  backendRemotePath: string;
}

export interface ReleasePackageProjectDraft extends ReleasePackageUploadConfig {
  // 保留现有本地构建与归档字段，字段名不变。
}
```

`validateReleasePackageUpload` 在 `uploadEnabled=false` 时返回 `null`；启用时严格校验 host、1..65535 端口、用户名、私钥路径条件必填，以及前后端绝对路径非空。密码和私钥口令不得加入任何 project/draft 类型。

- [ ] **Step 5: 运行定向测试并提交**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package -- --nocapture`

Expected: PASS。

Run: `pnpm --filter @lazycat/desktop exec vitest run src/utils/releasePackage.test.ts`

Expected: PASS。

```powershell
git add -- apps/desktop/src-tauri/src/tools/release_package.rs apps/desktop/src-tauri/src/tools/helpers.rs apps/desktop/src/types/release-package.ts apps/desktop/src/utils/releasePackage.ts apps/desktop/src/utils/releasePackage.test.ts
git commit -m "feat(release-package): 保存服务器上传配置"
```

### Task 3: 实现主机探测、指纹信任与远程路径校验

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/release_package_remote.rs`
- Modify: `apps/desktop/src-tauri/src/tools/release_package.rs`
- Modify: `apps/desktop/src/bridge/tauri.ts`
- Modify: `apps/desktop/src/types/release-package.ts`

- [ ] **Step 1: 写远程路径和信任判断失败测试**

在 remote 模块测试：

```rust
#[test]
fn validates_absolute_linux_deployment_paths() {
    assert_eq!(validate_remote_dir("/srv/app/web").unwrap(), "/srv/app/web");
    assert_eq!(validate_remote_file("/srv/app/app.jar").unwrap(), "/srv/app/app.jar");
    for invalid in ["", "/", "relative/path", "/srv/../root", "/srv/./app", "/srv/app\0x"] {
        assert!(validate_remote_dir(invalid).is_err(), "accepted {invalid:?}");
    }
    assert!(validate_remote_file("/srv/app/").is_err());
}

#[test]
fn classifies_known_host_without_silent_replacement() {
    assert_eq!(classify_trust(None, "SHA256:new"), HostTrust::Unknown);
    assert_eq!(classify_trust(Some("SHA256:new"), "SHA256:new"), HostTrust::Trusted);
    assert_eq!(classify_trust(Some("SHA256:old"), "SHA256:new"), HostTrust::Changed);
}
```

在 `release_package.rs` 测试 trusted host 的插入与显式替换：

```rust
assert!(trust_host_with_conn(&conn, &probe, false).is_ok());
assert!(trust_host_with_conn(&conn, &changed_probe, false).is_err());
assert!(trust_host_with_conn(&conn, &changed_probe, true).is_ok());
```

- [ ] **Step 2: 运行测试并确认失败**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package_remote -- --nocapture`

Expected: FAIL，新校验和信任类型不存在。

- [ ] **Step 3: 实现探测基础类型与 SSH 握手**

在 remote 模块定义稳定边界：

```rust
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostTrust { Trusted, Unknown, Changed }

#[derive(Clone)]
pub struct RemoteEndpoint {
    pub host: String,
    pub port: u16,
    pub username: String,
}

#[derive(Clone)]
pub struct ProbeSnapshot {
    pub endpoint: RemoteEndpoint,
    pub key_type: String,
    pub fingerprint_sha256: String,
}

pub fn fingerprint_sha256(key: &[u8]) -> String {
    use base64::Engine;
    let digest = openssl::sha::sha256(key);
    format!("SHA256:{}", base64::engine::general_purpose::STANDARD_NO_PAD.encode(digest))
}
```

`probe_host` 使用 `TcpStream::connect_timeout`，对 read/write 设置 10 秒超时，`Session::handshake()` 后通过 `host_key()` 取主机公钥；没有主机公钥时显式失败。此阶段不得调用认证 API。

探测结果放入 `OnceLock<Mutex<HashMap<String, ExpiringProbe>>>`，令牌使用 UUID、五分钟过期。`host_trust` 只能消费有效 probe token，并把规范化后的 host、port、key type 和指纹写入 known-host 表；指纹变化且 `replaceExisting=false` 时返回错误。

- [ ] **Step 4: 接入 actions 和前端契约**

Rust `ACTIONS` 增加 `remote_probe`、`host_trust`。bridge 增加：

```ts
"tool:release-package:remote-probe": { domain: "release_package", action: "remote_probe" },
"tool:release-package:host-trust": { domain: "release_package", action: "host_trust" },
```

前端类型增加：

```ts
export interface ReleasePackageRemoteProbeResult {
  probeToken: string;
  host: string;
  port: number;
  keyType: string;
  fingerprintSha256: string;
  trust: "trusted" | "unknown" | "changed";
  previousFingerprintSha256?: string;
}
```

`remote_probe` 只接收 `{ projectId }`，Rust 必须从数据库重新读取 endpoint；`host_trust` 接收 `{ probeToken, replaceExisting }`，不能接收前端自报指纹。前端收到换发令牌后覆盖 `probeResult.probeToken`；已信任主机不调用 `host_trust`，直接使用 `remote_probe` 返回的令牌。

- [ ] **Step 5: 验证并提交**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package -- --nocapture`

Expected: PASS，无网络单元测试只覆盖纯函数、令牌和数据库；不连接真实服务器。

```powershell
git add -- apps/desktop/src-tauri/src/tools/release_package_remote.rs apps/desktop/src-tauri/src/tools/release_package.rs apps/desktop/src/bridge/tauri.ts apps/desktop/src/types/release-package.ts
git commit -m "feat(release-package): 校验 SSH 主机指纹"
```

### Task 4: 实现认证、真实预检与一次性凭据

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/release_package_remote.rs`
- Modify: `apps/desktop/src-tauri/src/tools/release_package.rs`
- Modify: `apps/desktop/src/bridge/tauri.ts`
- Modify: `apps/desktop/src/types/release-package.ts`

- [ ] **Step 1: 写凭据生命周期和预检绑定失败测试**

```rust
#[test]
fn preflight_token_is_bound_and_consumed_once() {
    let store = PreflightStore::new(Duration::from_secs(300));
    let binding = PreflightBinding::test_value(7, vec![ReleaseTarget::Frontend]);
    let token = store.insert(binding.clone(), AuthSecret::Password(Zeroizing::new("secret".into())));
    assert!(store.consume(&token, &binding).is_ok());
    assert!(store.consume(&token, &binding).is_err());
}

#[test]
fn preflight_token_rejects_changed_remote_paths() {
    let store = PreflightStore::new(Duration::from_secs(300));
    let binding = PreflightBinding::test_value(7, vec![ReleaseTarget::Backend]);
    let token = store.insert(binding.clone(), AuthSecret::Password(Zeroizing::new("secret".into())));
    let mut changed = binding;
    changed.backend_remote_path = "/srv/other/app.jar".into();
    assert!(store.consume(&token, &changed).is_err());
}
```

增加错误脱敏测试：

```rust
let error = authenticate_for_test("deploy", AuthSecret::Password(Zeroizing::new("top-secret".into())))
    .unwrap_err();
assert!(!error.contains("top-secret"));
```

- [ ] **Step 2: 运行测试并确认失败**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package_remote -- --nocapture`

Expected: FAIL，预检存储和认证秘密类型不存在。

- [ ] **Step 3: 实现认证和短期预检存储**

定义不派生 `Debug`/`Serialize` 的秘密类型：

```rust
pub enum AuthSecret {
    Password(zeroize::Zeroizing<String>),
    PrivateKeyPassphrase(Option<zeroize::Zeroizing<String>>),
}

pub struct ConsumedPreflight {
    pub binding: PreflightBinding,
    pub secret: AuthSecret,
    pub expected_existing_targets: Vec<ReleaseTarget>,
}
```

密码调用：

```rust
session.userauth_password(&endpoint.username, password.as_str())
    .map_err(|_| "SSH 用户名或密码认证失败".to_string())?;
```

私钥调用：

```rust
session.userauth_pubkey_file(
    &endpoint.username,
    None,
    private_key_path,
    passphrase.as_ref().map(|value| value.as_str()),
).map_err(|_| "SSH 私钥认证失败，请检查私钥和口令".to_string())?;
```

认证前再次对比 known-host 指纹。私钥路径必须是常规文件。底层错误只能映射为连接、握手、主机不可信、认证、权限、路径或传输错误，不得格式化包含秘密的请求结构。

预检通过 SFTP 完成：检查/按配置创建目标父目录，在同级创建零字节 `.lazycat-preflight-<uuid>`、重命名一次后删除；读取正式目标存在性和类型；发现 `. __lazycat_tmp_`/backup 本次确定路径冲突时失败。实际临时名不包含空格，使用 `target.__lazycat_tmp_<token-prefix>`。

- [ ] **Step 4: 接入 remote_preflight action**

bridge 增加：

```ts
"tool:release-package:remote-preflight": { domain: "release_package", action: "remote_preflight" },
```

请求和响应类型：

```ts
export interface ReleasePackageRemotePreflightInput {
  projectId: number;
  targets: ReleasePackageTarget[];
  probeToken: string;
  password?: string;
  privateKeyPassphrase?: string;
}

export interface ReleasePackageRemoteTargetCheck {
  target: ReleasePackageTarget;
  remotePath: string;
  exists: boolean;
  parentReady: boolean;
  writable: boolean;
}

export interface ReleasePackageRemotePreflightResult {
  preflightToken: string;
  expiresAt: string;
  targets: ReleasePackageRemoteTargetCheck[];
}
```

Rust 严格要求密码模式只能出现 `password`，私钥模式只能出现可选 `privateKeyPassphrase`；空字符串按空秘密处理，不落库。`on_app_exit` 清空 probe/preflight stores。

- [ ] **Step 5: 验证并提交**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package -- --nocapture`

Expected: PASS。

```powershell
git add -- apps/desktop/src-tauri/src/tools/release_package_remote.rs apps/desktop/src-tauri/src/tools/release_package.rs apps/desktop/src/bridge/tauri.ts apps/desktop/src/types/release-package.ts
git commit -m "feat(release-package): 增加 SSH 认证预检"
```

### Task 5: 建立可验证的部署清单和 ZIP 重试源

**Files:**

- Create: `apps/desktop/src-tauri/src/tools/release_package_deploy.rs`
- Modify: `apps/desktop/src-tauri/src/tools/release_package_archive.rs`
- Modify: `apps/desktop/src-tauri/src/tools/mod.rs`

- [ ] **Step 1: 写产物清单和安全解压失败测试**

```rust
#[test]
fn manifest_rejects_changed_files() {
    let root = TestDir::new();
    let source = root.path().join("dist");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("index.html"), "v1").unwrap();
    let manifest = ArtifactManifest::from_directory(ReleaseTarget::Frontend, &source).unwrap();
    fs::write(source.join("index.html"), "changed").unwrap();
    assert!(manifest.verify_source().unwrap_err().contains("发生变化"));
}

#[test]
fn retry_zip_extraction_rejects_path_escape() {
    let root = TestDir::new();
    let zip_path = root.path().join("bad.zip");
    write_test_zip(&zip_path, "../escape.txt", b"bad");
    assert!(extract_retry_zip(&zip_path, &root.path().join("extract")).is_err());
    assert!(!root.path().join("escape.txt").exists());
}
```

增加空目录和后端单文件清单断言：

```rust
assert_eq!(ArtifactManifest::from_directory(ReleaseTarget::Frontend, &empty)?.file_count, 0);
assert_eq!(ArtifactManifest::from_file(ReleaseTarget::Backend, &jar)?.total_bytes, 3);
```

- [ ] **Step 2: 运行测试并确认失败**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package_deploy release_package_archive -- --nocapture`

Expected: FAIL，新模块和清单类型不存在。

- [ ] **Step 3: 实现清单与归档结果**

定义：

```rust
#[derive(Clone, Debug, Serialize)]
pub struct ArtifactEntry {
    pub relative_path: String,
    pub size: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ArtifactManifest {
    pub target: ReleaseTarget,
    pub source_path: PathBuf,
    pub entries: Vec<ArtifactEntry>,
    pub file_count: u64,
    pub total_bytes: u64,
}

#[derive(Clone, Debug)]
pub struct ArchivedTarget {
    pub target: ReleaseTarget,
    pub archive_entry_name: String,
    pub artifact_mode: String,
}
```

目录清单按相对路径排序，路径统一 `/` 分隔；拒绝符号链接，避免上传源逃逸。`verify_source` 重新检查文件集合和大小完全一致。

让 `archive_frontend_artifact` 和 `archive_backend_artifact` 返回实际归档入口名称而不是 `()`，runtime 汇总成 `ArchivedTarget`。现有调用方只需接收返回值，不改变复制/ZIP 行为。

- [ ] **Step 4: 实现 ZIP 重试解压**

`extract_retry_zip` 对每个 entry 使用 `enclosed_name()`；拒绝绝对路径、`..`、符号链接类型和目标根目录逃逸。解压目录使用 `.lazycat-upload-retry-<runId>`，由 guard 在失败、取消和成功结束后清理。解压后重新生成清单，禁止直接相信 ZIP metadata。

- [ ] **Step 5: 验证并提交**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package_deploy release_package_archive -- --nocapture`

Expected: PASS，既有归档测试也保持通过。

```powershell
git add -- apps/desktop/src-tauri/src/tools/release_package_deploy.rs apps/desktop/src-tauri/src/tools/release_package_archive.rs apps/desktop/src-tauri/src/tools/mod.rs
git commit -m "feat(release-package): 生成部署产物清单"
```

### Task 6: 实现 SFTP 递归上传和远端完整替换事务

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/release_package_remote.rs`
- Modify: `apps/desktop/src-tauri/src/tools/release_package_deploy.rs`

- [ ] **Step 1: 写 FakeRemoteFs 事务失败测试**

在 deploy 测试模块实现内存 fake，并锁定三条核心路径：

```rust
#[test]
fn deployment_replaces_targets_without_mixing_old_files() {
    let mut remote = FakeRemoteFs::with_file("/srv/app/web/old.js", b"old");
    let request = deployment_request_with_frontend("/srv/app/web");
    deploy(&mut remote, &request, &AtomicBool::new(false), |_| {}).unwrap();
    assert!(!remote.exists("/srv/app/web/old.js"));
    assert!(remote.exists("/srv/app/web/index.html"));
    assert!(!remote.any_path_contains("__lazycat_tmp_"));
    assert!(!remote.any_path_contains("__lazycat_backup_"));
}

#[test]
fn second_target_commit_failure_restores_first_target() {
    let mut remote = FakeRemoteFs::with_existing_release();
    remote.fail_rename_to("/srv/app/app.jar");
    let error = deploy(&mut remote, &two_target_request(), &AtomicBool::new(false), |_| {})
        .unwrap_err();
    assert!(error.message.contains("远端提交失败"));
    assert_eq!(remote.read("/srv/app/web/old.js"), b"old");
    assert_eq!(remote.read("/srv/app/app.jar"), b"old-jar");
}

#[test]
fn rollback_failure_reports_recovery_paths_without_deleting_backup() {
    let mut remote = FakeRemoteFs::with_existing_release();
    remote.fail_commit_and_rollback();
    let error = deploy(&mut remote, &two_target_request(), &AtomicBool::new(false), |_| {})
        .unwrap_err();
    assert!(error.message.contains("回滚失败"));
    assert!(error.recovery_paths.iter().any(|path| path.contains("__lazycat_backup_")));
}
```

增加取消测试，断言正式目标未变且安全临时路径被清理。

- [ ] **Step 2: 运行测试并确认失败**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package_deploy -- --nocapture`

Expected: FAIL，事务接口尚不存在。

- [ ] **Step 3: 实现最小 RemoteFs 边界与事务状态机**

只抽象测试必需的文件系统操作：

```rust
pub trait RemoteFs {
    fn metadata(&self, path: &str) -> Result<Option<RemoteMetadata>, DeployError>;
    fn create_dir(&mut self, path: &str) -> Result<(), DeployError>;
    fn read_dir(&self, path: &str) -> Result<Vec<RemoteDirEntry>, DeployError>;
    fn write_file(
        &mut self,
        remote_path: &str,
        local_path: &Path,
        cancelled: &AtomicBool,
        progress: &mut dyn FnMut(u64),
    ) -> Result<(), DeployError>;
    fn rename(&mut self, source: &str, target: &str) -> Result<(), DeployError>;
    fn remove_tree(&mut self, path: &str) -> Result<(), DeployError>;
}
```

`deploy` 先完整上传所有 temp，再通过 `read_dir` 递归统计远端文件数和大小，并校验正式目标状态，然后按 target 顺序 backup → commit。失败时逆序回滚。只有全部提交成功才删除 backup；回滚失败保留唯一副本并返回 `recovery_paths`。`RemoteDirEntry` 明确携带名称、文件类型和大小，遇到符号链接或未知类型直接失败。

- [ ] **Step 4: 实现 SftpRemoteFs 与进度/取消**

在 remote 模块用 `ssh2::Sftp` 实现 `RemoteFs`。上传每次读取 64 KiB，写入前后检查取消标记并累计字节：

```rust
let mut buffer = [0_u8; 64 * 1024];
loop {
    if cancelled.load(Ordering::Acquire) {
        return Err(DeployError::cancelled());
    }
    let size = local.read(&mut buffer).map_err(DeployError::local_io)?;
    if size == 0 { break; }
    remote.write_all(&buffer[..size]).map_err(DeployError::remote_io)?;
    progress(size as u64);
}
```

目录创建逐级执行，已存在目录允许继续，类型冲突显式失败。所有 rename 都在正式目标同级进行；SFTP rename 失败不降级为覆盖写入。

- [ ] **Step 5: 验证并提交**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package_deploy release_package_remote -- --nocapture`

Expected: PASS，包含替换、取消、回滚和回滚失败测试。

```powershell
git add -- apps/desktop/src-tauri/src/tools/release_package_remote.rs apps/desktop/src-tauri/src/tools/release_package_deploy.rs
git commit -m "feat(release-package): 实现 SFTP 安全部署事务"
```

### Task 7: 将上传、重试和取消接入现有运行时

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/release_package.rs`
- Modify: `apps/desktop/src-tauri/src/tools/release_package_runtime.rs`
- Modify: `apps/desktop/src-tauri/src/global_notification.rs`

- [ ] **Step 1: 写终态聚合与令牌授权失败测试**

```rust
#[test]
fn upload_failure_preserves_archive_and_returns_retry_descriptor() {
    let archive = PathBuf::from(r"D:\release\portal");
    let summary = combine_package_and_deploy(
        PackageResult::Succeeded { archive_path: archive.clone(), targets: archived_targets() },
        Err(DeployError::failed("SFTP 传输中断")),
    );
    assert_eq!(summary.status, "package_succeeded_upload_failed");
    assert_eq!(summary.archive_path, Some(archive));
    assert!(summary.retry_descriptor.is_some());
}

#[test]
fn start_rejects_remote_overwrite_not_confirmed_by_preflight() {
    let consumed = consumed_preflight_with_existing(vec![ReleaseTarget::Frontend]);
    assert!(validate_remote_overwrite(&consumed, &[]).is_err());
    assert!(validate_remote_overwrite(&consumed, &[ReleaseTarget::Frontend]).is_ok());
}
```

增加“部分成功不上传”“归档后上传取消保留 archivePath”“迟到取消不覆盖上传成功”的测试。

- [ ] **Step 2: 运行测试并确认失败**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package_runtime -- --nocapture`

Expected: FAIL，新状态和部署聚合不存在。

- [ ] **Step 3: 扩展运行资源、事件和 pipeline**

`ActiveRun` 增加可关闭连接槽：

```rust
ssh_socket: Arc<Mutex<Option<TcpStream>>>,
```

取消时在终止 PowerShell 进程后执行：

```rust
if let Some(socket) = active.ssh_socket.lock().unwrap().take() {
    let _ = socket.shutdown(Shutdown::Both);
}
```

`start` 解析：

```rust
enum StartMode { PackageOnly, PackageAndUpload }
struct DeployAuthorization {
    preflight_token: String,
    overwrite_targets: Vec<ReleaseTarget>,
}
```

上传模式启动前原子消费令牌并精确校验 project/config/targets/覆盖集合；仅打包模式拒绝携带 preflight token，避免歧义。pipeline 保持现有构建和归档逻辑；只有所有选中 target 成功且本地 archive 已 commit 后调用 deploy。

上传事件使用 `phase="upload"`，status 为 `uploading`，并带：

```rust
uploaded_bytes: Option<u64>,
total_bytes: Option<u64>,
current_path: Option<String>,
retry_token: Option<String>,
```

密码和口令不进入事件对象。

- [ ] **Step 4: 实现 upload_retry action 和通知终态**

`upload_retry` 只消费失败任务生成的 session 内 retry token 与新的 preflight token，重新构造归档源；ZIP 前端走安全临时解压。它调用同一 deploy 函数，不进入 `run_target` 或 PowerShell。

扩展通知允许：

```rust
"succeeded" | "partially_succeeded" | "package_succeeded_upload_failed" | "failed" | "cancelled"
```

`package_succeeded_upload_failed` 通知保留 `archive_path`，详情为“本地归档已完成，服务器上传失败”；取消且已有归档时详情为“本地归档已保留，服务器未更新”。

- [ ] **Step 5: 验证并提交**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package -- --nocapture`

Expected: PASS。

```powershell
git add -- apps/desktop/src-tauri/src/tools/release_package.rs apps/desktop/src-tauri/src/tools/release_package_runtime.rs apps/desktop/src-tauri/src/global_notification.rs
git commit -m "feat(release-package): 串联打包与服务器上传"
```

### Task 8: 扩展前端契约、运行态和预检编排

**Files:**

- Modify: `apps/desktop/src/types/release-package.ts`
- Modify: `apps/desktop/src/utils/releasePackage.ts`
- Modify: `apps/desktop/src/utils/releasePackage.test.ts`
- Modify: `apps/desktop/src/composables/useReleasePackageRuntime.ts`
- Modify: `apps/desktop/src/composables/useReleasePackageRuntime.test.ts`
- Create: `apps/desktop/src/composables/useReleasePackageUploadPreflight.ts`
- Create: `apps/desktop/src/composables/useReleasePackageUploadPreflight.test.ts`

- [ ] **Step 1: 写运行态和预检失败测试**

运行态测试：

```ts
runtime.beginStart(7, ["frontend", "backend"]);
runtime.bindStartedRun("run-1", 7);
emit("release-package://log", {
  runId: "run-1",
  projectId: 7,
  phase: "upload",
  stream: "system",
  line: "上传中",
  uploadedBytes: 50,
  totalBytes: 100,
  currentPath: "assets/app.js",
});
emit("release-package://status", {
  runId: "run-1",
  projectId: 7,
  phase: "overall",
  status: "package_succeeded_upload_failed",
  archivePath: "D:\\release\\portal",
  retryToken: "retry-1",
  error: "服务器上传失败",
});
expect(runtime.getProjectRuntime(7).uploadProgress).toEqual({
  uploadedBytes: 50,
  totalBytes: 100,
  currentPath: "assets/app.js",
});
expect(runtime.getProjectRuntime(7).retryToken).toBe("retry-1");
expect(runtime.isRunning.value).toBe(false);
```

预检 composable 测试使用 invoke mock：

```ts
await preflight.probe(7);
expect(invokeMock).toHaveBeenCalledWith("tool:release-package:remote-probe", { projectId: 7 });
await preflight.trustHost(true);
await preflight.check({ projectId: 7, targets: ["frontend"], password: "secret" });
expect(preflight.preflightToken.value).toBe("preflight-1");
preflight.reset();
expect(preflight.preflightToken.value).toBe("");
```

该 composable 的 state 不得包含 `password` 或 `privateKeyPassphrase` 字段。

- [ ] **Step 2: 运行测试并确认失败**

Run: `pnpm --filter @lazycat/desktop exec vitest run src/utils/releasePackage.test.ts src/composables/useReleasePackageRuntime.test.ts src/composables/useReleasePackageUploadPreflight.test.ts`

Expected: FAIL，upload phase、进度和预检 composable 不存在。

- [ ] **Step 3: 扩展 TypeScript 契约和运行态**

增加：

```ts
export type ReleasePackagePhase = ReleasePackageTarget | "upload" | "overall";
export type ReleasePackageStartMode = "package_only" | "package_and_upload";
```

`ReleasePackageRunStatus` 增加 `prechecking`、`uploading`、`package_succeeded_upload_failed`。事件增加可选进度和 retryToken。项目运行态增加：

```ts
uploadLogs: ReleasePackageLogEvent[];
uploadProgress: { uploadedBytes: number; totalBytes: number; currentPath: string };
retryToken: string;
```

`isRunning` 只对正式任务状态 `running | uploading` 返回 true；弹窗预检使用独立 `checking`，不冒充 active run。日志 listener 将 `upload` 分流到 `uploadLogs`，每 lane 仍限制 1,000 行。

- [ ] **Step 4: 实现无秘密状态的预检 composable**

公开接口固定为：

```ts
export function useReleasePackageUploadPreflight() {
  return {
    probeResult,
    preflightResult,
    preflightToken,
    checking,
    probe,
    trustHost,
    check,
    reset,
  };
}
```

`check(input)` 将秘密直接作为调用参数发送，不写入 composable state；请求完成后只保存脱敏结果和 token。任何 probe/trust/check 失败都清空旧 token。`reset` 清空 probe/preflight/token。

- [ ] **Step 5: 验证并提交**

Run: `pnpm --filter @lazycat/desktop exec vitest run src/utils/releasePackage.test.ts src/composables/useReleasePackageRuntime.test.ts src/composables/useReleasePackageUploadPreflight.test.ts`

Expected: PASS。

```powershell
git add -- apps/desktop/src/types/release-package.ts apps/desktop/src/utils/releasePackage.ts apps/desktop/src/utils/releasePackage.test.ts apps/desktop/src/composables/useReleasePackageRuntime.ts apps/desktop/src/composables/useReleasePackageRuntime.test.ts apps/desktop/src/composables/useReleasePackageUploadPreflight.ts apps/desktop/src/composables/useReleasePackageUploadPreflight.test.ts
git commit -m "feat(release-package): 管理上传预检与运行状态"
```

### Task 9: 完成服务器配置、启动预检和上传日志 UI

**Files:**

- Modify: `apps/desktop/src/components/ReleasePackagePanel.vue`
- Modify: `apps/desktop/src/components/ReleasePackagePanel.test.ts`

- [ ] **Step 1: 写面板行为失败测试**

扩展源码契约测试，锁定关键安全门禁：

```ts
it("configures upload separately and preflights before runtime start", () => {
  for (const model of [
    "draft.uploadEnabled",
    "draft.sshHost",
    "draft.sshPort",
    "draft.sshUsername",
    "draft.sshAuthType",
    "draft.sshPrivateKeyPath",
    "draft.frontendRemoteDir",
    "draft.backendRemotePath",
  ])
    expect(source).toContain(`v-model="${model}"`);
  expect(source).toContain("useReleasePackageUploadPreflight");
  expect(source).toContain("tool:release-package:upload-retry");
  expect(source.indexOf("await uploadPreflight.check")).toBeLessThan(
    source.indexOf("runtime.beginStart"),
  );
  expect(source).toContain('type="password"');
  expect(source).toContain('credentialSecret.value = ""');
  expect(source).not.toContain("draft.password");
});

it("renders a separate upload lane and explicit remote replacement confirmation", () => {
  expect(source).toContain("上传日志");
  expect(source).toContain("uploadProgress");
  expect(source).toContain("完整替换以上远程目标");
  expect(source).toContain("package_succeeded_upload_failed");
  expect(source).toContain("重试上传");
});
```

- [ ] **Step 2: 运行测试并确认失败**

Run: `pnpm --filter @lazycat/desktop exec vitest run src/components/ReleasePackagePanel.test.ts`

Expected: FAIL，服务器配置区和上传 lane 尚不存在。

- [ ] **Step 3: 实现独立服务器配置区**

在工程配置卡之后增加折叠区域，保持现有浅色风格。密码永远不在配置区出现；私钥方式显示只读路径和“选择私钥”按钮，复用 `chooseFile`。上传未启用时只保存配置，不触发必填报错；启用并保存时使用 `validateReleasePackageUpload`。

确认框增加 `startMode`：

```ts
const startMode = ref<ReleasePackageStartMode>("package_only");
const credentialSecret = ref("");
const overwriteRemoteTargets = ref<ReleasePackageTarget[]>([]);
```

打开确认框时按项目 `uploadEnabled` 初始化 mode；每次关闭、取消、成功启动或异常后执行：

```ts
credentialSecret.value = "";
uploadPreflight.reset();
overwriteRemoteTargets.value = [];
```

- [ ] **Step 4: 实现探测、信任、预检、启动和重试交互**

上传模式按顺序执行：probe → 未知/变化指纹确认 → trust → 凭据 check → 展示目标状态 → 同名目标确认。发生 `changed` 时确认框同时展示旧、新指纹；不得提供忽略按钮。

调用 start 的 payload：

```ts
{
  projectId,
  folderName: folderName.value,
  targets: [...selectedTargets.value],
  overwriteExisting,
  mode: startMode.value,
  preflightToken: startMode.value === "package_and_upload"
    ? uploadPreflight.preflightToken.value
    : undefined,
  overwriteRemoteTargets: [...overwriteRemoteTargets.value],
}
```

上传 lane 独立展示日志和进度条；只有 `package_succeeded_upload_failed` 且有 retry token 时显示“重试上传”。重试先重新 probe/preflight/覆盖确认，再调用 `tool:release-package:upload-retry`，不能调用 `start`。

- [ ] **Step 5: 验证面板和渲染层并提交**

Run: `pnpm --filter @lazycat/desktop exec vitest run src/components/ReleasePackagePanel.test.ts src/composables/useReleasePackageUploadPreflight.test.ts`

Expected: PASS。

Run: `pnpm --filter @lazycat/desktop typecheck`

Expected: PASS。

```powershell
git add -- apps/desktop/src/components/ReleasePackagePanel.vue apps/desktop/src/components/ReleasePackagePanel.test.ts
git commit -m "feat(release-package): 增加服务器上传工作流"
```

### Task 10: 更新通知、经验并完成真实协议验证

**Files:**

- Modify: `apps/desktop/src/types/global-notification.ts`
- Modify: `apps/desktop/src/utils/globalNotification.ts`
- Modify: `apps/desktop/src/utils/globalNotification.test.ts`
- Modify: `apps/desktop/src/components/GlobalNotificationPopup.vue`
- Modify: `apps/desktop/src-tauri/src/global_notification.rs`
- Modify: `apps/desktop/src-tauri/src/tools/release_package_remote.rs`
- Modify: `docs/experience/release-package.md`

- [ ] **Step 1: 写通知和真实 SSH 集成测试**

通知测试增加：

```ts
expect(releasePackageNotificationCopy("package_succeeded_upload_failed")).toEqual({
  title: "上线包上传失败",
  detail: "本地归档已完成，服务器上传失败",
});
```

在 `release_package_remote.rs` 的 `#[cfg(test)]` 模块中增加显式忽略的真实协议用例，直接访问模块私有测试接口，避免为测试扩大生产 API；只允许 loopback：

```rust
#[test]
#[ignore = "requires LAZYCAT_SSH_TEST_* variables and a loopback SSH fixture"]
fn password_and_private_key_upload_to_local_fixture() {
    let fixture = SshTestFixture::from_env().unwrap();
    assert!(matches!(fixture.host.as_str(), "127.0.0.1" | "localhost" | "::1"));

    for auth in [fixture.password_auth(), fixture.private_key_auth()] {
        let probe = probe_host(&fixture.endpoint()).unwrap();
        let mut client = RemoteClient::connect(&fixture.endpoint(), &probe.fingerprint_sha256, auth).unwrap();
        let request = fixture.deployment_request();
        deploy(&mut client, &request, &AtomicBool::new(false), |_| {}).unwrap();
        fixture.assert_uploaded(&mut client).unwrap();
        fixture.cleanup(&mut client).unwrap();
    }
}
```

同一集成文件再覆盖未知指纹、错误密码、递归目录、空目录、大文件、取消和完整替换。测试目标固定在测试账户临时目录 `/tmp/lazycat-release-package-test-<uuid>`，结束时只清理该 UUID 目录。

- [ ] **Step 2: 运行默认测试并确认通知失败、SSH 测试被忽略**

Run: `pnpm --filter @lazycat/desktop exec vitest run src/utils/globalNotification.test.ts`

Expected: FAIL，新通知状态尚未加入。

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package_remote::tests::password_and_private_key_upload_to_local_fixture -- --nocapture`

Expected: PASS，输出 0 passed、ignored 集成测试，不尝试连接网络。

- [ ] **Step 3: 实现通知契约和经验记录**

前后端通知状态都增加 `package_succeeded_upload_failed` 和 `cancelled`，popup 在存在 `archivePath` 时仍允许打开本地目录，上传失败时主按钮仍为“打开打包页面”。更新 `docs/experience/release-package.md`，增加：

- 构建前真实预检，上传前再次校验。
- 主机指纹先于认证，变化时阻止连接。
- 密码/口令只存在一次性后端令牌中。
- 远端通过 temp/backup/final 完整替换，跨目标只能尽力回滚。
- 本地归档成功与远端上传成功分别表达，不能伪合并。

- [ ] **Step 4: 运行全量相关验证**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package -- --nocapture`

Expected: PASS。

Run: `pnpm --filter @lazycat/desktop exec vitest run src/utils/releasePackage.test.ts src/composables/useReleasePackageRuntime.test.ts src/composables/useReleasePackageUploadPreflight.test.ts src/components/ReleasePackagePanel.test.ts src/utils/globalNotification.test.ts`

Expected: PASS。

Run: `pnpm typecheck`

Expected: PASS。

Run: `pnpm --filter @lazycat/desktop build:web`

Expected: PASS；只允许已有 chunk size 警告。

Run: `git diff --check`

Expected: PASS。

- [ ] **Step 5: 对受控 Linux 测试服务器做真实冒烟**

设置只指向 loopback 或用户明确提供的非生产测试服务器的环境变量，然后运行：

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package_remote::tests::password_and_private_key_upload_to_local_fixture -- --ignored --nocapture
```

Expected: 密码认证、私钥认证、目录上传、文件上传、取消和完整替换全部 PASS。若当前没有受控测试服务器，保留 ignored 测试并在交付中明确记录“真实 SSH/SFTP 冒烟未执行”，不得连接生产服务器补数。

- [ ] **Step 6: 提交收口改动**

```powershell
git add -- apps/desktop/src/types/global-notification.ts apps/desktop/src/utils/globalNotification.ts apps/desktop/src/utils/globalNotification.test.ts apps/desktop/src/components/GlobalNotificationPopup.vue apps/desktop/src-tauri/src/global_notification.rs apps/desktop/src-tauri/src/tools/release_package_remote.rs docs/experience/release-package.md
git commit -m "test(release-package): 验证 Linux 上传终态"
```

## 完成标准

- 仅打包路径的现有行为和测试不回退。
- 上传模式必须先完成可信主机确认、认证、权限和同名目标预检。
- 密码、私钥口令不落库、不进入日志/通知，预检令牌单次消费并过期清理。
- 前端原始目录和后端文件通过 SFTP 上传；本地 ZIP 模式不改变远端目录形态。
- 远端目标只通过临时/备份/正式切换完整替换，失败回滚或明确报告恢复路径。
- 上传取消不破坏原线上版本；本地归档成功后上传失败仍保留归档并可重试。
- 远程命令没有 UI、IPC action 或业务实现，仅 SSH 会话边界可扩展。
- Rust 定向测试、前端定向测试、全工作区 typecheck、渲染层 build 和 `git diff --check` 全部通过。
