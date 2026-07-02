# API Mock Content-Type 选择与内容校验设计

## 背景

API Mock 当前路由表单的 `Content-Type` 是普通文本输入框。用户可以填写任意值，但常见类型需要手写，文件响应也只在导入时按文件名推断并回写。这个行为可用，但对日常 Mock 响应不够直接，且缺少对响应内容和 `Content-Type` 是否匹配的提醒。

本次优化目标是增强前端表单体验：支持直接选择常见 `Content-Type`，继续允许自定义值，并在保存或导入文件时提醒用户上传和填写正确的响应内容。

## 目标

1. 路由表单支持从常见 `Content-Type` 列表中直接选择。
2. 用户仍可输入自定义 `Content-Type`。
3. 对可确定的内容错误进行阻断，例如 JSON 类型但响应 Body 不是合法 JSON。
4. 对可能不匹配但无法可靠判断的场景给出提醒，不阻断高级用法。
5. 保持现有持久化和后端响应模型不变。

## 非目标

1. 不修改 SQLite schema。
2. 不修改 `types/api-mock.ts` 中的 `contentType: string` 结构。
3. 不修改 Rust `api_mock.rs` 的保存和响应逻辑。
4. 不实现 Body 模板、自动格式化或自动替换示例内容。
5. 不限制文件上传类型。
6. 不根据 `Content-Type` 自动生成响应内容。

## 交互设计

`ApiMockPanel.vue` 中的 `Content-Type` 控件从 `el-input` 改为 `el-select`：

- 启用 `filterable`，方便搜索。
- 启用 `allow-create`，允许用户输入自定义 MIME。
- 启用 `clearable`，允许用户清空并依赖后端/文件兜底逻辑。
- 选项展示为“标签 + MIME 值”，保存时仍只保存 MIME 字符串。

常见列表采用偏完整覆盖，放在 `apps/desktop/src/utils/apiMock.ts`：

- `application/json; charset=utf-8`
- `application/json`
- `text/plain; charset=utf-8`
- `text/html; charset=utf-8`
- `application/xml`
- `text/xml; charset=utf-8`
- `text/csv; charset=utf-8`
- `application/x-www-form-urlencoded`
- `multipart/form-data`
- `image/png`
- `image/jpeg`
- `image/svg+xml`
- `image/webp`
- `image/gif`
- `application/pdf`
- `application/zip`
- `application/wasm`
- `application/octet-stream`
- `text/css; charset=utf-8`
- `text/javascript; charset=utf-8`

## 内容校验与提醒

新增前端纯函数处理响应内容检查，组件只负责调用和展示结果。

### Content-Type 值校验

保存路由前先校验 `Content-Type` 字符串本身：

1. 保存时对值做 `trim`，避免前后空白进入响应头。
2. 空值允许保存，沿用现有后端兜底逻辑。
3. 包含 CR / LF 的值必须阻断，避免生成非法 HTTP header。
4. 非空值至少应包含 MIME 主类型和子类型结构，例如 `type/subtype`；带参数的值继续允许，例如 `application/json; charset=utf-8`。
5. 校验仅约束 header 值的基本形态，不维护完整 IANA MIME 白名单，自定义厂商类型和私有类型继续允许。

### 静态 Body

保存路由前，根据归一化后的 MIME 判断：

1. `application/json`：尝试 `JSON.parse`。失败时阻断保存，提示“当前 Content-Type 是 JSON，但响应 Body 不是合法 JSON”。
2. `application/xml` / `text/xml`：不做严格 XML 解析，只提示用户确认 XML 内容结构正确。
3. `text/html`：不做严格校验，只提示用户确认返回内容是 HTML。
4. `application/x-www-form-urlencoded` / `multipart/form-data`：提示这类类型通常用于请求体，作为响应类型使用时需要确认是否符合预期。
5. 其他类型：不阻断保存。

只有 JSON Body 这种可以可靠判断的错误阻断保存。其他提醒使用 warning，不伪装成成功校验。

### 文件响应

导入文件时继续使用现有文件名推断逻辑。若用户当前选择的 `Content-Type` 和文件推断类型不一致，显示 warning：

```text
上传文件看起来是 image/png，当前 Content-Type 是 application/pdf，请确认是否正确。
```

保存文件响应时不做严格阻断，原因是文件扩展名和实际内容可能不一致，也存在无扩展名、自定义二进制或服务端生成文件的真实场景。

`application/octet-stream` 视为通用二进制，不触发不匹配提醒。

## 数据流

1. 用户选择或输入 `Content-Type`。
2. 保存前先校验并 trim `Content-Type` 值。
3. 静态 Body 保存前调用内容校验函数。
4. 若校验返回 error，组件显示错误并停止保存。
5. 若校验返回 warning，组件显示提醒并继续保存。
6. 文件导入时先按文件名推断 MIME，再根据当前 `Content-Type` 决定是否回写或提醒。
7. 路由保存 payload 继续传 `contentType: string`。
8. 后端继续按现有逻辑保存字符串，并在运行响应时补 `Content-Type` header。

## 实现边界

涉及文件：

- `apps/desktop/src/components/ApiMockPanel.vue`
- `apps/desktop/src/utils/apiMock.ts`
- `apps/desktop/src/utils/apiMock.test.ts`

不涉及文件：

- `apps/desktop/src/types/api-mock.ts`
- `apps/desktop/src-tauri/src/tools/api_mock.rs`
- 数据库迁移代码

## 测试计划

前端单测：

```bash
pnpm test src/utils/apiMock.test.ts
```

覆盖：

1. 常见 `Content-Type` 预设包含接口、文本、图片、PDF、压缩包、WASM 和通用二进制类型。
2. MIME 归一化会忽略参数并统一小写。
3. `Content-Type` 前后空白会被 trim。
4. 包含 CR / LF 的 `Content-Type` 返回 error。
5. 非空但不符合 `type/subtype` 基本形态的值返回 error。
6. `application/json` + 非法 Body 返回 error。
7. `application/json` + 合法 Body 通过。
8. XML、HTML、Form、Multipart 返回 warning 而不是 error。
9. 文件推断类型和当前类型不一致时返回 warning。
10. 当前类型为 `application/octet-stream` 时不提示文件不匹配。
11. 自定义 `Content-Type` 不被预设列表限制。

工程验证：

```bash
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
```

## 风险与取舍

1. XML/HTML 不做严格解析，避免引入复杂解析器或误伤合法片段。
2. 文件响应只提醒不阻断，避免扩展名推断不准确导致用户无法保存。
3. 只在前端做体验增强，后端保持字符串透传，兼容已有数据和运行行为。
4. `multipart/form-data` 作为响应类型并不常见，但保留给联调特殊场景，同时通过 warning 提醒用户确认。
