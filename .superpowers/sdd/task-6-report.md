# Task 6 报告：主从工作台 UI 与工具入口

## Status

完成。实现提交：待提交 `feat(release-package): 添加上线包打包工作台`。

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
