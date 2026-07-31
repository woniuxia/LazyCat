# 动作中心与打包提醒实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 建立无独立入口的通用动作中心，并交付首个 `Todo/提醒 -> 上线包打包` 动作：用户确认后启动已有上线包配置，完整成功时自动完成 Todo。

**Architecture:** Rust 动作中心持有静态动作定义、通用绑定和 dispatch 状态机；Todo 只保存触发对象并在同一事务内维护绑定，上线包仍独占配置、确认、执行和终态。前端用独立 dispatch intent 在主窗口路由动作请求，复用现有上线包确认链，不复制配置或秘密，也不绕过覆盖、SSH、Vault 和远程预检。

**Tech Stack:** Tauri 2、Rust、rusqlite、serde、Vue 3、TypeScript、Element Plus、Vitest、pnpm

---

## 文件职责与落点

**新增文件**

- `apps/desktop/src-tauri/src/tools/action_center/mod.rs`：动作中心 IPC 分发和供 Todo/上线包调用的稳定内部入口。
- `apps/desktop/src-tauri/src/tools/action_center/definitions.rs`：静态动作定义、目标适配器和目标摘要。
- `apps/desktop/src-tauri/src/tools/action_center/bindings.rs`：通用绑定解析、校验、事务保存和摘要查询。
- `apps/desktop/src-tauri/src/tools/action_center/dispatches.rs`：dispatch 创建、状态转换、外部 run 关联、终态联动和启动恢复。
- `apps/desktop/src/types/action-center.ts`：前端动作定义、目标、绑定、dispatch 和 intent 契约。
- `apps/desktop/src/composables/useTodoActionBinding.ts`：Todo 动作/目标选择、最近 dispatch 查询和手动派发。
- `apps/desktop/src/composables/useTodoActionBinding.test.ts`：Todo 动作选择的真实逻辑测试。
- `apps/desktop/src/composables/useActionDispatchIntent.ts`：与剪贴板无关的模块级 dispatch intent 交接。
- `apps/desktop/src/composables/useActionDispatchIntent.test.ts`：intent 的目标匹配、消费和替换测试。
- `apps/desktop/src/composables/useTodoItem.test.ts`：Todo 动作绑定归一化测试。
- `apps/desktop/src/components/todo/TodoActionBinding.test.ts`：Todo 编辑、详情和面板动作接线契约测试。

**主要修改文件**

- `apps/desktop/src-tauri/src/tools/helpers.rs`：初始化动作中心表和索引。
- `apps/desktop/src-tauri/src/tools/mod.rs`、`contract_tests.rs`：注册 action_center 域并守卫 IPC/事件契约。
- `apps/desktop/src-tauri/src/tools/todo/{mod.rs,items.rs,reminders.rs,types.rs}`：原子保存绑定、返回摘要、提醒携带动作、复用完成语义。
- `apps/desktop/src-tauri/src/tools/release_package.rs`、`release_package_runtime.rs`：目标适配、启动前绑定 run、终态回写。
- `apps/desktop/src-tauri/src/{events.rs,global_notification.rs,main.rs}`：dispatch 请求事件、通知负载和启动恢复。
- `apps/desktop/src/{bridge/tauri.ts,bridge/events.ts,App.vue}`：IPC channel、事件名和主窗口 intent 路由。
- `apps/desktop/src/types/{index.ts,todo.ts,global-notification.ts}`：集中导出跨层类型。
- `apps/desktop/src/composables/{useTodoItem.ts,useTodoScheduleFields.ts,useTodoDetailState.ts,useTodoCrudActions.ts}`：draft、归一化、保存校验和生命周期。
- `apps/desktop/src/components/todo/{TodoPanel.vue,TodoDetailEdit.vue,TodoDetailView.vue}`：动作配置、状态展示和手动触发。
- `apps/desktop/src/components/GlobalNotificationPopup.vue`、`apps/desktop/src/utils/globalNotification.ts`：提醒主操作切换为“开始打包”。
- `apps/desktop/src/components/ReleasePackagePanel.vue`：消费 intent、保护 dirty draft、沿现有确认链传递 dispatch ID。

## 统一契约

实现时以下命名不可漂移：

```ts
type ActionDispatchStatus =
  | "pending_confirmation"
  | "running"
  | "succeeded"
  | "failed"
  | "cancelled";

interface ActionBindingInput {
  actionType: string;
  targetId: string;
}

interface ActionDispatchRequest {
  dispatchId: string;
  actionType: string;
  targetToolId: string;
  targetId: string;
}
```

`dispatch-cancel` 只结束尚未进入运行态的确认请求。payload 使用 `outcome: 'cancelled' | 'failed'` 区分用户取消与页面/预检失败；进入 `running` 后的取消仍由上线包 `cancel` 产生原始终态，再由动作中心映射。

---

### Task 1: 建立动作定义注册表、目标适配器和数据库骨架

**Files:**

- Create: `apps/desktop/src-tauri/src/tools/action_center/mod.rs`
- Create: `apps/desktop/src-tauri/src/tools/action_center/definitions.rs`
- Create: `apps/desktop/src-tauri/src/tools/action_center/bindings.rs`
- Create: `apps/desktop/src-tauri/src/tools/action_center/dispatches.rs`
- Modify: `apps/desktop/src-tauri/src/tools/helpers.rs`
- Modify: `apps/desktop/src-tauri/src/tools/release_package.rs`
- Modify: `apps/desktop/src-tauri/src/tools/mod.rs`
- Modify: `apps/desktop/src-tauri/src/tools/contract_tests.rs`
- Modify: `apps/desktop/src/bridge/tauri.ts`

- [ ] **Step 1: 写动作定义、目标列表和 schema 失败测试**

在 `definitions.rs` 的测试模块固定第一版定义与已有上线包配置适配行为，在 `action_center/mod.rs` 测试两个表和活动唯一索引：

```rust
#[test]
fn release_package_definition_is_registered() {
    let definition = definition("release_package.run").expect("registered action");
    assert_eq!(definition.trigger_types, &["todo_item"]);
    assert_eq!(definition.target_kind, "release_package_project");
    assert_eq!(definition.target_tool_id, "release-package");
    assert_eq!(definition.execution_mode, "open_and_confirm");
    assert_eq!(definition.completion_policy, "on_succeeded");
}

#[test]
fn release_package_targets_only_return_saved_projects() {
    let conn = test_conn();
    seed_release_project(&conn, 7, "客户门户");
    assert_eq!(list_targets(&conn, "release_package.run").unwrap(), vec![
        ActionTargetOption {
            id: "7".into(),
            label: "客户门户".into(),
            available: true,
            unavailable_reason: None,
        },
    ]);
}

#[test]
fn action_center_schema_has_active_dispatch_uniqueness() {
    let conn = Connection::open_in_memory().unwrap();
    ensure_schema(&conn).unwrap();
    let sql: String = conn.query_row(
        "SELECT sql FROM sqlite_master WHERE name='idx_action_dispatches_one_active_binding'",
        [],
        |row| row.get(0),
    ).unwrap();
    assert!(sql.contains("pending_confirmation"));
    assert!(sql.contains("running"));
}
```

- [ ] **Step 2: 运行测试并确认按预期失败**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml action_center -- --nocapture`

Expected: FAIL，提示 `action_center` 模块、`definition`、`list_targets` 或 `ensure_schema` 尚不存在。

- [ ] **Step 3: 实现最小定义注册表、目标适配器和 schema**

`definitions.rs` 使用代码静态注册，不读数据库中的任意动作定义：

```rust
pub(crate) const RELEASE_PACKAGE_RUN: &str = "release_package.run";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ActionDefinition {
    pub action_type: &'static str,
    pub label: &'static str,
    pub trigger_types: &'static [&'static str],
    pub target_kind: &'static str,
    pub target_tool_id: &'static str,
    pub execution_mode: &'static str,
    pub completion_policy: &'static str,
}

pub(crate) fn definition(action_type: &str) -> Option<ActionDefinition> {
    match action_type {
        RELEASE_PACKAGE_RUN => Some(ActionDefinition {
            action_type: RELEASE_PACKAGE_RUN,
            label: "开始打包",
            trigger_types: &["todo_item"],
            target_kind: "release_package_project",
            target_tool_id: "release-package",
            execution_mode: "open_and_confirm",
            completion_policy: "on_succeeded",
        }),
        _ => None,
    }
}
```

在 `release_package.rs` 暴露窄接口 `list_action_target_rows(&Connection) -> Result<Vec<(i64, String)>, String>` 与 `load_action_target_label(&Connection, i64) -> Result<Option<String>, String>`；`definitions.rs` 只通过这两个接口生成 `ActionTargetOption`，不解析或复制上线包配置。

`action_center/mod.rs` 定义并由 `helpers.rs::ensure_schema` 调用：

```rust
pub(crate) const ACTION_CENTER_SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS action_bindings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    trigger_type TEXT NOT NULL,
    trigger_id TEXT NOT NULL,
    action_type TEXT NOT NULL,
    target_id TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(trigger_type, trigger_id)
);
CREATE TABLE IF NOT EXISTS action_dispatches (
    id TEXT PRIMARY KEY,
    binding_id INTEGER NULL REFERENCES action_bindings(id) ON DELETE SET NULL,
    trigger_type TEXT NOT NULL,
    trigger_id TEXT NOT NULL,
    trigger_event_id TEXT NOT NULL,
    action_type TEXT NOT NULL,
    target_id TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('pending_confirmation','running','succeeded','failed','cancelled')),
    external_run_id TEXT NULL,
    result_code TEXT NULL,
    error TEXT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    started_at TEXT NULL,
    finished_at TEXT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_action_dispatches_one_active_binding
ON action_dispatches(binding_id)
WHERE binding_id IS NOT NULL AND status IN ('pending_confirmation','running');
CREATE UNIQUE INDEX IF NOT EXISTS idx_action_dispatches_external_run
ON action_dispatches(external_run_id) WHERE external_run_id IS NOT NULL;
"#;
```

注册 `action_center` domain，第一批 `supported_actions()` 仅含 `definition_list`、`target_list`；同步 `DOMAINS` 与 channel：

```ts
"tool:action-center:definition-list": { domain: "action_center", action: "definition_list" },
"tool:action-center:target-list": { domain: "action_center", action: "target_list" },
```

- [ ] **Step 4: 运行定向测试和 IPC 契约测试**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml action_center -- --nocapture`

Expected: PASS，动作定义、目标列表和 schema 测试全部通过。

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml contract_tests -- --nocapture`

Expected: PASS，`action_center` domain 与两个 channel 完成对账。

- [ ] **Step 5: 提交动作中心骨架**

```powershell
git add apps/desktop/src-tauri/src/tools/action_center apps/desktop/src-tauri/src/tools/helpers.rs apps/desktop/src-tauri/src/tools/release_package.rs apps/desktop/src-tauri/src/tools/mod.rs apps/desktop/src-tauri/src/tools/contract_tests.rs apps/desktop/src/bridge/tauri.ts
git commit -m "feat: 添加动作中心定义与目标适配"
```

---

### Task 2: 实现通用绑定并让 Todo 与绑定原子保存

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/action_center/bindings.rs`
- Modify: `apps/desktop/src-tauri/src/tools/action_center/mod.rs`
- Modify: `apps/desktop/src-tauri/src/tools/todo/items.rs`
- Modify: `apps/desktop/src-tauri/src/tools/todo/mod.rs`
- Modify: `apps/desktop/src/bridge/tauri.ts`

- [ ] **Step 1: 写绑定校验、事务回滚和周期限制失败测试**

在 Todo 测试连接中补 `action_center::ensure_schema` 和上线包测试配置，然后增加：

```rust
#[test]
fn one_off_create_rolls_back_when_action_target_is_invalid() {
    let mut conn = create_test_conn();
    let error = item_create_with_conn(&mut conn, &json!({
        "title": "发布客户门户",
        "kind": "one_off",
        "actionBinding": { "actionType": "release_package.run", "targetId": "999" }
    })).unwrap_err();
    assert!(error.contains("上线包配置不存在"));
    assert_eq!(table_count(&conn, "todo_items"), 0);
    assert_eq!(table_count(&conn, "action_bindings"), 0);
}

#[test]
fn one_off_update_rolls_back_item_fields_when_binding_fails() {
    let mut conn = create_test_conn();
    seed_one_off(&conn, 1, "旧标题");
    item_update_with_conn(&mut conn, &json!({
        "id": 1,
        "kind": "one_off",
        "title": "新标题",
        "actionBinding": { "actionType": "release_package.run", "targetId": "999" }
    })).unwrap_err();
    assert_eq!(item_title(&conn, 1), "旧标题");
}

#[test]
fn recurring_item_rejects_action_binding() {
    let mut conn = create_test_conn();
    let error = item_create_with_conn(&mut conn, &recurring_payload_with_action()).unwrap_err();
    assert_eq!(error, "周期事项暂不支持执行动作");
}
```

同时覆盖字段三态：创建时缺失/`null` 都无绑定；更新时缺失保留、对象替换、`null` 解除；活动 dispatch 存在时替换和解除均失败。

- [ ] **Step 2: 运行 Todo 和绑定测试并确认失败**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml todo -- --nocapture`

Expected: FAIL，提示 `item_create_with_conn`、`item_update_with_conn`、绑定解析或摘要接口尚不存在。

- [ ] **Step 3: 实现 BindingPatch 和 Todo 单事务写入**

`bindings.rs` 明确保留字段缺失语义：

```rust
pub(crate) enum BindingPatch {
    Preserve,
    Remove,
    Set { action_type: String, target_id: String },
}

pub(crate) fn parse_binding_patch(payload: &Value) -> Result<BindingPatch, String> {
    match payload.get("actionBinding") {
        None => Ok(BindingPatch::Preserve),
        Some(Value::Null) => Ok(BindingPatch::Remove),
        Some(Value::Object(value)) => {
            let action_type = value.get("actionType").and_then(Value::as_str)
                .map(str::trim).filter(|value| !value.is_empty())
                .ok_or("actionBinding.actionType 不能为空")?;
            let target_id = value.get("targetId").and_then(Value::as_str)
                .map(str::trim).filter(|value| !value.is_empty())
                .ok_or("actionBinding.targetId 不能为空")?;
            Ok(BindingPatch::Set { action_type: action_type.into(), target_id: target_id.into() })
        }
        Some(_) => Err("actionBinding 必须是对象或 null".into()),
    }
}
```

把 `item_create`、`item_update` 改为只负责打开连接，所有写操作下沉到可测试函数并包在单一事务中：

```rust
pub(crate) fn item_create(payload: &Value) -> Result<Value, String> {
    let mut conn = db_conn()?;
    item_create_with_conn(&mut conn, payload)
}

pub(crate) fn item_create_with_conn(conn: &mut Connection, payload: &Value) -> Result<Value, String> {
    let binding_patch = parse_binding_patch(payload)?;
    let tx = conn.transaction().map_err(|error| format!("开启事务失败: {error}"))?;
    let item_id = insert_item_and_support_data(&tx, payload)?;
    apply_todo_binding_patch(&tx, item_id, parse_item_kind(payload).as_str(), binding_patch, true)?;
    tx.commit().map_err(|error| format!("提交事务失败: {error}"))?;
    Ok(json!({ "ok": true, "id": item_id, "rootId": item_id }))
}
```

`item_update_with_conn` 同样将类型切换、基础字段、`project_id`、执行人、链接、提醒、周期规则和绑定放入一个事务。删除现有两处忽略结果的 `let _ = conn.execute("UPDATE todo_items SET project_id...`，改成事务内 `.map_err(...) ?`。在 `one_off -> recurring` 写入前检查现有绑定或 payload 对象，要求用户先解除绑定。

`item_list` 提取 `item_list_with_conn`，由 `attach_todo_binding_summaries(&conn, &mut items)` 添加：

```json
{
  "id": 12,
  "actionType": "release_package.run",
  "actionLabel": "开始打包",
  "targetId": "7",
  "targetLabel": "客户门户",
  "available": true
}
```

目标删除后保持绑定，返回 `available: false` 和 `unavailableReason: "上线包配置不存在"`。Todo 删除时显式删除 `trigger_type='todo_item'` 的绑定；dispatch 依靠 `ON DELETE SET NULL` 保留快照。

新增 `binding_get` action 和 channel：

```ts
"tool:action-center:binding-get": { domain: "action_center", action: "binding_get" },
```

- [ ] **Step 4: 运行 Todo、动作中心和契约测试**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml todo -- --nocapture`

Expected: PASS，原子回滚、三态更新、周期拒绝、删除和摘要测试通过。

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml action_center -- --nocapture`

Expected: PASS，绑定新增、替换、解除、失效摘要和活动保护测试通过。

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml contract_tests -- --nocapture`

Expected: PASS，`binding_get` 前后端契约一致。

- [ ] **Step 5: 提交原子绑定实现**

```powershell
git add apps/desktop/src-tauri/src/tools/action_center apps/desktop/src-tauri/src/tools/todo apps/desktop/src/bridge/tauri.ts
git commit -m "feat: 支持 Todo 原子绑定动作"
```

---

### Task 3: 实现 dispatch 状态机、重复保护和启动恢复

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/action_center/dispatches.rs`
- Modify: `apps/desktop/src-tauri/src/tools/action_center/mod.rs`
- Modify: `apps/desktop/src-tauri/src/tools/mod.rs`
- Modify: `apps/desktop/src-tauri/src/events.rs`
- Modify: `apps/desktop/src-tauri/src/main.rs`
- Modify: `apps/desktop/src/bridge/tauri.ts`
- Modify: `apps/desktop/src/bridge/events.ts`

- [ ] **Step 1: 写 dispatch 创建、转换、幂等和恢复失败测试**

将带 AppHandle 的部分保持薄包装，核心用连接测试：

```rust
#[test]
fn same_binding_can_only_have_one_active_dispatch() {
    let mut conn = seeded_action_conn();
    let first = create_dispatch_with_conn(&mut conn, &manual_request(1)).unwrap();
    let error = create_dispatch_with_conn(&mut conn, &manual_request(1)).unwrap_err();
    assert!(error.contains("已有待确认或进行中的动作"));
    assert_eq!(first.status, "pending_confirmation");
}

#[test]
fn reminder_event_must_belong_to_the_trigger_todo() {
    let mut conn = seeded_action_conn();
    seed_reminder_event(&conn, 41, 2);
    let error = create_dispatch_with_conn(&mut conn, &reminder_request(1, 41)).unwrap_err();
    assert_eq!(error, "提醒事件与当前任务不匹配");
}

#[test]
fn pending_dispatch_can_end_as_cancelled_or_failed_but_not_running_via_cancel_api() {
    let mut conn = seeded_action_conn();
    let dispatch = create_dispatch_with_conn(&mut conn, &manual_request(1)).unwrap();
    stop_pending_with_conn(&mut conn, &dispatch.id, "failed", Some("页面有未保存配置")).unwrap();
    assert_dispatch(&conn, &dispatch.id, "failed", Some("页面有未保存配置"));
}

#[test]
fn startup_recovery_marks_orphaned_active_dispatches_interrupted() {
    let mut conn = seeded_action_conn();
    seed_dispatch(&conn, "pending", "pending_confirmation");
    seed_dispatch(&conn, "running", "running");
    assert_eq!(recover_interrupted_with_conn(&mut conn).unwrap(), 2);
    assert_interrupted(&conn, "pending");
    assert_interrupted(&conn, "running");
}
```

另测：目标失效拒绝派发、Todo 已完成拒绝派发、重复终态不改写、错误 `runId` 不匹配、绑定删除后历史 dispatch 保留。

- [ ] **Step 2: 运行测试并确认失败**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml action_center -- --nocapture`

Expected: FAIL，提示 dispatch helper、状态类型或恢复函数尚不存在。

- [ ] **Step 3: 实现事务状态机与 app-context 派发**

定义唯一状态转换并让所有写入口复用：

```rust
fn can_transition(current: &str, next: &str) -> bool {
    matches!(
        (current, next),
        ("pending_confirmation", "running")
            | ("pending_confirmation", "failed")
            | ("pending_confirmation", "cancelled")
            | ("running", "succeeded")
            | ("running", "failed")
            | ("running", "cancelled")
    )
}
```

`create_dispatch_with_conn` 使用 `TransactionBehavior::Immediate`，重新读取 Todo、绑定、动作定义、目标和活动 dispatch。提醒来源校验 `todo_reminder_events.id + task_id`，手动来源由后端生成新的 UUID 作为 `trigger_event_id`；成功创建提醒 dispatch 时把对应提醒事件标为已读。

`dispatch_with_app` 提交数据库后执行：

```rust
let intent = ActionDispatchRequest {
    dispatch_id: dispatch.id.clone(),
    action_type: dispatch.action_type.clone(),
    target_tool_id: definition.target_tool_id.to_string(),
    target_id: dispatch.target_id.clone(),
};
if let Err(error) = crate::navigate_main_window_to_tool(app, &intent.target_tool_id)
    .and_then(|_| emit_dispatch_request(app, &intent))
{
    fail_pending_dispatch(&dispatch.id, &error)?;
    return Err(error);
}
```

新增事件并同步契约：

```rust
pub const EVENT_ACTION_CENTER_DISPATCH_REQUEST: &str = "action-center://dispatch-request";
```

```ts
ACTION_CENTER_DISPATCH_REQUEST: "action-center://dispatch-request",
```

`tools::execute_tool_with_app` 将 `action_center` 交给 `execute_with_app`。最终 actions 和 channel 增加：

```text
dispatch
dispatch_cancel
dispatch_latest
```

```ts
"tool:action-center:dispatch": { domain: "action_center", action: "dispatch" },
"tool:action-center:dispatch-cancel": { domain: "action_center", action: "dispatch_cancel" },
"tool:action-center:dispatch-latest": { domain: "action_center", action: "dispatch_latest" },
```

`dispatch_cancel` 仅允许 `pending_confirmation`，`outcome` 只接受 `cancelled/failed`；`failed` 必须携带非空 `error`。`dispatch_latest` 按 `created_at DESC, rowid DESC` 返回触发对象最近一次记录。

在 `main.rs::setup` 启动调度器前调用 `recover_interrupted_dispatches()`；错误使用 `eprintln!("action-center recovery failed: {error}")` 明确暴露，不能阻止应用启动，也不自动重试或完成 Todo。

- [ ] **Step 4: 运行状态机和契约测试**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml action_center -- --nocapture`

Expected: PASS，创建、重复保护、状态转换、提醒校验、删除和恢复测试通过。

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml contract_tests -- --nocapture`

Expected: PASS，6 个 action-center channel 和新事件完成对账。

- [ ] **Step 5: 提交 dispatch 状态机**

```powershell
git add apps/desktop/src-tauri/src/tools/action_center apps/desktop/src-tauri/src/tools/mod.rs apps/desktop/src-tauri/src/events.rs apps/desktop/src-tauri/src/main.rs apps/desktop/src/bridge/tauri.ts apps/desktop/src/bridge/events.ts
git commit -m "feat: 添加动作派发状态机"
```

---

### Task 4: 增加 Todo 动作配置、详情状态和手动触发

**Files:**

- Create: `apps/desktop/src/types/action-center.ts`
- Create: `apps/desktop/src/composables/useTodoActionBinding.ts`
- Create: `apps/desktop/src/composables/useTodoActionBinding.test.ts`
- Create: `apps/desktop/src/composables/useTodoItem.test.ts`
- Create: `apps/desktop/src/components/todo/TodoActionBinding.test.ts`
- Modify: `apps/desktop/src/types/index.ts`
- Modify: `apps/desktop/src/types/todo.ts`
- Modify: `apps/desktop/src/composables/useTodoItem.ts`
- Modify: `apps/desktop/src/composables/useTodoScheduleFields.ts`
- Modify: `apps/desktop/src/composables/useTodoDetailState.ts`
- Modify: `apps/desktop/src/composables/useTodoCrudActions.ts`
- Modify: `apps/desktop/src/components/todo/TodoPanel.vue`
- Modify: `apps/desktop/src/components/todo/TodoDetailEdit.vue`
- Modify: `apps/desktop/src/components/todo/TodoDetailView.vue`

- [ ] **Step 1: 写类型归一化、联动和 UI 接线失败测试**

真实纯函数测试失效目标和空绑定：

```ts
it("normalizes an unavailable action binding without inventing a target", () => {
  const item = normalizeTodoItem({
    id: 1,
    kind: "one_off",
    actionBinding: {
      id: 9,
      actionType: "release_package.run",
      actionLabel: "开始打包",
      targetId: "404",
      targetLabel: "配置 #404",
      available: false,
      unavailableReason: "上线包配置不存在",
    },
  });
  expect(item.actionBinding?.available).toBe(false);
  expect(item.actionBinding?.targetId).toBe("404");
});
```

`useTodoActionBinding.test.ts` mock `invokeToolByChannel`，覆盖：定义加载后选择动作再加载目标；清空动作同时清空目标；派发 payload 使用字符串 `triggerId`；最近 dispatch 查询使用 `todo_item`。

`TodoActionBinding.test.ts` 对源码契约断言：周期事项不渲染动作字段、编辑区存在动作/目标选择、无配置可打开 `release-package`、详情区显示最近状态和“开始打包”。

- [ ] **Step 2: 运行前端定向测试并确认失败**

Run: `pnpm --filter @lazycat/desktop test -- src/composables/useTodoItem.test.ts src/composables/useTodoActionBinding.test.ts src/components/todo/TodoActionBinding.test.ts`

Expected: FAIL，提示动作类型、composable 或 UI 接线尚不存在。

- [ ] **Step 3: 实现集中类型、draft 生命周期和 UI**

`types/action-center.ts` 定义完整前端契约：

```ts
export type ActionDispatchStatus =
  | "pending_confirmation"
  | "running"
  | "succeeded"
  | "failed"
  | "cancelled";

export interface ActionDefinition {
  actionType: string;
  label: string;
  triggerTypes: string[];
  targetKind: string;
  targetToolId: string;
  executionMode: "open_and_confirm" | "direct" | "background";
  completionPolicy: "on_started" | "on_succeeded" | "manual";
}

export interface ActionTargetOption {
  id: string;
  label: string;
  available: boolean;
  unavailableReason?: string;
}

export interface ActionBindingInput {
  actionType: string;
  targetId: string;
}

export interface ActionBindingSummary {
  id: number;
  actionType: string;
  actionLabel: string;
  targetId: string;
  targetLabel: string;
  available: boolean;
  unavailableReason?: string;
}

export interface ActionDispatchSummary {
  id: string;
  triggerType: string;
  triggerId: string;
  actionType: string;
  targetId: string;
  status: ActionDispatchStatus;
  resultCode?: string;
  error?: string;
  createdAt: string;
  startedAt?: string;
  finishedAt?: string;
}

export interface ActionDispatchRequest {
  dispatchId: string;
  actionType: string;
  targetToolId: string;
  targetId: string;
}
```

`TodoItem` 增加 `actionBinding?: ActionBindingSummary | null`；`TodoItemUpsertPayload` 增加 `actionBinding?: ActionBindingInput | null`。`TodoItemDraft` 增加：

```ts
actionType: string | null;
actionTargetId: string | null;
```

`resetItemDraft`、`applyItemToDraft`、`snapshotItemDraft` 必须同步两个字段。`submitItemChanges` 在 `isRepeating` 时提交 `actionBinding: null`，在单次事项时按 draft 生成对象或 `null`；选择动作但未选择可用目标时明确 `ElMessage.warning("请选择打包配置")` 并停止保存。

`useTodoActionBinding` 暴露小接口：

```ts
const {
  actionDefinitions,
  actionTargets,
  latestDispatch,
  loadDefinitions,
  loadTargets,
  onActionTypeChange,
  loadLatestDispatch,
  dispatchTodoAction,
} = useTodoActionBinding(itemDraft);
```

编辑区只在 `draft.repeatPreset === 'none'` 显示“执行动作”和“打包配置”。没有目标时展示“暂无上线包配置”与“前往上线包”；失效绑定保留原 `targetId/targetLabel` 并明确标红，不回退第一项。

详情卡展示动作、目标、最近 dispatch 状态/错误，并在以下条件禁用主按钮：目标失效、Todo 已完成、最近状态是 `pending_confirmation/running`。点击调用：

```ts
await dispatchTodoAction(item, { triggerEventId: undefined });
await loadLatestDispatch(item.id);
```

周期事项切换前若 draft 有动作，`onRepeatPresetChange` 先确认解除；用户取消则恢复原重复预设和动作字段。

- [ ] **Step 4: 运行 Todo 前端测试、类型检查**

Run: `pnpm --filter @lazycat/desktop test -- src/composables/useTodoItem.test.ts src/composables/useTodoActionBinding.test.ts src/components/todo/TodoActionBinding.test.ts src/components/todo/TodoDetailView.layout.test.ts`

Expected: PASS，归一化、动作/目标联动、失效配置、周期限制、手动派发和详情状态测试通过。

Run: `pnpm typecheck`

Expected: PASS，无 draft、props、emit 或集中类型漂移。

- [ ] **Step 5: 提交 Todo 动作 UI**

```powershell
git add apps/desktop/src/types apps/desktop/src/composables/useTodoItem.ts apps/desktop/src/composables/useTodoItem.test.ts apps/desktop/src/composables/useTodoScheduleFields.ts apps/desktop/src/composables/useTodoDetailState.ts apps/desktop/src/composables/useTodoCrudActions.ts apps/desktop/src/composables/useTodoActionBinding.ts apps/desktop/src/composables/useTodoActionBinding.test.ts apps/desktop/src/components/todo
git commit -m "feat: 添加 Todo 动作配置与手动触发"
```

---

### Task 5: 让 Todo 提醒携带动作并显示“开始打包”

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/todo/types.rs`
- Modify: `apps/desktop/src-tauri/src/tools/todo/reminders.rs`
- Modify: `apps/desktop/src-tauri/src/tools/todo/mod.rs`
- Modify: `apps/desktop/src-tauri/src/global_notification.rs`
- Modify: `apps/desktop/src/types/todo.ts`
- Modify: `apps/desktop/src/types/global-notification.ts`
- Modify: `apps/desktop/src/utils/globalNotification.ts`
- Modify: `apps/desktop/src/utils/globalNotification.test.ts`
- Modify: `apps/desktop/src/components/GlobalNotificationPopup.vue`
- Modify: `apps/desktop/src/components/GlobalNotificationPopup.test.ts`

- [ ] **Step 1: 写提醒负载、通知验证和按钮行为失败测试**

Rust 测试在已有 `dispatch_due_reminders_should_include_priority_in_payload` 基础上增加绑定，并断言通用动作摘要：

```rust
assert_eq!(reminders[0].action.as_ref().unwrap().action_type, "release_package.run");
assert_eq!(reminders[0].action.as_ref().unwrap().target_label, "客户门户");
assert_eq!(reminders[0].action.as_ref().unwrap().active_dispatch_status, None);
```

再分别 seed `pending_confirmation`、`running` 和已删除目标，断言活动状态与 `available=false`。

前端通知测试增加：普通提醒动作仍为 `complete/dismiss/snooze`；绑定动作提醒为 `dispatch-action/dismiss/snooze`；缺少 `bindingId/actionType/actionLabel/targetLabel/available` 的动作摘要必须被拒绝。

Popup 契约断言“开始打包”、`tool:action-center:dispatch`、活动状态文案和不可用原因。

- [ ] **Step 2: 运行 Rust/前端提醒测试并确认失败**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml dispatch_due_reminders -- --nocapture`

Expected: FAIL，`ReminderDispatch.action` 尚不存在。

Run: `pnpm --filter @lazycat/desktop test -- src/utils/globalNotification.test.ts src/components/GlobalNotificationPopup.test.ts`

Expected: FAIL，动作提醒类型和按钮尚不存在。

- [ ] **Step 3: 实现通用提醒动作摘要和 popup 派发**

Rust 类型使用可选字段保持普通提醒不变：

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderActionSummary {
    pub binding_id: i64,
    pub action_type: String,
    pub action_label: String,
    pub target_label: String,
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unavailable_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_dispatch_status: Option<String>,
}
```

`dispatch_due_reminders` 创建事件后调用动作中心只读摘要查询，不根据摘要执行任何动作。`GlobalNotification::TodoReminder` 和 `todo_notifications` 透传 `action: Option<ReminderActionSummary>`。

前端 `TodoReminderNotification` 增加同构 `action?`。Popup 主按钮逻辑：

```ts
const todoPrimaryLabel = computed(() => {
  const action = currentTodo.value?.action;
  if (!action) return "完成";
  if (action.activeDispatchStatus === "pending_confirmation") return "打包待确认";
  if (action.activeDispatchStatus === "running") return "打包进行中";
  return action.actionLabel;
});

async function runCurrentReminderAction() {
  const item = currentTodo.value;
  if (!item?.action?.available || item.action.activeDispatchStatus) return;
  await runAction(() =>
    invokeToolByChannel("tool:action-center:dispatch", {
      triggerType: "todo_item",
      triggerId: String(item.taskId),
      triggerEventId: String(item.eventId),
    }).then(() => undefined),
  );
}
```

普通提醒继续调用 `reminder_popup_complete`；动作提醒替换“完成”为动作主按钮。“知道了”和“稍后提醒”保持原行为。目标失效时禁用主按钮并在卡片中显示 `unavailableReason`。

- [ ] **Step 4: 运行提醒和通知测试**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml dispatch_due_reminders -- --nocapture`

Expected: PASS，普通、动作、活动和失效提醒摘要均正确。

Run: `pnpm --filter @lazycat/desktop test -- src/utils/globalNotification.test.ts src/components/GlobalNotificationPopup.test.ts`

Expected: PASS，通知严格校验和按钮分支通过。

- [ ] **Step 5: 提交打包提醒**

```powershell
git add apps/desktop/src-tauri/src/tools/todo apps/desktop/src-tauri/src/global_notification.rs apps/desktop/src/types/todo.ts apps/desktop/src/types/global-notification.ts apps/desktop/src/utils/globalNotification.ts apps/desktop/src/utils/globalNotification.test.ts apps/desktop/src/components/GlobalNotificationPopup.vue apps/desktop/src/components/GlobalNotificationPopup.test.ts
git commit -m "feat: 添加任务触发打包提醒"
```

---

### Task 6: 用独立 intent 路由动作请求到主窗口目标工具

**Files:**

- Create: `apps/desktop/src/composables/useActionDispatchIntent.ts`
- Create: `apps/desktop/src/composables/useActionDispatchIntent.test.ts`
- Modify: `apps/desktop/src/types/action-center.ts`
- Modify: `apps/desktop/src/App.vue`
- Modify: `apps/desktop/src/components/ReleasePackagePanel.test.ts`

- [ ] **Step 1: 写 intent 消费和 App 路由失败测试**

```ts
it("only consumes an intent from its target tool", () => {
  const center = useActionDispatchIntent();
  center.setPendingIntent({
    dispatchId: "dispatch-1",
    actionType: "release_package.run",
    targetToolId: "release-package",
    targetId: "7",
  });
  expect(center.consumePendingIntent("todo")).toBeNull();
  expect(center.consumePendingIntent("release-package")?.dispatchId).toBe("dispatch-1");
  expect(center.consumePendingIntent("release-package")).toBeNull();
});
```

在 `ReleasePackagePanel.test.ts` 的 App 源码契约中断言监听 `APP_EVENTS.ACTION_CENTER_DISPATCH_REQUEST`、调用 `setPendingIntent` 后再 `onSelect(payload.targetToolId)`，且不调用 `setPendingToolInput`。

- [ ] **Step 2: 运行 intent 测试并确认失败**

Run: `pnpm --filter @lazycat/desktop test -- src/composables/useActionDispatchIntent.test.ts src/components/ReleasePackagePanel.test.ts`

Expected: FAIL，独立 intent composable 和 App listener 尚不存在。

- [ ] **Step 3: 实现模块级单例 intent 和主窗口监听**

`useActionDispatchIntent.ts` 沿用项目内模块级状态模式，但不引入剪贴板字段：

```ts
const pendingIntent = ref<ActionDispatchRequest | null>(null);

export function useActionDispatchIntent() {
  function setPendingIntent(intent: ActionDispatchRequest) {
    pendingIntent.value = intent;
  }
  function consumePendingIntent(toolId: string) {
    if (pendingIntent.value?.targetToolId !== toolId) return null;
    const current = pendingIntent.value;
    pendingIntent.value = null;
    return current;
  }
  function watchPendingIntent(
    toolId: string,
    apply: (intent: ActionDispatchRequest) => void | Promise<void>,
  ) {
    onMounted(() => {
      const current = consumePendingIntent(toolId);
      if (current) void apply(current);
    });
    watch(pendingIntent, (value) => {
      if (value?.targetToolId !== toolId) return;
      const current = consumePendingIntent(toolId);
      if (current) void apply(current);
    });
  }
  return { pendingIntent, setPendingIntent, consumePendingIntent, watchPendingIntent };
}
```

`App.vue` 在现有 hotkey listener 旁增加：

```ts
await listen<ActionDispatchRequest>(APP_EVENTS.ACTION_CENTER_DISPATCH_REQUEST, ({ payload }) => {
  if (!payload.dispatchId || !isRealToolId(payload.targetToolId)) {
    ElMessage.error("动作请求的目标工具无效");
    return;
  }
  setPendingIntent(payload);
  onSelect(payload.targetToolId);
});
```

先存 intent 再切 tab，确保目标面板首次挂载和已挂载两种情况都不会丢请求。

- [ ] **Step 4: 运行 intent 测试和类型检查**

Run: `pnpm --filter @lazycat/desktop test -- src/composables/useActionDispatchIntent.test.ts src/components/ReleasePackagePanel.test.ts`

Expected: PASS，目标匹配、一次消费、已挂载更新和 App 路由顺序通过。

Run: `pnpm typecheck`

Expected: PASS，事件 payload 与 intent 类型一致。

- [ ] **Step 5: 提交 intent 路由**

```powershell
git add apps/desktop/src/composables/useActionDispatchIntent.ts apps/desktop/src/composables/useActionDispatchIntent.test.ts apps/desktop/src/types/action-center.ts apps/desktop/src/App.vue apps/desktop/src/components/ReleasePackagePanel.test.ts
git commit -m "feat: 添加动作派发导航意图"
```

---

### Task 7: 让上线包面板消费 intent 并完整复用确认链

**Files:**

- Modify: `apps/desktop/src/components/ReleasePackagePanel.vue`
- Modify: `apps/desktop/src/components/ReleasePackagePanel.test.ts`
- Modify: `apps/desktop/src/utils/releasePackage.ts`
- Modify: `apps/desktop/src/utils/releasePackage.test.ts`
- Modify: `apps/desktop/src/types/release-package.ts`

- [ ] **Step 1: 写 dirty 保护、配置选择、取消和启动 payload 失败测试**

在纯 payload 测试中保证只有动作启动携带 ID：

```ts
expect(
  createReleasePackageStartPayload("local_archive", {
    projectId: 7,
    targets: ["frontend", "backend"],
    folderName: "20260725-客户门户",
    overwriteExisting: false,
    actionDispatchId: "dispatch-1",
  }),
).toMatchObject({ actionDispatchId: "dispatch-1" });

expect(
  createReleasePackageStartPayload("local_archive", {
    projectId: 7,
    targets: ["frontend"],
    folderName: "manual",
    overwriteExisting: false,
  }),
).not.toHaveProperty("actionDispatchId");
```

面板契约测试断言：

- `watchPendingIntent("release-package", applyActionDispatchIntent)`；
- `dirty.value` 时调用 `dispatch-cancel` 且 `outcome: "failed"`，不改 `selectedId/draft`；
- 找不到目标配置时显式失败，不选择第一项；
- 选中目标后调用原 `prepareStart`；
- 关闭/取消确认使用 `outcome: "cancelled"`；
- `tool:release-package:start` payload 带 `actionDispatchId`；
- `upload-retry` 永不带动作 dispatch ID。

- [ ] **Step 2: 运行上线包前端测试并确认失败**

Run: `pnpm --filter @lazycat/desktop test -- src/utils/releasePackage.test.ts src/components/ReleasePackagePanel.test.ts`

Expected: FAIL，面板尚未消费 intent，启动 payload 不支持 `actionDispatchId`。

- [ ] **Step 3: 实现 intent 生命周期并保持现有确认不变**

新增面板局部状态和三个显式结束函数：

```ts
const pendingActionDispatchId = ref<string | null>(null);

async function stopPendingActionDispatch(outcome: "cancelled" | "failed", error?: string) {
  const dispatchId = pendingActionDispatchId.value;
  if (!dispatchId) return;
  await invokeToolByChannel("tool:action-center:dispatch-cancel", {
    dispatchId,
    outcome,
    ...(error ? { error } : {}),
  });
  pendingActionDispatchId.value = null;
}
```

消费逻辑先保护现场，再加载最新配置：

```ts
async function applyActionDispatchIntent(intent: ActionDispatchRequest) {
  if (intent.actionType !== "release_package.run") return;
  pendingActionDispatchId.value = intent.dispatchId;
  if (dirty.value) {
    await stopPendingActionDispatch("failed", "上线包页面有未保存配置，未切换打包项目");
    ElMessage.error("上线包页面有未保存配置，动作已停止");
    return;
  }
  if (running.value) {
    await stopPendingActionDispatch("failed", "已有发布打包任务正在运行");
    return;
  }
  if (!(await loadProjects())) {
    await stopPendingActionDispatch("failed", "加载上线包配置失败");
    return;
  }
  const target = projects.value.find((project) => String(project.id) === intent.targetId);
  if (!target) {
    await stopPendingActionDispatch("failed", "上线包配置不存在");
    return;
  }
  selectedId.value = target.id;
  Object.assign(draft, projectToReleasePackageDraft(target));
  const prepareError = await prepareStart();
  if (prepareError) await stopPendingActionDispatch("failed", prepareError.message);
}
```

`prepareStart` 改为返回 `Error | null`，仍调用既有 `tool:release-package:prepare` 并打开原确认框；手动按钮和 intent 共用这一函数。

所有用户拒绝分支调用 `stopPendingActionDispatch("cancelled")`：关闭确认框、取消本地覆盖、取消主机信任、取消远端覆盖、启动前点击终止。配置/预检/后端启动异常调用 `failed`。进入 Rust `start` 成功后只清空前端 `pendingActionDispatchId`，不能调用 cancel，因为后端已将 dispatch 置为 `running`。

确认框右上角关闭也必须进入相同取消路径，不能只依赖 footer：

```vue
<el-dialog
  v-model="confirmVisible"
  :before-close="beforeCloseStartDialog"
  @closed="resetStartDialog"
>
```

```ts
async function beforeCloseStartDialog(done: () => void) {
  if (starting.value) return;
  await stopPendingActionDispatch("cancelled");
  done();
}
```

启动 payload 只在非重试且存在 ID 时追加：

```ts
createReleasePackageStartPayload(packageType, {
  projectId,
  targets: selectedTargets.value,
  folderName: folderName.value,
  overwriteExisting,
  preflightToken: uploadPreflight.preflightToken.value,
  overwriteRemoteTargets: overwriteRemoteTargets.value,
  actionDispatchId: pendingActionDispatchId.value ?? undefined,
});
```

不要移动或删除 `confirmArchiveOverwrite`、`ensureHostTrusted`、`runUploadPreflight`、`confirmRemoteOverwrite` 和 `runtime.ensureListeners()`；它们仍是动作启动必须经过的确认链。

- [ ] **Step 4: 运行上线包测试、类型检查和渲染层构建**

Run: `pnpm --filter @lazycat/desktop test -- src/utils/releasePackage.test.ts src/components/ReleasePackagePanel.test.ts src/composables/useReleasePackageUploadPreflight.test.ts src/composables/useReleasePackageRuntime.test.ts`

Expected: PASS，dirty、目标选择、取消、失败、payload 和原预检流程测试通过。

Run: `pnpm typecheck`

Expected: PASS。

Run: `pnpm --filter @lazycat/desktop build:web`

Expected: PASS，Vite 构建完成且无模板类型错误。

- [ ] **Step 5: 提交上线包确认集成**

```powershell
git add apps/desktop/src/components/ReleasePackagePanel.vue apps/desktop/src/components/ReleasePackagePanel.test.ts apps/desktop/src/utils/releasePackage.ts apps/desktop/src/utils/releasePackage.test.ts apps/desktop/src/types/release-package.ts
git commit -m "feat: 接入动作派发打包确认流程"
```

---

### Task 8: 启动前关联 run，并按上线包终态完成 dispatch/Todo

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/action_center/dispatches.rs`
- Modify: `apps/desktop/src-tauri/src/tools/action_center/mod.rs`
- Modify: `apps/desktop/src-tauri/src/tools/todo/items.rs`
- Modify: `apps/desktop/src-tauri/src/tools/todo/mod.rs`
- Modify: `apps/desktop/src-tauri/src/tools/release_package.rs`
- Modify: `apps/desktop/src-tauri/src/tools/release_package_runtime.rs`

- [ ] **Step 1: 写 run 关联顺序、终态映射和 Todo 幂等失败测试**

动作中心连接测试覆盖完整矩阵：

```rust
#[test]
fn only_full_success_completes_the_todo() {
    for (result_code, expected_dispatch, expected_todo) in [
        ("succeeded", "succeeded", "completed"),
        ("partially_succeeded", "failed", "pending"),
        ("package_succeeded_upload_failed", "failed", "pending"),
        ("failed", "failed", "pending"),
        ("cancelled", "cancelled", "pending"),
    ] {
        let mut conn = running_dispatch_fixture(result_code);
        finish_external_run_with_conn(&mut conn, "run-1", result_code).unwrap();
        assert_eq!(dispatch_status(&conn), expected_dispatch);
        assert_eq!(todo_status(&conn), expected_todo);
    }
}

#[test]
fn wrong_or_repeated_run_terminal_is_idempotent() {
    let mut conn = running_dispatch_fixture("succeeded");
    assert!(!finish_external_run_with_conn(&mut conn, "other-run", "succeeded").unwrap());
    assert!(finish_external_run_with_conn(&mut conn, "run-1", "succeeded").unwrap());
    let completed_at = todo_completed_at(&conn);
    assert!(!finish_external_run_with_conn(&mut conn, "run-1", "failed").unwrap());
    assert_eq!(todo_completed_at(&conn), completed_at);
}
```

另测：关联时 dispatch 必须是 `pending_confirmation`、action/target 与项目一致；关联成功同时写 `external_run_id/started_at/status=running`；Todo 已手动完成保持 `completed_at`；Todo 已删除仍结束 dispatch。

在 `release_package_runtime.rs` 增加结构顺序测试：

```rust
#[test]
fn action_dispatch_is_bound_before_worker_spawn() {
    let source = include_str!("release_package_runtime.rs");
    let start = &source[source.find("pub fn start(").unwrap()..];
    assert!(start.find("associate_release_package_run").unwrap()
        < start.find("thread::spawn").unwrap());
}
```

- [ ] **Step 2: 运行动作中心和上线包测试并确认失败**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml action_center -- --nocapture`

Expected: FAIL，外部 run 关联和终态函数尚不存在。

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package -- --nocapture`

Expected: FAIL，启动签名和关联顺序尚未实现。

- [ ] **Step 3: 实现关联、终态映射和 Todo 完成复用**

从 `item_change_status` 抽出连接版本，公共 IPC 仍保持原契约：

```rust
pub(crate) fn change_item_status_with_conn(
    conn: &Connection,
    id: i64,
    next: &str,
) -> Result<Value, String> {
    // 保留现有 can_transit_for_kind、completed_at、清 snooze、提醒已读和周期生成语义。
}

pub(crate) fn item_change_status(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("缺少事项 id")?;
    let next = normalize_status(payload.get("status").and_then(Value::as_str).unwrap_or(STATUS_PENDING))?;
    let conn = db_conn()?;
    change_item_status_with_conn(&conn, id, &next)
}
```

在 `todo/mod.rs` 通过 `pub(crate) use items::change_item_status_with_conn;` 暴露给动作中心；不要把整个私有 `items` 模块改成公共模块。

动作中心关联接口：

```rust
pub(crate) fn associate_release_package_run(
    dispatch_id: &str,
    run_id: &str,
    project_id: i64,
) -> Result<(), String>;

pub(crate) fn finish_release_package_run(
    run_id: &str,
    result_code: &str,
) -> Result<bool, String>;
```

`associate` 在事务中核对 `pending_confirmation + release_package.run + target_id == project_id` 后转 `running`。`finish` 只按 `external_run_id + status=running` 更新一次，映射如下：

```rust
let dispatch_status = match result_code {
    "succeeded" => "succeeded",
    "cancelled" => "cancelled",
    "partially_succeeded" | "package_succeeded_upload_failed" | "failed" => "failed",
    _ => return Err(format!("未知的上线包终态: {result_code}")),
};
```

仅 `succeeded` 且 Todo 仍存在时在同一事务中调用 `change_item_status_with_conn(..., "completed")`。Todo 已完成由现有幂等状态转换保留 `completed_at`；Todo 不存在则跳过完成但提交 dispatch 终态。

`release_package.rs::execute_with_app("start")` 严格解析可选 `actionDispatchId`，空字符串或非字符串报错；普通手动打包传 `None`。`release_package_runtime::start` 增加 `action_dispatch_id: Option<String>`：

```rust
if let Some(dispatch_id) = action_dispatch_id.as_deref() {
    if let Err(error) = crate::tools::action_center::associate_release_package_run(
        dispatch_id,
        &run_id,
        project.id,
    ) {
        if let Ok(mut active) = active_run().lock() {
            *active = None;
        }
        return Err(error);
    }
}

let thread_run_id = run_id.clone();
thread::spawn(move || { /* 现有流水线 */ });
```

必须在 `thread::spawn` 前完成关联；关联失败同时释放刚占用的 `ACTIVE_RUN`，不启动任何命令。

`emit_terminal_result` 形成原始 `status` 后、发送 overall status/通知前调用：

```rust
if let Err(error) = crate::tools::action_center::finish_release_package_run(run_id, status) {
    eprintln!("action-center terminal update failed for run {run_id}: {error}");
}
```

普通手动打包没有匹配 external run，返回 `Ok(false)` 且现有日志/通知完全不变。

- [ ] **Step 4: 运行 Rust 定向测试和全量前端测试**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml action_center -- --nocapture`

Expected: PASS，关联、矩阵映射、错误 run、幂等、删除和完成语义通过。

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package -- --nocapture`

Expected: PASS，关联先于线程、手动启动兼容、现有终态通知测试通过。

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml todo -- --nocapture`

Expected: PASS，抽取连接版本未改变 Todo 状态语义。

Run: `pnpm test`

Expected: PASS，所有前端单元和结构契约测试通过。

- [ ] **Step 5: 提交 run/终态联动**

```powershell
git add apps/desktop/src-tauri/src/tools/action_center apps/desktop/src-tauri/src/tools/todo apps/desktop/src-tauri/src/tools/release_package.rs apps/desktop/src-tauri/src/tools/release_package_runtime.rs
git commit -m "feat: 联动打包终态完成任务"
```

---

### Task 9: 完成跨层回归、经验沉淀和验收

**Files:**

- Modify: `docs/experience/architecture.md`
- Modify: `docs/experience/todo.md`
- Modify: `docs/experience/release-package.md`
- Verify: `docs/superpowers/specs/2026-07-25-action-center-package-reminder-design.md`

- [ ] **Step 1: 先运行完整定向回归，记录任何真实失败**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml action_center -- --nocapture`

Expected: PASS。

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml todo -- --nocapture`

Expected: PASS。

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package -- --nocapture`

Expected: PASS。

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml contract_tests -- --nocapture`

Expected: PASS。

Run: `pnpm --filter @lazycat/desktop test -- src/composables/useTodoItem.test.ts src/composables/useTodoActionBinding.test.ts src/composables/useActionDispatchIntent.test.ts src/utils/globalNotification.test.ts src/utils/releasePackage.test.ts src/components/todo/TodoActionBinding.test.ts src/components/GlobalNotificationPopup.test.ts src/components/ReleasePackagePanel.test.ts`

Expected: PASS。

- [ ] **Step 2: 若有失败，先写最小回归测试再修复根因**

只允许修复本功能暴露的契约、事务、状态机或生命周期问题。典型断言形式：

```rust
assert_eq!(dispatch_status(&conn, dispatch_id), "failed");
assert_eq!(todo_status(&conn, todo_id), "pending");
```

```ts
expect(invokeMock).toHaveBeenCalledWith("tool:action-center:dispatch-cancel", {
  dispatchId: "dispatch-1",
  outcome: "cancelled",
});
```

Run: 重跑最先失败的精确测试命令。

Expected: 新回归测试由 FAIL 变 PASS，且不改变非动作 Todo、普通手动打包或上传重试行为。

- [ ] **Step 3: 沉淀三条可复用经验**

在 `architecture.md` 增加“跨工具动作通过定义/适配器/dispatch 接入，不让触发源复制目标配置”；在 `todo.md` 增加“事项与通用绑定必须同事务，提醒摘要仅展示、点击重新校验”；在 `release-package.md` 增加“外部 dispatch 必须在线程启动前关联 run，终态只在统一 emit 点回写”。同步各文件目录锚点和使用次数。

- [ ] **Step 4: 运行最终验证和静态卫生检查**

Run: `pnpm test`

Expected: PASS。

Run: `pnpm typecheck`

Expected: PASS。

Run: `pnpm --filter @lazycat/desktop build:web`

Expected: PASS。

Run: `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml`

Expected: PASS，无新增编译错误；测试专用 helper 不产生生产构建 dead-code 警告。

Run: `git diff --check`

Expected: 无输出。

Run: `rg -n "actionDispatchId.*password|action_bindings.*ssh|action_dispatches.*ssh" apps/desktop/src apps/desktop/src-tauri/src docs/experience`

Expected: 无输出；不把密码、SSH 秘密或上线包配置复制进动作中心。

- [ ] **Step 5: 按验收矩阵做最小手工冒烟并提交文档**

不自动启动 `pnpm dev`。由执行者在用户明确允许启动 UI 后验证以下矩阵；未获允许时在交付说明中明确“未运行 UI 冒烟”，不把静态测试包装成手工验证：

```text
普通 Todo 提醒                 -> 仍显示“完成 / 知道了 / 稍后提醒”
动作 Todo 提醒                 -> 显示“开始打包 / 知道了 / 稍后提醒”
动作提醒 + 目标删除            -> 主按钮禁用并显示失效原因
动作请求 + 上线包 dirty draft  -> 不覆盖草稿，dispatch failed，Todo 未完成
取消任一确认                   -> dispatch cancelled，未启动打包，Todo 未完成
succeeded                      -> dispatch succeeded，Todo completed
partially_succeeded            -> dispatch failed，Todo 保持未完成
package_succeeded_upload_failed -> dispatch failed，Todo 保持未完成
failed                         -> dispatch failed，Todo 保持未完成
cancelled/interrupted           -> dispatch cancelled/failed，Todo 保持未完成
普通手动打包                   -> 行为、通知和重试保持不变
```

```powershell
git add docs/experience/architecture.md docs/experience/todo.md docs/experience/release-package.md
git commit -m "docs: 沉淀动作中心集成经验"
```

---

## 完成定义

- 第一版没有新增动作中心入口、菜单或独立页面。
- 动作定义只来自代码注册表；数据库不保存任意命令。
- 单次 Todo 最多一个动作，周期事项没有动作字段。
- Todo 数据和绑定在一个 SQLite 事务内提交或回滚。
- 每次点击重新校验绑定、目标、提醒事件和活动 dispatch。
- intent 与剪贴板状态完全分离。
- 上线包所有现有确认和秘密边界保持不变。
- `dispatchId + runId` 在线程启动前关联。
- 只有原始终态 `succeeded` 自动完成 Todo。
- 删除、重复触发、错误 run、应用中断和目标失效都显式处理。
- 定向测试、`pnpm test`、`pnpm typecheck`、`build:web`、`cargo check` 与 `git diff --check` 全部通过。
