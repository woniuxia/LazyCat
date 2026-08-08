# 工具标签页状态保留

## Destination

在 LazyCat 主界面的工具标签页导航中，仍打开的工具页面在切换后恢复原有页面状态，不因重新激活而重复初始化；关闭标签页后才销毁对应页面。暂不要求应用重启后恢复。

## Notes

- 领域：工具标签页、页面生命周期、页面状态保留。
- 需结合 `CONTEXT.md`、`docs/experience/architecture.md` 和 `docs/experience/ui-and-styling.md`。
- 当前事实源：`apps/desktop/src/App.vue`、`apps/desktop/src/composables/useTabs.ts`、工具面板组件及其生命周期代码。
- 本地图使用本地 Markdown issue tracker；每个子工单只解决一个决策或调查问题。

## Decisions so far

- [页面生命周期契约](E:/Projects/LazyCat/.scratch/tool-tab-state-retention/issues/01-page-lifecycle-contract.md) — Tab 页面保留 UI 实例并在失活时暂停页面级活动；应用级长任务和后台服务脱离页面继续运行，关闭 Tab 不自动取消。
- [缓存边界与稳定身份](E:/Projects/LazyCat/.scratch/tool-tab-state-retention/issues/02-cache-boundary-and-identity.md) — 每个标签页使用独立页面容器和独立 `KeepAlive` 边界，`tab.id` 保持一工具一 Tab 身份，关闭 Tab 通过卸载宿主精确销毁页面缓存但不取消应用级任务。
- [非活动数据刷新](E:/Projects/LazyCat/.scratch/tool-tab-state-retention/issues/03-inactive-data-refresh.md) — 重新激活默认保留页面结果；外部数据只做轻量刷新，保护脏草稿和局部浏览状态，用户已启动的单次异步操作继续执行且按 Tab/操作代次隔离响应。
- [缓存资源策略](E:/Projects/LazyCat/.scratch/tool-tab-state-retention/issues/04-cache-resource-policy.md) — 暂不自动淘汰仍打开的 Tab；失活时暂停或释放页面级重型资源，保留用户状态，关闭 Tab 才销毁页面缓存。
- [验收与回归范围](E:/Projects/LazyCat/.scratch/tool-tab-state-retention/issues/05-acceptance-and-regression-surface.md) — 以页面实例连续性、操作次数、资源清理和关键 UI 状态作为验收证据，覆盖代表性工具和单个/批量关闭路径。

## Not yet specified

- [应用级任务所有权与重新进入](E:/Projects/LazyCat/.scratch/tool-tab-state-retention/issues/06-app-task-ownership-and-reentry.md) — 应用级长任务的状态所有权、显式取消、退出和重新进入尚未定义。

## Out of scope

- 应用重启后的标签页和工具内容恢复。
- 将当前工具导航重构为 Vue Router 或新增独立窗口。
- 对单个工具业务数据、持久化模型或后台服务做无关重构。
