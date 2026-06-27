# 数据字典关系查询设计

## 概述

将现有「数据字典」从独立 JSON 字典检索升级为通用关系字典。用户仍然导入 JSON array，原始记录继续存放在 `data_dictionary_records.raw_json`，字段配置继续由 `data_dictionary_fields` 管理；新增字典主键配置、字典间关系配置和字段值派生索引，用 SQLite 支持人员、部门、岗位等资料之间的正向关联与反向查询。

本设计继续使用 SQLite，不引入 Tantivy 或其他全文检索引擎。当前需求的主要瓶颈不是全文检索能力，而是缺少结构化字段值索引和关系配置。SQLite 的普通索引足以支持主键查找、字段等值匹配、正向关联和反向关联；现有 `LIKE` 与可选 FTS5 继续服务包含匹配。

## 目标

1. 支持为每个字典配置主键字段，例如人员的 `employeeNo`、部门的 `id`、岗位的 `code`。
2. 支持在字段配置抽屉中维护当前字典的关系配置。
3. 关系统一表示为 `源字典.源字段 -> 目标字典.primary_field_path`，目标字段不单独配置。
4. 点击搜索结果后返回完整记录详情、正向关联记录和反向关联记录。
5. 关系结果全部返回，前端用滚动容器承载，不做后端截断。
6. 新增字段值派生索引 `data_dictionary_record_values`，用于主键查找、正向关联和反向关联。
7. 新增单字典右键菜单「重建索引」，从 `raw_json` 重建该字典字段值索引、搜索文本和 FTS。

## 非目标

1. 不新增 `entity_label`，继续使用 `data_dictionaries.name` 作为字典/实体显示名。
2. 不新增 `summary_visible`，继续使用现有 `visible` 控制列表摘要展示字段。
3. 不新增 `quick_copy`，一键复制字段后置。
4. 不支持目标任意字段关系；目标固定为目标字典主键字段。
5. 不支持多跳图查询、布尔关系表达式或 SQL 查询语言。
6. 不新增全局重建索引入口，只支持单个字典重建。

## 现有基础

当前数据字典已经具备：

- `data_dictionaries`：字典元数据、标题字段、排序字段、左侧导航顺序。
- `data_dictionary_fields`：字段显示名、含义、检索开关、展示开关、字段顺序。
- `data_dictionary_records`：原始 JSON、搜索文本和归一化搜索文本。
- `data_dictionary_fts`：可选 FTS5 表，创建失败时不阻断主流程。
- 前端三栏结构：左侧字典列表，中间搜索结果，右侧完整 JSON 详情。

本次设计沿用「原始 JSON 是唯一事实源，其他数据均为派生或配置」的原则。

## 数据模型

### `data_dictionaries`

新增 `primary_field_path`：

```sql
ALTER TABLE data_dictionaries
ADD COLUMN primary_field_path TEXT DEFAULT NULL;
```

字段语义：

- `name`：字典/实体显示名，例如人员、部门、岗位。
- `primary_field_path`：该字典主键字段路径，用于被其他字典引用和反向查询。
- `title_field_path`：已有，记录标题字段。
- `sort_field_path` / `sort_direction`：已有，当前字典记录排序配置。
- `nav_order`：已有，左侧字典排序。

### `data_dictionary_fields`

本轮不扩字段。继续承担字段本身配置：

```text
field_path
display_name
meaning
searchable
visible
sort_order
type_hint
sample_value
present_count
```

### `data_dictionary_relations`

新增关系配置表：

```sql
CREATE TABLE IF NOT EXISTS data_dictionary_relations (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  source_dictionary_id INTEGER NOT NULL,
  source_field_path TEXT NOT NULL,
  target_dictionary_id INTEGER NOT NULL,
  relation_name TEXT NOT NULL,
  reverse_name TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(source_dictionary_id, source_field_path, target_dictionary_id),
  FOREIGN KEY(source_dictionary_id) REFERENCES data_dictionaries(id) ON DELETE CASCADE,
  FOREIGN KEY(target_dictionary_id) REFERENCES data_dictionaries(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_data_dictionary_relations_source
  ON data_dictionary_relations(source_dictionary_id);

CREATE INDEX IF NOT EXISTS idx_data_dictionary_relations_target
  ON data_dictionary_relations(target_dictionary_id);
```

关系含义固定为：

```text
source_dictionary.source_field_path -> target_dictionary.primary_field_path
```

示例：

```text
人员.departmentId -> 部门.primary_field_path
人员.positionId   -> 岗位.primary_field_path
```

### `data_dictionary_record_values`

新增字段值派生索引：

```sql
CREATE TABLE IF NOT EXISTS data_dictionary_record_values (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  record_id INTEGER NOT NULL,
  dictionary_id INTEGER NOT NULL,
  field_path TEXT NOT NULL,
  value_text TEXT NOT NULL,
  normalized_value TEXT NOT NULL,
  UNIQUE(record_id, field_path),
  FOREIGN KEY(record_id) REFERENCES data_dictionary_records(id) ON DELETE CASCADE,
  FOREIGN KEY(dictionary_id) REFERENCES data_dictionaries(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_data_dictionary_record_values_lookup
  ON data_dictionary_record_values(dictionary_id, field_path, normalized_value);

CREATE INDEX IF NOT EXISTS idx_data_dictionary_record_values_record
  ON data_dictionary_record_values(record_id);
```

该表只由 `raw_json` 派生：

- 导入新字典时写入。
- 替换字典记录时重建。
- 单字典「重建索引」时重建。
- 字段配置变化一般不需要重建，除非未来字段展开规则变化。

## IPC 设计

### `get`

现有 `tool:data-dictionary:get` 扩展返回：

```ts
interface DataDictionaryGetResult {
  dictionary: DataDictionarySummary;
  fields: DataDictionaryField[];
  relations: DataDictionaryRelation[];
}
```

`DataDictionarySummary` 增加：

```ts
primaryFieldPath: string | null;
```

关系类型：

```ts
interface DataDictionaryRelation {
  id?: number;
  sourceFieldPath: string;
  targetDictionaryId: number;
  relationName: string;
  reverseName: string;
}
```

### `update-fields`

现有 `tool:data-dictionary:update-fields` 扩展为一次保存字典配置、字段配置和关系配置：

```ts
interface UpdateDataDictionaryFieldsRequest {
  dictionaryId: number;
  primaryFieldPath: string | null;
  titleFieldPath: string | null;
  sortFieldPath: string | null;
  sortDirection: "asc" | "desc";
  fields: DataDictionaryField[];
  relations: DataDictionaryRelation[];
}
```

保存策略：

1. 字典级配置写入 `data_dictionaries`。
2. 字段配置按现有方式更新 `data_dictionary_fields`。
3. 当前源字典的关系配置采用整组替换：删除旧关系，再插入请求中的关系。
4. 全部操作在同一事务中完成。
5. 保存后继续重建 `search_text`、`normalized_search_text` 和可选 FTS，保持现有搜索语义。

校验规则：

- `primaryFieldPath` 非空时必须存在于当前字典字段集合。
- `titleFieldPath`、`sortFieldPath` 保持现有字段存在校验。
- `relations[].sourceFieldPath` 必须存在于当前字典字段集合。
- `relations[].targetDictionaryId` 必须存在。
- 目标字典必须已配置 `primary_field_path`。
- `relationName`、`reverseName` 不能为空。
- 同一 `sourceFieldPath + targetDictionaryId` 不允许重复。
- 不允许关系指向自身的同一字段造成无意义自引用；如果源字典等于目标字典，源字段必须不同于目标主键字段。

### `record-detail`

新增：

```text
tool:data-dictionary:record-detail
```

请求：

```ts
interface DataDictionaryRecordDetailRequest {
  recordId: number;
}
```

返回：

```ts
interface DataDictionaryRecordBrief {
  id: number;
  dictionaryId: number;
  dictionaryName: string;
  title: string;
  rowIndex: number;
  summary: Array<{ fieldPath: string; label: string; value: string }>;
  rawJson: unknown;
}

interface DataDictionaryRelationGroup {
  relationId: number;
  name: string;
  direction: "forward" | "reverse";
  sourceDictionaryId: number;
  targetDictionaryId: number;
  items: DataDictionaryRecordBrief[];
}

interface DataDictionaryRecordDetail {
  record: DataDictionaryRecordBrief;
  fields: DataDictionaryField[];
  forwardRelations: DataDictionaryRelationGroup[];
  reverseRelations: DataDictionaryRelationGroup[];
}
```

查询流程：

1. 根据 `recordId` 加载当前记录和所属字典。
2. 根据当前字典字段配置生成当前记录标题和摘要。
3. 正向关联：读取当前记录在 `source_field_path` 上的值，到目标字典 `primary_field_path` 中查找同值记录。
4. 反向关联：读取当前记录在当前字典 `primary_field_path` 上的值，到所有指向当前字典的关系源字段中查找同值记录。
5. 关联结果全部返回，不做后端数量限制。

如果某条关系配置已经失效，例如目标字典缺少主键字段，`record-detail` 应返回错误，促使用户修复配置，而不是静默隐藏关系。

### `rebuild-indexes`

新增：

```text
tool:data-dictionary:rebuild-indexes
```

请求：

```ts
interface RebuildDataDictionaryIndexesRequest {
  dictionaryId: number;
}
```

返回：

```ts
interface RebuildDataDictionaryIndexesResult {
  recordCount: number;
  valueCount: number;
}
```

执行内容：

1. 删除该字典旧的 `data_dictionary_record_values`。
2. 从该字典每条 `data_dictionary_records.raw_json` 展开字段值并写入 `record_values`。
3. 按当前字段 `searchable` 配置重建 `search_text` 和 `normalized_search_text`。
4. 如果 `data_dictionary_fts` 存在，重建该字典 FTS。

错误策略：

- 如果某条 `raw_json` 解析失败，直接中断并返回明确记录信息。
- FTS5 不存在或写入失败不阻断主流程，沿用现有降级策略。
- 重建不修改原始记录、字段配置或关系配置。

## 前端设计

### 左侧字典右键菜单

菜单调整为：

```text
替换
字段
重建索引
重命名
删除
```

不新增「关系」入口。关系配置并入字段配置抽屉。

「重建索引」点击后确认：

```text
将使用「人员」的原始 JSON 重建字段值索引和搜索索引。
不会修改原始记录、字段配置和关系配置。
```

成功后提示：

```text
已重建索引：1200 条记录，9600 个字段值
```

### 字段配置抽屉

字段配置抽屉顺序调整为：

1. 字典配置
   - 主键字段
   - 标题字段
   - 排序字段
   - 排序方向

2. 关系配置
   - 源字段
   - 关系名
   - 目标字典
   - 反向关系名

3. 字段列表
   - 展示字段
   - 非展示字段

关系配置在字段列表前面，原因是主键字段和关系会影响用户理解字段含义，维护字段文案前应先看清字典关联方式。

目标字典选择后不显示目标字段选择框，但应展示目标字典当前主键字段，便于用户确认关系指向。例如：

```text
目标字典：部门
目标主键：id
```

如果目标字典未配置主键字段，关系行显示错误状态并禁止保存。

### 详情面板

详情面板从直接展示搜索项，改为点击结果后加载 `record-detail`。布局建议：

1. 顶部：标题、来源字典、行号。
2. 摘要字段：继续使用 `visible` 字段。
3. 关联信息：
   - 正向关系组，例如所属部门、担任岗位。
   - 反向关系组，例如部门人员、岗位人员。
   - 每组全部展示，组内使用滚动容器或自然列表，不在后端截断。
4. 完整 JSON：保留现有 `pre` JSON viewer。

关联记录卡片使用目标字典的标题字段和 `visible` 字段摘要，避免展示未配置的原始 JSON 噪音。

## 后端实现要点

1. 抽出可复用的字段值索引构建函数，输入 `record_id + dictionary_id + raw_json`，输出多条 `record_values`。
2. 字段值索引使用现有字段展开规则，保持点路径转义语义一致。
3. `normalized_value` 复用 `normalize_search_text`，保证等值匹配对大小写和空白一致。
4. 导入和替换记录时，在同一事务内写入 `data_dictionary_records` 和 `data_dictionary_record_values`。
5. `record-detail` 的正向和反向查询都走 `data_dictionary_record_values` 的 `(dictionary_id, field_path, normalized_value)` 索引。
6. 关联查询匹配值来自索引表，避免每次详情加载解析全部目标记录。
7. `update-fields` 保存关系时只校验配置，不重建 `record_values`。
8. `rebuild-indexes` 是索引修复入口，使用 `raw_json` 作为唯一来源。

## 错误处理

1. 主键字段不存在：保存字段配置时拒绝。
2. 目标字典没有主键字段：保存关系时拒绝。
3. 源字段不存在：保存关系时拒绝。
4. 重复关系：保存关系时拒绝。
5. 关系配置失效：详情加载时报错，提示用户重新进入字段配置修复关系。
6. 重建索引遇到损坏 `raw_json`：中断并返回记录 id 或行号。
7. 关系查询没有匹配记录：返回空列表，不视为错误。

## 测试计划

### Rust 单测

新增或扩展 `data_dictionary` 测试：

1. `record_values_rebuilds_from_raw_json`
2. `record_values_supports_escaped_nested_paths`
3. `update_fields_rejects_missing_primary_field`
4. `update_fields_rejects_relation_without_target_primary`
5. `update_fields_rejects_duplicate_relations`
6. `record_detail_returns_forward_relations`
7. `record_detail_returns_reverse_relations_without_limit`
8. `rebuild_indexes_refreshes_record_values_and_search_text`
9. `rebuild_indexes_fails_on_corrupt_raw_json`

### 前端单测

新增或扩展纯函数与源码约束测试：

1. 字段配置抽屉包含主键字段配置。
2. 关系配置位于字段列表前。
3. 关系配置只选择目标字典，不渲染目标字段选择。
4. 字典右键菜单包含「重建索引」，不包含「关系」。
5. 详情关联分组展示全部 items，不依赖 `hasMore`。

### 验证命令

1. `cargo test data_dictionary -- --nocapture`
2. `pnpm test src/utils/dataDictionary.test.ts src/components/DataDictionaryPanel.context-menu.test.ts`
3. `pnpm typecheck`
4. `pnpm --filter @lazycat/desktop build:web`

## 影响面

| 文件 | 类型 | 说明 |
|---|---|---|
| `apps/desktop/src-tauri/src/tools/helpers.rs` | 修改 | schema migration：主键字段、关系表、字段值索引表 |
| `apps/desktop/src-tauri/src/tools/data_dictionary.rs` | 修改 | 关系保存、详情查询、索引重建 |
| `apps/desktop/src/bridge/tauri.ts` | 修改 | 新增 `record-detail`、`rebuild-indexes` channel |
| `apps/desktop/src/types/data-dictionary.ts` | 修改 | 新增关系、详情、重建索引类型 |
| `apps/desktop/src/utils/dataDictionary.ts` | 修改 | 详情摘要和关联展示 helper |
| `apps/desktop/src/components/DataDictionaryPanel.vue` | 修改 | 字段抽屉、关系配置、详情面板、重建索引菜单 |
| `apps/desktop/src/components/DataDictionaryPanel.context-menu.test.ts` | 修改 | 菜单和抽屉结构回归 |
| `apps/desktop/src/utils/dataDictionary.test.ts` | 修改 | 详情与关系 helper 测试 |

## 风险与回滚

风险等级：中。

主要风险：

1. 关系配置使字段配置抽屉复杂度上升。
2. 详情页全量返回反向关系，如果某个关系命中几千条记录，前端渲染需要滚动容器承载。
3. 字段值索引引入派生数据一致性问题，需要导入、替换和手动重建路径都覆盖。

控制方式：

1. 保留 `raw_json` 作为唯一事实源，索引可重建。
2. 关系配置和字段配置同事务保存，避免半成功。
3. 单字典重建索引入口用于修复派生索引。
4. 不引入全文检索新引擎，避免索引目录和同步复杂度扩大。

回滚方式：

1. 前端隐藏关系配置和详情关联展示。
2. 后端保留新增表不使用，对旧数据字典搜索无影响。
3. 如需彻底回滚，移除新增 channel 和 UI 后，新增表可作为无副作用历史数据保留。
