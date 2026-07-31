# 动作中心组合动作实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 LazyCat 增加独立动作中心，让用户把 Hosts、浏览器身份和请求转发等可信原子动作保存为可复用组合，并按串行或并行模式一键运行和查看逐步结果。

**Architecture:** Rust 动作中心新增组合配置、可信原子适配器、可测试执行器和持久化运行协调器；组合只保存 `actionType + targetId`，目标工具继续拥有配置、校验和副作用。Vue 页面通过现有 IPC 管理组合并订阅运行事件，数据库是运行状态唯一事实源，事件与轮询只负责刷新。

**Tech Stack:** Tauri 2、Rust、rusqlite、serde、uuid、Vue 3、TypeScript、Element Plus、SortableJS、Vitest、pnpm

---

## 文件职责与落点

**新增文件**

- `apps/desktop/src-tauri/src/tools/action_center/combinations.rs`：组合配置模型、schema、校验、事务 CRUD 和步骤快照读取。
- `apps/desktop/src-tauri/src/tools/action_center/atomic_actions.rs`：可组合原子动作注册表、目标适配和统一执行结果。
- `apps/desktop/src-tauri/src/tools/action_center/combination_executor.rs`：与 Tauri/SQLite 解耦的串行、并行执行和结果聚合。
- `apps/desktop/src-tauri/src/tools/action_center/combination_runs.rs`：运行快照、全局单运行门禁、后台协调、事件通知、历史查询和中断恢复。
- `apps/desktop/src/utils/actionCombination.ts`：前端 draft 归一化、重排、状态标签和终态判断纯函数。
- `apps/desktop/src/utils/actionCombination.test.ts`：组合编辑与状态纯函数测试。
- `apps/desktop/src/composables/useActionCombinations.ts`：组合 CRUD、目标请求版本控制、运行事件与轮询。
- `apps/desktop/src/composables/useActionCombinations.test.ts`：IPC 调用、晚响应隔离和运行刷新测试。
- `apps/desktop/src/components/ActionCenterPanel.vue`：动作中心页面状态编排和双栏布局。
- `apps/desktop/src/components/action-center/ActionCombinationList.vue`：组合列表与最近状态。
- `apps/desktop/src/components/action-center/ActionCombinationEditor.vue`：组合表单、步骤编辑、SortableJS 重排和操作按钮。
- `apps/desktop/src/components/action-center/ActionRunHistory.vue`：当前运行进度、终态汇总和最近 20 次记录。
- `apps/desktop/src/components/ActionCenterPanel.test.ts`：页面注册、关键交互和可访问性契约测试。

**主要修改文件**

- `apps/desktop/src-tauri/src/tools/action_center/{mod.rs,definitions.rs}`：注册组合模块，保持现有 Todo/上线包动作不变，并扩展 IPC 白名单。
- `apps/desktop/src-tauri/src/tools/{hosts.rs,browser_profiles.rs,request_forward/mod.rs}`：暴露窄的内部动作目标与执行接口，复用现有领域实现。
- `apps/desktop/src-tauri/src/{events.rs,main.rs}`：组合运行更新事件和启动中断恢复。
- `apps/desktop/src/{bridge/tauri.ts,bridge/events.ts}`：新增组合动作 channel 和事件常量。
- `apps/desktop/src/types/{action-center.ts,index.ts}`：集中定义并导出组合配置、运行和步骤契约。
- `apps/desktop/src/{tool-registry.ts,composables/toolCatalog.ts}`：注册独立动作中心工具。
- `apps/desktop/src/composables/toolCatalog.test.ts`：守卫动作中心工具入口。
- `docs/experience/architecture.md`：沉淀组合动作的注册、快照和运行真值边界。

## 统一契约

实现时以下命名不可漂移：

```ts
export type ActionCombinationExecutionMode = "serial" | "parallel";
export type ActionCombinationRunStatus =
  | "pending"
  | "running"
  | "succeeded"
  | "partially_succeeded"
  | "failed";
export type ActionCombinationStepStatus =
  | "pending"
  | "running"
  | "succeeded"
  | "already_satisfied"
  | "failed";

export interface ActionCombinationStepInput {
  actionType: string;
  targetId: string;
}

export interface ActionCombinationSaveInput {
  id?: number;
  name: string;
  executionMode: ActionCombinationExecutionMode;
  steps: ActionCombinationStepInput[];
}
```

Rust 数据库状态使用同名 snake_case 字符串，serde 输出统一 `camelCase`。运行事件名固定为 `action-center://combination-run-updated`，事件只携带 `runId` 与 `status`，页面收到后重新读取完整运行详情。

---

### Task 1: 建立组合配置 schema 与事务 CRUD

**Files:**

- Create: `apps/desktop/src-tauri/src/tools/action_center/combinations.rs`
- Modify: `apps/desktop/src-tauri/src/tools/action_center/mod.rs`
- Test: `apps/desktop/src-tauri/src/tools/action_center/combinations.rs`

- [ ] **Step 1: 写 schema、校验和 CRUD 失败测试**

在 `combinations.rs` 的 `#[cfg(test)]` 模块加入以下核心用例：

```rust
#[test]
fn saves_and_replaces_ordered_combination_steps_atomically() {
    let mut conn = test_conn();
    let id = save_with_conn(&mut conn, CombinationSaveInput {
        id: None,
        name: "客户门户开发环境".into(),
        execution_mode: ExecutionMode::Parallel,
        steps: vec![
            step("hosts.activate", "7"),
            step("request_forward.start", "12"),
        ],
    }, allow_registered_target).unwrap();

    save_with_conn(&mut conn, CombinationSaveInput {
        id: Some(id),
        name: "客户门户".into(),
        execution_mode: ExecutionMode::Serial,
        steps: vec![step("browser_profile.launch", r#"["edge","Profile 1"]"#)],
    }, allow_registered_target).unwrap();

    let saved = get_with_conn(&conn, id).unwrap();
    assert_eq!(saved.name, "客户门户");
    assert_eq!(saved.execution_mode, ExecutionMode::Serial);
    assert_eq!(saved.steps.len(), 1);
    assert_eq!(saved.steps[0].sort_order, 0);
}

#[test]
fn rejects_empty_unknown_and_duplicate_steps_without_partial_write() {
    let mut conn = test_conn();
    for input in [
        input("", vec![step("hosts.activate", "1")]),
        input("空组合", vec![]),
        input("未知动作", vec![step("shell.run", "1")]),
        input("重复目标", vec![step("hosts.activate", "1"), step("hosts.activate", "1")]),
    ] {
        assert!(save_with_conn(&mut conn, input, allow_registered_target).is_err());
    }
    assert!(list_with_conn(&conn).unwrap().is_empty());
}

#[test]
fn deleting_combination_keeps_run_snapshot_but_cascades_editable_steps() {
    let mut conn = test_conn();
    let id = seed_combination(&mut conn);
    seed_finished_run(&conn, id);
    delete_with_conn(&conn, id).unwrap();
    assert!(get_with_conn(&conn, id).is_err());
    assert_eq!(run_combination_id(&conn), None);
}
```

- [ ] **Step 2: 运行测试并确认按预期失败**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml action_center::combinations -- --nocapture`

Expected: FAIL，提示 `combinations` 模块或 `save_with_conn` 等符号不存在。

- [ ] **Step 3: 实现 schema、模型和事务 CRUD**

在 `combinations.rs` 定义以下稳定接口，并由 `action_center::ensure_schema` 调用 `ensure_schema`：

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ExecutionMode { Serial, Parallel }

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CombinationStepInput {
    pub action_type: String,
    pub target_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CombinationSaveInput {
    pub id: Option<i64>,
    pub name: String,
    pub execution_mode: ExecutionMode,
    pub steps: Vec<CombinationStepInput>,
}

pub(crate) fn save_with_conn<F>(
    conn: &mut Connection,
    input: CombinationSaveInput,
    validate_target: F,
) -> Result<i64, String>
where
    F: Fn(&str, &str) -> Result<(), String>;

pub(crate) fn list_with_conn(conn: &Connection) -> Result<Vec<CombinationSummary>, String>;
pub(crate) fn get_with_conn(conn: &Connection, id: i64) -> Result<CombinationDetail, String>;
pub(crate) fn delete_with_conn(conn: &Connection, id: i64) -> Result<(), String>;
```

`CombinationSummary` 必须包含 `id/name/executionMode/stepCount/latestRunStatus/updatedAt`；`list_with_conn` 用最近一条 run 的只读子查询填充 `latestRunStatus`，没有运行记录时返回 `None`。

schema 必须包含：

```rust
const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS action_combinations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    execution_mode TEXT NOT NULL CHECK(execution_mode IN ('serial','parallel')),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE IF NOT EXISTS action_combination_steps (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    combination_id INTEGER NOT NULL REFERENCES action_combinations(id) ON DELETE CASCADE,
    action_type TEXT NOT NULL,
    target_id TEXT NOT NULL,
    sort_order INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(combination_id, sort_order),
    UNIQUE(combination_id, action_type, target_id)
);
CREATE TABLE IF NOT EXISTS action_combination_runs (
    id TEXT PRIMARY KEY,
    combination_id INTEGER NULL REFERENCES action_combinations(id) ON DELETE SET NULL,
    combination_name TEXT NOT NULL,
    execution_mode TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('pending','running','succeeded','partially_succeeded','failed')),
    result_code TEXT NULL,
    error TEXT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    started_at TEXT NULL,
    finished_at TEXT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_action_combination_runs_one_active
ON action_combination_runs((1)) WHERE status IN ('pending','running');
CREATE TABLE IF NOT EXISTS action_combination_run_steps (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT NOT NULL REFERENCES action_combination_runs(id) ON DELETE CASCADE,
    source_step_id INTEGER NULL,
    action_type TEXT NOT NULL,
    action_label TEXT NOT NULL,
    target_id TEXT NOT NULL,
    target_label TEXT NOT NULL,
    sort_order INTEGER NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('pending','running','succeeded','already_satisfied','failed')),
    result_code TEXT NULL,
    message TEXT NULL,
    started_at TEXT NULL,
    finished_at TEXT NULL,
    UNIQUE(run_id, sort_order)
);
"#;
```

`save_with_conn` 在开启事务前完成输入归一化和目标校验，在事务中 upsert 组合、删除旧步骤并按数组顺序插入新步骤；任一步失败都不提交。删除组合时先拒绝 `pending/running` 的关联运行。

- [ ] **Step 4: 运行定向测试并确认通过**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml action_center::combinations -- --nocapture`

Expected: PASS，组合 CRUD、约束和删除快照测试全部通过。

- [ ] **Step 5: 提交配置存储层**

```powershell
git add apps/desktop/src-tauri/src/tools/action_center/combinations.rs apps/desktop/src-tauri/src/tools/action_center/mod.rs
git commit -m "feat: 添加组合动作配置存储"
```

---

### Task 2: 建立可信原子动作注册表与目标查询

**Files:**

- Create: `apps/desktop/src-tauri/src/tools/action_center/atomic_actions.rs`
- Modify: `apps/desktop/src-tauri/src/tools/action_center/mod.rs`
- Modify: `apps/desktop/src-tauri/src/tools/action_center/definitions.rs`
- Modify: `apps/desktop/src-tauri/src/tools/hosts.rs`
- Modify: `apps/desktop/src-tauri/src/tools/browser_profiles.rs`
- Modify: `apps/desktop/src-tauri/src/tools/request_forward/mod.rs`
- Test: `apps/desktop/src-tauri/src/tools/action_center/atomic_actions.rs`

- [ ] **Step 1: 写注册、目标编码和目标列表失败测试**

```rust
#[test]
fn only_registered_safe_actions_are_composable() {
    assert_eq!(
        combination_definitions().iter().map(|item| item.action_type).collect::<Vec<_>>(),
        vec![HOSTS_ACTIVATE, BROWSER_PROFILE_LAUNCH, REQUEST_FORWARD_START],
    );
    assert!(!definition("release_package.run").unwrap().supports_combination);
    assert!(definition("shell.run").is_none());
}

#[test]
fn todo_definition_list_still_excludes_manual_combination_atoms() {
    assert_eq!(
        all_definitions().iter().map(|item| item.action_type).collect::<Vec<_>>(),
        vec![RELEASE_PACKAGE_RUN],
    );
}

#[test]
fn browser_target_key_round_trips_without_delimiter_assumptions() {
    let key = crate::tools::browser_profiles::encode_action_target("edge", "Profile 1").unwrap();
    assert_eq!(
        crate::tools::browser_profiles::decode_action_target(&key).unwrap(),
        ("edge".to_string(), "Profile 1".to_string()),
    );
}

#[test]
fn hosts_and_forward_targets_use_stable_numeric_ids() {
    let conn = test_conn();
    seed_hosts(&conn, 7, "开发");
    seed_forward(&conn, 12, "本地 API");
    assert_eq!(crate::tools::hosts::list_action_targets_with_conn(&conn).unwrap()[0].0, "7");
    assert_eq!(crate::tools::request_forward::list_action_targets_with_conn(&conn).unwrap()[0].0, "12");
}
```

- [ ] **Step 2: 运行测试并确认按预期失败**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml atomic_actions -- --nocapture`

Expected: FAIL，提示 `atomic_actions`、目标编码或领域窄接口不存在。

- [ ] **Step 3: 实现注册表和只读目标适配器**

在现有 `definitions.rs` 扩展唯一动作元数据注册表，禁止在适配器模块复制标签或目标工具：

```rust
pub(crate) const HOSTS_ACTIVATE: &str = "hosts.activate";
pub(crate) const BROWSER_PROFILE_LAUNCH: &str = "browser_profile.launch";
pub(crate) const REQUEST_FORWARD_START: &str = "request_forward.start";

// Add this field to the existing ActionDefinition.
pub supports_combination: bool,

// Keep all_definitions() limited to definitions with non-empty trigger_types
// so the existing Todo selector does not gain manual atoms.
pub(crate) fn combination_definitions() -> Vec<ActionDefinition>;
```

`definition()` 同时识别现有 `release_package.run` 和三个新动作；上线包设置 `supports_combination=false`，三个新动作设置 `trigger_types=&[]`、`supports_combination=true`。在 `atomic_actions.rs` 只实现目标和快照接口，复用现有 `definitions::ActionTargetOption`：

```rust
pub(crate) fn list_targets(action_type: &str) -> Result<Vec<ActionTargetOption>, String>;
pub(crate) fn validate_target(action_type: &str, target_id: &str) -> Result<ActionTargetOption, String>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AtomicTargetSnapshot {
    pub action_label: String,
    pub target_label: String,
    pub validation_error: Option<String>,
}

pub(crate) fn snapshot_target(action_type: &str, target_id: &str) -> AtomicTargetSnapshot;
```

领域文件只暴露窄接口：

```rust
// hosts.rs
pub(crate) fn list_action_targets_with_conn(conn: &Connection) -> Result<Vec<(String, String)>, String>;
pub(crate) fn load_action_target_with_conn(conn: &Connection, id: i64) -> Result<Option<(String, String)>, String>;

// browser_profiles.rs
pub(crate) fn encode_action_target(browser: &str, profile_dir: &str) -> Result<String, String>;
pub(crate) fn decode_action_target(target_id: &str) -> Result<(String, String), String>;
pub(crate) fn list_action_targets() -> Result<Vec<(String, String, bool, Option<String>)>, String>;

// request_forward/mod.rs
pub(crate) fn list_action_targets_with_conn(conn: &Connection) -> Result<Vec<(String, String)>, String>;
pub(crate) fn load_action_target_with_conn(conn: &Connection, id: i64) -> Result<Option<String>, String>;
```

浏览器 key 使用 `serde_json::to_string(&(browser, profile_dir))`，禁止自行拼接分隔符。浏览器不存在或 executable 不可用时目标仍返回但 `available=false` 和明确原因，已保存组合因此能显示失效目标。

- [ ] **Step 4: 运行注册表和领域现有测试**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml atomic_actions -- --nocapture`

Expected: PASS。

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml browser_profiles -- --nocapture`

Expected: PASS，既有 profile 扫描、别名和启动参数测试无回归。

- [ ] **Step 5: 提交原子动作注册与目标查询**

```powershell
git add apps/desktop/src-tauri/src/tools/action_center/atomic_actions.rs apps/desktop/src-tauri/src/tools/action_center/definitions.rs apps/desktop/src-tauri/src/tools/action_center/mod.rs apps/desktop/src-tauri/src/tools/hosts.rs apps/desktop/src-tauri/src/tools/browser_profiles.rs apps/desktop/src-tauri/src/tools/request_forward/mod.rs
git commit -m "feat: 注册组合动作原子目标"
```

---

### Task 3: 复用领域执行链并区分已满足状态

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/action_center/atomic_actions.rs`
- Modify: `apps/desktop/src-tauri/src/tools/hosts.rs`
- Modify: `apps/desktop/src-tauri/src/tools/browser_profiles.rs`
- Modify: `apps/desktop/src-tauri/src/tools/request_forward/mod.rs`
- Test: same files' `#[cfg(test)]` modules

- [ ] **Step 1: 写 Hosts、浏览器和转发执行语义失败测试**

```rust
#[test]
fn normalized_hosts_content_detects_already_satisfied_target() {
    assert!(hosts_content_matches("127.0.0.1 localhost\r\n", "127.0.0.1 localhost\n"));
    assert!(!hosts_content_matches("127.0.0.1 localhost\n", "192.0.2.1 api.test\n"));
}

#[test]
fn request_forward_running_is_already_satisfied_without_auto_start_mutation() {
    assert_eq!(classify_action_start(RuntimeState::Running), ActionStartDecision::AlreadySatisfied);
    assert_eq!(classify_action_start(RuntimeState::Stopped), ActionStartDecision::Start);
    assert_eq!(classify_action_start(RuntimeState::Failed), ActionStartDecision::Start);
}

#[test]
fn atomic_executor_maps_domain_boolean_to_step_outcome() {
    assert_eq!(AtomicStepSuccess::from_changed(false).status, AtomicStepSuccessStatus::AlreadySatisfied);
    assert_eq!(AtomicStepSuccess::from_changed(true).status, AtomicStepSuccessStatus::Succeeded);
}
```

- [ ] **Step 2: 运行测试并确认按预期失败**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml action_target -- --nocapture`

Expected: FAIL，提示状态分类和执行窄接口不存在。

- [ ] **Step 3: 实现领域执行窄接口和统一结果**

```rust
// atomic_actions.rs
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AtomicStepSuccessStatus { Succeeded, AlreadySatisfied }

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AtomicStepSuccess {
    pub status: AtomicStepSuccessStatus,
    pub result_code: Option<String>,
    pub message: Option<String>,
}

pub(crate) trait AtomicActionExecutor: Send + Sync + 'static {
    fn execute(&self, action_type: &str, target_id: &str) -> Result<AtomicStepSuccess, String>;
}

pub(crate) struct RegisteredAtomicActionExecutor;
impl AtomicActionExecutor for RegisteredAtomicActionExecutor {
    fn execute(&self, action_type: &str, target_id: &str) -> Result<AtomicStepSuccess, String> {
        match action_type {
            HOSTS_ACTIVATE => crate::tools::hosts::activate_action_target(target_id)
                .map(AtomicStepSuccess::from_changed),
            BROWSER_PROFILE_LAUNCH => crate::tools::browser_profiles::launch_action_target(target_id)
                .map(|message| AtomicStepSuccess::succeeded(message)),
            REQUEST_FORWARD_START => crate::tools::request_forward::start_action_target(target_id)
                .map(AtomicStepSuccess::from_changed),
            _ => Err(format!("组合动作类型不存在: {action_type}")),
        }
    }
}
```

领域行为必须是：

- `hosts::activate_action_target` 按稳定 ID 加载名称和内容，读取真实系统 Hosts；换行归一化后相同返回 `Ok(false)`，否则复用现有备份、`write_hosts_file`、最终校验和 enabled 标记更新并返回 `Ok(true)`。
- `browser_profiles::launch_action_target` 解码 `(browser, profileDir)` 后调用现有 `launch_profile` 内部实现；每次都启动并返回统计警告组成的可选消息。
- `request_forward::start_action_target` 解析正整数 ID，`running` 返回 `Ok(false)`；其他状态调用 `global_manager().start_loaded` 并返回 `Ok(true)`，全程不调用 `set_auto_start_with_conn`。

- [ ] **Step 4: 运行三个领域定向测试**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml hosts -- --nocapture`

Expected: PASS。

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml browser_profiles -- --nocapture`

Expected: PASS。

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml request_forward -- --nocapture`

Expected: PASS，且新增测试证明 `autoStart` 不变。

- [ ] **Step 5: 提交领域执行适配**

```powershell
git add apps/desktop/src-tauri/src/tools/action_center/atomic_actions.rs apps/desktop/src-tauri/src/tools/hosts.rs apps/desktop/src-tauri/src/tools/browser_profiles.rs apps/desktop/src-tauri/src/tools/request_forward/mod.rs
git commit -m "feat: 接入组合动作领域执行"
```

---

### Task 4: 实现可测试的串行与并行执行器

**Files:**

- Create: `apps/desktop/src-tauri/src/tools/action_center/combination_executor.rs`
- Modify: `apps/desktop/src-tauri/src/tools/action_center/mod.rs`
- Test: `apps/desktop/src-tauri/src/tools/action_center/combination_executor.rs`

- [ ] **Step 1: 写顺序、并发、失败继续和聚合失败测试**

```rust
#[test]
fn serial_execution_preserves_order_and_continues_after_failure() {
    let executor = FakeExecutor::with_results([
        ("a", Ok(success())),
        ("b", Err("端口占用".into())),
        ("c", Ok(already_satisfied())),
    ]);
    let results = execute_plan(ExecutionMode::Serial, steps(["a", "b", "c"]), Arc::new(executor));
    assert_eq!(results.iter().map(|item| item.status).collect::<Vec<_>>(), vec![
        StepTerminalStatus::Succeeded,
        StepTerminalStatus::Failed,
        StepTerminalStatus::AlreadySatisfied,
    ]);
    assert_eq!(aggregate_status(&results), RunTerminalStatus::PartiallySucceeded);
}

#[test]
fn parallel_execution_overlaps_workers_but_returns_configured_order() {
    let probe = ConcurrencyProbe::new();
    let results = execute_plan(
        ExecutionMode::Parallel,
        steps(["slow-1", "slow-2", "slow-3"]),
        Arc::new(probe.executor()),
    );
    assert!(probe.max_concurrency() >= 2);
    assert_eq!(results.iter().map(|item| item.sort_order).collect::<Vec<_>>(), vec![0, 1, 2]);
}

#[test]
fn worker_panic_becomes_failed_step_instead_of_aborting_run() {
    let results = execute_plan(ExecutionMode::Parallel, steps(["panic", "ok"]), Arc::new(PanicExecutor));
    assert_eq!(results[0].status, StepTerminalStatus::Failed);
    assert_eq!(results[1].status, StepTerminalStatus::Succeeded);
}
```

- [ ] **Step 2: 运行测试并确认按预期失败**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml combination_executor -- --nocapture`

Expected: FAIL，提示执行器模块和状态类型不存在。

- [ ] **Step 3: 实现无 Tauri/SQLite 依赖的执行核心**

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PlannedStep {
    pub run_step_id: i64,
    pub action_type: String,
    pub target_id: String,
    pub sort_order: i64,
    pub validation_error: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StepTerminalStatus { Succeeded, AlreadySatisfied, Failed }

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExecutedStep {
    pub run_step_id: i64,
    pub sort_order: i64,
    pub status: StepTerminalStatus,
    pub result_code: Option<String>,
    pub message: Option<String>,
}

pub(crate) enum RunTerminalStatus { Succeeded, PartiallySucceeded, Failed }

pub(crate) fn execute_plan(
    mode: ExecutionMode,
    steps: Vec<PlannedStep>,
    executor: Arc<dyn AtomicActionExecutor>,
) -> Vec<ExecutedStep>;

pub(crate) trait ExecutionObserver: Send + Sync + 'static {
    fn step_started(&self, run_step_id: i64);
    fn step_finished(&self, result: &ExecutedStep);
}

pub(crate) fn execute_plan_with_observer(
    mode: ExecutionMode,
    steps: Vec<PlannedStep>,
    executor: Arc<dyn AtomicActionExecutor>,
    observer: Arc<dyn ExecutionObserver>,
) -> Vec<ExecutedStep>;

pub(crate) fn aggregate_status(results: &[ExecutedStep]) -> RunTerminalStatus;
```

`execute_plan` 使用无操作 observer，供纯测试和简单调用；运行协调器使用 `execute_plan_with_observer` 持久化逐步状态。步骤存在 `validation_error` 时不调用领域执行器，直接生成该步骤失败结果并继续。每个真实单步调用都用 `catch_unwind(AssertUnwindSafe(...))` 包住。串行分支逐项执行；并行分支为每步创建 `std::thread::spawn`，收集 join 结果后按 `sort_order` 排序。聚合规则严格对应规格：全成功/已满足为成功，成功与失败混合为部分成功，全失败为失败。

- [ ] **Step 4: 运行执行器测试并确认通过**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml combination_executor -- --nocapture`

Expected: PASS，测试能观测到并发且 panic 被局部收口。

- [ ] **Step 5: 提交执行核心**

```powershell
git add apps/desktop/src-tauri/src/tools/action_center/combination_executor.rs apps/desktop/src-tauri/src/tools/action_center/mod.rs
git commit -m "feat: 添加组合动作执行器"
```

---

### Task 5: 持久化运行快照、逐步进度与全局门禁

**Files:**

- Create: `apps/desktop/src-tauri/src/tools/action_center/combination_runs.rs`
- Modify: `apps/desktop/src-tauri/src/tools/action_center/mod.rs`
- Modify: `apps/desktop/src-tauri/src/events.rs`
- Test: `apps/desktop/src-tauri/src/tools/action_center/combination_runs.rs`

- [ ] **Step 1: 写快照、门禁、进度和恢复失败测试**

```rust
#[test]
fn run_snapshot_survives_combination_and_target_changes() {
    let mut conn = test_conn();
    let combination_id = seed_combination(&mut conn, "开发环境", ExecutionMode::Serial);
    let run = create_run_with_conn(&mut conn, combination_id, snapshot_target).unwrap();
    rename_combination(&conn, combination_id, "已改名");
    delete_source_target(&conn);
    let loaded = get_run_with_conn(&conn, &run.id).unwrap();
    assert_eq!(loaded.combination_name, "开发环境");
    assert_eq!(loaded.steps[0].target_label, "开发 Hosts");
}

#[test]
fn active_run_unique_index_rejects_second_combination() {
    let mut conn = test_conn();
    let first = seed_combination(&mut conn, "A", ExecutionMode::Serial);
    let second = seed_combination(&mut conn, "B", ExecutionMode::Parallel);
    create_run_with_conn(&mut conn, first, snapshot_target).unwrap();
    let error = create_run_with_conn(&mut conn, second, snapshot_target).unwrap_err();
    assert!(error.contains("已有组合动作正在运行"));
}

#[test]
fn observer_persists_each_step_and_aggregates_partial_success() {
    let mut conn = test_conn();
    let run = seed_pending_run(&mut conn, 3);
    persist_step_started_with_conn(&conn, run.steps[0].id).unwrap();
    persist_step_finished_with_conn(&conn, &succeeded(run.steps[0].id)).unwrap();
    persist_step_finished_with_conn(&conn, &failed(run.steps[1].id, "端口占用")).unwrap();
    persist_step_finished_with_conn(&conn, &already(run.steps[2].id)).unwrap();
    finish_run_with_conn(&conn, &run.id).unwrap();
    assert_eq!(get_run_with_conn(&conn, &run.id).unwrap().status, "partially_succeeded");
}

#[test]
fn recovery_marks_active_run_and_steps_as_interrupted_failures() {
    let mut conn = test_conn();
    let run = seed_running_run(&mut conn);
    assert_eq!(recover_interrupted_with_conn(&conn).unwrap(), 1);
    let recovered = get_run_with_conn(&conn, &run.id).unwrap();
    assert_eq!(recovered.status, "failed");
    assert_eq!(recovered.result_code.as_deref(), Some("interrupted"));
    assert!(recovered.steps.iter().all(|step| step.status == "failed"));
}
```

- [ ] **Step 2: 运行测试并确认按预期失败**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml combination_runs -- --nocapture`

Expected: FAIL，提示运行存储、observer 和恢复函数不存在。

- [ ] **Step 3: 实现运行仓储、observer 和后台协调器**

在 `combination_runs.rs` 提供以下接口：

```rust
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CombinationRunDetail {
    pub id: String,
    pub combination_id: Option<i64>,
    pub combination_name: String,
    pub execution_mode: ExecutionMode,
    pub status: String,
    pub result_code: Option<String>,
    pub error: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub steps: Vec<CombinationRunStep>,
}

pub(crate) fn create_run_with_conn<F>(
    conn: &mut Connection,
    combination_id: i64,
    snapshot_target: F,
) -> Result<CombinationRunDetail, String>
where
    F: Fn(&str, &str) -> AtomicTargetSnapshot;

pub(crate) fn get_run_with_conn(conn: &Connection, run_id: &str) -> Result<CombinationRunDetail, String>;
pub(crate) fn list_runs_with_conn(conn: &Connection, combination_id: i64) -> Result<Vec<CombinationRunDetail>, String>;
pub(crate) fn recover_interrupted_with_conn(conn: &Connection) -> Result<usize, String>;
pub(crate) fn start_with_app(app: &tauri::AppHandle, combination_id: i64) -> Result<CombinationRunDetail, String>;
```

全局门禁使用 `OnceLock<Mutex<Option<String>>>` 保存当前 run ID，但最终互斥由 `idx_action_combination_runs_one_active` 保证。启动顺序固定为：创建快照事务成功、占用内存槽、`thread::Builder::spawn`；spawn 失败时立即把 run 标记为 `failed/start_failed` 并释放槽。

`DatabaseRunObserver` 每次回调都获取独立 `db_conn()`，更新步骤后调用：

```rust
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CombinationRunUpdatedEvent<'a> {
    run_id: &'a str,
    status: &'a str,
}

app.emit(
    crate::events::EVENT_ACTION_CENTER_COMBINATION_RUN_UPDATED,
    CombinationRunUpdatedEvent { run_id, status },
).map_err(|error| format!("发送组合动作状态失败: {error}"))?;
```

事件失败只写 `eprintln!`，不能修改 run 或步骤业务结果；页面轮询仍能读取真实状态。`list_runs_with_conn` 固定 `ORDER BY created_at DESC, id DESC LIMIT 20`。后台协调器用 finally 等价的 guard 确保聚合、落库或事件通知发生错误时都释放内存运行槽。

- [ ] **Step 4: 运行运行态与执行器测试**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml combination_runs -- --nocapture`

Expected: PASS。

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml combination_executor -- --nocapture`

Expected: PASS，observer 接口加入后纯执行器无回归。

- [ ] **Step 5: 提交组合运行协调器**

```powershell
git add apps/desktop/src-tauri/src/tools/action_center/combination_runs.rs apps/desktop/src-tauri/src/tools/action_center/mod.rs apps/desktop/src-tauri/src/events.rs
git commit -m "feat: 持久化组合动作运行态"
```

---

### Task 6: 暴露 IPC、事件契约和启动恢复

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/action_center/mod.rs`
- Modify: `apps/desktop/src-tauri/src/tools/contract_tests.rs`
- Modify: `apps/desktop/src-tauri/src/events.rs`
- Modify: `apps/desktop/src-tauri/src/main.rs`
- Modify: `apps/desktop/src/bridge/tauri.ts`
- Modify: `apps/desktop/src/bridge/events.ts`
- Test: `apps/desktop/src-tauri/src/tools/action_center/mod.rs`

- [ ] **Step 1: 写组合 IPC 和事件契约失败测试**

在 `action_center/mod.rs` 测试 payload 门禁，在现有契约测试中依赖 action/event 对账：

```rust
#[test]
fn combination_run_rejects_missing_positive_id() {
    for payload in [json!({}), json!({ "combinationId": 0 }), json!({ "combinationId": "1" })] {
        assert!(parse_combination_id(&payload).is_err());
    }
    assert_eq!(parse_combination_id(&json!({ "combinationId": 7 })).unwrap(), 7);
}

#[test]
fn combination_actions_are_in_supported_action_contract() {
    for action in [
        "combination_definition_list", "combination_target_list", "combination_list",
        "combination_get", "combination_save", "combination_delete",
        "combination_run", "combination_run_get", "combination_run_list",
    ] {
        assert!(supported_actions().contains(&action));
    }
}
```

- [ ] **Step 2: 运行契约测试并确认按预期失败**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml contract_tests -- --nocapture`

Expected: FAIL，新增 backend actions 或 Rust event 尚未在前端声明。

- [ ] **Step 3: 接通 action_center 分发、channel、event 和恢复**

`action_center/mod.rs` 的 `ACTIONS` 增加九个 snake_case action。普通 CRUD 在 `execute` 中分发；只有 `combination_run` 与现有 `dispatch` 一样在 `execute_with_app` 中使用 `AppHandle`。

`combination_list/get` 返回配置时通过原子适配器补齐当前 `targetLabel/available/unavailableReason`；失效目标仍保留。保存时把 `atomic_actions::validate_target` 传给事务 CRUD；运行快照调用 `atomic_actions::snapshot_target`，把失效原因写入该 planned step 的 `validation_error`，不能让整次运行创建失败。所有 `combinationId` 必须是正整数，`runId` 必须是去空白后的非空字符串。

前端 channel 必须逐项加入 `CHANNEL_MAP`：

```ts
"tool:action-center:combination-definition-list": { domain: "action_center", action: "combination_definition_list" },
"tool:action-center:combination-target-list": { domain: "action_center", action: "combination_target_list" },
"tool:action-center:combination-list": { domain: "action_center", action: "combination_list" },
"tool:action-center:combination-get": { domain: "action_center", action: "combination_get" },
"tool:action-center:combination-save": { domain: "action_center", action: "combination_save" },
"tool:action-center:combination-delete": { domain: "action_center", action: "combination_delete" },
"tool:action-center:combination-run": { domain: "action_center", action: "combination_run" },
"tool:action-center:combination-run-get": { domain: "action_center", action: "combination_run_get" },
"tool:action-center:combination-run-list": { domain: "action_center", action: "combination_run_list" },
```

事件两端固定同步：

```rust
pub const EVENT_ACTION_CENTER_COMBINATION_RUN_UPDATED: &str =
    "action-center://combination-run-updated";
```

```ts
ACTION_CENTER_COMBINATION_RUN_UPDATED: "action-center://combination-run-updated",
```

在 `main.rs` 现有 `recover_interrupted_dispatches()` 后调用 `recover_interrupted_combination_runs()`；两个恢复错误分别显式记录，互不吞并。

- [ ] **Step 4: 运行 action_center 与契约测试**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml action_center -- --nocapture`

Expected: PASS。

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml contract_tests -- --nocapture`

Expected: PASS，channel 和 event 双向对账无缺项。

- [ ] **Step 5: 提交 IPC 与恢复链路**

```powershell
git add apps/desktop/src-tauri/src/tools/action_center/mod.rs apps/desktop/src-tauri/src/tools/contract_tests.rs apps/desktop/src-tauri/src/events.rs apps/desktop/src-tauri/src/main.rs apps/desktop/src/bridge/tauri.ts apps/desktop/src/bridge/events.ts
git commit -m "feat: 暴露组合动作运行接口"
```

---

### Task 7: 建立前端类型、编辑纯函数与状态 composable

**Files:**

- Modify: `apps/desktop/src/types/action-center.ts`
- Modify: `apps/desktop/src/types/index.ts`
- Create: `apps/desktop/src/utils/actionCombination.ts`
- Create: `apps/desktop/src/utils/actionCombination.test.ts`
- Create: `apps/desktop/src/composables/useActionCombinations.ts`
- Create: `apps/desktop/src/composables/useActionCombinations.test.ts`

- [ ] **Step 1: 写 draft、重排、晚响应和运行刷新失败测试**

```ts
it("normalizes a saved detail into an isolated editable draft", () => {
  const draft = createCombinationDraft({
    id: 7,
    name: "开发环境",
    executionMode: "parallel",
    steps: [{ id: 11, actionType: "hosts.activate", targetId: "2", sortOrder: 0 }],
    createdAt: "2026-07-26 10:00:00",
    updatedAt: "2026-07-26 10:00:00",
  });
  draft.steps[0].targetId = "3";
  expect(toCombinationSaveInput(draft)).toEqual({
    id: 7,
    name: "开发环境",
    executionMode: "parallel",
    steps: [{ actionType: "hosts.activate", targetId: "3" }],
  });
});

it("moves steps without mutating the original array", () => {
  const source = [step("a"), step("b"), step("c")];
  expect(moveCombinationStep(source, 2, 0).map((item) => item.localId)).toEqual(["c", "a", "b"]);
  expect(source.map((item) => item.localId)).toEqual(["a", "b", "c"]);
});

it("ignores target responses for an old action selection", async () => {
  const first = deferredTargets();
  invokeMock
    .mockReturnValueOnce(first.promise)
    .mockResolvedValueOnce({ targets: [target("edge")] });
  const state = useActionCombinations({ pollIntervalMs: 10_000 });
  const oldRequest = state.loadStepTargets("step-1", "hosts.activate");
  await state.loadStepTargets("step-1", "browser_profile.launch");
  first.resolve({ targets: [target("hosts")] });
  await oldRequest;
  expect(state.stepTargets.value.get("step-1")?.[0].id).toBe("edge");
});

it("reloads the active run from an event and stops polling at terminal state", async () => {
  const state = useActionCombinations({ pollIntervalMs: 5 });
  await state.trackRun(runningRun("run-1"));
  emitRunUpdate({ runId: "run-1", status: "succeeded" });
  await flushPromises();
  expect(invokeMock).toHaveBeenCalledWith("tool:action-center:combination-run-get", {
    runId: "run-1",
  });
  expect(state.activeRun.value?.status).toBe("succeeded");
});
```

- [ ] **Step 2: 运行前端测试并确认按预期失败**

Run: `pnpm --filter @lazycat/desktop test -- src/utils/actionCombination.test.ts src/composables/useActionCombinations.test.ts`

Expected: FAIL，提示新模块和类型不存在。

- [ ] **Step 3: 实现类型、纯函数和 composable**

在 `types/action-center.ts` 保留现有单动作类型，并新增：

```ts
// Add to the existing ActionDefinition returned by the shared registry.
supportsCombination: boolean;

export interface CombinationAtomicDefinition {
  actionType: string;
  label: string;
  targetKind: string;
  targetToolId: string;
}

export interface ActionCombinationTarget {
  id: string;
  label: string;
  available: boolean;
  unavailableReason?: string;
}

export interface ActionCombinationStep {
  id: number;
  actionType: string;
  targetId: string;
  sortOrder: number;
  targetLabel?: string;
  available?: boolean;
  unavailableReason?: string;
}

export interface ActionCombinationDetail {
  id: number;
  name: string;
  executionMode: ActionCombinationExecutionMode;
  steps: ActionCombinationStep[];
  createdAt: string;
  updatedAt: string;
}

export interface ActionCombinationSummary {
  id: number;
  name: string;
  executionMode: ActionCombinationExecutionMode;
  stepCount: number;
  latestRunStatus?: ActionCombinationRunStatus;
  updatedAt: string;
}

export interface ActionCombinationRunStep {
  id: number;
  actionType: string;
  actionLabel: string;
  targetId: string;
  targetLabel: string;
  sortOrder: number;
  status: ActionCombinationStepStatus;
  resultCode?: string;
  message?: string;
  startedAt?: string;
  finishedAt?: string;
}

export interface ActionCombinationRunDetail {
  id: string;
  combinationId?: number;
  combinationName: string;
  executionMode: ActionCombinationExecutionMode;
  status: ActionCombinationRunStatus;
  resultCode?: string;
  error?: string;
  createdAt: string;
  startedAt?: string;
  finishedAt?: string;
  steps: ActionCombinationRunStep[];
}
```

`actionCombination.ts` 导出 `createEmptyCombinationDraft`、`createCombinationDraft`、`toCombinationSaveInput`、`moveCombinationStep`、`isCombinationRunTerminal`、`combinationRunStatusLabel` 和 `combinationStepStatusLabel`。local step ID 使用模块内递增序号或 `crypto.randomUUID()`，只服务前端 key，不传给后端。

`useActionCombinations` 必须集中持有 definitions、summaries、selected detail、draft、dirty、per-step targets、active run 和 history；所有 IPC 使用 `invokeToolByChannel`。目标请求版本按 `localStepId` 保存，事件监听在 `start()` 创建、`stop()` 释放；活动运行每 1000ms 轮询，进入终态立即清理 timer。删除、保存和运行失败向调用方抛出，不在 composable 内伪装成功。

- [ ] **Step 4: 运行前端定向测试**

Run: `pnpm --filter @lazycat/desktop test -- src/utils/actionCombination.test.ts src/composables/useActionCombinations.test.ts`

Expected: PASS。

- [ ] **Step 5: 提交前端状态层**

```powershell
git add apps/desktop/src/types/action-center.ts apps/desktop/src/types/index.ts apps/desktop/src/utils/actionCombination.ts apps/desktop/src/utils/actionCombination.test.ts apps/desktop/src/composables/useActionCombinations.ts apps/desktop/src/composables/useActionCombinations.test.ts
git commit -m "feat: 添加组合动作前端状态"
```

---

### Task 8: 实现动作中心页面并接入工具目录

**Files:**

- Create: `apps/desktop/src/components/ActionCenterPanel.vue`
- Create: `apps/desktop/src/components/action-center/ActionCombinationList.vue`
- Create: `apps/desktop/src/components/action-center/ActionCombinationEditor.vue`
- Create: `apps/desktop/src/components/action-center/ActionRunHistory.vue`
- Create: `apps/desktop/src/components/ActionCenterPanel.test.ts`
- Modify: `apps/desktop/src/tool-registry.ts`
- Modify: `apps/desktop/src/composables/toolCatalog.ts`
- Modify: `apps/desktop/src/composables/toolCatalog.test.ts`

- [ ] **Step 1: 写工具注册和页面关键行为失败测试**

```ts
it("registers the action center as a real tool", () => {
  expect(getAllTools()).toContainEqual(
    expect.objectContaining({ id: "action-center", name: "动作中心" }),
  );
  expect(isRealToolId("action-center")).toBe(true);
});

it("keeps run guarded by saved state and exposes step results", () => {
  const panelSource = readFileSync(new URL("./ActionCenterPanel.vue", import.meta.url), "utf8");
  const editorSource = readFileSync(
    new URL("./action-center/ActionCombinationEditor.vue", import.meta.url),
    "utf8",
  );
  expect(panelSource).toContain("ActionRunHistory");
  expect(panelSource).toContain('@run="runCombination"');
  expect(editorSource).toContain(':disabled="dirty || !canRun || runActive"');
});

it("uses SortableJS with a dedicated drag handle", () => {
  const source = readFileSync(
    new URL("./action-center/ActionCombinationEditor.vue", import.meta.url),
    "utf8",
  );
  expect(source).toContain('import Sortable from "sortablejs"');
  expect(source).toContain('handle: ".action-step-drag"');
  expect(source).toContain("sortable?.destroy()");
});
```

- [ ] **Step 2: 运行组件与目录测试并确认按预期失败**

Run: `pnpm --filter @lazycat/desktop test -- src/components/ActionCenterPanel.test.ts src/composables/toolCatalog.test.ts`

Expected: FAIL，页面和工具注册不存在。

- [ ] **Step 3: 实现双栏页面、步骤编辑和结果历史**

`ActionCenterPanel.vue` 只编排 composable、确认弹窗和消息反馈：

```vue
<template>
  <section class="action-center-panel">
    <ActionCombinationList
      :items="combinations"
      :selected-id="selectedId"
      :run-active="runActive"
      @create="createCombination"
      @select="selectCombination"
    />
    <main class="action-center-workspace">
      <ActionCombinationEditor
        v-if="draft"
        v-model="draft"
        :definitions="definitions"
        :targets="stepTargets"
        :dirty="dirty"
        :run-active="runActive"
        @load-targets="loadStepTargets"
        @save="saveCombination"
        @copy="copyCombination"
        @delete="confirmDeleteCombination"
        @run="runCombination"
        @reorder="reorderSteps"
        @open-tool="$emit('open-tool', $event)"
      />
      <ActionRunHistory :active-run="activeRun" :history="runHistory" />
    </main>
  </section>
</template>
```

交互要求：

- 左栏不是卡片嵌套，只使用紧凑列表项；主编辑区为无额外浮卡的工作面。
- 串行/并行使用 `el-segmented`；新增步骤使用带 `Plus` 图标按钮；拖拽使用 `Rank` 图标按钮并带 `title="拖动排序"`。
- 步骤行固定列宽约束，动作和目标选择在窄屏改为两行，不允许按钮文字溢出。
- 动作变化先把 `targetId` 清空，再请求目标；失效目标保留禁用 option、错误原因和带 `Link` 图标的工具跳转。
- dirty、空名称、空步骤、未选目标或存在失效目标时禁用运行；运行中锁定全部组合的编辑、复制、删除和运行入口。
- 删除使用 `ElMessageBox.confirm`；复制生成 `${name} 副本` 并立即进入 dirty draft，保存前不写库。
- 完成通知按 `succeeded / partially_succeeded / failed` 分别使用 success/warning/error，正文列出失败步骤，不只显示“执行失败”。
- `ActionRunHistory` 展示当前步骤进度和最近 20 条折叠记录；所有状态标签从纯函数读取，不在模板复制映射。

在 `tool-registry.ts` 注册：

```ts
"action-center": defineAsyncComponent(() => import("./components/ActionCenterPanel.vue")),
```

在“更多工具”中加入：

```ts
{ id: "action-center", name: "动作中心", desc: "组合并一键运行常用开发动作" },
```

- [ ] **Step 4: 运行组件测试、类型检查和渲染层构建**

Run: `pnpm --filter @lazycat/desktop test -- src/components/ActionCenterPanel.test.ts src/composables/toolCatalog.test.ts src/utils/actionCombination.test.ts src/composables/useActionCombinations.test.ts`

Expected: PASS。

Run: `pnpm typecheck`

Expected: PASS，无 Vue props/emits 或组合契约类型错误。

Run: `pnpm --filter @lazycat/desktop build:web`

Expected: PASS，动作中心异步 chunk 正常生成且不引用公网资源。

- [ ] **Step 5: 提交动作中心页面**

```powershell
git add apps/desktop/src/components/ActionCenterPanel.vue apps/desktop/src/components/action-center apps/desktop/src/components/ActionCenterPanel.test.ts apps/desktop/src/tool-registry.ts apps/desktop/src/composables/toolCatalog.ts apps/desktop/src/composables/toolCatalog.test.ts
git commit -m "feat: 添加动作中心组合页面"
```

---

### Task 9: 补齐经验文档并执行完整验证

**Files:**

- Modify: `docs/experience/architecture.md`
- Verify: all files changed in Tasks 1-8

- [ ] **Step 1: 更新动作中心架构经验**

在 `docs/experience/architecture.md` 现有“跨工具动作使用注册表、适配器与派发状态机”章节补充组合动作结论：

```markdown
组合动作使用代码注册的原子动作和持久化目标引用；配置、步骤快照与运行事实分离。串行和并行只作为组合级模式，单步失败独立收口；数据库活动运行唯一索引是全局单运行真值，事件只用于刷新。目标适配器必须复用 Hosts、浏览器身份和请求转发的真实状态与执行链，不能复制配置或通过前端 IPC 自调用。
```

把该经验“使用次数”加 1；不新建重复主题，不修改 `AGENTS.md` 或 `CLAUDE.md`。

- [ ] **Step 2: 运行 Rust 定向测试**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml action_center -- --nocapture`

Expected: PASS。

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml request_forward -- --nocapture`

Expected: PASS。

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml contract_tests -- --nocapture`

Expected: PASS。

- [ ] **Step 3: 运行前端定向测试与全量类型检查**

Run: `pnpm --filter @lazycat/desktop test -- src/components/ActionCenterPanel.test.ts src/composables/useActionCombinations.test.ts src/utils/actionCombination.test.ts src/composables/toolCatalog.test.ts`

Expected: PASS。

Run: `pnpm typecheck`

Expected: PASS。

- [ ] **Step 4: 运行渲染层构建和差异检查**

Run: `pnpm --filter @lazycat/desktop build:web`

Expected: PASS。

Run: `git diff --check`

Expected: 无输出，退出码 0。

Run: `git status --short`

Expected: 只出现本任务尚未提交的经验文档，或工作区干净；不得夹带无关文件。

- [ ] **Step 5: 提交文档与最终验证记录**

```powershell
git add docs/experience/architecture.md
git commit -m "docs: 沉淀组合动作执行经验"
```

最终不要自动启动 `pnpm dev` 或产品 UI；真实 Hosts/UAC、浏览器和请求转发副作用留给用户手动冒烟。
