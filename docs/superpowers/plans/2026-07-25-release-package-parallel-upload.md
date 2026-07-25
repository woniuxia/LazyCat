# 上线包 SSH 目标级并行上传 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在不改变远端完整替换事务、IPC 契约和认证秘密边界的前提下，减少 SFTP 重复往返，并让前后端目标使用最多两个独立 SSH/SFTP 会话并行上传。

**Architecture:** 先把远端部署拆成可复用的 `DeploymentPlan`：本地校验和路径规划不连接服务器，远端准备只创建一次目录，worker 只上传并校验各自 temp 目标，协调器最后串行提交。runtime 建立 1 或 2 个独立 `SftpRemoteFs`，用连接注册表支持全量取消；并行进度通过线程安全 reporter 聚合并按 100ms 节流，现有 `deploy` 保留为串行回归入口。

**Tech Stack:** Rust、Tauri 2、`ssh2`/SFTP、标准线程与原子类型、Cargo tests、现有 Vue/IPC 契约（本计划不改前端契约）。

---

## 前置条件与文件职责

执行本计划前，先完成并验证 `docs/superpowers/plans/2026-07-24-release-package-review-fixes.md`。该计划会改变提交成功警告、远端目标预检和令牌生命周期；本计划基于那些结构化结果工作，不把两组改动混在同一提交。

当前工作区使用 `main`，按仓库规则直接修改；每个任务只暂存本任务文件。

文件职责固定如下：

- `apps/desktop/src-tauri/src/tools/release_package_deploy.rs`：部署计划、目录集合、worker 上传/校验、串行提交/回滚，以及共享 fake 远端测试。
- `apps/desktop/src-tauri/src/tools/release_package_remote.rs`：SSH socket 注册表、会话建立时的 socket 注册、SFTP 适配和真实 fixture 测试。
- `apps/desktop/src-tauri/src/tools/release_package_runtime.rs`：活动运行资源、用户取消与内部停止标记、连接数量编排、进度 reporter 和事件发送。
- `docs/experience/release-package.md`：固化目标级并发、目录预创建、进度节流和事务边界经验。

不修改 `apps/desktop/src/bridge/tauri.ts`、`apps/desktop/src/types/release-package.ts`、Vue 组件或数据库。

## Task 1: 建立本地部署计划和目录集合

**Files:**
- Modify: `apps/desktop/src-tauri/src/tools/release_package_deploy.rs:197-350`
- Test: `apps/desktop/src-tauri/src/tools/release_package_deploy.rs` 的 `transaction_tests` 模块

- [ ] **Step 1: 写失败测试，锁定目录集合和本地来源校验**

在现有 `two_target_request()` 后增加以下测试。测试只创建本地产物和 fake request，不触碰远端：

```rust
#[test]
fn deployment_plan_deduplicates_frontend_directories_parent_first() {
    let (root, request) = two_target_request();
    let plan = super::DeploymentPlan::new(request).unwrap();
    drop(root);

    assert_eq!(
        plan.frontend_directories(),
        &[
            "/srv/app/web.__lazycat_tmp_run-1".to_string(),
            "/srv/app/web.__lazycat_tmp_run-1/assets".to_string(),
        ]
    );
}

#[test]
fn deployment_plan_rejects_changed_sources_before_remote_prepare() {
    let (root, request) = two_target_request();
    fs::write(root.path().join("dist/index.html"), "changed-size").unwrap();

    let error = super::DeploymentPlan::new(request).unwrap_err();
    drop(root);

    assert!(error.message.contains("部署产物在打包后发生变化"));
}

#[test]
fn deployment_plan_keeps_backend_without_frontend_directories() {
    let (root, mut request) = two_target_request();
    request.targets.remove(0);
    let plan = super::DeploymentPlan::new(request).unwrap();
    drop(root);

    assert!(plan.frontend_directories().is_empty());
}
```

- [ ] **Step 2: 运行定向测试，确认 RED**

运行：

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml deployment_plan_deduplicates_frontend_directories_parent_first -- --nocapture
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml deployment_plan_rejects_changed_sources_before_remote_prepare -- --nocapture
```

预期：编译失败，提示 `DeploymentPlan` 或 `frontend_directories` 尚不存在。

- [ ] **Step 3: 实现 `DeploymentPlan` 和唯一目录计算**

在 `DeploymentRequest` 和 `TransactionTarget` 后增加以下结构与入口。`RemoteFs` 同时增加 `Send` 约束，为后续将 boxed remote 移入 worker 做类型保证：

```rust
pub trait RemoteFs: Send {
    fn metadata(&self, path: &str) -> Result<Option<RemoteMetadata>, DeployError>;
    fn create_dir(&mut self, path: &str) -> Result<(), DeployError>;
    fn read_dir(&self, path: &str) -> Result<Vec<RemoteDirEntry>, DeployError>;
    fn write_file(
        &mut self,
        remote_path: &str,
        local_path: &Path,
        stop_requested: &AtomicBool,
        progress: &mut dyn FnMut(u64),
    ) -> Result<(), DeployError>;
    fn rename(&mut self, source: &str, target: &str) -> Result<(), DeployError>;
    fn remove_tree(&mut self, path: &str) -> Result<(), DeployError>;
}

#[derive(Clone, Debug)]
pub(crate) struct DeploymentPlan {
    request: DeploymentRequest,
    transactions: Vec<TransactionTarget>,
    frontend_directories: Vec<String>,
}

impl DeploymentPlan {
    pub(crate) fn new(request: DeploymentRequest) -> Result<Self, DeployError> {
        validate_request(&request)?;
        for target in &request.targets {
            target
                .manifest
                .verify_source()
                .map_err(DeployError::failed)?;
        }
        let transactions = request
            .targets
            .iter()
            .map(|target| TransactionTarget {
                final_path: target.remote_path.clone(),
                temp_path: transaction_path(&target.remote_path, "tmp", &request.run_id),
                backup_path: transaction_path(&target.remote_path, "backup", &request.run_id),
                expected_exists: target.expected_exists,
                backed_up: false,
                committed: false,
            })
            .collect::<Vec<_>>();
        let frontend_directories = collect_frontend_directories(&request, &transactions)?;
        Ok(Self {
            request,
            transactions,
            frontend_directories,
        })
    }

    pub(crate) fn request(&self) -> &DeploymentRequest {
        &self.request
    }

    pub(crate) fn target_count(&self) -> usize {
        self.request.targets.len()
    }

    pub(crate) fn frontend_directories(&self) -> &[String] {
        &self.frontend_directories
    }

    pub(crate) fn temp_paths(&self) -> Vec<String> {
        self.transactions
            .iter()
            .map(|transaction| transaction.temp_path.clone())
            .collect()
    }
}
```

`validate_request` 搬出当前 `deploy` 的空目标、runId 和远端路径校验；`collect_frontend_directories` 对每个前端相对文件路径逐段构造 temp 父目录，加入 temp 根目录后使用 `BTreeSet` 去重，再按深度和字典序输出。后端目标不得贡献目录项。实现时使用下面的纯函数，确保空前端也会创建 temp 根目录：

```rust
fn collect_frontend_directories(
    request: &DeploymentRequest,
    transactions: &[TransactionTarget],
) -> Result<Vec<String>, DeployError> {
    let mut directories = BTreeSet::new();
    for (target, transaction) in request.targets.iter().zip(transactions) {
        if target.manifest.target != ReleaseTarget::Frontend {
            continue;
        }
        directories.insert(transaction.temp_path.clone());
        for entry in &target.manifest.entries {
            let mut current = transaction.temp_path.clone();
            let mut segments = entry.relative_path.split('/').peekable();
            while let Some(segment) = segments.next() {
                if segments.peek().is_none() {
                    break;
                }
                current.push('/');
                current.push_str(segment);
                directories.insert(current.clone());
            }
        }
    }
    let mut directories = directories.into_iter().collect::<Vec<_>>();
    directories.sort_by_key(|path| (path.split('/').count(), path.clone()));
    Ok(directories)
}
```

`DeploymentPlan::new` 必须在建立任何 SSH 会话前调用；为 `RemoteFs` 增加 `Send` 后，在模块测试中加入 `fn assert_send<T: Send>() {}` 和 `assert_send::<SftpRemoteFs>();`，让后续 worker 的类型约束在编译期暴露。

- [ ] **Step 4: 运行 Task 1 测试，确认 GREEN**

运行：

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml deployment_plan -- --nocapture
```

预期：3 个新增测试通过；尚未改变远端上传行为。

- [ ] **Step 5: 提交本任务**

```powershell
git add -- apps/desktop/src-tauri/src/tools/release_package_deploy.rs
git commit -m "feat(release-package): 增加远端部署计划"
```

## Task 2: 远端准备一次目录并保留串行回归入口

**Files:**
- Modify: `apps/desktop/src-tauri/src/tools/release_package_deploy.rs:280-620`
- Test: `apps/desktop/src-tauri/src/tools/release_package_deploy.rs` 的 `transaction_tests`

- [ ] **Step 1: 写目录调用次数和串行事务失败测试**

在 `FakeRemoteFs` 增加 `metadata_calls: Vec<String>`、`create_dir_calls: Vec<String>`，每次调用记录具体路径。增加测试：

```rust
#[test]
fn deployment_prepares_each_frontend_directory_once() {
    let (root, request) = two_target_request();
    let plan = super::DeploymentPlan::new(request).unwrap();
    let mut remote = FakeRemoteFs::with_existing_release();

    plan.prepare_remote(&mut remote).unwrap();
    drop(root);

    assert_eq!(
        remote
            .create_dir_calls
            .iter()
            .filter(|path| path.contains("__lazycat_tmp_run-1"))
            .count(),
        2
    );
    for directory in plan.frontend_directories() {
        assert_eq!(
            remote
                .metadata_calls
                .iter()
                .filter(|path| *path == directory)
                .count(),
            1,
            "directory was checked more than once: {directory}",
        );
    }
}

#[test]
fn serial_deploy_still_rolls_back_after_second_commit_failure() {
    let (root, request) = two_target_request();
    let mut remote = FakeRemoteFs::with_existing_release();
    remote.fail_rename_to("/srv/app/app.jar");

    let error = deploy(&mut remote, &request, &AtomicBool::new(false), |_, _| {})
        .expect_err("commit must fail");
    drop(root);

    assert!(error.message.contains("远端提交失败"));
    assert_eq!(remote.read("/srv/app/web/old.js"), b"old");
    assert_eq!(remote.read("/srv/app/app.jar"), b"old-jar");
}

#[test]
fn deployment_prepares_temp_root_for_empty_frontend() {
    let root = super::tests::TestDir::new();
    let source = root.path().join("empty");
    fs::create_dir_all(&source).unwrap();
    let request = DeploymentRequest {
        run_id: "run-empty".into(),
        targets: vec![DeploymentTarget {
            manifest: ArtifactManifest::from_directory(ReleaseTarget::Frontend, &source).unwrap(),
            remote_path: "/srv/app/empty-web".into(),
            expected_exists: false,
        }],
    };
    let plan = super::DeploymentPlan::new(request).unwrap();
    let mut remote = FakeRemoteFs::base();

    plan.prepare_remote(&mut remote).unwrap();

    assert!(remote.exists("/srv/app/empty-web.__lazycat_tmp_run-empty"));
}
```

- [ ] **Step 2: 运行测试，确认 RED**

运行：

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml deployment_prepares_each_frontend_directory_once -- --nocapture
```

预期：编译失败，`DeploymentPlan::prepare_remote` 和调用计数尚不存在。

- [ ] **Step 3: 实现远端准备和上传/校验方法**

在 `DeploymentPlan` 增加以下方法；`prepare_remote` 负责 temp/backup 冲突检查和按计划创建目录，`upload_target` 不再调用逐文件 `ensure_remote_dir`：

```rust
impl DeploymentPlan {
    pub(crate) fn prepare_remote(
        &self,
        remote: &mut dyn RemoteFs,
    ) -> Result<(), DeployError> {
        for transaction in &self.transactions {
            if remote.metadata(&transaction.temp_path)?.is_some()
                || remote.metadata(&transaction.backup_path)?.is_some()
            {
                return Err(DeployError::failed(format!(
                    "远端部署临时或备份路径已存在：{}",
                    transaction.final_path
                )));
            }
        }
        let frontend_roots = self
            .request
            .targets
            .iter()
            .zip(&self.transactions)
            .filter(|(target, _)| target.manifest.target == ReleaseTarget::Frontend)
            .map(|(_, transaction)| transaction.temp_path.as_str())
            .collect::<BTreeSet<_>>();
        for directory in &self.frontend_directories {
            if !frontend_roots.contains(directory.as_str()) {
                match remote.metadata(directory)? {
                    Some(metadata) if metadata.kind == RemoteKind::Directory => continue,
                    Some(_) => {
                        return Err(DeployError::failed(format!(
                            "远端目录路径已被非目录占用：{directory}"
                        )))
                    }
                    None => {}
                }
            }
            remote.create_dir(directory)?;
        }
        Ok(())
    }

    pub(crate) fn upload_target(
        &self,
        index: usize,
        remote: &mut dyn RemoteFs,
        stop_requested: &AtomicBool,
        progress: &mut dyn FnMut(u64, &str),
    ) -> Result<(), DeployError> {
        let target = self
            .request
            .targets
            .get(index)
            .ok_or_else(|| DeployError::failed("部署目标索引无效"))?;
        let transaction = self
            .transactions
            .get(index)
            .ok_or_else(|| DeployError::failed("部署事务索引无效"))?;
        if stop_requested.load(Ordering::Acquire) {
            return Err(DeployError::cancelled());
        }
        match target.manifest.target {
            ReleaseTarget::Frontend => {
                for entry in &target.manifest.entries {
                    if stop_requested.load(Ordering::Acquire) {
                        return Err(DeployError::cancelled());
                    }
                    let remote_path = format!("{}/{}", transaction.temp_path, entry.relative_path);
                    let mut file_progress = |bytes| progress(bytes, entry.relative_path.as_str());
                    remote.write_file(
                        &remote_path,
                        &local_entry_path(&target.manifest, entry),
                        stop_requested,
                        &mut file_progress,
                    )?;
                }
            }
            ReleaseTarget::Backend => {
                let entry = target
                    .manifest
                    .entries
                    .first()
                    .ok_or_else(|| DeployError::failed("后端部署清单缺少文件"))?;
                let mut file_progress = |bytes| progress(bytes, entry.relative_path.as_str());
                remote.write_file(
                    &transaction.temp_path,
                    &local_entry_path(&target.manifest, entry),
                    stop_requested,
                    &mut file_progress,
                )?;
            }
        }
        Ok(())
    }
}
```

把现有 `verify_remote_target`、`validate_formal_target`、`cleanup_temps` 和提交循环分别改为 `DeploymentPlan` 方法，固定使用以下签名，避免后续 worker 和协调器各自维护一套事务真值：

```rust
impl DeploymentPlan {
    pub(crate) fn verify_target(
        &self,
        index: usize,
        remote: &dyn RemoteFs,
    ) -> Result<(), DeployError>;

    pub(crate) fn validate_formal_targets(
        &self,
        remote: &dyn RemoteFs,
    ) -> Result<(), DeployError>;

    pub(crate) fn cleanup_temps(&self, remote: &mut dyn RemoteFs) -> Vec<String>;

    pub(crate) fn commit(
        &mut self,
        remote: &mut dyn RemoteFs,
        cancelled: &AtomicBool,
    ) -> Result<(), DeployError>;
}
```

`prepare_remote` 的 temp 冲突检查已经对前端 temp 根做过一次 `metadata`，因此根目录直接 `create_dir`；子目录各检查一次并按父目录优先顺序创建。失败时由调用方立即调用 `cleanup_temps` 并把恢复路径并入原错误；上传、临时目标校验、正式目标复核失败也必须走同一清理路径。`commit` 是唯一能修改 `backed_up`/`committed` 的方法，并沿用前置流程修复后的 `DeployError.committed` 字段：提交点后的备份清理失败返回 `committed: true`，提交点前的错误保持 `false`。删除旧的逐文件 `ensure_remote_dir` 及其不再使用的 `remote_parent`，不能在预创建循环中重新遍历绝对路径祖先。

- [ ] **Step 4: 让现有 `deploy` 委托给计划方法**

将 `deploy` 改为以下顺序，保持原有公开签名和所有已有事务错误文本。`run_step` 只负责在失败时补充临时路径，不能吞掉原错误：

```rust
pub fn deploy(
    remote: &mut dyn RemoteFs,
    request: &DeploymentRequest,
    cancelled: &AtomicBool,
    mut progress: impl FnMut(u64, &str),
) -> Result<(), DeployError> {
    let mut plan = DeploymentPlan::new(request.clone())?;
    if let Err(mut error) = plan.prepare_remote(remote) {
        error.recovery_paths.extend(plan.cleanup_temps(remote));
        error.recovery_paths.sort();
        error.recovery_paths.dedup();
        return Err(error);
    }
    for index in 0..plan.target_count() {
        if let Err(mut error) = plan.upload_target(index, remote, cancelled, &mut progress) {
            error.recovery_paths.extend(plan.cleanup_temps(remote));
            error.recovery_paths.sort();
            error.recovery_paths.dedup();
            return Err(error);
        }
        if let Err(mut error) = plan.verify_target(index, remote) {
            error.recovery_paths.extend(plan.cleanup_temps(remote));
            error.recovery_paths.sort();
            error.recovery_paths.dedup();
            return Err(error);
        }
    }
    if cancelled.load(Ordering::Acquire) {
        return plan.cancel_and_cleanup(remote);
    }
    if let Err(mut error) = plan.validate_formal_targets(remote) {
        error.recovery_paths.extend(plan.cleanup_temps(remote));
        error.recovery_paths.sort();
        error.recovery_paths.dedup();
        return Err(error);
    }
    plan.commit(remote, cancelled)
}
```

`DeploymentPlan::upload_target` 在进入文件循环前不再重复 `verify_source`；来源校验只发生在 `new`。不要在 wrapper 中绕过计划直接操作远端。

- [ ] **Step 5: 运行远端部署回归测试，确认 GREEN**

运行：

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package_deploy -- --nocapture
```

预期：原有 10 个左右部署/事务测试和新增目录调用测试全部通过；`second_target_commit_failure_restores_first_target`、`rollback_failure_reports_recovery_paths_without_deleting_backup` 和取消清理语义不变。

- [ ] **Step 6: 提交本任务**

```powershell
git add -- apps/desktop/src-tauri/src/tools/release_package_deploy.rs
git commit -m "perf(release-package): 预创建远端上传目录"
```

## Task 3: 增加可关闭的多会话 SSH socket 注册表

**Files:**
- Modify: `apps/desktop/src-tauri/src/tools/release_package_remote.rs:1-285,472-520`
- Modify: `apps/desktop/src-tauri/src/tools/release_package_runtime.rs:1-235,1287-1335,1418-1440`
- Test: 上述两个文件内现有测试模块

- [ ] **Step 1: 写连接注册表失败测试**

在 `release_package_remote.rs` 测试模块增加 loopback socket helper 和注册表测试：

```rust
#[test]
fn socket_registry_tracks_and_clears_all_connections() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let first = std::net::TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let second = std::net::TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let _ = listener.accept().unwrap();
    let _ = listener.accept().unwrap();

    let registry = SshSocketRegistry::new();
    registry.register(first.try_clone().unwrap()).unwrap();
    registry.register(second.try_clone().unwrap()).unwrap();
    assert_eq!(registry.len_for_test(), 2);

    registry.clear();
    assert_eq!(registry.len_for_test(), 0);
}
```

在 `release_package_runtime.rs` 把现有 `cancellation_closes_the_active_ssh_socket` 扩展为注册两个 client，并断言 `request_cancel` 后两个 registry socket 均已关闭/清空，同时 `upload_stop` 已置为 `true`。

- [ ] **Step 2: 运行测试，确认 RED**

运行：

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml socket_registry_tracks_and_clears_all_connections -- --nocapture
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml cancellation_closes_the_active_ssh_socket -- --nocapture
```

预期：编译失败，当前只有 `Arc<Mutex<Option<TcpStream>>>`，没有 registry 和 `upload_stop`。

- [ ] **Step 3: 实现 `SshSocketRegistry`**

在 `release_package_remote.rs` 的常量定义后增加：

```rust
#[derive(Default)]
pub struct SshSocketRegistry {
    sockets: Mutex<Vec<TcpStream>>,
    shutdown_requested: AtomicBool,
}

impl SshSocketRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, socket: TcpStream) -> Result<(), String> {
        let mut sockets = self
            .sockets
            .lock()
            .map_err(|_| "SSH 连接状态不可用".to_string())?;
        if self.shutdown_requested.load(Ordering::Acquire) {
            let _ = socket.shutdown(Shutdown::Both);
            return Err("SSH 上传已取消".to_string());
        }
        sockets.push(socket);
        Ok(())
    }

    pub fn shutdown_all(&self) {
        self.shutdown_requested.store(true, Ordering::Release);
        let sockets = match self.sockets.lock() {
            Ok(mut sockets) => std::mem::take(&mut *sockets),
            Err(_) => return,
        };
        for socket in sockets {
            let _ = socket.shutdown(Shutdown::Both);
        }
    }

    pub fn clear(&self) {
        if let Ok(mut sockets) = self.sockets.lock() {
            sockets.clear();
        }
    }

    #[cfg(test)]
    fn len_for_test(&self) -> usize {
        self.sockets.lock().map(|sockets| sockets.len()).unwrap_or(0)
    }
}
```

补充 `use std::net::Shutdown`。`register` 必须在持有 registry 锁时再次检查 `shutdown_requested`，避免“取消已清空旧连接、随后连接线程又注册新 socket”的竞态；`shutdown_all` 先设置停止位，再 drain 并关闭全部 socket，因此取消后 `len_for_test()` 为 0。再增加 `socket_registry_rejects_connections_registered_after_shutdown`，先调用 `shutdown_all`，随后断言 `register` 返回 `SSH 上传已取消` 且 registry 仍为空。

把 `handshake_session_with_socket` 的参数从 `Option<&Arc<Mutex<Option<TcpStream>>>>` 改成 `Option<&SshSocketRegistry>`。建立 TCP stream 后 clone 一份并调用 `registry.register(clone)`；注册失败要在 SSH handshake 前显式返回，原始 stream 由错误路径 drop。

- [ ] **Step 4: 接入 `SftpRemoteFs::connect` 和 probe/preflight 调用**

将签名固定为：

```rust
pub fn connect(
    binding: &PreflightBinding,
    expected_fingerprint: &str,
    secret: &AuthSecret,
    sockets: &SshSocketRegistry,
) -> Result<Self, DeployError>
```

`probe_host`、`run_remote_preflight` 继续通过 `handshake_session` 传 `None`，只有正式上传/重试传运行级 registry。更新 remote 测试和 loopback fixture helper 的所有 `Arc<Mutex<Option<_>>>` 用法，并增加 `assert_send::<SftpRemoteFs>()` 编译期测试。

- [ ] **Step 5: 接入运行态资源和取消**

把 `ActiveRun.ssh_socket` 改名为 `ssh_sockets: Arc<SshSocketRegistry>`，新增 `upload_stop: Arc<AtomicBool>`。`start` 和 `upload_retry` 各自初始化：

```rust
let upload_stop = Arc::new(AtomicBool::new(false));
let ssh_sockets = Arc::new(SshSocketRegistry::new());
```

`request_cancel` 在 `claim_lock` 内同步设置两个标记；若终态已经被任务抢占则同步恢复，只有取消赢得竞态时才关闭连接：

```rust
fn request_cancel(active: &ActiveRun) -> bool {
    let _guard = active.claim_lock.lock().unwrap();
    active.cancelled.store(true, Ordering::Release);
    active.upload_stop.store(true, Ordering::Release);
    if active.finished.load(Ordering::Acquire) {
        if active.cancel_won.load(Ordering::Acquire) {
            return true;
        }
        active.cancelled.store(false, Ordering::Release);
        active.upload_stop.store(false, Ordering::Release);
        return false;
    }
    active.process_slots.terminate_all();
    active.ssh_sockets.shutdown_all();
    true
}
```

正常部署结束调用 `ssh_sockets.clear()`，不能只移除一个 socket。`SshSocketRegistry` 为每次 start/retry 新建，取消后的实例不复用。

- [ ] **Step 6: 运行连接与取消回归并提交**

运行：

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package_remote -- --nocapture
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package_runtime -- --nocapture
```

预期：既有认证、令牌、取消和运行态测试通过；真实 SSH fixture 仍按环境变量缺失显示 ignored。

```powershell
git add -- apps/desktop/src-tauri/src/tools/release_package_remote.rs apps/desktop/src-tauri/src/tools/release_package_runtime.rs
git commit -m "feat(release-package): 支持关闭多个 SSH 上传连接"
```

## Task 4: 实现双目标 worker 协调和串行提交

**Files:**
- Modify: `apps/desktop/src-tauri/src/tools/release_package_deploy.rs:350-650`
- Test: `apps/desktop/src-tauri/src/tools/release_package_deploy.rs` 的新增并发测试模块

- [ ] **Step 1: 写共享 fake 远端和真实重叠测试**

新增 `SharedFakeRemoteFs`，其节点树放在 `Arc<Mutex<BTreeMap<String, Node>>>`，两个 remote 实例共享同一远端状态。每个实例保存独立的 `entered_first_write: bool`，barrier 只在该实例第一次 `write_file` 时进入，避免前端第二个文件再次等待已经结束的后端 worker：

```rust
struct ParallelProbe {
    active: AtomicUsize,
    max_active: AtomicUsize,
    entered: Barrier,
    uploads_finished: AtomicUsize,
    commit_started: AtomicBool,
}

impl ParallelProbe {
    fn new(worker_count: usize) -> Self {
        Self {
            active: AtomicUsize::new(0),
            max_active: AtomicUsize::new(0),
            entered: Barrier::new(worker_count),
            uploads_finished: AtomicUsize::new(0),
            commit_started: AtomicBool::new(false),
        }
    }
}
```

fake `write_file` 不得在持有共享节点树的 `MutexGuard` 时等待 barrier。先读取本地文件并完成一次性 barrier，再短暂加锁写节点：

```rust
if !self.entered_first_write {
    self.entered_first_write = true;
    let active = self.probe.active.fetch_add(1, Ordering::AcqRel) + 1;
    self.probe.max_active.fetch_max(active, Ordering::AcqRel);
    self.probe.entered.wait();
    self.probe.active.fetch_sub(1, Ordering::AcqRel);
    self.probe.uploads_finished.fetch_add(1, Ordering::AcqRel);
}
let content = fs::read(local_path).map_err(DeployError::local_io)?;
progress(content.len() as u64);
self.nodes
    .lock()
    .unwrap()
    .insert(remote_path.to_owned(), Node::File(content));
```

fake `rename` 在第一次正式提交前记录边界：

```rust
if target == "/srv/app/web" || target == "/srv/app/app.jar" {
    assert_eq!(self.probe.uploads_finished.load(Ordering::Acquire), 2);
    self.probe.commit_started.store(true, Ordering::Release);
}
```

增加重叠测试：

```rust
#[test]
fn parallel_deploy_overlaps_two_target_uploads_and_commits_serially() {
    let (root, request) = two_target_request();
    let probe = Arc::new(ParallelProbe::new(2));
    let remotes = vec![
        Box::new(SharedFakeRemoteFs::with_existing_release(probe.clone())) as Box<dyn RemoteFs>,
        Box::new(SharedFakeRemoteFs::with_existing_release(probe.clone())) as Box<dyn RemoteFs>,
    ];
    let user_cancelled = Arc::new(AtomicBool::new(false));
    let stop_requested = Arc::new(AtomicBool::new(false));

    let plan = super::DeploymentPlan::new(request).unwrap();
    super::deploy_parallel(
        remotes,
        plan,
        user_cancelled,
        stop_requested,
        Arc::new(|_, _| {}),
    )
    .unwrap();
    drop(root);

    assert_eq!(probe.max_active.load(Ordering::Acquire), 2);
    assert!(probe.commit_started.load(Ordering::Acquire));
}
```

失败测试使用单独的 `FailureProbe`，不复用双向 barrier：失败目标先等待另一个 worker 把 `sibling_started` 置为 true，再返回 `DeployError::failed("injected upload failure")`；另一个 worker 循环检查 `stop_requested` 并设置 `sibling_stopped`。测试断言 `sibling_stopped == true`、正式旧文件仍存在，且每个 temp 路径要么已删除、要么明确出现在 `error.recovery_paths`。这样失败 worker 不会在 sibling 尚未进入 barrier 时造成死锁。

- [ ] **Step 2: 运行并发测试，确认 RED**

运行：

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml parallel_deploy_overlaps_two_target_uploads_and_commits_serially -- --nocapture
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml parallel_deploy_stops_sibling_after_upload_failure -- --nocapture
```

预期：编译失败，`deploy_parallel` 和共享 fake 尚不存在。

- [ ] **Step 3: 实现 `deploy_parallel` 的固定签名和 worker 回收**

`DeploymentPlan` 必须由 runtime 在连接前创建，再传给协调器；这样本地产物变化或路径校验失败不会先建立 SSH 会话。固定签名为：

```rust
pub(crate) fn deploy_parallel(
    remotes: Vec<Box<dyn RemoteFs>>,
    plan: DeploymentPlan,
    user_cancelled: Arc<AtomicBool>,
    stop_requested: Arc<AtomicBool>,
    progress: Arc<dyn Fn(u64, &str) + Send + Sync>,
) -> Result<(), DeployError> {
    if remotes.len() != plan.target_count()
        || remotes.is_empty()
        || remotes.len() > 2
    {
        return Err(DeployError::failed("SSH 会话数量与部署目标不一致"));
    }
    let mut remotes = remotes;
    let mut control = remotes.remove(0);
    if let Err(mut error) = plan.prepare_remote(control.as_mut()) {
        error.recovery_paths.extend(plan.cleanup_temps(control.as_mut()));
        error.recovery_paths.sort();
        error.recovery_paths.dedup();
        return Err(error);
    }

    if plan.target_count() == 1 {
        let mut plan = plan;
        let upload = plan
            .upload_target(0, control.as_mut(), &stop_requested, &mut |bytes, path| {
                progress(bytes, path);
            })
            .and_then(|()| plan.verify_target(0, control.as_ref()))
            .and_then(|()| plan.validate_formal_targets(control.as_ref()));
        if let Err(mut error) = upload {
            error.recovery_paths.extend(plan.cleanup_temps(control.as_mut()));
            error.recovery_paths.sort();
            error.recovery_paths.dedup();
            return Err(error);
        }
        return plan.commit(control.as_mut(), user_cancelled.as_ref());
    }

    let plan = Arc::new(plan);
    let mut workers = vec![control];
    workers.extend(remotes);
    let mut handles = Vec::with_capacity(workers.len());
    for (index, mut remote) in workers.into_iter().enumerate() {
        let plan = Arc::clone(&plan);
        let stop_requested = Arc::clone(&stop_requested);
        let user_cancelled = Arc::clone(&user_cancelled);
        let progress = Arc::clone(&progress);
        handles.push(thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                plan.upload_target(
                    index,
                    remote.as_mut(),
                    &stop_requested,
                    &mut |bytes, path| progress(bytes, path),
                )
                .and_then(|()| plan.verify_target(index, remote.as_ref()))
            }))
            .unwrap_or_else(|_| Err(DeployError::failed("远端上传工作线程异常退出")));
            if result.is_err() && !user_cancelled.load(Ordering::Acquire) {
                stop_requested.store(true, Ordering::Release);
            }
            (index, remote, result)
        }));
    }

    let mut returned_remotes = Vec::with_capacity(handles.len());
    let mut primary_error = None;
    for handle in handles {
        match handle.join() {
            Ok((_index, remote, result)) => {
                returned_remotes.push(remote);
                if let Err(error) = result {
                    if !error.cancelled && primary_error.is_none() {
                        primary_error = Some(error);
                    }
                }
            }
            Err(_) => {
                if primary_error.is_none() {
                    primary_error = Some(DeployError::failed("远端上传工作线程异常退出"));
                }
            }
        }
    }

    let mut plan = match Arc::try_unwrap(plan) {
        Ok(plan) => plan,
        Err(plan) => {
            let mut error = DeployError::failed("远端上传计划仍被工作线程占用");
            if let Some(control) = returned_remotes.first_mut() {
                error.recovery_paths.extend(plan.cleanup_temps(control.as_mut()));
            } else {
                error.recovery_paths.extend(plan.temp_paths());
            }
            error.recovery_paths.sort();
            error.recovery_paths.dedup();
            return Err(error);
        }
    };
    let Some(mut control) = returned_remotes.into_iter().next() else {
        let mut error = primary_error
            .unwrap_or_else(|| DeployError::failed("没有可用于清理远端临时目标的 SSH 会话"));
        error.recovery_paths.extend(plan.temp_paths());
        error.recovery_paths.sort();
        error.recovery_paths.dedup();
        return Err(error);
    };
    if let Some(mut error) = primary_error {
        error.recovery_paths.extend(plan.cleanup_temps(control.as_mut()));
        error.recovery_paths.sort();
        error.recovery_paths.dedup();
        return Err(error);
    }
    if user_cancelled.load(Ordering::Acquire) {
        return plan.cancel_and_cleanup(control.as_mut());
    }
    if stop_requested.load(Ordering::Acquire) {
        let mut error = DeployError::failed("并行上传因其他目标失败而停止");
        error.recovery_paths.extend(plan.cleanup_temps(control.as_mut()));
        error.recovery_paths.sort();
        error.recovery_paths.dedup();
        return Err(error);
    }
    if let Err(mut error) = plan.validate_formal_targets(control.as_ref()) {
        error.recovery_paths.extend(plan.cleanup_temps(control.as_mut()));
        error.recovery_paths.sort();
        error.recovery_paths.dedup();
        return Err(error);
    }
    plan.commit(control.as_mut(), user_cancelled.as_ref())
}
```

`catch_unwind` 必须放在线程闭包内部，使 panic 后仍能把该 worker 的 remote 返回协调器。worker 只读 `Arc<DeploymentPlan>`；全部 `join` 后用 `Arc::try_unwrap` 取回可变计划，再串行提交。内部 stop 只设置 `stop_requested`，不设置 `user_cancelled`；若只有内部 stop 产生的取消错误而没有主业务错误，返回明确的 `DeployError::failed("并行上传因其他目标失败而停止")`。

现有 `deploy` wrapper 继续走 Task 2 的单会话 plan 方法，作为串行回归入口；不要为了复用 `deploy_parallel` 把借用的 remote 强行装箱或引入第二套事务逻辑。

- [ ] **Step 4: 实现失败/取消清理和提交边界**

将以下规则编码在 `DeploymentPlan` 方法中：

```rust
pub(crate) fn cancel_and_cleanup(
    &self,
    remote: &mut dyn RemoteFs,
) -> Result<(), DeployError> {
    let mut error = DeployError::cancelled();
    error.recovery_paths.extend(self.cleanup_temps(remote));
    error.recovery_paths.sort();
    error.recovery_paths.dedup();
    Err(error)
}
```

`commit` 在每个 backup/temp rename 前检查 `user_cancelled`；提交点之后沿用流程一致性修复后的结构化 `DeployError.committed` 警告结果。不要让并行 worker 直接调用 rename、remove_tree 或正式目标校验。

再增加 `parallel_worker_panic_returns_remote_and_cleans_temps`：共享 fake 在指定 worker 的 `write_file` 内 panic，断言错误包含“工作线程异常退出”，正式目标未变，temp 已删除或列入 recovery paths。该测试锁定 panic 后 remote 可回收的要求。

- [ ] **Step 5: 运行部署全量回归，确认 GREEN**

运行：

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package_deploy -- --nocapture
```

预期：并发 barrier、失败清理、串行提交、回滚、空目录、路径校验和原有 fake 事务测试全部通过。

- [ ] **Step 6: 提交本任务**

```powershell
git add -- apps/desktop/src-tauri/src/tools/release_package_deploy.rs
git commit -m "feat(release-package): 并行上传独立目标"
```

## Task 5: 接入 runtime 连接编排和进度节流

**Files:**
- Modify: `apps/desktop/src-tauri/src/tools/release_package_runtime.rs:221-340,822-930,1287-1478,1980-2030`
- Test: `apps/desktop/src-tauri/src/tools/release_package_runtime.rs` 的 `pipeline_tests`

- [ ] **Step 1: 写进度 gate 的失败测试**

增加纯函数状态和测试，使用传入的 `Instant`，不在测试中 `sleep`：

```rust
#[derive(Default)]
struct UploadProgressState {
    uploaded_bytes: u64,
    last_emitted_at: Option<Instant>,
    current_path: Option<String>,
}

fn should_emit_upload_progress(
    last_emitted_at: Option<Instant>,
    now: Instant,
    force: bool,
) -> bool {
    force || last_emitted_at
        .map(|last| now.duration_since(last) >= Duration::from_millis(100))
        .unwrap_or(true)
}
```

测试：

```rust
#[test]
fn upload_progress_is_initially_and_finally_forced_but_middle_is_throttled() {
    let start = Instant::now();
    assert!(should_emit_upload_progress(None, start, false));
    assert!(!should_emit_upload_progress(
        Some(start),
        start + Duration::from_millis(99),
        false,
    ));
    assert!(should_emit_upload_progress(
        Some(start),
        start + Duration::from_millis(100),
        false,
    ));
    assert!(should_emit_upload_progress(
        Some(start),
        start + Duration::from_millis(1),
        true,
    ));
}
```

再增加确定性的 reporter 累计测试，直接调用模块内部的 `report_at`/`force_emit_at`，不通过 `sleep` 猜时序：

```rust
#[test]
fn upload_progress_aggregates_concurrent_bytes_without_bypassing_throttle() {
    let sink = Arc::new(TerminalSink::default());
    let reporter = Arc::new(UploadProgressReporter::new(
        sink.clone(),
        "run-progress",
        7,
        200,
    ));
    let start = Instant::now();
    reporter.force_emit_at(start, false);

    let mut handles = Vec::new();
    for path in ["index.html", "app.jar"] {
        let reporter = Arc::clone(&reporter);
        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                reporter.report_at(1, path, start + Duration::from_millis(50));
            }
        }));
    }
    for handle in handles {
        handle.join().unwrap();
    }
    reporter.force_emit_at(start + Duration::from_millis(100), true);

    let statuses = sink.statuses.lock().unwrap();
    assert_eq!(statuses.len(), 2);
    assert_eq!(statuses[0].uploaded_bytes, Some(0));
    assert_eq!(statuses[1].uploaded_bytes, Some(200));
    assert_eq!(reporter.uploaded_bytes(), 200);
}
```

- [ ] **Step 2: 运行进度测试，确认 RED**

运行：

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml upload_progress_is_initially_and_finally_forced_but_middle_is_throttled -- --nocapture
```

预期：编译失败，因为当前 runtime 没有 gate/reporter。

- [ ] **Step 3: 实现线程安全 `UploadProgressReporter`**

在 `emit_upload_status` 后增加：

```rust
struct UploadProgressReporter {
    sink: Arc<dyn EventSink>,
    run_id: String,
    project_id: i64,
    total_bytes: u64,
    state: Mutex<UploadProgressState>,
}

impl UploadProgressReporter {
    fn new(
        sink: Arc<dyn EventSink>,
        run_id: impl Into<String>,
        project_id: i64,
        total_bytes: u64,
    ) -> Self {
        Self {
            sink,
            run_id: run_id.into(),
            project_id,
            total_bytes,
            state: Mutex::new(UploadProgressState::default()),
        }
    }

    fn report(&self, bytes: u64, path: &str) {
        self.report_at(bytes, path, Instant::now());
    }

    fn report_at(&self, bytes: u64, path: &str, now: Instant) {
        let mut state = self.state.lock().unwrap();
        state.uploaded_bytes = state.uploaded_bytes.saturating_add(bytes);
        state.current_path = Some(path.to_owned());
        if !should_emit_upload_progress(state.last_emitted_at, now, false) {
            return;
        }
        state.last_emitted_at = Some(now);
        emit_upload_status(
            self.sink.as_ref(),
            &self.run_id,
            self.project_id,
            state.uploaded_bytes,
            self.total_bytes,
            state.current_path.clone(),
        );
    }

    fn force_emit(&self, success: bool) {
        self.force_emit_at(Instant::now(), success);
    }

    fn force_emit_at(&self, now: Instant, success: bool) {
        let mut state = self.state.lock().unwrap();
        if success {
            debug_assert_eq!(state.uploaded_bytes, self.total_bytes);
            state.uploaded_bytes = self.total_bytes;
        }
        state.last_emitted_at = Some(now);
        emit_upload_status(
            self.sink.as_ref(),
            &self.run_id,
            self.project_id,
            state.uploaded_bytes,
            self.total_bytes,
            state.current_path.clone(),
        );
    }

    fn uploaded_bytes(&self) -> u64 {
        self.state.lock().unwrap().uploaded_bytes
    }
}
```

`force_emit(false)` 在上传开始前和失败/取消返回后调用；只有 `deploy_parallel` 已完成临时目标校验并成功提交时才调用 `force_emit(true)`。成功时远端校验已证明字节完整，因此把最终事件规范为 `total_bytes` 不会掩盖传输错误；`debug_assert_eq!` 和并发累计测试用于暴露 reporter 自身的漏计数。

- [ ] **Step 4: 把 runtime 连接和 worker 编排接入执行路径**

将 `execute_deployment_request` 改为接收 `Arc<dyn EventSink>`、`Arc<AtomicBool>` 的用户取消标记、`&Arc<SshSocketRegistry>` 和 `&Arc<AtomicBool>` 的 `upload_stop`。先创建本地计划，再按计划目标数建立会话，保证本地校验失败时没有 SSH 连接：

```rust
let reporter = Arc::new(UploadProgressReporter::new(
    sink.clone(),
    run_id,
    project_id,
    total_bytes,
));
reporter.force_emit(false);
let progress = {
    let reporter = Arc::clone(&reporter);
    Arc::new(move |bytes: u64, path: &str| reporter.report(bytes, path))
        as Arc<dyn Fn(u64, &str) + Send + Sync>
};
let deploy_result = (|| -> Result<(), DeployError> {
    let plan = DeploymentPlan::new(request)?;
    let mut remotes: Vec<Box<dyn RemoteFs>> = Vec::with_capacity(plan.target_count());
    for _ in 0..plan.target_count() {
        remotes.push(Box::new(SftpRemoteFs::connect(
            &consumed.binding,
            &consumed.expected_fingerprint,
            &consumed.secret,
            ssh_sockets,
        )?));
    }
    deploy_parallel(
        remotes,
        plan,
        Arc::clone(&cancelled),
        Arc::clone(upload_stop),
        progress,
    )
})();
ssh_sockets.clear();
reporter.force_emit(deploy_result.is_ok());
```

单目标也通过 `deploy_parallel` 传一个 remote，但函数内部不启动 worker。第二个会话失败时闭包直接返回，`remotes` 被 drop，且尚未调用 `prepare_remote`；随后统一 `ssh_sockets.clear()`。成功日志仍只在 `summary.remote_committed` 后发送。

更新 `run_deployment_phase`、`run_retry_deployment_phase`、`start` 和 `upload_retry` 的参数传递；`ActiveRun` 同步保存 `ssh_sockets` 与 `upload_stop`。`start`/`upload_retry` 把 `Arc<dyn EventSink>` 和 `Arc<AtomicBool>` 传入部署函数，不改 `StatusEvent` 字段。

- [ ] **Step 5: 更新取消和运行态测试**

把现有 `ActiveRun` 测试结构体字面量补齐新字段，并增加：

```rust
#[test]
fn internal_upload_stop_does_not_mark_user_cancelled() {
    let cancelled = AtomicBool::new(false);
    let upload_stop = AtomicBool::new(true);
    let finished = AtomicBool::new(false);
    let cancel_won = AtomicBool::new(false);
    let summary = PipelineSummary {
        status: "succeeded",
        archive_path: None,
        archived_targets: Vec::new(),
        manifests: Vec::new(),
        error: None,
        retry_descriptor: None,
        remote_committed: false,
    };
    let result = claim_pipeline_result(
        Ok(combine_package_and_deploy(
            summary,
            Err(DeployError::failed("并行上传因其他目标失败而停止")),
        )),
        &cancelled,
        &finished,
        &cancel_won,
        &Mutex::new(()),
    )
    .unwrap();

    assert_eq!(result.status, "package_succeeded_upload_failed");
    assert!(!cancelled.load(Ordering::Acquire));
    assert!(upload_stop.load(Ordering::Acquire));
}
```

更新 `cancellation_closes_the_active_ssh_socket` 为双 socket registry 测试，并断言 `request_cancel` 同时设置 `cancelled`、`upload_stop`、关闭全部连接；保留 `late_cancellation_does_not_override_a_committed_upload`。

- [ ] **Step 6: 运行 runtime 与 Rust 回归，确认 GREEN**

运行：

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package_runtime -- --nocapture
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package -- --nocapture
```

预期：进度节流、内部停止、全量取消、重试描述符、提交后迟到取消和原有构建/归档测试全部通过。

- [ ] **Step 7: 提交本任务**

```powershell
git add -- apps/desktop/src-tauri/src/tools/release_package_runtime.rs
git commit -m "perf(release-package): 聚合并节流上传进度"
```

## Task 6: 补充真实 SSH fixture、经验文档和最终验证

**Files:**
- Modify: `apps/desktop/src-tauri/src/tools/release_package_remote.rs` 的 ignored fixture 测试
- Modify: `docs/experience/release-package.md`
- Test: Rust release-package 全量测试和可用时的 loopback SSH fixture

- [ ] **Step 1: 写 ignored 双会话 fixture 测试**

在现有 `password_and_private_key_upload_to_local_fixture` 附近增加 `parallel_targets_upload_to_local_fixture`，继续只接受 loopback host 和 `LAZYCAT_SSH_TEST_*` 环境变量。复用现有 `LocalFixtureDir`、`deployment_request` 和 `SshTestFixture::binding`，先建 plan、再建立两个会话；认证秘密只创建一次并由协调器连续借用：

```rust
#[test]
#[ignore = "requires LAZYCAT_SSH_TEST_* variables and a loopback SSH fixture"]
fn parallel_targets_upload_to_local_fixture() {
    let fixture = SshTestFixture::from_env().unwrap();
    let probe = probe_host(&fixture.endpoint).unwrap();
    let suffix = Uuid::new_v4().simple().to_string();
    let remote_root = format!("/tmp/lazycat-release-package-parallel-{suffix}");
    let binding = fixture.binding(&remote_root, "password");
    let local = LocalFixtureDir::create().unwrap();
    let request = deployment_request(&local.0, &remote_root, "parallel", false, false).unwrap();
    let request_total_bytes = request
        .targets
        .iter()
        .map(|target| target.manifest.total_bytes)
        .sum::<u64>();
    let plan = DeploymentPlan::new(request).unwrap();
    let secret = fixture.password_auth();

    let sockets = SshSocketRegistry::new();
    let mut remotes = Vec::new();
    for _ in 0..plan.target_count() {
        remotes.push(Box::new(
            SftpRemoteFs::connect(
                &binding,
                &probe.fingerprint_sha256,
                &secret,
                &sockets,
            )
            .unwrap(),
        ) as Box<dyn RemoteFs>);
    }
    let uploaded = Arc::new(AtomicU64::new(0));
    let uploaded_for_progress = Arc::clone(&uploaded);
    let deploy_result = deploy_parallel(
        remotes,
        plan,
        Arc::new(AtomicBool::new(false)),
        Arc::new(AtomicBool::new(false)),
        Arc::new(move |bytes, _| {
            uploaded_for_progress.fetch_add(bytes, Ordering::AcqRel);
        }),
    );
    sockets.clear();

    let cleanup_sockets = SshSocketRegistry::new();
    let mut cleanup_remote = SftpRemoteFs::connect(
        &binding,
        &probe.fingerprint_sha256,
        &secret,
        &cleanup_sockets,
    )
    .unwrap();
    let cleanup_result = cleanup_remote.remove_tree(&remote_root);
    cleanup_sockets.clear();

    deploy_result.unwrap();
    cleanup_result.unwrap();
    assert_eq!(uploaded.load(Ordering::Acquire), request_total_bytes);
}
```

同步补充测试模块 imports：`AtomicU64`、`Ordering`、`deploy_parallel`、`DeploymentPlan` 和 `SshSocketRegistry`。清理使用独立的第三个会话，只发生在两个上传 worker 已结束之后，不计入活动上传会话上限。测试不得打印密码、私钥口令或完整认证结构。

- [ ] **Step 2: 运行真实 fixture（可用时）或明确记录跳过**

有受控 loopback SSH 服务和环境变量时运行：

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml parallel_targets_upload_to_local_fixture -- --ignored --nocapture
```

无 fixture 时不要加 `--ignored` 强行执行：全量 `release_package` 测试会把该用例报告为 ignored，交付记录写明“真实 SSH/SFTP 并行冒烟未执行”。有 fixture 时记录同一产物串行与并行上传阶段耗时作为观测值，但不设置不稳定的速度断言；不得连接生产服务器。

- [ ] **Step 3: 更新上线包经验**

在 `docs/experience/release-package.md` 的“多目标并行”之后新增固定规则：

```markdown
## SSH 上传采用目标级有界并发

服务器上传最多为每个选中目标建立一个独立 SSH/SFTP 会话，双目标并发上限为 2。只在线程中共享同一 `ssh2::Session` 不会提升阻塞 SFTP 吞吐，因为会话内部有互斥锁。

正式上传前先完成本地产物校验、全部会话建立、temp/backup 冲突检查和前端目录预创建；worker 只写各自 temp 并校验。正式目标的状态复核、backup、commit、rollback 和清理继续串行。

运行级 socket 必须集中注册，取消时关闭全部连接。内部 worker 失败使用独立停止标记，不能伪报用户取消。上传进度按时间节流，成功终态仍须精确累计全部字节。

前端文件级并发、单文件分块和并发数配置不属于首轮范围；真实收益须在受控 SSH fixture 上对比串行基线后再决定。
```

保留文末日期 `2026-07-25`，把当前使用次数从 `2` 增加到 `3`；不修改 `AGENTS.md` 或 `CLAUDE.md`。

- [ ] **Step 4: 执行最终验证**

按顺序运行：

```powershell
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package -- --nocapture
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml
pnpm typecheck
git diff --check
git status --short
```

预期：Rust release-package 测试无失败；无 fixture 时只有明确的 ignored 测试；类型检查和 cargo check 退出码为 0；`git diff --check` 无输出；工作区只包含本任务预期文件。

- [ ] **Step 5: 提交文档和最终验证**

```powershell
git add -- apps/desktop/src-tauri/src/tools/release_package_remote.rs docs/experience/release-package.md
git commit -m "docs(release-package): 记录 SSH 并行上传边界"
```

## 完成检查清单

- [ ] 双目标测试证明两个 worker 在同一时间进入写入阶段，活动 SSH 会话不超过 2 个。
- [ ] 单目标路径只建立一个 SSH 会话且不启动无效 worker。
- [ ] 所有会话建立前不会创建正式上传 temp 路径。
- [ ] 前端目录只预创建一次，上传循环不再重复 stat 父目录。
- [ ] 进度事件首帧、节流中间帧和最终帧行为有单元测试。
- [ ] worker 失败、panic、用户取消和提交后迟到取消均有明确测试。
- [ ] 正式目标只由串行提交阶段修改，失败仍可回滚并返回 recovery paths。
- [ ] 密码和私钥口令不进入 worker、日志、通知、配置或重试描述。
- [ ] 现有流程一致性修复回归测试与本计划测试共同通过。
- [ ] 真实 SSH fixture 可用时完成双会话冒烟；不可用时明确记录跳过。
