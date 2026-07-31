# 密码库与上线包上传整合 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让账户密码方式的上线包项目绑定 Vault 服务器凭据，并由 Rust 在已解锁 Vault 会话中安全完成 SSH 认证。

**Architecture:** 上线包项目只持久化可空的 `vaultEntryId`。Vue 仅读取 Vault 的非敏感服务器元数据来选择和展示绑定项；Rust 的 `release_package` 在主机指纹已受信任后通过 Vault 内部接口解密密码并交给既有一次性预检令牌。私钥认证、远端替换事务、运行时和重试协议保持原状。

**Tech Stack:** Vue 3、TypeScript、Vitest、Tauri 2、Rust、rusqlite、serde_json、zeroize、ssh2。

---

## 文件职责

| 文件                                                                    | 责任                                                              |
| ----------------------------------------------------------------------- | ----------------------------------------------------------------- |
| `apps/desktop/src/types/release-package.ts`                             | 上线包项目、Vault 绑定和无密码预检的前端契约。                    |
| `apps/desktop/src/utils/releasePackage.ts`                              | 默认草稿、项目映射、纯校验与 dirty 比较。                         |
| `apps/desktop/src/utils/releasePackage.test.ts`                         | 纯前端契约和校验回归。                                            |
| `apps/desktop/src/composables/useReleasePackageUploadPreflight.ts`      | 主机探测、信任与不携带密码的预检 IPC。                            |
| `apps/desktop/src/composables/useReleasePackageUploadPreflight.test.ts` | 预检 IPC payload 不泄漏密码。                                     |
| `apps/desktop/src-tauri/src/tools/vault.rs`                             | Vault 会话检查、服务器条目元数据读取和密码解密的 crate 内部接口。 |
| `apps/desktop/src-tauri/src/tools/release_package.rs`                   | 项目表迁移、绑定 ID CRUD、账户密码模式的端点解析和预检认证编排。  |
| `apps/desktop/src-tauri/src/tools/release_package_remote.rs`            | 将绑定凭据 ID 纳入预检令牌等值绑定。                              |
| `apps/desktop/src-tauri/src/tools/release_package_runtime.rs`           | 更新测试构造的 `PreflightBinding` 字段，保持运行时消费预检令牌。  |
| `apps/desktop/src/components/ReleasePackagePanel.vue`                   | 凭据选择、只读摘要、打开 Vault、确认框与私钥分支 UI。             |
| `apps/desktop/src/components/ReleasePackagePanel.test.ts`               | 面板结构、账户密码无输入框和 Vault 导航的回归守卫。               |
| `apps/desktop/src/App.vue`                                              | 监听上线包面板的 `open-tool` 事件，复用现有 tab 导航。            |
| `docs/experience/release-package.md`                                    | 记录“凭据只保存引用、秘密仅在后端预检链路取用”的新增边界。        |

### Task 1: 固化前端 Vault 绑定契约和无密码预检

**Files:**

- Modify: `apps/desktop/src/types/release-package.ts`
- Modify: `apps/desktop/src/utils/releasePackage.ts`
- Modify: `apps/desktop/src/utils/releasePackage.test.ts`
- Modify: `apps/desktop/src/composables/useReleasePackageUploadPreflight.ts`
- Modify: `apps/desktop/src/composables/useReleasePackageUploadPreflight.test.ts`

- [ ] **Step 1: 为密码认证绑定规则写失败的纯函数测试**

在 `releasePackage.test.ts` 的上传校验用例后加入以下断言。测试必须覆盖密码模式不再需要 `sshHost`/`sshUsername`、需要 `vaultEntryId`，以及私钥模式仍需要地址、用户名和私钥路径。

```ts
it("requires a Vault credential only for password upload", () => {
  const draft = createEmptyReleasePackageDraft();
  Object.assign(draft, {
    packageType: "server_upload",
    sshAuthType: "password",
    frontendRemoteDir: "/srv/portal/web",
    backendRemotePath: "/srv/portal/app.jar",
  });

  expect(validateReleasePackageUpload(draft)).toBe("请选择密码库服务器凭据");
  draft.vaultEntryId = 42;
  expect(validateReleasePackageUpload(draft)).toBeNull();

  draft.sshAuthType = "private_key";
  draft.vaultEntryId = null;
  expect(validateReleasePackageUpload(draft)).toBe("请输入服务器地址");
  draft.sshHost = "deploy.example.internal";
  expect(validateReleasePackageUpload(draft)).toBe("请输入 SSH 用户名");
  draft.sshUsername = "deploy";
  expect(validateReleasePackageUpload(draft)).toBe("请选择 SSH 私钥文件");
});

it("maps and compares vaultEntryId as part of the project draft", () => {
  const withBinding = { ...project, vaultEntryId: 9 };
  const draft = projectToReleasePackageDraft(withBinding);
  expect(draft.vaultEntryId).toBe(9);
  expect(isReleasePackageDraftDirty(withBinding, draft)).toBe(false);
  draft.vaultEntryId = 10;
  expect(isReleasePackageDraftDirty(withBinding, draft)).toBe(true);
});
```

- [ ] **Step 2: 运行纯函数测试并确认失败原因是缺少绑定字段和分支校验**

Run:

```powershell
pnpm --filter @lazycat/desktop test -- src/utils/releasePackage.test.ts
```

Expected: FAIL，`vaultEntryId` 尚不存在，且密码模式仍要求 `sshHost`。

- [ ] **Step 3: 扩展 TypeScript 类型、默认草稿和校验函数**

在 `release-package.ts` 的 `ReleasePackageUploadConfig` 中加入 `vaultEntryId: number | null`，使 `ReleasePackageProjectDraft`、`ReleasePackageProject` 和 CRUD payload 自然携带该字段。移除 `ReleasePackageRemotePreflightInput.password`，保留 `privateKeyPassphrase?: string`。

在 `releasePackage.ts` 中完成以下实现：

```ts
export function createEmptyReleasePackageDraft(): ReleasePackageProjectDraft {
  return {
    // 保留现有项目、构建和远程路径默认值
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

export function validateReleasePackageUpload(draft: ReleasePackageProjectDraft): string | null {
  const value = normalizeReleasePackageDraft(draft);
  if (!Number.isInteger(value.sshPort) || value.sshPort < 1 || value.sshPort > 65_535) {
    return "SSH 端口必须在 1 到 65535 之间";
  }
  if (value.sshAuthType === "password" && value.vaultEntryId === null) {
    return "请选择密码库服务器凭据";
  }
  if (value.sshAuthType === "private_key") {
    if (!value.sshHost) return "请输入服务器地址";
    if (!value.sshUsername) return "请输入 SSH 用户名";
    if (!value.sshPrivateKeyPath) return "请选择 SSH 私钥文件";
  }
  // 沿用既有 frontendRemoteDir/backendRemotePath 的完整校验。
  return null;
}
```

在 `projectToReleasePackageDraft()` 复制 `project.vaultEntryId`。更新原有空草稿和上传校验测试期望，确保私钥分支显式设置 `sshHost` 与 `sshUsername`，不要让旧测试掩盖认证分支。

- [ ] **Step 4: 让预检 composable 仅发送私钥口令**

把 `PreflightInput` 改成显式类型，不再从 `ReleasePackageRemotePreflightInput` 间接继承已删除的密码字段：

```ts
interface PreflightInput {
  projectId: number;
  targets: ReleasePackageTarget[];
  privateKeyPassphrase?: string;
}
```

更新 composable 测试中的第三次调用：

```ts
await preflight.check({
  projectId: 7,
  targets: ["frontend"],
  privateKeyPassphrase: "key-passphrase",
});

expect(invokeMock).toHaveBeenNthCalledWith(3, "tool:release-package:remote-preflight", {
  projectId: 7,
  targets: ["frontend"],
  probeToken: "probe-2",
  privateKeyPassphrase: "key-passphrase",
});
expect(JSON.stringify(invokeMock.mock.calls)).not.toContain('"password"');
```

保留 `reset()` 对 probe/preflight token 的清理，不在 composable 中新增凭据秘密状态。

- [ ] **Step 5: 运行前端定向测试并确认通过**

Run:

```powershell
pnpm --filter @lazycat/desktop test -- src/utils/releasePackage.test.ts src/composables/useReleasePackageUploadPreflight.test.ts
```

Expected: PASS，密码模式只需要 `vaultEntryId`，预检 IPC payload 不含 `password`。

- [ ] **Step 6: 提交前端契约批次**

```powershell
git add apps/desktop/src/types/release-package.ts apps/desktop/src/utils/releasePackage.ts apps/desktop/src/utils/releasePackage.test.ts apps/desktop/src/composables/useReleasePackageUploadPreflight.ts apps/desktop/src/composables/useReleasePackageUploadPreflight.test.ts
git commit -m "feat(release-package): 增加密码库凭据绑定契约"
```

### Task 2: 增加 Vault crate 内部服务器凭据解析接口

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/vault.rs`

- [ ] **Step 1: 为服务器元数据和密码解析写失败测试**

在 `vault.rs` 的测试模块中加入内存数据库辅助函数，创建 `vault_entries` 所需列并写入一条按现有 `split_fields()` 规则加密的 server 条目。测试名称和关键断言如下：

```rust
fn vault_test_conn() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE vault_entries (
            id INTEGER PRIMARY KEY, category TEXT NOT NULL,
            title TEXT NOT NULL DEFAULT '', environment TEXT NOT NULL DEFAULT '',
            iv TEXT NOT NULL, encrypted_blob TEXT NOT NULL, plain_fields TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );",
    ).unwrap();
    conn
}

fn insert_vault_entry(conn: &Connection, id: i64, category: &str, plain: &str, password: &str) {
    let key = [7u8; KEY_LEN];
    let iv = vec![9u8; IV_LEN];
    let secret = serde_json::to_vec(&json!({ "password": password })).unwrap();
    let encrypted = aes256_encrypt(&key, &iv, &secret).unwrap();
    conn.execute(
        "INSERT INTO vault_entries(id, category, iv, encrypted_blob, plain_fields)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, category, BASE64.encode(iv), BASE64.encode(encrypted), plain],
    ).unwrap();
}

#[test]
fn server_metadata_rejects_missing_wrong_type_and_incomplete_entry() {
    let conn = vault_test_conn();
    insert_vault_entry(&conn, 1, "server", r#"{"address":"10.0.0.8","account":"deploy"}"#, "secret");
    assert_eq!(server_credential_metadata(&conn, 999).unwrap_err(), "vault_entry_not_found");

    insert_vault_entry(&conn, 2, "app", r#"{"address":"10.0.0.9","account":"deploy"}"#, "secret");
    assert_eq!(server_credential_metadata(&conn, 2).unwrap_err(), "vault_entry_invalid_category");

    insert_vault_entry(&conn, 3, "server", r#"{"address":"","account":"deploy"}"#, "secret");
    assert_eq!(server_credential_metadata(&conn, 3).unwrap_err(), "vault_entry_incomplete");
}

#[test]
fn resolved_server_credential_requires_session_and_keeps_password_out_of_metadata() {
    let conn = vault_test_conn();
    insert_vault_entry(&conn, 1, "server", r#"{"address":"10.0.0.8","account":"deploy"}"#, "secret");

    force_lock();
    assert!(resolve_server_credential(&conn, 1).unwrap_err().contains("vault_locked"));

    install_test_session([7u8; KEY_LEN]);
    let credential = resolve_server_credential(&conn, 1).unwrap();
    assert_eq!(credential.metadata.address.as_str(), "10.0.0.8");
    assert_eq!(credential.metadata.account.as_str(), "deploy");
    assert_eq!(&*credential.password, "secret");
    assert!(!format!("{:?}", credential.metadata).contains("secret"));
    force_lock();
}
```

`install_test_session` 只在 `#[cfg(test)]` 下设置 `VAULT_SESSION`，并在每个测试末尾调用 `force_lock()`，避免全局会话泄漏到其他测试。

- [ ] **Step 2: 运行 Vault 测试并确认新符号尚不存在**

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml vault::tests::server_metadata_rejects_missing_wrong_type_and_incomplete_entry -- --nocapture
```

Expected: FAIL，`server_credential_metadata` 与 `resolve_server_credential` 尚未定义。

- [ ] **Step 3: 实现不跨 IPC 的服务器凭据 API**

在 `vault.rs` 中、`cmd_get` 之前定义 crate 内部结构和函数。元数据可派生 `Debug`，含密码结构不要派生 `Debug`。

```rust
#[derive(Debug)]
pub(crate) struct VaultServerCredentialMetadata {
    pub entry_id: i64,
    pub address: String,
    pub account: String,
}

pub(crate) struct VaultServerCredential {
    pub metadata: VaultServerCredentialMetadata,
    pub password: Zeroizing<String>,
}

pub(crate) fn require_unlocked() -> Result<(), String> {
    let mut guard = VAULT_SESSION
        .lock()
        .map_err(|error| format!("session lock: {error}"))?;
    ensure_session_alive(&mut guard)
}

#[cfg(test)]
pub(crate) fn install_test_session(key: [u8; KEY_LEN]) {
    let mut guard = VAULT_SESSION.lock().unwrap();
    *guard = Some(VaultSession {
        key: Some(key),
        last_activity: Instant::now(),
        hard_lock_after_secs: 1_800,
    });
}

#[cfg(test)]
pub(crate) fn insert_test_server_entry(
    conn: &Connection,
    entry_id: i64,
    address: &str,
    account: &str,
    password: &str,
) {
    let key = [7u8; KEY_LEN];
    let iv = vec![9u8; IV_LEN];
    let secret = serde_json::to_vec(&json!({ "password": password })).unwrap();
    let encrypted = aes256_encrypt(&key, &iv, &secret).unwrap();
    conn.execute(
        "INSERT INTO vault_entries(id, category, iv, encrypted_blob, plain_fields)
         VALUES (?1, 'server', ?2, ?3, ?4)",
        params![
            entry_id,
            BASE64.encode(iv),
            BASE64.encode(encrypted),
            json!({ "address": address, "account": account }).to_string(),
        ],
    ).unwrap();
}

pub(crate) fn server_credential_metadata(
    conn: &Connection,
    entry_id: i64,
) -> Result<VaultServerCredentialMetadata, String> {
    let (category, plain_fields): (String, Option<String>) = conn.query_row(
        "SELECT category, plain_fields FROM vault_entries WHERE id = ?1",
        [entry_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).map_err(|_| "vault_entry_not_found".to_string())?;
    if category != "server" { return Err("vault_entry_invalid_category".into()); }
    let fields: Value = plain_fields
        .as_deref()
        .and_then(|text| serde_json::from_str(text).ok())
        .unwrap_or(Value::Null);
    let address = fields["address"].as_str().unwrap_or("").trim().to_owned();
    let account = fields["account"].as_str().unwrap_or("").trim().to_owned();
    if address.is_empty() || account.is_empty() { return Err("vault_entry_incomplete".into()); }
    Ok(VaultServerCredentialMetadata { entry_id, address, account })
}

pub(crate) fn resolve_server_credential(
    conn: &Connection,
    entry_id: i64,
) -> Result<VaultServerCredential, String> {
    let metadata = server_credential_metadata(conn, entry_id)?;
    let mut key = get_session_key()?;
    let (iv_b64, blob_b64): (String, String) = conn.query_row(
        "SELECT iv, encrypted_blob FROM vault_entries WHERE id = ?1",
        [entry_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).map_err(|_| "vault_entry_not_found".to_string())?;
    let result = (|| -> Result<Zeroizing<String>, String> {
        let iv = BASE64.decode(iv_b64).map_err(|error| format!("vault credential iv: {error}"))?;
        let blob = BASE64.decode(blob_b64).map_err(|error| format!("vault credential blob: {error}"))?;
        let fields: Value = serde_json::from_slice(&aes256_decrypt(&key, &iv, &blob)?)
            .map_err(|error| format!("vault credential fields: {error}"))?;
        let password = fields["password"].as_str().unwrap_or("");
        if password.is_empty() { return Err("vault_entry_incomplete".into()); }
        Ok(Zeroizing::new(password.to_owned()))
    })();
    key.zeroize();
    Ok(VaultServerCredential { metadata, password: result? })
}
```

不要把这两个函数注册到 `ACTIONS`，不要新增 `tool:vault:*` channel；它们只能被同 crate 的上线包模块调用。保留现有 `cmd_meta_list`，它继续只返回明文元数据。

- [ ] **Step 4: 运行 Vault 模块测试并确认通过**

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml vault::tests -- --nocapture
```

Expected: PASS，错误码为 `vault_entry_not_found`、`vault_entry_invalid_category`、`vault_entry_incomplete` 或既有 `vault_locked*`，测试输出中不出现 `secret`。

- [ ] **Step 5: 提交 Vault 内部接口批次**

```powershell
git add apps/desktop/src-tauri/src/tools/vault.rs
git commit -m "feat(vault): 提供服务器凭据内部解析"
```

### Task 3: 持久化上线包的 Vault 绑定并保持私钥兼容

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/release_package.rs`

- [ ] **Step 1: 为 schema 迁移与项目 CRUD 写失败测试**

在 `release_package.rs` 的测试模块中加入：

```rust
#[test]
fn schema_migrates_vault_entry_id_and_project_round_trips_it() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE release_package_projects (
            id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL,
            output_root TEXT NOT NULL, frontend_project_path TEXT NOT NULL,
            frontend_build_command TEXT NOT NULL, frontend_artifact_path TEXT NOT NULL,
            frontend_artifact_mode TEXT NOT NULL, backend_project_path TEXT NOT NULL,
            backend_build_command TEXT NOT NULL, backend_artifact_path TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );"
    ).unwrap();
    ensure_schema(&conn).unwrap();

    let columns = conn.prepare("PRAGMA table_info(release_package_projects)").unwrap()
        .query_map([], |row| row.get::<_, String>(1)).unwrap()
        .collect::<Result<Vec<_>, _>>().unwrap();
    assert!(columns.contains(&"vault_entry_id".to_string()));
}

#[test]
fn password_project_requires_vault_entry_but_private_key_keeps_host_and_username() {
    let mut password = payload();
    password["sshAuthType"] = json!("password");
    password["vaultEntryId"] = Value::Null;
    password["sshHost"] = json!("");
    password["sshUsername"] = json!("");
    assert_eq!(
        parse_project_payload(&password).unwrap_err(),
        "vaultEntryId is required for password authentication"
    );

    let private_key = parse_project_payload(&payload()).unwrap();
    assert_eq!(private_key.vault_entry_id, None);
}
```

另加以下 CRUD 用例，继续使用现有 `test_conn()`，不要连接用户真实数据库：

```rust
#[test]
fn password_project_round_trips_only_the_vault_entry_id() {
    let conn = test_conn();
    conn.execute_batch(
        "CREATE TABLE vault_entries (
            id INTEGER PRIMARY KEY, category TEXT NOT NULL, plain_fields TEXT
        );
        INSERT INTO vault_entries(id, category, plain_fields)
        VALUES (17, 'server', '{\"address\":\"10.0.0.8\",\"account\":\"deploy\"}');"
    ).unwrap();
    let mut input = payload();
    input["sshAuthType"] = json!("password");
    input["vaultEntryId"] = json!(17);
    input["sshHost"] = json!("");
    input["sshUsername"] = json!("");

    let id = project_create_with_conn(&conn, &input).unwrap()["id"].as_i64().unwrap();
    let saved = load_project(&conn, id).unwrap();
    assert_eq!(saved.vault_entry_id, Some(17));
    let listed = project_list_with_conn(&conn).unwrap();
    assert_eq!(listed["projects"][0]["vaultEntryId"], 17);
    assert!(!serde_json::to_string(&listed).unwrap().contains("password"));
}
```

- [ ] **Step 2: 运行 Rust 定向测试并确认新字段缺失**

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package::tests::schema_migrates_vault_entry_id_and_project_round_trips_it -- --nocapture
```

Expected: FAIL，`vault_entry_id` 尚未迁移且项目类型没有该字段。

- [ ] **Step 3: 增加 schema、结构与 payload 解析**

在 `RELEASE_PACKAGE_SCHEMA_SQL` 的 SSH 字段附近添加：

```sql
vault_entry_id INTEGER NULL,
```

并在 `ensure_schema()` 幂等迁移列表加入：

```rust
("vault_entry_id", "ALTER TABLE release_package_projects ADD COLUMN vault_entry_id INTEGER NULL"),
```

按下列字段顺序同步更新 `ProjectPayload`、`ReleasePackageProjectConfig`、`project_from_row`、三个 SELECT、INSERT、UPDATE 和 `params![]`：

```rust
pub vault_entry_id: Option<i64>,

fn optional_i64(payload: &Value, key: &str) -> Result<Option<i64>, String> {
    match payload.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value.as_i64()
            .filter(|id| *id > 0)
            .map(Some)
            .ok_or_else(|| format!("{key} must be a positive integer")),
    }
}
```

密码认证解析规则：`vaultEntryId` 必填，`sshHost` 和 `sshUsername` 可为空；私钥认证要求现有 `sshHost`、`sshUsername` 和 `sshPrivateKeyPath`，可原样保留 `vaultEntryId`，但运行时完全忽略它。这样切到私钥不会擦除密码模式的既有绑定，切回密码时仍能复用。在 create/update 的密码分支调用 `vault::server_credential_metadata(conn, vault_entry_id)`，使已删除、非 server 或地址/账号不完整的绑定不能保存，但不要求 Vault 已解锁。

- [ ] **Step 4: 保持运行前项目校验的认证分支清晰**

将 `validate_upload_project` 拆成“通用远程路径/端口校验”和“认证来源校验”。私钥分支继续检查项目地址、用户名和私钥路径；密码分支只检查 `vault_entry_id.is_some()`。后续 Task 4 将端点解析集中到同一个 helper，不能在多个地方重复读取 Vault。

- [ ] **Step 5: 运行上线包 Rust 测试并确认通过**

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package::tests -- --nocapture
```

Expected: PASS，旧项目迁移后 `vault_entry_id` 为 `None`，密码项目只保存 ID，私钥项目仍保存原 SSH 字段。

- [ ] **Step 6: 提交项目持久化批次**

```powershell
git add apps/desktop/src-tauri/src/tools/release_package.rs
git commit -m "feat(release-package): 持久化密码库凭据绑定"
```

### Task 4: 在 SSH 探测和预检中由 Rust 解析绑定凭据

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/release_package.rs`
- Modify: `apps/desktop/src-tauri/src/tools/release_package_remote.rs`
- Modify: `apps/desktop/src-tauri/src/tools/release_package_runtime.rs`

- [ ] **Step 1: 为端点解析、密码 payload 拒绝和令牌绑定写失败测试**

在 `release_package.rs` 测试模块中添加用例，直接断言密码模式不接受前端秘密：

```rust
#[test]
fn password_preflight_uses_bound_vault_credential_and_rejects_password_payload() {
    let conn = test_conn();
    conn.execute_batch(
        "CREATE TABLE vault_entries (
            id INTEGER PRIMARY KEY, category TEXT NOT NULL, plain_fields TEXT,
            iv TEXT NOT NULL, encrypted_blob TEXT NOT NULL
        );"
    ).unwrap();
    super::super::vault::insert_test_server_entry(
        &conn, 11, "deploy.example", "deploy", "secret"
    );
    let mut input = payload();
    input["sshAuthType"] = json!("password");
    input["vaultEntryId"] = json!(11);
    input["sshHost"] = json!("");
    input["sshUsername"] = json!("");
    let project_id = project_create_with_conn(&conn, &input).unwrap()["id"].as_i64().unwrap();
    super::super::vault::install_test_session([7u8; 32]);

    let endpoint = upload_endpoint_with_conn(&conn, &load_project(&conn, project_id).unwrap()).unwrap();
    assert_eq!(endpoint.endpoint.host, "deploy.example");
    assert_eq!(endpoint.endpoint.username, "deploy");

    let error = parse_private_key_auth_secret(&json!({ "password": "injected" })).unwrap_err();
    assert_eq!(error, "私钥认证不能提交密码");
    super::super::vault::force_lock();
}
```

测试写入使用 Task 2 已定义的 `vault::insert_test_server_entry`，不复制 Vault 加密实现，也不调用真实数据库。

在 `release_package_remote.rs` 测试中让两个仅凭证不同的 binding 不相等：

```rust
let mut first = binding(vec![RemoteTarget::Frontend]);
first.vault_entry_id = Some(1);
let mut second = first.clone();
second.vault_entry_id = Some(2);
assert_ne!(first, second);
```

- [ ] **Step 2: 运行定向测试并确认端点 helper 和 binding 字段不存在**

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package::tests::password_preflight_uses_bound_vault_credential_and_rejects_password_payload -- --nocapture
```

Expected: FAIL，`upload_endpoint_with_conn` 不存在，`PreflightBinding` 还没有 `vault_entry_id`。

- [ ] **Step 3: 统一账户密码和私钥的运行时端点解析**

在 `release_package.rs` 定义一个只在本模块使用的端点解析结构：

```rust
struct UploadEndpoint {
    endpoint: RemoteEndpoint,
    vault_entry_id: Option<i64>,
}

fn upload_endpoint_with_conn(
    conn: &Connection,
    project: &ReleasePackageProjectConfig,
) -> Result<UploadEndpoint, String> {
    if project.ssh_auth_type == "password" {
        let entry_id = project.vault_entry_id.ok_or("vault_entry_id_missing")?;
        let metadata = super::vault::server_credential_metadata(conn, entry_id)?;
        super::vault::require_unlocked()?;
        return Ok(UploadEndpoint {
            endpoint: RemoteEndpoint {
                host: metadata.address.to_ascii_lowercase(),
                port: project.ssh_port,
                username: metadata.account,
            },
            vault_entry_id: Some(entry_id),
        });
    }
    Ok(UploadEndpoint {
        endpoint: RemoteEndpoint {
            host: project.ssh_host.trim().to_ascii_lowercase(),
            port: project.ssh_port,
            username: project.ssh_username.clone(),
        },
        vault_entry_id: None,
    })
}
```

`remote_probe_with_conn` 使用该 helper 生成探测 endpoint。`preflight_binding` 接受 `UploadEndpoint`，并把 `vault_entry_id` 写入绑定。`remote_preflight_with_conn` 在 probe token、known-host 比对通过后执行以下认证分支：

```rust
let secret = if project.ssh_auth_type == "password" {
    if payload.get("password").is_some() || payload.get("privateKeyPassphrase").is_some() {
        return Err("密码库认证不接受前端认证秘密".into());
    }
    let credential = super::vault::resolve_server_credential(
        conn,
        project.vault_entry_id.ok_or("vault_entry_id_missing")?,
    )?;
    AuthSecret::Password(credential.password)
} else {
    parse_private_key_auth_secret(payload)?
};
```

确保 `resolve_server_credential` 在 host probe 信任前不调用：探测阶段只读取元数据并检查 Vault 会话，密码只在上述预检位置解密。

- [ ] **Step 4: 将 Vault ID 绑定进预检令牌等值比较**

在 `release_package_remote.rs` 中扩展：

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreflightBinding {
    pub project_id: i64,
    pub endpoint: RemoteEndpoint,
    pub auth_type: String,
    pub vault_entry_id: Option<i64>,
    pub private_key_path: String,
    pub targets: Vec<RemoteTarget>,
    pub frontend_remote_dir: String,
    pub backend_remote_path: String,
}
```

所有测试 fixture、`release_package_runtime.rs` 中的 `PreflightBinding { ... }` 和 `release_package_remote.rs` 的 fixture 都要显式传入 `vault_entry_id: None`，绑定 Vault 的测试传入 `Some(id)`。这样预检令牌不能因同项目、同主机、同路径但不同 Vault 条目而被复用。

- [ ] **Step 5: 运行 Rust 上传域测试并确认通过**

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package release_package_remote release_package_runtime -- --nocapture
```

Expected: PASS，密码认证的 preflight payload 不接受 `password`，锁定 Vault 在 probe 前失败，预检 token 比较包含 `vault_entry_id`，私钥认证测试保持通过。

- [ ] **Step 6: 提交后端预检接线批次**

```powershell
git add apps/desktop/src-tauri/src/tools/release_package.rs apps/desktop/src-tauri/src/tools/release_package_remote.rs apps/desktop/src-tauri/src/tools/release_package_runtime.rs
git commit -m "feat(release-package): 从密码库预检服务器凭据"
```

### Task 5: 在上线包面板绑定凭据并移除密码输入

**Files:**

- Modify: `apps/desktop/src/components/ReleasePackagePanel.vue`
- Modify: `apps/desktop/src/components/ReleasePackagePanel.test.ts`
- Modify: `apps/desktop/src/App.vue`

- [ ] **Step 1: 为面板结构和安全分支写失败测试**

在 `ReleasePackagePanel.test.ts` 增加：

```ts
it("binds a Vault server credential for password auth without rendering a password field", () => {
  expect(source).toContain('label="密码库凭据"');
  expect(source).toContain('v-model="draft.vaultEntryId"');
  expect(source).toContain("tool:vault:meta-list");
  expect(source).toContain("v-if=\"draft.sshAuthType === 'password'\"");
  expect(source).toContain("密码由密码库提供");
  expect(source).not.toContain("请输入服务器密码");
  expect(source).not.toContain("? { password: credentialSecret.value }");
});

it("keeps only the private-key passphrase input in the start dialog", () => {
  expect(source).toContain("draft.sshAuthType === 'private_key'");
  expect(source).toContain("privateKeyPassphrase: credentialSecret.value || undefined");
});

it("opens the Vault through the application tool navigation event", () => {
  expect(source).toContain('emit("open-tool", "vault")');
});

it("renders an explicit state when the saved Vault binding no longer exists", () => {
  expect(source).toContain('class="vault-binding-invalid"');
  expect(source).toContain("绑定的密码库凭据已失效，请重新选择");
});
```

在 `App.vue` 的动态组件上添加面板事件断言所需的实现：`@open-tool="onSelect"`。

- [ ] **Step 2: 运行面板测试并确认失败**

Run:

```powershell
pnpm --filter @lazycat/desktop test -- src/components/ReleasePackagePanel.test.ts
```

Expected: FAIL，当前模板没有 `vaultEntryId` 控件，确认框仍显示服务器密码输入。

- [ ] **Step 3: 加载并归一化非敏感 Vault 服务器元数据**

在 `ReleasePackagePanel.vue` 中定义局部只读类型和状态：

```ts
interface VaultServerOption {
  id: number;
  title: string;
  environment: string;
  address: string;
  account: string;
  complete: boolean;
}

const vaultServerOptions = ref<VaultServerOption[]>([]);
const vaultOptionsLoading = ref(false);
const emit = defineEmits<{ (event: "open-tool", toolId: string): void }>();

async function loadVaultServerOptions(): Promise<void> {
  vaultOptionsLoading.value = true;
  try {
    const result = (await invokeToolByChannel("tool:vault:meta-list", {
      category: "server",
    })) as unknown[];
    vaultServerOptions.value = result
      .map((entry) => {
        const value = entry as {
          id?: number;
          title?: string;
          environment?: string;
          plainFields?: Record<string, unknown> | null;
        };
        const address =
          typeof value.plainFields?.address === "string" ? value.plainFields.address.trim() : "";
        const account =
          typeof value.plainFields?.account === "string" ? value.plainFields.account.trim() : "";
        return {
          id: value.id ?? 0,
          title: value.title ?? "",
          environment: value.environment ?? "",
          address,
          account,
          complete: Boolean(address && account),
        };
      })
      .filter((entry) => entry.id > 0);
  } finally {
    vaultOptionsLoading.value = false;
  }
}

function openVault(): void {
  emit("open-tool", "vault");
}
```

调用 `loadVaultServerOptions()` 于 `onMounted`，并在“刷新凭据”按钮点击时调用；加载失败调用既有 `showError`，不能把失败伪装为空列表。

- [ ] **Step 4: 替换账户密码模式的表单和确认框**

在服务器配置 grid 中保留端口、远程目录和后端远程文件。认证方式为 `password` 时渲染：

```vue
<el-form-item label="密码库凭据" required class="server-config-span-2">
  <div class="vault-credential-field">
    <el-select v-model="draft.vaultEntryId" :loading="vaultOptionsLoading" filterable placeholder="选择服务器凭据" :disabled="running">
      <el-option v-for="entry in vaultServerOptions" :key="entry.id" :value="entry.id" :disabled="!entry.complete">
        <span>{{ entry.title || "未命名服务器" }}</span>
        <small>{{ entry.environment }} · {{ entry.address || "缺少地址" }} · {{ entry.account || "缺少账号" }}</small>
      </el-option>
    </el-select>
    <el-button :icon="Refresh" :loading="vaultOptionsLoading" :disabled="running" aria-label="刷新密码库凭据" @click="loadVaultServerOptions" />
    <el-button text @click="openVault">密码管理</el-button>
  </div>
</el-form-item>
<el-form-item
  label="服务器地址"
><el-input :model-value="selectedVaultCredential?.address || ''" readonly /></el-form-item>
<el-form-item
  label="SSH 用户名"
><el-input :model-value="selectedVaultCredential?.account || ''" readonly /></el-form-item>
```

`selectedVaultCredential` 是由 `draft.vaultEntryId` 和 `vaultServerOptions` 计算的值。私钥分支保留现有可编辑地址、用户名、私钥路径 UI，并在切换到 `private_key` 时不清空保留字段。

当 `draft.vaultEntryId !== null` 但 `selectedVaultCredential` 为空时，在选择器下渲染：

```vue
<p class="vault-binding-invalid" role="alert">
  绑定的密码库凭据已失效，请重新选择
</p>
```

该状态不能把保存的 ID 静默改成 `null`；用户必须显式选择新条目，后端仍会在保存和预检时再次校验。

确认框中仅对私钥显示 `credentialSecret`：

```vue
<el-form-item v-if="isUploadStart && draft.sshAuthType === 'private_key'" label="私钥口令（可选）">
  <el-input v-model="credentialSecret" type="password" show-password autocomplete="new-password" :disabled="starting" />
</el-form-item>
<p v-else-if="isUploadStart" class="vault-credential-summary">
  {{ selectedVaultCredential ? `使用密码库凭据：${selectedVaultCredential.title}` : "未绑定密码库凭据" }}
</p>
```

将 `runUploadPreflight` 改为：

```ts
if (draft.sshAuthType === "password" && draft.vaultEntryId === null) {
  throw new Error("请选择密码库服务器凭据");
}
if (!(await ensureHostTrusted(projectId))) return false;
await uploadPreflight.check({
  projectId,
  targets: [...targets],
  ...(draft.sshAuthType === "private_key"
    ? { privateKeyPassphrase: credentialSecret.value || undefined }
    : {}),
});
return confirmRemoteOverwrite();
```

当后端返回 `vault_locked` 时，显示带“打开密码管理”确认按钮的 `ElMessageBox.confirm`，确认后调用 `openVault()`；`vault_entry_not_found`、`vault_entry_invalid_category`、`vault_entry_incomplete` 显示错误并将焦点留在凭据选择器。其他错误继续走 `showError`。

- [ ] **Step 5: 在 App 接收面板导航事件，并补齐轻量样式**

在 `App.vue` 动态组件上加入：

```vue
<component
  v-else-if="currentComponent"
  :is="currentComponent"
  :key="activeTool"
  v-bind="currentComponentProps"
  @open-tool="onSelect"
/>
```

在 `ReleasePackagePanel.vue` 增加最小样式，保证凭据选择器、刷新图标按钮、密码管理文本按钮和只读摘要在窄屏时折行且不溢出。不要改动用户当前未提交的标题编辑、日志列和现有布局以外的样式。

- [ ] **Step 6: 运行面板和类型检查**

Run:

```powershell
pnpm --filter @lazycat/desktop test -- src/components/ReleasePackagePanel.test.ts src/utils/releasePackage.test.ts src/composables/useReleasePackageUploadPreflight.test.ts
pnpm --filter @lazycat/desktop typecheck
```

Expected: PASS，账户密码 UI 没有密码输入和 password payload，私钥分支仍通过类型检查。

- [ ] **Step 7: 提交面板交互批次**

```powershell
git add apps/desktop/src/components/ReleasePackagePanel.vue apps/desktop/src/components/ReleasePackagePanel.test.ts apps/desktop/src/App.vue
git commit -m "feat(release-package): 在上传配置绑定密码库凭据"
```

### Task 6: 联合回归、经验沉淀与交付检查

**Files:**

- Modify: `docs/experience/release-package.md`
- Modify: `docs/experience/README.md` only if its usage index requires a new independent record

- [ ] **Step 1: 更新上线包经验中的凭据安全边界**

在 `release-package.md` 的“认证秘密只存在于一次性链路”之后增加：

```md
## 密码库绑定只保存引用

账户密码认证的上线包项目只保存 Vault 服务器条目 ID。地址和账号在运行时从条目的非敏感元数据读取，密码仅在已解锁 Vault、主机指纹受信任后由 Rust 解密并交给一次性预检令牌。前端不得接收或提交服务器密码；Vault 锁定、条目缺失、类型变化或字段不完整必须显式阻止预检，不回退到手填密码。
```

将该经验的“使用次数”加一。只有当 `docs/experience/README.md` 的维护规则要求新增独立索引项时才添加对应记录；否则不改该索引。

- [ ] **Step 2: 运行 Rust 联合回归**

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml vault release_package release_package_remote release_package_runtime -- --nocapture
```

Expected: PASS，所有 Vault、上线包、预检和运行态测试通过。

- [ ] **Step 3: 运行前端联合回归和渲染层构建**

Run:

```powershell
pnpm --filter @lazycat/desktop test -- src/utils/releasePackage.test.ts src/composables/useReleasePackageUploadPreflight.test.ts src/components/ReleasePackagePanel.test.ts
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
```

Expected: PASS，类型检查和 Vite 构建均无错误。

- [ ] **Step 4: 执行最小人工冒烟**

在已有本地应用中验证以下场景，不启动新的 `pnpm dev`：

1. 创建或编辑 Vault server 条目，绑定到账户密码上线包项目并保存。
2. 锁定 Vault 后点击开始打包，确认在任何主机认证前被阻止并可打开密码管理。
3. 解锁 Vault 后开始上传，确认主机指纹、远程预检和覆盖确认仍按原流程出现，确认框不显示服务器密码输入。
4. 切换到私钥认证，确认地址、用户名、私钥路径和可选口令仍可编辑和预检。
5. 编辑绑定条目的密码后重新预检，确认配置项目不变且运行使用新凭据；删除该条目后确认项目提示重新绑定。

- [ ] **Step 5: 检查差异与提交最终文档**

Run:

```powershell
git diff --check
git status --short
```

Expected: `git diff --check` 无输出；只暂存本任务实际修改的文件，保留既有 dirty worktree 文件的用户改动。

```powershell
git add docs/experience/release-package.md docs/experience/README.md
git commit -m "docs: 记录上线包密码库绑定边界"
```

若 `docs/experience/README.md` 未改动，不把它加入 `git add`。
