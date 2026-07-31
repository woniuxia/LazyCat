# 结构治理批次 3 实施计划（Todo 域）

- 日期：2026-07-11
- 依据 spec：`docs/superpowers/specs/2026-07-04-structure-refactor-roadmap-design.md` 第 8 节（框架），接缝清单按 2026-07-11 现场代码核实
- 影响范围：3a/3b 仅 `apps/desktop` 前端；3c 仅 `src-tauri/src/tools/todo*`
- 执行约定（与批次 0-2 一致）：**行为保持机械拆分**——不改任何行为、接口语义、文案、样式效果；每阶段验证通过即提交；改良点只记 `process.md` 不顺手做；每阶段开工前 `git status` 确认工作区干净

## 总览

| 阶段 | 内容                                                          | 提交数 | 核心验收                                    |
| ---- | ------------------------------------------------------------- | ------ | ------------------------------------------- |
| 3a   | 9 个 Todo\* 组件 + 5 个 colocated 测试迁入 `components/todo/` | 1      | typecheck + build:web + test + e2e          |
| 3b   | TodoPanel.vue（2961 行）拆 5 个 composable                    | 5      | 同上 + 行为清单手工冒烟                     |
| 3c   | todo.rs（3253 行）目录化拆分                                  | 约 7   | `cargo test todo` 用例数前后一致（基线 22） |
| 3d   | process.md 经验沉淀                                           | 1      | —                                           |

前端路径均相对 `apps/desktop/src/`，Rust 路径相对 `apps/desktop/src-tauri/src/`。

## 阶段 3a：Todo 域目录搬迁

### 3a.1 `git mv` 清单（→ `components/todo/`）

9 组件：TodoBasicsDialog / TodoCalendarGrid / TodoContextMenu / TodoDetailEdit / TodoDetailView / TodoEmptyState / TodoPanel / TodoQuickAddBar / TodoSidebar（各 `.vue`）

5 测试：TodoDetailView.layout.test.ts / TodoPanel.edit-focus.test.ts / TodoPanel.quick-add.test.ts / TodoPanel.title-enter.test.ts / TodoQuickAddBar.test.ts。**已核实**：5 个测试均用 `readFileSync(new URL("./Xxx.vue", import.meta.url))` 相对定位源码，随组件同目录搬迁后路径依然成立，**零改动**。

留在根目录：`InlineTodoList.vue`（跨域桥组件，spec 6 节先例）、`WidgetTodoList.vue`（挂件域）。

### 3a.2 引用更新（已核实）

- 外部引用仅 1 处：`tool-registry.ts:58` `./components/TodoPanel.vue` → `./components/todo/TodoPanel.vue`
- 集合内部 `./Todo*` 相对引用随整体平移继续有效，不改。
- 集合外相对引用统一加一级（已核实清单）：
  - TodoDetailEdit.vue:427-428：`./InlinePmSelector.vue`、`./RichDescriptionEditor.vue` → `../Xxx.vue`
  - TodoDetailView.vue:339：`./RichDescriptionViewer.vue` → `../RichDescriptionViewer.vue`
  - 各组件 `../types|utils|bridge|composables/...` → `../../...`（TodoPanel 约 20 处、其余组件各 1-4 处）
- `components.d.ts` 自动生成；若 build 后有 diff 随本提交带上（批次 0-2 后 acf0bc9 先例）。

### 3a.3 验证与提交

`pnpm typecheck` → `pnpm --filter @lazycat/desktop build:web` → `pnpm test` → `pnpm test:e2e`

**提交**：`refactor(todo): Todo 域组件迁入 components/todo 子目录`

## 阶段 3b：TodoPanel.vue 拆分（2961 行）

script 段 455-2187；模板 1-454 与样式 2189+ 本阶段不动。五个抽取物**各自独立提交**，每步 typecheck + build:web + `pnpm test`。行号为 2026-07-11 版锚点（3a 搬迁不改行号结构，仍可用）。

**共享状态留壳层**（批次 1b 先例）：`items/types/assignees/projectOptions`、`itemDraft`、`detailMode/itemDialogMode/selectedItemId/draftBaseline/editingItemSnapshot`、筛选条件 refs、`todoDetailEditRef`、右键菜单 `todoContextMenu`。composable 只接收 refs/回调、返回派生与函数。

**测试约束（已核实断言目标）**：

- `TodoPanel.quick-add.test.ts` 盯 `quickAddContext` 派生、`onQuickAddCreated`/`isItemVisibleInList`、高亮 timer、onBeforeUnmount 清理 → 这些**全部留壳层**，测试零改动。
- `TodoPanel.title-enter.test.ts` 盯 `onTitleEnter` 函数体与 `void saveItem()` 调用 → `onTitleEnter`、`saveItem` 胶水**留壳层**，测试零改动。
- `TodoPanel.edit-focus.test.ts` 盯 `enterEditMode` 签名与 focus 链 → 随编辑器状态机迁出，**同步把该测试对应断言的读取源改为 composable 文件**（行为断言本身不变，机械改路径）。

### 3b.1 `composables/useTodoItemFilters.ts`（筛选分组）

- 搬移 775-853：`filteredItems`、`sortedTypes`、`bucketedItems/activeItems/recentWeekItems/doneItems`、`hasActiveFilter`、`applyDisplayFilter`、`displayActiveItems/displayRecentWeekItems/displayDoneItems`、`todayDueCount`、`overdueCount`、`clearAllFilters`。
- 输入：`items/types/itemKeyword/filterType/filterPriority` refs + `itemScheduleAt`/`isItemOverdue` 回调（这两个 helper 及 `isActionableStatus` 被模板行渲染共用，留壳层传入）。
- `quickAddContext`（855-862）与 quick-add 高亮块（864-883）**留壳层**（测试约束）。
- **提交**：`refactor(todo): 抽取 useTodoItemFilters`

### 3b.2 `composables/useTodoScheduleFields.ts`(调度字段)

- 搬移：616-637（pad2/splitDraftEventTime/composeDraftEventTime）、719-736（hourOptions/minuteOptions/repeatPresetOptions/weekdayOptions/priorityOptions）、592-601（reminderPresetOptions）、1085-1108（showRecurrenceFields/showCustomRepeatFields/showCronRepeatFields/eventHour/eventMinute；editingItemIsRecurring 见下）、1215-1221（disabled\* 时间选择限制）、1243-1380（buildRulePayload/buildEndValue/buildEventAt/syncSimpleDraftFromRule/applyRepeatPresetRule/onRepeatPresetChange/onCustomFrequencyChange/onReminderPresetsChange/resetReminderPresetsToNone/clearEventSchedule/fillDefaultDateTime/fillQuickDate）、`isRepeating`（773）。
- 输入：`itemDraft`、`lastReminderPresetSelection`、`editingItemSnapshot`、`itemDialogMode`。`editingItemIsRecurring`（1085-1087）随迁（仅 onRepeatPresetChange 与模板使用，从返回值暴露）。
- `initialCreateSchedule`（738）使用点实施时核实，跟随使用方。
- **提交**：`refactor(todo): 抽取 useTodoScheduleFields`

### 3b.3 `composables/useTodoCrudActions.ts`（CRUD 操作）

- 搬移：1478-1507（loadTypes/loadAssignees/loadItems/loadProjects）、1705-1746（resolveTypeId/resolveAssigneeIds + normalizeName/getNextTypeSortOrder 1235-1241）、1775-1891（submitItemChanges）、1899-1932（changeItemStatus/toggleItemPin/openLink/snoozeItem）、1934-2095（deleteItem/showDeleteScopeDialog）、2097-2099（onBasicsChanged）。
- 输入：`items/types/assignees/projectOptions/filterProjectId` refs、`itemDraft`、`itemDialogMode`、`todoDetailEditRef`、`closeTodoContextMenu` 回调（loadItems 首行调用，菜单留壳层）、3b.2 返回的 `isRepeating/showRecurrenceFields/showCronRepeatFields/showCustomRepeatFields/buildEventAt/buildRulePayload/buildEndValue`。
- `saveItem`（1893-1897）为 CRUD×编辑器胶水，**留壳层**（title-enter 测试约束）；`onCheckItem`/`copyTitle` 模板胶水留壳层。
- **提交**：`refactor(todo): 抽取 useTodoCrudActions`

### 3b.4 `composables/useTodoPmLink.ts`（PM 关联）

- 搬移：562-568（todoPmLinkItemId/todoPmCandidates/todoLinkedPmItem/pmCreateDialogVisible/pmCreateTitle/pmCreateProjectId）、564（skipProjectWatch，`let` 改为 composable 内 ref，壳层引用点 `.value` 化——机械适配点）、534-539（pmStatusColor/pmStatusLabel）、1509-1703（loadTodoPmCandidates/onPmCreateConfirm/onPmCreateClosed/onTodoPmLinkChange/handlePmSelectChange/handlePmProjectChange/handlePmCreate/handlePmSearch/navigateToPmItem/pmItemTagStyle）、2103-2122（watch(itemDraft.projectId) 关联重置）。
- 输入：`itemDraft`、`submitChanges`（= 3b.3 的 submitItemChanges）、`loadItems`、`requestPmFocus`/`openTab`。
- 注意：`resetItemDraft`/`applyItemToDraft`（壳层，3b.5 前）直接写 todoPmLink 状态——composable 返回这些 refs 后壳层代码字面不变。命名避让 PM 侧已有 `usePmTodoLinking`。
- **提交**：`refactor(todo): 抽取 useTodoPmLink`

### 3b.5 `composables/useTodoDetailState.ts`（编辑器状态机）

- 搬移：885-928（normalizeDraftTypeValue/normalizeDraftAssigneeValues/snapshotItemDraft/markDraftBaseline）、831-836（isDetailEditing/isDraftDirty）、825-829（selectedItem computed）、930-978（ensureDetailCanLeave/finalizeDetailAfterSave/selectItem/selectItemAsync/prepareItemForInlineAction）、1021-1039（focusTitleInputWhenActive/focusCreateTitleInput + titleFocusTimer 及其清理）、1049-1083（startCreate/createOnDate/cancelDetailEdit）、1223-1233（toDraftAssigneeValues）、1382-1476（resetItemDraft/applyItemToDraft）、1753-1773（enterEditMode）、2124-2132（watch(selectedItem)）。
- 输入：`detailMode/itemDialogMode/selectedItemId/draftBaseline/editingItemSnapshot`、`itemDraft`、`showMoreFields`、`todoDetailEditRef`、`submitChanges` 回调、PM 关联 refs（resetItemDraft/applyItemToDraft 写入）、`loadTodoPmCandidates`。
- `onTitleEnter`（1041-1047）、`applyPendingTodoInput` 剪贴板簇（639-717）、`saveItem`、quick-add 簇留壳层。
- **同步修改** `TodoPanel.edit-focus.test.ts`：enterEditMode/focus 断言的读取源改为 `../../composables/useTodoDetailState.ts`（断言内容不变）。
- **提交**：`refactor(todo): 抽取 useTodoDetailState`

### 3b.6 行为清单手工冒烟（3b 完成后整体执行，需运行应用，与用户协调）

列表/日历双视图切换、创建（含快速添加栏与日历格创建）、编辑与脏保护（切换事项自动保存）、标题回车保存、右键菜单四动作（置顶/完成/编辑时间/删除）、重复事项规则编辑与删除范围对话框、提醒预设联动、PM 关联（选择/新建/解除/跳转 PM）、筛选与搜索联动、已办/最近一周折叠、收纳箱转待办（剪贴板 pending input）。发现行为差异：停下修复或 revert 对应提交。

## 阶段 3c：todo.rs 目录化拆分（3253 行）

对账基准（2026-07-11 已核实）：ACTIONS 26 词条（83-104 行）；`execute` 分发 111-138；`#[cfg(test)] mod tests` 2730 行至文件尾，22 个 `#[test]`。

**外部符号（必须经 mod.rs `pub use` 再导出保持调用点零改动，已核实 6 处调用方）**：

- `execute`、`scheduler_tick`、`supported_actions`（cfg(test)）——留 mod.rs 本体
- `ReminderDispatch`（main.rs:373/411）、`ReminderConfig` → types.rs 后 `pub use types::...`
- `compute_remind_at`、`reminder_configs_from_presets`（pm_todo_link.rs:7）→ reminders.rs 后 `pub use`
- `is_open_status`（widget/dashboard_logic.rs:13）→ helpers.rs 后 `pub use`
- `sync_item_reminders`（pub，实施时核实调用方）

### 3c.1 迁移前基线

`cargo test todo -- --list` 记录用例清单与数量（当前 22）。

### 3c.2 文件转目录（纯移动，独立提交）

`git mv tools/todo.rs tools/todo/mod.rs`，验证 `cargo check`。
**提交**：`refactor(todo): 模块转为目录形态`

### 3c.3 逐模块抽取（每步 `cargo test todo` 通过且用例数不变后提交）

1. `types.rs`（常量 11-40 + 结构体 42-81：ReminderDispatch/SeriesRuleRow/ReminderConfig/TaskReminderSummary）+ `helpers.rs`（147-402：payload 解析/日期时间解析校验/scope/kind/优先级状态归一化/状态迁移 can_transit\*/排序 sort_item_rows 簇）。
   **提交**：`refactor(todo): 抽取共享 types 与 helpers`
2. `recurrence.rs`（402-632：rule mode/cron 构建/next occurrence/end rule + 1086-1275：load_series_rule/should_stop_series/has_other_open_in_series/generate_next_item）。
   **提交**：`refactor(todo): 抽取 recurrence 子模块`
3. `reminders.rs`（634-752：preset 偏移/归一化/ReminderConfig 构建/compute_remind_at + 842-1083：reminder 配置与摘要加载/sync_item_reminders/snooze 清理与解析/事件已读 + 2426-2511：reminder_list_unread/reminder_mark_read + 2524-2605：dispatch_due_reminders）。`scheduler_tick` 留 mod.rs（调用 dispatch_due_reminders）。
   **提交**：`refactor(todo): 抽取 reminders 子模块`
4. `taxonomy.rs`（1277-1398：type_list/type_upsert/type_delete/assignee_list/assignee_upsert/assignee_delete）。
   **提交**：`refactor(todo): 抽取 taxonomy 子模块`
5. `items.rs`（754-840：sync/load assignees+links + 1400-2424：item_list/item_create/item_update/item_upsert/item_change_status/item_delete/delete_item_by_id/item_toggle_pin/item_snooze/item_toggle_active + 2513-2522：open_link）。
   **提交**：`refactor(todo): 抽取 items 子模块`
6. `pm_link.rs`（2607-2728：pm_candidates/item_set_pm_link）。
   **提交**：`refactor(todo): 抽取 pm_link 子模块`

每步把对应内嵌测试从 `mod tests` 搬到该子模块自己的 `#[cfg(test)] mod tests`（批次 2 纪律：cfg 函数对成对迁移，如有）。`ACTIONS`、`supported_actions`、`execute` 分发、`scheduler_tick`、`parse_i64/parse_string` 等分发层小工具留 mod.rs（实施时按依赖归属微调，原则：函数体零改动，仅移动 + 可见性 + use 路径）。

### 3c.4 收尾对账与验收

1. `cargo test todo -- --list` 用例数与基线一致；`cargo test` 全量通过。
2. `pnpm typecheck` + `pnpm test:e2e`（无前端改动，跑基线确认无误伤）。
3. Todo 面板手工冒烟（与 3b.6 合并执行）。
4. Windows 注意：`cargo` 报文件锁时先结束运行中的 lazycat 进程。

## 阶段 3d：经验沉淀

- `process.md` 记录批次 3 经验（源码断言测试随代码迁移的处理、`let` 共享标志 ref 化适配点、与批次 0-2 模式差异）。
- 核对 spec 完成定义：TodoPanel 行数（`wc -l`）、todo 各子模块行数。
- 更新记忆 `structure-refactor-roadmap-status`。
- **提交**：`docs(process): 记录结构治理批次 3 拆分经验`

## 风险与注意

- **每阶段开工前** `git status` 确认干净；一个阶段未验收不开下一阶段。
- 3b 每个抽取物提交后发现行为差异，优先 `git revert` 该提交再重做，不带病前进。
- 3c 每步抽取保持函数体零改动（仅移动 + 可见性调整 + `use` 路径），diff 应呈现"删一块/加一块"形态。
- 3b.4 的 `skipProjectWatch` ref 化是本批唯一预期的非纯移动适配，diff 中显式可见、plan 已注记。
- 手工冒烟需运行应用：与用户协调时机，不自动启动 dev server。
- 批次 4+ 不在本计划：完成后按候选池机制重排（spec 第 9 节）。
