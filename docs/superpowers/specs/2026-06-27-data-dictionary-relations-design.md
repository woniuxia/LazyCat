# 数据字典关系查询设计

## 概述

将现有「数据字典」从独立 JSON 字典检索升级为通用关系字典。用户仍然导入 JSON array，原始记录继续存放在 `data_dictionary_records.raw_json`，字段配置继续由 `data_dictionary_fields` 管理；新增字典主键配置、字典间关系配置和字段值派生索引，用 SQLite 支持人员、部门、岗位等资料之间的正向关联与反向查询。

本轮核心取舍是：关系能力只依赖用户显式配置的字段路径和派生字段值索引，不改变 JSON 原始数据模型。主键字段是真正的业务唯一键，有效主键值必须是标量、非空、非 `null` 且在字典内唯一；缺失、空值、`null`、非标量或重复主键的异常记录不写入导入结果，并由前端提示跳过数量。字段值索引必须记录值类型和索引就绪状态，避免把历史未建索引误判为无关系，也避免 JSON `null` 误匹配字符串 `"null"`。

本设计继续使用 SQLite，不引入 Tantivy 或其他全文检索引擎。当前需求的主要瓶颈不是全文检索能力，而是缺少结构化字段值索引和关系配置。SQLite 的普通索引足以支持主键查找、字段等值匹配、正向关联和反向关联；现有 `LIKE` 与可选 FTS5 继续服务包含匹配。

## 目标

1. 支持为每个字典配置主键字段，例如人员的 `employeeNo`、部门的 `id`、岗位的 `code`。
2. 支持在字段配置抽屉中维护当前字典的关系配置。
3. 关系统一表示为 `源字典.源字段 -> 目标字典.primary_field_path`，目标字段不单独配置。
4. 点击搜索结果后返回完整记录详情、正向关联记录和反向关联记录。
5. 关系结果全部返回，前端用滚动容器承载，不做后端截断。
6. 新增字段值派生索引 `data_dictionary_record_values`，用于主键查找、正向关联和反向关联。
7. 新增单字典右键菜单「重建索引」，从 `raw_json` 重建该字典字段值索引、搜索文本和 FTS。
8. 对历史字典提供明确的单字典索引修复路径；缺少字段值索引时关系详情返回可操作错误，不返回伪空结果。
9. 导入或替换时允许存在主键异常记录；异常记录跳过不入库，接口返回跳过数量供前端提示。

## 非目标

1. 不新增 `entity_label`，继续使用 `data_dictionaries.name` 作为字典/实体显示名。
2. 不新增 `summary_visible`，继续使用现有 `visible` 控制列表摘要展示字段。
3. 不新增 `quick_copy`，一键复制字段后置。
4. 不支持目标任意字段关系；目标固定为目标字典主键字段。
5. 不支持多跳图查询、布尔关系表达式或 SQL 查询语言。
6. 不新增全局重建索引入口，只支持单个字典重建。
7. 不支持数组/多值关系字段；关系源字段和目标主键字段都按单个标量值匹配。

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

新增 `primary_field_path` 和字段值索引就绪标记：

```sql
ALTER TABLE data_dictionaries
ADD COLUMN primary_field_path TEXT DEFAULT NULL;

ALTER TABLE data_dictionaries
ADD COLUMN field_value_indexed_at TEXT DEFAULT NULL;
```

字段语义：

- `name`：字典/实体显示名，例如人员、部门、岗位。
- `primary_field_path`：该字典主键字段路径，用于被其他字典引用和反向查询。
- `field_value_indexed_at`：该字典 `data_dictionary_record_values` 最近一次成功构建时间；`NULL` 表示升级前历史字典或索引缺失。
- `title_field_path`：已有，记录标题字段。
- `sort_field_path` / `sort_direction`：已有，当前字典记录排序配置。
- `nav_order`：已有，左侧字典排序。

落地要求：

- `helpers.rs` 中既要给新表定义增加 `primary_field_path` / `field_value_indexed_at`，也要在前置 `ALTER TABLE` 中补齐历史库字段。
- `list` 和 `get` 都必须返回 `primaryFieldPath`，否则字段配置抽屉无法在目标字典下拉中展示目标主键。
- `field_value_indexed_at` 不需要暴露给前端列表；后端用它判断 `record-detail` 是否可以信任字段值索引。
- 主键字段路径必须存在于当前字典字段集合；具体记录的主键值逐条判断，`array`、`object`、空值和重复值对应记录会被跳过。

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
  value_type TEXT NOT NULL,
  value_text TEXT NOT NULL,
  normalized_value TEXT NOT NULL,
  UNIQUE(record_id, field_path),
  FOREIGN KEY(record_id) REFERENCES data_dictionary_records(id) ON DELETE CASCADE,
  FOREIGN KEY(dictionary_id) REFERENCES data_dictionaries(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_data_dictionary_record_values_lookup
  ON data_dictionary_record_values(dictionary_id, field_path, normalized_value, value_type);

CREATE INDEX IF NOT EXISTS idx_data_dictionary_record_values_record
  ON data_dictionary_record_values(record_id);
```

该表只由 `raw_json` 派生：

- 导入新字典时写入。
- 替换字典记录时重建。
- 单字典「重建索引」时重建。
- 字段展示/检索配置变化不改变字段值索引内容；但 `update-fields` 作为配置保存入口仍会重建当前字典索引，用于修复历史索引、刷新主键有效性和更新索引就绪标记。

值语义：

- `value_text` 使用现有 `value_to_search_text`，保持数组和复杂对象的紧凑 JSON 字符串语义。
- `value_type` 使用现有 `value_type_hint` 语义：`string`、`number`、`boolean`、`null`、`array`、`object`。
- `normalized_value` 复用 `normalize_search_text`，用于等值查询；它不是全文搜索字段。
- 关系匹配只使用 `value_type IN ('string', 'number', 'boolean')` 且 `normalized_value <> ''` 的索引行。
- JSON `null` 会以 `value_type = 'null'` 写入索引，但不参与关系匹配；字符串 `"null"` 会以 `value_type = 'string'` 写入，可以正常匹配字符串 `"null"`。
- 如果某字段在一条记录中不存在，则该记录不会写入对应 `field_path` 的索引行。

历史数据兼容：

- schema migration 只创建列、表、索引和 `field_value_indexed_at` 列，不在 `ensure_schema` 中全量解析历史 `raw_json`，避免启动阶段阻塞。
- 对升级前已存在的字典，用户可以通过单字典「重建索引」补齐 `data_dictionary_record_values`。
- `record-detail` 发现当前字典、目标字典或反向源字典 `field_value_indexed_at IS NULL` 时，必须返回明确错误，例如「字段值索引缺失，请先对“人员”执行重建索引」，不能把关系结果静默返回为空。
- 替换记录和保存主键配置时，如果已配置主键字段，缺失、空值、`null`、非标量或归一化后重复的主键记录会被跳过，不写入最终记录集合。
- 重建索引不删除已有原始记录；如果历史数据存在主键异常，异常记录的主键字段不写入索引，其他字段仍可写入索引，并返回异常统计，提示用户通过替换导入清理原始数据。
- `create`、`replace-records`、`update-fields` 和 `rebuild-indexes` 成功写入字段值索引后，都必须在同一事务内更新 `field_value_indexed_at = CURRENT_TIMESTAMP`。

## IPC 设计

### `list`

现有 `tool:data-dictionary:list` 扩展 `DataDictionarySummary`：

```ts
interface DataDictionarySummary {
  id: number;
  name: string;
  description: string;
  recordCount: number;
  titleFieldPath: string | null;
  primaryFieldPath: string | null;
  sortFieldPath: string | null;
  sortDirection: "asc" | "desc";
  navOrder: number;
  createdAt: string;
  updatedAt: string;
}
```

`primaryFieldPath` 必须在列表接口返回，字段配置抽屉依赖它展示目标字典主键状态。

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
  id: number;
  sourceDictionaryId: number;
  sourceFieldPath: string;
  targetDictionaryId: number;
  targetDictionaryName: string;
  targetPrimaryFieldPath: string | null;
  relationName: string;
  reverseName: string;
}

interface DataDictionaryRelationDraft {
  sourceFieldPath: string;
  targetDictionaryId: number;
  relationName: string;
  reverseName: string;
}
```

`get` 需要返回当前源字典已有关系，即使目标字典后来清空了主键字段。此时 `targetPrimaryFieldPath` 为 `null`，字段配置抽屉显示错误状态并禁止保存，方便用户修复或删除这条失效关系。

### `create` / `replace-records`

导入和替换记录的响应增加主键异常跳过统计：

```ts
interface DataDictionaryImportWriteResult {
  ok: true;
  id?: number;
  recordCount: number;
  skippedPrimaryRecordCount: number;
  skippedPrimaryInvalidCount: number;
  skippedPrimaryDuplicateCount: number;
}
```

字段语义：

- `recordCount`：实际写入 `data_dictionary_records` 的记录数。
- `skippedPrimaryRecordCount`：因为主键异常未写入的记录总数。
- `skippedPrimaryInvalidCount`：主键字段缺失、空字符串、`null` 或非标量的记录数。
- `skippedPrimaryDuplicateCount`：主键归一化后重复而被跳过的记录数；同一主键值保留第一条有效记录，后续重复记录跳过。

新建字典如果暂未配置 `primaryFieldPath`，不做主键跳过，所有合法 JSON object 记录照常导入。未来如果导入流程支持在创建时选择主键字段，则同样使用以上统计。

当接口因为主键异常跳过记录时，写入记录的 `row_index` 保留源 JSON array 中的 0 基下标，不压缩成连续序号；这样排序和详情中的 `#行号` 仍指向原始导入位置。保存主键配置时删除历史异常记录，也保留剩余记录原有 `row_index`。

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
  relations: DataDictionaryRelationDraft[];
}
```

返回：

```ts
interface UpdateDataDictionaryFieldsResult {
  ok: true;
  recordCount: number;
  skippedPrimaryRecordCount: number;
  skippedPrimaryInvalidCount: number;
  skippedPrimaryDuplicateCount: number;
}
```

保存策略：

1. 字典级配置写入 `data_dictionaries`。
2. 字段配置按现有方式更新 `data_dictionary_fields`。
3. 当前源字典的关系配置采用整组替换：删除旧关系，再插入请求中的关系。
4. 如果设置了 `primaryFieldPath`，在同一事务中按主键有效性过滤当前记录集合：有效记录保留，主键异常记录从 `data_dictionary_records` 和派生索引中移除。
5. 在同一事务中重建 `search_text`、`normalized_search_text`、字段值索引，并更新 `field_value_indexed_at`。
6. 全部强一致数据操作在同一事务中完成；字段值索引写入失败必须回滚字段、关系和记录变更。
7. 事务提交后按现有降级策略重建可选 FTS；FTS 失败只影响 FTS 候选，不影响 LIKE 搜索和关系能力。
8. 如果跳过了主键异常记录，返回跳过统计，前端提示用户有多少记录未纳入字典。

校验规则：

- `primaryFieldPath` 非空时必须存在于当前字典字段集合；已有记录中主键缺失、空值、`null`、非标量或重复的记录会被跳过，不再阻断保存。
- `titleFieldPath`、`sortFieldPath` 保持现有字段存在校验。
- `relations[].sourceFieldPath` 必须存在于当前字典字段集合。
- `relations[].sourceFieldPath` 必须是标量字段；校验不能只依赖 `type_hint`，需要扫描现有 `raw_json` 中该字段的已出现值：缺失、`null` 和空字符串允许，任一 `array` / `object` 值都应拒绝。
- `relations[].targetDictionaryId` 必须存在。
- 目标字典必须已配置 `primary_field_path`。
- `relationName`、`reverseName` 不能为空。
- 同一 `sourceFieldPath + targetDictionaryId` 不允许重复。
- 不允许关系指向自身的同一字段造成无意义自引用；如果源字典等于目标字典，源字段必须不同于目标主键字段。
- 关系保存只接受当前源字典的关系；请求中的 `sourceDictionaryId` 如果出现应忽略或拒绝，不能让前端伪造其他字典关系。

### `record-detail`

新增：

```text
tool:data-dictionary:record-detail
```

`bridge/tauri.ts` 映射为：

```ts
"tool:data-dictionary:record-detail": { domain: "data_dictionary", action: "record_detail" }
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
}

interface DataDictionaryRecordFull extends DataDictionaryRecordBrief {
  rawJson: unknown;
}

interface DataDictionaryRelationGroup {
  relationId: number;
  name: string;
  direction: "forward" | "reverse";
  sourceDictionaryId: number;
  targetDictionaryId: number;
  itemCount: number;
  items: DataDictionaryRecordBrief[];
}

interface DataDictionaryRecordDetail {
  record: DataDictionaryRecordFull;
  fields: DataDictionaryField[];
  forwardRelations: DataDictionaryRelationGroup[];
  reverseRelations: DataDictionaryRelationGroup[];
}
```

查询流程：

1. 根据 `recordId` 加载当前记录和所属字典。
2. 检查当前字典、正向目标字典和反向源字典的 `field_value_indexed_at` 均非空。
3. 根据当前字典字段配置生成当前记录标题和摘要。
4. 正向关联：从当前记录的 `data_dictionary_record_values` 读取 `source_field_path` 的有效标量值，到目标字典 `primary_field_path` 的有效标量索引中查找同值记录。
5. 反向关联：从当前记录的 `data_dictionary_record_values` 读取当前字典 `primary_field_path` 的有效标量值，到所有指向当前字典的关系源字段有效标量索引中查找同值记录。
6. 关联结果全部返回，不做后端数量限制。

查询规则：

- 当前记录返回完整 `rawJson`；关联记录只返回 `id`、标题、来源字典、行号和摘要，不随列表返回 `rawJson`。用户点击关联记录时再用该记录 `id` 加载新的 `record-detail`。
- 关联记录摘要使用各自字典的 `visible` 字段配置和 `titleFieldPath` 生成，不能套用当前字典字段配置。
- 正向关系结果按目标字典排序配置排序；无排序配置时按目标记录 `row_index ASC, id ASC`。
- 反向关系结果按源字典排序配置排序；无排序配置时按源记录 `row_index ASC, id ASC`。
- 关系种子值和目标匹配值都必须来自字段值索引，而不是临时解析 `raw_json`；这样可以统一复用索引缺失、类型过滤和归一化规则。
- 关系源值或当前记录主键值为空字符串 / JSON `null` 时，对应关系组返回空列表，不视为错误。
- 历史字典重建索引后，如果某条历史记录自身没有有效主键索引行，则它的反向关系组返回空列表，不把该记录当作合法目标。
- 如果某条关系配置已经失效，例如目标字典缺少主键字段、字段值索引缺失或字段类型变成数组，`record-detail` 应返回错误，促使用户修复配置或重建索引，而不是静默隐藏关系。

### `rebuild-indexes`

新增：

```text
tool:data-dictionary:rebuild-indexes
```

`bridge/tauri.ts` 映射为：

```ts
"tool:data-dictionary:rebuild-indexes": { domain: "data_dictionary", action: "rebuild_indexes" }
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
  skippedPrimaryRecordCount: number;
  skippedPrimaryInvalidCount: number;
  skippedPrimaryDuplicateCount: number;
}
```

执行内容：

1. 删除该字典旧的 `data_dictionary_record_values`。
2. 从该字典每条 `data_dictionary_records.raw_json` 展开字段值并写入 `record_values`。
3. 如果该字典已配置 `primary_field_path`，统计主键异常记录；异常记录的主键字段不写入 `data_dictionary_record_values`，因此不会作为目标参与关系匹配。
4. 按当前字段 `searchable` 配置重建 `search_text` 和 `normalized_search_text`。
5. 更新 `data_dictionaries.field_value_indexed_at = CURRENT_TIMESTAMP`。
6. 如果 `data_dictionary_fts` 存在，重建该字典 FTS。

错误策略：

- 如果某条 `raw_json` 解析失败，直接中断并返回明确记录信息。
- 如果已配置主键字段但存在缺失、空值、`null`、非标量或重复值，不中断重建；返回异常统计，前端提示这些记录不会参与主键关系匹配。
- FTS5 不存在或写入失败不阻断主流程，沿用现有降级策略。
- 重建不修改原始记录、字段配置或关系配置。

## 前端设计

### 导入与替换提示

导入或替换记录成功后，如果后端返回 `skippedPrimaryRecordCount > 0`，成功提示必须包含跳过数量：

```text
已导入 1197 条记录，3 条主键异常记录未导入
```

如果需要展示详情，使用后端返回的拆分统计：

```text
未导入：2 条主键缺失/空值，1 条主键重复
```

异常记录不弹出阻断式错误；用户可在修正源 JSON 后重新替换导入。

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

如果存在主键异常记录，提示：

```text
已重建索引：1200 条记录，9600 个字段值。3 条主键异常记录不会参与关系匹配
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

前端保存前做轻量校验，后端仍是最终校验来源：

- 主键字段、标题字段、排序字段只能从当前字段集合选择。
- 关系源字段只能从当前字段集合选择。
- 目标字典必须已配置 `primaryFieldPath`。
- 同一 `源字段 + 目标字典` 不能重复。
- 自引用关系中，源字段不能等于当前字典主键字段。

保存失败时保留抽屉和用户输入，不清空草稿；错误信息使用后端返回的具体字段路径或字典名。

保存字段配置成功后，如果 `skippedPrimaryRecordCount > 0`，提示：

```text
已保存字段配置，3 条主键异常记录未纳入字典
```

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

详情加载状态：

- 点击搜索结果后用 `recordId` 调用 `record-detail`，右侧进入 loading 态。
- 详情请求必须使用请求序号或参数快照绑定当前选中项；旧响应晚到时直接丢弃，避免快速切换搜索结果后右侧展示错记录。
- 详情加载失败时保留当前选中搜索项，并在右侧显示错误和「重试」入口。
- 点击关联记录卡片时复用同一详情加载流程，不直接展开关联项 `rawJson`。

## 后端实现要点

1. 抽出可复用的字段值索引构建函数，输入 `record_id + dictionary_id + raw_json`，输出多条包含 `value_type` 的 `record_values`。
2. 字段值索引使用现有字段展开规则，保持点路径转义语义一致。
3. `normalized_value` 复用 `normalize_search_text`，保证等值匹配对大小写和空白一致。
4. 导入和替换记录时，在同一事务内写入 `data_dictionary_records` 和 `data_dictionary_record_values`。
5. `record-detail` 的正向和反向查询都走 `data_dictionary_record_values` 的 `(dictionary_id, field_path, normalized_value)` 索引。
6. 关联查询匹配值来自索引表，避免每次详情加载解析全部目标记录，同时避免 JSON `null` 与字符串 `"null"` 混淆。
7. `update-fields` 保存关系时校验配置合法性；如果主键配置存在或发生变化，需要过滤主键异常记录并重建 `record_values`。
8. `rebuild-indexes` 是索引修复入口，使用 `raw_json` 作为唯一来源。

建议函数边界：

```rust
fn build_record_values(record_id: i64, dictionary_id: i64, raw_json: &str) -> Result<Vec<RecordValue>, String>;
fn rebuild_dictionary_indexes(conn: &Connection, dictionary_id: i64) -> Result<RebuildStats, String>;
fn partition_records_by_primary(records: Vec<IndexedRecord>, primary_field_path: Option<&str>) -> Result<PrimaryPartition, String>;
fn load_relation_configs(conn: &Connection, source_dictionary_id: i64) -> Result<Vec<RelationConfig>, String>;
fn load_record_brief(conn: &Connection, record_id: i64) -> Result<RecordBrief, String>;
fn ensure_field_value_index_ready(conn: &Connection, dictionary_id: i64) -> Result<(), String>;
```

`PrimaryPartition` 至少包含：

```rust
struct IndexedRecord {
    source_row_index: i64,
    value: Value,
}

struct PrimaryPartition {
    accepted_records: Vec<IndexedRecord>,
    skipped_invalid_count: usize,
    skipped_duplicate_count: usize,
}
```

异常判定规则：

- 主键路径不存在：跳过。
- 主键值为 `null`：跳过。
- 主键值归一化后为空字符串：跳过。
- 主键值为数组或对象：跳过。
- 主键值归一化后已出现过：保留第一条，跳过后续重复记录。

实现细节：

- `create`：插入 `data_dictionaries`、字段配置、记录和字段值索引后提交事务；新字典默认 `primary_field_path = NULL`，不做主键校验。
- `replace_records`：如果字典已有 `primary_field_path`，先对新记录集合按主键分区；只写入有效记录，跳过异常记录并返回统计。
- `update_fields`：在事务内保存字典级配置、字段配置和关系配置；事务提交前校验主键字段路径存在和关系配置合法性。若设置了主键字段，则按当前 `raw_json` 重新分区并移除异常记录，同时返回跳过统计。
- `record_detail`：先加载当前记录和当前字典主键，并确认涉及字典字段值索引就绪；再分别用 `record_values` 查询正向关系和反向关系。任何一个已配置关系失效都返回错误，不在单个关系组内吞错。
- `delete`：删除字典前仍先清理 FTS；关系和字段值索引依赖外键级联删除。
- FTS5 写入失败沿用现有 `eprintln!` 降级策略；字段值索引写入失败必须返回错误并回滚事务。

## 错误处理

1. 主键字段不存在：保存字段配置时拒绝。
2. 主键值缺失、为空、为 `null`、非标量或归一化后重复：导入、替换或保存主键配置时跳过该记录，并返回跳过统计。
3. 目标字典没有主键字段：保存关系时拒绝。
4. 源字段不存在或不是标量字段：保存关系时拒绝。
5. 重复关系：保存关系时拒绝。
6. 字段值索引缺失或 `field_value_indexed_at IS NULL`：详情加载时报错，提示用户对对应字典执行「重建索引」。
7. 关系配置失效：详情加载时报错，提示用户重新进入字段配置修复关系。
8. 重建索引遇到损坏 `raw_json`：中断并返回记录 id 或行号。
9. 重建索引遇到主键异常：不中断，返回异常统计；异常记录不参与主键关系匹配。
10. JSON `null` 与字符串 `"null"`：必须通过 `value_type` 区分，二者不能互相匹配。
11. 关系查询没有匹配记录：返回空列表，不视为错误。

## 测试计划

### Rust 单测

新增或扩展 `data_dictionary` 测试：

1. `record_values_rebuilds_from_raw_json`
2. `record_values_supports_escaped_nested_paths`
3. `record_values_persists_value_type_for_null_vs_string_null`
4. `update_fields_rejects_missing_primary_field`
5. `update_fields_skips_non_scalar_primary_values`
6. `update_fields_skips_invalid_primary_records`
7. `update_fields_rejects_relation_without_target_primary`
8. `update_fields_rejects_non_scalar_relation_source`
9. `update_fields_rejects_duplicate_relations`
10. `record_detail_returns_forward_relations`
11. `record_detail_returns_reverse_relations_without_limit`
12. `record_detail_returns_error_when_value_index_missing`
13. `record_detail_does_not_match_json_null_to_string_null`
14. `record_detail_ignores_blank_relation_value`
15. `rebuild_indexes_refreshes_record_values_and_search_text`
16. `rebuild_indexes_marks_field_value_index_ready`
17. `rebuild_indexes_fails_on_corrupt_raw_json`
18. `replace_records_skips_duplicate_primary_values_when_primary_configured`
19. `replace_records_returns_skipped_primary_counts`
20. `replace_records_preserves_source_row_index_after_skips`

### 前端单测

新增或扩展纯函数与源码约束测试：

1. 字段配置抽屉包含主键字段配置。
2. 关系配置位于字段列表前。
3. 关系配置只选择目标字典，不渲染目标字段选择。
4. 字典右键菜单包含「重建索引」，不包含「关系」。
5. 详情关联分组展示全部 items，不依赖 `hasMore`。
6. 详情请求使用请求序号或参数快照，旧响应不会覆盖当前选中项。
7. 关联记录卡片点击后按 `recordId` 重新加载详情，不依赖关联项 `rawJson`。
8. 导入、替换和保存字段配置成功后展示主键异常跳过数量。

### 验证命令

1. `cargo test data_dictionary -- --nocapture`
2. `pnpm test src/utils/dataDictionary.test.ts src/utils/dataDictionaryRelations.test.ts src/components/DataDictionaryPanel.context-menu.test.ts`
3. `pnpm typecheck`
4. `pnpm --filter @lazycat/desktop build:web`

## 影响面

| 文件                                                                   | 类型 | 说明                                             |
| ---------------------------------------------------------------------- | ---- | ------------------------------------------------ |
| `apps/desktop/src-tauri/src/tools/helpers.rs`                          | 修改 | schema migration：主键字段、关系表、字段值索引表 |
| `apps/desktop/src-tauri/src/tools/data_dictionary.rs`                  | 修改 | 关系保存、详情查询、索引重建                     |
| `apps/desktop/src/bridge/tauri.ts`                                     | 修改 | 新增 `record-detail`、`rebuild-indexes` channel  |
| `apps/desktop/src/types/data-dictionary.ts`                            | 修改 | 新增关系、详情、重建索引类型                     |
| `apps/desktop/src/types/index.ts`                                      | 修改 | 导出新增数据字典类型                             |
| `apps/desktop/src/utils/dataDictionary.ts`                             | 修改 | 详情摘要和关联展示 helper                        |
| `apps/desktop/src/utils/dataDictionaryRelations.ts`                    | 新增 | 关系草稿校验、目标主键展示、重复关系检测等纯函数 |
| `apps/desktop/src/components/DataDictionaryPanel.vue`                  | 修改 | 字段抽屉、关系配置、详情面板、重建索引菜单       |
| `apps/desktop/src/components/DataDictionaryPanel.context-menu.test.ts` | 修改 | 菜单和抽屉结构回归                               |
| `apps/desktop/src/utils/dataDictionary.test.ts`                        | 修改 | 详情与关系 helper 测试                           |
| `apps/desktop/src/utils/dataDictionaryRelations.test.ts`               | 新增 | 关系草稿纯函数测试                               |

## 风险与回滚

风险等级：中。

主要风险：

1. 关系配置使字段配置抽屉复杂度上升。
2. 详情页全量返回反向关系，如果某个关系命中几千条记录，前端渲染需要滚动容器承载。
3. 字段值索引引入派生数据一致性问题，需要导入、替换和手动重建路径都覆盖。
4. 主键异常记录会被跳过，用户可能发现导入后记录数少于源 JSON，需要明确提示未导入数量。

控制方式：

1. 保留 `raw_json` 作为唯一事实源，索引可重建。
2. 关系配置和字段配置同事务保存，避免半成功。
3. 单字典重建索引入口用于修复派生索引。
4. 不引入全文检索新引擎，避免索引目录和同步复杂度扩大。
5. 关联列表不返回关联项 `rawJson`，降低反向关系大结果集的 IPC 体积。
6. 所有会跳过主键异常记录的接口都返回统一统计字段，前端提示总数和原因拆分。

回滚方式：

1. 前端隐藏关系配置和详情关联展示。
2. 后端保留新增表不使用，对旧数据字典搜索无影响。
3. 如需彻底回滚，移除新增 channel 和 UI 后，新增表可作为无副作用历史数据保留。
