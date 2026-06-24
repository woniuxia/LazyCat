# 数据字典工具设计

## 概述

新增独立工具「数据字典」，用于导入一段 JSON array，每个元素必须是 JSON object。用户可以为 object 字段配置显示名和字段含义，并在多个字典之间切换检索。检索输入命中任一参与检索字段后，返回整条 JSON object。

第一版采用 SQLite 持久化，保留原始 JSON object 作为唯一事实源，字段配置和检索文本作为派生数据维护。嵌套对象支持点路径，例如 `user.name`、`department.code`。SQLite FTS5 可用时参与检索，`LIKE` 始终兜底，保证“输入一段内容后做包含匹配”的直觉语义。

## 目标 / 非目标

### 目标

1. 支持创建、重命名、删除多个数据字典。
2. 支持导入或替换 JSON array，要求顶层是数组且数组元素均为 object。
3. 自动推断字段路径，支持嵌套 object 点路径。
4. 支持配置字段显示名、字段含义、是否参与检索、是否在结果表展示。
5. 支持在当前字典内输入关键字检索，匹配各字段值后返回完整 JSON object。
6. 支持查看命中字段和完整格式化 JSON。
7. 离线运行，不新增运行时公网依赖。

### 非目标

1. 第一版不做跨字典全局检索。
2. 第一版不支持数组索引路径或通配路径，例如 `items.0.name`、`items.*.name`。
3. 第一版不把字段含义、字段显示名纳入数据检索，检索只匹配记录字段值。
4. 第一版不做复杂查询语言，不支持 `field:value`、布尔表达式、范围查询。
5. 第一版不引入 Tantivy 等额外全文检索引擎。

## 用户流程

1. 用户进入「数据字典」工具。
2. 左侧选择已有字典，或点击新建字典。
3. 新建时粘贴 JSON array，点击预览。
4. 预览解析字段路径、样例值、记录数；用户确认后保存。
5. 字典详情页显示字段配置表，用户填写字段含义、调整是否检索和是否展示。
6. 用户在搜索框输入关键字，结果区展示匹配记录摘要。
7. 点击结果后，右侧展示完整 JSON object 和命中字段。

## 前端接入

### 工具入口

修改：

- `apps/desktop/src/composables/toolCatalog.ts`：在「数据转换」组加入 `{ id: "data-dictionary", name: "数据字典", desc: "JSON 数组字段释义与数据检索" }`。
- `apps/desktop/src/tool-registry.ts`：注册 `DataDictionaryPanel.vue`。
- `apps/desktop/src/bridge/tauri.ts`：新增 `tool:data-dictionary:*` channel。

新增：

- `apps/desktop/src/components/DataDictionaryPanel.vue`
- `apps/desktop/src/types/data-dictionary.ts`
- `apps/desktop/src/utils/dataDictionary.ts`
- `apps/desktop/src/utils/dataDictionary.test.ts`

### 页面结构

页面采用三栏：

1. 左栏：字典列表、新建、重命名、删除、记录数。
2. 中栏：搜索框、展示字段表格、结果列表。
3. 右栏：字段配置抽屉入口、完整 JSON viewer、命中字段列表。

导入/替换使用弹窗：

1. 输入字典名称和 JSON array。
2. 点击「预览」后展示记录数、字段数、字段路径、样例值。
3. 点击「保存」才写入数据库。

## 后端接入

新增：

- `apps/desktop/src-tauri/src/tools/data_dictionary.rs`

修改：

- `apps/desktop/src-tauri/src/tools/mod.rs`：注册 `data_dictionary` domain。
- `apps/desktop/src-tauri/src/tools/helpers.rs`：新增 schema migration 和可选 FTS5 表。

### IPC action

通道映射：

| Channel | Action | 说明 |
|---|---|---|
| `tool:data-dictionary:list` | `list` | 字典列表 |
| `tool:data-dictionary:get` | `get` | 字典详情和字段配置 |
| `tool:data-dictionary:import-preview` | `import_preview` | 解析 JSON，返回字段推断和样例 |
| `tool:data-dictionary:create` | `create` | 创建字典并导入记录 |
| `tool:data-dictionary:rename` | `rename` | 重命名字典 |
| `tool:data-dictionary:replace-records` | `replace_records` | 替换某字典的全部记录 |
| `tool:data-dictionary:update-fields` | `update_fields` | 保存字段配置 |
| `tool:data-dictionary:search` | `search` | 当前字典内检索 |
| `tool:data-dictionary:delete` | `delete` | 删除字典 |

## 数据模型

### `data_dictionaries`

```sql
CREATE TABLE IF NOT EXISTS data_dictionaries (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  record_count INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
```

### `data_dictionary_fields`

```sql
CREATE TABLE IF NOT EXISTS data_dictionary_fields (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  dictionary_id INTEGER NOT NULL,
  field_path TEXT NOT NULL,
  display_name TEXT NOT NULL DEFAULT '',
  meaning TEXT NOT NULL DEFAULT '',
  searchable INTEGER NOT NULL DEFAULT 1,
  visible INTEGER NOT NULL DEFAULT 1,
  sort_order INTEGER NOT NULL DEFAULT 0,
  type_hint TEXT NOT NULL DEFAULT 'unknown',
  sample_value TEXT NOT NULL DEFAULT '',
  present_count INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(dictionary_id, field_path),
  FOREIGN KEY(dictionary_id) REFERENCES data_dictionaries(id) ON DELETE CASCADE
);
```

### `data_dictionary_records`

```sql
CREATE TABLE IF NOT EXISTS data_dictionary_records (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  dictionary_id INTEGER NOT NULL,
  row_index INTEGER NOT NULL,
  raw_json TEXT NOT NULL,
  search_text TEXT NOT NULL,
  normalized_search_text TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY(dictionary_id) REFERENCES data_dictionaries(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_data_dictionary_records_dictionary
  ON data_dictionary_records(dictionary_id, row_index);
```

### `data_dictionary_fts`

FTS5 可用时创建：

```sql
CREATE VIRTUAL TABLE IF NOT EXISTS data_dictionary_fts USING fts5(
  record_id UNINDEXED,
  dictionary_id UNINDEXED,
  search_text,
  tokenize = 'unicode61 remove_diacritics 2'
);
```

创建 FTS5 失败时，不阻断数据库初始化；搜索自动只走 `LIKE`。这沿用现有 snippets / inbox 的降级策略。

## 字段路径与字段推断

字段推断由 Rust 后端完成，避免前后端口径分叉。

规则：

1. 顶层 array 的每个元素必须是 object，否则导入失败。
2. 递归展开 object 字段。
3. primitive 值作为叶子字段：string、number、boolean、null。
4. array 作为叶子字段，值序列化为紧凑 JSON 字符串。
5. object 只作为中间节点；空 object 作为叶子字段，值为 `{}`。
6. 普通嵌套路径使用点分隔，例如 `user.name`。
7. 原始 key 中若包含 `.` 或 `\`，路径段内转义为 `\.` 和 `\\`，避免与层级分隔冲突。

示例：

```json
[
  {
    "id": 1,
    "user": { "name": "张三", "role": "admin" },
    "tags": ["A", "B"]
  }
]
```

推断字段：

```text
id
user.name
user.role
tags
```

## 检索设计

### 检索文本

每条记录导入时，根据当前字段配置生成：

- `search_text`：参与检索字段的原始可读文本。
- `normalized_search_text`：用于包含匹配的归一化文本。

归一化规则第一版保持简单：

1. trim。
2. Unicode lowercase。
3. 多空白折叠为单个空格。

字段配置更新后，需要重建当前字典全部记录的 `search_text`、`normalized_search_text` 和 FTS 行。

### 搜索语义

搜索 action 输入：

```ts
interface DataDictionarySearchRequest {
  dictionaryId: number;
  keyword: string;
  limit?: number;
}
```

语义：

1. 空 keyword 返回当前字典前 `limit` 条记录，按 `row_index ASC`。
2. 非空 keyword 在当前字典内搜索。
3. `normalized_search_text LIKE %normalizedKeyword% ESCAPE '\'` 始终执行，保证包含匹配。
4. FTS5 表存在且 keyword 能构造安全 MATCH query 时，追加 FTS 候选。
5. 最终结果去重，优先展示 LIKE 命中，再展示 FTS 命中，默认 limit 100。
6. 对返回候选逐条解析 `raw_json`，按字段配置重新计算命中字段，返回 `matches`。

### 为什么不用 Tantivy

Tantivy 是 Rust 生态成熟的嵌入式全文检索引擎，但第一版不引入，原因：

1. 会新增索引目录、同步、重建、备份和清理逻辑。
2. 打包体积和测试面增加。
3. 当前需求是字段值包含匹配，不需要复杂排名、分词器、查询语法或跨字典大规模搜索。
4. SQLite 已在本项目中承担持久化和 FTS5 能力，维护成本更低。

后续如果出现十万级以上记录、跨字典检索、复杂排序或高亮排名，再评估 Tantivy。

## 字段配置与导入替换

### 新建字典

1. `import_preview` 解析 JSON 并返回字段推断。
2. `create` 在事务内写入字典、字段、记录、FTS。
3. 字段默认：
   - `display_name` = 路径最后一段。
   - `meaning` = 空。
   - `searchable` = true。
   - `visible` = 前 6 个字段 true，其余 false，避免结果表过宽。

### 替换记录

`replace_records` 以新 JSON 的字段 union 为准：

1. 相同 `field_path` 继承原显示名、含义、检索、展示配置。
2. 新增字段按默认值创建。
3. 新数据不存在的旧字段保留配置，但 `present_count = 0`，默认不再展示在结果表。
4. 删除旧记录和旧 FTS 行，写入新记录和新 FTS 行。

保留旧字段配置是为了避免用户因为一次临时数据缺字段丢失已维护的字段含义。

## 错误处理

1. JSON 解析失败：返回明确错误位置和 serde_json 错误信息。
2. 顶层不是数组：返回“请输入 JSON array”。
3. 数组为空：允许预览，但保存时提示确认；第一版可保存空字典。
4. 数组元素不是 object：返回第一个非法行号。
5. FTS5 创建或写入失败：记录降级状态，主流程继续，搜索走 `LIKE`。
6. 字段配置为空路径或重复路径：后端拒绝。
7. 删除字典：前端二次确认，后端事务删除。

## 测试计划

### Rust 单测

新增 `data_dictionary.rs` 内部单测：

1. `parse_import_payload_rejects_invalid_json`
2. `parse_import_payload_requires_array`
3. `parse_import_payload_requires_object_items`
4. `flatten_object_supports_nested_dot_path`
5. `flatten_object_escapes_dot_in_key`
6. `flatten_object_treats_array_as_leaf`
7. `build_search_text_uses_only_searchable_fields`
8. `normalize_search_text_lowercases_and_collapses_spaces`
9. `search_like_pattern_escapes_percent_underscore_and_backslash`
10. `compute_matches_returns_full_field_paths`

必要时增加 DB 级测试：

1. create 后 list/get/search 可返回记录。
2. update_fields 后搜索文本重建。
3. replace_records 继承旧字段配置。
4. delete 级联删除字段和记录。

### 前端单测

新增 `apps/desktop/src/utils/dataDictionary.test.ts`：

1. 结果摘要按可见字段生成。
2. 字段路径排序稳定。
3. 命中字段展示使用 `display_name || field_path`。
4. JSON viewer 输入保持原始 object，不消费搜索摘要。

### 验证命令

1. `cargo test data_dictionary`
2. `pnpm test src/utils/dataDictionary.test.ts`
3. `pnpm typecheck`
4. `pnpm --filter @lazycat/desktop build:web`

## 影响面

| 文件 | 类型 | 说明 |
|---|---|---|
| `apps/desktop/src/composables/toolCatalog.ts` | 修改 | 新增工具入口 |
| `apps/desktop/src/tool-registry.ts` | 修改 | 注册面板组件 |
| `apps/desktop/src/bridge/tauri.ts` | 修改 | 新增 channel |
| `apps/desktop/src/components/DataDictionaryPanel.vue` | 新增 | 工具 UI |
| `apps/desktop/src/types/data-dictionary.ts` | 新增 | 类型定义 |
| `apps/desktop/src/utils/dataDictionary.ts` | 新增 | 前端纯函数 |
| `apps/desktop/src/utils/dataDictionary.test.ts` | 新增 | 前端单测 |
| `apps/desktop/src-tauri/src/tools/data_dictionary.rs` | 新增 | 后端逻辑 |
| `apps/desktop/src-tauri/src/tools/mod.rs` | 修改 | 注册 domain |
| `apps/desktop/src-tauri/src/tools/helpers.rs` | 修改 | schema 和 FTS |

## 风险与回滚

风险等级：低到中。

主要风险在搜索语义和大数据性能。第一版用 `LIKE` 保证正确语义，用 FTS5 增强常规 token 查询；如果数据量非常大，`LIKE` 可能变慢，但个人离线工具的常见字典规模可接受。后续可基于同一 `search_text` 模型替换或补充 Tantivy，不影响原始数据。

回滚方式：删除入口和后端 domain 后，新增表留在 SQLite 中无副作用；再次启用时可继续读取。
