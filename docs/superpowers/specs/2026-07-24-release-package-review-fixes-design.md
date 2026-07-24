# 上线包用户流程一致性修复设计

## 背景

上线包的本地归档和服务器上传主流程已经具备构建、预检、事务提交、取消和通知能力，但代码审查发现四个用户可感知问题：交付已经提交后，旧备份清理失败仍被误报成交付失败；运行错误只保存在状态中，打包页面没有持续展示；远端目标互相包含时要等构建结束才失败；关闭或取消上传确认后，未消费的预检令牌及认证秘密不会立即撤销。

本设计只修复以上四项，不调整打包类型、项目字段、构建并行方式、上传来源或远端替换事务。

## 目标

1. 新版本已经提交后，即使旧备份清理失败，也以交付成功终态呈现，同时明确告知残留备份路径。
2. 打包页面持续展示整体、目标和上传错误或警告，用户不依赖瞬时通知定位问题。
3. 本次同时上传的远端目标相同或互相包含时，在构建开始前的真实预检中拒绝。
4. 上传确认关闭、覆盖取消或预检后异常时，主动撤销未消费令牌并立即释放后端内存中的认证秘密。

## 非目标

- 不新增 `succeeded_with_warnings` 等公开状态。
- 不改变“部分构建失败不上传服务器”的规则。
- 不改变本地归档或远端 temp、backup、commit 的事务步骤。
- 不增加远端回滚、版本管理或部署历史。
- 不修改项目保存时的远端路径完整性要求；目标关系按本次选择在预检阶段校验。

## 提交成功后的清理警告

本地归档和远端部署都把“正式目标已经切换”作为交付提交点。提交点之后删除旧备份失败，不再返回普通失败：

- 本地归档返回最终归档路径和清理警告。
- 服务器上传返回远端已提交标记和清理警告。
- 整体状态保持 `succeeded`；本地部分成功归档保持 `partially_succeeded`。
- `error` 字段携带可直接展示的警告文本及残留备份路径。
- 服务器上传不生成 retry token，因为重复上传不能解决已经完成的提交，也可能误导用户再次发布。

提交前失败、提交中失败或回滚失败仍保持现有失败语义。回滚失败继续返回明确恢复路径，不伪造成功。

实现上使用内部结构化结果区分成功警告和失败，不能根据错误字符串判断是否已提交。

## 打包页面错误与警告展示

运行 composable 继续作为状态唯一来源，不复制错误状态。面板按以下层级展示：

- 整体 `runtime.error`：显示在日志卡标题下方，覆盖归档提交错误、上传错误和清理警告。
- `targetErrors.frontend/backend`：显示在对应日志 lane 的状态区域下方。
- 上传失败沿用整体错误，在上传 lane 中提供同一错误摘要；存在有效 retry token 时保留“重试上传”。

错误和警告使用可换行文本，不遮挡日志。开始新运行时随项目 runtime 一起清空；切换项目时只显示该项目最近一次结果。

## 远端目标集合预检

新增共享的远端目标集合校验，输入为本次选中的目标及其规范化绝对路径。选择前后端两个目标时拒绝：

- 两个正式目标路径完全相同。
- 前端目录是后端文件的父路径。
- 后端路径是前端目录的父路径。

校验在建立认证后的远端写入探针之前执行，并继续在部署入口保留防御性校验。只选择一个目标时不检查未选择目标，因此不会因为未参与本次上传的配置阻止构建。

返回错误必须包含冲突的两个路径，并提示用户修改远端目标配置后重新预检。

## 临时令牌撤销

上线包后端增加令牌撤销 action，接受当前弹窗持有的可选 probe token 和 preflight token。撤销操作遵循以下规则：

- 删除存在的令牌并立即 drop 其中的 `Zeroizing` 认证秘密。
- 令牌不存在、已消费或已过期时幂等成功，方便前端在 `finally` 和关闭事件中重复调用。
- 不返回秘密、绑定信息或令牌是否曾存在。
- 正式启动消费 preflight token 后，后续撤销仍幂等成功。

前端在以下时机调用撤销：

- 用户关闭确认弹窗。
- 用户取消远端覆盖确认。
- 主机信任、认证预检或启动流程抛错。
- 启动成功后的敏感状态清理。

前端先捕获待撤销令牌，再清空输入和本地 ref；撤销失败只显示明确错误，不恢复秘密或旧令牌。应用退出时继续保留全量临时存储清理作为最后防线。

## 测试策略

严格按回归测试先行：

1. 本地归档测试模拟正式目录提交成功、旧备份删除失败，断言保留最终路径和成功终态警告。
2. 远端部署测试模拟所有正式目标提交成功、备份清理失败，断言结果标记已提交、无 retry descriptor，并携带残留路径。
3. 前端测试断言整体错误、目标错误和上传错误在对应区域持续渲染。
4. 远端预检纯函数测试覆盖相同、父子和单目标路径；部署入口原有防御测试继续通过。
5. 令牌存储测试断言撤销后不可消费、重复撤销成功；前端 composable 测试断言 reset 会提交当前令牌并立即清空本地状态。

最低验证：

```text
pnpm --filter @lazycat/desktop exec vitest run src/utils/releasePackage.test.ts src/composables/useReleasePackageRuntime.test.ts src/composables/useReleasePackageUploadPreflight.test.ts src/components/ReleasePackagePanel.test.ts
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package -- --nocapture
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
git diff --check
```

真实 SSH fixture 不可用时必须明确记录跳过，不能用单元测试替代真实协议验证。

## 影响文件

- `apps/desktop/src-tauri/src/tools/release_package_archive.rs`
- `apps/desktop/src-tauri/src/tools/release_package_deploy.rs`
- `apps/desktop/src-tauri/src/tools/release_package_remote.rs`
- `apps/desktop/src-tauri/src/tools/release_package_runtime.rs`
- `apps/desktop/src-tauri/src/tools/release_package.rs`
- `apps/desktop/src/types/release-package.ts`
- `apps/desktop/src/composables/useReleasePackageUploadPreflight.ts`
- `apps/desktop/src/composables/useReleasePackageUploadPreflight.test.ts`
- `apps/desktop/src/components/ReleasePackagePanel.vue`
- `apps/desktop/src/components/ReleasePackagePanel.test.ts`
- 对应 Rust 单元测试
