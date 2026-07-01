# API Workbench 响应内容预览设计

## 背景

接口调试工具当前的响应区只支持“美化 / 原文”两种文本展示方式。后端会把响应字节统一通过 `String::from_utf8_lossy` 转成字符串，再返回给前端。这个模型适合 JSON 和普通文本，但会损坏图片、PDF、Office 等二进制响应，也无法让历史记录重新预览这些内容。

本次设计目标是让接口调试工具根据响应内容类型选择合适的预览方式：

- JSON 格式化展示。
- HTML 默认沙箱预览，并保留源码视图。
- 图片、PDF 正常应用内预览。
- Word、Excel、PPT 做离线基础可读预览，不追求高保真还原。
- 常见未知二进制响应保留文件信息和原文件操作。
- 二进制响应可在历史记录中重新预览，并在删除历史时清理对应缓存文件。

## 已确认决策

1. Office 预览选择“离线内置基础预览”。目标是可读、可检查，不承诺完全还原 Word / Excel / PowerPoint 版式。
2. HTML 默认使用沙箱 iframe 预览，不执行脚本；用户可以切换到源码。
3. 二进制响应写入数据目录下的接口调试缓存目录。
4. 缓存默认不按时间或容量自动清理。
5. 删除单条历史、清空历史、清理未收藏历史时，同时删除被清理历史引用且不再被其他历史引用的缓存文件。
6. 不引入 `open-file-viewer` 等大文件预览 SDK。前端采用自研轻量 Viewer；Office 只做基础内容提取和展示。

## 可行性结论

该功能在当前项目架构下可行，但需要明确几个实现边界：

- 现有发送路径已经能拿到原始响应字节，只是当前在 `response_to_json` 中统一通过 `String::from_utf8_lossy` 转成字符串。实现时应把“读取字节、判断存储类型、生成响应 JSON”拆开，避免二进制被提前损坏。
- 数据目录能力已经存在，响应缓存可以复用 `get_data_dir()`，落到 `<dataDir>/api-workbench/response-cache/`。
- 图片和 PDF 预览可以复用前端 `convertFileSrc()`。
- Excel 基础预览可以复用后端已有 `calamine` 依赖；CSV 可以复用现有 `csv` 依赖或前端轻量解析。
- Word / PowerPoint 的 `docx/pptx` 是 OpenXML zip 包。当前项目已有 `quick-xml`，但没有通用 zip 解包依赖；首版若要支持 `docx/pptx` 基础提取，需要新增轻量 `zip` crate。该依赖只用于解压 OpenXML 文本，不属于大型预览 SDK。
- 旧版 `doc/ppt` 二进制格式不适合作为首版重点。若无法可靠提取，直接回退到文件信息和打开文件。

首版推荐实现范围：

- 完整支持 JSON、HTML、图片、PDF、文本和未知二进制兜底。
- 表格类优先支持 `xlsx/xls/ods/csv` 基础表格预览。
- 文档和演示类优先支持 `docx/pptx` 的基础文本提取；`doc/ppt/rtf/odt/odp` 能提取则展示，不能提取则明确提示不支持高保真预览。

## 非目标

- 不实现高保真 Office 渲染。
- 不接入 LibreOffice、OnlyOffice、WPS、Microsoft Graph 或其他服务端转换链路。
- 不让 HTML 响应默认执行脚本。
- 不把大二进制内容或完整大文本塞进 SQLite。
- 不在本次实现缓存容量管理、缓存列表管理或手动缓存清理页面。

## 总体架构

采用“后端保留原始响应产物，前端选择渲染器”的结构。

后端发送请求后读取原始响应字节，并根据响应头、文件名、扩展名和字节特征进行分类：

- 文本类响应继续返回 `bodyText`。
- 二进制类响应写入缓存目录，并返回缓存文件元信息。
- 历史记录保存响应元信息和缓存文件引用。

发送路径改造建议拆成 4 个小函数，避免继续膨胀主流程：

- `read_response_body(resp)`：读取最多 `MAX_RESPONSE_BODY_BYTES + 1` 字节，返回 `bytes`、`bodySize`、`bodyTruncated`。
- `classify_response_body(headers, finalUrl, bytes)`：判断 `text` / `file` / `empty` / `truncated-binary`，并推导 MIME、扩展名和展示类型线索。
- `persist_response_cache(bytes, meta)`：只在完整二进制响应时写入缓存目录。
- `build_send_result(...)`：统一组装前端和历史使用的响应 JSON。

前端新增响应预览分类层和响应 Viewer 组件：

- 分类层是纯函数，输入响应元信息，输出 `viewerKind`。
- Viewer 组件根据 `viewerKind` 渲染 JSON、HTML、图片、PDF、Office、文本或二进制兜底视图。
- `ApiWorkbenchPanel.vue` 只负责状态编排、发送请求、载入历史和展示响应容器，不承担复杂分类和文件解析逻辑。

## 后端响应模型

扩展 `ApiWorkbenchSendResult` 对应的后端 JSON 结构，保留现有字段并新增二进制缓存相关字段。

建议字段：

```ts
interface ApiWorkbenchSendResult {
  finalUrl: string;
  status: number | null;
  statusText: string;
  ok: boolean;
  durationMs: number;
  requestHeaders: ApiWorkbenchKeyValueRow[];
  responseHeaders: ApiWorkbenchKeyValueRow[];
  bodyText: string;
  bodySize: number;
  bodyTruncated: boolean;
  contentType: string;
  error: string | null;

  bodyStorage: "text" | "file" | "empty" | "truncated-binary";
  bodyFilePath: string;
  bodyFileName: string;
  bodyExtension: string;
  bodyHash: string;
  bodyPreviewError: string | null;
}
```

行为规则：

- `text`：`bodyText` 有效，`bodyFilePath` 为空。
- `file`：缓存文件有效，`bodyText` 通常为空或只保存短提示。
- `empty`：无响应体。
- `truncated-binary`：响应体超过读取上限且不是可安全预览文本，不写半截二进制预览文件。

现有 `bodyText/bodySize/bodyTruncated/contentType` 保留，降低前端和历史兼容成本。

字段语义补充：

- `bodySize` 表示本次实际读取到的响应体字节数；如果超过读取上限，则为截断后的读取大小，`bodyTruncated=true`。首版不尝试从 `Content-Length` 推断完整远端大小。
- `bodyHash` 使用读取到的完整二进制内容计算。截断二进制不生成 hash，避免半截内容参与引用去重。
- `bodyFilePath` 使用绝对路径，便于 `convertFileSrc()` 和本地文件操作；所有后端 action 必须再次校验该路径位于响应缓存目录下。
- `bodyFileName` 是展示名，优先来自 `Content-Disposition`，其次 URL 文件名，最后由时间戳和扩展名生成。
- `bodyPreviewError` 只描述“缓存或预览准备失败”，不覆盖 HTTP 请求错误；HTTP 请求错误仍使用 `error`。
- `contentType` 保留响应头原值；分类时应先解析出不带参数的小写 MIME，例如 `text/html; charset=utf-8` 归一为 `text/html`。

## 缓存目录

缓存目录位于数据目录下：

```text
<dataDir>/api-workbench/response-cache/
```

缓存文件命名建议：

```text
<yyyyMMddHHmmss>-<hash-prefix>.<ext>
```

扩展名来源优先级：

1. `Content-Disposition` 的 filename。
2. URL 路径扩展名。
3. MIME 类型映射。
4. 字节特征识别。
5. `bin`。

写入规则：

- 只允许写入数据目录下的响应缓存目录。
- 创建缓存文件失败时，请求本身仍返回状态、响应头和错误提示；前端显示“缓存失败，无法预览二进制响应”。
- 对文本响应不写缓存文件。
- 对截断二进制不写预览文件，避免用户误以为文件完整。

路径安全规则：

- 后端提供 `get_api_workbench_response_cache_dir()` 类似 helper，集中创建和返回缓存目录。
- 所有接收 `filePath` 的 API Workbench action 都必须执行 `canonicalize` 后的目录前缀校验。
- 不允许通过 `..`、相对路径、设备命名空间或符号链接跳出响应缓存目录。
- 前端可以展示和复制缓存路径，但不能把用户可编辑路径直接传给 Office 解析或删除接口。
- 打开缓存文件和定位缓存文件应使用 API Workbench 专用受限 action，或在复用 `system.open_local_path` / `system.reveal_in_folder` 前先由 API Workbench 后端校验路径归属。

## 历史表扩展

在 API Workbench 历史表中增加缓存引用字段，保持兼容迁移：

- `response_body_storage TEXT NOT NULL DEFAULT 'text'`
- `response_body_file_path TEXT NOT NULL DEFAULT ''`
- `response_body_file_name TEXT NOT NULL DEFAULT ''`
- `response_body_extension TEXT NOT NULL DEFAULT ''`
- `response_body_hash TEXT NOT NULL DEFAULT ''`
- `response_preview_error TEXT`

历史保存规则：

- 文本响应继续保存现有 `response_body_preview`，遵守现有截断限制。
- 二进制响应保存缓存文件引用、hash、大小和类型，不把文件内容写入数据库。
- 历史详情返回缓存元信息，前端可重新预览缓存文件。
- 历史重放生成新的响应结果和新的历史记录；不会复用旧历史的响应缓存作为新响应。

历史清理规则：

- 删除单条历史时，删除该历史引用的缓存文件。
- 清空历史时，删除所有被清理历史引用的缓存文件。
- 清理未收藏历史时，只删除未收藏且被清理历史引用的缓存文件，收藏历史的缓存保留。
- 删除缓存前检查是否仍有其他历史记录引用同一路径或同一 hash。仍被引用时不删除。
- 缓存文件不存在时，历史删除仍成功。
- 缓存删除失败不应导致历史删除回滚；返回或记录警告即可。

当前代码已有 `history_clear`，但没有单条历史删除 action。若前端要提供单条删除入口，应新增 `history_delete`，并复用同一套缓存引用清理函数。

自动裁剪规则：

- 当前 `insert_history_with_conn` 会按 `MAX_HISTORY_ROWS` 裁剪未标星历史。新增缓存后，裁剪不能只执行 `DELETE`。
- 裁剪前先查询将被删除历史的缓存引用，删除历史记录后再按引用计数清理缓存文件。
- 引用计数应基于数据库中剩余历史记录的 `response_body_file_path` 或 `response_body_hash` 判断。
- 裁剪缓存失败不回滚新历史写入，但返回值中可带 `cacheWarnings`，前端可在需要时提示。
- 标星历史不参与自动裁剪，因此其缓存文件也应保留。

## 内容类型映射

分类优先级：

1. 响应头 MIME。
2. 缓存文件扩展名或 URL 文件名。
3. 字节特征。
4. 文本可解析性兜底。

主要映射：

- `json`：`application/json`、`+json`、可解析 JSON 文本。
- `html`：`text/html`、`application/xhtml+xml`。
- `image`：`image/*`。SVG 作为受限图片或源码视图处理，避免脚本风险。
- `pdf`：`application/pdf` 或 `%PDF-` 文件头。
- `office-word`：`docx/doc/rtf/odt` 和 Word MIME。
- `office-sheet`：`xlsx/xls/csv/ods` 和 Excel MIME。
- `office-slides`：`pptx/ppt/odp` 和 PowerPoint MIME。
- `text`：XML、CSS、JS、Markdown、纯文本等源码视图。
- `binary`：其他未知二进制。
- `empty`：无响应体。
- `unsupported`：有文件但当前无法预览。

## 前端组件设计

新增组件：

- `ApiWorkbenchResponseViewer.vue`

新增工具函数：

- `utils/apiWorkbenchResponsePreview.ts`
- 对应测试 `utils/apiWorkbenchResponsePreview.test.ts`

`ApiWorkbenchResponseViewer.vue` 输入完整响应对象，负责渲染响应内容区。它不发请求、不修改历史，只处理展示和本地操作。

响应页签结构：

- 顶部摘要：状态码、耗时、大小、Content-Type、缓存状态、截断状态。
- 响应内容：默认进入最合适的“预览”模式。
- 模式切换：预览、原文 / 源码、元信息。
- 响应头：沿用现有响应头页签。
- 历史：沿用现有历史页签；历史详情载入响应后使用同一个 Viewer。

常用操作：

- 复制响应体。
- 复制最终 URL。
- 复制缓存路径。
- 打开缓存文件。
- 保存为示例响应。

示例响应保存规则：

- 文本、JSON、HTML 等文本类响应继续按现有方式保存响应体示例。
- 二进制响应首版只保存元信息摘要，不直接保存历史缓存文件引用。
- 原因是历史缓存会随历史清理被删除；如果接口示例直接引用历史缓存，会产生悬空引用。
- 后续若要支持二进制示例可重新预览，应单独设计 request-owned 示例缓存目录，并把请求删除、示例覆盖和缓存清理纳入引用计数。

## 各类型渲染策略

### JSON

- 对 `bodyText` 执行 `JSON.parse` 和 `JSON.stringify(value, null, 2)`。
- 解析失败时回退原文，并显示“不是合法 JSON”提示。
- 保留复制格式化内容能力。

### HTML

- 默认使用 `iframe sandbox` + `srcdoc`。
- 不加 `allow-scripts`。
- 不主动补全远程相对资源，不把预览做成浏览器。
- 源码模式展示 HTML 文本。

### 图片

- 缓存文件路径通过 Tauri `convertFileSrc()` 转成本地 URL。
- 使用 `<img>` 预览。
- 首版只做适应容器展示；缩放、旋转可以后续增强。
- 加载失败时显示文件信息和打开文件按钮。

### PDF

- 缓存文件路径通过 `convertFileSrc()` 转成本地 URL。
- 使用 `<iframe>` 或 `<object>` 进行应用内预览。
- 加载失败时显示文件信息和打开文件按钮。
- 不引入 `pdfjs-dist`，避免额外资源和 worker 配置。

### Word

首版优先支持 `docx`：

- 后端校验缓存路径后读取文件。
- 使用轻量 `zip` crate 解包 OpenXML，复用现有 `quick-xml` 提取文本。
- 提取正文段落、标题、表格文本和图片引用信息。
- 渲染为可读文档流。

旧版 `doc`、复杂 `rtf/odt`：

- 能提取文本则展示文本。
- 不能可靠解析时显示“基础预览暂不支持该格式”，并提供打开文件。

### Excel

优先使用后端现有 Rust 依赖 `calamine` 解析表格文件：

- 返回工作表列表。
- 返回当前工作表前 N 行、前 M 列。
- 前端展示工作表切换和表格。
- 大表格限制行列数量，避免前端卡顿。

CSV 可以作为文本 / 表格两种方式预览；默认走表格预览。

### PowerPoint

首版支持 `pptx` 基础内容提取：

- 后端校验缓存路径后读取文件。
- 使用轻量 `zip` crate 解包 OpenXML，复用现有 `quick-xml` 提取文本。
- 提取幻灯片标题、正文文本、备注文本、图片数量。
- 按幻灯片卡片展示。

旧版 `ppt` 或复杂格式：

- 能提取文本片段则展示。
- 否则回退文件信息和打开文件。

## 后端 Office 解析接口

为了避免把大文件通过通用 IPC 全量传到前端，Office 预览优先由后端提供解析 action。

建议新增 action：

- `response_preview_office`
- `response_cache_open`
- `response_cache_reveal`
- `history_delete`（仅当本次提供单条历史删除入口时需要）

Office 解析输入：

```json
{
  "filePath": "...",
  "kind": "word|sheet|slides"
}
```

缓存文件操作输入：

```json
{
  "filePath": "..."
}
```

输出按类型区分：

- Word：标题、段落、表格文本、图片计数。
- Sheet：工作表列表、当前工作表窗口数据、总行列数。
- Slides：幻灯片列表、标题、文本、备注、图片计数。

后端必须校验 `filePath` 位于 API Workbench 响应缓存目录下，禁止读取任意路径。

分页和窗口规则：

- Sheet 默认返回第一个工作表的前 200 行、前 50 列。
- Sheet 支持 `sheetName`、`offset`、`limit` 参数，避免一次性把大表格推到前端。
- Word 默认限制段落和表格文本总字符数，例如 200KB；超过后返回 `truncated=true`。
- Slides 默认限制幻灯片数量，例如前 100 页；超过后返回 `truncated=true`。
- 解析失败只影响预览，不影响缓存文件打开、定位和历史查看。

## 安全边界

- HTML 预览不执行脚本。
- SVG 不直接作为可执行 HTML 注入。
- Office 解析只读取缓存目录内文件。
- 打开或定位缓存文件走明确用户动作，不自动打开外部程序。
- 不把响应体中的远程资源当作可信应用资源。
- 所有缓存路径操作都要校验位于数据目录下的响应缓存目录。
- `docx/pptx` 解析只读取必要 XML 和媒体计数，不展开任意路径，不执行宏，不解析外部关系目标。
- HTML `iframe sandbox` 不加 `allow-scripts`，也不加 `allow-same-origin`，除非后续有明确安全评估。

## 错误处理

- JSON 格式化失败：回退原文，并显示提示。
- HTML 预览异常：回退源码。
- 图片 / PDF 加载失败：显示文件信息和打开文件按钮。
- Office 解析失败：显示“基础预览失败”和错误摘要，保留原文件操作。
- 缓存写入失败：响应状态和响应头仍展示；二进制预览显示缓存失败。
- 缓存文件丢失：历史详情显示元信息，并提示缓存文件不存在。
- 历史清理中的缓存删除失败：历史清理继续完成，返回警告。
- Office 依赖缺失或格式暂不支持：显示“该格式暂不支持基础预览”，不要伪装为空文档。

## 影响文件

预计涉及：

- `apps/desktop/src-tauri/src/tools/api_workbench.rs`
- `apps/desktop/src/bridge/tauri.ts`
- `apps/desktop/src/types/api-workbench.ts`
- `apps/desktop/src/components/ApiWorkbenchPanel.vue`
- `apps/desktop/src/components/ApiWorkbenchResponseViewer.vue`
- `apps/desktop/src/utils/apiWorkbench.ts`
- `apps/desktop/src/utils/apiWorkbenchResponsePreview.ts`
- `apps/desktop/src/utils/apiWorkbenchResponsePreview.test.ts`
- `apps/desktop/src/utils/apiWorkbench.test.ts`
- `apps/desktop/src-tauri/Cargo.toml`（如支持 `docx/pptx`，新增轻量 `zip` crate）

如果实现中发现 Office 解析逻辑较大，应拆到 Rust 子模块或前端独立工具文件，避免继续膨胀 `ApiWorkbenchPanel.vue`。

建议拆分：

- Rust：若 Office 或缓存逻辑超过小型 helper 范围，拆到 `api_workbench_response.rs` 或 `api_workbench_preview.rs`，`api_workbench.rs` 只保留 action 分发和主业务编排。
- 前端：`ApiWorkbenchPanel.vue` 只持有 `response`、`responseTab`、历史载入和操作回调；展示判断、模式选择和预览错误态放入 `ApiWorkbenchResponseViewer.vue`。
- 纯函数：内容类型到 Viewer 的映射、默认模式选择、文件操作按钮可用性放入 `apiWorkbenchResponsePreview.ts` 并配套测试。

## 验证计划

前端：

- 内容类型分类测试。
- JSON 格式化和非法 JSON 回退测试。
- 响应模式默认选择测试。
- 缓存缺失提示测试。
- Viewer 组件的基础渲染测试按影响面补充。

后端：

- 文本响应不写缓存。
- 二进制响应写缓存并返回文件元信息。
- 截断二进制不生成可预览缓存文件。
- 历史保存缓存引用。
- 删除单条历史清理缓存。
- 清空历史清理缓存。
- 清理未收藏历史只清理对应缓存。
- 自动裁剪历史时清理被裁剪历史的缓存。
- 多历史共享同一缓存时不误删仍被引用文件。
- 缓存路径校验拒绝缓存目录外文件。
- Office 解析 action 拒绝缓存目录外路径。
- `xlsx/csv/docx/pptx` 基础解析样例。

命令：

```bash
cargo test api_workbench -- --nocapture
pnpm test src/utils/apiWorkbench.test.ts src/utils/apiWorkbenchResponsePreview.test.ts
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
```

## 开放问题

1. 是否需要在后续版本提供“响应缓存管理”页面，显示缓存占用和手动清理入口。
2. 是否需要给 PDF 引入本地 `pdfjs-dist`，以获得更一致的跨 WebView 预览效果。
3. 是否需要为 Office 高保真预览预留服务端转换扩展点。

这些问题不阻塞首版实现。
