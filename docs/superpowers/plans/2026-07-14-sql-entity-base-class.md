# SQL 转 Java 实体类基类管理 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 SQL 转 Java 实体类工具增加可持久化的基类管理、多基类字段排除和单一实际父类继承能力。

**Architecture:** SQLite 单表保存基类别名、完整类名和字段 JSON；独立 `sql_entity` Rust 域提供 CRUD。前端加载配置并把本次选择快照传给 `convert::sql_to_entity`，生成器在命名转换后过滤字段，再基于剩余字段计算 import、MyBatis-Plus 注解和类声明。

**Tech Stack:** Tauri 2、Rust、rusqlite、serde_json、Vue 3、TypeScript、Element Plus、Vitest。

## Global Constraints

- 功能只作用于 Java，TypeScript、Go、Python、Kotlin、C# 输出保持不变。
- 可以选择多个基类配置，但只能指定其中一个生成 `extends`；其余配置只参与字段排除。
- 基类字段按生成后的 Java 属性名区分大小写精确匹配。
- 别名与完整类名均唯一。
- 未选择基类时 Java 输出必须与当前行为一致。
- 所有外部资源保持本地打包，本任务不新增依赖。
- 修改数据库结构仅新增 `CREATE TABLE IF NOT EXISTS`，不改写已有业务数据。

---

## File Map

- Create: `apps/desktop/src/types/sql-entity.ts` — 前后端 DTO 与表单类型。
- Create: `apps/desktop/src/utils/sqlEntityBaseClass.ts` — 字段解析、Java 标识符校验、选择修正纯函数。
- Create: `apps/desktop/src/utils/sqlEntityBaseClass.test.ts` — 前端规则单测。
- Create: `apps/desktop/src-tauri/src/tools/sql_entity.rs` — 基类表 schema、校验和 CRUD action。
- Create: `apps/desktop/src/components/SqlEntityBaseClassDialog.vue` — 基类管理弹窗。
- Modify: `apps/desktop/src-tauri/src/tools/helpers.rs` — 初始化基类表。
- Modify: `apps/desktop/src-tauri/src/tools/mod.rs` — 注册 `sql_entity` 域和 supported actions。
- Modify: `apps/desktop/src-tauri/src/tools/convert.rs` — Java 基类选项解析、字段过滤、继承生成和 Rust 测试。
- Modify: `apps/desktop/src/bridge/tauri.ts` — 注册四个 CRUD 通道。
- Modify: `apps/desktop/src/components/SqlEntityPanel.vue` — 加载配置、选择基类、指定父类、传递生成参数。
- Modify: `process.md` — 记录“先过滤字段再统一推导生成依赖”的经验。

---

### Task 1: 前端基类模型和纯函数

**Files:**
- Create: `apps/desktop/src/types/sql-entity.ts`
- Create: `apps/desktop/src/utils/sqlEntityBaseClass.ts`
- Test: `apps/desktop/src/utils/sqlEntityBaseClass.test.ts`

**Interfaces:**
- Produces: `SqlEntityBaseClass`、`SqlEntityBaseClassDraft`、`parseBaseClassFields()`、`validateJavaQualifiedName()`、`reconcileBaseClassSelection()`。
- Consumes: 无。

- [ ] **Step 1: 写字段清洗和选择修正失败测试**

```ts
import { describe, expect, it } from "vitest";
import {
  parseBaseClassFields,
  reconcileBaseClassSelection,
  validateJavaQualifiedName,
} from "./sqlEntityBaseClass";

describe("sqlEntityBaseClass", () => {
  it("按逗号和换行拆分字段并保持首次出现顺序", () => {
    expect(parseBaseClassFields("id, createdAt\nupdatedAt, id")).toEqual([
      "id",
      "createdAt",
      "updatedAt",
    ]);
  });

  it("拒绝非法 Java 完整类名和字段名", () => {
    expect(validateJavaQualifiedName("com.example.BaseEntity")).toBe("");
    expect(validateJavaQualifiedName("com.example.1Base")).toBe("完整类名包含非法 Java 标识符：1Base");
    expect(() => parseBaseClassFields("created-at")).toThrow("非法 Java 字段名：created-at");
  });

  it("单选时自动设为父类，移除父类时回退到第一项", () => {
    expect(reconcileBaseClassSelection([2], null, [1, 2, 3])).toEqual({
      selectedIds: [2],
      parentId: 2,
    });
    expect(reconcileBaseClassSelection([1, 3], 2, [1, 3])).toEqual({
      selectedIds: [1, 3],
      parentId: 1,
    });
  });

  it("清理已经删除的基类选择", () => {
    expect(reconcileBaseClassSelection([1, 2], 2, [1])).toEqual({
      selectedIds: [1],
      parentId: 1,
    });
  });
});
```

- [ ] **Step 2: 运行测试并确认因模块不存在而失败**

Run: `pnpm test src/utils/sqlEntityBaseClass.test.ts`

Expected: FAIL，提示无法解析 `./sqlEntityBaseClass`。

- [ ] **Step 3: 创建类型和最小纯函数实现**

```ts
// apps/desktop/src/types/sql-entity.ts
export interface SqlEntityBaseClass {
  id: number;
  alias: string;
  qualifiedName: string;
  fields: string[];
  sortOrder: number;
  createdAt: string;
  updatedAt: string;
}

export interface SqlEntityBaseClassDraft {
  alias: string;
  qualifiedName: string;
  fieldsText: string;
}

export interface SqlEntityBaseClassListResponse {
  items: SqlEntityBaseClass[];
}
```

```ts
// apps/desktop/src/utils/sqlEntityBaseClass.ts
const JAVA_IDENTIFIER = /^[A-Za-z_$][A-Za-z0-9_$]*$/;

export function validateJavaQualifiedName(value: string): string {
  const normalized = value.trim();
  if (!normalized) return "完整类名不能为空";
  const invalid = normalized.split(".").find((part) => !JAVA_IDENTIFIER.test(part));
  return invalid ? `完整类名包含非法 Java 标识符：${invalid}` : "";
}

export function parseBaseClassFields(input: string): string[] {
  const result: string[] = [];
  const seen = new Set<string>();
  for (const field of input.split(/[\n,]/).map((item) => item.trim()).filter(Boolean)) {
    if (!JAVA_IDENTIFIER.test(field)) throw new Error(`非法 Java 字段名：${field}`);
    if (!seen.has(field)) {
      seen.add(field);
      result.push(field);
    }
  }
  return result;
}

export function reconcileBaseClassSelection(
  selectedIds: number[],
  parentId: number | null,
  availableIds: number[],
): { selectedIds: number[]; parentId: number | null } {
  const available = new Set(availableIds);
  const nextSelected = selectedIds.filter((id) => available.has(id));
  if (nextSelected.length === 0) return { selectedIds: [], parentId: null };
  if (nextSelected.length === 1) return { selectedIds: nextSelected, parentId: nextSelected[0] };
  return {
    selectedIds: nextSelected,
    parentId: parentId !== null && nextSelected.includes(parentId) ? parentId : nextSelected[0],
  };
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `pnpm test src/utils/sqlEntityBaseClass.test.ts`

Expected: PASS，4 个测试通过。

- [ ] **Step 5: 提交前端规则基础**

```powershell
git add apps/desktop/src/types/sql-entity.ts apps/desktop/src/utils/sqlEntityBaseClass.ts apps/desktop/src/utils/sqlEntityBaseClass.test.ts
git commit -m "feat(sql-entity): 添加基类配置规则模型"
```

---

### Task 2: SQLite schema、Rust CRUD 和 bridge 契约

**Files:**
- Create: `apps/desktop/src-tauri/src/tools/sql_entity.rs`
- Modify: `apps/desktop/src-tauri/src/tools/helpers.rs`
- Modify: `apps/desktop/src-tauri/src/tools/mod.rs`
- Modify: `apps/desktop/src/bridge/tauri.ts`

**Interfaces:**
- Produces domain: `sql_entity`。
- Produces actions: `base_class_list`、`base_class_create`、`base_class_update`、`base_class_delete`。
- Produces channels: `tool:sql-entity:base-class-list/create/update/delete`。

- [ ] **Step 1: 在新模块中先写 in-memory CRUD 失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use serde_json::json;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SQL_ENTITY_SCHEMA_SQL).unwrap();
        conn
    }

    #[test]
    fn base_class_crud_normalizes_fields_and_preserves_order() {
        let conn = test_conn();
        let created = create_with_conn(&conn, &json!({
            "alias": "审计基类",
            "qualifiedName": "com.example.AuditEntity",
            "fields": ["createdAt", "updatedAt", "createdAt"]
        })).unwrap();
        assert_eq!(created["item"]["fields"], json!(["createdAt", "updatedAt"]));

        let listed = list_with_conn(&conn).unwrap();
        assert_eq!(listed["items"].as_array().unwrap().len(), 1);

        let id = created["item"]["id"].as_i64().unwrap();
        update_with_conn(&conn, &json!({
            "id": id,
            "alias": "基础审计",
            "qualifiedName": "com.example.AuditEntity",
            "fields": ["createdAt"]
        })).unwrap();
        delete_with_conn(&conn, &json!({ "id": id })).unwrap();
        assert!(list_with_conn(&conn).unwrap()["items"].as_array().unwrap().is_empty());
    }

    #[test]
    fn rejects_duplicate_alias_and_invalid_java_names() {
        let conn = test_conn();
        create_with_conn(&conn, &json!({
            "alias": "基础",
            "qualifiedName": "com.example.BaseEntity",
            "fields": ["id"]
        })).unwrap();
        let duplicate = create_with_conn(&conn, &json!({
            "alias": "基础",
            "qualifiedName": "com.example.OtherEntity",
            "fields": []
        })).unwrap_err();
        assert!(duplicate.contains("别名已存在"));

        let invalid = create_with_conn(&conn, &json!({
            "alias": "非法",
            "qualifiedName": "com.example.1Base",
            "fields": ["created-at"]
        })).unwrap_err();
        assert!(invalid.contains("非法 Java 标识符"));
    }
}
```

- [ ] **Step 2: 运行测试并确认新模块尚未注册而失败**

Run: `cargo test sql_entity:: -- --nocapture`

Expected: FAIL，新模块或函数不存在。

- [ ] **Step 3: 实现 schema、校验和 CRUD**

在 `sql_entity.rs` 定义：

```rust
use rusqlite::{params, Connection, ErrorCode};
use serde_json::{json, Value};
use std::collections::HashSet;

use super::helpers::db_conn;

pub const SQL_ENTITY_SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS sql_entity_base_classes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    alias TEXT NOT NULL UNIQUE,
    qualified_name TEXT NOT NULL UNIQUE,
    fields_json TEXT NOT NULL DEFAULT '[]',
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_sql_entity_base_classes_sort
    ON sql_entity_base_classes(sort_order ASC, id ASC);
"#;

const ACTIONS: &[&str] = &[
    "base_class_list",
    "base_class_create",
    "base_class_update",
    "base_class_delete",
];

#[cfg(test)]
pub(crate) fn supported_actions() -> &'static [&'static str] { ACTIONS }

fn is_java_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some(c) if c == '_' || c == '$' || c.is_ascii_alphabetic())
        && chars.all(|c| c == '_' || c == '$' || c.is_ascii_alphanumeric())
}

pub(crate) fn validate_java_qualified_name(value: &str) -> Result<String, String> {
    let normalized = value.trim();
    if normalized.is_empty() { return Err("完整类名不能为空".into()); }
    for part in normalized.split('.') {
        if !is_java_identifier(part) {
            return Err(format!("完整类名包含非法 Java 标识符：{part}"));
        }
    }
    Ok(normalized.to_string())
}

pub(crate) fn normalize_java_fields(payload: &Value) -> Result<Vec<String>, String> {
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for value in payload.as_array().ok_or("字段列表格式错误")? {
        let field = value.as_str().ok_or("字段列表格式错误")?.trim();
        if !is_java_identifier(field) {
            return Err(format!("非法 Java 标识符：{field}"));
        }
        if seen.insert(field.to_string()) { result.push(field.to_string()); }
    }
    Ok(result)
}
```

CRUD helper 使用以下固定签名和 SQL：

```rust
fn list_with_conn(conn: &Connection) -> Result<Value, String>;
fn create_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String>;
fn update_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String>;
fn delete_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String>;

const LIST_SQL: &str = "SELECT id, alias, qualified_name, fields_json, sort_order, created_at, updated_at FROM sql_entity_base_classes ORDER BY sort_order ASC, id ASC";
const INSERT_SQL: &str = "INSERT INTO sql_entity_base_classes(alias, qualified_name, fields_json, sort_order, updated_at) VALUES(?1, ?2, ?3, (SELECT COALESCE(MAX(sort_order), 0) + 1 FROM sql_entity_base_classes), CURRENT_TIMESTAMP)";
const UPDATE_SQL: &str = "UPDATE sql_entity_base_classes SET alias = ?1, qualified_name = ?2, fields_json = ?3, updated_at = CURRENT_TIMESTAMP WHERE id = ?4";
const DELETE_SQL: &str = "DELETE FROM sql_entity_base_classes WHERE id = ?1";
```

`create_with_conn` 和 `update_with_conn` 必须调用 `validate_java_qualified_name()`、`normalize_java_fields()`，并把字段用 `serde_json::to_string` 保存。唯一约束错误根据 `ErrorCode::ConstraintViolation` 查询 alias/qualified_name 是否已存在，分别返回“别名已存在”和“完整类名已存在”；update/delete 影响行数为 0 时返回“基类不存在”。公开 `execute()` 只打开 `db_conn()` 并按 action 转发到上述 helper。

- [ ] **Step 4: 接入 schema、domain、supported actions 和 CHANNEL_MAP**

在 `helpers.rs::ensure_schema` 调用：

```rust
conn.execute_batch(super::sql_entity::SQL_ENTITY_SCHEMA_SQL)
    .map_err(|e| format!("create sql entity schema failed: {e}"))?;
```

在 `tools/mod.rs` 同步增加：

```rust
pub mod sql_entity;
// dispatch_tool
"sql_entity" => sql_entity::execute(action, payload),
// supported_actions
"sql_entity" => Some(sql_entity::supported_actions()),
```

在 `bridge/tauri.ts` 增加四行：

```ts
"tool:sql-entity:base-class-list": { domain: "sql_entity", action: "base_class_list" },
"tool:sql-entity:base-class-create": { domain: "sql_entity", action: "base_class_create" },
"tool:sql-entity:base-class-update": { domain: "sql_entity", action: "base_class_update" },
"tool:sql-entity:base-class-delete": { domain: "sql_entity", action: "base_class_delete" },
```

- [ ] **Step 5: 运行 CRUD 与契约测试**

Run: `cargo test sql_entity:: -- --nocapture`

Expected: PASS。

Run: `cargo test contract_tests -- --nocapture`

Expected: PASS，四个新通道均能在后端 supported actions 中找到。

- [ ] **Step 6: 提交持久化与契约**

```powershell
git add apps/desktop/src-tauri/src/tools/sql_entity.rs apps/desktop/src-tauri/src/tools/helpers.rs apps/desktop/src-tauri/src/tools/mod.rs apps/desktop/src/bridge/tauri.ts
git commit -m "feat(sql-entity): 添加基类配置持久化"
```

---

### Task 3: Java 生成器支持基类字段排除与继承

**Files:**
- Modify: `apps/desktop/src-tauri/src/tools/convert.rs`

**Interfaces:**
- Consumes: `options.baseClasses`、`options.parentBaseClassId`。
- Produces: Java `import`、`extends` 和过滤后的字段集合。

- [ ] **Step 1: 写生成行为失败测试**

在 `convert.rs` 现有测试模块增加：

```rust
#[test]
fn sql_to_entity_java_excludes_selected_base_fields_and_extends_parent() {
    let result = execute("sql_to_entity", &json!({
        "sql": "CREATE TABLE t_user (id BIGINT NOT NULL AUTO_INCREMENT, tenant_id BIGINT, created_at DATETIME, name VARCHAR(100), PRIMARY KEY (id));",
        "language": "java",
        "options": {
            "comments": false,
            "naming": "camelCase",
            "mybatisPlus": true,
            "baseClasses": [
                { "id": 1, "alias": "基础", "qualifiedName": "com.example.BaseEntity", "fields": ["id", "createdAt"] },
                { "id": 2, "alias": "租户", "qualifiedName": "com.example.TenantFields", "fields": ["tenantId"] }
            ],
            "parentBaseClassId": 1
        }
    })).unwrap();
    let code = result["code"].as_str().unwrap();
    assert!(code.contains("import com.example.BaseEntity;"));
    assert!(code.contains("public class User extends BaseEntity"));
    assert!(code.contains("private String name;"));
    assert!(!code.contains("private Long id;"));
    assert!(!code.contains("private Long tenantId;"));
    assert!(!code.contains("private LocalDateTime createdAt;"));
    assert!(!code.contains("TableId"));
    assert!(!code.contains("IdType"));
    assert!(!code.contains("java.time.LocalDateTime"));
    assert!(!code.contains("TenantFields"));
}

#[test]
fn sql_to_entity_java_rejects_parent_outside_selection() {
    let error = execute("sql_to_entity", &json!({
        "sql": "CREATE TABLE users (id BIGINT);",
        "language": "java",
        "options": {
            "baseClasses": [{ "id": 1, "alias": "基础", "qualifiedName": "BaseEntity", "fields": ["id"] }],
            "parentBaseClassId": 2
        }
    })).unwrap_err();
    assert!(error.contains("实际父类必须属于已选基类"));
}

#[test]
fn sql_to_entity_java_rejects_invalid_base_class_snapshot() {
    let error = execute("sql_to_entity", &json!({
        "sql": "CREATE TABLE users (id BIGINT);",
        "language": "java",
        "options": {
            "baseClasses": [{ "id": 1, "alias": "非法", "qualifiedName": "com.example.1Base", "fields": ["created-at"] }],
            "parentBaseClassId": 1
        }
    })).unwrap_err();
    assert!(error.contains("非法 Java 标识符"));
}
```

- [ ] **Step 2: 运行测试确认因基类选项尚未生效而失败**

Run: `cargo test sql_to_entity_java_excludes_selected_base_fields_and_extends_parent -- --nocapture`

Expected: FAIL，输出仍包含全部字段且没有 `extends`。

- [ ] **Step 3: 实现 Java 基类选项解析**

在 `convert.rs` 增加：

```rust
#[derive(Debug, Clone)]
struct JavaBaseClassOption {
    id: i64,
    qualified_name: String,
    fields: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct JavaBaseOptions {
    excluded_fields: std::collections::HashSet<String>,
    parent_qualified_name: Option<String>,
}

fn parse_java_base_options(options: &Value) -> Result<JavaBaseOptions, String> {
    let items = options["baseClasses"].as_array().cloned().unwrap_or_default();
    if items.is_empty() return Ok(JavaBaseOptions::default());
    let parent_id = options["parentBaseClassId"].as_i64()
        .ok_or("已选择基类时必须指定实际父类")?;
    let mut result = JavaBaseOptions::default();
    for item in items {
        let id = item["id"].as_i64().ok_or("基类 ID 无效")?;
        let qualified_name = super::sql_entity::validate_java_qualified_name(
            item["qualifiedName"].as_str().ok_or("基类完整类名无效")?
        )?;
        for field in super::sql_entity::normalize_java_fields(&item["fields"])? {
            result.excluded_fields.insert(field);
        }
        if id == parent_id { result.parent_qualified_name = Some(qualified_name); }
    }
    if result.parent_qualified_name.is_none() {
        return Err("实际父类必须属于已选基类".into());
    }
    Ok(result)
}
```

- [ ] **Step 4: 在生成前统一过滤字段并基于过滤结果计算依赖**

将 `generate_java` 签名改为：

```rust
fn generate_java(
    table: &SqlTable,
    naming: &str,
    comments: bool,
    mybatis_plus: bool,
    base_options: &JavaBaseOptions,
) -> String
```

函数开头构建：

```rust
let included_columns: Vec<(&SqlColumn, String)> = table.columns.iter()
    .map(|column| (column, convert_field_name(&column.name, naming)))
    .filter(|(_, field_name)| !base_options.excluded_fields.contains(field_name))
    .collect();
```

后续类型 import、`TableField`/`TableId`/`IdType` 判断和字段输出全部遍历 `included_columns`。实际父类存在时加入完整类名 import（包含 `.` 才加入）并把类声明改为：

```rust
let parent_name = base_options.parent_qualified_name.as_deref()
    .and_then(|name| name.rsplit('.').next());
let extends_clause = parent_name.map(|name| format!(" extends {name}")).unwrap_or_default();
out.push_str(&format!("public class {}{} {{\n", class_name, extends_clause));
```

`sql_to_entity` 仅在 Java 分支调用 `parse_java_base_options(options)?`；其他语言不解析也不校验这些参数。

- [ ] **Step 5: 运行新增和现有 SQL 实体测试**

Run: `cargo test sql_to_entity_ -- --nocapture`

Expected: PASS，新增测试和现有 Java/TS/Go/Python/Kotlin/C# 测试全部通过。

- [ ] **Step 6: 提交生成器能力**

```powershell
git add apps/desktop/src-tauri/src/tools/convert.rs
git commit -m "feat(sql-entity): 支持基类继承和字段排除"
```

---

### Task 4: 基类管理弹窗与 SQL 实体面板接入

**Files:**
- Create: `apps/desktop/src/components/SqlEntityBaseClassDialog.vue`
- Modify: `apps/desktop/src/components/SqlEntityPanel.vue`

**Interfaces:**
- Consumes CRUD channels and Task 1 types/utilities。
- Produces `changed` event and exposed `open()` method。
- Sends selected base-class snapshots to `tool:convert:sql-to-entity`。

- [ ] **Step 1: 先扩充纯函数测试覆盖删除后的选择同步**

在 `sqlEntityBaseClass.test.ts` 增加：

```ts
it("保留可用选择的原顺序并去掉重复 ID", () => {
  expect(reconcileBaseClassSelection([3, 1, 3, 2], 3, [1, 2, 3])).toEqual({
    selectedIds: [3, 1, 2],
    parentId: 3,
  });
});
```

先运行测试，预期因当前函数未去重而 FAIL；随后在 `reconcileBaseClassSelection` 中增加 `seen` 集合去重，再运行确认 PASS。

- [ ] **Step 2: 实现管理弹窗**

弹窗状态采用显式列表/编辑模式：

```ts
const visible = ref(false);
const mode = ref<"list" | "edit">("list");
const items = ref<SqlEntityBaseClass[]>([]);
const editingId = ref<number | null>(null);
const draft = reactive<SqlEntityBaseClassDraft>({ alias: "", qualifiedName: "", fieldsText: "" });

async function loadItems() {
  const result = await invokeToolByChannel("tool:sql-entity:base-class-list", {}) as SqlEntityBaseClassListResponse;
  items.value = result.items;
}

async function save() {
  const qualifiedNameError = validateJavaQualifiedName(draft.qualifiedName);
  if (!draft.alias.trim()) return ElMessage.warning("请输入别名");
  if (qualifiedNameError) return ElMessage.warning(qualifiedNameError);
  let fields: string[];
  try { fields = parseBaseClassFields(draft.fieldsText); }
  catch (error) { return ElMessage.warning((error as Error).message); }
  const channel = editingId.value === null
    ? "tool:sql-entity:base-class-create"
    : "tool:sql-entity:base-class-update";
  await invokeToolByChannel(channel, {
    ...(editingId.value === null ? {} : { id: editingId.value }),
    alias: draft.alias.trim(),
    qualifiedName: draft.qualifiedName.trim(),
    fields,
  });
  await loadItems();
  mode.value = "list";
  emit("changed", items.value);
}
```

模板要求：

- `el-dialog` 标题“基类管理”。
- 列表模式有“新增基类”按钮和 `el-table`，列为别名、完整类名、字段数量、操作。
- 编辑模式有别名 input、完整类名 input、字段 textarea，提示“支持逗号或换行分隔，填写生成后的 Java 属性名”。
- 删除使用 `ElMessageBox.confirm` 二次确认，成功后重新加载并 emit。
- `defineExpose({ open })`，`open()` 每次先加载列表再展示。

- [ ] **Step 3: 接入 Java 工具栏和生成请求**

在 `SqlEntityPanel.vue` 增加状态：

```ts
const baseClasses = ref<SqlEntityBaseClass[]>([]);
const selectedBaseClassIds = ref<number[]>(sqlEntityState.selectedBaseClassIds);
const parentBaseClassId = ref<number | null>(sqlEntityState.parentBaseClassId);
const baseClassDialog = ref<InstanceType<typeof SqlEntityBaseClassDialog> | null>(null);

const selectedBaseClasses = computed(() => {
  const selected = new Set(selectedBaseClassIds.value);
  return baseClasses.value.filter((item) => selected.has(item.id));
});

function syncBaseClassSelection() {
  const next = reconcileBaseClassSelection(
    selectedBaseClassIds.value,
    parentBaseClassId.value,
    baseClasses.value.map((item) => item.id),
  );
  selectedBaseClassIds.value = next.selectedIds;
  parentBaseClassId.value = next.parentId;
}
```

Java 工具栏增加多选框、实际父类单选框和管理按钮；`generate()` 的 `options` 增加：

```ts
...(language.value === "java" ? {
  baseClasses: selectedBaseClasses.value,
  parentBaseClassId: parentBaseClassId.value,
} : {}),
```

`onMounted` 调用 list channel 加载配置；加载、弹窗 changed、选择变化后统一调用 `syncBaseClassSelection()`。在 `sqlEntityState` 和 `onBeforeUnmount` 中保存 `selectedBaseClassIds`、`parentBaseClassId`，只保持当前应用会话，不写 `user_settings`。

- [ ] **Step 4: 运行前端测试和类型检查**

Run: `pnpm test src/utils/sqlEntityBaseClass.test.ts`

Expected: PASS，5 个测试通过。

Run: `pnpm typecheck`

Expected: exit 0，无 TypeScript/Vue 类型错误。

- [ ] **Step 5: 运行 Web 构建验证模板和样式**

Run: `pnpm --filter @lazycat/desktop build:web`

Expected: exit 0，Vite 构建成功。

- [ ] **Step 6: 提交界面接入**

```powershell
git add apps/desktop/src/components/SqlEntityBaseClassDialog.vue apps/desktop/src/components/SqlEntityPanel.vue apps/desktop/src/utils/sqlEntityBaseClass.ts apps/desktop/src/utils/sqlEntityBaseClass.test.ts
git commit -m "feat(sql-entity): 添加基类管理和选择界面"
```

---

### Task 5: 经验沉淀与完整验证

**Files:**
- Modify: `process.md`

**Interfaces:**
- Consumes: 所有前序任务产物。
- Produces: 可交付、验证完成的功能与经验记录。

- [ ] **Step 1: 记录稳定工程经验**

在 `process.md` 末尾增加日期章节，内容聚焦：

```markdown
## 2026-07-14: SQL 实体生成器基类字段排除

**场景**: Java 实体生成需要从多个基类模板汇总字段排除，同时只生成一个合法父类继承声明。

1. 先把 SQL 列转换成最终 Java 属性名，再与基类字段集合精确匹配；不要混用数据库列名和属性名。
2. 字段过滤必须发生在类型 import、MyBatis-Plus 注解和字段正文生成之前，所有派生输出共享同一个过滤后集合，避免残留无用 import 或注解。
3. Java 多个“基类配置”应拆成一个实际父类和多个字段模板；只有实际父类生成 `extends/import`，其余模板只提供排除字段。
```

- [ ] **Step 2: 运行针对性 Rust 测试**

Run: `cargo test sql_entity:: -- --nocapture`

Expected: PASS。

Run: `cargo test sql_to_entity_ -- --nocapture`

Expected: PASS。

Run: `cargo test contract_tests -- --nocapture`

Expected: PASS。

- [ ] **Step 3: 运行前端测试、类型检查和构建**

Run: `pnpm test src/utils/sqlEntityBaseClass.test.ts`

Expected: PASS。

Run: `pnpm typecheck`

Expected: exit 0。

Run: `pnpm --filter @lazycat/desktop build:web`

Expected: exit 0。

- [ ] **Step 4: 检查最终 diff 和工作区**

Run: `git diff --check`

Expected: 无输出，exit 0。

Run: `git status --short`

Expected: 仅包含 `process.md` 的未提交修改；若出现其他文件，逐项核对是否属于本计划。

- [ ] **Step 5: 提交经验记录**

```powershell
git add process.md
git commit -m "docs(sql-entity): 记录基类字段过滤经验"
```

- [ ] **Step 6: 最终新鲜验证**

重新执行 Step 2 和 Step 3 的全部命令。只有所有命令均 exit 0 后，才能报告功能完成。
