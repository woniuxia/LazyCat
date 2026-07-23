# Release Package Windows Rename Retry Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让 Windows 10 上线包完整替换事务能够容忍目录名短暂不可复用导致的 `os error 5`，同时保留失败回滚和显式错误。

**Architecture:** 仅修改 `release_package_archive.rs`，增加一个 Windows 错误 5 专用的有限退避重命名原语，并将旧目录备份、新目录提交、失败回滚三处 `fs::rename` 统一接入。重试核心注入单次 rename 与 sleep 操作以便确定性测试；归档事务、路径模型和备份清理语义保持不变。

**Tech Stack:** Rust 2021、`std::fs`、`std::io`、`std::thread`、内联单元测试、Cargo test

---

## 文件结构

- Modify and test: `apps/desktop/src-tauri/src/tools/release_package_archive.rs`
  - 定义 Windows 错误 5 分类、退避时长、可测试重试核心和生产包装函数。
  - 将 `ArchiveSession::commit` 的三个事务重命名接入重试，并保持回滚优先于取消终态。
  - 在现有 `#[cfg(test)]` 模块加入重试、非重试、上限和取消测试。

不创建新源码模块：重试逻辑只服务于归档事务，放在同一模块可保持 API 私有且避免额外分层。

### Task 1: 用失败测试定义有限重命名重试行为

**Files:**
- Modify: `apps/desktop/src-tauri/src/tools/release_package_archive.rs:600-640`
- Test: `apps/desktop/src-tauri/src/tools/release_package_archive.rs` 内联 `tests` 模块

- [ ] **Step 1: 写入重试行为测试**

在 `tests` 模块末尾加入以下测试；它们先引用尚不存在的 `rename_with_retry_using` 和 `RenameFailure`：

```rust
    #[cfg(windows)]
    #[test]
    fn access_denied_rename_retries_until_success() {
        let cancelled = AtomicBool::new(false);
        let mut attempts = 0;
        let mut sleeps = 0;

        let result = rename_with_retry_using(
            Path::new("source"),
            Path::new("target"),
            Some(&cancelled),
            &[Duration::ZERO, Duration::ZERO],
            |_, _| {
                attempts += 1;
                if attempts < 3 {
                    Err(std::io::Error::from_raw_os_error(5))
                } else {
                    Ok(())
                }
            },
            |_| sleeps += 1,
        );

        assert!(result.is_ok());
        assert_eq!(attempts, 3);
        assert_eq!(sleeps, 2);
    }

    #[test]
    fn non_access_denied_rename_error_is_not_retried() {
        let mut attempts = 0;

        let result = rename_with_retry_using(
            Path::new("source"),
            Path::new("target"),
            None,
            &[Duration::ZERO],
            |_, _| {
                attempts += 1;
                Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "missing source",
                ))
            },
            |_| panic!("non-retryable error must not sleep"),
        );

        assert!(matches!(result, Err(RenameFailure::Io(_))));
        assert_eq!(attempts, 1);
    }

    #[cfg(windows)]
    #[test]
    fn access_denied_rename_stops_after_retry_budget() {
        let mut attempts = 0;

        let result = rename_with_retry_using(
            Path::new("source"),
            Path::new("target"),
            None,
            &[Duration::ZERO, Duration::ZERO],
            |_, _| {
                attempts += 1;
                Err(std::io::Error::from_raw_os_error(5))
            },
            |_| {},
        );

        assert!(matches!(result, Err(RenameFailure::Io(error)) if error.raw_os_error() == Some(5)));
        assert_eq!(attempts, 3);
    }

    #[cfg(windows)]
    #[test]
    fn access_denied_rename_honors_cancellation_before_waiting() {
        let cancelled = AtomicBool::new(true);
        let mut attempts = 0;

        let result = rename_with_retry_using(
            Path::new("source"),
            Path::new("target"),
            Some(&cancelled),
            &[Duration::ZERO],
            |_, _| {
                attempts += 1;
                Err(std::io::Error::from_raw_os_error(5))
            },
            |_| panic!("cancelled retry must not sleep"),
        );

        assert!(matches!(result, Err(RenameFailure::Cancelled)));
        assert_eq!(attempts, 1);
    }
```

- [ ] **Step 2: 运行测试并确认 RED**

Run:

```powershell
cargo test rename_ -- --nocapture
```

Working directory: `apps/desktop/src-tauri`

Expected: 编译失败，明确提示找不到 `rename_with_retry_using`、`RenameFailure` 或测试所需的 `Duration`，证明测试在生产实现前确实为红。

- [ ] **Step 3: 实现最小重试原语**

将导入调整为：

```rust
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::time::Duration;
```

在 `check_cancel` 之后加入：

```rust
#[derive(Debug)]
enum RenameFailure {
    Cancelled,
    Io(io::Error),
}

#[cfg(windows)]
fn is_retryable_rename_error(error: &io::Error) -> bool {
    error.raw_os_error() == Some(5)
}

#[cfg(not(windows))]
fn is_retryable_rename_error(_error: &io::Error) -> bool {
    false
}

fn rename_with_retry_using<R, S>(
    source: &Path,
    target: &Path,
    cancelled: Option<&AtomicBool>,
    retry_delays: &[Duration],
    mut rename: R,
    mut sleep: S,
) -> Result<(), RenameFailure>
where
    R: FnMut(&Path, &Path) -> io::Result<()>,
    S: FnMut(Duration),
{
    let mut retry_delays = retry_delays.iter().copied();
    loop {
        match rename(source, target) {
            Ok(()) => return Ok(()),
            Err(error) if is_retryable_rename_error(&error) => {
                let Some(delay) = retry_delays.next() else {
                    return Err(RenameFailure::Io(error));
                };
                if cancelled.is_some_and(|flag| flag.load(Ordering::Acquire)) {
                    return Err(RenameFailure::Cancelled);
                }
                sleep(delay);
            }
            Err(error) => return Err(RenameFailure::Io(error)),
        }
    }
}
```

在测试模块导入中加入：

```rust
    use std::time::Duration;
```

- [ ] **Step 4: 运行重试单元测试并确认 GREEN**

Run:

```powershell
cargo test rename_ -- --nocapture
```

Expected: `access_denied_rename_retries_until_success`、`non_access_denied_rename_error_is_not_retried`、`access_denied_rename_stops_after_retry_budget`、`access_denied_rename_honors_cancellation_before_waiting` 全部通过；无 panic 或 warning。

- [ ] **Step 5: 提交重试原语与单元测试**

```powershell
git add apps/desktop/src-tauri/src/tools/release_package_archive.rs
git commit -m "fix(release-package): 增加 Windows 重命名有限重试"
```

### Task 2: 将归档替换事务接入重试并保持回滚语义

**Files:**
- Modify: `apps/desktop/src-tauri/src/tools/release_package_archive.rs:127-176`
- Test: `apps/desktop/src-tauri/src/tools/release_package_archive.rs:723-766`

- [ ] **Step 1: 补充取消后必须恢复旧目录的失败测试**

为保证提交重试被取消时不会跳过回滚，在 `ArchiveSession` 测试附近加入一个可注入事务提交测试接口的测试。先将生产方法调用改为尚不存在的 `commit_with_rename`，测试代码如下：

```rust
    #[cfg(windows)]
    #[test]
    fn cancelled_commit_retry_restores_existing_directory() {
        let root = TestDir::new();
        let output = root.0.join("output");
        let final_path = output.join("release");
        fs::create_dir_all(&final_path).unwrap();
        fs::write(final_path.join("old.txt"), "old").unwrap();
        let cancelled = AtomicBool::new(false);
        let mut session =
            ArchiveSession::create(&output, "release", "run-cancel-retry", true, &cancelled)
                .unwrap();
        fs::write(session.staging_path().join("new.txt"), "new").unwrap();
        let mut rename_count = 0;

        let result = session.commit_with_rename(&cancelled, |source, target, retry_cancelled| {
            rename_count += 1;
            if rename_count == 1 {
                fs::rename(source, target).map_err(RenameFailure::Io)
            } else if rename_count == 2 {
                cancelled.store(true, Ordering::Release);
                Err(RenameFailure::Cancelled)
            } else {
                rename_with_retry(source, target, retry_cancelled)
            }
        });

        assert!(matches!(result, Err(ArchiveError::Cancelled)));
        assert_eq!(fs::read_to_string(final_path.join("old.txt")).unwrap(), "old");
        assert!(!final_path.join("new.txt").exists());
    }
```

- [ ] **Step 2: 运行测试并确认 RED**

Run:

```powershell
cargo test cancelled_commit_retry_restores_existing_directory -- --nocapture
```

Expected: 编译失败，提示 `ArchiveSession` 尚无 `commit_with_rename` 方法。

- [ ] **Step 3: 提取可测试提交核心并接入三处重命名**

先增加生产退避参数和包装函数。将 `std::thread` 加入顶部导入，并在 `rename_with_retry_using` 之后加入：

```rust
const RENAME_RETRY_DELAYS: [Duration; 5] = [
    Duration::from_millis(50),
    Duration::from_millis(100),
    Duration::from_millis(200),
    Duration::from_millis(400),
    Duration::from_millis(800),
];

fn rename_with_retry(
    source: &Path,
    target: &Path,
    cancelled: Option<&AtomicBool>,
) -> Result<(), RenameFailure> {
    rename_with_retry_using(
        source,
        target,
        cancelled,
        &RENAME_RETRY_DELAYS,
        |source, target| fs::rename(source, target),
        |delay| thread::sleep(delay),
    )
}
```

将 `ArchiveSession::commit` 保留为生产入口，并新增私有泛型核心：

```rust
    pub fn commit(&mut self, cancelled: &AtomicBool) -> Result<PathBuf, ArchiveError> {
        self.commit_with_rename(cancelled, rename_with_retry)
    }

    fn commit_with_rename<R>(
        &mut self,
        cancelled: &AtomicBool,
        mut rename: R,
    ) -> Result<PathBuf, ArchiveError>
    where
        R: FnMut(&Path, &Path, Option<&AtomicBool>) -> Result<(), RenameFailure>,
    {
        check_cancel(cancelled)?;
        let mut backup_created = false;
        if self.final_path.exists() {
            if !self.final_path.is_dir() {
                return Err(ArchiveError::Failed(
                    "目标归档路径已存在且不是文件夹".into(),
                ));
            }
            if !self.overwrite_existing {
                return Err(ArchiveError::Failed("目标归档目录在执行期间被创建".into()));
            }
            if self.backup_path.exists() {
                return Err(ArchiveError::Failed("本次运行备份目录已存在".into()));
            }
            match rename(&self.final_path, &self.backup_path, Some(cancelled)) {
                Ok(()) => backup_created = true,
                Err(RenameFailure::Cancelled) => return Err(ArchiveError::Cancelled),
                Err(RenameFailure::Io(error)) => {
                    return Err(io_error(
                        "备份已有归档目录",
                        &self.final_path,
                        &self.backup_path,
                        error,
                    ));
                }
            }
        }

        let commit_error = match rename(&self.staging_path, &self.final_path, Some(cancelled)) {
            Ok(()) => None,
            Err(error) => Some(error),
        };
        if let Some(commit_error) = commit_error {
            let was_cancelled = matches!(commit_error, RenameFailure::Cancelled);
            let mut message = match commit_error {
                RenameFailure::Cancelled => "提交最终归档目录已取消".to_string(),
                RenameFailure::Io(error) => format!(
                    "提交最终归档目录失败（源：{}，目标：{}）：{error}",
                    self.staging_path.display(),
                    self.final_path.display()
                ),
            };
            if backup_created {
                if let Err(rollback_error) = rename(&self.backup_path, &self.final_path, None) {
                    let rollback_error = match rollback_error {
                        RenameFailure::Cancelled => "回滚不应被取消".to_string(),
                        RenameFailure::Io(error) => error.to_string(),
                    };
                    message.push_str(&format!(
                        "；恢复原归档目录失败（源：{}，目标：{}）：{rollback_error}",
                        self.backup_path.display(),
                        self.final_path.display()
                    ));
                    return Err(ArchiveError::Failed(message));
                }
            }
            return if was_cancelled {
                Err(ArchiveError::Cancelled)
            } else {
                Err(ArchiveError::Failed(message))
            };
        }

        self.committed = true;
        if backup_created {
            fs::remove_dir_all(&self.backup_path).map_err(|error| {
                io_error("清理旧归档备份", &self.backup_path, &self.final_path, error)
            })?;
        }
        Ok(self.final_path.clone())
    }
```

关键约束：前向重命名传 `Some(cancelled)`；回滚传 `None`，确保任务取消不会阻止旧目录恢复。只有回滚失败时，取消才升级为包含恢复失败信息的 `ArchiveError::Failed`。

- [ ] **Step 4: 运行定向测试并确认 GREEN**

Run:

```powershell
cargo test cancelled_commit_retry_restores_existing_directory -- --nocapture
cargo test overwrite_ -- --nocapture
cargo test rename_ -- --nocapture
```

Expected: 新增取消回滚测试通过；现有完整替换、提交失败恢复、文件目标拒绝测试通过；Task 1 的重试测试继续通过。

- [ ] **Step 5: 提交事务接入改动**

```powershell
git add apps/desktop/src-tauri/src/tools/release_package_archive.rs
git commit -m "fix(release-package): 重试 Windows 归档目录切换"
```

### Task 3: 完整验证

**Files:**
- Verify: `apps/desktop/src-tauri/src/tools/release_package_archive.rs`
- Verify: `docs/superpowers/specs/2026-07-23-release-package-windows-rename-retry-design.md`

- [ ] **Step 1: 运行上线包 Rust 定向测试**

Run:

```powershell
cargo test release_package -- --nocapture
```

Working directory: `apps/desktop/src-tauri`

Expected: 所有匹配 `release_package` 的 Rust 测试通过，0 failed。

- [ ] **Step 2: 运行 Rust 编译检查**

Run:

```powershell
cargo check
```

Working directory: `apps/desktop/src-tauri`

Expected: exit code 0，无新增编译错误。

- [ ] **Step 3: 检查最终差异**

Run:

```powershell
git diff --check HEAD~2
git status --short
```

Expected: `git diff --check` 无输出；工作区没有未提交文件。最终源码改动仅涉及 `release_package_archive.rs`，文档改动仅涉及本设计与计划文件。
