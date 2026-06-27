# 数据字典查询结果按左侧字典顺序与派生排序键排序设计

## 背景

数据字典左侧列表已经支持拖拽排序，后端通过 `data_dictionaries.nav_order` 持久化导航顺序。当前“全部”查询结果仍按 `updated_at DESC, row_index ASC` 排序，和左侧字典顺序不一致。

数据字典还支持每个字典配置记录排序字段：`sort_field_path` 与 `sort_direction`。当前单字典搜索会在后端解析每条记录的 `raw_json` 后排序，再截断结果。这个方案语义正确，但全局搜索要跨字典排序时，如果仍在内存中按每个字典临时解析，查询成本和排序链路都会继续复杂化。

本次设计采用记录级派生排序键：生成索引时把当前排序字段对应的值编码为一个可直接 `ORDER BY` 的 `sort_key`，查询时不再解析 `raw_json`。

## 目标

1. “全部”查询结果先按左侧字典顺序排列，再按每个字典自己的记录排序配置排列。
2. 排序必须发生在结果截断前，避免先取 100 条再排序造成结果不完整。
3. 查询阶段直接使用记录上的派生排序键，避免搜索时反复解析 `raw_json`。
4. `sort_key` 只保留一个字段，表示“按当前字典排序配置预先编码好的完整排序键”，查询永远按 `sort_key COLLATE BINARY ASC`。
5. 没有配置排序字段时，组内保持原始 `row_index ASC, id ASC` 顺序。

## 非目标

1. 不改变字段配置抽屉的交互。
2. 不新增前端排序开关。
3. 不支持跨字典直接混排。全局结果仍以字典为分组，先尊重左侧字典顺序。
4. 不把 `data_dictionary_record_values.normalized_value` 直接作为排序字段使用。

## 数据模型

在 `data_dictionary_records` 增加一个派生字段：

```sql
ALTER TABLE data_dictionary_records ADD COLUMN sort_key TEXT NOT NULL DEFAULT '';
CREATE INDEX IF NOT EXISTS idx_data_dictionary_records_dictionary_sort
  ON data_dictionary_records(dictionary_id, sort_key, id);
```

`DEFAULT ''` 只用于兼容 SQLite `ALTER TABLE ADD COLUMN` 的过渡状态；稳定状态下 `sort_key` 必须由回填或重建流程写成非空编码。

`sort_key` 的含义：

- 它不是原始业务值。
- 它由当前字典的 `sort_field_path` 和 `sort_direction` 生成。
- 它总是非空；当没有业务排序值时，使用 `row_index` 生成兜底排序键。
- 它只用于排序，真实记录内容仍以 `raw_json` 为唯一事实源。
- 当字典未配置 `sort_field_path`，使用 `row_index` 生成完整 `sort_key`。
- 当字典已配置 `sort_field_path` 但某条记录缺失该字段，使用“缺失值桶 + row_index”生成完整 `sort_key`，让缺失记录排在有值记录之后，并在缺失记录内部保持原始行序。

查询排序直接使用 `sort_key`：

```sql
ORDER BY
  d.nav_order ASC,
  r.sort_key COLLATE BINARY ASC,
  r.id ASC
```

这样可以把“排序字段值”“缺失值位置”“row_index 兜底”都归一化到一个字段中。没有排序配置的字典自然按 `row_index` 排序。

## Sort Key 编码

`sort_key` 使用可按字典序比较的字符串编码。编码必须同时处理类型、升降序和缺失值。

生成流程固定为：

1. 根据排序字段配置、字段值和 `row_index` 生成“升序原始键字节”。
2. 如果 `sort_direction = 'desc'` 且记录存在配置字段值，只对 `encoded-value` 段的每个字节执行 `255 - byte`，得到反向值段。
3. 把最终键字节编码成大写十六进制字符串，写入 `sort_key`。

升序原始键字节的逻辑结构：

```text
<bucket-rank>|<encoded-value>|<row-index-key>
```

`bucket-rank` 固定表达排序来源：

```text
0 configured value present
1 row_index fallback because sort_field_path is not configured
2 row_index fallback because configured sort field is missing
```

配置了排序字段且记录有对应值时，`encoded-value` 内部再带类型顺序：

```text
1 number
2 string
3 boolean
4 null
5 other
```

`row-index-key` 始终追加到 `sort_key` 末尾，作为同值兜底。未配置排序字段或记录缺失排序字段时，`encoded-value` 为空，仅通过 bucket 和 `row-index-key` 排序。

### 升序

数字类型必须编码为定长可比较形式，不能直接使用文本值。否则 `"10"` 会排在 `"2"` 前面。实现时使用 JSON number 的有限 `f64` 值生成可排序位模式：

1. 取 `f64::to_bits()`。
2. 正数翻转符号位。
3. 负数翻转所有位。
4. 输出 16 位大写十六进制。

这类编码可以保证按文本比较时仍保持数值升序。

字符串类型使用 UTF-8 字节的大写十六进制表示，并追加一个小于十六进制字符的终止标记，保证 `"a"` 排在 `"aa"` 前。布尔值编码为 `0/1`。`null` 使用固定空值编码。数组和对象按紧凑 JSON 文本走 `other` 类型编码。

`row_index` 也必须编码为定长可比较形式，例如 16 位十六进制无符号整数，保证文本排序等价于数值排序。

### 降序

方案固定为“生成当前方向生效的 `sort_key`，查询永远 `ASC`”。

当 `sort_direction = 'desc'` 时，只反转“配置字段值”部分，不反转 bucket 和 `row-index-key`。这样可以保证：

1. 有排序字段值的记录按业务值降序。
2. 缺失排序字段的记录仍排在有值记录之后。
3. 同值记录与缺失记录内部仍按 `row_index ASC` 稳定排序。

查询仍使用：

```sql
ORDER BY r.sort_key COLLATE BINARY ASC
```

未配置排序字段时不受 `sort_direction` 影响，始终生成 `row_index` 兜底键。

## 维护时机

所有会改变记录内容、排序字段或排序方向的路径都必须同步维护 `sort_key`。

需要覆盖：

1. 新建字典：初始通常没有排序配置，插入记录时按 `row_index` 生成 `sort_key`。
2. 替换记录：按当前字典排序配置为新记录生成 `sort_key`。
3. 字段配置保存：如果 `sort_field_path` 或 `sort_direction` 变化，重算该字典所有记录的 `sort_key`。
4. 重建索引：从 `raw_json` 重建字段值索引与搜索文本时，同时重建 `sort_key`。
5. 历史数据库新增 `sort_key` 后，必须回填所有既有记录；回填失败时显式报错，不允许保留空排序键进入查询。

`raw_json` 仍是唯一事实源。`sort_key` 是可重建的派生数据，不能作为业务数据来源。

## 查询语义

### 当前字典查询

当前字典查询按该字典记录排序：

```sql
ORDER BY
  r.sort_key COLLATE BINARY ASC,
  r.id ASC
```

当字典没有排序字段时，所有 `sort_key` 都由 `row_index` 编码生成，最终等价于 `row_index ASC, id ASC`。

### 全部查询

全部查询按左侧导航顺序分组，再在组内按每个字典自己的 `sort_key` 排序：

```sql
ORDER BY
  d.nav_order ASC,
  r.sort_key COLLATE BINARY ASC,
  r.id ASC
```

这里不做不同字典之间的排序键混排。原因是不同字典的排序字段含义不同，把它们混成一个全局顺序会降低结果可解释性，也会破坏用户通过左侧顺序组织字典的意图。

## 与字段值索引的关系

`data_dictionary_record_values` 继续表示每条记录的字段值派生索引，服务关系查询和等值匹配。它不直接承担记录排序。

生成 `sort_key` 时可以复用同一套字段展开与路径解析规则，但排序键写入 `data_dictionary_records.sort_key`，查询时直接按记录表字段排序。

这样可以避免：

1. 搜索查询为了排序额外 join 字段值表。
2. `normalized_value` 同时承担匹配和排序两种语义。
3. 排序逻辑依赖关系索引是否包含某个特殊字段。

## 错误处理

1. `sort_field_path` 配置仍必须由后端校验，字段不存在时拒绝保存。
2. `raw_json` 解析失败时，重建索引或重算排序键必须显式失败，不能静默生成空排序键。
3. 排序值为数组或对象时按 `other` 类型编码，语义保持稳定但不承诺业务含义优先级。
4. 历史数据库新增 `sort_key` 后必须立即回填；查询路径如果发现空 `sort_key`，应拒绝继续并提示重建索引，不能静默返回错误顺序。

## 前端影响

前端不需要新增状态或交互。

现有“全部”入口继续调用 `tool:data-dictionary:search`，后端返回顺序即为最终顺序。搜索结果展示、标题字段、摘要字段和详情加载逻辑保持不变。

## 测试计划

### Rust 单测

新增或扩展 `data_dictionary` 测试：

1. `build_record_sort_key_orders_numbers_numerically`
2. `build_record_sort_key_supports_desc_without_moving_missing_values_first`
3. `build_record_sort_key_uses_row_index_when_sort_field_is_not_configured`
4. `build_record_sort_key_uses_missing_bucket_and_row_index_when_sort_field_is_missing`
5. `query_all_orders_by_nav_order_then_record_sort_key`
6. `query_current_falls_back_to_row_index_without_sort_field`
7. `update_fields_rebuilds_sort_keys_when_sort_config_changes`
8. `rebuild_indexes_refreshes_sort_keys`

### 前端测试

前端不新增交互，只保留现有数据字典组件测试。若源码约束测试需要更新，只验证搜索请求仍不携带前端排序参数。

### 验证命令

1. `cargo test data_dictionary -- --nocapture`
2. `pnpm test src/components/DataDictionaryPanel.context-menu.test.ts src/utils/dataDictionary.test.ts`
3. `pnpm typecheck`
4. `pnpm --filter @lazycat/desktop build:web`

## 影响面

| 文件 | 类型 | 说明 |
|---|---|---|
| `apps/desktop/src-tauri/src/tools/helpers.rs` | 修改 | schema 增加 `data_dictionary_records.sort_key` |
| `apps/desktop/src-tauri/src/tools/data_dictionary.rs` | 修改 | 生成、重算、查询排序键 |
| `process.md` | 可能修改 | 完成复杂任务后记录经验 |

前端文件原则上不需要改动。

## 风险与回滚

风险等级：中。

主要风险：

1. 排序键编码必须保证数字排序正确。
2. 降序编码如果反转 bucket 或 `row-index-key`，缺失值可能被排到最前，或同值记录顺序会反转。
3. 历史数据回填失败会留下空排序键，查询结果会不可信。

控制方式：

1. 用单元测试覆盖数字、字符串、布尔、缺失值和降序。
2. 重建索引路径同步刷新 `sort_key`，作为历史数据修复入口。
3. 查询路径发现空 `sort_key` 时显式失败，并提示重建索引。
