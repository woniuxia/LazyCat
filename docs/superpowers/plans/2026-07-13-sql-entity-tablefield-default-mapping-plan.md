# SQL 实体 TableField 默认驼峰映射实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 标准数据库下划线列名转换为 Java 驼峰属性时，依赖 MyBatis-Plus 默认映射，不生成冗余 `@TableField`。

**Architecture:** 在现有 Rust Java 生成器中增加一个纯判断函数，将“属性名不同”改为“属性名既不同于原始列名，也不同于标准驼峰结果”。前端选项和 IPC 参数不变，`@TableName`、`@TableId`、`IdType.AUTO` 行为不变。

**Tech Stack:** Rust、regex、Cargo tests、Vue 3/TypeScript 现有前端接口。

## Global Constraints

- `created_at → createdAt`、`user_name → userName` 不生成 `@TableField`。
- Java 属性名等于数据库列名时不生成 `@TableField`。
- Java 属性名既不等于原始列名，也不等于标准驼峰结果时才生成 `@TableField`。
- `@TableName`、`@TableId`、`IdType.AUTO` 和前端 MyBatis-Plus 选项保持不变。
- 不修改前端组件、IPC 通道、数据库结构或其他语言生成器。
- 必须先修改测试并确认红灯，再修改生产逻辑。

---

## File Map

- Modify/Test: `apps/desktop/src-tauri/src/tools/convert.rs`
  - 增加 `needs_table_field(column_name, field_name)` 纯函数。
  - 复用该函数决定 `TableField` import 和字段注解。
  - 更新 MyBatis-Plus 生成测试并增加边界测试。

---

### Task 1: 用 TDD 修正 TableField 自动映射规则

**Files:**
- Modify: `apps/desktop/src-tauri/src/tools/convert.rs:630-662`
- Modify: `apps/desktop/src-tauri/src/tools/convert.rs:824-924`
- Test: `apps/desktop/src-tauri/src/tools/convert.rs:1666-1752`

**Interfaces:**
- Consumes: existing `to_camel_case(name: &str) -> String` and generated Java `field_name`.
- Produces: `needs_table_field(column_name: &str, field_name: &str) -> bool`.

- [ ] **Step 1: Change the integration expectations to the desired default mapping**

在 `sql_to_entity_java_mybatis_plus_annotations` 的 SQL 中增加：

```rust
created_at DATETIME NOT NULL,
```

将现有 `TableField` import 和 `user_name` 注解断言替换为：

```rust
assert!(!code.contains("import com.baomidou.mybatisplus.annotation.TableField;"));
assert!(!code.contains("@TableField"));
assert!(code.contains("private String userName;"));
assert!(code.contains("private LocalDateTime createdAt;"));
```

保留 `@TableName`、`@TableId(type = IdType.AUTO)`、`IdType` 和 `TableId` import 断言。

在 `sql_to_entity_java_mybatis_plus_composite_primary_key` 中将两个 `@TableField` 断言替换为：

```rust
assert!(!code.contains("import com.baomidou.mybatisplus.annotation.TableField;"));
assert!(!code.contains("@TableField"));
assert!(code.contains("private Long orderId;"));
assert!(code.contains("private Long itemId;"));
```

- [ ] **Step 2: Run the integration tests and verify RED**

Run from `apps/desktop/src-tauri`:

```powershell
cargo test sql_to_entity_java_mybatis_plus_ -- --nocapture
```

Expected: the annotation and composite-primary-key tests fail because the generator still imports and emits `@TableField` for standard snake-case columns.

- [ ] **Step 3: Add a focused unit test for the mapping predicate**

在 SQL 转实体测试区域新增：

```rust
#[test]
fn table_field_mapping_uses_default_camel_case() {
    assert!(!needs_table_field("email", "email"));
    assert!(!needs_table_field("created_at", "createdAt"));
    assert!(!needs_table_field("user_name", "userName"));
    assert!(needs_table_field("legacy_code", "legacyCodeValue"));
}
```

- [ ] **Step 4: Run the predicate test and verify RED**

Run from `apps/desktop/src-tauri`:

```powershell
cargo test table_field_mapping_uses_default_camel_case -- --nocapture
```

Expected: the predicate test fails to compile because `needs_table_field` does not exist. Keep the test active and proceed to the minimal implementation.

- [ ] **Step 5: Add the minimal mapping predicate**

紧跟 `to_camel_case` 后增加：

```rust
fn needs_table_field(column_name: &str, field_name: &str) -> bool {
    field_name != column_name && field_name != to_camel_case(column_name)
}
```

- [ ] **Step 6: Use the predicate for imports and annotations**

将 import 判断中的：

```rust
single_primary_key != Some(col.name.as_str()) && field_name != col.name
```

替换为：

```rust
single_primary_key != Some(col.name.as_str())
    && needs_table_field(&col.name, &field_name)
```

将字段注解判断中的：

```rust
} else if mybatis_plus && field_name != col.name {
```

替换为：

```rust
} else if mybatis_plus && needs_table_field(&col.name, &field_name) {
```

主键的 `renamed` 判定保持现状，继续由 `@TableId` 显式携带原始列名。

- [ ] **Step 7: Run focused tests and verify GREEN**

Run from `apps/desktop/src-tauri`:

```powershell
cargo test table_field_mapping_uses_default_camel_case -- --nocapture
cargo test sql_to_entity_java_mybatis_plus_ -- --nocapture
```

Expected: predicate test 1/1 通过，MyBatis-Plus Java integration tests 全部通过。

- [ ] **Step 8: Run all SQL-to-entity regressions**

Run from `apps/desktop/src-tauri`:

```powershell
cargo test sql_to_entity_ -- --nocapture
```

Expected: all SQL-to-entity tests pass with zero failures; other languages remain unchanged.

- [ ] **Step 9: Run workspace validation**

Run from repository root:

```powershell
pnpm test
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
```

Expected: all commands exit 0. The renderer build may retain the existing large-chunk warning, but must complete successfully.

- [ ] **Step 10: Inspect and commit only the Rust change**

```powershell
git diff --check -- apps/desktop/src-tauri/src/tools/convert.rs
git add apps/desktop/src-tauri/src/tools/convert.rs
git commit -m "fix(sql-entity): 避免生成冗余 TableField 注解"
```

Confirm the commit contains only `apps/desktop/src-tauri/src/tools/convert.rs`; preserve unrelated modified or untracked documentation files.
