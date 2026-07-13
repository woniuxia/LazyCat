# SQL 转 Java 实体 MyBatis-Plus 注解设计

## 背景

现有“SQL 转实体类”工具可以把 `CREATE TABLE` 转为 Java、TypeScript、Go、Python、Kotlin 和 C# 代码。Java 输出固定生成 Lombok `@Data`，但不会保留数据库表名、列名和主键信息，用户仍需手工补充 MyBatis-Plus 映射注解。

本次仅增强 Java 输出，并保持默认关闭时的现有输出不变。

## 目标

- Java 模式提供“生成 MyBatis-Plus 注解”选项，默认关闭。
- 开启后生成 `@TableName`、必要的 `@TableField` 和单主键 `@TableId`。
- 识别列级或表级主键声明，以及 `AUTO_INCREMENT` 自增属性。
- 不改变其他语言的生成结果，不引入新依赖。

## 前端交互

- 在 `SqlEntityPanel.vue` 工具栏中，仅当语言为 Java 时显示“生成 MyBatis-Plus 注解”复选框。
- 复选框状态与现有语言、命名方式、注释选项一样，在组件卸载后保存在模块级状态中。
- 生成请求在 `options` 中增加布尔值 `mybatisPlus`。未传递或为 `false` 时，后端按旧逻辑生成。

## SQL 解析模型

扩展现有结构化解析结果，而不是在生成后的 Java 文本上做替换：

- `SqlColumn` 增加是否自增的信息。
- `SqlTable` 保存主键列名集合。
- 同时识别 `id BIGINT PRIMARY KEY AUTO_INCREMENT` 形式的列级主键，以及 `PRIMARY KEY (id)` 形式的表级主键。
- 主键匹配使用原始数据库列名，不依赖 Java 命名策略。

## Java 注解生成规则

启用 MyBatis-Plus 注解时：

1. 增加 `com.baomidou.mybatisplus.annotation.TableName` 导入，并在类上生成 `@TableName("原始表名")`。
2. 数据表恰好只有一个主键时，该字段生成 `@TableId`：
   - 属性名与列名一致且非自增：`@TableId`。
   - 属性名与列名不一致且非自增：`@TableId("原始列名")`。
   - 自增且名称一致：`@TableId(type = IdType.AUTO)`。
   - 自增且名称不一致：`@TableId(value = "原始列名", type = IdType.AUTO)`。
   - 仅在存在自增主键时导入 `IdType`。
3. 非主键字段仅在 Java 属性名与数据库列名不一致时生成 `@TableField("原始列名")`。
4. 主键字段不重复生成 `@TableField`。
5. 复合主键不生成多个无效的 `@TableId`；其字段按普通字段映射规则处理。
6. 注解位于字段 Javadoc 之后、字段声明之前。

关闭选项时，不增加任何 MyBatis-Plus 导入或注解，保持当前 Lombok `@Data` 输出格式。

## 数据流与兼容性

前端将 `mybatisPlus` 随现有 `comments`、`naming` 一并传入 `tool:convert:sql-to-entity`。Rust 端使用 `false` 作为缺省值，只把该选项传给 Java 生成器；其他语言生成器不接收也不处理该选项。

该变更不修改 IPC 通道、不修改持久化结构、不修改数据库，也不依赖运行时公网资源。

## 测试与验证

按 TDD 增加 Rust 单元测试，至少覆盖：

- 默认关闭时不生成 MyBatis-Plus 注解。
- 开启后生成 `@TableName`。
- 蛇形列名转驼峰属性时生成 `@TableField`，名称一致时不生成。
- 表级单主键生成 `@TableId`。
- 列级自增主键生成带 `IdType.AUTO` 的 `@TableId`。
- 主键名称发生转换时由 `@TableId` 携带原始列名，且不重复生成 `@TableField`。
- 复合主键不生成 `@TableId`。

完成后执行相关 Rust 测试、`pnpm typecheck` 和 `pnpm --filter @lazycat/desktop build:web`，并检查 Java/非 Java 模式下复选框显示条件及请求参数。

## 改动边界

- 修改 `apps/desktop/src/components/SqlEntityPanel.vue`。
- 修改 `apps/desktop/src-tauri/src/tools/convert.rs` 及其同文件单元测试。
- 不重构通用转换架构，不增加 MyBatis XML、Swagger、JPA 或其他注解选项。
