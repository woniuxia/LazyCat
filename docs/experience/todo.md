# Todo 经验

适用范围：事项/系列模型、事件时间、提醒、逾期、列表与详情交互。

关键词：`eventAt`、`displayAt`、`completedAt`、`reminderPresets`、`series`、`actionBinding`

## 目录

- [2026-07-26：事项与动作绑定必须同事务](#2026-07-26事项与动作绑定必须同事务)
- [当前模型：事项实例与周期系列](#当前模型事项实例与周期系列)
- [时间字段语义不可回退混用](#时间字段语义不可回退混用)
- [提醒与事件时间分离](#提醒与事件时间分离)
- [列表分层与逾期判断](#列表分层与逾期判断)
- [表单与受控状态](#表单与受控状态)
- [结构治理](#结构治理)

## 2026-07-26：事项与动作绑定必须同事务

**场景**：单次 Todo 可选择一个动作及已有目标，提醒到期或用户在任务详情中手动触发动作。

**问题**：Todo 与动作绑定分开保存会产生孤儿绑定或半成功；提醒生成时保存的动作摘要也可能因目标被删除、绑定被修改而过期。

**解决**：Todo 新增、修改、删除与通用动作绑定在同一 SQLite 事务内提交或回滚。提醒负载只携带只读摘要用于展示；用户点击“开始打包”时重新校验 Todo、提醒事件、绑定、目标可用性和活动 dispatch，再创建本次派发。

**关键点**：第一版仅允许 `one_off` Todo 绑定一个动作；周期事项显式拒绝动作字段；动作只有完整成功时才复用 Todo 的既有状态转换完成任务，手动完成时间保持不变。

**涉及文件**：

- `apps/desktop/src-tauri/src/tools/todo/`
- `apps/desktop/src-tauri/src/tools/action_center/bindings.rs`
- `apps/desktop/src-tauri/src/tools/action_center/dispatches.rs`

**验证**：

- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml todo -- --nocapture`
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml action_center -- --nocapture`
- `pnpm --filter @lazycat/desktop test -- src/composables/useTodoItem.test.ts src/composables/useTodoActionBinding.test.ts src/components/todo/TodoActionBinding.test.ts`

**使用次数**：0

## 当前模型：事项实例与周期系列

主列表以可执行事项为中心，单次事项也属于 `one_off` 系列；周期系列生成实例。编辑周期实例时区分当前实例与后续系列，删除系列不删除历史实例。旧 `task_*` / `template_*` 仅是兼容背景，不应继续作为新功能入口。

## 时间字段语义不可回退混用

- `eventAt`：真实事件时间。
- `displayAt`：可展示的计划时间；周期根可表示下一次发生时间，普通事项不得回退到 `updatedAt`。
- `completedAt`：真实完成时间，最近一周已办按它统计和展示。
- `updatedAt`：元数据，不参与日程展示或业务排序。

解析 `YYYY-MM-DD` 时使用本地年月日构造，禁止 `new Date('YYYY-MM-DD')` 引入 UTC 偏移。

## 提醒与事件时间分离

前端使用 `eventAt + reminderPresets`；提醒可多选，实例保存时由事件时间计算调度时间。修改事件时间或提醒预设时清理旧的稍后提醒/已通知状态。提醒触发由独立窗口/统一全局通知承载，不恢复已删除的“提醒中心”视图。

## 列表分层与逾期判断

当前面板显示任务列表、最近一周已办和已办事项。逾期只面向 `pending/in_progress`，由真实事件时间判断；已办不标逾期。筛选先形成基础集合，再分桶与排序，UI 折叠状态不进入纯函数。

## 表单与受控状态

程序同步和用户手动操作需要区分时使用 `:model-value` + `@update:model-value`。清空日期/时间必须持久化为空，不能用隐藏默认值补回。标题自动聚焦应等待实际编辑 DOM 挂载后执行。

## 结构治理

Todo 已按 types、纯函数、composables 与子组件拆分。新增行为优先落到现有 `useTodo*` composable 或 `src/utils/todo*.ts`，避免把逻辑重新堆回 `TodoPanel.vue`。

**使用次数**：0
