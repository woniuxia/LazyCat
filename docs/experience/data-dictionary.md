# 数据字典经验

适用范围：动态 JSON、字段配置、检索、排序、关系、导入和异步交互。

关键词：`raw_json`、`sort_key`、`record_values`、`FTS`、`nav_order`

## 原始 JSON 是唯一业务事实源

`data_dictionary_records.raw_json` 是唯一业务事实源；`search_text`、`normalized_search_text`、`sort_key`、`data_dictionary_record_values`、`data_dictionary_fts` 都是可重建派生索引。导入、替换、字段配置保存、重建索引和历史回填必须同步维护派生数据；派生索引失败时不得破坏原始记录。

## 配置职责必须独立

字典级的主键、记录标题、记录排序、导航顺序与字段级的标签、可见性、展示顺序是独立模型。字段路径遵循既有转义点路径规则，不能简单 `split('.')`。展示拖拽只作用于可见字段，隐藏字段不混入索引。

## 排序使用记录级 `sort_key`

查询统一按 `sort_key COLLATE BINARY ASC`。降序只反转业务值编码段，不反转缺失 bucket 与 `row_index` 兜底；未配置或值缺失时按 `row_index` 保持稳定顺序。跨字典搜索先按 `nav_order`，再按记录 `sort_key`；`hasMore` 用多取一条判断。

## 关系查询使用类型化字段值索引

关系查询依赖 `data_dictionary_record_values`，必须保留 `value_type` 区分 `null` 与字符串 `"null"` 等 JSON 类型。索引未就绪时显式提示重建，不能表现为“无关联记录”。

## 大 JSON 导入绕开通用 IPC 文本负载

10MB 级 JSON 优先传 `inputPath` 由后端读取。预览绑定输入来源，保存前确认预览与提交来源一致，避免用户切换文件后提交旧预览。

## 异步响应绑定当前意图

搜索、字典详情和记录详情使用请求序号或参数快照；旧响应不得覆盖当前字典、当前筛选或当前详情。影响范围大的替换、删除、重建操作必须二次确认目标与数量。

## Spotlight 使用查询时 provider

数据字典内容动态且可能较大，Spotlight 通过 query-time provider 查询，不把全部记录灌入长期静态缓存；缓存失效按 provider 粒度处理。

## 验证

```powershell
cargo test data_dictionary -- --nocapture
pnpm test src/components/DataDictionaryPanel.context-menu.test.ts src/utils/dataDictionary.test.ts src/utils/dataDictionaryRelations.test.ts src/utils/dataDictionaryMenu.test.ts
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
```

**使用次数**：0
