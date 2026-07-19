# Task 6 报告：主从工作台 UI 与工具入口

## Status

完成。实现提交：`4bcd750 feat(release-package): 添加上线包打包工作台`；review 修复提交：`fix(release-package): 收紧工作台启动与路径状态`。

## TDD

### RED

先新增 `ReleasePackagePanel.test.ts` 的源码契约断言和 `toolCatalog.test.ts` 的工具注册断言，运行：

```text
pnpm --filter @lazycat/desktop test -- src/components/ReleasePackagePanel.test.ts src/composables/toolCatalog.test.ts
```

结果按预期失败：组件文件不存在，且 `release-package` 尚未注册到工具目录。

### GREEN

实现后运行：

```text
pnpm --filter @lazycat/desktop test -- src/components/ReleasePackagePanel.test.ts src/composables/toolCatalog.test.ts src/composables/useReleasePackageRuntime.test.ts src/utils/releasePackage.test.ts
```

结果：4 个测试文件、15 个测试全部通过。

## 实现

- 新增紧凑浅色主从工作台：左侧项目配置列表，右侧基本信息、前后端工程配置，底部实时日志。
- 在工具目录注册 `release-package`（上线包打包），并动态加载 `ReleasePackagePanel.vue`。
- 接入项目列表、创建、更新、删除、预检和启动 IPC；保存前执行 draft 校验，脏表单切换和删除均二次确认。
- 全局归档根目录、前端工程目录、后端工程目录使用 Tauri 目录选择器；全局设置使用 awaited `setSettingAndWait`。
- 打包前调用 `prepare` 生成最近周四默认目录名，确认 Dialog 允许修改并实时展示完整归档路径；拒绝路径分隔符输入。
- 启动使用 runtime singleton 的 listener、日志、状态、取消控制；运行中锁定配置，仅保留终止操作。
- 支持 `copy_directory` / `zip_directory` 选择，后端产物路径保留文件或目录文本输入；成功后可打开归档目录。
- 日志容器使用 `aria-live="polite"`，stderr 使用危险色，不持久化日志、不在前端覆盖归档目录。

## 文件

- `apps/desktop/src/components/ReleasePackagePanel.vue`
- `apps/desktop/src/components/ReleasePackagePanel.test.ts`
- `apps/desktop/src/composables/toolCatalog.ts`
- `apps/desktop/src/composables/toolCatalog.test.ts`
- `apps/desktop/src/tool-registry.ts`

## Validation

```text
pnpm --filter @lazycat/desktop test -- src/components/ReleasePackagePanel.test.ts src/composables/toolCatalog.test.ts src/composables/useReleasePackageRuntime.test.ts src/utils/releasePackage.test.ts
PASS: 4 files, 15 tests

pnpm typecheck
PASS: packages/formatters and apps/desktop

git diff --check
PASS
```

## Concerns

- 未启动正式 dev server 或执行 UI/E2E；本任务验证覆盖源码契约、runtime/纯函数回归和 TypeScript 类型检查。

## Review 修复

- 全局归档根目录改为只读输入，只能通过目录选择器和 `setSettingAndWait` 更新，消除 UI 与后端设置双真值。
- 启动请求等待期间提供“终止打包”，`runId` 尚未返回时记录待取消状态，返回后立即调用 runtime cancel；事件已先绑定 `runId` 时直接取消。
- 将 listener 初始化纳入启动 `try/finally`，初始化失败会 `abortStart`、显示错误并恢复 starting 状态。
- 首次加载优先恢复 singleton runtime 的 `activeProjectId` 项目，不再固定选择列表首项。
- 前端增加 Windows 目录名校验：空值、首尾空格、`.` / `..`、控制字符、非法字符、尾点/空格、超长名称和保留设备名。
- `loadProjects` 显式返回成功状态；保存或删除后的刷新失败不再误报成功，并提前维护新建 ID、删除后的选择和 draft 状态。
- 确认 Dialog 的路径预览只使用 `prepare` 返回的 `outputRoot` / `archivePath`，避免本地设置状态参与执行确认。
- review 修复后 focused Vitest 为 4 个文件、17 个测试全部通过；`pnpm typecheck` 和 `git diff --check` 通过。

### 复审补充

- active 项目只在面板首次无选中项或 runtime 确实处于 running 状态时优先；普通刷新保持已有 selectedId 和脏草稿，保存后仍显式恢复 savedId。
- 删除 IPC 成功后先清空 selectedId 与 draft，再刷新项目列表；即使刷新失败，也不会继续把已删除配置显示为有效选中项。
- 新增源码契约测试固定 active 选择条件与删除清空顺序；focused 测试更新为 4 个文件、18 个测试全部通过。
- 进一步限定 active 恢复：新建脏 draft（`selectedId === null && dirty`）刷新时保持新建态，只有首次干净状态或 running 才恢复 active；删除成功后立即从本地项目列表移除目标，刷新失败也不会残留已删除项。
