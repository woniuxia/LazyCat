# SQL 转 Java 实体类基类管理设计

## 背景

现有 SQL 转实体类工具支持 Java、TypeScript、Go、Python、Kotlin、C#。Java 生成器需要支持常用基类场景：用户可以管理多个基类模板及其包含的 Java 属性，在转换时选择多个模板，并指定其中一个作为实际父类；所有选中模板中的字段都不再重复生成到子类。

本次功能只作用于 Java，其他语言保持现有行为。

## 目标与边界

- 支持管理多个 Java 基类配置。
- 每个配置包含唯一别名、唯一完整类名和字段列表。
- 转换时可以选择多个基类模板，但只能指定一个实际父类。
- 实际父类生成合法的 `extends` 和 `import`。
- 所有选中基类的字段按生成后的 Java 属性名合并排除。
- 未选择基类时，生成结果与现有行为一致。
- 基类配置持久化到 SQLite，并提供后端 CRUD。

不在本次范围内：

- TypeScript、Go、Python、Kotlin、C# 的继承或组合支持。
- Java 多继承语法生成（Java 不允许 `extends A, B`）。
- 基类源码、方法、注解或继承关系的存储。
- 基类字段的类型、注释或数据库列映射；字段仅作为 Java 属性名排除模板。

## 方案与架构

采用独立 SQLite 表 + Rust CRUD + 前端选择快照传给生成器。

理由：基类配置是可管理的持久化业务数据，使用独立表比 `user_settings` JSON 更容易校验、排序、编辑和删除；生成器接收本次选择的配置快照，不直接查询数据库，使 SQL 解析和 Java 生成逻辑保持可测试、低耦合。

### 数据模型

新增表 `sql_entity_base_classes`：

| 字段 | 类型 | 约束 | 说明 |
| --- | --- | --- | --- |
| `id` | `INTEGER` | 主键自增 | 基类配置 ID |
| `alias` | `TEXT` | 非空、唯一 | 用户可读名称，例如“审计基类” |
| `qualified_name` | `TEXT` | 非空、唯一 | 完整类名，例如 `com.example.BaseEntity` |
| `fields_json` | `TEXT` | 非空 | Java 属性名数组 JSON |
| `sort_order` | `INTEGER` | 非空，默认 0 | 管理列表顺序 |
| `created_at` | `TEXT` | 非空 | 创建时间 |
| `updated_at` | `TEXT` | 非空 | 更新时间 |

通过现有 `helpers.rs` 的 `CREATE TABLE IF NOT EXISTS` 初始化，不改写已有数据，也不需要历史数据迁移。

`fields_json` 保存清洗后的字符串数组：去除首尾空格、忽略空值、去重，并保持用户输入顺序。

### 后端模块与通道

- 新增 `tools/sql_entity.rs`，只负责基类配置 CRUD；`convert.rs` 继续负责 SQL 解析和代码生成。
- 在 `tools/mod.rs` 注册 `sql_entity` 域，并在 dispatch 中路由其 action。
- 在 `bridge/tauri.ts` 增加 `tool:sql-entity:*` 通道。
- CRUD 返回明确的结构化结果；重复别名、重复完整类名、非法 Java 标识符直接返回错误。

推荐通道与 action：

- `tool:sql-entity:base-class-list` → `sql_entity::base_class_list`
- `tool:sql-entity:base-class-create` → `sql_entity::base_class_create`
- `tool:sql-entity:base-class-update` → `sql_entity::base_class_update`
- `tool:sql-entity:base-class-delete` → `sql_entity::base_class_delete`

## 前端交互

Java 模式工具栏增加：

- “参与字段排除”多选框：按基类别名展示，可选择多个配置。
- “实际父类”单选框：选项只来自已选基类。
- “基类管理”按钮：打开管理弹窗。

交互规则：

- 只选择一个基类时，自动将它设为实际父类。
- 删除或取消当前实际父类后，自动选择剩余列表第一项；没有剩余项则清空。
- 没有选择基类时不生成继承语句。
- 非 Java 模式隐藏这些控件，不向转换接口传递基类参数。
- 基类配置持久化在数据库；当前转换选择沿用面板现有会话状态，不额外增加设置项。

管理弹窗使用单个弹窗在列表视图和编辑视图之间切换，避免叠加弹窗：

- 列表展示别名、完整类名、字段数量，并提供新增、编辑、删除。
- 表单包含别名、完整类名、字段列表。
- 字段支持按换行或逗号批量输入，保存前统一清洗。
- 删除前二次确认；删除成功后同步清理当前转换选择。

## Java 生成规则

转换请求增加 Java 专用参数：

```ts
baseClasses: Array<{
  id: number;
  alias: string;
  qualifiedName: string;
  fields: string[];
}>;
parentBaseClassId: number | null;
```

Rust 生成流程：

1. 校验 `parentBaseClassId` 必须属于 `baseClasses`；存在基类选择但没有实际父类时拒绝生成。
2. 将所有选中基类的 `fields` 合并为排除集合。
3. SQL 列按当前命名规则转换成 Java 属性名后，进行区分大小写的精确匹配。
4. 命中的列不生成注释、字段声明、MyBatis-Plus 字段注解或相关类型 import。
5. 实际父类取完整类名的最后一段作为简单类名，生成：

   ```java
   import com.example.BaseEntity;

   public class User extends BaseEntity {
   }
   ```

6. 只有实际父类产生 `import` 和 `extends`；其他选中基类仅参与字段排除。
7. `@TableName` 仍保留在子类上。
8. 若主键字段被排除，不生成对应 `@TableId`、`TableId` 或 `IdType` import。
9. 未选择基类时复用当前生成路径，保证现有输出不变。

完整类名无包路径时不生成 import；有包路径时按完整类名生成 import。类名各段和字段名均必须是合法 Java 标识符。

## 校验与错误处理

### CRUD 保存校验

- 别名不能为空且不能重复。
- 完整类名不能为空且不能重复。
- 完整类名的每一段必须是合法 Java 标识符。
- 字段名必须是合法 Java 属性名。
- 字段空值被移除，重复字段被去重。

### 转换校验

- 实际父类必须属于已选基类。
- 基类配置快照必须包含有效 ID、完整类名和字段数组。
- 错误以现有 IPC 错误机制直接返回，前端显示明确中文提示，不静默降级。

## 测试与验收

### Rust

- 基类 CRUD、排序和唯一性校验。
- 非法完整类名、非法字段名的错误。
- 多个基类字段取并集并从子类排除。
- 只有指定父类生成 `import` 和 `extends`。
- `camelCase` 场景下 `createdAt` 能排除 SQL 列 `created_at`。
- 被排除字段不会留下无用类型、`TableId`、`TableField` 或 `IdType` import。
- 未选择基类时现有输出保持不变。
- 实际父类不在已选列表时拒绝生成。

### 前端

- 换行和逗号字段输入的清洗、去重和校验。
- 单选基类时自动设置实际父类。
- 删除或取消实际父类后的选择修正。
- 删除基类后同步清理当前选择。

### 构建验证

- SQL 实体相关 Rust 测试。
- 对应 Vitest 测试。
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

由于实现会涉及数据库初始化、Rust 工具、bridge、Vue 面板、类型和测试等多个文件，完成后同步将稳定经验记录到 `process.md`。
