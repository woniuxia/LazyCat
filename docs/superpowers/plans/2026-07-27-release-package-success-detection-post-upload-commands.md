# 上线包成功检测与上传后命令 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为上线包增加前后端独立的构建成功关键字检测、上传后命令，以及只重试明确失败命令的安全链路。

**Architecture:** 沿用现有“目标级构建 → SFTP 事务上传 → 终态”流水线：构建执行器记录关键字命中，部署成功后返回一条已认证控制连接，运行时按前端、后端顺序执行命令。命令失败使用独立终态和内存重试快照；终态后的重试重新完成主机信任与认证，但不触碰构建产物和 SFTP 目标。

**Tech Stack:** Vue 3、TypeScript、Vitest、Element Plus、Tauri 2、Rust、rusqlite、ssh2。

---

## 文件职责

- `apps/desktop/src/types/release-package.ts`：前端项目配置、运行状态、命令状态和命令重试 IPC 类型。
- `apps/desktop/src/utils/releasePackage.ts`：空草稿、项目回填、规范化、状态文案和启动负载纯函数。
- `apps/desktop/src/utils/releasePackage.test.ts`：四个新配置字段和新终态的纯函数回归。
- `apps/desktop/src-tauri/src/tools/release_package.rs`：SQLite 幂等迁移、项目 CRUD、命令重试 IPC 解析与分发。
- `apps/desktop/src-tauri/src/tools/release_package_runtime.rs`：构建关键字判定、后置命令聚合、重试快照、运行槽和终态事件。
- `apps/desktop/src-tauri/src/tools/release_package_remote.rs`：已认证 SSH/SFTP 连接和远程 command channel 的底层执行。
- `apps/desktop/src-tauri/src/tools/release_package_deploy.rs`：远端文件事务；成功时把控制连接交回运行时。
- `apps/desktop/src/composables/useReleasePackageRuntime.ts`：前端运行态归并，分别保存上传重试和命令重试令牌。
- `apps/desktop/src/composables/useReleasePackageCommandRetry.ts`：命令重试准备、主机信任、认证和启动的一次性状态。
- `apps/desktop/src/components/ReleasePackagePanel.vue`：四个配置项、命令状态和“仅重试失败命令”交互。
- `apps/desktop/src/bridge/tauri.ts`：三个命令重试 channel 映射。
- `apps/desktop/src-tauri/src/global_notification.rs`、`apps/desktop/src/components/GlobalNotificationPopup.vue`：新终态通知文案与色调。
- `apps/desktop/src-tauri/src/tools/action_center/dispatches.rs`：新终态映射为动作失败，不能自动完成 Todo。

## Task 1：持久化四个项目配置字段

**Files:**

- Modify: `apps/desktop/src/types/release-package.ts`
- Modify: `apps/desktop/src/utils/releasePackage.ts`
- Test: `apps/desktop/src/utils/releasePackage.test.ts`
- Modify: `apps/desktop/src-tauri/src/tools/release_package.rs`

- [ ] **Step 1：先写前端失败测试**

在 `releasePackage.test.ts` 的项目 fixture 和空草稿断言中加入四字段，并增加规范化测试：

```ts
frontendSuccessKeyword: "Build completed",
backendSuccessKeyword: "BUILD SUCCESS",
frontendPostUploadCommand: "systemctl reload nginx",
backendPostUploadCommand: "systemctl restart portal",
```

```ts
it("normalizes optional release checks and post-upload commands", () => {
  const draft = createEmptyReleasePackageDraft();
  draft.frontendSuccessKeyword = "  Build completed  ";
  draft.backendSuccessKeyword = "  BUILD SUCCESS  ";
  draft.frontendPostUploadCommand = "\n  cd /srv/web\n  ./reload.sh\n";
  draft.backendPostUploadCommand = "\n systemctl restart portal \n";

  expect(normalizeReleasePackageDraft(draft)).toMatchObject({
    frontendSuccessKeyword: "Build completed",
    backendSuccessKeyword: "BUILD SUCCESS",
    frontendPostUploadCommand: "cd /srv/web\n  ./reload.sh",
    backendPostUploadCommand: "systemctl restart portal",
  });
});
```

- [ ] **Step 2：运行前端测试并确认失败**

Run:

```powershell
pnpm --filter @lazycat/desktop test -- src/utils/releasePackage.test.ts
```

Expected: FAIL，提示 `ReleasePackageProjectDraft` 缺少四个字段或空草稿断言不一致。

- [ ] **Step 3：补前端类型和纯函数最小实现**

在 `ReleasePackageProjectDraft` 增加：

```ts
frontendSuccessKeyword: string;
backendSuccessKeyword: string;
frontendPostUploadCommand: string;
backendPostUploadCommand: string;
```

在 `createEmptyReleasePackageDraft()` 和 `projectToReleasePackageDraft()` 显式映射四字段。保留 `normalizeReleasePackageDraft()` 对字符串统一 `trim()` 的现有行为，确保多行命令只移除整体首尾空白。

- [ ] **Step 4：写 Rust schema/CRUD 失败测试**

在 `release_package.rs` 测试模块的 `payload()` 增加 camelCase 字段，并扩展 schema 迁移和 round-trip 测试：

```rust
payload["frontendSuccessKeyword"] = json!("Build completed");
payload["backendSuccessKeyword"] = json!("BUILD SUCCESS");
payload["frontendPostUploadCommand"] = json!("systemctl reload nginx");
payload["backendPostUploadCommand"] = json!("systemctl restart portal");
```

```rust
assert_eq!(loaded.frontend_success_keyword, "Build completed");
assert_eq!(loaded.backend_success_keyword, "BUILD SUCCESS");
assert_eq!(loaded.frontend_post_upload_command, "systemctl reload nginx");
assert_eq!(loaded.backend_post_upload_command, "systemctl restart portal");
```

旧表迁移测试还要查询 `PRAGMA table_info(release_package_projects)`，断言四列存在且旧行读取为空字符串。

- [ ] **Step 5：运行 Rust 测试并确认失败**

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package::tests -- --nocapture
```

Expected: FAIL，提示 payload 字段未解析或数据库列不存在。

- [ ] **Step 6：实现 SQLite 幂等迁移与 CRUD**

在建表 SQL 和 `ensure_schema()` 的列迁移清单中加入：

```rust
("frontend_success_keyword", "ALTER TABLE release_package_projects ADD COLUMN frontend_success_keyword TEXT NOT NULL DEFAULT ''"),
("backend_success_keyword", "ALTER TABLE release_package_projects ADD COLUMN backend_success_keyword TEXT NOT NULL DEFAULT ''"),
("frontend_post_upload_command", "ALTER TABLE release_package_projects ADD COLUMN frontend_post_upload_command TEXT NOT NULL DEFAULT ''"),
("backend_post_upload_command", "ALTER TABLE release_package_projects ADD COLUMN backend_post_upload_command TEXT NOT NULL DEFAULT ''"),
```

给 `ReleasePackageProjectConfig`、`ProjectPayload`、`parse_project_payload()`、`row_to_project()`、所有 SELECT/INSERT/UPDATE 参数增加四字段。解析使用现有 `optional_string()`，由后端再次 `trim()`，不接受前端未规范化输入作为第二真值。

- [ ] **Step 7：运行前后端定向测试**

Run:

```powershell
pnpm --filter @lazycat/desktop test -- src/utils/releasePackage.test.ts
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package::tests -- --nocapture
```

Expected: PASS。

- [ ] **Step 8：提交配置契约闭环**

```powershell
git add apps/desktop/src/types/release-package.ts apps/desktop/src/utils/releasePackage.ts apps/desktop/src/utils/releasePackage.test.ts apps/desktop/src-tauri/src/tools/release_package.rs
git commit -m "feat(release-package): 保存成功检测与后置命令配置"
```

## Task 2：以日志关键字参与目标构建判定

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/release_package_runtime.rs`

- [ ] **Step 1：写关键字匹配失败测试**

给 PowerShell 测试辅助函数传入可选关键字，覆盖 stdout、stderr、大小写和退出码优先级：

```rust
#[test]
fn powershell_success_keyword_matches_stdout_or_stderr_case_sensitively() {
    assert!(run_test_command("Write-Output 'Build completed'", Some("Build completed")).is_ok());
    assert!(run_test_command("[Console]::Error.WriteLine('BUILD SUCCESS')", Some("BUILD SUCCESS")).is_ok());
    let error = run_test_command("Write-Output 'build completed'", Some("Build completed"))
        .unwrap_err();
    assert!(error.message().contains("日志未匹配成功关键字"));
}

#[test]
fn non_zero_exit_cannot_be_overridden_by_keyword() {
    let error = run_test_command("Write-Output 'BUILD SUCCESS'; exit 7", Some("BUILD SUCCESS"))
        .unwrap_err();
    assert!(error.message().contains("退出码 7"));
}
```

增加目标级测试，确保关键字命中后仍检查产物，未命中时目标失败且不会进入上传。

- [ ] **Step 2：运行关键字测试并确认失败**

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml success_keyword -- --nocapture
```

Expected: FAIL，现有 `run_powershell()` 不接受关键字也不返回命中结果。

- [ ] **Step 3：实现命令结果与匹配状态**

将命令成功结果从 `()` 改为：

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CommandOutcome {
    success_keyword_matched: bool,
}
```

`run_powershell()` 接收 `success_keyword: Option<&str>`。两个日志 reader 在已经拆分出的完整 `line` 上执行区分大小写的 `line.contains(keyword)`，用共享 `AtomicBool` 记录命中，同时继续原样发送日志。命令非零退出仍先返回 `CommandError::ExitCode`；退出为零后再返回 `CommandOutcome`。

- [ ] **Step 4：在目标阶段组合三个成功条件**

让 `run_command_phase()` 返回 `CommandOutcome`，`run_target()` 按目标取关键字：

```rust
let success_keyword = match target {
    ReleaseTarget::Frontend => project.frontend_success_keyword.trim(),
    ReleaseTarget::Backend => project.backend_success_keyword.trim(),
};
let outcome = run_command_phase(
    run_id,
    project.id,
    phase,
    &project_path,
    command,
    (!success_keyword.is_empty()).then_some(success_keyword),
    cancelled.clone(),
    pid,
    sink.clone(),
)?;
if !success_keyword.is_empty() && !outcome.success_keyword_matched {
    return Err(PipelineError::Failed {
        message: format!("{}构建命令退出成功，但日志未匹配成功关键字：{}", target_label(target), success_keyword),
    });
}
```

随后保留现有产物目录/文件校验。空关键字令 `success_keyword_matched` 视为 `true`，保证旧项目行为不变。

- [ ] **Step 5：运行上线包 runtime 定向测试**

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package_runtime -- --nocapture
```

Expected: PASS，且现有取消、并行构建和本地归档测试不回归。

- [ ] **Step 6：提交构建判定闭环**

```powershell
git add apps/desktop/src-tauri/src/tools/release_package_runtime.rs
git commit -m "feat(release-package): 支持日志关键字成功检测"
```

## Task 3：在已认证 SSH 连接上执行远程命令

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/release_package_remote.rs`
- Modify: `apps/desktop/src-tauri/src/tools/release_package_deploy.rs`

- [ ] **Step 1：定义命令回调和结果并写 fake 测试**

在 `release_package_deploy.rs` 定义最小跨模块契约：

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoteCommandResult {
    pub exit_code: i32,
}

pub trait RemoteFs: Send {
    // 保留现有文件方法。
    fn execute_command(
        &mut self,
        command: &str,
        cancelled: &AtomicBool,
        output: &mut dyn FnMut(&str, String),
    ) -> Result<RemoteCommandResult, DeployError>;
}
```

给 fake remote 增加命令输出、退出码和调用记录，并添加测试：

```rust
#[test]
fn remote_command_reports_both_streams_and_exit_code() {
    let mut remote = command_remote(7, [("stdout", b"ready\n"), ("stderr", b"warning\n")]);
    let mut lines = Vec::new();
    let result = remote.execute_command("./deploy.sh", &AtomicBool::new(false), &mut |stream, line| {
        lines.push((stream.to_string(), line));
    }).unwrap();
    assert_eq!(result.exit_code, 7);
    assert_eq!(lines, [("stdout".into(), "ready".into()), ("stderr".into(), "warning".into())]);
}
```

- [ ] **Step 2：运行测试并确认失败**

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml remote_command -- --nocapture
```

Expected: FAIL，`RemoteFs` 尚无 command channel 能力。

- [ ] **Step 3：让真实连接同时持有 Session 与 SFTP**

把 `SftpRemoteFs` 调整为：

```rust
pub struct SftpRemoteFs {
    session: Session,
    sftp: Sftp,
}
```

`connect()` 完成握手、指纹和认证后先创建 `sftp`，再同时保存 `session`。现有文件方法只使用 `sftp`，不改变路径和事务语义。

- [ ] **Step 4：实现 command channel 输出与取消**

在真实 `RemoteFs` 实现中：

```rust
fn execute_command(
    &mut self,
    command: &str,
    cancelled: &AtomicBool,
    output: &mut dyn FnMut(&str, String),
) -> Result<RemoteCommandResult, DeployError> {
    if cancelled.load(Ordering::Acquire) {
        return Err(DeployError::cancelled_command());
    }
    let mut channel = self.session.channel_session()
        .map_err(|error| DeployError::failed(format!("创建 SSH 命令通道失败：{error}")))?;
    channel.exec(command)
        .map_err(|error| DeployError::failed(format!("发送上传后命令失败：{error}")))?;
    read_command_streams(&mut channel, cancelled, output)?;
    channel.wait_close()
        .map_err(|error| DeployError::failed(format!("等待上传后命令结束失败：{error}")))?;
    let exit_code = channel.exit_status()
        .map_err(|error| DeployError::failed(format!("读取上传后命令退出码失败：{error}")))?;
    Ok(RemoteCommandResult { exit_code })
}
```

`read_command_streams()` 同时推进 stdout 和 stderr，按换行拆分并以 `String::from_utf8_lossy()` 生成可见文本；EOF 前的尾行也必须发送。底层读取错误返回失败，不能当作 EOF。取消依赖现有 `SshSocketRegistry::shutdown_all()` 中断阻塞 IO。

- [ ] **Step 5：补有损 UTF-8、读取失败和取消测试**

断言非法字节输出包含 `�`，模拟读取错误时结果为失败，取消时 `cancelled == true`。所有 fake `RemoteFs` 实现显式提供命令方法，未用于命令的 fake 返回稳定错误 `该测试连接不支持远程命令`，避免默认伪成功。

- [ ] **Step 6：运行 remote/deploy 测试**

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package_remote -- --nocapture
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package_deploy -- --nocapture
```

Expected: PASS。

- [ ] **Step 7：提交 SSH 命令底座**

```powershell
git add apps/desktop/src-tauri/src/tools/release_package_remote.rs apps/desktop/src-tauri/src/tools/release_package_deploy.rs
git commit -m "feat(release-package): 支持 SSH 上传后命令"
```

## Task 4：上传提交后交回控制连接并聚合前后端命令

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/release_package_deploy.rs`
- Modify: `apps/desktop/src-tauri/src/tools/release_package_runtime.rs`

- [ ] **Step 1：写部署成功交回连接测试**

将 `deploy_parallel()` 成功类型改为 `Result<DeploymentSuccess, DeployError>`，测试确认事务已经全部提交且返回的控制连接仍可调用：

```rust
let mut success = deploy_parallel(remotes, plan, cancelled, stop, progress, interrupt, recover)
    .expect("deployment succeeds");
assert_eq!(probe.committed_targets(), vec!["/srv/web", "/srv/app.jar"]);
let result = success.control.execute_command("true", &AtomicBool::new(false), &mut |_, _| {})
    .unwrap();
assert_eq!(result.exit_code, 0);
```

- [ ] **Step 2：运行部署测试并确认失败**

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml committed_connection -- --nocapture
```

Expected: FAIL，当前部署成功仅返回 `()` 并销毁连接。

- [ ] **Step 3：最小调整部署返回所有权**

让串行和并行路径在 `plan.commit(...)` 成功后返回用于 commit 的 control remote。所有错误、取消、回滚和恢复路径仍返回 `DeployError`，不得在失败分支泄漏连接或绕过临时路径清理。

定义：

```rust
pub struct DeploymentSuccess {
    pub control: Box<dyn RemoteFs>,
}
```

`deploy_parallel()` 返回 `Result<DeploymentSuccess, DeployError>`。现有仅关心成功/失败的调用方显式消费并丢弃 `control`，服务器上传运行时保留它。

- [ ] **Step 4：写运行时命令顺序与失败聚合测试**

使用 fake control 记录命令：

```rust
#[test]
fn post_upload_commands_run_after_commit_in_target_order() {
    let summary = run_commands_after_upload(
        succeeded_upload_summary(),
        vec![
            CommandSnapshot::new(ReleaseTarget::Frontend, "reload-web"),
            CommandSnapshot::new(ReleaseTarget::Backend, "restart-api"),
        ],
        fake_control([("reload-web", 7), ("restart-api", 0)]),
        sink(),
        &AtomicBool::new(false),
    );
    assert_eq!(executed_commands(), vec!["reload-web", "restart-api"]);
    assert_eq!(summary.status, "upload_succeeded_command_failed");
    assert!(summary.remote_committed);
    assert_eq!(summary.failed_commands.len(), 1);
}
```

再覆盖：无命令为成功、上传失败不执行、仅选择 backend 时不执行 frontend、前端失败不阻止后端、取消后不再启动后续命令。

- [ ] **Step 5：实现命令阶段和新终态**

扩展 `PipelineSummary`：

```rust
failed_commands: Vec<CommandSnapshot>,
command_retry_descriptor: Option<CommandRetryDescriptor>,
```

`execute_deployment_request()` 取得 `DeploymentSuccess.control` 后，只有 `remote_committed` 且配置非空才调用 `run_post_upload_commands()`。该函数按 `ReleaseTarget::Frontend`、`Backend` 排序逐条执行，给上传日志加 `[前端命令]` / `[后端命令]` 前缀；退出码非零记录失败但继续下一条。至少一条失败时设置：

```rust
summary.status = "upload_succeeded_command_failed";
summary.error = Some("服务器文件已上传，但上传后命令未全部成功".into());
```

取消时状态为 `cancelled`，错误明确服务器文件已上传，并且不生成任何命令重试描述符。

- [ ] **Step 6：运行部署和 runtime 测试**

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package_deploy -- --nocapture
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml post_upload -- --nocapture
```

Expected: PASS。

- [ ] **Step 7：提交上传与命令编排**

```powershell
git add apps/desktop/src-tauri/src/tools/release_package_deploy.rs apps/desktop/src-tauri/src/tools/release_package_runtime.rs
git commit -m "feat(release-package): 编排前后端上传后命令"
```

## Task 5：实现只重试明确失败命令的后端链路

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/release_package_runtime.rs`
- Modify: `apps/desktop/src-tauri/src/tools/release_package.rs`
- Modify: `apps/desktop/src/bridge/tauri.ts`

- [ ] **Step 1：写重试令牌生命周期失败测试**

在 runtime 测试中覆盖：

```rust
#[test]
fn command_retry_contains_only_failed_commands_and_rotates_after_failure() {
    let first = issue_command_retry(7, binding(), vec![
        CommandSnapshot::new(ReleaseTarget::Frontend, "reload-web"),
    ]).unwrap();
    let prepared = prepare_command_retry(&first, 7).unwrap();
    assert_eq!(prepared.targets, vec![ReleaseTarget::Frontend]);

    let job = consume_command_retry(&first, 7).unwrap();
    assert!(consume_command_retry(&first, 7).is_err());
    let second = finish_command_retry(job, vec![CommandSnapshot::new(
        ReleaseTarget::Frontend,
        "reload-web",
    )]).unwrap();
    assert_ne!(first, second);
}
```

增加取消测试，断言取消不签发新令牌；项目 ID 不匹配不能准备或消费；项目配置修改不改变快照命令和 endpoint/auth binding。

- [ ] **Step 2：运行 retry 测试并确认失败**

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml command_retry -- --nocapture
```

Expected: FAIL，命令重试存储和动作尚不存在。

- [ ] **Step 3：实现内存快照与一次性令牌**

在 runtime 中新增：

```rust
#[derive(Clone)]
struct CommandRetryJob {
    project_id: i64,
    binding: CommandAuthBinding,
    failed_commands: Vec<CommandSnapshot>,
}

static COMMAND_RETRIES: OnceLock<Mutex<HashMap<String, CommandRetryJob>>> = OnceLock::new();
```

`CommandAuthBinding` 保存失败运行时的 endpoint、用户名、认证类型、Vault entry ID 或私钥路径、可信指纹；不保存密码和私钥口令。`issue/prepare/consume` 均校验非空目标、项目 ID 和单次消费。`on_app_exit()` 清空命令重试和命令认证令牌。

- [ ] **Step 4：写 IPC 解析与认证边界失败测试**

在 `release_package.rs` 测试：

```rust
assert!(supported_actions().contains(&"command_retry_prepare"));
assert!(supported_actions().contains(&"command_retry_preflight"));
assert!(supported_actions().contains(&"command_retry_start"));
```

断言 `command_retry_prepare` 返回目标、host、port、username、authType、fingerprint，不返回命令正文、Vault 密码或私钥口令。`command_retry_preflight` 只做握手、指纹和认证，不调用 SFTP。`command_retry_start` 必须在启动线程前原子消费 retry/auth token 并占用运行槽。

- [ ] **Step 5：实现三个 IPC 动作**

契约固定为：

```ts
interface CommandRetryPrepareInput {
  projectId: number;
  retryToken: string;
}
interface CommandRetryPrepareResult {
  targets: ReleasePackageTarget[];
  host: string;
  port: number;
  username: string;
  authType: ReleasePackageSshAuthType;
  fingerprintSha256: string;
  probeToken: string;
}
interface CommandRetryPreflightInput {
  projectId: number;
  retryToken: string;
  probeToken: string;
  privateKeyPassphrase?: string;
}
interface CommandRetryPreflightResult {
  authToken: string;
  expiresAt: string;
}
interface CommandRetryStartInput {
  projectId: number;
  retryToken: string;
  authToken: string;
}
```

`prepare` 从快照 endpoint 探测主机并签发与 retry job 绑定的 probe token；已有 `host_trust` 继续消费该通用 probe token。`preflight` 在主机受信任后解析 Vault 或私钥秘密，建立 SSH 会话并验证 `channel_session()` 可创建，随后关闭测试通道和连接，只把秘密放入短期一次性 auth token。`start` 消费两个令牌，使用现有运行槽和事件 sink 执行失败命令快照。

- [ ] **Step 6：注册 bridge channel 并跑契约测试**

在 `tauri.ts` 增加：

```ts
"tool:release-package:command-retry-prepare": { domain: "release_package", action: "command_retry_prepare" },
"tool:release-package:command-retry-preflight": { domain: "release_package", action: "command_retry_preflight" },
"tool:release-package:command-retry-start": { domain: "release_package", action: "command_retry_start" },
```

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml command_retry -- --nocapture
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml contract_tests -- --nocapture
```

Expected: PASS。

- [ ] **Step 7：提交命令重试后端**

```powershell
git add apps/desktop/src-tauri/src/tools/release_package_runtime.rs apps/desktop/src-tauri/src/tools/release_package.rs apps/desktop/src/bridge/tauri.ts
git commit -m "feat(release-package): 支持仅重试失败命令"
```

## Task 6：扩展前端运行状态和命令重试 composable

**Files:**

- Modify: `apps/desktop/src/types/release-package.ts`
- Modify: `apps/desktop/src/composables/useReleasePackageRuntime.ts`
- Test: `apps/desktop/src/composables/useReleasePackageRuntime.test.ts`
- Create: `apps/desktop/src/composables/useReleasePackageCommandRetry.ts`
- Create: `apps/desktop/src/composables/useReleasePackageCommandRetry.test.ts`

- [ ] **Step 1：写 runtime reducer 失败测试**

扩展测试事件并断言两个令牌和目标命令状态互不覆盖：

```ts
emit("release-package://status", {
  ...status("run-1", 7, "running", "upload"),
  commandTarget: "frontend",
  commandStatus: "failed",
  error: "退出码 7",
});
emit("release-package://status", {
  ...status("run-1", 7, "upload_succeeded_command_failed"),
  commandRetryToken: "command-retry-1",
});

const projectRuntime = runtime.getProjectRuntime(7);
expect(projectRuntime.commandStatus.frontend).toBe("failed");
expect(projectRuntime.commandStatus.backend).toBe("pending");
expect(projectRuntime.commandRetryToken).toBe("command-retry-1");
expect(projectRuntime.retryToken).toBe("");
```

- [ ] **Step 2：运行 runtime 测试并确认失败**

Run:

```powershell
pnpm --filter @lazycat/desktop test -- src/composables/useReleasePackageRuntime.test.ts
```

Expected: FAIL，新状态和事件字段尚未定义。

- [ ] **Step 3：实现前端类型和状态归并**

增加：

```ts
export type ReleasePackageCommandStatus =
  | "skipped"
  | "pending"
  | "running"
  | "succeeded"
  | "failed"
  | "cancelled";
```

`ReleasePackageRunStatus` 增加 `upload_succeeded_command_failed`；`ReleasePackageStatusEvent` 增加可选 `commandTarget`、`commandStatus`、`commandRetryToken`。`ReleasePackageProjectRuntime` 增加：

```ts
commandStatus: Record<ReleasePackageTarget, ReleasePackageCommandStatus>;
commandErrors: Partial<Record<ReleasePackageTarget, string>>;
commandRetryToken: string;
```

`applyProjectStatus()` 在 `phase === "upload" && commandTarget` 时只更新命令状态，不覆盖上传字节；overall 事件分别写入 `retryToken` 和 `commandRetryToken`。`isRunning` 继续把命令阶段的 overall `running` 视为运行中。

- [ ] **Step 4：写命令重试 composable 失败测试**

mock `invokeToolByChannel`，覆盖 prepare → host trust → preflight → start 和 reset：

```ts
await retry.prepare(7, "retry-1");
expect(invokeMock).toHaveBeenCalledWith("tool:release-package:command-retry-prepare", {
  projectId: 7,
  retryToken: "retry-1",
});
await retry.preflight("");
expect(invokeMock).toHaveBeenCalledWith(
  "tool:release-package:command-retry-preflight",
  expect.objectContaining({ projectId: 7, retryToken: "retry-1", probeToken: "probe-1" }),
);
const started = await retry.start();
expect(started.runId).toBe("command-run-1");
expect(retry.privateKeyPassphrase.value).toBe("");
```

- [ ] **Step 5：实现 `useReleasePackageCommandRetry`**

composable 只持有一次性 UI 状态：

```ts
const prepareResult = ref<ReleasePackageCommandRetryPrepareResult | null>(null);
const authToken = ref("");
const privateKeyPassphrase = ref("");
const projectId = ref<number | null>(null);
const retryToken = ref("");
```

暴露 `prepare`、`trustHost`、`preflight`、`start`、`discard`、`reset`。`start()` 成功、弹窗关闭和任意失败路径都清空私钥口令；`reset()` 不吞掉 discard 错误，调用方负责展示。

- [ ] **Step 6：运行两个 composable 测试**

Run:

```powershell
pnpm --filter @lazycat/desktop test -- src/composables/useReleasePackageRuntime.test.ts src/composables/useReleasePackageCommandRetry.test.ts
```

Expected: PASS。

- [ ] **Step 7：提交前端运行态**

```powershell
git add apps/desktop/src/types/release-package.ts apps/desktop/src/composables/useReleasePackageRuntime.ts apps/desktop/src/composables/useReleasePackageRuntime.test.ts apps/desktop/src/composables/useReleasePackageCommandRetry.ts apps/desktop/src/composables/useReleasePackageCommandRetry.test.ts
git commit -m "feat(release-package): 跟踪后置命令运行与重试状态"
```

## Task 7：完成上线包面板配置和重试交互

**Files:**

- Modify: `apps/desktop/src/components/ReleasePackagePanel.vue`
- Test: `apps/desktop/src/components/ReleasePackagePanel.test.ts`

- [ ] **Step 1：写面板结构失败测试**

扩展源码结构和挂载测试，断言四个输入、状态和互斥按钮：

```ts
expect(source).toContain('v-model="draft.frontendSuccessKeyword"');
expect(source).toContain('v-model="draft.backendSuccessKeyword"');
expect(source).toContain('v-model="draft.frontendPostUploadCommand"');
expect(source).toContain('v-model="draft.backendPostUploadCommand"');
expect(source).toContain("仅重试失败命令");
expect(source).toContain("upload_succeeded_command_failed");
```

挂载后分别发送 `package_succeeded_upload_failed` 和 `upload_succeeded_command_failed`，断言前者只有“重试上传”，后者只有“仅重试失败命令”。

- [ ] **Step 2：运行面板测试并确认失败**

Run:

```powershell
pnpm --filter @lazycat/desktop test -- src/components/ReleasePackagePanel.test.ts
```

Expected: FAIL，页面尚无新配置和按钮。

- [ ] **Step 3：添加成功关键字输入**

在前后端构建命令提示下各加入单行输入：

```vue
<el-form-item label="成功日志关键字（可选）">
  <el-input
    v-model="draft.frontendSuccessKeyword"
    :disabled="running"
    placeholder="例如：Build completed"
  />
  <p class="command-hint">同时匹配 stdout 和 stderr，区分大小写；留空不检测。</p>
</el-form-item>
```

后端使用 `draft.backendSuccessKeyword` 和 `BUILD SUCCESS` 示例。

- [ ] **Step 4：添加前后端上传后命令输入**

在服务器远程目标配置内按目标放置多行输入：

```vue
<el-form-item label="前端上传后命令（可选）">
  <el-input
    v-model="draft.frontendPostUploadCommand"
    type="textarea"
    :autosize="{ minRows: 3, maxRows: 8 }"
    :disabled="running"
    placeholder="例如：systemctl reload nginx"
  />
</el-form-item>
```

后端使用 `draft.backendPostUploadCommand`。共用提示：“全部选中目标上传成功后执行；不自动注入 sudo、工作目录或路径变量。” 本地归档隐藏这两个输入但不清空字段。

- [ ] **Step 5：显示命令状态与互斥重试入口**

上传日志 header 下显示前后端命令 tag。只在 `status === "upload_succeeded_command_failed" && commandRetryToken` 显示“仅重试失败命令”；保留现有上传失败按钮条件，不让两个按钮同时出现。

命令重试弹窗复用现有主机指纹确认风格，但状态使用 `useReleasePackageCommandRetry()`，标题为“重试上传后命令”，不展示远端路径或覆盖选项。密码认证展示 Vault 摘要；有口令私钥允许重新输入。确认后调用 runtime `beginStart`、command retry `start` 和 `bindStartedRun`。

- [ ] **Step 6：补响应式样式和失败文案测试**

只复用现有 form/grid/tag token；窄宽度下输入框单列。测试命令失败文案必须包含“服务器文件已上传”，不得出现“上传失败”。关闭弹窗后断言私钥口令为空。

- [ ] **Step 7：运行面板及纯函数测试**

Run:

```powershell
pnpm --filter @lazycat/desktop test -- src/components/ReleasePackagePanel.test.ts src/utils/releasePackage.test.ts src/composables/useReleasePackageCommandRetry.test.ts
```

Expected: PASS。

- [ ] **Step 8：提交面板闭环**

```powershell
git add apps/desktop/src/components/ReleasePackagePanel.vue apps/desktop/src/components/ReleasePackagePanel.test.ts
git commit -m "feat(release-package): 配置并重试前后端后置命令"
```

## Task 8：对齐通知、动作中心和经验文档

**Files:**

- Modify: `apps/desktop/src-tauri/src/global_notification.rs`
- Modify: `apps/desktop/src/components/GlobalNotificationPopup.vue`
- Test: `apps/desktop/src/components/GlobalNotificationPopup.test.ts`
- Modify: `apps/desktop/src/utils/globalNotification.ts`
- Test: `apps/desktop/src/utils/globalNotification.test.ts`
- Modify: `apps/desktop/src-tauri/src/tools/action_center/dispatches.rs`
- Modify: `docs/experience/release-package.md`

- [ ] **Step 1：写新终态通知和动作映射失败测试**

Rust 通知测试断言新终态可生成通知且不含归档路径：

```rust
let payload = serde_json::to_value(build_release_package_notification(
    "run-command-failed",
    7,
    "Portal",
    ReleasePackageType::ServerUpload,
    "overall",
    "upload_succeeded_command_failed",
    None,
    Some("服务器文件已上传，但后置命令失败".into()),
).unwrap()).unwrap();
assert_eq!(payload["status"], "upload_succeeded_command_failed");
assert!(!payload.as_object().unwrap().contains_key("archivePath"));
```

动作中心参数化测试加入：

```rust
("upload_succeeded_command_failed", "failed", "pending"),
```

前端通知测试断言标题为“服务器命令执行失败”、详情说明文件已上传、图标和色调为 warning/danger，而不是成功样式。

- [ ] **Step 2：运行通知与动作测试并确认失败**

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml global_notification -- --nocapture
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml only_full_release_package_success_completes_the_todo -- --nocapture
pnpm --filter @lazycat/desktop test -- src/utils/globalNotification.test.ts src/components/GlobalNotificationPopup.test.ts
```

Expected: FAIL，新终态尚未列入允许集合和文案映射。

- [ ] **Step 3：实现通知与动作中心映射**

将 `upload_succeeded_command_failed` 加入 Rust 通知允许终态；前端 `releasePackageNotificationCopy()` 返回：

```ts
{
  title: "服务器命令执行失败",
  detail: "服务器文件已上传，但上传后命令未全部成功，请打开上线包工具查看日志。",
}
```

弹窗状态标签用“命令失败”，错误图标和独立 tone class。动作中心将新终态映射为 `STATUS_FAILED`，只有 `succeeded` 自动完成 Todo。

- [ ] **Step 4：更新上线包经验**

在 `docs/experience/release-package.md` 增加“文件提交与后置命令是两个不可回滚阶段”：全部文件提交后才执行目标命令；命令失败不能伪装为上传失败或回滚文件；仅重试明确失败命令并重新认证。补充本次验证命令和使用次数。

- [ ] **Step 5：运行定向测试**

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml global_notification -- --nocapture
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml action_center -- --nocapture
pnpm --filter @lazycat/desktop test -- src/utils/globalNotification.test.ts src/components/GlobalNotificationPopup.test.ts
```

Expected: PASS。

- [ ] **Step 6：提交联动闭环**

```powershell
git add apps/desktop/src-tauri/src/global_notification.rs apps/desktop/src/components/GlobalNotificationPopup.vue apps/desktop/src/components/GlobalNotificationPopup.test.ts apps/desktop/src/utils/globalNotification.ts apps/desktop/src/utils/globalNotification.test.ts apps/desktop/src-tauri/src/tools/action_center/dispatches.rs docs/experience/release-package.md
git commit -m "feat(release-package): 对齐后置命令失败终态"
```

## Task 9：完整验证与最终审查

**Files:**

- Test: all files changed by Tasks 1-8

- [ ] **Step 1：运行 Rust 上线包和动作中心测试**

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package -- --nocapture
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml action_center -- --nocapture
```

Expected: PASS，0 failed。

- [ ] **Step 2：运行前端定向测试**

```powershell
pnpm --filter @lazycat/desktop test -- src/components/ReleasePackagePanel.test.ts src/utils/releasePackage.test.ts src/composables/useReleasePackageRuntime.test.ts src/composables/useReleasePackageUploadPreflight.test.ts src/composables/useReleasePackageCommandRetry.test.ts src/utils/globalNotification.test.ts src/components/GlobalNotificationPopup.test.ts
```

Expected: PASS，0 failed。

- [ ] **Step 3：运行类型检查和渲染层构建**

```powershell
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
```

Expected: 两条命令退出码均为 0，无 TypeScript 或 Vite 构建错误。

- [ ] **Step 4：检查 diff 质量和任务范围**

```powershell
git diff --check
git status --short
git diff --stat HEAD~7..HEAD
```

Expected: `git diff --check` 无输出；状态只包含本任务预期文件或完全干净；没有 CDN、持久化秘密、上传失败回滚后置命令副作用或通用编排器改动。

- [ ] **Step 5：执行 Superpowers 完成前验证与代码评审**

使用 `superpowers:verification-before-completion` 复核所有最新命令输出，再使用 `superpowers:requesting-code-review` 检查：

```text
退出码 + 关键字 + 产物是否同为成功必要条件；
全部上传提交后才执行命令；
前端失败不阻止后端命令；
文件已提交后不错误回滚；
仅明确失败命令进入重试；
重试重新认证但不执行 SFTP；
秘密和令牌不进入日志、通知或数据库。
```

- [ ] **Step 6：若评审修复了问题，重新执行完整验证并提交**

只在确有修复时执行：

```powershell
git add apps/desktop/src apps/desktop/src-tauri/src docs/experience/release-package.md
git commit -m "fix(release-package): 修正后置命令评审问题"
```

然后重新执行 Task 9 Steps 1-4，所有命令必须再次通过后才能交付。
