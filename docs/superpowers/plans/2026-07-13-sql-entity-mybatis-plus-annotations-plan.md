# SQL 转 Java 实体 MyBatis-Plus 注解实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为“SQL 转实体类”的 Java 输出增加默认关闭的 MyBatis-Plus 注解选项，生成 `@TableName`、必要的 `@TableField` 和安全的单主键 `@TableId`。

**Architecture:** 保持现有前端通道和多语言生成结构不变，在 Rust SQL 解析模型中补充主键与自增元数据，再由 Java 生成器依据 `options.mybatisPlus` 结构化输出注解。前端只负责显示 Java 专属复选框、记忆状态并传递布尔选项；其他语言和默认关闭路径保持兼容。

**Tech Stack:** Vue 3、TypeScript、Element Plus、Tauri 2、Rust、serde_json、regex、Cargo tests、Vitest workspace、Vite build。

## Global Constraints

- MyBatis-Plus 注解选项仅对 Java 生效，默认值必须为 `false`。
- 关闭选项时，现有 Java 输出不得增加任何 MyBatis-Plus import 或注解。
- `@TableField` 仅在 Java 属性名与数据库列名不一致时生成。
- 单主键才生成 `@TableId`；复合主键不得生成多个无效的 `@TableId`。
- 自增单主键使用 `IdType.AUTO`，且仅在需要时导入 `IdType`。
- 不修改 IPC 通道、数据库结构、其他语言生成规则，不增加依赖。
- 先完成失败测试并确认按预期失败，再写生产代码。

---

## File Map

- Modify: `apps/desktop/src-tauri/src/tools/convert.rs`
  - 扩展 SQL 表/列解析元数据。
  - 解析列级、表级主键与 `AUTO_INCREMENT`。
  - 按 MyBatis-Plus 选项生成 Java imports 和注解。
  - 在同文件 `#[cfg(test)]` 模块增加回归测试。
- Modify: `apps/desktop/src/components/SqlEntityPanel.vue`
  - 增加 Java 专属复选框、模块级状态记忆和请求参数。

---

### Task 1: 用失败测试固定 MyBatis-Plus Java 输出契约

**Files:**

- Modify/Test: `apps/desktop/src-tauri/src/tools/convert.rs:1512-1733`

**Interfaces:**

- Consumes: existing `execute("sql_to_entity", payload)` test entrypoint.
- Produces: executable expectations for `options.mybatisPlus`, annotation imports, single primary keys, auto increment, renamed fields, and composite primary keys.

- [ ] **Step 1: Strengthen the legacy-default test**

在 `sql_to_entity_java_basic` 的现有断言末尾、读取 `tables` 之前加入：

```rust
assert!(!code.contains("com.baomidou.mybatisplus.annotation"));
assert!(!code.contains("@TableName"));
assert!(!code.contains("@TableField"));
assert!(!code.contains("@TableId"));
```

该步骤证明未传 `mybatisPlus` 时旧输出保持不变。

- [ ] **Step 2: Add a failing test for table, field, and auto-increment id annotations**

在 `sql_to_entity_java_basic` 后新增：

```rust
#[test]
fn sql_to_entity_java_mybatis_plus_annotations() {
    let sql = r#"
        CREATE TABLE t_user (
            id BIGINT NOT NULL AUTO_INCREMENT COMMENT 'primary key',
            user_name VARCHAR(100) NOT NULL COMMENT 'user name',
            email VARCHAR(200),
            PRIMARY KEY (id)
        );
    "#;
    let r = execute(
        "sql_to_entity",
        &json!({
            "sql": sql,
            "language": "java",
            "options": {
                "comments": true,
                "naming": "camelCase",
                "mybatisPlus": true
            }
        }),
    )
    .unwrap();
    let code = r["code"].as_str().unwrap();

    assert!(code.contains("import com.baomidou.mybatisplus.annotation.IdType;"));
    assert!(code.contains("import com.baomidou.mybatisplus.annotation.TableField;"));
    assert!(code.contains("import com.baomidou.mybatisplus.annotation.TableId;"));
    assert!(code.contains("import com.baomidou.mybatisplus.annotation.TableName;"));
    assert!(code.contains("@TableName(\"t_user\")"));
    assert!(code.contains("@TableId(type = IdType.AUTO)\n    private Long id;"));
    assert!(code.contains("@TableField(\"user_name\")\n    private String userName;"));
    assert!(!code.contains("@TableField(\"email\")"));
}
```

- [ ] **Step 3: Add a failing test for inline primary key with renamed property**

继续新增：

```rust
#[test]
fn sql_to_entity_java_mybatis_plus_inline_primary_key() {
    let sql = "CREATE TABLE account (user_id BIGINT NOT NULL PRIMARY KEY, display_name VARCHAR(100));";
    let r = execute(
        "sql_to_entity",
        &json!({
            "sql": sql,
            "language": "java",
            "options": {
                "comments": false,
                "naming": "camelCase",
                "mybatisPlus": true
            }
        }),
    )
    .unwrap();
    let code = r["code"].as_str().unwrap();

    assert!(code.contains("@TableId(\"user_id\")\n    private Long userId;"));
    assert!(!code.contains("@TableField(\"user_id\")"));
    assert!(!code.contains("import com.baomidou.mybatisplus.annotation.IdType;"));
}
```

- [ ] **Step 4: Add a failing test for composite primary keys**

继续新增：

```rust
#[test]
fn sql_to_entity_java_mybatis_plus_composite_primary_key() {
    let sql = r#"
        CREATE TABLE order_item (
            order_id BIGINT NOT NULL,
            item_id BIGINT NOT NULL,
            quantity INT,
            PRIMARY KEY (order_id, item_id)
        );
    "#;
    let r = execute(
        "sql_to_entity",
        &json!({
            "sql": sql,
            "language": "java",
            "options": {
                "comments": false,
                "naming": "camelCase",
                "mybatisPlus": true
            }
        }),
    )
    .unwrap();
    let code = r["code"].as_str().unwrap();

    assert!(!code.contains("@TableId"));
    assert!(!code.contains("import com.baomidou.mybatisplus.annotation.TableId;"));
    assert!(code.contains("@TableField(\"order_id\")\n    private Long orderId;"));
    assert!(code.contains("@TableField(\"item_id\")\n    private Long itemId;"));
}
```

- [ ] **Step 5: Run the focused tests and verify RED**

Run from `apps/desktop/src-tauri`:

```powershell
cargo test sql_to_entity_java_ -- --nocapture
```

Expected: the new tests fail because `mybatisPlus` is ignored and no MyBatis-Plus annotations are generated. The existing default test remains green.

- [ ] **Step 6: Commit the test-only red state only if the team accepts red commits**

默认不提交红灯状态。保留失败测试，直接进入 Task 2；Task 2 绿灯后一起提交。

---

### Task 2: 解析主键/自增元数据并生成 Java 注解

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/convert.rs:402-436`
- Modify: `apps/desktop/src-tauri/src/tools/convert.rs:473-540`
- Modify: `apps/desktop/src-tauri/src/tools/convert.rs:775-827`
- Modify: `apps/desktop/src-tauri/src/tools/convert.rs:992-1021`
- Test: `apps/desktop/src-tauri/src/tools/convert.rs:1512-1733`

**Interfaces:**

- Consumes: `options.mybatisPlus: bool`, defaulting to `false`.
- Produces: `generate_java(table: &SqlTable, naming: &str, comments: bool, mybatis_plus: bool) -> String`.
- Produces: `SqlTable.primary_keys: Vec<String>` and `SqlColumn.auto_increment: bool`.

- [ ] **Step 1: Extend parsed SQL metadata**

将结构体调整为：

```rust
#[derive(Debug, Clone)]
struct SqlColumn {
    name: String,
    sql_type: String,
    nullable: bool,
    auto_increment: bool,
    default_val: Option<String>,
    comment: Option<String>,
}

#[derive(Debug, Clone)]
struct SqlTable {
    name: String,
    columns: Vec<SqlColumn>,
    primary_keys: Vec<String>,
}
```

- [ ] **Step 2: Parse table-level and inline primary keys**

在 `parse_create_tables` 前增加：

```rust
fn parse_primary_keys(body: &str) -> Vec<String> {
    let parts = split_top_level_commas(body);
    let re_table_primary_key =
        regex::Regex::new(r#"(?i)^\s*PRIMARY\s+KEY\s*\(([^)]+)\)"#).unwrap();
    let re_column =
        regex::Regex::new(r#"(?i)^\s*[`"\[]?(\w+)[`"\]]?\s+"#).unwrap();
    let re_inline_primary_key = regex::Regex::new(r"(?i)\bPRIMARY\s+KEY\b").unwrap();
    let mut primary_keys = Vec::new();

    for part in parts {
        let trimmed = part.trim();
        if let Some(cap) = re_table_primary_key.captures(trimmed) {
            if let Some(columns) = cap.get(1) {
                for column in columns.as_str().split(',') {
                    let name = column
                        .trim()
                        .trim_matches('`')
                        .trim_matches('"')
                        .trim_matches('[')
                        .trim_matches(']');
                    if !name.is_empty()
                        && !primary_keys.iter().any(|key| key.as_str() == name)
                    {
                        primary_keys.push(name.to_string());
                    }
                }
            }
            continue;
        }

        if re_inline_primary_key.is_match(trimmed) {
            if let Some(cap) = re_column.captures(trimmed) {
                let name = cap.get(1).unwrap().as_str();
                if !primary_keys.iter().any(|key| key.as_str() == name) {
                    primary_keys.push(name.to_string());
                }
            }
        }
    }

    primary_keys
}
```

在 `parse_create_tables` 中同时构造字段和主键：

```rust
if let Some(body) = find_paren_body(sql, start) {
    let columns = parse_columns(&body);
    let primary_keys = parse_primary_keys(&body);
    tables.push(SqlTable {
        name: table_name,
        columns,
        primary_keys,
    });
}
```

- [ ] **Step 3: Parse AUTO_INCREMENT on columns**

在 `parse_columns` 的正则初始化处增加：

```rust
let re_auto_increment = regex::Regex::new(r"(?i)\bAUTO_INCREMENT\b").unwrap();
```

构造 `SqlColumn` 时加入：

```rust
let auto_increment = re_auto_increment.is_match(trimmed);
columns.push(SqlColumn {
    name: col_name,
    sql_type: col_type,
    nullable,
    auto_increment,
    default_val,
    comment,
});
```

- [ ] **Step 4: Extend the Java generator signature and imports**

将签名改为：

```rust
fn generate_java(
    table: &SqlTable,
    naming: &str,
    comments: bool,
    mybatis_plus: bool,
) -> String {
```

在类型 import 收集完成后、排序前增加：

```rust
let single_primary_key = if table.primary_keys.len() == 1 {
    table.primary_keys.first().map(String::as_str)
} else {
    None
};

if mybatis_plus {
    imports.push("com.baomidou.mybatisplus.annotation.TableName".into());

    if single_primary_key.is_some() {
        imports.push("com.baomidou.mybatisplus.annotation.TableId".into());
    }

    if table.columns.iter().any(|col| {
        let field_name = convert_field_name(&col.name, naming);
        single_primary_key != Some(col.name.as_str()) && field_name != col.name
    }) {
        imports.push("com.baomidou.mybatisplus.annotation.TableField".into());
    }

    if table.columns.iter().any(|col| {
        single_primary_key == Some(col.name.as_str()) && col.auto_increment
    }) {
        imports.push("com.baomidou.mybatisplus.annotation.IdType".into());
    }
}
```

- [ ] **Step 5: Generate class and field annotations**

将类声明前的输出改为：

```rust
out.push_str("@Data\n");
if mybatis_plus {
    out.push_str(&format!("@TableName(\"{}\")\n", table.name));
}
out.push_str(&format!("public class {} {{\n", class_name));
```

在字段循环中，计算 `field_name` 后、字段声明前加入：

```rust
let is_single_primary_key = single_primary_key == Some(col.name.as_str());

if mybatis_plus && is_single_primary_key {
    let renamed = field_name != col.name;
    match (renamed, col.auto_increment) {
        (false, false) => out.push_str("    @TableId\n"),
        (true, false) => {
            out.push_str(&format!("    @TableId(\"{}\")\n", col.name));
        }
        (false, true) => {
            out.push_str("    @TableId(type = IdType.AUTO)\n");
        }
        (true, true) => {
            out.push_str(&format!(
                "    @TableId(value = \"{}\", type = IdType.AUTO)\n",
                col.name
            ));
        }
    }
} else if mybatis_plus && field_name != col.name {
    out.push_str(&format!("    @TableField(\"{}\")\n", col.name));
}
```

保留现有字段声明：

```rust
let type_name = get_type_for_lang(&col.sql_type, "java");
out.push_str(&format!("    private {} {};\n", type_name, field_name));
```

- [ ] **Step 6: Read the payload option and pass it only to Java**

在 `sql_to_entity` 中读取：

```rust
let mybatis_plus = options["mybatisPlus"].as_bool().unwrap_or(false);
```

只修改 Java 分支：

```rust
"java" => generate_java(table, naming, comments, mybatis_plus),
```

其他语言分支保持原样。

- [ ] **Step 7: Run the focused tests and verify GREEN**

Run from `apps/desktop/src-tauri`:

```powershell
cargo test sql_to_entity_java_ -- --nocapture
```

Expected: all Java SQL-to-entity tests pass, including default-off, annotations, inline primary key, auto increment, and composite primary key cases.

- [ ] **Step 8: Run all SQL-to-entity regression tests**

Run from `apps/desktop/src-tauri`:

```powershell
cargo test sql_to_entity_ -- --nocapture
```

Expected: Java、TypeScript、Go、Python、Kotlin、C#、多表、错误输入和命名测试全部通过。

- [ ] **Step 9: Commit the backend feature**

```powershell
git add apps/desktop/src-tauri/src/tools/convert.rs
git commit -m "feat(sql-entity): 生成 MyBatis-Plus 实体注解"
```

---

### Task 3: 增加 Java 专属前端选项并传递参数

**Files:**

- Modify: `apps/desktop/src/components/SqlEntityPanel.vue:2-13`
- Modify: `apps/desktop/src/components/SqlEntityPanel.vue:43-56`
- Modify: `apps/desktop/src/components/SqlEntityPanel.vue:81-91`
- Modify: `apps/desktop/src/components/SqlEntityPanel.vue:93-112`
- Modify: `apps/desktop/src/components/SqlEntityPanel.vue:130-136`

**Interfaces:**

- Consumes: existing `language` state and `invokeToolByChannel("tool:convert:sql-to-entity", payload)`.
- Produces: `options.mybatisPlus: boolean`; backend defaults remain compatible when absent.

- [ ] **Step 1: Add the Java-only checkbox to the toolbar**

紧跟“注释”复选框后加入：

```vue
<el-checkbox v-if="language === 'java'" v-model="mybatisPlus">
  MyBatis-Plus 注解
</el-checkbox>
```

不为其他语言显示禁用控件，避免产生“该选项是否影响其他语言”的歧义。

- [ ] **Step 2: Persist the option in the module-level panel state**

在 `sqlEntityState` 中加入：

```ts
mybatisPlus: false,
```

在 setup 状态中加入：

```ts
const mybatisPlus = ref(sqlEntityState.mybatisPlus);
```

在 `onBeforeUnmount` 中加入：

```ts
sqlEntityState.mybatisPlus = mybatisPlus.value;
```

- [ ] **Step 3: Send the option explicitly with every generation request**

将请求 `options` 改为：

```ts
options: {
  comments: comments.value,
  naming: naming.value,
  mybatisPlus: language.value === "java" && mybatisPlus.value,
},
```

即使用户先在 Java 开启选项、再切换其他语言，发送值也必须为 `false`；切回 Java 后保留之前的勾选状态。

- [ ] **Step 4: Run TypeScript validation**

Run from repository root:

```powershell
pnpm typecheck
```

Expected: exit code 0, no TypeScript or Vue template errors.

- [ ] **Step 5: Run the renderer build**

Run from repository root:

```powershell
pnpm --filter @lazycat/desktop build:web
```

Expected: exit code 0 and Vite build completes.

- [ ] **Step 6: Commit the frontend integration**

```powershell
git add apps/desktop/src/components/SqlEntityPanel.vue
git commit -m "feat(sql-entity): 添加 MyBatis-Plus 注解选项"
```

---

### Task 4: 完整验证与交付检查

**Files:**

- Verify: `apps/desktop/src-tauri/src/tools/convert.rs`
- Verify: `apps/desktop/src/components/SqlEntityPanel.vue`
- Verify: `docs/superpowers/specs/2026-07-13-sql-entity-mybatis-plus-annotations-design.md`

**Interfaces:**

- Consumes: Tasks 1-3 completed commits.
- Produces: evidence that code matches the approved spec and existing conversion behavior remains intact.

- [ ] **Step 1: Run focused Rust regression tests fresh**

```powershell
Set-Location apps/desktop/src-tauri
cargo test sql_to_entity_ -- --nocapture
Set-Location ../../..
```

Expected: all filtered tests pass with zero failures.

- [ ] **Step 2: Run workspace unit tests**

```powershell
pnpm test
```

Expected: exit code 0 with zero failed tests.

- [ ] **Step 3: Run typecheck and renderer build fresh**

```powershell
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
```

Expected: both commands exit 0.

- [ ] **Step 4: Inspect the final diff**

```powershell
git status --short
git diff HEAD~2 -- apps/desktop/src-tauri/src/tools/convert.rs apps/desktop/src/components/SqlEntityPanel.vue
git diff --check HEAD~2
```

Confirm:

- Only the approved backend generator/test changes and frontend option changes are present.
- No MyBatis-Plus annotation is generated by default.
- No other language generator receives the new option.
- Imports are conditional and deduplicated.
- Composite primary keys do not generate `@TableId`.
- No whitespace errors are reported.

- [ ] **Step 5: Minimal manual smoke check if the user authorizes UI startup**

Do not start `pnpm dev` automatically. If authorized, verify:

1. Java shows “MyBatis-Plus 注解”; other languages hide it.
2. Default Java generation matches the old output.
3. Enabled generation for the sample `t_user` includes `@TableName("t_user")`, auto-increment `@TableId`, and `@TableField("user_name")`.
4. Switching away from Java sends no MyBatis-Plus behavior; switching back restores the checkbox state.

- [ ] **Step 6: Check whether process.md needs an entry**

The expected implementation modifies only two production files, so do not update `process.md`. If execution introduces a third file or reveals a reusable structural lesson, add a concise entry and commit it separately.
