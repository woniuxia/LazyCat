# Todo 重构设计：迁移到 todo_items（完成触发生成下一次）

日期：2026-03-14

> 目标：后端从 `todo_tasks/todo_templates` 模型切换到 migration 22 的统一模型 `todo_items + todo_item_*`，并将"重复事项"改为 **仅在完成时触发生成下一次**。重复规则独立存储在 `todo_series_rules` 表中（series 级别）。

## 1. 背景与现状（已在仓库中验证）

- migration 22 已落库（`apps/desktop/src-tauri/src/tools/helpers.rs:1089-1499`）：
  - 新表：`todo_items`
  - 支撑表：`todo_item_assignees` / `todo_item_reminders` / `todo_item_links`
  - 迁移后会 DROP 旧表：`todo_tasks` / `todo_templates` 以及对应 \*\_reminders/\_assignees/\_links
- 迁移过程会导致同一 `series_id` 下可能存在多条 `pending/in_progress` 的事项（旧逻辑会预生成实例，migration 也会把历史实例拷贝进来）。
- **本次设计决定**：在 migration 22 之后追加 migration 23，将规则字段从 `todo_items` 提取到独立的 `todo_series_rules` 表，并从 `todo_items` 中移除规则列和 `active` 列。

## 2. 数据模型

### 2.1 `todo_items`（主表）

存储每一条具体的待办事项（无论单次还是重复），不含重复规则。

- `id`: 主键
- `title`, `type_id`, `priority`, `description`: 基础业务字段
- `kind`: `one_off` | `recurring`
- `status`: `pending` | `completed` | `canceled`
- `series_id`: 重复系列标识；`one_off` 时为 NULL。同一系列的所有事项共享此值。
- `parent_id`: 指向同系列中前一个实例（链式历史）。
- `event_at`: 事项的计划时间（ISO8601）。
- `pinned`: 0/1，置顶。
- `remind_at`: 行级提醒时间（遗留字段，与 `todo_item_reminders` 表并存）。
- `snooze_until`: 行级贪睡时间（遗留字段）。
- `last_notified_at`: 行级最后通知时间（遗留字段）。
- `created_at`, `updated_at`: 时间戳。

> **行级提醒字段 vs `todo_item_reminders` 表**：两者并存。行级字段服务于简单的"单提醒"场景（向后兼容）；`todo_item_reminders` 表支持多提醒 preset。本次重构不改变此双轨机制。

### 2.2 `todo_series_rules`（重复规则表，新增）

每个重复系列一行，存储该系列的规则和推进状态。

```sql
CREATE TABLE todo_series_rules (
  series_id INTEGER PRIMARY KEY,    -- = todo_items.series_id，每个系列唯一
  rule_mode TEXT NOT NULL,          -- 'simple' | 'cron'
  rule_json TEXT,                   -- 频率/间隔等 JSON
  cron_expression TEXT,             -- 标准化 cron 表达式
  timezone TEXT,                    -- 时区
  start_at TEXT,                    -- 起始时间
  end_mode TEXT NOT NULL DEFAULT 'never',  -- 'never' | 'until_date' | 'after_count'
  end_value TEXT,                   -- 终止阈值
  occurrence_index INTEGER NOT NULL DEFAULT 1,  -- 已生成次数（用于 after_count 终止判断）
  active INTEGER NOT NULL DEFAULT 1,  -- 0=暂停，1=正常
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
```

**核心优势**：规则跟着 series 走，不绑定任何特定事项。任何项完成/删除时都能通过 `series_id` 找到规则，**无需 promote 算法**。

### 2.3 支撑表（不变）

- `todo_item_assignees(item_id, assignee_id)`
- `todo_item_reminders(item_id, reminder_preset, offset_minutes, remind_at, snooze_until, last_notified_at, ...)`
- `todo_item_links(item_id, url, title, sort_order, ...)`

## 3. 状态模型

### 3.1 存储层状态值

数据库保留 4 个值（受 migration 22 的 CHECK 约束限制）：

- `pending`
- `in_progress`
- `completed`
- `canceled`

### 3.2 语义层（后端统一口径）

- **Open（未完成）**：`pending` + `in_progress`
- **Done（已结束）**：`completed` + `canceled`

> 本次重构将 `in_progress` 视同 `pending`，仅用于历史兼容。

### 3.3 产品呈现层（A1 归一化）

- **不再使用/创建 `in_progress`**
- `item-list` 返回时执行 **A1 归一化**：将 `in_progress` 映射为 `pending`，前端完全不可见。

### 3.4 done→pending 撤销规则

- `one_off`：允许 `completed/canceled -> pending`
- `recurring`：**不允许** `completed/canceled -> pending`

理由：recurring 的 `completed` 已触发生成下一条，回滚会导致系列出现多条 open，破坏稳态模型。

### 3.5 合法状态转换表

| 当前状态    | 目标状态  | one_off | recurring |
| ----------- | --------- | ------- | --------- |
| pending     | completed | 允许    | 允许      |
| pending     | canceled  | 允许    | 允许      |
| in_progress | completed | 允许    | 允许      |
| in_progress | canceled  | 允许    | 允许      |
| completed   | pending   | 允许    | **拒绝**  |
| canceled    | pending   | 允许    | **拒绝**  |

## 4. 重复事项（recurring）生成模型

### 4.1 总原则

- **仅 `completed` 触发推进/生成下一次**
- `canceled`：不推进、不生成

### 4.2 完成时的处理流程

当某条 recurring 事项被标记为 `completed`：

**步骤 1 — 读取规则**：通过 `series_id` 查询 `todo_series_rules`。若规则不存在或 `active=0`，不生成，流程结束。

**步骤 2 — 去重检查**：查询同 `series_id` 下是否已存在其它 open 项（`status∈{pending, in_progress} && id != 当前id`）。

- **存在其它 open**：不生成新项（避免膨胀）。流程结束。
- **不存在其它 open**：进入步骤 3。

**步骤 3 — 终止条件检查**（从 `todo_series_rules` 读取）：

| end_mode      | 条件                                    | 结果     |
| ------------- | --------------------------------------- | -------- |
| `never`       | —                                       | 继续生成 |
| `until_date`  | `next_event_at > end_value`             | 停止生成 |
| `after_count` | `occurrence_index >= end_value`（整数） | 停止生成 |

**步骤 4 — 计算下一次时间**：调用 `compute_next_occurrence(cron_expression, timezone, base_time)`，其中 `base_time = max(当前项.event_at, now)`；若 `event_at` 为 NULL 则以 `now()` 为基准。若 cron 表达式在 `base_time` 之后无有效匹配，则视为终止，不生成。

**步骤 5 — 插入新项**：

```sql
INSERT INTO todo_items (
  title, type_id, priority, description, kind, series_id, parent_id,
  status, event_at, pinned
) VALUES (
  -- 继承自已完成项：title, type_id, priority, description, kind, series_id
  -- parent_id = 已完成项.id
  -- status = 'pending'
  -- event_at = 步骤 4 计算的下一次时间
  -- pinned = 0（新实例不继承置顶）
)
```

**步骤 6 — 复制支撑表数据**：

- `todo_item_assignees`：全量复制
- `todo_item_reminders`：复制 `reminder_preset` + `offset_minutes`，根据新 `event_at` 重算 `remind_at`；`snooze_until`、`last_notified_at` 置 NULL
- `todo_item_links`：全量复制

**步骤 7 — 更新规则表进度**：

```sql
UPDATE todo_series_rules SET
  occurrence_index = occurrence_index + 1,
  updated_at = CURRENT_TIMESTAMP
WHERE series_id = ?
```

### 4.3 删除语义（两种模式）

#### A) 仅删除本次（this_instance）

适用于：recurring 且 open（`status∈{pending, in_progress}`）的事项。

**删除流程**：

1. 读取被删项的 `series_id` 和支撑表数据（assignees/reminders/links），因为 DELETE 后级联清理会丢失。
2. 查询同 `series_id` 下是否存在其它 open 项（`status∈{pending, in_progress} && id != 当前id`）。
3. 删除该项（`DELETE FROM todo_items WHERE id=?`，支撑表通过 `ON DELETE CASCADE` 自动清理）。
4. 根据步骤 2 的结果：
   - **有其它 open**：不生成。流程结束。
   - **无其它 open**：通过 `series_id` 从 `todo_series_rules` 读取规则，检查终止条件，补生成 1 条新项（保持系列不断档）。`base_time = max(被删项.event_at, now)`。使用步骤 1 缓存的支撑表数据复制到新项。`occurrence_index` 不递增（删除不算完成）。

> 关键：步骤 1 必须在步骤 3（DELETE）之前完成，因为级联删除会清除支撑表数据。

#### B) 终止整条系列（future_instances / terminate）

采用"暂停规则"策略：

```sql
UPDATE todo_series_rules SET active=0, updated_at=CURRENT_TIMESTAMP
WHERE series_id=?
```

- 不删除任何事项记录（保留历史）。
- open 项仍在列表中可见，但完成时不再生成下一条（§4.2 步骤 1 检查 `active=0` 拦截）。

> **行为变更**：旧代码对 `scope=future_instances` 执行"暂停模板 + 删除当前实例"两步操作。新逻辑仅暂停规则，**不删除当前项**。这是有意简化——暂停 = 停推进，语义更一致。

### 4.4 暂停与恢复（active 语义）

`active` 字段存储在 `todo_series_rules` 表中（series 级别），影响：

- **生成**：`active=0` 时完成事项不触发生成下一条（§4.2 步骤 1 拦截）。
- **提醒**：`dispatch_due_reminders` 查询时 JOIN `todo_series_rules`，`active=0` 的系列不触发提醒。
- **列表**：`item-list` 默认不返回暂停系列的 open 项（`includeInactive=false`）。传 `includeInactive=true` 时返回，前端根据 `recurrence.active=false` 标记显示暂停状态。

**恢复**：`UPDATE todo_series_rules SET active=1 WHERE series_id=?`，恢复后下次完成时正常生成。

### 4.5 去重策略（migration 后多 open 的收敛）

migration 22 后同一 series 可能存在多条 open 项。收敛策略：

- 完成任意一条 → 检查是否还有其它 open → 有则不生成
- 逐条完成直至 series 只剩 1 条 open → 回归稳态
- 不做一次性批量清理，依靠用户自然操作逐步收敛

## 5. 接口行为

### 5.1 `item-list`

**通道**：`tool:todo:item-list`

**请求**：`{ "status": "pending | completed | canceled (可选)", "includeInactive": "boolean (可选，默认 false)" }`

- 不传 `status` → 返回所有项。
- 传 `status` → 按 status 过滤（A1 归一化后过滤，即 `status=pending` 同时包含数据库中的 `pending` 和 `in_progress`）。
- `includeInactive=false`（默认）→ 排除 `todo_series_rules.active=0` 的系列中的 open 项。
- `includeInactive=true` → 同时返回暂停系列的项（用于发现/恢复暂停的系列）。

**行为**：

1. 查询 `todo_items`，对 `kind='recurring'` 的项 LEFT JOIN `todo_series_rules` 获取规则和 `active` 状态。
2. A1 归一化：返回结果中 `in_progress` 映射为 `pending`。
3. 对 `kind='recurring'` 的项，从 `todo_series_rules` 组装 `recurrence` 对象。`rule_json` 在数据库中存储为 TEXT，后端返回时**解析为 JSON 对象**。
4. 已完成/已取消的历史项正常返回。
5. 后端为每条返回项计算 `displayAt`：统一使用 `COALESCE(event_at, created_at)` 作为展示排序时间（不再区分 root/occurrence）。

**返回**：`{ items: TodoItem[] }`

### 5.2 `item-create`

**通道**：`tool:todo:item-create`

**请求**：

```json
{
  "title": "string",
  "typeId": "number | null",
  "priority": "P0 | P1 | P2 | P3",
  "description": "string",
  "eventAt": "ISO8601 | null",
  "kind": "one_off | recurring",
  "recurrence": {
    "ruleMode": "simple | cron",
    "rule": { ... },
    "cronExpression": "string | null",
    "timezone": "string",
    "startAt": "ISO8601",
    "endMode": "never | until_date | after_count",
    "endValue": "string | null"
  },
  "assigneeIds": [1, 2],
  "links": [{ "url": "...", "title": "..." }],
  "reminders": [{ "reminderPreset": "...", "offsetMinutes": 30 }]
}
```

**行为**：

- `one_off`：INSERT `todo_items`（基础字段），`series_id=NULL`。
- `recurring`：
  1. INSERT `todo_items`（基础字段 + `series_id=自身id`）。
  2. INSERT `todo_series_rules`（`series_id=新item.id`，规则字段，`occurrence_index=1`，`active=1`）。若 `ruleMode=simple` 且 `cronExpression` 为 null，后端调用 `build_cron_from_rule(rule)` 计算并存入。
- 同时插入 assignees / reminders / links。

**错误**：

- `title` 为空 → `"标题不能为空"`
- `kind=recurring` 但缺少 `recurrence` → `"重复事项必须提供重复规则"`

### 5.3 `item-update`

**通道**：`tool:todo:item-update`

**请求**：

```json
{
  "id": "number",
  "title": "string (可选)",
  "typeId": "number | null (可选)",
  "priority": "string (可选)",
  "description": "string (可选)",
  "eventAt": "ISO8601 | null (可选)",
  "recurrence": {
    "ruleMode": "simple | cron",
    "rule": { ... },
    "cronExpression": "string | null",
    "timezone": "string",
    "startAt": "ISO8601",
    "endMode": "never | until_date | after_count",
    "endValue": "string | null"
  }
}
```

**行为**：

- `UPDATE todo_items SET ... WHERE id=?`（基础字段）。
- `kind` 字段创建后不可修改。
- 若传入 `recurrence`：`UPDATE todo_series_rules SET ... WHERE series_id=?`（整体替换规则字段）。根据新规则重算 `event_at`：`base_time = now()`。若 `ruleMode=simple` 且 `cronExpression` 为 null，后端先计算 cron。
- 对 `kind=one_off` 的项传入 `recurrence` → 忽略（不报错，因为无对应规则表行）。
- 不再有 scope 概念；编辑当前 pending 项的基础字段即影响当前项，编辑 `recurrence` 影响整个系列未来。

**错误**：

- `id` 不存在 → `"事项不存在"`
- 修改已完成/已取消项的规则字段 → `"已结束的事项不可修改重复规则"`

### 5.4 `item-change-status`

**通道**：`tool:todo:item-change-status`

**请求**：`{ "id": number, "status": "pending | completed | canceled" }`

**行为**：

- 状态转换校验（见 §3.5）。`in_progress` 按 §3.2 视同 `pending`。
- `recurring` + `completed` → 触发 §4.2 完成流程。
- 返回值包含 `nextItemId`（若生成了下一条）。

**返回**：`{ "ok": true, "nextItemId": 42 }` 或 `{ "ok": true }`

**错误**：不合法的状态转换 → `"不允许的状态变更: {旧} -> {新}"`

### 5.5 `item-delete`

**通道**：`tool:todo:item-delete`

**请求**：`{ "id": number, "scope": "this_instance | future_instances" }`

**行为**：

- `scope=this_instance`：执行 §4.3-A 流程。对 `one_off` 项直接删除。对 recurring 且 done 的项直接删除，不触发补生成。
- `scope=future_instances`：执行 §4.3-B 流程（暂停规则）。仅对 `recurring` 有效。
- `one_off` + `scope=future_instances` → 忽略 scope，按普通删除处理。

**返回**：`{ "ok": true }`

**错误**：`id` 不存在 → `"事项不存在"`

### 5.6 `item-toggle-active`

**通道**：`tool:todo:item-toggle-active`

**请求**：`{ "id": number }`

**行为**：

- 读取该项的 `series_id` 和 `kind`。
- `kind=one_off` → 返回错误。
- `kind=recurring` → 切换规则表：`UPDATE todo_series_rules SET active=1-active WHERE series_id=?`。

**返回**：`{ "ok": true, "active": <new_value> }`

### 5.7 `item-upsert`

现有通道保留，行为等价于：有 `id` 时走 `item-update`，无 `id` 时走 `item-create`。

### 5.8 调度器变更（`scheduler_tick`）

**删除** `generate_recurring_instances()` 调用——不再有定时批量生成逻辑。

`scheduler_tick()` 简化为仅调度提醒：

```rust
pub fn scheduler_tick() -> Result<Vec<ReminderDispatch>, String> {
    let conn = db_conn()?;
    dispatch_due_reminders(&conn, Utc::now())
}
```

`dispatch_due_reminders()` 查询 `todo_items` JOIN `todo_item_reminders`，对 `kind='recurring'` 的项额外 JOIN `todo_series_rules` 检查 `active=1`。

### 5.9 其他接口（逻辑不变，底层表切换）

> 以下接口的业务逻辑不变，但 `todo_tasks` 表引用统一切换为 `todo_items`。

- `item-toggle-pin`：`UPDATE todo_items SET pinned=1-pinned WHERE id=?`。
- `item-snooze`：`UPDATE todo_items SET snooze_until=? WHERE id=?`。
- `reminder-list-unread` / `reminder-mark-read`：查询/更新 `todo_reminder_events` 表（`task_id` 列指向 `todo_items.id`，列名保持不变）。
- `open-link`：纯前端行为。

## 6. Migration 23（规则字段拆分）

在 migration 22 之后追加 migration 23，将规则字段从 `todo_items` 提取到 `todo_series_rules`：

1. 创建 `todo_series_rules` 表（schema 见 §2.2）。
2. 从 `todo_items` 中提取规则数据：
   ```sql
   INSERT INTO todo_series_rules (series_id, rule_mode, rule_json, cron_expression,
     timezone, start_at, end_mode, end_value, occurrence_index, active)
   SELECT series_id, rule_mode, rule_json, cron_expression,
     timezone, start_at, end_mode, end_value, occurrence_index, active
   FROM todo_items
   WHERE kind='recurring' AND rule_mode IS NOT NULL
   ```
   > 使用 `rule_mode IS NOT NULL` 而非 `cron_expression IS NOT NULL`，以防 `ruleMode=simple` 但未计算 cron 的边界情况丢失规则数据。migration 22 中每个 series 只有一条 open 实例携带规则字段，所以每个 series 只会 INSERT 一行。
3. 重建 `todo_items` 表，移除规则列（`rule_mode`, `rule_json`, `cron_expression`, `timezone`, `start_at`, `end_mode`, `end_value`, `occurrence_index`, `active`）和 `due_at` 列（该字段已由 `event_at` 取代，前端遗留引用将在 §7 中清理）。使用 SQLite 标准的"创建新表 → 复制数据 → 删旧表 → 重命名"流程。
4. 重建索引和外键。

## 7. 前端变更要点

- 删除 `recordRole`（root / occurrence）概念：所有项统一为普通事项。
- `rootId` 语义变更为 `seriesId`（字段可保持 `rootId` 命名以减少前端改动，值 = `series_id`）。
- 删除 `canEditFuture` 和编辑 scope 选择 UI。
- 删除 `rootMap` 查找逻辑：`recurrence` 信息直接从 `item-list` 返回的 `recurrence` 字段获取（由后端从 `todo_series_rules` 组装）。
- 删除列表中对 `isRootItem` 的过滤（不再有独立根记录需要隐藏）。
- 删除按钮：保留 scope 选择（this_instance / future_instances），语义为"删除范围"。
- 清理 `dueAt` 遗留引用：`TodoPanel.vue` 中 `payload.dueAt = payload.eventAt` 的 fallback 赋值应删除（`due_at` 列在 migration 23 中移除）。

### 7.1 前端 recurrence 对象变更

后端 `item-list` 返回的 `recurrence` 对象字段映射：

| 旧字段             | 新字段 / 处理                                                           |
| ------------------ | ----------------------------------------------------------------------- |
| `nextOccurrenceAt` | **删除**——前端如需展示可从 `event_at` 推导                              |
| `generatedCount`   | 重命名为 `occurrenceIndex`（值 = `occurrence_index`）                   |
| `active`           | **提升为 recurrence 内的独立字段**（来源为 `todo_series_rules.active`） |

### 7.2 其它受影响文件

- `TodoCalendarGrid.vue`：使用 `displayAt` 定位日历格，需确认 `displayAt` 计算逻辑变更后仍兼容。
- `todoBuckets.ts`：删除 `recordRole`、`canEditFuture` 相关分组逻辑。
- `todoBuckets.test.ts`：同步更新测试数据。
- `TodoReminderDispatch` / `TodoReminderEvent` 类型：`taskId` 字段名保持不变。

## 8. 错误处理策略

- 所有写操作在单个 SQLite 事务中执行。
- 生成下一条失败时：事务回滚，完成操作本身也撤销，返回错误给前端。
- 前端收到错误字符串后展示 toast 提示，不做静默吞错。

## 9. 非目标（本次不做）

- 不引入新的状态值（受 migration 约束）
- 不在本轮为 `in_progress` 设计独立业务含义
- 不实现系列恢复的专用 UI（通过 `item-toggle-active` 可恢复）

---

确认记录（用户已确认）：

- canceled 不推进、不生成
- 完成时若 series 已存在其它 open：不生成
- 删除 this_instance 且删除后无 open：补生成 1 条
- 终止系列：暂停规则（`todo_series_rules.active=0`）
- active=0：停推进 + 停提醒
- 状态收敛：A1（in_progress 映射为 pending）
- 撤销 done→pending：仅 one_off 允许
- 规则字段存储：独立表 `todo_series_rules`（series 级别，消除 promote 算法）
