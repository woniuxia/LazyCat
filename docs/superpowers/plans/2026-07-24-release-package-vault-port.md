# 上线包读取密码库服务器端口 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 账户密码认证从密码库读取 SSH 端口并移除重复输入，私钥认证继续使用上线包项目端口。

**Architecture:** Rust 的 `VaultServerCredentialMetadata` 成为账户密码模式下地址、端口和账号的唯一来源，`upload_endpoint_with_conn` 按认证方式解析最终 SSH 端点。前端用一个纯函数统一处理密码库端口的默认值和有效性，项目 `sshPort` 仅在私钥模式展示与校验，原数据库字段继续保留以兼容私钥配置。

**Tech Stack:** Rust、Rusqlite、Serde JSON、Vue 3、TypeScript、Element Plus、Vitest、Cargo Test

---

## 文件结构

- Modify: `apps/desktop/src-tauri/src/tools/vault.rs` — 解析和验证服务器凭据端口，并通过凭据元数据暴露端口。
- Modify: `apps/desktop/src-tauri/src/tools/release_package.rs` — 按认证方式选择端口来源，兼容账户密码项目中的旧端口值。
- Modify: `apps/desktop/src/utils/releasePackage.ts` — 规范化密码库端口，并只在私钥模式校验项目端口。
- Modify: `apps/desktop/src/utils/releasePackage.test.ts` — 覆盖前端端口规范化和认证方式相关校验。
- Modify: `apps/desktop/src/components/ReleasePackagePanel.vue` — 条件展示端口输入，读取并展示密码库端口。
- Modify: `apps/desktop/src/components/ReleasePackagePanel.test.ts` — 固化密码模式无端口输入、私钥模式保留输入和凭据摘要展示端口。

### Task 1: 让密码库服务器元数据返回可信端口

**Files:**
- Modify: `apps/desktop/src-tauri/src/tools/vault.rs:277-318`
- Test: `apps/desktop/src-tauri/src/tools/vault.rs:1550-1635`

- [ ] **Step 1: 先增加服务器元数据端口测试**

在 `vault.rs` 的测试模块中，紧跟 `server_metadata_rejects_missing_wrong_type_and_incomplete_entry` 增加：

```rust
#[test]
fn server_metadata_reads_explicit_port_and_defaults_legacy_to_22() {
    let conn = vault_test_conn();
    insert_vault_entry(
        &conn,
        4,
        "server",
        r#"{"address":"10.0.0.8","port":2200,"account":"deploy"}"#,
        "secret",
    );
    insert_vault_entry(
        &conn,
        5,
        "server",
        r#"{"address":"10.0.0.9","account":"legacy"}"#,
        "secret",
    );

    assert_eq!(server_credential_metadata(&conn, 4).unwrap().port, 2200);
    assert_eq!(server_credential_metadata(&conn, 5).unwrap().port, 22);
}

#[test]
fn server_metadata_rejects_explicit_invalid_ports() {
    let conn = vault_test_conn();
    for (id, plain) in [
        (6, r#"{"address":"10.0.0.8","port":0,"account":"deploy"}"#),
        (7, r#"{"address":"10.0.0.8","port":65536,"account":"deploy"}"#),
        (8, r#"{"address":"10.0.0.8","port":22.5,"account":"deploy"}"#),
        (9, r#"{"address":"10.0.0.8","port":"22","account":"deploy"}"#),
        (10, r#"{"address":"10.0.0.8","port":null,"account":"deploy"}"#),
    ] {
        insert_vault_entry(&conn, id, "server", plain, "secret");
        assert_eq!(
            server_credential_metadata(&conn, id).unwrap_err(),
            "vault_entry_incomplete"
        );
    }
}
```

在 `resolved_server_credential_requires_session_and_keeps_password_out_of_metadata` 中增加：

```rust
assert_eq!(credential.metadata.port, 22);
```

- [ ] **Step 2: 运行测试并确认 RED**

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml tools::vault::tests::server_metadata
```

Expected: 编译失败，提示 `VaultServerCredentialMetadata` 没有 `port` 字段；该失败证明测试要求的新契约尚未实现。

- [ ] **Step 3: 实现端口解析和元数据字段**

在 `VaultServerCredentialMetadata` 增加字段，并在 `server_credential_metadata` 前增加解析函数：

```rust
#[derive(Debug)]
pub(crate) struct VaultServerCredentialMetadata {
    pub entry_id: i64,
    pub address: String,
    pub port: u16,
    pub account: String,
}

fn server_port(fields: &Value) -> Result<u16, String> {
    let Some(value) = fields.get("port") else {
        return Ok(22);
    };
    value
        .as_u64()
        .filter(|port| (1..=u16::MAX as u64).contains(port))
        .map(|port| port as u16)
        .ok_or_else(|| "vault_entry_incomplete".to_string())
}
```

读取地址和账号后解析端口，并写入返回值：

```rust
let address = fields["address"].as_str().unwrap_or("").trim().to_owned();
let port = server_port(&fields)?;
let account = fields["account"].as_str().unwrap_or("").trim().to_owned();
if address.is_empty() || account.is_empty() {
    return Err("vault_entry_incomplete".to_string());
}

Ok(VaultServerCredentialMetadata {
    entry_id,
    address,
    port,
    account,
})
```

- [ ] **Step 4: 重跑 Vault 定向测试并确认 GREEN**

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml tools::vault::tests::server_metadata
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml tools::vault::tests::resolved_server_credential
```

Expected: 两组测试均通过；显式端口、历史默认端口、无效端口和解密凭据元数据均符合契约。

- [ ] **Step 5: 提交密码库元数据改动**

```powershell
git add apps/desktop/src-tauri/src/tools/vault.rs
git commit -m "feat: 让密码库凭据元数据返回端口"
```

### Task 2: 让上线包按认证方式解析和校验端口

**Files:**
- Modify: `apps/desktop/src-tauri/src/tools/vault.rs:362-389`
- Modify: `apps/desktop/src-tauri/src/tools/release_package.rs:340-402`
- Modify: `apps/desktop/src-tauri/src/tools/release_package.rs:700-755`
- Test: `apps/desktop/src-tauri/src/tools/release_package.rs:1125-1220`

- [ ] **Step 1: 扩充测试凭据辅助函数并写失败的端点测试**

将 `insert_test_server_entry` 增加 `port: u16` 参数，并把端口写入测试条目的明文字段：

```rust
#[cfg(test)]
pub(crate) fn insert_test_server_entry(
    conn: &Connection,
    entry_id: i64,
    address: &str,
    port: u16,
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
            json!({ "address": address, "port": port, "account": account }).to_string(),
        ],
    )
    .unwrap();
}
```

将 Vault 测试中的既有调用改为：

```rust
insert_test_server_entry(&conn, 1, "10.0.0.8", 22, "deploy", "secret");
```

在 `password_preflight_uses_bound_vault_credential_and_rejects_password_payload` 中把调用改为：

```rust
super::super::vault::insert_test_server_entry(
    &conn,
    11,
    "deploy.example",
    2200,
    "deploy",
    "secret",
);
```

并在已有地址、用户名断言之间加入端口断言：

```rust
assert_eq!(endpoint.endpoint.host, "deploy.example");
assert_eq!(endpoint.endpoint.port, 2200);
assert_eq!(endpoint.endpoint.username, "deploy");
```

该项目测试载荷中的 `sshPort` 为 `2222`，所以断言明确证明最终端口来自密码库而不是项目。

- [ ] **Step 2: 运行端点测试并确认 RED**

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml tools::release_package::tests::password_preflight_uses_bound_vault_credential_and_rejects_password_payload
```

Expected: FAIL，实际端口为项目端口 `2222`，期望密码库端口 `2200`。

- [ ] **Step 3: 让密码模式端点使用元数据端口**

在 `upload_endpoint_with_conn` 的账户密码分支修改端点构建：

```rust
return Ok(UploadEndpoint {
    endpoint: RemoteEndpoint {
        host: metadata.address.to_ascii_lowercase(),
        port: metadata.port,
        username: metadata.account,
    },
    vault_entry_id: Some(metadata.entry_id),
});
```

- [ ] **Step 4: 重跑端点测试并确认 GREEN**

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml tools::release_package::tests::password_preflight_uses_bound_vault_credential_and_rejects_password_payload
```

Expected: PASS，最终端点为 `deploy.example:2200`。

- [ ] **Step 5: 增加私钥端点继续使用项目端口的回归测试**

在账户密码端点测试后增加：

```rust
#[test]
fn private_key_endpoint_keeps_using_the_project_port() {
    let conn = test_conn();
    let project_id = project_create_with_conn(&conn, &payload()).unwrap()["id"]
        .as_i64()
        .unwrap();

    let endpoint =
        upload_endpoint_with_conn(&conn, &load_project(&conn, project_id).unwrap()).unwrap();

    assert_eq!(endpoint.endpoint.host, "deploy.example.internal");
    assert_eq!(endpoint.endpoint.port, 2222);
    assert_eq!(endpoint.endpoint.username, "deploy");
    assert_eq!(endpoint.vault_entry_id, None);
}
```

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml tools::release_package::tests::private_key_endpoint_keeps_using_the_project_port
```

Expected: PASS，证明密码模式改用密码库端口没有改变私钥模式的端点来源。

- [ ] **Step 6: 先增加密码模式忽略项目端口的解析与校验测试**

在 `password_project_requires_vault_entry_but_private_key_keeps_host_and_username` 后增加：

```rust
#[test]
fn project_port_is_only_validated_for_private_key_authentication() {
    let mut password = payload();
    password["sshAuthType"] = json!("password");
    password["vaultEntryId"] = json!(17);
    password["sshPort"] = json!(0);
    let mut parsed_password = parse_project_payload(&password).unwrap();
    assert_eq!(parsed_password.ssh_port, 22);
    parsed_password.ssh_port = 0;
    assert!(validate_upload_project(&parsed_password).is_ok());

    let mut private_key = payload();
    private_key["sshPort"] = json!(0);
    assert_eq!(
        parse_project_payload(&private_key).unwrap_err(),
        "sshPort must be between 1 and 65535"
    );

    let mut parsed_private_key = parse_project_payload(&payload()).unwrap();
    parsed_private_key.ssh_port = 0;
    assert_eq!(
        validate_upload_project(&parsed_private_key).unwrap_err(),
        "SSH 端口必须在 1 到 65535 之间"
    );
}
```

- [ ] **Step 7: 运行认证方式校验测试并确认 RED**

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml tools::release_package::tests::project_port_is_only_validated_for_private_key_authentication
```

Expected: FAIL，账户密码载荷仍因 `sshPort: 0` 被通用解析拒绝。

- [ ] **Step 8: 按认证方式解析项目端口**

在 `parse_project_payload` 开头先解析认证方式和端口，再构建 `ProjectPayload`：

```rust
fn parse_project_payload(payload: &Value) -> Result<ProjectPayload, String> {
    let mut ssh_auth_type = optional_string(payload, "sshAuthType")?;
    if ssh_auth_type.is_empty() {
        ssh_auth_type = "password".into();
    }
    if !matches!(ssh_auth_type.as_str(), "password" | "private_key") {
        return Err("sshAuthType must be password or private_key".into());
    }
    let ssh_port = if ssh_auth_type == "private_key" {
        optional_port(payload, "sshPort", 22)?
    } else {
        22
    };

    let project = ProjectPayload {
        name: required_string(payload, "name")?,
        output_root: optional_string(payload, "outputRoot")?,
        package_type: ReleasePackageType::parse(&required_string(payload, "packageType")?)?,
        frontend_project_path: required_string(payload, "frontendProjectPath")?,
        frontend_build_command: required_string(payload, "frontendBuildCommand")?,
        frontend_artifact_path: required_string(payload, "frontendArtifactPath")?,
        frontend_artifact_mode: required_string(payload, "frontendArtifactMode")?,
        backend_project_path: required_string(payload, "backendProjectPath")?,
        backend_build_command: required_string(payload, "backendBuildCommand")?,
        backend_artifact_path: required_string(payload, "backendArtifactPath")?,
        ssh_host: optional_string(payload, "sshHost")?,
        ssh_port,
        ssh_username: optional_string(payload, "sshUsername")?,
        ssh_auth_type,
        vault_entry_id: optional_i64(payload, "vaultEntryId")?,
        ssh_private_key_path: optional_string(payload, "sshPrivateKeyPath")?,
        frontend_remote_dir: optional_string(payload, "frontendRemoteDir")?,
        backend_remote_path: optional_string(payload, "backendRemotePath")?,
    };
```

保留其后的项目类型和路径校验，删除原来在结构构建后重复设置、检查 `ssh_auth_type` 的代码。

将 `validate_upload_project` 的端口校验移入私钥分支：

```rust
if project.ssh_auth_type == "password" {
    if project.vault_entry_id.is_none() {
        return Err("密码认证必须绑定密码库服务器凭据".into());
    }
} else {
    if project.ssh_port == 0 {
        return Err("SSH 端口必须在 1 到 65535 之间".into());
    }
    if project.ssh_host.trim().is_empty() || project.ssh_username.trim().is_empty() {
        return Err("SSH 服务器地址和用户名不能为空".into());
    }
    if project.ssh_private_key_path.trim().is_empty() {
        return Err("私钥认证必须配置 SSH 私钥文件".into());
    }
}
```

- [ ] **Step 9: 运行上线包 Rust 测试并确认 GREEN**

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml tools::release_package::tests
```

Expected: 上线包 Rust 单元测试全部通过，密码模式使用密码库端口，私钥模式继续要求有效项目端口。

- [ ] **Step 10: 提交 Rust 上线包改动**

```powershell
git add apps/desktop/src-tauri/src/tools/vault.rs apps/desktop/src-tauri/src/tools/release_package.rs
git commit -m "fix: 上线包使用密码库服务器端口"
```

### Task 3: 规范化前端凭据端口并按认证方式校验

**Files:**
- Modify: `apps/desktop/src/utils/releasePackage.ts:145-190`
- Test: `apps/desktop/src/utils/releasePackage.test.ts:175-225`

- [ ] **Step 1: 先写端口规范化和条件校验测试**

在 `releasePackage.test.ts` 的导入列表加入 `normalizeVaultServerPort`，并在上传设置测试前增加：

```ts
it("normalizes Vault server ports without hiding explicit invalid values", () => {
  expect(normalizeVaultServerPort(undefined)).toBe(22);
  expect(normalizeVaultServerPort(2200)).toBe(2200);
  for (const invalid of [null, 0, 65_536, 22.5, "22", Number.NaN]) {
    expect(normalizeVaultServerPort(invalid)).toBeNull();
  }
});

it("validates the project SSH port only for private-key authentication", () => {
  const draft = createEmptyReleasePackageDraft();
  Object.assign(draft, {
    packageType: "server_upload",
    sshAuthType: "password",
    vaultEntryId: 17,
    sshPort: 0,
    frontendRemoteDir: "/srv/app/web",
    backendRemotePath: "/srv/app/app.jar",
  });
  expect(validateReleasePackageUpload(draft)).toBeNull();

  Object.assign(draft, {
    sshAuthType: "private_key",
    sshHost: "10.0.0.8",
    sshUsername: "deploy",
    sshPrivateKeyPath: "C:\\Keys\\deploy",
  });
  expect(validateReleasePackageUpload(draft)).toBe("SSH 端口必须在 1 到 65535 之间");
});
```

删除 `validates enabled server upload settings` 末尾将密码模式 `sshPort = 0` 期望为错误的旧断言。

- [ ] **Step 2: 运行前端工具测试并确认 RED**

Run:

```powershell
pnpm --filter @lazycat/desktop test -- releasePackage.test.ts
```

Expected: 编译失败，提示 `normalizeVaultServerPort` 尚未导出；实现函数后，条件校验用例仍会因密码模式端口被校验而失败。

- [ ] **Step 3: 实现密码库端口规范化函数**

在 `normalizeReleasePackageDraft` 前增加：

```ts
export function normalizeVaultServerPort(value: unknown): number | null {
  if (value === undefined) return 22;
  if (typeof value !== "number" || !Number.isInteger(value)) return null;
  return value >= 1 && value <= 65_535 ? value : null;
}
```

- [ ] **Step 4: 将项目端口校验移入私钥认证分支**

将 `validateReleasePackageUpload` 开头调整为：

```ts
export function validateReleasePackageUpload(draft: ReleasePackageProjectDraft): string | null {
  const value = normalizeReleasePackageDraft(draft);
  if (value.sshAuthType === "password" && value.vaultEntryId === null) {
    return "请选择密码库服务器凭据";
  }
  if (value.sshAuthType === "private_key") {
    if (!Number.isInteger(value.sshPort) || value.sshPort < 1 || value.sshPort > 65_535) {
      return "SSH 端口必须在 1 到 65535 之间";
    }
    if (!value.sshHost) return "请输入服务器地址";
    if (!value.sshUsername) return "请输入 SSH 用户名";
    if (!value.sshPrivateKeyPath) return "请选择 SSH 私钥文件";
  }
```

保留后续远程路径校验不变。

- [ ] **Step 5: 重跑前端工具测试并确认 GREEN**

Run:

```powershell
pnpm --filter @lazycat/desktop test -- releasePackage.test.ts
```

Expected: `releasePackage.test.ts` 全部通过。

- [ ] **Step 6: 提交前端纯逻辑改动**

```powershell
git add apps/desktop/src/utils/releasePackage.ts apps/desktop/src/utils/releasePackage.test.ts
git commit -m "fix: 按认证方式校验上线包端口"
```

### Task 4: 移除密码模式端口输入并展示凭据端口

**Files:**
- Modify: `apps/desktop/src/components/ReleasePackagePanel.vue:249-300`
- Modify: `apps/desktop/src/components/ReleasePackagePanel.vue:500-690`
- Modify: `apps/desktop/src/components/ReleasePackagePanel.vue:1384-1410`
- Test: `apps/desktop/src/components/ReleasePackagePanel.test.ts:210-245`

- [ ] **Step 1: 先写组件契约测试**

在 `binds a Vault server credential for password auth without rendering a password field` 测试后增加：

```ts
it("gets password-auth ports from Vault and keeps the input for private keys", () => {
  expect(source).toContain(
    '<el-form-item v-if="draft.sshAuthType === \'private_key\'" label="SSH 端口" required>',
  );
  expect(source).not.toContain('<el-form-item label="SSH 端口" required>');
  expect(source).toContain("port?: unknown");
  expect(source).toContain("normalizeVaultServerPort(entry.plainFields?.port)");
  expect(source).toContain("complete: Boolean(address && account && port !== null)");
  expect(source).toContain("{{ selectedVaultCredential.port }}");
  expect(source).toContain("缺少地址、端口、账号或密码");
});
```

- [ ] **Step 2: 运行组件测试并确认 RED**

Run:

```powershell
pnpm --filter @lazycat/desktop test -- ReleasePackagePanel.test.ts
```

Expected: FAIL，当前端口输入无认证方式条件，凭据模型不包含端口，摘要也未展示端口。

- [ ] **Step 3: 扩充凭据类型并导入端口规范化函数**

在 `ReleasePackagePanel.vue` 的工具函数导入列表加入：

```ts
normalizeVaultServerPort,
```

扩充本地类型：

```ts
interface VaultServerOption {
  id: number;
  title: string;
  environment: string;
  address: string;
  port: number | null;
  account: string;
  complete: boolean;
}

interface VaultMetaEntry {
  id: number;
  category: string;
  title: string;
  environment?: string;
  plainFields?: {
    address?: string;
    port?: unknown;
    account?: string;
  } | null;
}
```

- [ ] **Step 4: 从密码库选项读取端口并标记无效凭据**

将 `loadVaultServerOptions` 的映射调整为：

```ts
.map((entry) => {
  const address = entry.plainFields?.address?.trim() ?? "";
  const port = normalizeVaultServerPort(entry.plainFields?.port);
  const account = entry.plainFields?.account?.trim() ?? "";
  return {
    id: entry.id,
    title: entry.title || `(未命名凭据 #${entry.id})`,
    environment: entry.environment?.trim() ?? "",
    address,
    port,
    account,
    complete: Boolean(address && account && port !== null),
  };
});
```

将绑定失效判断扩充为已选凭据不完整也视为无效：

```ts
const vaultBindingInvalid = computed(() => (
  draft.sshAuthType === "password"
  && draft.vaultEntryId !== null
  && vaultOptionsLoaded.value
  && (selectedVaultCredential.value === null || !selectedVaultCredential.value.complete)
));
```

- [ ] **Step 5: 条件显示端口输入并补充只读摘要**

把现有端口表单项改为：

```vue
<el-form-item v-if="draft.sshAuthType === 'private_key'" label="SSH 端口" required>
  <el-input-number v-model="draft.sshPort" :disabled="running" :min="1" :max="65535" controls-position="right" class="full-width" />
</el-form-item>
```

在 `selectedVaultCredential` 摘要的服务器地址和 SSH 用户名之间增加：

```vue
<div>
  <span>SSH 端口</span>
  <code>{{ selectedVaultCredential.port }}</code>
</div>
```

将凭据不完整提示改为：

```ts
ElMessage.error("绑定的服务器凭据缺少地址、端口、账号或密码，请在密码管理中补充");
```

桌面宽度下让三个摘要字段稳定排成三列：

```css
.vault-credential-summary { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 8px; }
```

保留现有移动端 `.vault-credential-summary { grid-template-columns: 1fr; }` 规则。

- [ ] **Step 6: 重跑组件与工具测试并确认 GREEN**

Run:

```powershell
pnpm --filter @lazycat/desktop test -- ReleasePackagePanel.test.ts releasePackage.test.ts
```

Expected: 两个测试文件全部通过，密码模式只读展示密码库端口，私钥模式保留项目端口输入。

- [ ] **Step 7: 提交界面集成改动**

```powershell
git add apps/desktop/src/components/ReleasePackagePanel.vue apps/desktop/src/components/ReleasePackagePanel.test.ts
git commit -m "feat: 上线包展示密码库服务器端口"
```

### Task 5: 完整验证

**Files:**
- Verify only: all files changed in Tasks 1-4

- [ ] **Step 1: 运行相关前端测试**

Run:

```powershell
pnpm --filter @lazycat/desktop test -- ReleasePackagePanel.test.ts releasePackage.test.ts VaultEntryDialog.test.ts vault.test.ts
```

Expected: 所有相关 Vitest 用例通过，无失败和未处理错误。

- [ ] **Step 2: 运行相关 Rust 测试**

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml tools::vault::tests
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml tools::release_package::tests
```

Expected: Vault 与上线包 Rust 单元测试全部通过。

- [ ] **Step 3: 运行类型检查**

Run:

```powershell
pnpm typecheck
```

Expected: 全工作区类型检查通过。

- [ ] **Step 4: 构建桌面渲染层**

Run:

```powershell
pnpm --filter @lazycat/desktop build:web
```

Expected: Vite 构建成功，未引用公网资源。

- [ ] **Step 5: 检查最终差异**

Run:

```powershell
git status --short
git diff --check
git log -5 --oneline
```

Expected: `git diff --check` 无输出；工作区没有遗漏的未提交任务文件；最近提交包含本计划的四个实现提交。
