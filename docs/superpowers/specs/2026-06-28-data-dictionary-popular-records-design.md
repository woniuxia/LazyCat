# 数据字典常用记录与主键强制化设计

## 概述

本轮迭代聚焦已有数据字典的日常查找效率。当前数据字典已经支持导入、替换、字段配置、标题字段、排序字段、跨字典搜索、正反向关系详情、索引重建和 Spotlight 检索；继续增加复杂查询、多跳关系或详情工作台的边际收益不高。

本设计新增「常用记录」能力，并把业务主键升级为数据字典的必要配置。常用记录不保存数据库行 ID、标题或摘要快照，只保存业务主键引用和访问统计。展示时实时按当前主键字段解析记录，确保页面看到的是当前数据，而不是历史快照。

## 目标

1. 新建字典必须配置主键字段，主键校验通过后才能保存。
2. 历史无主键字典进入受限状态，引导用户先配置主键。
3. 常用记录在数据库中使用 `dictionary_id + normalized_value` 定位，不使用数据库行 ID 作为长期引用。
4. 记录详情有效展示后累计使用次数和最后使用时间。
5. 空关键词时展示常用记录，按使用次数和最近使用时间排序。
6. 输入关键词后沿用现有搜索排序，不让常用度干扰搜索可信度。
7. 字典替换、删除或主键变更后，常用记录按实时解析结果保留或清理。

## 非目标

1. 不实现记录工作台或详情区大重构。
2. 不新增固定记录、收藏记录或手动分组。
3. 不新增复杂查询语言、字段筛选 DSL、多跳关系或关系图。
4. 不支持无主键字典写入常用记录。
5. 不保存标题、摘要、字段值等展示快照；只保存业务主键值和归一化查找值。
6. 不迁移主键字段变更前的常用记录。

## 产品规则

### 主键规则

所有字典都必须具备主键字段。

新建字典流程：

1. 用户选择或粘贴 JSON 后点击预览。
2. 预览区展示字段列表，并要求选择主键字段。
3. 未选择主键字段时禁用保存。
4. 保存时把 `primaryFieldPath` 传给后端。
5. 后端校验主键值必须是可用标量，且在字典内非空、非 `null`、不重复。
6. 主键异常记录沿用现有策略不写入，并返回跳过数量供前端提示；前端必须明确展示跳过数量，不能静默成功。

历史字典流程：

1. 如果 `primaryFieldPath` 为空，当前字典模式显示受限提示。
2. 受限提示提供「配置主键」入口，打开现有字段配置抽屉。
3. 配置前不展示常用记录，不写入使用次数。
4. 全部搜索仍保留历史行为，避免旧数据突然不可查。

主键更换流程：

1. 字段配置中允许更换主键，但不允许清空主键。
2. 后端按新主键重新校验记录。
3. 如果主键变更会剔除现有记录，后端先返回明确错误和跳过统计；前端二次确认后带确认标记重试。
4. 已有常用记录不做迁移；后续读取常用记录时，无法命中的条目自动清理。

### 常用记录规则

常用记录本质是访问索引，不是纯最近查看列表。

数据库记录维度：

- `dictionary_id`：字典 ID。
- `record_id`：记录自身的业务主键值，来自当前字典 `primary_field_path` 对应字段。
- `normalized_value`：业务主键值按字段值索引口径归一化后的查找值。
- `used_count`：成功查看记录详情的累计次数。
- `last_used_at`：最近一次成功查看详情的时间。

这里的字段调整只作用于新增的 `data_dictionary_record_usage` 表。现有 `data_dictionary_records.id` 仍是数据库行 ID，继续用于详情加载、关系查询和内部 join。

行为规则：

1. 只有记录详情成功加载，并且该详情响应仍对应当前用户选择时，才累计使用次数。
2. 计数使用显式 API，不在 `record-detail` 内隐式写入，避免快速切换时旧详情响应误计数。
3. 对同一 `(dictionary_id, normalized_value)` 执行 upsert：首次写入 `used_count = 1`，后续 `used_count + 1` 并更新 `last_used_at`。
4. 空关键词时展示常用记录分区，排序为 `usedCount DESC, lastUsedAt DESC`。
5. 关键词非空时隐藏常用记录分区，搜索结果排序完全沿用现有 `search`。
6. 常用记录最多展示 10 条。

## 数据模型

新增表：

```sql
CREATE TABLE IF NOT EXISTS data_dictionary_record_usage (
  dictionary_id INTEGER NOT NULL,
  record_id TEXT NOT NULL,
  normalized_value TEXT NOT NULL,
  used_count INTEGER NOT NULL DEFAULT 1,
  last_used_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY(dictionary_id, normalized_value),
  FOREIGN KEY(dictionary_id) REFERENCES data_dictionaries(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_data_dictionary_record_usage_order
  ON data_dictionary_record_usage(dictionary_id, used_count DESC, last_used_at DESC);

CREATE INDEX IF NOT EXISTS idx_data_dictionary_record_usage_global_order
  ON data_dictionary_record_usage(used_count DESC, last_used_at DESC);
```

字段语义：

- `dictionary_id`：字典 ID。
- `record_id`：当前记录主键字段的业务值，用作对外记录标识；它不是 `data_dictionary_records.id`。
- `normalized_value`：按现有字段值索引归一化口径得到的查找值，用于匹配当前记录。
- `used_count`：成功查看记录详情的累计次数。
- `last_used_at`：最近一次成功查看详情的时间。
- 该表不新增 `primary_value` 字段；原本容易误解的 `primaryValue` 概念统一落到数据库字段 `normalized_value`。

不保存：

- 不保存数据库行 ID，因为替换数据后数据库行 ID 会变化。
- 不保存标题或摘要快照，避免展示旧信息。
- 不保存原始主键字段路径，读取时始终使用当前字典的 `primary_field_path`。

## 后端 API

### `create`

扩展现有 `tool:data-dictionary:create`：

```ts
interface CreateDataDictionaryRequest {
  name: string;
  description?: string;
  input?: string;
  inputPath?: string;
  primaryFieldPath: string;
}
```

行为：

1. `primaryFieldPath` 必填。
2. 字段路径必须存在于导入预览推断出的字段集合。
3. 按主键规则过滤异常记录。
4. 写入 `data_dictionaries.primary_field_path`。
5. 写入记录、字段值索引、搜索文本和排序键。

### `update_fields`

调整现有字段配置保存：

1. `primaryFieldPath` 必填，不接受 `null` 或空字符串。
2. 主键字段必须存在于本次提交字段集合。
3. 如果主键变更，按现有主键校验路径预检查异常记录。
4. 如果预检查发现会剔除记录，且请求未带确认标记，返回错误和跳过统计，不修改数据。
5. 前端二次确认后带确认标记重试，后端重新过滤异常记录并重建索引。
6. 不主动清理 usage；读取常用记录时按实时命中结果清理。

### `popular_records`

新增 action：

```ts
interface DataDictionaryPopularRecordsRequest {
  dictionaryId?: number;
  limit?: number;
}

interface DataDictionaryPopularRecord {
  id: number;
  recordId: string;
  dictionaryId: number;
  dictionaryName: string;
  title: string;
  rowIndex: number;
  summary: DataDictionaryRecordSummaryPart[];
  normalizedValue: string;
  usedCount: number;
  lastUsedAt: string;
}

interface DataDictionaryPopularRecordsResult {
  items: DataDictionaryPopularRecord[];
}
```

行为：

1. `id` 是当前 `data_dictionary_records.id`，只用于加载详情；`recordId` 是数据主键字段的业务值。
2. `dictionaryId` 为空时返回跨字典常用记录；有值时只返回当前字典。
3. 默认 `limit = 10`，最大不超过 50。
4. 只返回有主键的字典。
5. 按 `used_count DESC, last_used_at DESC` 读取 usage 候选。
6. 对每条候选，使用当前字典 `primary_field_path` 和 `normalized_value` 到 `data_dictionary_record_values` 查当前记录，且只匹配 `value_type IN ('string', 'number', 'boolean')`。
7. 命中后用现有标题和摘要构造逻辑返回轻量记录。
8. 未命中的 usage 行立即删除。
9. 删除失效 usage 后不循环补齐；本次有几条有效常用记录就返回几条。

### `mark_record_used`

新增 action：

```ts
interface MarkDataDictionaryRecordUsedRequest {
  id: number;
}

interface MarkDataDictionaryRecordUsedResult {
  ok: true;
}
```

行为：

1. 通过当前 `data_dictionary_records.id` 加载当前记录和字典。
2. 字典必须已配置 `primary_field_path`。
3. 从字段值索引读取该记录主键字段的值。
4. 主键值必须满足关系匹配可用条件：`string | number | boolean`，且归一化后非空。
5. 将主键字段业务值写入 `record_id`，将字段值索引中的 `normalized_value` 写入 `normalized_value`。
6. 对 `(dictionary_id, normalized_value)` 执行 upsert。
7. 如果记录不存在、字典无主键或主键值不可用，返回明确错误；前端可以静默忽略或提示。

### `record_detail`

保持只读，不在内部写 usage。

原因：

1. 前端现有详情加载有请求序号，旧响应会被丢弃。
2. 如果 `record_detail` 内隐式计数，旧响应即使被前端丢弃，也已经污染使用次数。
3. 显式 `mark_record_used` 让“用户实际看到了当前详情”成为计数前提。

## 前端交互

### 导入弹窗

预览成功后增加主键字段选择：

1. 主键字段选项来自 `preview.fields`。
2. 默认不自动保存；可以后续根据字段名给出推荐值，但本轮不做推荐规则。
3. 未选择主键时保存按钮禁用。
4. 保存请求带上 `primaryFieldPath`。
5. 后端返回主键异常统计时沿用现有导入提示。

### 字段配置抽屉

调整主键字段控制：

1. 主键字段不再允许清空。
2. 历史无主键字典打开抽屉时，主键字段为空但保存前必须选择。
3. 保存时如果后端提示主键变更会剔除记录，弹出二次确认，用户确认后带确认标记重试。

### 当前字典受限状态

当用户选择一个无主键历史字典：

1. 中间结果区显示「请先配置主键字段」。
2. 提供「配置主键」按钮。
3. 当前字典模式不展示常用记录。
4. 当前字典模式不写入使用次数。
5. 全部模式搜索仍可命中该字典记录，但这些记录详情不写 usage。

### 常用记录分区

空关键词时展示：

1. 当前字典模式：请求该字典常用记录。
2. 全部模式：请求跨字典常用记录。
3. 常用记录展示在现有结果列表顶部。
4. 卡片复用现有结果项结构，增加 `使用 N 次` 状态。
5. 常用记录下面继续显示现有默认搜索结果，避免空关键词只剩历史访问入口。
6. 默认选中第一条常用记录；没有常用记录时才选中默认搜索结果第一条。
7. 同一记录同时出现在常用记录和默认搜索结果时，只在常用记录分区展示一次。

关键词非空时：

1. 隐藏常用记录分区。
2. 只展示现有搜索结果。
3. 搜索排序不受 `usedCount` 影响。

### 计数触发

前端流程：

1. 用户点击搜索结果、常用记录、关联记录或通过 Spotlight 定位记录。
2. 前端调用 `record-detail`。
3. 详情响应返回后，先检查请求序号仍有效。
4. 当前详情真实展示后，再调用 `mark-record-used`。
5. `mark-record-used` 失败不影响详情展示。
6. 计数成功后可轻量刷新常用记录分区；如果关键词非空则无需刷新。

## 错误处理

1. 新建字典缺少主键：前端禁用保存，后端仍返回明确错误。
2. 主键字段不存在：后端拒绝保存。
3. 主键值缺失、空值、`null`、数组、对象或重复：按现有主键异常策略跳过记录并返回统计。
4. 历史字典无主键：当前字典模式展示配置引导，不写 usage。
5. 常用记录解析不到当前记录：后端删除 usage 行，不返回给前端。
6. 主键变更会剔除记录：首次保存返回错误和跳过统计，前端确认后才允许继续。
7. `mark_record_used` 失败：不阻断详情展示。
8. 删除字典：usage 通过外键级联或显式删除清理。

## 测试计划

Rust 单测：

1. `create` 缺少 `primaryFieldPath` 时拒绝。
2. `create` 写入主键字段并按主键异常统计跳过记录。
3. `update_fields` 拒绝清空主键。
4. `mark_record_used` 对同一主键执行 upsert 并增加 `used_count`。
5. 无主键字典调用 `mark_record_used` 返回明确错误。
6. `popular_records` 按 `used_count DESC, last_used_at DESC` 排序。
7. `popular_records` 实时解析标题和摘要，不依赖快照。
8. 替换字典后，同主键 usage 仍能命中当前记录。
9. 删除记录或主键变更后，`popular_records` 清理失效 usage。
10. 主键变更会剔除记录且缺少确认标记时，`update_fields` 拒绝修改并返回跳过统计。

前端测试：

1. 导入预览后必须选择主键才能保存。
2. 创建请求携带 `primaryFieldPath`。
3. 字段配置抽屉不允许清空主键。
4. 空关键词时显示常用记录分区。
5. 关键词非空时隐藏常用记录分区。
6. 详情响应仍有效后才调用 `mark-record-used`。
7. `mark-record-used` 失败不清空详情。
8. 主键变更会剔除记录时，字段配置抽屉展示二次确认并带确认标记重试。
9. 空关键词下常用记录与默认搜索结果重复时只展示一次，并优先选中常用记录。

验证命令：

1. `cargo test data_dictionary -- --nocapture`
2. `pnpm test src/components/DataDictionaryPanel.context-menu.test.ts src/utils/dataDictionary.test.ts src/utils/dataDictionaryRelations.test.ts src/utils/dataDictionaryMenu.test.ts`
3. `pnpm typecheck`
4. `pnpm --filter @lazycat/desktop build:web`

## 影响范围

预计修改文件：

| 文件                                                                   | 类型 | 说明                                             |
| ---------------------------------------------------------------------- | ---- | ------------------------------------------------ |
| `apps/desktop/src-tauri/src/tools/helpers.rs`                          | 修改 | 新增 usage 表和索引                              |
| `apps/desktop/src-tauri/src/tools/data_dictionary.rs`                  | 修改 | 主键必填、常用记录查询、使用次数写入             |
| `apps/desktop/src/bridge/tauri.ts`                                     | 修改 | 新增 action 映射                                 |
| `apps/desktop/src/types/data-dictionary.ts`                            | 修改 | 新增常用记录类型                                 |
| `apps/desktop/src/components/DataDictionaryPanel.vue`                  | 修改 | 主键必选、无主键受限状态、常用记录分区、显式计数 |
| `apps/desktop/src/utils/dataDictionary.ts`                             | 修改 | 常用记录展示辅助逻辑                             |
| `apps/desktop/src/components/DataDictionaryPanel.context-menu.test.ts` | 修改 | 组件结构和交互回归                               |
| `apps/desktop/src/utils/dataDictionary.test.ts`                        | 修改 | 展示逻辑测试                                     |

## 取舍

1. 强制主键会让新建字典多一步选择，但换来稳定的记录引用。
2. 常用记录不存快照，展示时需要实时解析；这增加一次后端查询，但避免陈旧信息。
3. 显式 `mark_record_used` 比隐式计数多一次 IPC，但能避免旧详情响应误计数。
4. 历史无主键字典仍可参与全部搜索，降低兼容风险；只有常用记录和计数依赖主键。
