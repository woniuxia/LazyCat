# 上线包用户流程一致性修复 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修复上线包提交后清理误报、错误不可见、远端目标关系晚失败和预检秘密不及时撤销四个用户流程问题。

**Architecture:** 保留现有公开运行状态和 IPC 结构，在 Rust 内部为“提交成功但清理失败”增加结构化结果；远端目标关系校验在预检和部署两层执行。前端继续以 `useReleasePackageRuntime` 为唯一运行状态源，增加错误摘要渲染；预检 composable 通过新增一次性撤销 action 管理 probe/preflight token 生命周期。

**Tech Stack:** Vue 3 + TypeScript、Tauri IPC、Rust、rusqlite、ssh2、Vitest、Cargo test。

---

## 文件职责与改动边界

- `apps/desktop/src-tauri/src/tools/release_package_archive.rs`：本地归档提交结果及清理警告。
- `apps/desktop/src-tauri/src/tools/release_package_deploy.rs`：远端部署错误的“已提交”标记及清理警告。
- `apps/desktop/src-tauri/src/tools/release_package_runtime.rs`：把提交警告聚合为成功终态，禁止错误生成重试令牌。
- `apps/desktop/src-tauri/src/tools/release_package_remote.rs`：远端目标关系校验和 probe/preflight 令牌撤销存储。
- `apps/desktop/src-tauri/src/tools/release_package.rs`：新增撤销 action 的解析和分发。
- `apps/desktop/src/bridge/tauri.ts`：注册撤销 IPC channel。
- `apps/desktop/src/types/release-package.ts`：补充撤销请求类型（若现有桥接类型需要显式声明）。
- `apps/desktop/src/composables/useReleasePackageUploadPreflight.ts`：统一捕获、撤销和清空令牌。
- `apps/desktop/src/components/ReleasePackagePanel.vue`：渲染整体、目标和上传错误摘要，并在关闭/异常路径等待撤销。
- 对应 Rust/Vitest 测试文件：先锁定每个回归行为，再实现。

## Task 1: 锁定提交成功后的清理警告语义

**Files:**
- Modify: `apps/desktop/src-tauri/src/tools/release_package_archive.rs`
- Modify: `apps/desktop/src-tauri/src/tools/release_package_deploy.rs`
- Modify: `apps/desktop/src-tauri/src/tools/release_package_runtime.rs`
- Test: 上述三个文件内现有 `#[cfg(test)]` 模块

- [ ] **Step 1: 写本地归档失败测试**

在 `release_package_archive.rs` 的归档测试中增加一个可注入 `remove_dir_all` 失败的 commit 测试。先让 staging 和旧 final 都存在，模拟 final→backup、staging→final 均成功，模拟 backup 清理失败；断言 commit 结果包含最终路径和“清理旧归档备份”的警告，而不是普通 `ArchiveError::Failed`。

- [ ] **Step 2: 运行本地归档测试并确认 RED**

运行：

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package_archive -- --nocapture
```

预期：新增测试失败，失败点是当前实现把备份删除错误直接转换为普通失败。

- [ ] **Step 3: 写远端部署和运行时失败测试**

在 `release_package_deploy.rs` 的事务测试中模拟所有正式目标 rename 成功、`remove_tree(backup)` 失败，断言 `DeployError.committed == true` 且 `recovery_paths` 包含备份路径。在 `release_package_runtime.rs` 测试中把该错误传给 `combine_package_and_deploy`，断言状态仍为 `succeeded`、`remote_committed == true`、`retry_descriptor.is_none()`，错误文本包含备份路径。

- [ ] **Step 4: 运行新增测试并确认 RED**

运行：

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package_deploy::transaction_tests -- --nocapture
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package_runtime::pipeline_tests -- --nocapture
```

预期：新增断言失败，当前 `DeployError` 没有提交状态，`combine_package_and_deploy` 会生成上传失败重试描述符。

- [ ] **Step 5: 实现结构化提交结果**

在归档模块新增明确的 `ArchiveError::CommittedWithWarning`（携带最终路径和警告）或等价结构化结果；`ArchiveSession::commit` 在正式目录切换成功后将备份清理错误转换为该结果。不要通过匹配错误字符串判断提交状态。

在 `DeployError` 增加 `committed: bool`，所有提交前错误构造为 `false`，备份清理失败构造为 `true`。更新 `combine_package_and_deploy`：`committed == true` 时保留成功状态、设置 `remote_committed`、合并警告、清空 retry descriptor；其他错误保持原失败/回滚语义。

- [ ] **Step 6: 运行 Task 1 测试并确认 GREEN**

运行：

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package -- --nocapture
```

预期：新增测试与原有归档、部署、运行时测试全部通过。

## Task 2: 在真实预检阶段拒绝冲突远端目标

**Files:**
- Modify: `apps/desktop/src-tauri/src/tools/release_package_remote.rs`
- Modify: `apps/desktop/src-tauri/src/tools/release_package_deploy.rs`（仅保留/复用防御性校验）
- Test: `release_package_remote.rs` 与 `release_package_deploy.rs` 测试模块

- [ ] **Step 1: 写目标关系纯函数的失败测试**

新增测试覆盖：

```rust
assert!(validate_target_relationships(&binding_with("/srv/app", "/srv/app/app.jar")).is_err());
assert!(validate_target_relationships(&binding_with("/srv/app/app.jar", "/srv/app")).is_err());
assert!(validate_target_relationships(&binding_with("/srv/app/web", "/srv/app/app.jar")).is_ok());
assert!(validate_target_relationships(&frontend_only_binding()).is_ok());
```

错误断言必须包含两个冲突路径。

- [ ] **Step 2: 运行目标关系测试并确认 RED**

运行：

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package_remote -- --nocapture
```

预期：新增测试因校验函数不存在或未拒绝父子路径而失败。

- [ ] **Step 3: 实现规范路径关系校验并接入预检**

在 `release_package_remote.rs` 增加基于 `/` 分段的父子判断，不能用裸字符串前缀误判 `/srv/app2`。仅比较 `PreflightBinding.targets` 中本次选中的目标；在 `remote_preflight_with_conn` 创建 SSH 写入探针前调用。保留 `release_package_deploy.rs` 的 `validate_remote_target_paths` 作为部署入口防御校验。

- [ ] **Step 4: 运行目标关系与全 Rust 回归测试**

运行：

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package -- --nocapture
```

预期：目标关系新增测试通过，原有路径安全、预检绑定和部署事务测试不回归。

## Task 3: 增加 probe/preflight 令牌撤销链路

**Files:**
- Modify: `apps/desktop/src-tauri/src/tools/release_package_remote.rs`
- Modify: `apps/desktop/src-tauri/src/tools/release_package.rs`
- Modify: `apps/desktop/src/bridge/tauri.ts`
- Modify: `apps/desktop/src/composables/useReleasePackageUploadPreflight.ts`
- Test: `release_package_remote.rs`、`release_package.rs`、`useReleasePackageUploadPreflight.test.ts`

- [ ] **Step 1: 写后端撤销失败测试**

在 remote store 测试中先 `store_probe` 和 `issue_preflight`，调用新撤销函数后断言 `load_probe` / `consume_preflight` 均返回无效；重复撤销必须返回 `Ok(())`。对 preflight 使用密码或私钥口令的测试同时断言撤销后令牌不可再次消费。

- [ ] **Step 2: 运行令牌测试并确认 RED**

运行：

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package_remote::tests -- --nocapture
```

预期：新增撤销测试因缺少撤销函数或令牌仍可消费而失败。

- [ ] **Step 3: 实现幂等撤销 action**

在 remote store 增加 `discard_probe`、`discard_preflight`，使用 `HashMap::remove`，不存在时也返回成功。`release_package.rs` 增加 `remote-discard` action，读取可选 `probeToken` / `preflightToken`，逐一调用撤销函数且不返回存在性。更新 `ACTIONS` 和 `apps/desktop/src/bridge/tauri.ts` 的 channel 映射。

- [ ] **Step 4: 写前端 reset 撤销测试并确认 RED**

在 `useReleasePackageUploadPreflight.test.ts` 中让 probe/check 返回令牌，调用 `reset()`，断言 invoke 收到 `tool:release-package:remote-discard` 及两个令牌；随后断言本地 `probeResult`、`preflightResult`、`preflightToken` 已清空。重复 `reset()` 不应提交旧令牌。

- [ ] **Step 5: 实现 composable 的令牌生命周期**

让 `reset()` 先捕获当前令牌，再在 `try/finally` 中清空本地 secret、结果和 request token，并调用撤销 action。更新 `ReleasePackagePanel.vue` 的关闭、覆盖取消、预检异常和 `finally` 路径等待或触发 reset；启动成功消费令牌后重复撤销必须按幂等处理，不恢复旧状态。

- [ ] **Step 6: 运行令牌单测和类型检查**

运行：

```powershell
pnpm --filter @lazycat/desktop exec vitest run src/composables/useReleasePackageUploadPreflight.test.ts
pnpm typecheck
```

预期：令牌撤销测试与 TypeScript 类型检查通过。

## Task 4: 在打包页面持续展示错误与警告

**Files:**
- Modify: `apps/desktop/src/components/ReleasePackagePanel.vue`
- Modify: `apps/desktop/src/components/ReleasePackagePanel.test.ts`

- [ ] **Step 1: 写组件结构回归测试**

在组件测试中断言源码包含：整体错误绑定 `currentProjectRuntime` 的 `error`、前端和后端 `targetErrors`、上传 lane 的错误摘要，以及可换行/可访问的 `role="alert"` 容器。测试同时断言错误展示位于日志卡内，而不是只依赖全局通知。

- [ ] **Step 2: 运行组件测试并确认 RED**

运行：

```powershell
pnpm --filter @lazycat/desktop exec vitest run src/components/ReleasePackagePanel.test.ts
```

预期：新增结构断言失败，因为当前模板没有错误摘要。

- [ ] **Step 3: 实现错误摘要渲染**

增加 `frontendError`、`backendError` 和 `overallError` computed，从当前项目 runtime 读取，不创建第二份状态。整体错误显示在日志卡头部；目标错误显示在对应 lane 的状态 tag 下方；上传 lane 显示整体错误。使用 `white-space: pre-wrap`、`role="alert"` 和限制宽度，避免长路径撑破布局。成功清理警告也使用同一摘要位置展示。

- [ ] **Step 4: 运行前端上线包测试**

运行：

```powershell
pnpm --filter @lazycat/desktop exec vitest run src/components/ReleasePackagePanel.test.ts src/composables/useReleasePackageRuntime.test.ts src/utils/releasePackage.test.ts
```

预期：所有相关测试通过，且错误不会在项目切换或新运行时残留。

## Task 5: 联合验证与交付检查

**Files:**
- Modify: 仅在前述测试驱动步骤需要时更新对应测试和实现文件。

- [ ] **Step 1: 运行 Rust 完整上线包测试**

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package -- --nocapture
```

预期：0 failed；真实 SSH fixture 若缺少 `LAZYCAT_SSH_TEST_*` 环境变量，明确记录 ignored。

- [ ] **Step 2: 运行前端相关测试**

```powershell
pnpm --filter @lazycat/desktop exec vitest run src/utils/releasePackage.test.ts src/composables/useReleasePackageRuntime.test.ts src/composables/useReleasePackageUploadPreflight.test.ts src/components/ReleasePackagePanel.test.ts src/utils/globalNotification.test.ts src/components/GlobalNotificationPopup.test.ts
```

预期：所有测试文件通过。

- [ ] **Step 3: 运行类型、构建和差异检查**

```powershell
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
git diff --check
git status --short
```

预期：类型检查、渲染层构建和差异检查退出码为 0；工作区只包含本任务明确的文件。

- [ ] **Step 4: 汇总用户可感知验收**

手工按以下路径核对：

1. 本地覆盖时模拟旧备份无法删除，页面仍显示成功目录并提示残留备份。
2. 上传覆盖时模拟旧备份清理失败，页面显示已上传警告且没有重试按钮。
3. 配置父子远端目标，预检在构建前直接拒绝并显示两个路径。
4. 预检后关闭确认框，旧 token 不能再次消费，密码/私钥口令不出现在日志或通知中。
5. 构建产物缺失或上传失败后，重新打开打包页面仍能看到对应错误摘要。
